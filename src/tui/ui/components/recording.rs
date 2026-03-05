use crate::configs::theme::ThemeConfig;
use crate::tui::db::models::UIFolio;
use crate::tui::ui::components::overlay_window::OverlayWindow;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, HighlightSpacing, List, ListItem, Padding, Paragraph, StatefulWidget, Widget,
};
use textwrap::wrap;

pub struct RecordingScreen;

impl RecordingScreen {
    /// Render the recording screen inside an overlay window.
    /// The overlay sits on top of the main UI, clearing only its own area.
    pub fn render(
        is_paused: bool,
        selected_folio: Option<&UIFolio>,
        dots: &str,
        list_state: &mut ratatui::widgets::ListState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Build footer command hints (empty left, commands on right)
        let footer_hints_left = Line::from("");
        let footer_hints_right = Self::build_footer_hints(is_paused, theme);

        // Render the overlay window and get the inner content area
        let content_area = OverlayWindow::render(
            footer_hints_left,
            footer_hints_right,
            Some(60),
            Some(80),
            area,
            buf,
            theme,
        );

        // Render the recording content inside the overlay
        Self::render_content(
            is_paused,
            selected_folio,
            dots,
            list_state,
            content_area,
            buf,
            theme,
        );
    }

    /// Build the footer command hints line (for right side)
    fn build_footer_hints(is_paused: bool, theme: &ThemeConfig) -> Line<'static> {
        let pause_text = if is_paused { "resume" } else { "pause" };

        Line::from(vec![
            Span::styled("[Space]", Style::default().fg(theme.highlight)),
            Span::styled(
                format!(" {} ", pause_text),
                Style::default().fg(theme.dark_shadow),
            ),
            Span::styled("[Esc]", Style::default().fg(theme.highlight)),
            Span::styled(" stop ", Style::default().fg(theme.dark_shadow)),
        ])
    }

    /// Render the recording content (header + fragmenta) inside the given area
    fn render_content(
        is_paused: bool,
        selected_folio: Option<&UIFolio>,
        dots: &str,
        list_state: &mut ratatui::widgets::ListState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        // Calculate layout: header, content
        let layout = Layout::vertical([
            Constraint::Length(3), // Header with status
            Constraint::Min(5),    // Fragmenta content
        ]);
        let [header_area, fragmenta_area] = layout.areas(area);

        // Render header with status indicator
        Self::render_header(is_paused, header_area, buf, theme);

        // Render fragmenta list
        Self::render_fragmenta(selected_folio, dots, list_state, fragmenta_area, buf, theme);
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
            .style(Style::default().bg(theme.page));

        paragraph.render(area, buf);
    }

    fn render_fragmenta(
        selected_folio: Option<&UIFolio>,
        dots: &str,
        list_state: &mut ratatui::widgets::ListState,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) {
        let block = Block::default();

        if let Some(folio) = selected_folio {
            // Calculate available width for text wrapping
            let available_width = area.width.saturating_sub(4) as usize;

            // Create list items from fragmenta
            let mut items: Vec<ListItem> = folio
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

                    ListItem::new(Text::from(lines).style(Style::default().fg(theme.dark_shadow)))
                })
                .collect();

            // Add animated dots loading indicator at the end (theme.highlight, no highlight/arrow)
            items.push(ListItem::new(Line::from(Span::styled(
                dots,
                Style::default().fg(theme.highlight),
            ))));

            // Select last item (dots) for autoscroll; use empty symbol and matching style so nothing looks selected
            list_state.select(Some(items.len() - 1));

            let list = List::new(items)
                .block(block)
                .highlight_symbol("")
                .highlight_style(Style::default().fg(theme.dark_shadow))
                .highlight_spacing(HighlightSpacing::WhenSelected);

            StatefulWidget::render(list, area, buf, list_state);
        } else {
            // No folio selected - show waiting message
            let waiting_text = Paragraph::new("Waiting for transcription...")
                .block(block)
                .style(Style::default().fg(theme.dark_shadow).bg(theme.page))
                .alignment(Alignment::Center);

            waiting_text.render(area, buf);
        }
    }
}
