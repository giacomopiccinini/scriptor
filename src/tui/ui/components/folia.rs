use crate::tui::db::models::{Folio, NewFolio, UICodex};
use anyhow::Result;
use sqlx::SqlitePool;

pub struct FoliaComponent;

impl FoliaComponent {
    /// Select next folio in the list
    pub fn select_next_folio(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select_next();
    }

    /// Select previous folio in the list
    pub fn select_previous_folio(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select_previous();
    }

    /// Remove folio selection (deselect current folio)
    pub fn remove_folio_selection(ui_codex: &mut UICodex) {
        ui_codex.folio_state.select(None);
    }

    /// Select the first folio in the list
    pub fn select_first_item(ui_codex: &mut UICodex) {
        if ui_codex.folio_state.selected().is_none() {
            ui_codex.folio_state.select_first();
        }
    }

    /// Create a new folio in the given codex
    pub async fn create_item(
        ui_codex: &mut UICodex,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        let new_folio = NewFolio {
            name,
            codex_id: ui_codex.codex.id,
        };

        Folio::create(pool, new_folio).await?;
        ui_codex.update_folia(pool).await?;
        Ok(())
    }

    /// Update an existing folio
    pub async fn update_item(
        ui_codex: &mut UICodex,
        name: String,
        pool: &SqlitePool,
    ) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.update_name(pool, name).await?;

            // Update folia
            ui_codex.update_folia(pool).await?;
        }
        Ok(())
    }

    /// Move the currently selected folio up
    pub async fn move_selected_folio_up(ui_codex: &mut UICodex, pool: &SqlitePool) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.move_up(pool).await?;

            // Update folia to reflect the new order
            ui_codex.update_folia(pool).await?;

            // Adjust selection to follow the moved folio
            if j > 0 {
                ui_codex.folio_state.select(Some(j - 1));
            }
        }
        Ok(())
    }

    /// Move the currently selected folio down
    pub async fn move_selected_folio_down(ui_codex: &mut UICodex, pool: &SqlitePool) -> Result<()> {
        if let Some(j) = ui_codex.folio_state.selected() {
            let mut folio = ui_codex.folia[j].folio.clone();
            folio.move_down(pool).await?;

            // Update folia to reflect the new order
            ui_codex.update_folia(pool).await?;

            // Adjust selection to follow the moved folio
            if j + 1 < ui_codex.folia.len() {
                ui_codex.folio_state.select(Some(j + 1));
            }
        }
        Ok(())
    }
}
