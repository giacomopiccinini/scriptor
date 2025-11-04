pub struct ArchivumSelector;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget};
use std::str::FromStr;

impl ArchivumSelector {
    pub fn render(area: Rect, buf: &mut Buffer, current_archivum_name: &str) {
        // Command hints for archivum
        let archivum_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "[Tab]",
                Style::default().fg(Color::from_str("#FFA69E").unwrap()),
            ),
            Span::styled(
                " Change",
                Style::default().fg(Color::from_str("#FCF1D5").unwrap()),
            ),
            Span::raw(" "),
        ])
        .left_aligned();

        let block = Block::default()
            .padding(Padding::new(2, 2, 0, 0))
            .title_top(Line::raw("  A R C H I V U M  ").left_aligned())
            .title_bottom(archivum_command_hints)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        Paragraph::new(current_archivum_name)
            .left_aligned()
            .block(block)
            .render(area, buf);
    }
}
