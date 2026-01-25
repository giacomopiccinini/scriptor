use crate::configs::db::DBConfig;
use crate::configs::scriptor::ScriptorConfig;
use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::playback::Player;
use crate::stt::rec::RecorderConfig;
use crate::stt::vad::VADModel;
use crate::tui::app::events::EventHandler;
use crate::tui::db::connections::init_db;
use crate::tui::ui::components::{
    AddArchivumPopUp, AddCodexPopUp, AddFolioPopUp, ChangeArchivumPopUp, CodicesComponent,
    FragmentaComponent, InputState, ModifyCodexPopUp, ModifyFolioPopUp, RecordingScreen,
};
use crate::tui::ui::cursor::CursorState;
use crate::tui::ui::layout::AppLayout;
use crate::utils::aws::{ModelsConfig, download_missing_files, download_models_list};
use anyhow::Context;
use color_eyre::Result;
use crossterm::event::{self, KeyEvent};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use spinoff::{Color, Spinner, Streams, spinners};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;

/// Enum representing the different screens in the application
#[derive(Debug, Clone, PartialEq)]
pub enum CurrentScreen {
    /// Main screen showing codices and folia
    Main,
    /// Pop-up screen for adding a new
    AddCodex,
    /// Pop-up screen for modifying an existing codex
    ModifyCodex,
    /// Pop-up screen for adding a new folio (from path)
    AddFolio,
    /// Pop-up screen for modifying an existing folio
    ModifyFolio,
    /// Pop-up screen for recording a folio
    RecordFolio,
    /// Pop-up for changing archivum
    ChangeArchivum,
    /// Pop-up for adding a new archivum
    AddArchivum,
}

/// Current region to signal where the cursor is
#[derive(Debug, Clone, PartialEq)]
pub enum CurrentRegion {
    CodexAndFolio,
    Fragmentum,
}

/// All necessary tools to handle Speech to Text
pub struct STTTools {
    pub fractor: Option<Fractor>,
    pub stt_model: Option<STTModel>,
    pub player: Player,
    /// Max queue elements for fragmentum channel
    pub max_queue_elements: usize,
}

/// Main application state
pub struct App {
    /// App configuration (db, stt inference parameters, theme)
    pub config: ScriptorConfig,
    // Collection of tools to perform speech to text
    pub stt_tools: STTTools,
    /// Current active screen
    pub current_screen: CurrentScreen,
    /// Current active region
    pub current_region: CurrentRegion,
    /// Database (archivum) connection pool
    pub pool: SqlitePool,
    ///  Component for managing codices
    pub codices_component: CodicesComponent,
    /// State of user-provided input
    pub input_state: InputState,
    /// Flag to signal if user is recording
    pub is_recording: bool,
    /// Flag to signal if recording is paused
    pub is_paused: bool,
    /// Flag to signal if inference is running
    pub is_transcribing: bool,
    /// Selected archivum index for Archivum selector
    pub selected_archivum_index: usize,
    /// ID of the folio being recorded to (when recording)
    pub recording_folio_id: Option<i64>,
    /// Stop signal for the recording thread
    pub recording_stop_signal: Option<Arc<AtomicBool>>,
    /// Pause signal for the recording thread
    pub recording_pause_signal: Option<Arc<AtomicBool>>,
    /// Handle for fractor thread (returns temp dir path and VADModel for reuse)
    pub fractor_handle: Option<JoinHandle<anyhow::Result<(Option<PathBuf>, VADModel)>>>,
    /// Handle for transcriber thread (returns STTModel for reuse)
    pub transcriber_handle: Option<JoinHandle<anyhow::Result<STTModel>>>,
    /// Last observed playback file index (for tracking when to advance selection)
    pub last_playback_file_index: usize,
    /// Flag to indicate if the application should exit
    pub exit: bool,
}

impl STTTools {
    pub fn new(config: &ScriptorConfig) -> anyhow::Result<Self> {
        // Load STT model
        let stt_model = STTModel::new(&config.default.stt, config.default.inference.clone())?;

        // Create recorder config (actual stream created inside thread for macOS compatibility)
        let recorder_config =
            RecorderConfig::new(config.default.fractor.max_fragmentum_duration_seconds)
                .with_context(|| "Failed to create recorder config")?;

        // Create VAD model
        let vad_model = VADModel::new(&config.default.vad, config.default.inference.clone())
            .with_context(|| "Failed to create voice activity detector")?;

        // Create fractor
        let fractor = Fractor::new(recorder_config, vad_model);

        // Create the player with no files in the queue
        let player = Player::new(None).with_context(|| "Unable to setup player")?;

        Ok(Self {
            fractor: Some(fractor),
            stt_model: Some(stt_model),
            player,
            max_queue_elements: config.default.queue.max_queue_elements,
        })
    }

    /// Restore models after recording completes by reusing the existing VAD and STT models.
    /// Only recreates the cheap RecorderConfig and Fractor wrapper.
    pub fn restore_from_recording(
        &mut self,
        config: &ScriptorConfig,
        vad_model: VADModel,
        stt_model: STTModel,
    ) -> anyhow::Result<()> {
        // Create recorder config (actual stream created inside thread for macOS compatibility)
        // This is cheap - just configuration, no model loading
        let recorder_config =
            RecorderConfig::new(config.default.fractor.max_fragmentum_duration_seconds)
                .with_context(|| "Failed to create recorder config")?;

        // Create fractor with the existing VAD model (no model reloading!)
        let fractor = Fractor::new(recorder_config, vad_model);

        // Restore models
        self.fractor = Some(fractor);
        self.stt_model = Some(stt_model);

        Ok(())
    }
}

impl App {
    /// Create new app instance
    ///
    /// Initializes the archivum connection, loads existing codices from the archivum,
    /// and sets up the initial UI state.
    pub async fn new() -> Self {
        // Read the config (creates default if missing)
        let config = ScriptorConfig::read().expect("Failed to read config file");

        // Fetch the list of available models
        let available_models = match ModelsConfig::read() {
            Ok(config) => config,
            Err(_) => {
                download_models_list()
                    .await
                    .expect("Failed to download available models list");
                ModelsConfig::read().expect("Failed to read models config after download")
            }
        };

        // Check if files are missing and, if so, download them
        if let Some(missing_files) = config.check_missing(&available_models) {
            let mut spinner = Spinner::new_with_stream(
                spinners::Dots,
                "Downloading models...",
                Color::Blue,
                Streams::Stderr,
            );
            download_missing_files(&missing_files).await;
            spinner.success("Models downloaded!");
        };

        // Create STT tools. Add a spinner first, as it might take a while and we want the user to be aware.
        // Create spinner
        let mut spinner = Spinner::new_with_stream(
            spinners::Dots,
            "Loading models...",
            Color::Blue,
            Streams::Stderr,
        );
        let stt_tools = STTTools::new(&config).expect("Failed to setup STT tools");
        spinner.success("STT tools loaded!");

        // Connect to default archivum (db)
        let pool = init_db(&config.default.db.connection_str)
            .await
            .expect("Failed to connect to archivum");

        // Start from main screen
        let current_screen = CurrentScreen::Main;

        // Create codices component and load data
        let mut codices_component = CodicesComponent::new();
        codices_component
            .load_codices(&pool)
            .await
            .expect("Failed to read codices");

        // Return initial state of the app
        Self {
            config,
            stt_tools,
            current_screen,
            current_region: CurrentRegion::CodexAndFolio,
            pool,
            codices_component,
            input_state: InputState::new(),
            is_recording: false,
            is_paused: false,
            is_transcribing: false,
            selected_archivum_index: 0,
            recording_folio_id: None,
            recording_stop_signal: None,
            recording_pause_signal: None,
            fractor_handle: None,
            transcriber_handle: None,
            last_playback_file_index: 0,
            exit: false,
        }
    }

    /// Run the application
    ///
    /// Main event loop that handles terminal drawing and user input.
    /// Continues until the user exits the application.
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        use std::time::Duration;

        while !self.exit {
            // Draw the current state of the application
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            // Use non-blocking poll when recording or playing to allow periodic updates
            let poll_timeout = if self.is_recording || self.stt_tools.player.is_playing {
                Duration::from_millis(100) // Fast refresh during recording or playback
            } else {
                Duration::from_secs(60) // Longer timeout when idle
            };

            // Poll for events with timeout
            if event::poll(poll_timeout)?
                && let Some(key) = event::read()?.as_key_press_event()
            {
                self.handle_key_event(key).await;
            }

            // Periodic refresh of fragmenta during recording
            if self.is_recording
                && let Some(selected_codex) = self.codices_component.get_selected_codex_mut()
                && let Some(folio_idx) = selected_codex.folio_state.selected()
                && let Some(selected_folio) = selected_codex.folia.get_mut(folio_idx)
            {
                // Refresh fragmenta from DB (ignore errors during recording)
                let _ = selected_folio.update_fragmenta(&self.pool).await;
            }

            // Sync UI with playback progress
            if self.stt_tools.player.is_playing {
                // Check if file index advanced
                let current_idx = self.stt_tools.player.queue.current();
                if current_idx > self.last_playback_file_index {
                    // Advance fragmentum selection
                    if let Some(selected_codex) = self.codices_component.get_selected_codex_mut()
                        && let Some(folio_idx) = selected_codex.folio_state.selected()
                        && let Some(selected_folio) = selected_codex.folia.get_mut(folio_idx)
                    {
                        // Jump to the current file idx. This prevents that moving up/down with the arrows
                        // renders the selected fragmentum out of sync with the player
                        FragmentaComponent::jump_to_fragmentum(selected_folio, current_idx);
                    }
                    self.last_playback_file_index = current_idx;
                }

                // Auto-stop when queue finishes
                if self.stt_tools.player.queue.is_queue_finished() {
                    let _ = self.stt_tools.player.pause();
                }

                // Trigger preload if needed
                let _ = self.stt_tools.player.check_and_preload();
            }
        }
        Ok(())
    }

    /// Create a new archivum with the given name
    pub async fn create_new_archivum(
        &mut self,
        archivum_name: String,
        set_as_default: bool,
    ) -> Result<()> {
        // Use data directory to standardize storage
        let data_dir = dirs::data_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not find data directory"))?
            .join("scriptor");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to create data directory: {}", e))?;

        // Create path to new archivum file
        let archivum_file = format!("{}.db", archivum_name);
        let path = data_dir.join(archivum_file);

        // Create connection string (only SQLite is admissible)
        let connection_str = format!("sqlite:{}", path.display());

        // Create new archivum config
        let new_archivum_config = DBConfig {
            name: archivum_name.clone(),
            connection_str: connection_str.clone(),
        };

        // Initialize the new archivum (this creates the file and runs migrations)
        init_db(&connection_str)
            .await
            .map_err(|e| color_eyre::eyre::eyre!("Failed to initialize new archivum: {}", e))?;

        // Add to config
        self.config.dbs.push(new_archivum_config);

        // Set as default if requested
        if set_as_default {
            self.config.default.db.name = archivum_name.clone();
        }

        // Write updated config to file
        let config_dir = dirs::config_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?
            .join("scriptor");
        let config_path = config_dir.join("scriptor.toml");

        self.config
            .write(&config_path)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to save config: {}", e))?;

        // Update selected index to point to the new archivum
        self.selected_archivum_index = self.config.dbs.len() - 1;

        Ok(())
    }

    /// Handle key events and delegate to appropriate handler
    async fn handle_key_event(&mut self, key: KeyEvent) {
        match self.current_screen {
            CurrentScreen::Main => EventHandler::handle_main_screen_key(self, key).await,
            CurrentScreen::AddCodex | CurrentScreen::ModifyCodex => {
                EventHandler::handle_add_or_modify_codex_screen_key(self, key).await
            }
            CurrentScreen::AddFolio | CurrentScreen::ModifyFolio => {
                EventHandler::handle_add_or_modify_folio_screen_key(self, key).await
            }
            CurrentScreen::RecordFolio => {
                EventHandler::handle_record_folio_screen_key(self, key).await
            }
            CurrentScreen::ChangeArchivum => {
                EventHandler::handle_change_archivum_screen_key(self, key).await
            }
            CurrentScreen::AddArchivum => {
                EventHandler::handle_add_archivum_screen_key(self, key).await
            }
        }
    }

    /// Enter the "Add Codex" screen by opening the corresponding pop-up
    pub fn enter_add_codex_screen(&mut self) {
        self.input_state = InputState::default();
        self.current_screen = CurrentScreen::AddCodex;
    }

    /// Enter the "Modify Codex" screen by opening the corresponding pop-up
    pub fn enter_modify_codex_screen(&mut self, name: String) {
        self.input_state = InputState {
            current_input: name,
            cursor_pos: 0,
            is_modifying: true,
        };
        self.current_screen = CurrentScreen::ModifyCodex;
    }

    /// Enter the "Modify Folio" screen by opening the corresponding pop-up
    pub fn enter_modify_folio_screen(&mut self, name: String) {
        self.input_state = InputState {
            current_input: name,
            cursor_pos: 0,
            is_modifying: true,
        };
        self.current_screen = CurrentScreen::ModifyFolio;
    }

    /// Enter the "Add Folio" screen by opening the corresponding pop-up
    pub fn enter_add_folio_screen(&mut self) {
        if self.codices_component.codex_state.selected().is_some() {
            self.input_state = InputState::default();
            self.current_screen = CurrentScreen::AddFolio;
        }
    }

    /// Exit the Add Codex screen without saving
    pub fn exit_add_or_modify_codex_without_saving(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.input_state.clear();
    }

    /// Exit the Add Folio screen without saving
    pub fn exit_add_or_modify_folio_without_saving(&mut self) {
        self.current_screen = CurrentScreen::Main;
        self.input_state.clear();
    }

    /// Enter the "Change Archivum" screen by opening the corresponding pop-up
    pub fn enter_change_archivum_screen(&mut self) {
        // Find the index of the current archivum in the config
        self.selected_archivum_index = self
            .config
            .dbs
            .iter()
            .position(|archivum| archivum.name == self.config.default.db.name)
            .unwrap_or(0);
        self.current_screen = CurrentScreen::ChangeArchivum;
    }

    /// Exit the Change Archivum screen without saving
    pub fn exit_change_archivum_without_saving(&mut self) {
        self.current_screen = CurrentScreen::Main;
    }

    /// Enter the "Add Archivum" screen by opening the corresponding pop-up
    pub fn enter_add_archivum_screen(&mut self) {
        self.current_screen = CurrentScreen::AddArchivum;
    }

    /// Exit the Add Archivum screen without saving
    pub fn exit_add_archivum_without_saving(&mut self) {
        self.current_screen = CurrentScreen::ChangeArchivum;
        self.input_state.clear();
    }

    /// Move selection up in Archivum codex
    pub fn select_previous_archivum(&mut self) {
        if self.config.dbs.is_empty() {
            return;
        }
        self.selected_archivum_index = if self.selected_archivum_index == 0 {
            self.config.dbs.len() - 1
        } else {
            self.selected_archivum_index - 1
        };
    }

    /// Move selection down in Archivum codex
    pub fn select_next_archivum(&mut self) {
        if self.config.dbs.is_empty() {
            return;
        }
        self.selected_archivum_index = (self.selected_archivum_index + 1) % self.config.dbs.len();
    }

    /// Switch to the selected archivum
    pub async fn switch_to_selected_archivum(&mut self) -> Result<()> {
        if let Some(selected_archivum) = self.config.dbs.get(self.selected_archivum_index) {
            // Initialize connection to the new archivum
            let new_pool = init_db(&selected_archivum.connection_str)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to connect to archivum: {}", e))?;

            // Update app state
            self.config.default.db = selected_archivum.clone();
            self.pool = new_pool;

            // Reload all codices from the new archivum
            self.codices_component = CodicesComponent::new();
            self.codices_component
                .load_codices(&self.pool)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to load codices: {}", e))?;

            // Return to main screen
            self.current_screen = CurrentScreen::Main;
        }
        Ok(())
    }

    /// Set the selected archivum as default
    pub async fn set_selected_archivum_as_default(&mut self) -> Result<()> {
        if let Some(selected_archivum) = self.config.dbs.get(self.selected_archivum_index) {
            // Update the default in config
            self.config.default.db.name = selected_archivum.name.clone();

            // Write updated config to file
            let config_dir = dirs::config_dir()
                .ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?
                .join("scriptor");
            let config_path = config_dir.join("scriptor.toml");

            self.config
                .write(&config_path)
                .map_err(|e| color_eyre::eyre::eyre!("Failed to save config: {}", e))?;
        }
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Get theme reference
        let theme = &self.config.default.theme;

        // RecordFolio is a full-screen mode
        if self.current_screen == CurrentScreen::RecordFolio {
            // Get the selected folio for displaying fragmenta
            let selected_folio =
                if let Some(codex) = self.codices_component.get_selected_codex_mut() {
                    if let Some(folio_idx) = codex.folio_state.selected() {
                        codex.folia.get(folio_idx)
                    } else {
                        None
                    }
                } else {
                    None
                };

            RecordingScreen::render(self.is_paused, selected_folio, area, buf, theme);
            return;
        }

        // Render background
        AppLayout::render_background(area, buf, theme);

        // Calculate layout areas
        let (
            codices_header_area,
            codices_area,
            bookmark_area,
            fragmenta_header_area,
            fragmenta_area,
        ) = AppLayout::calculate_main_layout(area);

        // Render column headers
        AppLayout::render_header(codices_header_area, buf, "C O D I C E S", theme);
        AppLayout::render_header(fragmenta_header_area, buf, "F R A G M E N T A", theme);

        // Render bookmark area (archivum selector in the middle)
        AppLayout::render_bookmark(bookmark_area, buf, "   A R C H I V U M", theme);

        // Render the main areas
        self.codices_component.render(codices_area, buf, theme);

        // Render fragmenta with the selected folio
        let selected_folio = if let Some(codex) = self.codices_component.get_selected_codex_mut() {
            if let Some(folio_idx) = codex.folio_state.selected() {
                codex.folia.get_mut(folio_idx)
            } else {
                None
            }
        } else {
            None
        };
        FragmentaComponent::render(
            selected_folio,
            self.stt_tools.player.is_playing,
            fragmenta_area,
            buf,
            theme,
        );

        // Render popup screens if active
        match self.current_screen {
            CurrentScreen::AddCodex => {
                AddCodexPopUp::render(&self.input_state, codices_area, buf, theme)
            }
            CurrentScreen::ModifyCodex => {
                ModifyCodexPopUp::render(&self.input_state, codices_area, buf, theme)
            }
            CurrentScreen::AddFolio => {
                AddFolioPopUp::render(&self.input_state, codices_area, buf, theme)
            }
            CurrentScreen::ModifyFolio => {
                ModifyFolioPopUp::render(&self.input_state, codices_area, buf, theme)
            }
            CurrentScreen::ChangeArchivum => ChangeArchivumPopUp::render(
                &self.config,
                self.selected_archivum_index,
                bookmark_area,
                buf,
                theme,
            ),
            CurrentScreen::AddArchivum => {
                AddArchivumPopUp::render(&self.input_state, bookmark_area, buf, theme)
            }
            _ => {}
        }
    }
}
