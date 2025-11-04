use crate::tui::db::config::Config;
use crate::tui::ui::cursor::CursorState;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Widget, Wrap,
};
use std::str::FromStr;

pub struct AddCodexPopUp;
pub struct ModifyCodexPopUp;

fn render_codex_popup_kernel<T: CursorState>(
    state: &T,
    area: Rect,
    buf: &mut Buffer,
    popup_title: &str,
) {
    // Command hints for add/modify codex popup
    let add_or_modify_codex_command_hints = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "[Esc]",
            Style::default().fg(Color::from_str("#FFA69E").unwrap()),
        ),
        Span::raw(" "),
    ]);

    // Calculate popup dimensions
    let popup_width = (area.width * 3) / 4; // 75% of the area width
    let popup_height = 4; // Fixed height for just the input field

    // Center horizontally within the area
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;

    // Center vertically within the area
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    // Define the pop-up area
    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Clear the background of the popup area first
    Clear.render(popup_area, buf);
    Block::default()
        .style(Style::default().bg(Color::from_str("#002626").unwrap()))
        .render(popup_area, buf);

    // Define the popup block with styling
    let popup_block = Block::new()
        .padding(Padding::new(2, 2, 1, 1))
        .title(format!("  {}  ", popup_title))
        .title_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
        .title_bottom(add_or_modify_codex_command_hints)
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1));

    // Define the text to render
    let text_spans = state.create_cursor_text_spans();
    let text_line = Line::from(text_spans);

    // Render the input field
    Paragraph::new(text_line)
        .wrap(Wrap { trim: true })
        .block(popup_block)
        .render(popup_area, buf);
}

impl AddCodexPopUp {
    /// Render popup for entering a new codex name
    pub fn render<T: CursorState>(state: &T, area: Rect, buf: &mut Buffer) {
        render_codex_popup_kernel(state, area, buf, "Add Codex");
    }
}

impl ModifyCodexPopUp {
    /// Render popup for modifying a codex name
    pub fn render<T: CursorState>(state: &T, area: Rect, buf: &mut Buffer) {
        render_codex_popup_kernel(state, area, buf, "Modify Codex");
    }
}

pub struct AddFolioPopUp;
pub struct ModifyFolioPopUp;

/// Render popup for entering a new folio name
pub fn render_folio_popup_kernel<T: CursorState>(
    state: &T,
    area: Rect,
    buf: &mut Buffer,
    popup_title: &str,
) {
    // Command hints for add/modify folio popup
    let add_folio_command_hints = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "[Esc]",
            Style::default().fg(Color::from_str("#FFA69E").unwrap()),
        ),
        Span::raw(" "),
    ]);

    // Calculate popup dimensions
    let popup_width = (area.width * 3) / 4; // 75% of the area width
    let popup_height = 4; // Fixed height for just the input field

    // Center horizontally within the area
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;

    // Center vertically within the area
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    // Define the pop-up area
    let popup_area = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_width,
        height: popup_height,
    };

    // Clear the background of the popup area first
    Clear.render(popup_area, buf);
    Block::default()
        .style(Style::default().bg(Color::from_str("#002626").unwrap()))
        .render(popup_area, buf);

    // Define the popup block with styling
    let popup_block = Block::new()
        .padding(Padding::new(2, 2, 1, 1))
        .title(format!("  {}  ", popup_title))
        .title_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
        .title_bottom(add_folio_command_hints)
        .borders(Borders::ALL)
        .border_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1));

    // Define the text to render
    let text_spans = state.create_cursor_text_spans();
    let text_line = Line::from(text_spans);

    // Render the input field
    Paragraph::new(text_line)
        .wrap(Wrap { trim: true })
        .block(popup_block)
        .render(popup_area, buf);
}

impl AddFolioPopUp {
    /// Render popup for entering a new folio
    pub fn render<T: CursorState>(state: &T, area: Rect, buf: &mut Buffer) {
        render_folio_popup_kernel(state, area, buf, "Add Folio");
    }
}

impl ModifyFolioPopUp {
    /// Render popup for modifying folio name
    pub fn render<T: CursorState>(state: &T, area: Rect, buf: &mut Buffer) {
        render_folio_popup_kernel(state, area, buf, "Modify Folio");
    }
}

pub struct ChangeArchivumPopUp;

impl ChangeArchivumPopUp {
    /// Render popup for selecting archivum
    pub fn render(config: &Config, selected_index: usize, area: Rect, buf: &mut Buffer) {
        // Command hints for change archivum popup
        let change_archivum_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled(" ↑↓ ", Style::default()),
            Span::styled(
                "[A]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::styled(
                "dd",
                Style::default().fg(Color::from_str("#FCF1D5").unwrap()),
            ),
            Span::styled(
                " [S]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::styled(
                "et Default",
                Style::default().fg(Color::from_str("#FCF1D5").unwrap()),
            ),
            Span::styled(
                " [Esc]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::raw(" "),
        ]);

        Block::default()
            .style(
                Style::default()
                    .bg(Color::from_str("#002626").unwrap())
                    .fg(Color::from_str("#FCF1D5").unwrap()),
            )
            .render(area, buf);

        // Define the popup block with styling
        let popup_block = Block::new()
            .padding(Padding::new(2, 2, 1, 1))
            .title(" Select Archivum ")
            .title_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
            .title_bottom(change_archivum_command_hints)
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
            .border_type(BorderType::Rounded);

        // Create list items from archiva (databases)
        let items: Vec<ListItem> = config
            .dbs
            .iter()
            .map(|archivum| ListItem::from(archivum.name.clone()))
            .collect();

        // Create a mutable list state for rendering
        let mut temp_list_state = ratatui::widgets::ListState::default();
        temp_list_state.select(Some(selected_index));

        // Render the archivum list
        let list = List::new(items)
            .block(popup_block)
            .highlight_symbol(" ▸ ") // Selection indicator
            .highlight_style(
                // Swap foreground and background for selected item
                Style::default()
                    .bg(Color::from_str("#FCF1D5").unwrap())
                    .fg(Color::from_str("#002626").unwrap()),
            )
            .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);

        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut temp_list_state);
    }
}

pub struct AddArchivumPopUp;

impl AddArchivumPopUp {
    /// Render popup for entering a new archivum name
    pub fn render<T: CursorState>(state: &T, area: Rect, buf: &mut Buffer) {
        // Command hints for add archivum popup
        let add_archivum_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "[Esc]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::raw(" "),
        ]);

        // Clear the entire area background first
        Clear.render(area, buf);
        Block::default()
            .style(
                Style::default()
                    .bg(Color::from_str("#002626").unwrap())
                    .fg(Color::from_str("#FCF1D5").unwrap()),
            )
            .render(area, buf);

        // Define the popup block with styling - use full width
        let popup_block = Block::new()
            .padding(Padding::new(2, 2, 1, 1))
            .title(" Add Archivum ")
            .title_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
            .title_bottom(add_archivum_command_hints)
            .borders(Borders::ALL)
            .border_style(Style::new().fg(Color::from_str("#FCF1D5").unwrap()))
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1));

        // Define the text to render
        let text_spans = state.create_cursor_text_spans();
        let text_line = Line::from(text_spans);

        // Render the input field using the full area
        Paragraph::new(text_line)
            .wrap(Wrap { trim: true })
            .block(popup_block)
            .render(area, buf);
    }
}
