use crate::configs::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, Widget};

pub struct AppLayout;

impl AppLayout {
    /// Calculate responsive layout areas
    /// Returns: (codices_header_area, codices_area, bookmark_area, fragmenta_header_area, fragmenta_area, codex_footer_area)
    pub fn calculate_main_layout(area: Rect) -> (Rect,Rect, Rect, Rect, Rect) {

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
        ] = content_layout.areas(area);

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
        let block = Block::default().padding(Padding::new(0, 0, 2, 0));

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
