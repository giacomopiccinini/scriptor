use crate::tui::db::models::UIFolio;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, HighlightSpacing, List, ListItem, Padding, StatefulWidget, Widget,
};
use sqlx::SqlitePool;
use std::str::FromStr;

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
    pub fn render(selected_folio: Option<&mut UIFolio>, area: Rect, buf: &mut Buffer) {
        // Command hints for fragmenta
        let fragmentum_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled(" ↑↓ ", Style::default()),
            Span::styled(
                "[c]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::styled(
                "opy ",
                Style::default().fg(Color::from_str("#FCF1D5").unwrap()),
            ),
            Span::raw(" "),
        ])
        .left_aligned();

        // Add "quit" hint, in the bottom right corner
        let quit_hint = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "[q]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::styled(
                "uit ",
                Style::default().fg(Color::from_str("#FCF1D5").unwrap()),
            ),
            Span::raw(" "),
        ])
        .right_aligned();

        let block = Block::default()
            .padding(Padding::new(2, 2, 1, 1))
            .title_top(Line::raw("  F R A G M E N T U M  ").left_aligned())
            .title_bottom(fragmentum_command_hints)
            .title_bottom(quit_hint)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

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
                    Style::default()
                        .bg(Color::from_str("#FCF1D5").unwrap())
                        .fg(Color::from_str("#002626").unwrap()),
                )
                .highlight_spacing(HighlightSpacing::Always);

            StatefulWidget::render(list, area, buf, &mut ui_folio.fragmentum_state);
        } else {
            // No folio selected - render empty block
            block.render(area, buf);
        }
    }
}
