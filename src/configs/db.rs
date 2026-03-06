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
        let data_dir = dirs::data_dir()
            .expect("Could not find data directory")
            .join("scriptor");

        // Create path to db
        let db_dir_path = data_dir.join("databases");
        let db_path = db_dir_path.join(DEFAULT_DB_FILE);

        // Create directory
        std::fs::create_dir_all(&db_dir_path).expect("Could not create database directory");

        // Create connection string (only SQLite is admissible)
        let connection_str = format!("sqlite:{}", db_path.display());

        Self {
            name: DEFAULT_DB_NAME.to_string(),
            connection_str,
        }
    }
}
