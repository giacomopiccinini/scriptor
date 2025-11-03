-- Initial schema for Scriba speech-to-text app
-- Codex = Project, Folio = Recording, Fragmentum = Text/Audio chunk

-- Table for codices (projects)
CREATE TABLE IF NOT EXISTS codices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Table for folia (recordings within a codex)
CREATE TABLE IF NOT EXISTS folia (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    codex_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (codex_id) REFERENCES codices(id) ON DELETE CASCADE
);

-- Table for fragmenta (text/audio chunks within a folio)
CREATE TABLE IF NOT EXISTS fragmenta (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folio_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    audio_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (folio_id) REFERENCES folia(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_codices_ordering ON codices(ordering);
CREATE INDEX IF NOT EXISTS idx_folia_codex_id ON folia(codex_id);
CREATE INDEX IF NOT EXISTS idx_folia_ordering ON folia(ordering);
CREATE INDEX IF NOT EXISTS idx_fragmenta_folio_id ON fragmenta(folio_id);

