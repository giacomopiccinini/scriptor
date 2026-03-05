use crate::configs::theme::ThemeConfig;
use crate::tui::db::models::UIFolio;
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, StatefulWidget, Widget};
use sqlx::SqlitePool;
use textwrap::wrap;

/// Convert seconds to hh:mm:ss format
pub fn format_timestamp(seconds: f32) -> String {
    let total_seconds = seconds as u32;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

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

    /// Select fragmentum at given index
    pub fn jump_to_fragmentum(ui_folio: &mut UIFolio, fragmentum_idx: usize) {
        ui_folio.fragmentum_state.select(Some(fragmentum_idx))
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
        is_playing: bool,
        show_timestamp: bool,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Create play/pause text based on status
        let play_text = if is_playing {
            Span::styled("ause ", Style::default().fg(theme.dark_shadow))
        } else {
            Span::styled("lay ", Style::default().fg(theme.dark_shadow))
        };

        // Create timestamp toggle text based on status
        let timestamp_text = if show_timestamp {
            Span::styled("imestamp off ", Style::default().fg(theme.dark_shadow))
        } else {
            Span::styled("imestamp on ", Style::default().fg(theme.dark_shadow))
        };

        // Command hints for fragmenta
        let fragmentum_command_hints = Line::from(vec![
            Span::styled("[p]", Style::default().fg(theme.highlight)),
            play_text,
            Span::raw("   "),
            Span::styled("[c]", Style::default().fg(theme.highlight)),
            Span::styled("opy ", Style::default().fg(theme.dark_shadow)),
            Span::raw("   "),
            Span::styled("[C]", Style::default().fg(theme.highlight)),
            Span::styled("opy all ", Style::default().fg(theme.dark_shadow)),
            Span::raw("   "),
            Span::styled("[t]", Style::default().fg(theme.highlight)),
            timestamp_text,
        ])
        .centered();

        let block = Block::default()
            .title_bottom(fragmentum_command_hints)
            .title_alignment(Alignment::Center);

        if let Some(ui_folio) = selected_folio {
            // Calculate available width for text wrapping
            // Account for: highlight symbol " ▸ " (4 chars) + some margin
            let highlight_symbol = "  ";
            let highlight_width = highlight_symbol.chars().count();
            let available_width = area.width.saturating_sub(highlight_width as u16 + 2) as usize;

            // Wrap each fragmentum's content to fit the available width
            let items: Vec<ListItem> = ui_folio
                .fragmenta
                .iter()
                .map(|ui_fragmentum| {
                    let content = &ui_fragmentum.fragmentum.content;

                    // Build the first line with optional timestamp prefix
                    let first_line_spans: Vec<Span> = if show_timestamp {
                        if let Some(timestamp_start) = ui_fragmentum.fragmentum.timestamp_start {
                            let timestamp_str = format!("[{}] ", format_timestamp(timestamp_start));
                            vec![Span::styled(
                                timestamp_str,
                                Style::default().fg(theme.highlight),
                            )]
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    };

                    // Use textwrap to wrap the content into multiple lines
                    let wrapped_lines: Vec<Line> = if available_width > 0 {
                        let wrapped = wrap(content, available_width);
                        wrapped
                            .iter()
                            .enumerate()
                            .map(|(i, line)| {
                                if i == 0 && !first_line_spans.is_empty() {
                                    // First line: prepend timestamp spans
                                    let mut spans = first_line_spans.clone();
                                    spans.push(Span::raw(line.to_string()));
                                    Line::from(spans)
                                } else {
                                    Line::from(line.to_string())
                                }
                            })
                            .collect()
                    } else if !first_line_spans.is_empty() {
                        let mut spans = first_line_spans.clone();
                        spans.push(Span::raw(content.clone()));
                        vec![Line::from(spans)]
                    } else {
                        vec![Line::from(content.clone())]
                    };

                    // Add empty line separator between fragmenta for visual clarity
                    let mut lines = wrapped_lines;
                    lines.push(Line::from(""));

                    ListItem::new(Text::from(lines))
                })
                .collect();

            let list: List = List::new(items)
                .block(block)
                .style(Style::default().fg(theme.medium_shadow))
                .highlight_symbol(highlight_symbol)
                .highlight_style(
                    // Swap foreground and background for selected item
                    Style::default().bg(theme.dark_shadow).fg(theme.page),
                )
                .highlight_spacing(HighlightSpacing::Always);

            StatefulWidget::render(list, area, buf, &mut ui_folio.fragmentum_state);
        } else {
            // No folio selected - render empty block
            block.render(area, buf);
        }
    }
}
