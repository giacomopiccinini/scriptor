use crate::tui::db::models::{Folio, NewFolio, UICodex};
use anyhow::Result;
use sqlx::SqlitePool;

pub struct FoliaComponent;

impl FoliaComponent {
    /// Select next folio in the list
    pub fn select_next_folio(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select_next();
    }

    /// Select previous folio in the list
    pub fn select_previous_folio(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select_previous();
    }

    /// Remove folio selection (deselect current folio)
    pub fn remove_folio_selection(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select(None);
    }

    /// Select the first folio in the list
    pub fn select_first_item(ui_codex: &mut UICodex) {
        if ui_codex.folio_state.selected().is_none() {
            ui_codex.folio_state.select_first();
        }
    }

    /// Create a new folio in the given codex
    pub async fn create_item(
        ui_codex: &mut UICodex,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        let new_folio = NewFolio {
            name,
            codex_id: ui_codex.codex.id,
        };

        Folio::create(pool, new_folio).await?;
        ui_codex.update_folia(pool).await?;
        Ok(())
    }

    /// Update an existing folio
    pub async fn update_item(
        ui_codex: &mut UICodex,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        println!("OK here");
        if let Some(j) = ui_codex.folio_state.selected() {
            println!("FOUND");
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.update_name(pool, name).await?;

            // Update folia
            ui_codex.update_folia(pool).await?;
        }
        Ok(())
    }

    /// Delete the currently selected folio
    pub async fn delete_selected_folio(ui_codex: &mut UICodex, pool: &SqlitePool) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let folio = ui_codex.folia[j].folio.clone();
            folio.delete(pool).await?;

            // Update folia
            ui_codex.update_folia(pool).await?;

            // Adjust selection after deletion - check bounds first
            if ui_codex.folia.is_empty() {
                ui_codex.folio_state.select(None);
            } else if j >= ui_codex.folia.len() {
                ui_codex.folio_state.select(Some(ui_codex.folia.len() - 1));
            }
        }
        Ok(())
    }

    /// Move the currently selected folio up
    pub async fn move_selected_folio_up(ui_codex: &mut UICodex, pool: &SqlitePool) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.move_up(pool).await?;

            // Update folia to reflect the new order
            ui_codex.update_folia(pool).await?;

            // Adjust selection to follow the moved folio
            if j > 0 {
                ui_codex.folio_state.select(Some(j - 1));
            }
        }
        Ok(())
    }

    /// Move the currently selected folio down
    pub async fn move_selected_folio_down(ui_codex: &mut UICodex, pool: &SqlitePool) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.move_down(pool).await?;

            // Update folia to reflect the new order
            ui_codex.update_folia(pool).await?;

            // Adjust selection to follow the moved folio
            if j + 1 < ui_codex.folia.len() {
                ui_codex.folio_state.select(Some(j + 1));
            }
        }
        Ok(())
    }

    // /// Render the list of folia for the selected codex
    // pub fn render(
    //     selected_codex: Option<&mut UICodex>,
    //     area: Rect,
    //     buf: &mut Buffer,
    //     theme: &ThemeConfig,
    // ) {
    //     // Command hints for folia
    //     let folio_command_hints = Line::from(vec![
    //         Span::raw(" "),
    //         Span::styled("[a]", Style::default().fg(theme.highlight)),
    //         Span::styled("dd", Style::default().fg(theme.foreground)),
    //         Span::styled(" [d]", Style::default().fg(theme.highlight)),
    //         Span::styled("el", Style::default().fg(theme.foreground)),
    //         Span::styled(" [m]", Style::default().fg(theme.highlight)),
    //         Span::styled("odify ", Style::default().fg(theme.foreground)),
    //         Span::raw(" "),
    //     ])
    //     .left_aligned();

    //     let block = Block::default()
    //         .padding(Padding::new(2, 2, 1, 1))
    //         .title_top(Line::raw("  F O L I O  ").left_aligned())
    //         .title_bottom(folio_command_hints)
    //         .title_alignment(Alignment::Center)
    //         .borders(Borders::TOP | Borders::BOTTOM | Borders::LEFT)
    //         .border_type(BorderType::Rounded);

    //     if let Some(ui_codex) = selected_codex {
    //         // Extract the folia
    //         let items: Vec<ListItem> = ui_codex
    //             .folia
    //             .iter()
    //             .map(|ui_folio| ListItem::from(ui_folio.folio.name.clone()))
    //             .collect();

    //         let list: List = List::new(items)
    //             .block(block)
    //             .highlight_symbol(" ▸ ")
    //             .highlight_style(
    //                 // Swap foreground and background for selected item
    //                 Style::default().bg(theme.foreground).fg(theme.background),
    //             )
    //             .highlight_spacing(HighlightSpacing::Always);

    //         StatefulWidget::render(list, area, buf, &mut ui_codex.folio_state);
    //     } else {
    //         // No codex selected - render empty block
    //         block.render(area, buf);
    //     }
    // }
}
