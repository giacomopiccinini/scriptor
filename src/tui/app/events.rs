use crate::tui::app::state::{App, CurrentRegion, CurrentScreen};
use crate::tui::ui::components::{CodicesComponent, FoliaComponent};
use crate::tui::ui::cursor::CursorState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub struct EventHandler;

impl EventHandler {
    /// Handle key press from user in main screen
    pub async fn handle_main_screen_key(app: &mut App, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Quit application
            (KeyCode::Char('q'), KeyModifiers::NONE) => app.exit = true,

            // Navigate up in codices, folia or fragmenta
            (KeyCode::Up, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::Codex => app.codices_component.select_previous(),
                CurrentRegion::Folio => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut() {
                        FoliaComponent::select_previous_folio(selected_codex);
                    }
                }
                CurrentRegion::Fragmentum => todo!(),
            },

            // Navigate down in codices, folia or fragmenta
            (KeyCode::Down, KeyModifiers::NONE) => match app.current_region {
                CurrentRegion::Codex => app.codices_component.select_next(),
                CurrentRegion::Folio => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut() {
                        FoliaComponent::select_next_folio(selected_codex);
                    }
                }
                CurrentRegion::Fragmentum => todo!(),
            },

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
                CurrentRegion::Codex => {
                    if let Err(e) = CodicesComponent::move_selected_codex_down(
                        &mut app.codices_component,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to move codex down: {}", e);
                    }
                }
                CurrentRegion::Folio => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        && let Err(e) =
                            FoliaComponent::move_selected_folio_down(selected_codex, &app.pool)
                                .await
                    {
                        eprintln!("Failed to move folio up: {}", e);
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            // Move codex/folio up, reordering the list
            (KeyCode::Up, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::Codex => {
                    if let Err(e) = CodicesComponent::move_selected_codex_up(
                        &mut app.codices_component,
                        &app.pool,
                    )
                    .await
                    {
                        eprintln!("Failed to move codex up: {}", e);
                    }
                }
                CurrentRegion::Folio => {
                    if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        && let Err(e) =
                            FoliaComponent::move_selected_folio_up(selected_codex, &app.pool).await
                    {
                        eprintln!("Failed to move folio up: {}", e);
                    }
                }
                CurrentRegion::Fragmentum => {}
            },

            (KeyCode::Left, KeyModifiers::NONE) => {
                match app.current_region {
                    CurrentRegion::Codex => {} // Nothing here
                    CurrentRegion::Folio => {
                        if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                        {
                            FoliaComponent::remove_folio_selection(selected_codex);
                        }
                        app.current_region = CurrentRegion::Codex;
                    }
                    CurrentRegion::Fragmentum => todo!(),
                }
            }

            (KeyCode::Right, KeyModifiers::NONE) => {
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut() {
                    FoliaComponent::select_first_item(selected_codex);
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

    /// Handle key press from user in add item screen
    pub async fn handle_add_or_modify_folio_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_add_folio_without_saving(),
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

    /// Handle change of db
    pub async fn handle_change_archivum_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_change_archivum_without_saving(),
            KeyCode::Up => app.select_previous_db(),
            KeyCode::Down => app.select_next_db(),
            KeyCode::Enter => {
                if let Err(e) = app.switch_to_selected_db().await {
                    eprintln!("Failed to switch database: {}", e);
                }
            }
            KeyCode::Char('A') => app.enter_add_archivum_screen(),
            KeyCode::Char('S') => {
                // Set selected database as default
                if let Err(e) = app.set_selected_archivum_as_default().await {
                    eprintln!("Failed to set database as default: {}", e);
                }
            }
            _ => {}
        }
    }

    /// Handle key press from user in add database screen
    pub async fn handle_add_archivum_screen_key(app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => app.exit_add_archivum_without_saving(),
            KeyCode::Backspace => app.input_state.remove_char_before_cursor(),
            KeyCode::Delete => app.input_state.delete_char_after_cursor(),
            KeyCode::Char(value) => app.input_state.add_char(value),
            KeyCode::Left => app.input_state.move_cursor_left(),
            KeyCode::Right => app.input_state.move_cursor_right(),
            KeyCode::Enter => {
                let db_name = app.input_state.get_text().to_string();
                if !db_name.trim().is_empty() {
                    if let Err(e) = app.create_new_database(db_name, false).await {
                        eprintln!("Failed to create database: {}", e);
                    } else {
                        app.current_screen = CurrentScreen::ChangeDB;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
    }
}
