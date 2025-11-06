use crate::tui::db::config::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Widget};
use ratatui::widgets::{
    BorderType, Borders, HighlightSpacing, List, ListItem, ListState, Padding, StatefulWidget,
};

pub struct AppLayout;

impl AppLayout {
    /// Calculate responsive layout areas
    /// Returns: (codices_header_area, codices_area, bookmark_area, fragmenta_header_area, fragmenta_area, logo_area)
    pub fn calculate_main_layout(area: Rect) -> (Rect, Rect, Rect, Rect, Rect, Rect) {
        // Add overall padding around the entire TUI
        // Adjust these values to control how much space you want from terminal borders
        let padded_area = area.inner(Margin {
            horizontal: 2, // 2 columns of padding on left and right
            vertical: 1,   // 1 row of padding on top and bottom
        });

        // Calculate responsive header height based on terminal size
        let header_height = if padded_area.height < 15 {
            // Very small terminal - minimal header
            Constraint::Length(3)
        } else if padded_area.height < 25 {
            // Small terminal - reduced header
            Constraint::Length(8)
        } else {
            // Normal terminal - full header
            Constraint::Percentage(20)
        };

        let main_layout = Layout::vertical([
            header_height,
            Constraint::Min(10), // Ensure minimum content area
        ]);

        // Extract the areas from the main layout using the padded area
        let [header_area, content_area] = main_layout.areas(padded_area);

        // Use entire header area for logo
        let logo_area = header_area;

        // Subdivide the content area into three columns: codex, folio, fragmentum
        let content_layout = Layout::horizontal([
            Constraint::Percentage(48), // Codex column
            Constraint::Percentage(4),  // Bookmark column
            Constraint::Percentage(48), // Fragmentum column
        ]);

        // Extract the areas for codices, folia, and fragmenta
        let [
            codices_and_header_area,
            bookmark_area,
            fragmenta_and_header_area,
        ] = content_layout.areas(content_area);

        // Page layout for both codex and fragment
        let page_layout =
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(80)]);
        let [codices_header_area, codices_area] = page_layout.areas(codices_and_header_area);
        let [fragmenta_header_area, fragmenta_area] = page_layout.areas(fragmenta_and_header_area);

        (
            codices_header_area,
            codices_area,
            bookmark_area,
            fragmenta_header_area,
            fragmenta_area,
            logo_area,
        )
    }

    /// Render a background that fills the entire area
    pub fn render_background(area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        let background =
            Block::default().style(Style::default().bg(theme.background).fg(theme.foreground));
        background.render(area, buf);
    }

    /// Render a simple centered header title
    pub fn render_header(area: Rect, buf: &mut Buffer, title: &str, theme: &ThemeConfig) {
        let block = Block::default()
            .padding(Padding::new(0, 0, 1, 0))
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .border_type(BorderType::Rounded);

        let header_text = Line::from(vec![Span::styled(
            title,
            Style::default().fg(theme.highlight),
        )])
        .centered();

        let paragraph = Paragraph::new(header_text)
            .block(block)
            .style(Style::default().bg(theme.background));

        paragraph.render(area, buf);
    }

    /// Render the bookmark area (archivum selector) with red background and vertical text
    pub fn render_bookmark(area: Rect, buf: &mut Buffer, archivum_name: &str, theme: &ThemeConfig) {
        // Red background for bookmark area
        let background =
            Block::default().style(Style::default().bg(theme.highlight).fg(theme.background));
        background.render(area, buf);

        // Render archivum name vertically (one char per line)
        let chars: Vec<Line> = archivum_name
            .chars()
            .map(|c| Line::from(c.to_string()).centered())
            .collect();

        let paragraph =
            Paragraph::new(chars).style(Style::default().bg(theme.highlight).fg(theme.background));

        paragraph.render(area, buf);
    }
}
