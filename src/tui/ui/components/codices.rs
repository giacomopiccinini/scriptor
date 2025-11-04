use crate::tui::db::config::ThemeConfig;
use crate::tui::db::models::{Codex, NewCodex, UICodex};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, HighlightSpacing, List, ListItem, ListState, Padding,
    StatefulWidget,
};
use sqlx::SqlitePool;

/// Component for managing and displaying codices (projects)
pub struct CodicesComponent {
    pub codices: Vec<UICodex>,
    pub codex_state: ListState,
}

impl Default for CodicesComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl CodicesComponent {
    pub fn new() -> Self {
        Self {
            codices: Vec::new(),
            codex_state: ListState::default(),
        }
    }

    /// Initialize codices from archivum (database)
    pub async fn load_codices(&mut self, pool: &SqlitePool) -> Result<()> {
        self.codices = UICodex::get_all(pool).await?;
        Ok(())
    }

    /// Select next codex in the list
    pub fn select_next(&mut self) {
        self.codex_state.select_next();
    }

    /// Select previous codex in the list
    pub fn select_previous(&mut self) {
        self.codex_state.select_previous();
    }

    /// Get currently selected codex index
    pub fn selected(&self) -> Option<usize> {
        self.codex_state.selected()
    }

    /// Get the currently selected codex (mutable)
    pub fn get_selected_codex_mut(&mut self) -> Option<&mut UICodex> {
        if let Some(i) = self.codex_state.selected() {
            self.codices.get_mut(i)
        } else {
            None
        }
    }

    /// Get the currently selected codex (immutable)
    pub fn get_selected_codex(&self) -> Option<&UICodex> {
        if let Some(i) = self.codex_state.selected() {
            self.codices.get(i)
        } else {
            None
        }
    }

    /// Refresh codices from archivum (used after reordering)
    pub async fn refresh_codices(&mut self, pool: &SqlitePool) -> Result<()> {
        let selected_index = self.codex_state.selected();
        self.load_codices(pool).await?;

        // Restore selection if it was set and still valid
        if let Some(index) = selected_index {
            if index < self.codices.len() {
                self.codex_state.select(Some(index));
            } else if !self.codices.is_empty() {
                self.codex_state.select(Some(self.codices.len() - 1));
            }
        }

        Ok(())
    }

    /// Move the currently selected codex up
    pub async fn move_selected_codex_up(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected() {
            let mut codex = codices_component.codices[i].codex.clone();
            codex.move_up(pool).await?;

            // Refresh codices to reflect the new order
            codices_component.refresh_codices(pool).await?;

            // Adjust selection to follow the moved codex
            if i > 0 {
                codices_component.codex_state.select(Some(i - 1));
            }
        }
        Ok(())
    }

    /// Move the currently selected codex down
    pub async fn move_selected_codex_down(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected() {
            let mut codex = codices_component.codices[i].codex.clone();
            codex.move_down(pool).await?;

            // Refresh codices to reflect the new order
            codices_component.refresh_codices(pool).await?;

            // Adjust selection to follow the moved codex
            if i + 1 < codices_component.codices.len() {
                codices_component.codex_state.select(Some(i + 1));
            }
        }
        Ok(())
    }

    /// Delete the currently selected codex
    pub async fn delete_selected_codex_static(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected() {
            let codex = codices_component.codices[i].codex.clone();
            codex.delete(pool).await?;

            // Refresh the codices from archivum
            codices_component.load_codices(pool).await?;

            // Adjust selection after deletion
            if codices_component.codices.is_empty() {
                codices_component.codex_state.select(None);
            } else if i >= codices_component.codices.len() {
                codices_component
                    .codex_state
                    .select(Some(codices_component.codices.len() - 1));
            }
        }
        Ok(())
    }

    /// Create a new codex and refresh data
    pub async fn create_codex(
        codices_component: &mut CodicesComponent,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        let new_codex = NewCodex { name };
        Codex::create(pool, new_codex).await?;
        codices_component.load_codices(pool).await?;
        Ok(())
    }

    /// Update an existing codex
    pub async fn update_codex(
        codices_component: &mut CodicesComponent,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected() {
            let mut codex = codices_component.codices[i].codex.clone();
            codex.update_name(pool, name).await?;
            codices_component.load_codices(pool).await?;
        }
        Ok(())
    }

    /// Render the list of codices
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        // Command hints for codices
        let codex_command_hints = Line::from(vec![
            Span::raw(" "),
            Span::styled("[A]", Style::default().fg(theme.highlight)),
            Span::styled("dd", Style::default().fg(theme.foreground)),
            Span::styled(" [D]", Style::default().fg(theme.highlight)),
            Span::styled("el", Style::default().fg(theme.foreground)),
            Span::styled(" [M]", Style::default().fg(theme.highlight)),
            Span::styled("odify ", Style::default().fg(theme.foreground)),
            Span::raw(" "),
        ])
        .left_aligned();

        let block = Block::default()
            .padding(Padding::new(2, 2, 1, 1))
            .title_top(Line::raw("  C O D E X  ").left_aligned())
            .title_bottom(codex_command_hints)
            .title_alignment(Alignment::Center)
            .borders(Borders::TOP | Borders::LEFT | Borders::BOTTOM)
            .border_type(BorderType::Rounded);

        // Convert codices to display items
        let items: Vec<ListItem> = self
            .codices
            .iter()
            .map(|ui_codex| ListItem::from(ui_codex.codex.name.clone()))
            .collect();

        let list: List = List::new(items)
            .block(block)
            .highlight_symbol(" ▸ ") // Selection indicator
            .highlight_style(
                // Swap foreground and background for selected item
                Style::default().bg(theme.foreground).fg(theme.background),
            )
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.codex_state);
    }
}
