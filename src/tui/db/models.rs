use chrono::{DateTime, Utc};
use ratatui::widgets::ListState;
use sqlx::FromRow;

#[derive(Debug, FromRow, Clone)]
pub struct Codex {
    pub id: i64,
    pub name: String,
    pub ordering: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
pub struct Folio {
    pub id: i64,
    pub codex_id: i64,
    pub name: String,
    pub ordering: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
pub struct Fragmentum {
    pub id: i64,
    pub folio_id: i64,
    pub content: String,
    pub audio_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Structs for creating new records (without id and timestamps)
#[derive(Debug)]
pub struct NewCodex {
    pub name: String,
}

#[derive(Debug)]
pub struct NewFolio {
    pub codex_id: i64,
    pub name: String,
}

#[derive(Debug)]
pub struct NewFragmentum {
    pub folio_id: i64,
    pub content: String,
}

// Convenient repackaging of DB items to cache reads from DB
#[derive(Debug, Clone)]
pub struct UICodex {
    pub codex: Codex,
    pub folio_state: ListState,
    pub folia: Vec<UIFolio>,
}

#[derive(Debug, Clone)]
pub struct UIFolio {
    pub folio: Folio,
    pub fragmentum_state: ListState,
    pub fragmenta: Vec<UIFragmentum>,
}

#[derive(Debug, Clone)]
pub struct UIFragmentum {
    pub fragmentum: Fragmentum,
    pub state: ListState,
}
