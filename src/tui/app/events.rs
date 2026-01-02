use crate::tui::app::state::{App, CurrentRegion, CurrentScreen};
use crate::tui::ui::components::{CodicesComponent, FoliaComponent, FragmentaComponent};
use crate::tui::ui::cursor::CursorState;
use arboard::{Clipboard, SetExtLinux};
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
            (KeyCode::Char('n'), KeyModifiers::NONE) => app.enter_add_codex_screen(),

            // Add new item
            // TODO
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

            // Start recording (placeholder)
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                // TODO: Implement recording functionality
                eprintln!("Recording not yet implemented");
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
}
