use crate::configs::theme::ThemeConfig;
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

    /// Returns the currently selected codex and folio (if a folio is selected).
    ///
    /// # Returns
    /// - `(Some(codex), Some(folio))` - A folio within a codex is selected
    /// - `(Some(codex), None)` - A codex header is selected (no folio)
    /// - `(None, None)` - Nothing is selected or invalid index
    pub fn get_selected_codex_and_folio(&self) -> (Option<&UICodex>, Option<&UIFolio>) {
        self.codex_state
            .selected()
            .and_then(|codex_idx| {
                self.codices.get(codex_idx).map(|codex| {
                    codex
                        .folio_state
                        .selected()
                        .map(|folio_idx| (Some(codex), codex.folia.get(folio_idx)))
                        .unwrap_or((Some(codex), None))
                })
            })
            .unwrap_or((None, None))
    }

    pub fn check_codex_folio_selection(&self) -> (bool, bool) {
        let Some(codex_idx) = self.codex_state.selected() else {
            return (false, false);
        };

        let Some(codex) = self.codices.get(codex_idx) else {
            return (false, false);
        };

        let folio_selected = codex
            .folio_state
            .selected()
            .and_then(|i| codex.folia.get(i))
            .is_some();

        (true, folio_selected)
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
                        } else if has_next_codex {
                            self.list_state.select_next();
                            self.codex_state.select_next();
                            selected_codex.folio_state.select(None);
                        }
                    } else if selected_folio_idx.unwrap() < n_folia - 1 {
                        self.list_state.select_next();
                        selected_codex.folio_state.select_next();
                    } else if has_next_codex {
                        self.list_state.select_next();
                        self.codex_state.select_next();
                        selected_codex.folio_state.select(None);
                    }
                } else if has_next_codex {
                    self.list_state.select_next();
                    self.codex_state.select_next();
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
                    } else if has_previous_codex {
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
                } else if has_previous_codex {
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

    /// Toggle expand/collapse for the currently selected codex
    pub fn toggle_selected_codex_expansion(&mut self) {
        if let Some(selected_codex_idx) = self.codex_state.selected()
            && let Some(codex) = self.codices.get_mut(selected_codex_idx)
        {
            if codex.is_expanded
                && let Some(selected_folio_idx) = codex.folio_state.selected()
            {
                self.list_state.scroll_up_by(selected_folio_idx as u16 + 1);
                codex.folio_state.select(None);
            }
            codex.is_expanded = !codex.is_expanded;
        }
    }

    /// Move the currently selected codex up
    pub async fn move_selected_codex_up(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected()
            && i > 0
        {
            let mut codex_below = codices_component.codices[i].codex.clone();

            // Extract the visual length of the codex above
            // visual length = 1 (codex) + # folia (if expanded)
            let visual_len_codex_above = 1
                + (codices_component.codices[i - 1].is_expanded as usize)
                    * codices_component.codices[i - 1].folia.len();

            codex_below.move_up(pool).await?;

            // Refresh codices to reflect the new order
            codices_component.codices.swap(i, i - 1);

            // Adjust selection to follow the moved codex
            codices_component.codex_state.select(Some(i - 1));
            codices_component
                .list_state
                .scroll_up_by(visual_len_codex_above as u16);
        }
        Ok(())
    }

    /// Move the currently selected codex down
    pub async fn move_selected_codex_down(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(i) = codices_component.codex_state.selected()
            && i < codices_component.codices.len() - 1
        {
            let mut codex_above = codices_component.codices[i].codex.clone();

            // Extract the visual length of the codex below
            // visual length = 1 (codex) + # folia (if expanded)
            let visual_len_codex_below = 1
                + (codices_component.codices[i + 1].is_expanded as usize)
                    * codices_component.codices[i + 1].folia.len();

            codex_above.move_down(pool).await?;

            // Refresh codices to reflect the new order
            codices_component.codices.swap(i, i + 1);

            // Adjust selection to follow the moved codex
            codices_component.codex_state.select(Some(i + 1));
            codices_component
                .list_state
                .scroll_down_by(visual_len_codex_below as u16);
        }
        Ok(())
    }

    /// Move the currently selected folio up
    pub async fn move_selected_folio_up(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(codex_idx) = codices_component.codex_state.selected()
            && let Some(folio_idx) = codices_component.codices[codex_idx].folio_state.selected()
            && folio_idx > 0
        {
            let mut folio = codices_component.codices[codex_idx].folia[folio_idx]
                .folio
                .clone();
            folio.move_up(pool).await?;

            // Swap folia in the Vec (no DB refresh)
            codices_component.codices[codex_idx]
                .folia
                .swap(folio_idx, folio_idx - 1);

            // Adjust both folio_state and list_state
            codices_component.codices[codex_idx]
                .folio_state
                .select(Some(folio_idx - 1));
            codices_component.list_state.scroll_up_by(1);
        }
        Ok(())
    }

    /// Move the currently selected folio down
    pub async fn move_selected_folio_down(
        codices_component: &mut CodicesComponent,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(codex_idx) = codices_component.codex_state.selected()
            && let Some(folio_idx) = codices_component.codices[codex_idx].folio_state.selected()
            && folio_idx < codices_component.codices[codex_idx].folia.len() - 1
        {
            let mut folio = codices_component.codices[codex_idx].folia[folio_idx]
                .folio
                .clone();
            folio.move_down(pool).await?;

            // Swap folia in the Vec (no DB refresh)
            codices_component.codices[codex_idx]
                .folia
                .swap(folio_idx, folio_idx + 1);

            // Adjust both folio_state and list_state
            codices_component.codices[codex_idx]
                .folio_state
                .select(Some(folio_idx + 1));
            codices_component.list_state.scroll_down_by(1);
        }
        Ok(())
    }

    /// Delete selected item, either codex or folio
    pub async fn delete_selected(&mut self, pool: &SqlitePool) -> Result<()> {
        // Extract the selected codex and folio (might be none)
        // Clone the data we need BEFORE mutating, to release the immutable borrow
        let (codex, folio) = self.get_selected_codex_and_folio();
        let codex_to_delete = codex.map(|c| c.codex.clone());
        let folio_to_delete = folio.map(|f| f.folio.clone());

        // A codex must be selected, or else neither a folio nor a codex are selected
        if codex_to_delete.is_some() {
            // There has to be a valid codex idx, we can unwrap
            let codex_idx = self.codex_state.selected().unwrap();

            // If also a folio is selected
            if let Some(folio) = folio_to_delete {
                // There has to be a valid folio idx, we can unwrap
                let folio_idx = self.codices[codex_idx].folio_state.selected().unwrap();

                // Remove from list and database
                self.select_previous();
                self.codices[codex_idx].folia.remove(folio_idx);
                folio.delete(pool).await?;
            } else if let Some(codex) = codex_to_delete {
                self.select_previous();
                self.codices.remove(codex_idx);
                codex.delete(pool).await?;
            }

            // self.select_previous();
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
        let db_codex = Codex::create(pool, new_codex).await?;
        let ui_codex = UICodex {
            codex: db_codex,
            folio_state: ListState::default(),
            folia: Vec::new(),
            is_expanded: false,
        };
        codices_component.codices.push(ui_codex);
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
            codices_component.codices[i].codex = codex;
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
            let n_blanks = area_width// Width of the allocated space, i.e. the max
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
            Span::styled("ew", Style::default().fg(theme.dark_shadow)),
            Span::raw("   "),
            Span::styled("[↵]", Style::default().fg(theme.highlight)),
            Span::styled("expand", Style::default().fg(theme.dark_shadow)),
            Span::raw("   "),
            Span::styled("[s]", Style::default().fg(theme.highlight)),
            Span::styled("ettings", Style::default().fg(theme.dark_shadow)),
            Span::raw("   "),
            Span::styled("[q]", Style::default().fg(theme.highlight)),
            Span::styled("uit", Style::default().fg(theme.dark_shadow)),
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
            .style(Style::default().fg(theme.medium_shadow))
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL)
            .highlight_style(
                // Swap foreground and background for selected item
                Style::default().bg(theme.dark_shadow).fg(theme.page),
            )
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.list_state);
    }
}
