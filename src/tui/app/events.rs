use crate::stt::queue::{create_fragmentum_channel, transcriber_to_db_worker};
use crate::tui::app::state::{App, CurrentRegion, CurrentScreen};
use crate::tui::db::models::{Folio, NewFolio};
use crate::tui::ui::components::{CodicesComponent, FoliaComponent, FragmentaComponent};
use crate::tui::ui::cursor::CursorState;
use arboard::{Clipboard, SetExtLinux};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct EventHandler;

impl EventHandler {
    /// Handle key press from user in main screen
    pub async fn handle_main_screen_key(app: &mut App, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Quit application
            (KeyCode::Char('q'), KeyModifiers::NONE) => app.exit = true,

            // Navigate up in codices tree or fragmenta
            (KeyCode::Up, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::CodexAndFolio => app.codices_component.select_previous(),
                CurrentRegion::Fragmentum => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                        && let Some(selected_folio) =
                            selected_codex.folia.get_mut(selected_folio_idx)
                    {
                        FragmentaComponent::select_previous_fragmentum(selected_folio);
                    }
                }
            },

            // Navigate down in codices tree or fragmenta
            (KeyCode::Down, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    // Navigate to next
                    app.codices_component.select_next();
                }
                CurrentRegion::Fragmentum => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                        && let Some(selected_folio) =
                            selected_codex.folia.get_mut(selected_folio_idx)
                    {
                        FragmentaComponent::select_next_fragmentum(selected_folio);
                    }
                }
            },

            // Expand/collapse codex with Enter key
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if app.current_region == CurrentRegion::CodexAndFolio {
                    app.codices_component.toggle_selected_codex_expansion();
                }
            }

            // Add new codex
            (KeyCode::Char('n'), KeyModifiers::NONE) => app.enter_add_codex_screen(),

            // Add new item
            (KeyCode::Char('a'), KeyModifiers::NONE) => app.enter_add_folio_screen(),

            // Change archivum
            (KeyCode::Tab, KeyModifiers::NONE) => app.enter_change_archivum_screen(),

            // Modify existing item
            (KeyCode::Char('m'), KeyModifiers::NONE) => {
                match app.codices_component.get_selected_codex_and_folio() {
                    // If a folio is selected, modify the folio
                    (Some(_codex), Some(folio)) => {
                        app.enter_modify_folio_screen(folio.folio.name.clone());
                    }
                    // If only a codex is selected (no folio), modify the codex
                    (Some(codex), None) => {
                        app.enter_modify_codex_screen(codex.codex.name.clone());
                    }

                    // Cannot happen, but needed for consistency
                    (None, Some(_folio)) => {}

                    // Nothing selected, do nothing
                    (None, None) => {}
                }
            }

            // Delete folio or codex, depending on which one is selected
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                if let Err(e) = app.codices_component.delete_selected(&app.pool).await {
                    eprintln!("Failed to delete: {}", e);
                }
            }

            // Move codex/folio down, reordering the list
            (KeyCode::Down, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    match app.codices_component.get_selected_codex_and_folio() {
                        // Folio selected - move folio down
                        (Some(_), Some(_)) => {
                            if let Err(e) = CodicesComponent::move_selected_folio_down(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            {
                                eprintln!("Failed to move folio down: {}", e);
                            }
                        }
                        // Only codex selected - move codex down
                        (Some(_), None) => {
                            if let Err(e) = CodicesComponent::move_selected_codex_down(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            {
                                eprintln!("Failed to move codex down: {}", e);
                            }
                        }
                        // Nothing selected
                        _ => {}
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            // Move codex/folio up, reordering the list
            (KeyCode::Up, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    match app.codices_component.get_selected_codex_and_folio() {
                        // Folio selected - move folio up
                        (Some(_), Some(_)) => {
                            if let Err(e) = CodicesComponent::move_selected_folio_up(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            {
                                eprintln!("Failed to move folio up: {}", e);
                            }
                        }
                        // Only codex selected - move codex up
                        (Some(_), None) => {
                            if let Err(e) = CodicesComponent::move_selected_codex_up(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            {
                                eprintln!("Failed to move codex up: {}", e);
                            }
                        }
                        // Nothing selected
                        _ => {}
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            // Navigate left between regions (Codex <- Folio <- Fragmentum)
            (KeyCode::Left, KeyModifiers::NONE) => {
                match app.current_region {
                    CurrentRegion::CodexAndFolio => {} // Already at leftmost
                    CurrentRegion::Fragmentum => {
                        // Deselect fragmentum when leaving the region
                        if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                            && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                            && let Some(selected_folio) =
                                selected_codex.folia.get_mut(selected_folio_idx)
                        {
                            FragmentaComponent::remove_fragmentum_selection(selected_folio);
                        }
                        app.current_region = CurrentRegion::CodexAndFolio;
                    }
                }
            }

            // Navigate right between regions (Codex -> Fragmentum)
            (KeyCode::Right, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    // Auto-select first fragmentum when entering the region
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                        && let Some(selected_folio) =
                            selected_codex.folia.get_mut(selected_folio_idx)
                    {
                        FragmentaComponent::select_first_fragmentum(selected_folio);
                    }
                    app.current_region = CurrentRegion::Fragmentum;
                }
                CurrentRegion::Fragmentum => {}
            },

            // Start recording
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                if let (true, false) = app.codices_component.check_codex_folio_selection() {
                    let codex = app
                        .codices_component
                        .get_selected_codex_mut()
                        .expect("Codex should exist but can't be reached");
                    // Create new folio with datetime name. This is the default in the TUI.
                    let folio_name = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

                    // Create ibject to be added to db
                    let new_folio = NewFolio {
                        codex_id: codex.codex.id,
                        name: folio_name,
                    };

                    // Create new folio in db
                    let folio = Folio::create(&app.pool, new_folio)
                        .await
                        .expect("Unable to add folio to archivum");
                    let folio_id = folio.id;

                    // Take ownership of fractor and stt_model for threads
                    if let (Some(fractor), Some(stt_model)) =
                        (app.stt_tools.fractor.take(), app.stt_tools.stt_model.take())
                    {
                        // Create path to output directory for audio files
                        let output_dir = dirs::data_dir()
                            .expect("Unable to get data directory")
                            .join("scriptor")
                            .join("audios")
                            .join(format!("{:04}", codex.codex.id))
                            .join(format!("{:04}", folio_id));

                        fs::create_dir_all(&output_dir).expect("Unable to create audio directory");

                        // Create stop and pause signals
                        let stop_signal = Arc::new(AtomicBool::new(false));
                        let pause_signal = Arc::new(AtomicBool::new(false));

                        // Store signals in app state
                        app.recording_stop_signal = Some(Arc::clone(&stop_signal));
                        app.recording_pause_signal = Some(Arc::clone(&pause_signal));
                        app.recording_folio_id = Some(folio_id);

                        // Create channel for fragmenta
                        let (tx, rx) = create_fragmentum_channel(app.stt_tools.max_queue_elements);

                        // Clone pool for transcriber thread
                        let pool_clone = app.pool.clone();

                        // Capture runtime handle BEFORE spawning std::thread
                        let runtime_handle = tokio::runtime::Handle::current();

                        // Spawn fractor thread
                        let fractor_handle = std::thread::spawn(move || {
                            fractor.run(Some(output_dir), stop_signal, pause_signal, tx)
                        });

                        // Spawn transcriber thread
                        let transcriber_handle = std::thread::spawn(move || {
                            transcriber_to_db_worker(
                                stt_model,
                                folio_id,
                                pool_clone,
                                rx,
                                runtime_handle,
                            )
                        });

                        // Update thread state
                        app.fractor_handle = Some(fractor_handle);
                        app.transcriber_handle = Some(transcriber_handle);

                        // Update UI state
                        app.is_recording = true;
                        app.is_paused = false;

                        // Refresh folia to include the new one
                        codex
                            .update_folia(&app.pool)
                            .await
                            .expect("Unable to update folia");

                        // Select the newly created folio
                        let folio_count = codex.folia.len();

                        // Open up codex
                        codex.expand();
                        if folio_count > 0 {
                            codex.folio_state.select(Some(folio_count - 1));
                            // FIX
                            app.codices_component
                                .list_state
                                .scroll_down_by(folio_count as u16);
                        }

                        // Switch to recording screen
                        app.current_screen = CurrentScreen::RecordFolio;
                    }
                }
            }

            // Copy single fragmentum to clipboard
            (KeyCode::Char('c'), KeyModifiers::NONE) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                    && let Some(selected_folio) = selected_codex.folia.get(selected_folio_idx)
                    && let Some(fragmentum_idx) = selected_folio.fragmentum_state.selected()
                    && let Some(ui_fragmentum) = selected_folio.fragmenta.get(fragmentum_idx)
                {
                    // Clone content for the thread
                    let content = ui_fragmentum.fragmentum.content.clone();
                    // Spawn thread to keep clipboard alive until content is read
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            // Use Linux extension to wait until clipboard is read
                            let _ = clipboard.set().wait().text(content);
                        }
                    });
                }
            }

            // Copy all fragmenta from selected folio to clipboard
            (KeyCode::Char('C'), KeyModifiers::SHIFT) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                    && let Some(selected_folio) = selected_codex.folia.get(selected_folio_idx)
                {
                    // Concatenate all fragmenta content with newlines
                    let all_content: String = selected_folio
                        .fragmenta
                        .iter()
                        .map(|f| f.fragmentum.content.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    // Spawn thread to keep clipboard alive until content is read
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            // Use Linux extension to wait until clipboard is read
                            let _ = clipboard.set().wait().text(all_content);
                        }
                    });
                }
            }
            _ => {}
        }
    }

    /// Handle key press from user in add codex screen
    pub async fn handle_add_or_modify_codex_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_add_or_modify_codex_without_saving(),
            KeyCode::Backspace => app.input_state.remove_char_before_cursor(),
            KeyCode::Delete => app.input_state.delete_char_after_cursor(),
            KeyCode::Char(value) => app.input_state.add_char(value),
            KeyCode::Left => app.input_state.move_cursor_left(),
            KeyCode::Right => app.input_state.move_cursor_right(),
            KeyCode::Enter => {
                let codex_name = app.input_state.get_text().to_string();
                // Only do something if the codex has a name
                if !codex_name.trim().is_empty() {
                    if app.input_state.is_modifying {
                        if let Err(e) = CodicesComponent::update_codex(
                            &mut app.codices_component,
                            codex_name,
                            &app.pool,
                        )
                        .await
                        {
                            eprintln!("Failed to update codex: {}", e);
                        } else {
                            app.current_screen = CurrentScreen::Main;
                            app.input_state.clear();
                        }
                    } else if let Err(e) = CodicesComponent::create_codex(
                        &mut app.codices_component,
                        codex_name,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to create codex: {}", e);
                    } else {
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle key press from user in add folio screen
    pub async fn handle_add_or_modify_folio_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_add_or_modify_folio_without_saving(),
            KeyCode::Backspace => app.input_state.remove_char_before_cursor(),
            KeyCode::Delete => app.input_state.delete_char_after_cursor(),
            KeyCode::Left => app.input_state.move_cursor_left(),
            KeyCode::Right => app.input_state.move_cursor_right(),
            KeyCode::Char(value) => app.input_state.add_char(value),
            KeyCode::Enter => {
                let folio_name = app.input_state.get_text().to_string();
                if !folio_name.trim().is_empty()
                    && let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                {
                    if app.input_state.is_modifying {
                        if let Err(e) =
                            FoliaComponent::update_item(selected_codex, folio_name, &app.pool).await
                        {
                            eprintln!("Failed to update item: {}", e);
                        } else {
                            app.current_screen = CurrentScreen::Main;
                            app.input_state.clear();
                        }
                    } else if let Err(e) = FoliaComponent::create_item(
                        selected_codex,
                        folio_name,
                        &mut app.stt_tools,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to create item: {}", e);
                    } else {
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle change of archivum
    pub async fn handle_change_archivum_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_change_archivum_without_saving(),
            KeyCode::Up => app.select_previous_archivum(),
            KeyCode::Down => app.select_next_archivum(),
            KeyCode::Enter => {
                if let Err(e) = app.switch_to_selected_archivum().await {
                    eprintln!("Failed to switch archivum: {}", e);
                }
            }
            KeyCode::Char('A') => app.enter_add_archivum_screen(),
            KeyCode::Char('S') => {
                // Set selected archivum as default
                if let Err(e) = app.set_selected_archivum_as_default().await {
                    eprintln!("Failed to set archivum as default: {}", e);
                }
            }
            _ => {}
        }
    }

    /// Handle key press from user in add archivum screen
    pub async fn handle_add_archivum_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_add_archivum_without_saving(),
            KeyCode::Backspace => app.input_state.remove_char_before_cursor(),
            KeyCode::Delete => app.input_state.delete_char_after_cursor(),
            KeyCode::Char(value) => app.input_state.add_char(value),
            KeyCode::Left => app.input_state.move_cursor_left(),
            KeyCode::Right => app.input_state.move_cursor_right(),
            KeyCode::Enter => {
                let archivum_name = app.input_state.get_text().to_string();
                if !archivum_name.trim().is_empty() {
                    if let Err(e) = app.create_new_archivum(archivum_name, false).await {
                        eprintln!("Failed to create archivum: {}", e);
                    } else {
                        app.current_screen = CurrentScreen::ChangeArchivum;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle key press from user in recording screen
    pub async fn handle_record_folio_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            // Toggle pause/resume
            // TODO: Fix this as currently it is not working properly
            KeyCode::Char(' ') => {
                if let Some(pause_signal) = &app.recording_pause_signal {
                    let currently_paused = pause_signal.load(Ordering::SeqCst);
                    pause_signal.store(!currently_paused, Ordering::SeqCst);
                    app.is_paused = !currently_paused;
                }
            }

            // Stop recording and return to main screen
            KeyCode::Esc => {
                // Signal fractor to stop
                if let Some(stop_signal) = &app.recording_stop_signal {
                    stop_signal.store(true, Ordering::SeqCst);
                }

                // Wait for threads to finish processing
                if let Some(fractor_handle) = app.fractor_handle.take() {
                    fractor_handle
                        .join()
                        .expect("Recording thread panicked")
                        .expect("Recording failed");
                };
                if let Some(transcriber_handle) = app.transcriber_handle.take() {
                    transcriber_handle
                        .join()
                        .expect("Transcribing thread panicked")
                        .expect("Transcribing failed");
                };

                // Refresh the folio's fragmenta from DB
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Some(folio_idx) = selected_codex.folio_state.selected()
                    && let Some(selected_folio) = selected_codex.folia.get_mut(folio_idx)
                    && let Err(e) = selected_folio.update_fragmenta(&app.pool).await
                {
                    eprintln!("Failed to refresh fragmenta: {}", e);
                }

                // Reinitialize STT tools for next recording
                if let Err(e) = app.stt_tools.reinitialize(&app.config) {
                    eprintln!("Failed to reinitialize STT tools: {}", e);
                }

                // Clear recording state
                app.recording_stop_signal = None;
                app.recording_pause_signal = None;
                app.recording_folio_id = None;
                app.is_recording = false;
                app.is_paused = false;
                app.fractor_handle = None;
                app.transcriber_handle = None;

                // Return to main screen
                app.current_screen = CurrentScreen::Main;
            }

            _ => {}
        }
    }
}
