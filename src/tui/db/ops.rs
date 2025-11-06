use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::SqlitePool;

use crate::tui::db::models::{
    Codex, Folio, Fragmentum, NewCodex, NewFolio, NewFragmentum, UICodex, UIFolio, UIFragmentum,
};
use ratatui::widgets::ListState;

// ============================================================================
// Codex Operations
// ============================================================================

impl Codex {
    /// Create a new codex (project)
    pub async fn create(pool: &SqlitePool, new_codex: NewCodex) -> Result<Codex> {
        let now = Utc::now();

        // Get the next ordering value (max + 1)
        let next_ordering: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(ordering), 0) + 1 FROM codex")
                .fetch_one(pool)
                .await
                .with_context(|| "Failed to get next ordering value for codex")?;

        let row = sqlx::query_as::<_, Codex>(
            r#"
            INSERT INTO codex (name, ordering, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            RETURNING id, name, ordering, created_at, updated_at
            "#,
        )
        .bind(&new_codex.name)
        .bind(next_ordering)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .with_context(|| "Failed to create codex")?;

        Ok(row)
    }

    /// Get all codices ordered by ordering
    pub async fn get_all(pool: &SqlitePool) -> Result<Vec<Codex>> {
        let codices = sqlx::query_as::<_, Codex>(
            "SELECT id, name, ordering, created_at, updated_at FROM codex ORDER BY ordering",
        )
        .fetch_all(pool)
        .await
        .with_context(|| "Failed to fetch all codices")?;

        Ok(codices)
    }

    /// Get a specific codex by ID
    pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Codex>> {
        let codex = sqlx::query_as::<_, Codex>(
            "SELECT id, name, ordering, created_at, updated_at FROM codex WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to fetch codex by id")?;

        Ok(codex)
    }

    /// Update codex name
    pub async fn update_name(&mut self, pool: &SqlitePool, new_name: String) -> Result<()> {
        let now = Utc::now();

        sqlx::query("UPDATE codex SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&new_name)
            .bind(now)
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to update codex name")?;

        self.name = new_name;
        self.updated_at = now;
        Ok(())
    }

    /// Delete codex (cascades to folia and fragmenta)
    pub async fn delete(self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM codex WHERE id = ?1")
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to delete codex")?;

        Ok(())
    }

    /// Move codex up (decrease ordering, swap with previous)
    pub async fn move_up(&mut self, pool: &SqlitePool) -> Result<()> {
        // Find the codex with the next lower ordering value
        let prev_codex: Option<(i64, i64)> = sqlx::query_as(
            "SELECT id, ordering FROM codex WHERE ordering < ?1 ORDER BY ordering DESC LIMIT 1",
        )
        .bind(self.ordering)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to find previous codex")?;

        if let Some((prev_id, prev_ordering)) = prev_codex {
            // Swap orderings
            sqlx::query("UPDATE codex SET ordering = ?1 WHERE id = ?2")
                .bind(self.ordering)
                .bind(prev_id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update previous codex ordering")?;

            sqlx::query("UPDATE codex SET ordering = ?1 WHERE id = ?2")
                .bind(prev_ordering)
                .bind(self.id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update current codex ordering")?;

            self.ordering = prev_ordering;
        }

        Ok(())
    }

    /// Move codex down (increase ordering, swap with next)
    pub async fn move_down(&mut self, pool: &SqlitePool) -> Result<()> {
        // Find the codex with the next higher ordering value
        let next_codex: Option<(i64, i64)> = sqlx::query_as(
            "SELECT id, ordering FROM codex WHERE ordering > ?1 ORDER BY ordering ASC LIMIT 1",
        )
        .bind(self.ordering)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to find next codex")?;

        if let Some((next_id, next_ordering)) = next_codex {
            // Swap orderings
            sqlx::query("UPDATE codex SET ordering = ?1 WHERE id = ?2")
                .bind(self.ordering)
                .bind(next_id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update next codex ordering")?;

            sqlx::query("UPDATE codex SET ordering = ?1 WHERE id = ?2")
                .bind(next_ordering)
                .bind(self.id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update current codex ordering")?;

            self.ordering = next_ordering;
        }

        Ok(())
    }
}

// ============================================================================
// Folio Operations
// ============================================================================

impl Folio {
    /// Create a new folio (recording)
    pub async fn create(pool: &SqlitePool, new_folio: NewFolio) -> Result<Folio> {
        let now = Utc::now();

        // Get the next ordering value for this codex (max + 1)
        let next_ordering: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(ordering), 0) + 1 FROM folio WHERE codex_id = ?1",
        )
        .bind(new_folio.codex_id)
        .fetch_one(pool)
        .await
        .with_context(|| "Failed to get next ordering value for folio")?;

        let row = sqlx::query_as::<_, Folio>(
            r#"
            INSERT INTO folio (codex_id, name, ordering, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, codex_id, name, ordering, created_at, updated_at
            "#,
        )
        .bind(new_folio.codex_id)
        .bind(&new_folio.name)
        .bind(next_ordering)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .with_context(|| "Failed to create folio")?;

        Ok(row)
    }

    /// Get all folia for a specific codex
    pub async fn get_by_codex_id(pool: &SqlitePool, codex_id: i64) -> Result<Vec<Folio>> {
        let folia = sqlx::query_as::<_, Folio>(
            r#"
            SELECT id, codex_id, name, ordering, created_at, updated_at
            FROM folio 
            WHERE codex_id = ?1 
            ORDER BY ordering
            "#,
        )
        .bind(codex_id)
        .fetch_all(pool)
        .await
        .with_context(|| "Failed to fetch folia")?;

        Ok(folia)
    }

    /// Get folio by specific id
    pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Folio>> {
        let folio = sqlx::query_as::<_, Folio>(
            r#"
            SELECT id, codex_id, name, ordering, created_at, updated_at
            FROM folio 
            WHERE id = ?1 
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to fetch folio")?;

        Ok(folio)
    }

    /// Update folio name
    pub async fn update_name(&mut self, pool: &SqlitePool, new_name: String) -> Result<()> {
        let now = Utc::now();

        sqlx::query("UPDATE folio SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&new_name)
            .bind(now)
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to update folio name")?;

        self.name = new_name;
        self.updated_at = now;

        Ok(())
    }

    /// Delete folio (cascades to fragmenta)
    pub async fn delete(self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM folio WHERE id = ?1")
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to delete folio")?;

        Ok(())
    }

    /// Move folio up (decrease ordering, swap with previous in same codex)
    pub async fn move_up(&mut self, pool: &SqlitePool) -> Result<()> {
        // Find the folio with the next lower ordering value in the same codex
        let prev_folio: Option<(i64, i64)> = sqlx::query_as(
            "SELECT id, ordering FROM folio WHERE codex_id = ?1 AND ordering < ?2 ORDER BY ordering DESC LIMIT 1"
        )
        .bind(self.codex_id)
        .bind(self.ordering)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to find previous folio")?;

        if let Some((prev_id, prev_ordering)) = prev_folio {
            // Swap orderings
            sqlx::query("UPDATE folio SET ordering = ?1 WHERE id = ?2")
                .bind(self.ordering)
                .bind(prev_id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update previous folio ordering")?;

            sqlx::query("UPDATE folio SET ordering = ?1 WHERE id = ?2")
                .bind(prev_ordering)
                .bind(self.id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update current folio ordering")?;

            self.ordering = prev_ordering;
        }

        Ok(())
    }

    /// Move folio down (increase ordering, swap with next in same codex)
    pub async fn move_down(&mut self, pool: &SqlitePool) -> Result<()> {
        // Find the folio with the next higher ordering value in the same codex
        let next_folio: Option<(i64, i64)> = sqlx::query_as(
            "SELECT id, ordering FROM folio WHERE codex_id = ?1 AND ordering > ?2 ORDER BY ordering ASC LIMIT 1"
        )
        .bind(self.codex_id)
        .bind(self.ordering)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to find next folio")?;

        if let Some((next_id, next_ordering)) = next_folio {
            // Swap orderings
            sqlx::query("UPDATE folio SET ordering = ?1 WHERE id = ?2")
                .bind(self.ordering)
                .bind(next_id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update next folio ordering")?;

            sqlx::query("UPDATE folio SET ordering = ?1 WHERE id = ?2")
                .bind(next_ordering)
                .bind(self.id)
                .execute(pool)
                .await
                .with_context(|| "Failed to update current folio ordering")?;

            self.ordering = next_ordering;
        }

        Ok(())
    }
}

// ============================================================================
// Fragmentum Operations
// ============================================================================

impl Fragmentum {
    /// Create a new fragmentum (text/audio chunk)
    pub async fn create(pool: &SqlitePool, new_fragmentum: NewFragmentum) -> Result<Fragmentum> {
        let now = Utc::now();

        let row = sqlx::query_as::<_, Fragmentum>(
            r#"
            INSERT INTO fragmentum (folio_id, content, audio_path, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING id, folio_id, content, audio_path, created_at, updated_at
            "#,
        )
        .bind(new_fragmentum.folio_id)
        .bind(&new_fragmentum.content)
        .bind("") // Default empty audio_path, can be updated later
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .with_context(|| "Failed to create fragmentum")?;

        Ok(row)
    }

    /// Get all fragmenta for a specific folio
    pub async fn get_by_folio_id(pool: &SqlitePool, folio_id: i64) -> Result<Vec<Fragmentum>> {
        let fragmenta = sqlx::query_as::<_, Fragmentum>(
            r#"
            SELECT id, folio_id, content, audio_path, created_at, updated_at
            FROM fragmentum 
            WHERE folio_id = ?1 
            ORDER BY id
            "#,
        )
        .bind(folio_id)
        .fetch_all(pool)
        .await
        .with_context(|| "Failed to fetch fragmenta")?;

        Ok(fragmenta)
    }

    /// Get fragmentum by specific id
    pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Fragmentum>> {
        let fragmentum = sqlx::query_as::<_, Fragmentum>(
            r#"
            SELECT id, folio_id, content, audio_path, created_at, updated_at
            FROM fragmentum 
            WHERE id = ?1 
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .with_context(|| "Failed to fetch fragmentum")?;

        Ok(fragmentum)
    }

    /// Update fragmentum content (transcription)
    pub async fn update_content(&mut self, pool: &SqlitePool, new_content: String) -> Result<()> {
        let now = Utc::now();

        sqlx::query("UPDATE fragmentum SET content = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&new_content)
            .bind(now)
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to update fragmentum content")?;

        self.content = new_content;
        self.updated_at = now;

        Ok(())
    }

    /// Update fragmentum audio path
    pub async fn update_audio_path(
        &mut self,
        pool: &SqlitePool,
        new_audio_path: String,
    ) -> Result<()> {
        let now = Utc::now();

        sqlx::query("UPDATE fragmentum SET audio_path = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&new_audio_path)
            .bind(now)
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to update fragmentum audio path")?;

        self.audio_path = new_audio_path;
        self.updated_at = now;

        Ok(())
    }

    /// Delete fragmentum
    pub async fn delete(self, pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM fragmentum WHERE id = ?1")
            .bind(self.id)
            .execute(pool)
            .await
            .with_context(|| "Failed to delete fragmentum")?;

        Ok(())
    }
}

// ============================================================================
// UI Helper Operations
// ============================================================================

impl UICodex {
    /// Get all codices with their nested folia and fragmenta
    pub async fn get_all(pool: &SqlitePool) -> Result<Vec<UICodex>> {
        // Fetch all codices
        let codices = Codex::get_all(pool)
            .await
            .with_context(|| "Failed to fetch codices from db")?;

        let mut ui_codices = Vec::new();

        // For each codex, fetch its folia and create a UICodex
        for codex in codices {
            let folia = Folio::get_by_codex_id(pool, codex.id)
                .await
                .with_context(|| format!("Failed to fetch folia for codex {}", codex.id))?;

            let mut ui_folia = Vec::new();

            // For each folio, fetch its fragmenta
            for folio in folia {
                let fragmenta = Fragmentum::get_by_folio_id(pool, folio.id)
                    .await
                    .with_context(|| format!("Failed to fetch fragmenta for folio {}", folio.id))?
                    .into_iter()
                    .map(|f| UIFragmentum {
                        fragmentum: f,
                        state: ListState::default(),
                    })
                    .collect();

                ui_folia.push(UIFolio {
                    folio,
                    fragmentum_state: ListState::default(),
                    fragmenta,
                });
            }

            ui_codices.push(UICodex {
                codex,
                folio_state: ListState::default(),
                folia: ui_folia,
                is_expanded: false, // Start collapsed by default
            });
        }

        Ok(ui_codices)
    }

    /// Update folia when something changes (new folio, deleted folio)
    /// Keeps the same state instead of reinitializing it
    pub async fn update_folia(&mut self, pool: &SqlitePool) -> Result<()> {
        // Re-fetch the folia but don't change the codex state
        let folia = Folio::get_by_codex_id(pool, self.codex.id)
            .await
            .with_context(|| "Failed to fetch folia for codex")?;

        let mut ui_folia = Vec::new();

        // For each folio, fetch its fragmenta
        for folio in folia {
            let fragmenta = Fragmentum::get_by_folio_id(pool, folio.id)
                .await
                .with_context(|| format!("Failed to fetch fragmenta for folio {}", folio.id))?
                .into_iter()
                .map(|f| UIFragmentum {
                    fragmentum: f,
                    state: ListState::default(),
                })
                .collect();

            ui_folia.push(UIFolio {
                folio,
                fragmentum_state: ListState::default(),
                fragmenta,
            });
        }

        // Update the folia
        self.folia = ui_folia;

        Ok(())
    }
}

impl UIFolio {
    /// Update fragmenta when something changes (new fragmentum, deleted fragmentum, edited content)
    /// Keeps the same state instead of reinitializing it
    pub async fn update_fragmenta(&mut self, pool: &SqlitePool) -> Result<()> {
        // Re-fetch the fragmenta but don't change the folio state
        let fragmenta = Fragmentum::get_by_folio_id(pool, self.folio.id)
            .await
            .with_context(|| "Failed to fetch fragmenta for folio")?
            .into_iter()
            .map(|f| UIFragmentum {
                fragmentum: f,
                state: ListState::default(),
            })
            .collect();

        // Update the fragmenta
        self.fragmenta = fragmenta;

        Ok(())
    }
}
