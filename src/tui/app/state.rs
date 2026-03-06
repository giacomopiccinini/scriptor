use crate::configs::db::DBConfig;
use crate::configs::scriptor::ScriptorConfig;
use crate::configs::settings::SettingsState;
use crate::configs::stt::AvailableSTTModel;
use crate::configs::vad::AvailableVADModel;
use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::playback::Player;
use crate::stt::rec::{RecorderConfig, enumerate_input_devices};
use crate::stt::vad::VADModel;
use crate::tui::app::events::EventHandler;
use crate::tui::db::connections::init_db;
use crate::tui::ui::components::{
    AddArchivumPopUp, AddCodexPopUp, AddFolioPopUp, ChangeArchivumPopUp, CodicesComponent,
    DeleteArchivumPopUp, DeleteCodexPopUp, DeleteFolioPopUp, FragmentaComponent, InputState,
    ModifyArchivumPopUp, ModifyCodexPopUp, ModifyFolioPopUp, RecordingScreen, SettingsScreen,
};
use crate::tui::ui::cursor::CursorState;
use crate::tui::ui::layout::AppLayout;
use crate::utils::aws::{ModelsConfig, download_missing_files, download_models_list};
use anyhow::Context;
use color_eyre::Result;
use crossterm::event::{self, KeyEvent};
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{ListState, Widget};
use spinoff::{Color, Spinner, Streams, spinners};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use std::time::Instant;

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
    /// Pop-up for modifying an existing archivum
    ModifyArchivum,
    /// Pop-up for deleting an archivum (confirmation required)
    DeleteArchivum,
    /// Pop-up for deleting a codex (confirmation required)
    DeleteCodex,
    /// Pop-up for deleting a folio (confirmation required)
    DeleteFolio,
    /// Settings screen for configuring input device and VAD threshold
    Settings,
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
    pub max_queue_elements: usize,
}

/// Main application state
pub struct App {
    /// App configuration (db, stt inference parameters, theme)
    pub config: ScriptorConfig,
    /// Available models from models.toml (cached at startup)
    pub available_models: ModelsConfig,
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
    /// Flag to toggle timestamp display on fragmenta
    pub show_timestamp: bool,
    /// State for the settings screen (populated when settings is opened)
    pub settings_state: Option<SettingsState>,
    /// List state for the recording overlay (separate from fragmentum_state for dots item)
    pub recording_list_state: ListState,
    /// When we entered RecordFolio screen (for animated dots)
    pub recording_screen_start: Option<Instant>,
}

impl STTTools {
    pub fn new(config: &ScriptorConfig) -> anyhow::Result<Self> {
        // Load STT model
        let stt_model = STTModel::new(&config.default.stt, config.default.inference.clone())?;

        // Create recorder config (actual stream created inside thread for macOS compatibility)
        let recorder_config = RecorderConfig::new(
            config.default.fractor.max_fragmentum_duration_seconds,
            config.default.input_device.as_deref(),
        )
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
        // This is cheap, just configuration, no model loading
        let recorder_config = RecorderConfig::new(
            config.default.fractor.max_fragmentum_duration_seconds,
            config.default.input_device.as_deref(),
        )
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
            available_models,
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
            show_timestamp: false,
            settings_state: None,
            recording_list_state: ListState::default(),
            recording_screen_start: None,
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
                // Autoscroll: select last fragmentum so both overlay and background scroll to show latest
                if !selected_folio.fragmenta.is_empty() {
                    selected_folio
                        .fragmentum_state
                        .select(Some(selected_folio.fragmenta.len() - 1));
                }
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
            .join("scriptor")
            .join("databases");

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

    /// Rename an existing archivum
    pub fn rename_archivum(&mut self, new_name: String) -> Result<()> {
        let selected = self
            .config
            .dbs
            .get(self.selected_archivum_index)
            .ok_or_else(|| color_eyre::eyre::eyre!("No archivum selected"))?
            .clone();

        let old_path = selected
            .connection_str
            .strip_prefix("sqlite:")
            .ok_or_else(|| color_eyre::eyre::eyre!("Invalid connection string"))?;
        let old_path = std::path::Path::new(old_path);

        let parent_dir = old_path
            .parent()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not get parent directory"))?;
        let new_path = parent_dir.join(format!("{}.db", new_name));
        let new_connection_str = format!("sqlite:{}", new_path.display());

        std::fs::rename(old_path, &new_path)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to rename archivum file: {}", e))?;

        let new_archivum_config = DBConfig {
            name: new_name.clone(),
            connection_str: new_connection_str,
        };
        self.config.dbs[self.selected_archivum_index] = new_archivum_config;

        if self.config.default.db.name == selected.name {
            self.config.default.db.name = new_name;
        }

        let config_dir = dirs::config_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?
            .join("scriptor");
        let config_path = config_dir.join("scriptor.toml");

        self.config
            .write(&config_path)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to save config: {}", e))?;

        Ok(())
    }

    /// Delete an archivum (actual sqlite file and config entry in scriptor.toml)
    pub async fn delete_archivum(&mut self) -> Result<()> {
        // Last archivum cannot be deleted
        if self.config.dbs.len() == 1 {
            return Err(color_eyre::eyre::eyre!("Cannot delete the last archivum"));
        }

        // Get the selected db
        let selected = self
            .config
            .dbs
            .get(self.selected_archivum_index)
            .ok_or_else(|| color_eyre::eyre::eyre!("No archivum selected"))?
            .clone();

        // Extract the path to the DB fro the connection string
        let file_path = selected
            .connection_str
            .strip_prefix("sqlite:")
            .ok_or_else(|| color_eyre::eyre::eyre!("Invalid connection string"))?;

        // Delete the file
        std::fs::remove_file(file_path)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to delete archivum file: {}", e))?;

        // Flag to signal if the deleted db was the actual default or not
        // In this case, we would need to promote an existing db to default
        let was_default = self.config.default.db.name == selected.name;

        // Drop the db from the config
        self.config.dbs.remove(self.selected_archivum_index);

        self.selected_archivum_index = self
            .selected_archivum_index
            .min(self.config.dbs.len().saturating_sub(1));

        // If it was default, the first remaining db becomes the default
        if was_default {
            let new_default = self
                .config
                .dbs
                .first()
                .ok_or_else(|| color_eyre::eyre::eyre!("No archivum remaining"))?
                .clone();

            self.config.default.db = new_default.clone();

            let new_pool = init_db(&new_default.connection_str)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to connect to archivum: {}", e))?;

            self.pool = new_pool;

            self.codices_component = CodicesComponent::new();
            self.codices_component
                .load_codices(&self.pool)
                .await
                .map_err(|e| color_eyre::eyre::eyre!("Failed to load codices: {}", e))?;
        }

        let config_dir = dirs::config_dir()
            .ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?
            .join("scriptor");
        let config_path = config_dir.join("scriptor.toml");

        self.config
            .write(&config_path)
            .map_err(|e| color_eyre::eyre::eyre!("Failed to save config: {}", e))?;

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
            CurrentScreen::AddArchivum | CurrentScreen::ModifyArchivum => {
                EventHandler::handle_add_or_modify_archivum_screen_key(self, key).await
            }
            CurrentScreen::DeleteArchivum => {
                EventHandler::handle_delete_archivum_screen_key(self, key).await
            }
            CurrentScreen::DeleteCodex => {
                EventHandler::handle_delete_codex_screen_key(self, key).await
            }
            CurrentScreen::DeleteFolio => {
                EventHandler::handle_delete_folio_screen_key(self, key).await
            }
            CurrentScreen::Settings => EventHandler::handle_settings_screen_key(self, key).await,
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
        self.input_state = InputState::default();
        self.current_screen = CurrentScreen::AddArchivum;
    }

    /// Exit the Add Archivum screen without saving
    pub fn exit_add_archivum_without_saving(&mut self) {
        self.current_screen = CurrentScreen::ChangeArchivum;
        self.input_state.clear();
    }

    /// Enter the "Modify Archivum" screen by opening the corresponding pop-up
    pub fn enter_modify_archivum_screen(&mut self, name: String) {
        self.input_state = InputState {
            current_input: name,
            cursor_pos: 0,
            is_modifying: true,
        };
        self.current_screen = CurrentScreen::ModifyArchivum;
    }

    /// Exit the Modify Archivum screen without saving
    pub fn exit_modify_archivum_without_saving(&mut self) {
        self.current_screen = CurrentScreen::ChangeArchivum;
        self.input_state.clear();
    }

    /// Enter the "Delete Archivum" screen by opening the confirmation pop-up
    pub fn enter_delete_archivum_screen(&mut self) {
        self.input_state = InputState::default();
        self.current_screen = CurrentScreen::DeleteArchivum;
    }

    /// Exit the Delete Archivum screen without saving
    pub fn exit_delete_archivum_without_saving(&mut self) {
        self.current_screen = CurrentScreen::ChangeArchivum;
        self.input_state.clear();
    }

    /// Enter the "Delete Codex" screen by opening the confirmation pop-up
    pub fn enter_delete_codex_screen(&mut self) {
        self.input_state = InputState::default();
        self.current_screen = CurrentScreen::DeleteCodex;
    }

    /// Enter the "Delete Folio" screen by opening the confirmation pop-up
    pub fn enter_delete_folio_screen(&mut self) {
        self.input_state = InputState::default();
        self.current_screen = CurrentScreen::DeleteFolio;
    }

    /// Exit the Delete Codex or Folio screen without saving
    pub fn exit_delete_codex_or_folio_without_saving(&mut self) {
        self.current_screen = CurrentScreen::Main;
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

    /// Enter the "Settings" screen by opening the settings overlay
    pub fn enter_settings_screen(&mut self) {
        // Enumerate available input devices
        let available_devices = enumerate_input_devices();

        // Get available models from cached config
        let available_stt_models = self.available_models.available_stt_models();
        let available_vad_models = self.available_models.available_vad_models();

        // Create settings state with current config values
        self.settings_state = Some(SettingsState::new(
            available_devices,
            self.config.default.input_device.as_deref(),
            self.config.default.vad.threshold,
            self.config.default.fractor.min_fragmentum_duration_seconds,
            self.config.default.fractor.max_fragmentum_duration_seconds,
            self.config.default.fractor.pause_threshold_in_chunks,
            available_stt_models,
            &self.config.default.stt.model,
            available_vad_models,
            &self.config.default.vad.model,
        ));

        self.current_screen = CurrentScreen::Settings;
    }

    /// Exit the Settings screen without saving (discard changes)
    pub fn exit_settings_without_saving(&mut self) {
        self.settings_state = None;
        self.current_screen = CurrentScreen::Main;
    }

    /// Save settings to current session only (don't write to file)
    pub async fn save_settings_to_session(&mut self) -> Result<()> {
        self.apply_settings_changes(false).await
    }

    /// Save settings to session and write to scriptor.toml file
    pub async fn save_settings_as_default(&mut self) -> Result<()> {
        self.apply_settings_changes(true).await
    }

    /// Apply settings changes, optionally writing to file
    async fn apply_settings_changes(&mut self, write_to_file: bool) -> Result<()> {
        let settings = match &self.settings_state {
            Some(s) => s.clone(),
            None => {
                self.current_screen = CurrentScreen::Main;
                return Ok(());
            }
        };

        // Check if STT model changed
        let stt_model_changed = settings
            .selected_stt_model_key()
            .map(|key| key != self.config.default.stt.model.as_key())
            .unwrap_or(false);

        // Check if VAD model changed
        let vad_model_changed = settings
            .selected_vad_model_key()
            .map(|key| key != self.config.default.vad.model.as_key())
            .unwrap_or(false);

        // Check if VAD threshold changed
        let vad_threshold_changed =
            (settings.vad_threshold - self.config.default.vad.threshold).abs() > 0.001;

        // Update config values
        self.config.default.vad.threshold = settings.vad_threshold;
        self.config.default.input_device = settings.selected_device_name().map(|s| s.to_string());
        self.config.default.fractor.min_fragmentum_duration_seconds =
            settings.min_fragmentum_duration_seconds;
        self.config.default.fractor.max_fragmentum_duration_seconds =
            settings.max_fragmentum_duration_seconds;
        self.config.default.fractor.pause_threshold_in_chunks = settings.pause_threshold_in_chunks;

        // Update STT model in config if changed
        if stt_model_changed && let Some(key) = settings.selected_stt_model_key() {
            self.config.default.stt.model = AvailableSTTModel::from_key(key);
        }

        // Update VAD model in config if changed
        if vad_model_changed && let Some(key) = settings.selected_vad_model_key() {
            self.config.default.vad.model = AvailableVADModel::from_key(key);
        }

        // Write to file if requested
        if write_to_file {
            let config_dir = dirs::config_dir()
                .ok_or_else(|| color_eyre::eyre::eyre!("Could not find config directory"))?
                .join("scriptor");
            let config_path = config_dir.join("scriptor.toml");

            self.config
                .write(&config_path)
                .map_err(|e| color_eyre::eyre::eyre!("Failed to save config: {}", e))?;
        }

        // If any model changed, check for missing files and download
        if (stt_model_changed || vad_model_changed)
            && let Some(missing_files) = self.config.check_missing(&self.available_models)
        {
            let mut spinner = Spinner::new_with_stream(
                spinners::Dots,
                "Downloading models...",
                Color::Blue,
                Streams::Stderr,
            );
            download_missing_files(&missing_files).await;
            spinner.success("Models downloaded!");
        }

        // Reload STT model if changed
        if stt_model_changed {
            let mut spinner = Spinner::new_with_stream(
                spinners::Dots,
                "Loading STT model...",
                Color::Blue,
                Streams::Stderr,
            );
            let stt_model = STTModel::new(
                &self.config.default.stt,
                self.config.default.inference.clone(),
            )
            .map_err(|e| color_eyre::eyre::eyre!("Failed to load STT model: {}", e))?;
            self.stt_tools.stt_model = Some(stt_model);
            spinner.success("STT model loaded!");
        }

        // Rebuild Fractor if VAD model or threshold changed (VAD is inside Fractor)
        if vad_model_changed || vad_threshold_changed {
            let mut spinner = Spinner::new_with_stream(
                spinners::Dots,
                "Loading VAD model...",
                Color::Blue,
                Streams::Stderr,
            );

            // Create new VAD model with updated config
            let vad_model = VADModel::new(
                &self.config.default.vad,
                self.config.default.inference.clone(),
            )
            .map_err(|e| color_eyre::eyre::eyre!("Failed to load VAD model: {}", e))?;

            // Create new recorder config
            let recorder_config = RecorderConfig::new(
                self.config.default.fractor.max_fragmentum_duration_seconds,
                self.config.default.input_device.as_deref(),
            )
            .map_err(|e| color_eyre::eyre::eyre!("Failed to create recorder config: {}", e))?;

            // Rebuild Fractor with new VAD model
            let fractor = Fractor::new(recorder_config, vad_model);
            self.stt_tools.fractor = Some(fractor);

            spinner.success("VAD model loaded!");
        }

        self.settings_state = None;
        self.current_screen = CurrentScreen::Main;

        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = &self.config.default.theme;

        // Destructure 13-area book-style layout
        let (
            outer_area_0,
            outer_area_1,
            outer_area_2,
            outer_area_3,
            inner_area,
            page_l,
            light_shadow_l,
            medium_shadow_l,
            medium_shadow_r,
            light_shadow_r,
            right_page_area,
            bookmark_area,
            _empty_area,
        ) = AppLayout::calculate_main_layout(area);

        // Render layered book backgrounds (outermost → innermost)
        AppLayout::render_table(theme.very_dark_shadow, outer_area_0, buf);
        AppLayout::render_spine(theme.dark_shadow, theme.very_dark_shadow, outer_area_1, buf);
        AppLayout::render_spine(
            theme.medium_shadow,
            theme.very_dark_shadow,
            outer_area_2,
            buf,
        );
        AppLayout::render_spine(
            theme.light_shadow,
            theme.very_dark_shadow,
            outer_area_3,
            buf,
        );
        AppLayout::render_inner_page(inner_area, buf, theme);

        // Render spine separators
        AppLayout::render_light_shadow(theme.light_shadow, light_shadow_l, buf);
        AppLayout::render_medium_shadow(
            theme.medium_shadow,
            theme.dark_shadow,
            medium_shadow_l,
            buf,
        );
        AppLayout::render_medium_shadow(
            theme.medium_shadow,
            theme.dark_shadow,
            medium_shadow_r,
            buf,
        );
        AppLayout::render_light_shadow(theme.light_shadow, light_shadow_r, buf);

        // Render bookmark tab with archivum name written vertically
        AppLayout::render_bookmark(
            bookmark_area,
            buf,
            &format!("  {}", &self.config.default.db.name),
            theme,
        );

        // Split left/right pages into header (10%) + content + 2-row bottom margin
        let page_split = Layout::vertical([
            Constraint::Percentage(10),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [codices_header_area, codices_area, _] = page_split.areas(page_l);
        let [fragmenta_header_area, fragmenta_area, _] = page_split.areas(right_page_area);

        // Render column headers
        AppLayout::render_header(codices_header_area, buf, "C O D I C E S", theme);
        AppLayout::render_header(fragmenta_header_area, buf, "F R A G M E N T A", theme);

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
            self.show_timestamp,
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
                fragmenta_area,
                buf,
                theme,
            ),
            CurrentScreen::AddArchivum => {
                // Render ChangeArchivum as background (archivum list), then AddArchivum popup on top
                ChangeArchivumPopUp::render(
                    &self.config,
                    self.selected_archivum_index,
                    fragmenta_area,
                    buf,
                    theme,
                );
                AddArchivumPopUp::render(&self.input_state, fragmenta_area, buf, theme)
            }
            CurrentScreen::ModifyArchivum => {
                // Render ChangeArchivum as background (archivum list), then ModifyArchivum popup on top
                ChangeArchivumPopUp::render(
                    &self.config,
                    self.selected_archivum_index,
                    fragmenta_area,
                    buf,
                    theme,
                );
                ModifyArchivumPopUp::render(&self.input_state, fragmenta_area, buf, theme)
            }
            CurrentScreen::DeleteArchivum => {
                // Render ChangeArchivum as background (archivum list), then DeleteArchivum popup on top
                if let Some(archivum) = self.config.dbs.get(self.selected_archivum_index) {
                    ChangeArchivumPopUp::render(
                        &self.config,
                        self.selected_archivum_index,
                        fragmenta_area,
                        buf,
                        theme,
                    );
                    DeleteArchivumPopUp::render(
                        &self.input_state,
                        &archivum.name,
                        fragmenta_area,
                        buf,
                        theme,
                    );
                }
            }
            CurrentScreen::DeleteCodex => {
                if let (Some(codex), None) = self.codices_component.get_selected_codex_and_folio() {
                    DeleteCodexPopUp::render(
                        &self.input_state,
                        &codex.codex.name,
                        codices_area,
                        buf,
                        theme,
                    );
                }
            }
            CurrentScreen::DeleteFolio => {
                if let (Some(_codex), Some(folio)) =
                    self.codices_component.get_selected_codex_and_folio()
                {
                    DeleteFolioPopUp::render(
                        &self.input_state,
                        &folio.folio.name,
                        codices_area,
                        buf,
                        theme,
                    );
                }
            }
            CurrentScreen::RecordFolio => {
                let mut selected_folio =
                    if let Some(codex) = self.codices_component.get_selected_codex_mut() {
                        if let Some(folio_idx) = codex.folio_state.selected() {
                            codex.folia.get_mut(folio_idx)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                // Autoscroll: ensure last fragmentum is selected for background
                if let Some(ref mut folio) = selected_folio {
                    if !folio.fragmenta.is_empty() {
                        folio
                            .fragmentum_state
                            .select(Some(folio.fragmenta.len() - 1));
                    }
                }
                // Animated dots: . .. ... cycling every ~400ms
                let dots = self
                    .recording_screen_start
                    .map(|start| {
                        let elapsed = start.elapsed().as_millis();
                        let frame = (elapsed / 400) % 3;
                        match frame {
                            0 => ".",
                            1 => "..",
                            _ => "...",
                        }
                    })
                    .unwrap_or(".");
                RecordingScreen::render(
                    self.is_paused,
                    selected_folio.as_deref(),
                    dots,
                    &mut self.recording_list_state,
                    area,
                    buf,
                    theme,
                );
            }
            CurrentScreen::Settings => {
                if let Some(settings_state) = &self.settings_state {
                    SettingsScreen::render(settings_state, area, buf, theme);
                }
            }
            _ => {}
        }
    }
}
