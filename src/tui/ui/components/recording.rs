use crate::configs::theme::ThemeConfig;
use crate::tui::db::models::UIFolio;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, Padding, Paragraph, Widget};
use textwrap::wrap;

pub struct RecordingScreen;

impl RecordingScreen {
    /// Render the recording screen with status indicator and live fragmenta list
    pub fn render(
        is_paused: bool,
        selected_folio: Option<&UIFolio>,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Clear background
        let background =
            Block::default().style(Style::default().bg(theme.background).fg(theme.foreground));
        background.render(area, buf);

        // Calculate layout: header, content, footer
        let layout = Layout::vertical([
            Constraint::Length(5), // Header with status
            Constraint::Min(10),   // Fragmenta content
            Constraint::Length(3), // Footer with commands
        ]);
        let [header_area, content_area, footer_area] = layout.areas(area);

        // Render header with status indicator
        Self::render_header(is_paused, header_area, buf, theme);

        // Render fragmenta list
        Self::render_fragmenta(selected_folio, content_area, buf, theme);

        // Render footer with command hints
        Self::render_footer(is_paused, footer_area, buf, theme);
    }

    fn render_header(is_paused: bool, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        let block = Block::default().padding(Padding::new(0, 0, 1, 0));

        let status_text = if is_paused {
            "P A U S E D"
        } else {
            "R E C O R D I N G"
        };

        let header_text = Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(theme.highlight),
        )])
        .centered();

        let paragraph = Paragraph::new(header_text)
            .block(block)
            .style(Style::default().bg(theme.background));

        paragraph.render(area, buf);
    }

    fn render_fragmenta(
        selected_folio: Option<&UIFolio>,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        let block = Block::default();

        if let Some(folio) = selected_folio {
            // Calculate available width for text wrapping
            let available_width = area.width.saturating_sub(4) as usize;

            // Create list items from fragmenta
            let items: Vec<ListItem> = folio
                .fragmenta
                .iter()
                .map(|ui_fragmentum| {
                    let content = &ui_fragmentum.fragmentum.content;

                    // Wrap text to fit available width
                    let wrapped_lines: Vec<Line> = if available_width > 0 {
                        wrap(content, available_width)
                            .iter()
                            .map(|line| Line::from(line.to_string()))
                            .collect()
                    } else {
                        vec![Line::from(content.clone())]
                    };

                    // Add separator
                    let mut lines = wrapped_lines;
                    lines.push(Line::from(""));

                    ListItem::new(Text::from(lines))
                })
                .collect();

            let list = List::new(items)
                .block(block)
                .highlight_symbol(" ▸ ")
                .highlight_style(Style::default().bg(theme.foreground).fg(theme.background))
                .highlight_spacing(HighlightSpacing::Always);

            list.render(area, buf);
        } else {
            // No folio selected - show waiting message
            let waiting_text = Paragraph::new("Waiting for transcription...")
                .block(block)
                .style(Style::default().fg(theme.foreground).bg(theme.background))
                .alignment(Alignment::Center);

            waiting_text.render(area, buf);
        }
    }

    fn render_footer(is_paused: bool, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        let pause_text = if is_paused { "resume" } else { "pause" };

        let command_hints = Line::from(vec![
            Span::styled("[Space]", Style::default().fg(theme.highlight)),
            Span::styled(
                format!(" {} ", pause_text),
                Style::default().fg(theme.foreground),
            ),
            Span::raw("   "),
            Span::styled("[Esc]", Style::default().fg(theme.highlight)),
            Span::styled(" stop ", Style::default().fg(theme.foreground)),
        ])
        .centered();

        let paragraph = Paragraph::new(command_hints)
            .style(Style::default().bg(theme.background))
            .alignment(Alignment::Center);

        paragraph.render(area, buf);
    }
}
