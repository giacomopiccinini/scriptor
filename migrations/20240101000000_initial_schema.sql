-- Initial schema for Scriptor speech-to-text app
-- Codex = Project, Folio = Recording, Fragmentum = Text/Audio chunk

-- Table for codex
CREATE TABLE IF NOT EXISTS codex (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Table for folio
CREATE TABLE IF NOT EXISTS folio (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codex_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (codex_id) REFERENCES codex(id) ON DELETE CASCADE
);

-- Table for fragmentum
CREATE TABLE IF NOT EXISTS fragmentum (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folio_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    audio_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (folio_id) REFERENCES folio(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_codex_ordering ON codex(ordering);
CREATE INDEX IF NOT EXISTS idx_folio_codex_id ON folio(codex_id);
CREATE INDEX IF NOT EXISTS idx_folio_ordering ON folio(ordering);
CREATE INDEX IF NOT EXISTS idx_fragmentum_folio_id ON fragmentum(folio_id);

