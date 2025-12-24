use serde::{Deserialize, Serialize};

// Default variables
const DEFAULT_DB_NAME: &str = "archivum";
const DEFAULT_DB_FILE: &str = "archivum.db";

/// Database configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct DBConfig {
    pub name: String,
    pub connection_str: String,
}

impl Default for DBConfig {
    fn default() -> Self {
        // Use data directory to standardize storage
        let data_dir = dirs::data_dir().unwrap().join("scriptor");

        // Create directory
        std::fs::create_dir_all(&data_dir).unwrap();

        // Create path to db
        let path = data_dir.join(DEFAULT_DB_FILE);

        // Create connection string (only SQLite is admissible)
        let connection_str = format!("sqlite:{}", path.display());

        Self {
            name: DEFAULT_DB_NAME.to_string(),
            connection_str,
        }
    }
}
