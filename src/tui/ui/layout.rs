use crate::configs::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::Line;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, BorderType, Borders, Widget};

pub struct AppLayout;

impl AppLayout {
    /// Calculate all layout areas for the main book-style UI.
    ///
    /// Returns, in order:
    /// - `outer_area_0`: full terminal area (table background)
    /// - `outer_area_1..3`: progressively inset spine layers
    /// - `inner_area`: the inner page background
    /// - `page_l`: left page content area
    /// - `light_shadow_l/r`: thin light-shadow columns flanking the spine
    /// - `medium_shadow_l/r`: thin medium-shadow columns at the spine center
    /// - `right_page_area`: right page content area (excluding bookmark column)
    /// - `bookmark_area`: the visible bookmark tab (top ~60% of the bookmark column)
    /// - `empty_area`: the empty remainder below the bookmark tab
    pub fn calculate_main_layout(
        area: Rect,
    ) -> (
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
        Rect,
    ) {
        let table_margin = Margin {
            horizontal: 4,
            vertical: 2,
        };

        let book_margin = Margin {
            horizontal: 1,
            vertical: 0,
        };

        let outer_area_0 = area;
        let outer_area_1 = area.inner(table_margin);
        let outer_area_2 = outer_area_1.inner(book_margin);
        let outer_area_3 = outer_area_2.inner(book_margin);
        let inner_area = outer_area_3.inner(book_margin);

        // Split inner area horizontally into left page, spine separators, and right page
        let page_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Percentage(50),
            ]);

        let [
            page_l,
            light_shadow_l,
            medium_shadow_l,
            medium_shadow_r,
            light_shadow_r,
            page_r,
        ] = page_layout.areas(inner_area);

        // Split right page into a narrow bookmark column and the actual content area
        let page_bookmark_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Max(4), Constraint::Min(20)]);

        let [full_bookmark_area, right_page_area] = page_bookmark_layout.areas(page_r);

        // Split the bookmark column into the visible tab and the empty area below
        let bookmark_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)]);

        let [bookmark_area, empty_area] = bookmark_split.areas(full_bookmark_area);

        (
            outer_area_0,
            outer_area_1,
            outer_area_2,
            outer_area_3,
            inner_area,
            page_l,
            light_shadow_l,
            medium_shadow_l,
            medium_shadow_r,
            light_shadow_r,
            right_page_area,
            bookmark_area,
            empty_area,
        )
    }

    /// Fill an area with a solid background color (used for the outermost table layer).
    pub fn render_table(color: Color, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, Style::default().bg(color));
    }

    /// Render a spine layer with a background color and double-line left/right borders.
    pub fn render_spine(bg_color: Color, fg_color: Color, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(bg_color))
            .border_style(Style::default().fg(fg_color))
            .border_type(BorderType::Double)
            .borders(Borders::LEFT | Borders::RIGHT)
            .render(area, buf);
    }

    /// Fill an area with a solid block background color.
    pub fn render_color(color: Color, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(color))
            .render(area, buf);
    }

    /// Render a light-shadow spine separator column.
    pub fn render_light_shadow(bg_color: Color, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(bg_color))
            .render(area, buf);
    }

    /// Render a medium-shadow spine separator column with a thick left border line.
    pub fn render_medium_shadow(bg_color: Color, line_color: Color, area: Rect, buf: &mut Buffer) {
        Block::default()
            .style(Style::default().bg(bg_color))
            .border_style(Style::default().fg(line_color))
            .border_type(BorderType::Thick)
            .borders(Borders::LEFT)
            .merge_borders(MergeStrategy::Exact)
            .render(area, buf);
    }

    /// Render the inner page background.
    pub fn render_inner_page(area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        Block::default()
            .style(Style::default().bg(theme.page))
            .render(area, buf);
    }

    /// Render a centered page header with the highlight color on the page background.
    pub fn render_header(area: Rect, buf: &mut Buffer, title: &str, theme: &ThemeConfig) {
        let block = Block::default().padding(Padding::new(0, 0, 2, 0));
        let header_text = Line::from(title)
            .style(Style::default().fg(theme.highlight))
            .centered();
        Paragraph::new(header_text)
            .block(block)
            .style(Style::default().bg(theme.page))
            .render(area, buf);
    }

    /// Render the bookmark tab: highlight background with the archivum name written vertically.
    pub fn render_bookmark(area: Rect, buf: &mut Buffer, archivum_name: &str, theme: &ThemeConfig) {
        Block::default()
            .style(Style::default().bg(theme.highlight).fg(theme.page))
            .render(area, buf);

        let chars: Vec<Line> = archivum_name
            .chars()
            .map(|c| Line::from(c.to_string()).centered())
            .collect();

        Paragraph::new(chars)
            .style(Style::default().bg(theme.highlight).fg(theme.page))
            .render(area, buf);
    }
}
