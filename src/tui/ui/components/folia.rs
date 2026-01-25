use crate::tui::app::state::STTTools;
use crate::tui::db::models::{Folio, Fragmentum, NewFolio, NewFragmentum, UICodex};
use anyhow::Result;
use sqlx::SqlitePool;
use std::path::PathBuf;

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
        stt_tools: &mut STTTools,
        pool: &SqlitePool,
    ) -> Result<()> {
        // Convert the name into a path
        let folio_path = PathBuf::from(name);

        // Sanity checks on input
        if !folio_path.exists() {
            anyhow::bail!("Audio file does not exist")
        };
        if !folio_path.is_file() {
            anyhow::bail!("Folio is not a path to a file")
        };
        if folio_path.extension().and_then(|s| s.to_str()) != Some("wav") {
            anyhow::bail!("Folio is not a path to a wav file")
        };

        // Load audio (requires STT model to be available)
        let stt_model = stt_tools.stt_model.as_mut().ok_or_else(|| {
            anyhow::anyhow!("STT model not available - recording may be in progress")
        })?;
        let audio_samples = stt_model.load_audio(&folio_path)?;

        // Transcribe
        let transcription = stt_model.transcribe(audio_samples)?;

        // Split the transcription in chunks (fragmenta) of 500 chars with timestamps
        let fragmenta = transcription.split_with_timestamps(500);

        // The default folio name is just the name of the target file, not the full path
        let folio_name = folio_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .to_string();

        // Create new folio in DB
        let new_folio = NewFolio {
            codex_id: ui_codex.codex.id,
            name: folio_name,
        };

        let folio_db = Folio::create(pool, new_folio).await?;

        // Add all the fragmenta we have created
        let folio_id = folio_db.id;

        // Convert fragmenta into db-aware fragmenta and add to db in batch
        let new_fragmenta: Vec<NewFragmentum> = fragmenta
            .into_iter()
            .map(|f| NewFragmentum {
                folio_id,
                path: folio_path.display().to_string(),
                content: f.text,
                timestamp_start: Some(f.start),
                timestamp_end: Some(f.end),
            })
            .collect();
        Fragmentum::create_batch(pool, new_fragmenta).await?;

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
}
