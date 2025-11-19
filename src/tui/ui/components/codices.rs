use crate::tui::db::config::ThemeConfig;
use crate::tui::db::models::{Codex, NewCodex, UICodex};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, ListState, StatefulWidget};
use sqlx::SqlitePool;

// Blank spaces to use to pad the list to declutter view
const LIST_HIGHLIGHT_SYMBOL: &str = "  ";
const OPEN_CODEX_SYMBOL: &str = "❖";
const CLOSED_CODEX_SYMBOL: &str = "◆";

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

    /// Get codex index and optional folio index from visual list index
    /// Returns (codex_idx, Some(folio_idx)) if on a folio, or (codex_idx, None) if on a codex
    fn get_codex_and_folio_at_visual_index(&self, visual_idx: usize) -> (usize, Option<usize>) {
        let mut current_visual_idx = 0;

        for (codex_idx, codex) in self.codices.iter().enumerate() {
            if current_visual_idx == visual_idx {
                // We're on the codex line
                return (codex_idx, None);
            }
            current_visual_idx += 1;

            if codex.is_expanded {
                for folio_idx in 0..codex.folia.len() {
                    if current_visual_idx == visual_idx {
                        // We're on a folio line
                        return (codex_idx, Some(folio_idx));
                    }
                    current_visual_idx += 1;
                }
            }
        }

        // Default: return last codex
        (self.codices.len().saturating_sub(1), None)
    }

    /// Toggle expand/collapse for the currently selected codex
    pub fn toggle_selected_codex_expansion(&mut self) {
        if let Some(selected_idx) = self.codex_state.selected() {
            // Find which codex we're on
            let (codex_idx, _) = self.get_codex_and_folio_at_visual_index(selected_idx);

            if let Some(codex) = self.codices.get_mut(codex_idx) {
                codex.is_expanded = !codex.is_expanded;

                // If collapsing, ensure selection stays on the codex line
                if !codex.is_expanded {
                    let visual_idx = self.get_visual_index_for_codex(codex_idx);
                    self.codex_state.select(Some(visual_idx));
                }
            }
        }
    }

    /// Get visual index for a given codex index
    fn get_visual_index_for_codex(&self, codex_idx: usize) -> usize {
        let mut visual_idx = 0;

        for (idx, codex) in self.codices.iter().enumerate() {
            if idx == codex_idx {
                return visual_idx;
            }
            visual_idx += 1;
            if codex.is_expanded {
                visual_idx += codex.folia.len();
            }
        }

        visual_idx
    }

    /// Smart navigation: handle boundary overflow (auto-expand next/previous codex)
    pub fn handle_smart_navigation_down(&mut self) -> bool {
        if let Some(selected_idx) = self.codex_state.selected() {
            let (codex_idx, folio_idx_opt) = self.get_codex_and_folio_at_visual_index(selected_idx);

            // If we're on a folio and it's the last one in an expanded codex
            if let Some(folio_idx) = folio_idx_opt
                && let Some(codex) = self.codices.get(codex_idx)
                && codex.is_expanded
                && folio_idx == codex.folia.len() - 1
            {
                // Try to open next codex
                if codex_idx + 1 < self.codices.len()
                    && let Some(next_codex) = self.codices.get_mut(codex_idx + 1)
                {
                    next_codex.is_expanded = true;
                    // Move to the next codex line
                    self.select_next();
                    return true;
                }
            }
        }
        false
    }

    /// Smart navigation: handle boundary overflow upward (auto-expand previous codex)
    pub fn handle_smart_navigation_up(&mut self) -> bool {
        if let Some(selected_idx) = self.codex_state.selected() {
            let (codex_idx, folio_idx_opt) = self.get_codex_and_folio_at_visual_index(selected_idx);

            // If we're on a folio and it's the first one in an expanded codex
            if let Some(folio_idx) = folio_idx_opt
                && folio_idx == 0
            {
                // Try to open previous codex
                if codex_idx > 0
                    && let Some(prev_codex) = self.codices.get_mut(codex_idx - 1)
                {
                    prev_codex.is_expanded = true;
                    // Move to the previous codex line
                    self.select_previous();
                    return true;
                }
            }
        }
        false
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

    /// Render the hierarchical tree of codices with collapsible folia
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        // Command hints for codices
        let codex_command_hints = Line::from(vec![
            Span::styled("[A]", Style::default().fg(theme.highlight)),
            Span::styled("dd", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[D]", Style::default().fg(theme.highlight)),
            Span::styled("elete", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[E]", Style::default().fg(theme.highlight)),
            Span::styled("xpand", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[M]", Style::default().fg(theme.highlight)),
            Span::styled("odify", Style::default().fg(theme.foreground)),
        ])
        .centered();

        let block = Block::default().title_bottom(codex_command_hints);

        // Build hierarchical tree view.
        // We initiate a list that is composed of both codices and folia, in a nested way
        let mut codices_and_folia: Vec<ListItem> = Vec::new();

        // Commands
        let codex_commands = "[M]odify  [D]elete  ".to_string();

        for (codex_idx, ui_codex) in self.codices.iter().enumerate() {
            // Show codex with medieval expand/collapse indicator
            let indicator = if ui_codex.is_expanded {
                OPEN_CODEX_SYMBOL
            } else {
                CLOSED_CODEX_SYMBOL
            };

            // Default text, irrespective of whether the codex is selected or not
            let mut codex_text = format!("{} {}", indicator, ui_codex.codex.name);

            // If Codex is selected, add relevant commands *inline*
            if self.selected() == Some(codex_idx) {
                // Compute number of blanks spaces to leave after the codex name
                let n_blanks = area.width // Width of the allocated space, i.e. the max
                - codex_text.chars().count()as u16 // Characters occupied by the codex name
                - codex_commands.chars().count()as u16 // Characters occupied the commands
                - LIST_HIGHLIGHT_SYMBOL.chars().count()as u16; // Characters occupied by the highlight of the list

                // Add commands to selected codex
                codex_text = format!(
                    "{}{}{}",
                    codex_text,
                    " ".repeat(n_blanks as usize),
                    codex_commands
                );
            }
            codices_and_folia.push(ListItem::from(codex_text));

            // Show folia if expanded (no symbol, just indent)
            if ui_codex.is_expanded {
                for ui_folio in &ui_codex.folia {
                    codices_and_folia.push(ListItem::from(format!("    {}", ui_folio.folio.name)));
                }
            }
        }

        let list: List = List::new(codices_and_folia)
            .block(block)
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL) // No symbol, just space for padding
            .highlight_style(
                // Swap foreground and background for selected item
                Style::default().bg(theme.foreground).fg(theme.background),
            )
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.codex_state);
    }
}
