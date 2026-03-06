use crate::configs::settings::SettingsField;
use crate::stt::fractor::Fractor;
use crate::stt::model::STTModel;
use crate::stt::queue::{create_fragmentum_channel, transcriber_to_db_worker};
use crate::tui::app::state::{App, CurrentRegion, CurrentScreen};
use crate::tui::db::models::{Folio, Fragmentum, NewFolio};
use crate::tui::ui::components::{
    CodicesComponent, FoliaComponent, FragmentaComponent, format_timestamp,
};
use crate::tui::ui::cursor::CursorState;
use anyhow::{Context, Result};
use arboard::Clipboard;
#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct EventHandler;

impl EventHandler {
    /// Handle key press from user in main screen
    pub async fn handle_main_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
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

            // Add new item (import folio)
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                if let (true, false) | (true, true) =
                    app.codices_component.check_codex_folio_selection()
                {
                    app.enter_add_folio_screen();
                }
            }

            // Change archivum
            (KeyCode::Char('a'), KeyModifiers::NONE) => app.enter_change_archivum_screen(),

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

            // Delete folio or codex - open confirmation popup
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                match app.codices_component.get_selected_codex_and_folio() {
                    (Some(_), Some(_)) => app.enter_delete_folio_screen(),
                    (Some(_), None) => app.enter_delete_codex_screen(),
                    (None, _) => {}
                }
            }

            // Move codex/folio down, reordering the list
            (KeyCode::Down, KeyModifiers::CONTROL) => match app.current_region {
                CurrentRegion::CodexAndFolio => {
                    match app.codices_component.get_selected_codex_and_folio() {
                        // Folio selected - move folio down
                        (Some(_), Some(_)) => {
                            CodicesComponent::move_selected_folio_down(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            .with_context(|| "Failed to move folio down")?;
                        }
                        // Only codex selected - move codex down
                        (Some(_), None) => {
                            CodicesComponent::move_selected_codex_down(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            .with_context(|| "Failed to move codex down")?;
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
                            CodicesComponent::move_selected_folio_up(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            .with_context(|| "Failed to move folio up")?;
                        }
                        // Only codex selected - move codex up
                        (Some(_), None) => {
                            CodicesComponent::move_selected_codex_up(
                                &mut app.codices_component,
                                &app.pool,
                            )
                            .await
                            .with_context(|| "Failed to move codex up")?;
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
                        // Block navigation during playback to prevent folio change (would desync playback)
                        if !app.stt_tools.player.is_playing {
                            // Deselect fragmentum when leaving the region
                            if let Some(selected_codex) =
                                app.codices_component.get_selected_codex_mut()
                                && let Some(selected_folio_idx) =
                                    selected_codex.folio_state.selected()
                                && let Some(selected_folio) =
                                    selected_codex.folia.get_mut(selected_folio_idx)
                            {
                                FragmentaComponent::remove_fragmentum_selection(selected_folio);
                            }
                            app.current_region = CurrentRegion::CodexAndFolio;
                        }
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

            // Start recording (creates new folio)
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                // Ideally, you should only record from a codex, not a folio.
                // Here we are improving the UX by letting the user launch a recording even from a folio
                // The caveat is that we won't show the corresponding command hint but still allow the
                // recording from the "parent" codex
                if let (true, false) | (true, true) =
                    app.codices_component.check_codex_folio_selection()
                {
                    let codex =
                        app.codices_component
                            .get_selected_codex_mut()
                            .ok_or_else(|| {
                                anyhow::anyhow!("Codex should exist but can't be reached")
                            })?;
                    let codex_id = codex.codex.id;

                    // Create new folio with datetime name
                    let folio_name = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                    let new_folio = NewFolio {
                        codex_id,
                        name: folio_name,
                    };
                    let folio = Folio::create(&app.pool, new_folio)
                        .await
                        .with_context(|| "Unable to import folio to archivum")?;
                    let folio_id = folio.id;

                    if let (Some(fractor), Some(stt_model)) =
                        (app.stt_tools.fractor.take(), app.stt_tools.stt_model.take())
                    {
                        EventHandler::start_recording_for_folio(
                            app, fractor, stt_model, codex_id, folio_id, false,
                        )
                        .await
                        .with_context(|| "Unable to start recording")?;

                        {
                            // Post-recording UI updates for new folio: refresh, select, expand, scroll
                            let codex = app
                                .codices_component
                                .get_selected_codex_mut()
                                .ok_or_else(|| anyhow::anyhow!("Codex should exist"))?;
                            codex
                                .update_folia(&app.pool)
                                .await
                                .with_context(|| "Unable to update folia")?;

                            let folio_count = codex.folia.len();
                            let previous_folio_idx = codex.folio_state.selected();
                            codex.expand();
                            let scroll_amount = if folio_count > 0 {
                                codex.folio_state.select(Some(folio_count - 1));
                                if let Some(selected_folio) = codex.folia.get_mut(folio_count - 1) {
                                    if !selected_folio.fragmenta.is_empty() {
                                        selected_folio
                                            .fragmentum_state
                                            .select(Some(selected_folio.fragmenta.len() - 1));
                                    }
                                }
                                match previous_folio_idx {
                                    None => folio_count as u16,
                                    Some(k) => (folio_count - 1 - k) as u16,
                                }
                            } else {
                                0
                            };

                            if scroll_amount > 0 {
                                app.codices_component
                                    .list_state
                                    .scroll_down_by(scroll_amount);
                            }
                        }
                    }
                }
            }

            // Extend recording (append to existing folio when a folio is selected)
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                if app.codices_component.check_codex_folio_selection() == (true, true) {
                    let codex_folio_ids = match app.codices_component.get_selected_codex_and_folio()
                    {
                        (Some(codex), Some(folio)) => Some((codex.codex.id, folio.folio.id)),
                        _ => None,
                    };
                    if let Some((codex_id, folio_id)) = codex_folio_ids
                        && let (Some(fractor), Some(stt_model)) =
                            (app.stt_tools.fractor.take(), app.stt_tools.stt_model.take())
                    {
                        EventHandler::start_recording_for_folio(
                            app, fractor, stt_model, codex_id, folio_id, true,
                        )
                        .await
                        .with_context(|| "Unable to extend recording")?;
                    }
                }
            }

            // Play fragmenta
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                if app.stt_tools.player.is_playing {
                    app.stt_tools
                        .player
                        .pause()
                        .with_context(|| "Unable to pause player")?;
                } else if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Some(selected_folio_idx) = selected_codex.folio_state.selected()
                    && let Some(selected_folio) = selected_codex.folia.get_mut(selected_folio_idx)
                    && let Some(fragmentum_idx) = selected_folio.fragmentum_state.selected()
                    && let Some(ui_fragmentum) = selected_folio.fragmenta.get(fragmentum_idx)
                {
                    // Find all audio files that come after the one corresponding to the selected fragmentum
                    let audio_paths = Fragmentum::get_subsequent_fragmenta_audio_paths(
                        &app.pool,
                        selected_folio.folio.id,
                        ui_fragmentum.fragmentum.id,
                    )
                    .await
                    .with_context(|| "Unable to fetch audio paths")?;

                    // Add files to player queue
                    app.stt_tools
                        .player
                        .load_files(audio_paths)
                        .with_context(|| "Unable to enqueue fragmenta to player queue")?;

                    // Reset playback file index tracker
                    app.last_playback_file_index = 0;
                    app.playback_start_fragmentum_idx = Some(fragmentum_idx);

                    // Play audio
                    app.stt_tools
                        .player
                        .play()
                        .with_context(|| "Unable to play")?;
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
                    // Build content with optional timestamp prefix
                    let content = if app.show_timestamp {
                        if let Some(ts) = ui_fragmentum.fragmentum.timestamp_start {
                            format!(
                                "[{}] {}",
                                format_timestamp(ts),
                                ui_fragmentum.fragmentum.content
                            )
                        } else {
                            ui_fragmentum.fragmentum.content.clone()
                        }
                    } else {
                        ui_fragmentum.fragmentum.content.clone()
                    };
                    // Spawn thread to keep clipboard alive until content is read
                    #[cfg(target_os = "linux")]
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            // Use Linux extension to wait until clipboard is read
                            let _ = clipboard.set().wait().text(content);
                        }
                    });
                    #[cfg(not(target_os = "linux"))]
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            let _ = clipboard.set_text(content);
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
                    // Concatenate all fragmenta content with newlines, optionally with timestamps
                    let show_ts = app.show_timestamp;
                    let all_content: String = selected_folio
                        .fragmenta
                        .iter()
                        .map(|f| {
                            if show_ts {
                                if let Some(ts) = f.fragmentum.timestamp_start {
                                    format!("[{}] {}", format_timestamp(ts), f.fragmentum.content)
                                } else {
                                    f.fragmentum.content.clone()
                                }
                            } else {
                                f.fragmentum.content.clone()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    // Spawn thread to keep clipboard alive until content is read
                    #[cfg(target_os = "linux")]
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            // Use Linux extension to wait until clipboard is read
                            let _ = clipboard.set().wait().text(all_content);
                        }
                    });
                    #[cfg(not(target_os = "linux"))]
                    std::thread::spawn(move || {
                        if let Ok(mut clipboard) = Clipboard::new() {
                            let _ = clipboard.set_text(all_content);
                        }
                    });
                }
            }

            // Toggle timestamp display on fragmenta
            (KeyCode::Char('t'), KeyModifiers::NONE) => {
                if app.current_region == CurrentRegion::Fragmentum {
                    app.show_timestamp = !app.show_timestamp;
                }
            }

            // Open settings screen
            (KeyCode::Char('s'), KeyModifiers::NONE) => app.enter_settings_screen(),

            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in add codex screen
    pub async fn handle_add_or_modify_codex_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                app.input_state.move_cursor_to_start();
                Ok(())
            }
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                app.input_state.move_cursor_to_end();
                Ok(())
            }
            (KeyCode::Esc, KeyModifiers::NONE) => {
                app.exit_add_or_modify_codex_without_saving();
                Ok(())
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                app.input_state.remove_char_before_cursor();
                Ok(())
            }
            (KeyCode::Delete, KeyModifiers::NONE) => {
                app.input_state.delete_char_after_cursor();
                Ok(())
            }
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => {
                app.input_state.add_char(value);
                Ok(())
            }
            (KeyCode::Left, KeyModifiers::NONE) => {
                app.input_state.move_cursor_left();
                Ok(())
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                app.input_state.move_cursor_right();
                Ok(())
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let codex_name = app.input_state.get_text().to_string();
                // Only do something if the codex has a name
                if !codex_name.trim().is_empty() {
                    if app.input_state.is_modifying {
                        CodicesComponent::update_codex(
                            &mut app.codices_component,
                            codex_name,
                            &app.pool,
                        )
                        .await
                        .with_context(|| "Failed to update codex")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    } else {
                        CodicesComponent::create_codex(
                            &mut app.codices_component,
                            codex_name,
                            &app.pool,
                        )
                        .await
                        .with_context(|| "Failed to create codex")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Handle key press from user in add folio screen
    pub async fn handle_add_or_modify_folio_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_start(),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_end(),
            (KeyCode::Esc, KeyModifiers::NONE) => app.exit_add_or_modify_folio_without_saving(),
            (KeyCode::Backspace, KeyModifiers::NONE) => app.input_state.remove_char_before_cursor(),
            (KeyCode::Delete, KeyModifiers::NONE) => app.input_state.delete_char_after_cursor(),
            (KeyCode::Left, KeyModifiers::NONE) => app.input_state.move_cursor_left(),
            (KeyCode::Right, KeyModifiers::NONE) => app.input_state.move_cursor_right(),
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => app.input_state.add_char(value),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let folio_name = app.input_state.get_text().to_string();
                if !folio_name.trim().is_empty()
                    && let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                {
                    if app.input_state.is_modifying {
                        FoliaComponent::update_item(selected_codex, folio_name, &app.pool)
                            .await
                            .with_context(|| "Failed to update item")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    } else {
                        FoliaComponent::create_item(
                            selected_codex,
                            folio_name,
                            &mut app.stt_tools,
                            &app.pool,
                        )
                        .await
                        .with_context(|| "Failed to create item")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();

                        // Post-import UI updates for new folio: select, expand, scroll
                        let codex = app
                            .codices_component
                            .get_selected_codex_mut()
                            .ok_or_else(|| anyhow::anyhow!("Codex should exist"))?;
                        let folio_count = codex.folia.len();
                        let previous_folio_idx = codex.folio_state.selected();
                        codex.expand();
                        let scroll_amount = if folio_count > 0 {
                            codex.folio_state.select(Some(folio_count - 1));
                            if let Some(selected_folio) = codex.folia.get_mut(folio_count - 1) {
                                if !selected_folio.fragmenta.is_empty() {
                                    selected_folio
                                        .fragmentum_state
                                        .select(Some(selected_folio.fragmenta.len() - 1));
                                }
                            }
                            match previous_folio_idx {
                                None => folio_count as u16,
                                Some(k) => (folio_count - 1 - k) as u16,
                            }
                        } else {
                            0
                        };
                        if scroll_amount > 0 {
                            app.codices_component
                                .list_state
                                .scroll_down_by(scroll_amount);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle change of archivum
    pub async fn handle_change_archivum_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => app.exit_change_archivum_without_saving(),
            KeyCode::Up => app.select_previous_archivum(),
            KeyCode::Down => app.select_next_archivum(),
            KeyCode::Enter => {
                app.switch_to_selected_archivum()
                    .await
                    .with_context(|| "Failed to switch archivum")?;
            }
            KeyCode::Char('a') => app.enter_add_archivum_screen(),
            KeyCode::Char('m') => {
                if let Some(archivum) = app.config.dbs.get(app.selected_archivum_index) {
                    app.enter_modify_archivum_screen(archivum.name.clone());
                }
            }
            KeyCode::Char('d') => {
                // Can't delete a db if it's the only db left
                if app.config.dbs.len() > 1 {
                    app.enter_delete_archivum_screen();
                }
            }
            KeyCode::Char('s') => {
                // Set selected archivum as default
                app.set_selected_archivum_as_default()
                    .await
                    .with_context(|| "Failed to set archivum as default")?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in add or modify archivum screen
    pub async fn handle_add_or_modify_archivum_screen_key(
        app: &mut App,
        key: KeyEvent,
    ) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_start(),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_end(),
            (KeyCode::Esc, KeyModifiers::NONE) => {
                if app.input_state.is_modifying {
                    app.exit_modify_archivum_without_saving();
                } else {
                    app.exit_add_archivum_without_saving();
                }
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => app.input_state.remove_char_before_cursor(),
            (KeyCode::Delete, KeyModifiers::NONE) => app.input_state.delete_char_after_cursor(),
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => app.input_state.add_char(value),
            (KeyCode::Left, KeyModifiers::NONE) => app.input_state.move_cursor_left(),
            (KeyCode::Right, KeyModifiers::NONE) => app.input_state.move_cursor_right(),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let archivum_name = app.input_state.get_text().to_string();
                if !archivum_name.trim().is_empty() {
                    if app.input_state.is_modifying {
                        app.rename_archivum(archivum_name)
                            .with_context(|| "Failed to rename archivum")?;
                        app.current_screen = CurrentScreen::ChangeArchivum;
                        app.input_state.clear();
                    } else {
                        app.create_new_archivum(archivum_name, false)
                            .await
                            .with_context(|| "Failed to create archivum")?;
                        app.current_screen = CurrentScreen::ChangeArchivum;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in delete archivum screen
    pub async fn handle_delete_archivum_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_start(),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_end(),
            (KeyCode::Esc, KeyModifiers::NONE) => app.exit_delete_archivum_without_saving(),
            (KeyCode::Backspace, KeyModifiers::NONE) => app.input_state.remove_char_before_cursor(),
            (KeyCode::Delete, KeyModifiers::NONE) => app.input_state.delete_char_after_cursor(),
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => app.input_state.add_char(value),
            (KeyCode::Left, KeyModifiers::NONE) => app.input_state.move_cursor_left(),
            (KeyCode::Right, KeyModifiers::NONE) => app.input_state.move_cursor_right(),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if let Some(archivum) = app.config.dbs.get(app.selected_archivum_index) {
                    let typed = app.input_state.get_text().trim();
                    // AWS inspired: delete only if the typed in text matches the actual name
                    if typed == archivum.name {
                        app.delete_archivum()
                            .await
                            .with_context(|| "Failed to delete archivum")?;
                        app.current_screen = CurrentScreen::ChangeArchivum;
                        app.input_state.clear();
                    }
                    // If no match, do nothing and stay in popup
                    // That is, don't exit until the user gets the name right
                    // or exits with esc on purpose
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in delete codex screen
    pub async fn handle_delete_codex_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_start(),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_end(),
            (KeyCode::Esc, KeyModifiers::NONE) => app.exit_delete_codex_or_folio_without_saving(),
            (KeyCode::Backspace, KeyModifiers::NONE) => app.input_state.remove_char_before_cursor(),
            (KeyCode::Delete, KeyModifiers::NONE) => app.input_state.delete_char_after_cursor(),
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => app.input_state.add_char(value),
            (KeyCode::Left, KeyModifiers::NONE) => app.input_state.move_cursor_left(),
            (KeyCode::Right, KeyModifiers::NONE) => app.input_state.move_cursor_right(),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if let (Some(codex), None) = app.codices_component.get_selected_codex_and_folio() {
                    let typed = app.input_state.get_text().trim();
                    if typed == codex.codex.name {
                        app.codices_component
                            .delete_selected(&app.pool)
                            .await
                            .with_context(|| "Failed to delete codex")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in delete folio screen
    pub async fn handle_delete_folio_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_start(),
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => app.input_state.move_cursor_to_end(),
            (KeyCode::Esc, KeyModifiers::NONE) => app.exit_delete_codex_or_folio_without_saving(),
            (KeyCode::Backspace, KeyModifiers::NONE) => app.input_state.remove_char_before_cursor(),
            (KeyCode::Delete, KeyModifiers::NONE) => app.input_state.delete_char_after_cursor(),
            (KeyCode::Char(value), KeyModifiers::NONE)
            | (KeyCode::Char(value), KeyModifiers::SHIFT) => app.input_state.add_char(value),
            (KeyCode::Left, KeyModifiers::NONE) => app.input_state.move_cursor_left(),
            (KeyCode::Right, KeyModifiers::NONE) => app.input_state.move_cursor_right(),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if let (Some(_), Some(folio)) = app.codices_component.get_selected_codex_and_folio()
                {
                    let typed = app.input_state.get_text().trim();
                    if typed == folio.folio.name {
                        app.codices_component
                            .delete_selected(&app.pool)
                            .await
                            .with_context(|| "Failed to delete folio")?;
                        app.current_screen = CurrentScreen::Main;
                        app.input_state.clear();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in recording screen
    pub async fn handle_record_folio_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match key.code {
            // Toggle pause/resume
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

                // Wait for threads to finish and extract the returned models for reuse
                let vad_model = if let Some(fractor_handle) = app.fractor_handle.take() {
                    let (_temp_dir, vad) = fractor_handle
                        .join()
                        .map_err(|_| anyhow::anyhow!("Recording thread panicked"))?
                        .with_context(|| "Recording failed")?;
                    Some(vad)
                } else {
                    None
                };

                let stt_model = if let Some(transcriber_handle) = app.transcriber_handle.take() {
                    let stt = transcriber_handle
                        .join()
                        .map_err(|_| anyhow::anyhow!("Transcribing thread panicked"))?
                        .with_context(|| "Transcribing failed")?;
                    Some(stt)
                } else {
                    None
                };

                // Refresh the folio's fragmenta from DB
                if let Some(selected_codex) = app.codices_component.get_selected_codex_mut()
                    && let Some(folio_idx) = selected_codex.folio_state.selected()
                    && let Some(selected_folio) = selected_codex.folia.get_mut(folio_idx)
                {
                    selected_folio
                        .update_fragmenta(&app.pool)
                        .await
                        .with_context(|| "Failed to refresh fragmenta")?;
                }

                // Restore STT tools using the returned models (no reloading from disk!)
                if let (Some(vad), Some(stt)) = (vad_model, stt_model) {
                    app.stt_tools
                        .restore_from_recording(&app.config, vad, stt)
                        .with_context(|| "Failed to restore STT tools")?;
                }

                // Clear recording state
                app.recording_stop_signal = None;
                app.recording_pause_signal = None;
                app.recording_folio_id = None;
                app.is_recording = false;
                app.is_paused = false;
                app.fractor_handle = None;
                app.transcriber_handle = None;
                app.recording_screen_start = None;

                // Return to main screen
                app.current_screen = CurrentScreen::Main;
            }

            _ => {}
        }
        Ok(())
    }

    /// Handle key press from user in settings screen
    pub async fn handle_settings_screen_key(app: &mut App, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            // Discard changes and return to main screen
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                app.exit_settings_without_saving();
            }

            // Save to session only (don't write to file)
            (KeyCode::Char('s'), KeyModifiers::NONE) => {
                app.save_settings_to_session()
                    .await
                    .with_context(|| "Failed to save settings to session")?;
            }

            // Save as default (write to file)
            (KeyCode::Char('S'), KeyModifiers::SHIFT) => {
                app.save_settings_as_default()
                    .await
                    .with_context(|| "Failed to save settings as default")?;
            }

            // Move to previous field
            (KeyCode::Up, KeyModifiers::NONE) => {
                if let Some(settings) = &mut app.settings_state {
                    settings.previous_field();
                }
            }

            // Move to next field
            (KeyCode::Down, KeyModifiers::NONE) => {
                if let Some(settings) = &mut app.settings_state {
                    settings.next_field();
                }
            }

            // Adjust value (Left)
            (KeyCode::Left, KeyModifiers::NONE) => {
                if let Some(settings) = &mut app.settings_state {
                    match settings.active_field {
                        SettingsField::InputDevice => settings.previous_device(),
                        SettingsField::VadThreshold => settings.decrease_threshold(),
                        SettingsField::MinFragmentumDurationSeconds => {
                            settings.decrease_min_fragmentum_duration()
                        }
                        SettingsField::MaxFragmentumDurationSeconds => {
                            settings.decrease_max_fragmentum_duration()
                        }
                        SettingsField::PauseThresholdInChunks => {
                            settings.decrease_pause_threshold()
                        }
                        SettingsField::STTModel => settings.previous_stt_model(),
                        SettingsField::VADModel => settings.previous_vad_model(),
                    }
                }
            }

            // Adjust value (Right)
            (KeyCode::Right, KeyModifiers::NONE) => {
                if let Some(settings) = &mut app.settings_state {
                    match settings.active_field {
                        SettingsField::InputDevice => settings.next_device(),
                        SettingsField::VadThreshold => settings.increase_threshold(),
                        SettingsField::MinFragmentumDurationSeconds => {
                            settings.increase_min_fragmentum_duration()
                        }
                        SettingsField::MaxFragmentumDurationSeconds => {
                            settings.increase_max_fragmentum_duration()
                        }
                        SettingsField::PauseThresholdInChunks => {
                            settings.increase_pause_threshold()
                        }
                        SettingsField::STTModel => settings.next_stt_model(),
                        SettingsField::VADModel => settings.next_vad_model(),
                    }
                }
            }

            _ => {}
        }
        Ok(())
    }

    /// Starts the recording pipeline for a folio.
    ///
    /// When `is_extending` is true, appends to the existing folio (no new folio created,
    /// timestamps continue from the last fragmentum). When false, use for a newly created folio.
    async fn start_recording_for_folio(
        app: &mut App,
        fractor: Fractor,
        stt_model: STTModel,
        codex_id: i64,
        folio_id: i64,
        is_extending: bool,
    ) -> Result<()> {
        // Output directory: audios/{codex_id}/{folio_id}
        let output_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Unable to get data directory"))?
            .join("scriptor")
            .join("audios")
            .join(format!("{:04}", codex_id))
            .join(format!("{:04}", folio_id));
        fs::create_dir_all(&output_dir).with_context(|| "Unable to create audio directory")?;

        // When extending, continue timestamps from the last fragmentum
        let initial_offset_secs = if is_extending {
            Fragmentum::get_max_timestamp_end_for_folio(&app.pool, folio_id)
                .await
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let stop_signal = Arc::new(AtomicBool::new(false));
        let pause_signal = Arc::new(AtomicBool::new(false));
        app.recording_stop_signal = Some(Arc::clone(&stop_signal));
        app.recording_pause_signal = Some(Arc::clone(&pause_signal));
        app.recording_folio_id = Some(folio_id);

        let (tx, rx) = create_fragmentum_channel(app.stt_tools.max_queue_elements);
        let pool_clone = app.pool.clone();
        let runtime_handle = tokio::runtime::Handle::current();

        let fractor_handle = std::thread::spawn(move || {
            fractor.run(
                Some(output_dir),
                stop_signal,
                pause_signal,
                tx,
                initial_offset_secs,
            )
        });

        let transcriber_handle = std::thread::spawn(move || {
            transcriber_to_db_worker(stt_model, folio_id, pool_clone, rx, runtime_handle)
        });

        app.fractor_handle = Some(fractor_handle);
        app.transcriber_handle = Some(transcriber_handle);
        app.is_recording = true;
        app.is_paused = false;
        app.current_screen = CurrentScreen::RecordFolio;
        app.recording_screen_start = Some(std::time::Instant::now());

        Ok(())
    }
}
