use crate::configs::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Widget};

const WINDOW_WIDTH_PERCENTAGE_DEFAULT: usize = 60;
const WINDOW_HEIGHT_PERCENTAGE_DEFAULT: usize = 80;

/// A reusable overlay window component that renders a centered window on top of the main UI.
/// The window clears only its own area, leaving the main UI visible around the edges.
pub struct OverlayWindow;

impl OverlayWindow {
    /// Render an overlay window and return the inner content area.
    ///
    /// # Arguments
    /// * `footer_hints_left` - Command hints displayed at the bottom left of the window
    /// * `footer_hints_right` - Command hints displayed at the bottom right of the window
    /// * `window_width_percentage` - The width of the window as a percentage of the terminal width
    /// * `window_height_percentage` - The height of the window as a percentage of the terminal height
    /// * `area` - The full terminal area to center the window within
    /// * `buf` - The buffer to render into
    /// * `theme` - The theme configuration for styling
    ///
    /// # Returns
    /// The inner `Rect` area where content should be rendered
    pub fn render(
        footer_hints_left: Line<'_>,
        footer_hints_right: Line<'_>,
        window_width_percentage: Option<usize>,
        window_height_percentage: Option<usize>,
        area: Rect,
        buf: &mut Buffer,
        theme: &ThemeConfig,
    ) -> Rect {
        // Default to fixed percentages if not provided
        let window_width_percentage =
            window_width_percentage.unwrap_or(WINDOW_WIDTH_PERCENTAGE_DEFAULT);
        let window_height_percentage =
            window_height_percentage.unwrap_or(WINDOW_HEIGHT_PERCENTAGE_DEFAULT);

        // Calculate window dimensions
        let window_width = (area.width * window_width_percentage as u16) / 100;
        let window_height = (area.height * window_height_percentage as u16) / 100;

        // Center horizontally within the area
        let window_x = area.x + (area.width.saturating_sub(window_width)) / 2;

        // Center vertically within the area
        let window_y = area.y + (area.height.saturating_sub(window_height)) / 2;

        // Define the window area
        let window_area = Rect {
            x: window_x,
            y: window_y,
            width: window_width,
            height: window_height,
        };

        // Clear only the window area (main UI still visible around edges)
        Clear.render(window_area, buf);

        // Render background for the window
        Block::default()
            .style(Style::default().bg(theme.background))
            .render(window_area, buf);

        // Define the window block with styling
        // Use left-aligned and right-aligned lines for footer hints
        let window_block = Block::new()
            .title_bottom(footer_hints_left.left_aligned())
            .title_bottom(footer_hints_right.right_aligned())
            .borders(Borders::ALL)
            .border_style(Style::new().fg(theme.foreground))
            .border_type(BorderType::Rounded)
            .padding(Padding::new(1, 1, 1, 1));

        // Calculate the inner area (content area inside the window)
        let inner_area = window_block.inner(window_area);

        // Render the window block
        window_block.render(window_area, buf);

        // Return the inner area for content rendering
        inner_area
    }
}
