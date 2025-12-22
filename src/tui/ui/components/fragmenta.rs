use crate::configs::theme::ThemeConfig;
use crate::tui::db::models::UIFolio;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, StatefulWidget, Widget};
use sqlx::SqlitePool;

pub struct FragmentaComponent;

impl FragmentaComponent {
    /// Select next fragmentum in the list
    pub fn select_next_fragmentum(ui_folio: &mut UIFolio) {
        ui_folio.fragmentum_state.select_next();
    }

    /// Select previous fragmentum in the list
    pub fn select_previous_fragmentum(ui_folio: &mut UIFolio) {
        ui_folio.fragmentum_state.select_previous();
    }

    /// Remove fragmentum selection (deselect current fragmentum)
    pub fn remove_fragmentum_selection(ui_folio: &mut UIFolio) {
        ui_folio.fragmentum_state.select(None);
    }

    /// Select the first fragmentum in the list
    pub fn select_first_fragmentum(ui_folio: &mut UIFolio) {
        if ui_folio.fragmentum_state.selected().is_none() {
            ui_folio.fragmentum_state.select_first();
        }
    }

    /// Delete the currently selected fragmentum
    pub async fn delete_selected_fragmentum(
        ui_folio: &mut UIFolio,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(k) = ui_folio.fragmentum_state.selected() {
            let fragmentum = ui_folio.fragmenta[k].fragmentum.clone();
            fragmentum.delete(pool).await?;

            // Update fragmenta
            ui_folio.update_fragmenta(pool).await?;

            // Adjust selection after deletion - check bounds first
            if ui_folio.fragmenta.is_empty() {
                ui_folio.fragmentum_state.select(None);
            } else if k >= ui_folio.fragmenta.len() {
                ui_folio
                    .fragmentum_state
                    .select(Some(ui_folio.fragmenta.len() - 1));
            }
        }
        Ok(())
    }

    /// Render the list of fragmenta for the selected folio
    pub fn render(
        selected_folio: Option<&mut UIFolio>,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Command hints for fragmenta
        let fragmentum_command_hints = Line::from(vec![
            Span::styled("[c]", Style::default().fg(theme.highlight)),
            Span::styled("opy ", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[C]", Style::default().fg(theme.highlight)),
            Span::styled("opy all ", Style::default().fg(theme.foreground)),
        ])
        .centered();

        let block = Block::default()
            .title_bottom(fragmentum_command_hints)
            .title_alignment(Alignment::Center);

        if let Some(ui_folio) = selected_folio {
            // Extract the fragmenta and display the first few characters of content
            let items: Vec<ListItem> = ui_folio
                .fragmenta
                .iter()
                .map(|ui_fragmentum| {
                    // Show first 50 characters of content as preview
                    let preview = if ui_fragmentum.fragmentum.content.len() > 50 {
                        format!("{}...", &ui_fragmentum.fragmentum.content[..50])
                    } else {
                        ui_fragmentum.fragmentum.content.clone()
                    };
                    ListItem::from(preview)
                })
                .collect();

            let list: List = List::new(items)
                .block(block)
                .highlight_symbol(" ▸ ")
                .highlight_style(
                    // Swap foreground and background for selected item
                    Style::default().bg(theme.foreground).fg(theme.background),
                )
                .highlight_spacing(HighlightSpacing::Always);

            StatefulWidget::render(list, area, buf, &mut ui_folio.fragmentum_state);
        } else {
            // No folio selected - render empty block
            block.render(area, buf);
        }
    }
}
