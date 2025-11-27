use crate::tui::db::config::ThemeConfig;
use crate::tui::db::models::{Codex, NewCodex, UICodex, UIFolio};
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
const CODEX_COMMANDS_INLINE: &str = "[r]ec  [a]dd  [m]od  [d]el  ";
const FOLIO_COMMANDS_INLINE: &str = "[m]od  [d]el  ";

/// Component for managing and displaying codices (projects)
pub struct CodicesComponent {
    pub codices: Vec<UICodex>,
    pub list_state: ListState,
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
            list_state: ListState::default(),
            codex_state: ListState::default(),
        }
    }

    /// Initialize codices from archivum (database)
    pub async fn load_codices(&mut self, pool: &SqlitePool) -> Result<()> {
        self.codices = UICodex::get_all(pool).await?;
        Ok(())
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

    /// Get the visual index (display position) for a given codex index
    ///
    /// This function converts a logical codex index (position in the codices vector)
    /// to a visual index (position in the displayed list, accounting for expanded folia).
    ///
    /// # Arguments
    /// * `codex_idx` - The logical index of the codex in the codices vector
    ///
    /// # Returns
    /// The visual index where this codex appears in the displayed list
    ///
    /// # Example
    /// If we have 3 codices where the first is expanded with 2 folia:
    /// - Codex 0 (expanded): visual index 0
    /// - Folio 0.1: visual index 1  
    /// - Folio 0.2: visual index 2
    /// - Codex 1: visual index 3
    /// - Codex 2: visual index 4
    ///
    /// So `get_visual_index_for_codex(1)` would return 3.
    pub fn get_visual_index_for_codex(&self, codex_idx: usize) -> usize {
        let mut visual_idx = 0;

        // Iterate through all codices up to the target index
        for (idx, codex) in self.codices.iter().enumerate() {
            // If we've reached the target codex, return its visual position
            if idx == codex_idx {
                return visual_idx;
            }

            // Count this codex in the visual index
            visual_idx += 1;

            // If this codex is expanded, also count all its folia
            if codex.is_expanded {
                visual_idx += codex.folia.len();
            }
        }

        // If codex_idx is out of bounds, return the next visual position
        visual_idx
    }

    /// Get codex index and optional folio index from visual list index
    /// Returns (codex_idx, Some(folio_idx)) if on a folio, or (codex_idx, None) if on a codex
    pub fn get_codex_and_folio_index_at_visual_index(
        &self,
        visual_idx: usize,
    ) -> (usize, Option<usize>) {
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

    /// Returns the currently selected codex and folio (if a folio is selected).
    ///
    /// This method translates the visual selection index (which accounts for expanded/collapsed
    /// codices) into actual codex and folio references.
    ///
    /// # Returns
    /// - `(Some(codex), Some(folio))` - A folio within a codex is selected
    /// - `(Some(codex), None)` - A codex header is selected (no folio)
    /// - `(None, None)` - Nothing is selected or invalid index
    pub fn get_selected_codex_and_folio(&self) -> (Option<&UICodex>, Option<&UIFolio>) {
        self.codex_state
            .selected()
            .and_then(|visual_idx| {
                // Convert visual index to actual codex and folio indices
                let (codex_idx, folio_idx) =
                    self.get_codex_and_folio_index_at_visual_index(visual_idx);

                // Get the codex, returning None if index is invalid
                self.codices.get(codex_idx).map(|codex| {
                    // Get the folio if a folio index is provided
                    let folio = folio_idx.and_then(|idx| codex.folia.get(idx));
                    (Some(codex), folio)
                })
            })
            .unwrap_or((None, None))
    }

    ///  Scroll down in the component, moving between codices and folia
    pub fn select_next(&mut self) {
        if let Some(selected_codex_idx) = self.codex_state.selected() {
            let has_next_codex = selected_codex_idx < self.codices.len() - 1;
            if let Some(selected_codex) = self.codices.get_mut(selected_codex_idx) {
                if selected_codex.is_expanded {
                    let selected_folio_idx = selected_codex.folio_state.selected_mut();
                    let n_folia = selected_codex.folia.len();

                    if selected_folio_idx.is_none() {
                        if n_folia > 0 {
                            self.list_state.select_next();
                            selected_codex.folio_state.select_first();
                        } else {
                            if has_next_codex {
                                self.list_state.select_next();
                                self.codex_state.select_next();
                                selected_codex.folio_state.select(None);
                            }
                        }
                    } else {
                        if selected_folio_idx.unwrap() < n_folia - 1 {
                            self.list_state.select_next();
                            selected_codex.folio_state.select_next();
                        } else {
                            if has_next_codex {
                                self.list_state.select_next();
                                self.codex_state.select_next();
                                selected_codex.folio_state.select(None);
                            }
                        }
                    }
                } else {
                    if has_next_codex {
                        self.list_state.select_next();
                        self.codex_state.select_next();
                    }
                }
            }
        } else {
            self.list_state.select_first();
            self.codex_state.select_first();
        }
    }

    ///  Scroll up in the component, moving between codices and folia
    pub fn select_previous(&mut self) {
        if let Some(selected_codex_idx) = self.codex_state.selected() {
            let has_previous_codex = selected_codex_idx > 0;

            if let Some(selected_codex) = self.codices.get_mut(selected_codex_idx) {
                if selected_codex.is_expanded {
                    if let Some(selected_folio_idx) = selected_codex.folio_state.selected_mut() {
                        if *selected_folio_idx > 0 {
                            self.list_state.select_previous();
                            selected_codex.folio_state.select_previous();
                        } else {
                            self.list_state.select_previous();
                            selected_codex.folio_state.select(None);
                        }
                    } else {
                        if has_previous_codex {
                            self.list_state.select_previous();
                            self.codex_state.select_previous();
                            if let Some(previous_codex) =
                                self.codices.get_mut(selected_codex_idx - 1)
                            {
                                if previous_codex.is_expanded {
                                    previous_codex
                                        .folio_state
                                        .select(previous_codex.folia.len().checked_sub(1));
                                } else {
                                    previous_codex.folio_state.select(None);
                                }
                            }
                        }
                    }
                } else {
                    if has_previous_codex {
                        self.list_state.select_previous();
                        self.codex_state.select_previous();
                        if let Some(previous_codex) = self.codices.get_mut(selected_codex_idx - 1) {
                            if previous_codex.is_expanded {
                                previous_codex
                                    .folio_state
                                    .select(previous_codex.folia.len().checked_sub(1));
                            } else {
                                previous_codex.folio_state.select(None);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Toggle expand/collapse for the currently selected codex
    pub fn toggle_selected_codex_expansion(&mut self) {
        if let Some(selected_codex_idx) = self.codex_state.selected() {
            if let Some(codex) = self.codices.get_mut(selected_codex_idx) {
                if codex.is_expanded {
                    if let Some(selected_folio_idx) = codex.folio_state.selected() {
                        self.list_state.scroll_up_by(selected_folio_idx as u16 + 1);
                        codex.folio_state.select(None);
                    }
                }
                codex.is_expanded = !codex.is_expanded;
            }
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

    /// Builds the rendered string for a codex row, optionally appending inline commands
    /// when the row represents the currently selected codex.
    ///
    /// * `codex` – codex being rendered
    /// * `is_selected` – whether this codex is the active selection in the list
    /// * `area_width` – width of the list area used to compute padding/truncation
    fn format_codex(&self, codex: &UICodex, is_selected: bool, area_width: i16) -> String {
        // Show codex with medieval expand/collapse indicator
        let indicator = if codex.is_expanded {
            OPEN_CODEX_SYMBOL
        } else {
            CLOSED_CODEX_SYMBOL
        };
        let mut codex_text = format!("{} {}", indicator, codex.codex.name);

        if is_selected {
            // Compute number of blanks spaces to leave after the codex name
            let n_blanks = area_width// Width of the allocated space, i.e. the max
            - codex_text.chars().count()as i16 // Characters occupied by the codex name
            - CODEX_COMMANDS_INLINE.chars().count()as i16 // Characters occupied the commands
            - LIST_HIGHLIGHT_SYMBOL.chars().count()as i16; // Characters occupied by the highlight of the list

            if n_blanks <= 0 {
                // Not enough space - truncate the codex name
                // 3 chars for ...
                // 2 chars for blank spaces to improve readibility
                let n_chars_to_keep = codex_text.chars().count() as i16 + n_blanks - 3 - 2;
                codex_text = codex_text.chars().take(n_chars_to_keep as usize).collect();
                codex_text = format!("{}...  {}", codex_text, CODEX_COMMANDS_INLINE);
            } else {
                // Add commands to selected codex
                codex_text = format!(
                    "{}{}{}",
                    codex_text,
                    " ".repeat(n_blanks as usize),
                    CODEX_COMMANDS_INLINE
                );
            }
        }

        codex_text
    }

    /// Builds the rendered string for a folio row, optionally appending inline commands
    /// when the row represents the currently selected folio.
    ///
    /// * `folio` – codex being rendered
    /// * `is_selected` – whether this folio is the active selection in the list
    /// * `area_width` – width of the list area used to compute padding/truncation
    fn format_folio(&self, folio: &UIFolio, is_selected: bool, area_width: i16) -> String {
        let mut folio_text = format!("    {}", folio.folio.name);

        if is_selected {
            // Compute number of blanks spaces to leave after the folio name
            let n_blanks = area_width as i16// Width of the allocated space, i.e. the max
            - folio_text.chars().count()as i16 // Characters occupied by the folio name
            - FOLIO_COMMANDS_INLINE.chars().count()as i16 // Characters occupied the commands
            - LIST_HIGHLIGHT_SYMBOL.chars().count()as i16; // Characters occupied by the highlight of the list

            if n_blanks <= 0 {
                // Not enough space - truncate the folio name
                // 3 chars for ...
                // 2 chars for blank spaces to improve readibility
                let n_chars_to_keep = folio_text.chars().count() as i16 + n_blanks - 3 - 2;
                folio_text = folio_text.chars().take(n_chars_to_keep as usize).collect();
                folio_text = format!("{}...  {}", folio_text, FOLIO_COMMANDS_INLINE);
            } else {
                // Add commands to selected folio
                folio_text = format!(
                    "{}{}{}",
                    folio_text,
                    " ".repeat(n_blanks as usize),
                    FOLIO_COMMANDS_INLINE
                );
            }
        }

        folio_text
    }

    /// Render the hierarchical tree of codices with collapsible folia
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &ThemeConfig) {
        // Command hints for codices
        let codex_command_hints = Line::from(vec![
            Span::styled("[n]", Style::default().fg(theme.highlight)),
            Span::styled("ew", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[↵]", Style::default().fg(theme.highlight)),
            Span::styled("expand", Style::default().fg(theme.foreground)),
            Span::raw("   "),
            Span::styled("[q]", Style::default().fg(theme.highlight)),
            Span::styled("uit", Style::default().fg(theme.foreground)),
        ])
        .centered();

        let block = Block::default().title_bottom(codex_command_hints);

        // Build hierarchical tree view.
        // We initiate a list that is composed of both codices and folia, in a nested way
        let mut codices_and_folia: Vec<ListItem> = Vec::new();

        // Find the index of the selected codex
        let selected_codex_idx = self.codex_state.selected();

        // Add all codices and possibly folia to the list to be displayed in the UI
        for (codex_idx, ui_codex) in self.codices.iter().enumerate() {
            // Format the codex text to be displayed, possibly with inline command hints
            let codex_text = self.format_codex(
                ui_codex,
                selected_codex_idx
                    .map(|idx| idx == codex_idx)
                    .unwrap_or(false)
                    && ui_codex.folio_state.selected().is_none(),
                area.width as i16,
            );

            // Always add the formatted codex text to the list
            codices_and_folia.push(ListItem::from(codex_text));

            // If the codex is expanded, we also need to add the folia to the list
            if ui_codex.is_expanded {
                // Find the index of the selected folio
                let selected_folio_idx = ui_codex.folio_state.selected();

                for (folio_idx, ui_folio) in ui_codex.folia.iter().enumerate() {
                    // Format the folio text to be displayed, possibly with inline command hints
                    let folio_text = self.format_folio(
                        ui_folio,
                        selected_folio_idx
                            .map(|idx| idx == folio_idx)
                            .unwrap_or(false),
                        area.width as i16,
                    );

                    // Always add the formatted codex text to the list
                    codices_and_folia.push(ListItem::from(folio_text));
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

        StatefulWidget::render(list, area, buf, &mut self.list_state);
    }
}
