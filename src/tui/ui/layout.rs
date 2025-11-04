use crate::tui::db::config::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};

pub struct AppLayout;

impl AppLayout {
    /// Calculate responsive layout areas
    /// Returns: (codices_area, folia_area, fragmenta_area, logo_area, archivum_selector_area, closed_selector_area)
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

        // Divide header between pure logo and archivum selector
        let header_layout = Layout::horizontal([Constraint::Min(50), Constraint::Length(35)]);

        // Extract the areas from the header layout
        let [logo_area, archivum_selector_area] = header_layout.areas(header_area);

        // Split between closed and open selector
        let selector_layout = Layout::vertical([Constraint::Percentage(56), Constraint::Fill(1)]);

        // When the user changes archivum it opens up as a dropdown
        let [_, closed_selector_area] = selector_layout.areas(archivum_selector_area);

        // Subdivide the content area into three columns: codex, folio, fragmentum
        let content_layout = Layout::horizontal([
            Constraint::Percentage(25), // Codex column
            Constraint::Percentage(25), // Folio column
            Constraint::Percentage(50), // Fragmentum column (largest for text display)
        ]);

        // Extract the areas for codices, folia, and fragmenta
        let [codices_area, folia_area, fragmenta_area] = content_layout.areas(content_area);

        (
            codices_area,
            folia_area,
            fragmenta_area,
            logo_area,
            archivum_selector_area,
            closed_selector_area,
        )
    }

    /// Render a background that fills the entire area
    pub fn render_background(area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        let background =
            Block::default().style(Style::default().bg(theme.background).fg(theme.foreground));
        background.render(area, buf);
    }
}
