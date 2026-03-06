//! Integration tests for Scriptor config.

mod common;

use scriptor::configs::scriptor::ScriptorConfig;
use std::fs;

#[test]
fn test_config_write_read_roundtrip() {
    let config = ScriptorConfig::default();
    let (_temp_dir, config_dir) = common::temp_config_dir().unwrap();
    let config_path = config_dir.join("scriptor.toml");

    config.write(&config_path).unwrap();
    let content = fs::read_to_string(&config_path).unwrap();
    let parsed: ScriptorConfig = toml::from_str(&content).unwrap();

    assert_eq!(parsed.dbs.len(), config.dbs.len());
}
