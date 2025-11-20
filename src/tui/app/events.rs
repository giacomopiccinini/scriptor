use crate::tui::app::state::{App, CurrentRegion, CurrentScreen};
use crate::tui::ui::components::{CodicesComponent, FoliaComponent, FragmentaComponent};
use crate::tui::ui::cursor::CursorState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
            (KeyCode::Char('A'), KeyModifiers::SHIFT) => app.enter_add_codex_screen(),

            // Add new item
            (KeyCode::Char('a'), KeyModifiers::NONE) => app.enter_add_folio_screen(),

            // Change archivum
            (KeyCode::Tab, KeyModifiers::NONE) => app.enter_change_archivum_screen(),

            // Modify existing codex
            (KeyCode::Char('M'), KeyModifiers::SHIFT) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex() {
                    app.enter_modify_codex_screen(&selected_codex.codex.clone())
                }
            }

            // Modify existing item
            (KeyCode::Char('m'), KeyModifiers::NONE) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex() {
                    app.enter_modify_folio_screen(&selected_codex.clone())
                }
            }

            // Delete codex
            (KeyCode::Char('D'), KeyModifiers::SHIFT) => {
                if let Err(e) = CodicesComponent::delete_selected_codex_static(
                    &mut app.codices_component,
                    &app.pool,
                )
                .await
                {
                    // Log error but don't crash the application
                    eprintln!("Failed to delete codex: {}", e);
                }
            }

            // Delete folio
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Err(e) =
                        FoliaComponent::delete_selected_folio(selected_codex, &app.pool).await
                {
                    eprintln!("Failed to delete folio: {}", e);
                }
            }

            // Move codex/folio down, reordering the list
            (KeyCode::Down, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    if let Err(e) = CodicesComponent::move_selected_codex_down(
                        &mut app.codices_component,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to move codex down: {}", e);
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            // Move codex/folio up, reordering the list
            (KeyCode::Up, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    if let Err(e) = CodicesComponent::move_selected_codex_up(
                        &mut app.codices_component,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to move codex up: {}", e);
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            // Navigate left between regions (Codex <- Folio <- Fragmentum)
            (KeyCode::Left, KeyModifiers::NONE) => {
                match app.current_region {
                    CurrentRegion::CodexAndFolio => {} // Already at leftmost
                    CurrentRegion::Fragmentum => {
                        app.current_region = CurrentRegion::CodexAndFolio;
                    }
                }
            }

            // Navigate right between regions (Codex -> Fragmentum)
            (KeyCode::Right, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    app.current_region = CurrentRegion::Fragmentum;
                }
                CurrentRegion::Fragmentum => {}
            },

            // Start recording (placeholder)
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                // TODO: Implement recording functionality
                eprintln!("Recording not yet implemented");
            }

            // Copy single fragmentum (placeholder)
            (KeyCode::Char('c'), KeyModifiers::NONE) => {
                // TODO: Implement copy fragmentum to clipboard
                eprintln!("Copy fragmentum not yet implemented");
            }

            // Copy all fragmenta from selected folio (placeholder)
            (KeyCode::Char('C'), KeyModifiers::SHIFT) => {
                // TODO: Implement copy all fragmenta to clipboard
                eprintln!("Copy all fragmenta not yet implemented");
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
            KeyCode::Esc => app.exit_add_item_without_saving(),
            KeyCode::Backspace => app.input_state.remove_char_before_cursor(),
            KeyCode::Delete => app.input_state.delete_char_after_cursor(),
            KeyCode::Left => app.input_state.move_cursor_left(),
            KeyCode::Right => app.input_state.move_cursor_right(),
            KeyCode::Char(value) => app.input_state.add_char(value),
            KeyCode::Enter => {
                let item_name = app.input_state.get_text().to_string();
                if !item_name.trim().is_empty()
                    && let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                {
                    if app.input_state.is_modifying {
                        if let Err(e) =
                            FoliaComponent::update_item(selected_codex, item_name, &app.pool).await
                        {
                            eprintln!("Failed to update item: {}", e);
                        } else {
                            app.current_screen = CurrentScreen::Main;
                            app.input_state.clear();
                        }
                    } else if let Err(e) =
                        FoliaComponent::create_item(selected_codex, item_name, &app.pool).await
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
}
