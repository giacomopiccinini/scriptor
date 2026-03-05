pub struct ArchivumSelector;
use crate::configs::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget};

impl ArchivumSelector {
    pub fn render(area: Rect, buf: &mut Buffer, current_archivum_name: &str, theme: &ThemeConfig) {
        // Command hints for archivum
        let archivum_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled("[Tab]", Style::default().fg(theme.highlight)),
            Span::styled(" Change", Style::default().fg(theme.dark_shadow)),
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
