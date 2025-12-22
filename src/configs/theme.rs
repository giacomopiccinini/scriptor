use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

// Default variables
const DEFAULT_BACKGROUND_COLOR: &str = "#F3E5AB";
const DEFAULT_FOREGROUND_COLOR: &str = "#2B1810";
const DEFAULT_HIGHLIGHT_COLOR: &str = "#8B0000";

/// Custom serialization for ratatui Color - only hex format
fn serialize_color<S>(color: &Color, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Convert Color to hex string for serialization
    match color {
        Color::Rgb(r, g, b) => {
            let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
            serializer.serialize_str(&hex)
        }
        _ => {
            // For non-RGB colors, serialize as black hex
            serializer.serialize_str("#000000")
        }
    }
}

/// Deserialize background color with fallback to default
fn deserialize_background<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try to parse as hex color, fall back to default background on error
    Color::from_str(&s).or_else(|_| Ok(Color::from_str(DEFAULT_BACKGROUND_COLOR).unwrap()))
}

/// Deserialize foreground color with fallback to default
fn deserialize_foreground<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try to parse as hex color, fall back to default foreground on error
    Color::from_str(&s).or_else(|_| Ok(Color::from_str(DEFAULT_FOREGROUND_COLOR).unwrap()))
}

/// Deserialize highlight color with fallback to default
fn deserialize_highlight<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try to parse as hex color, fall back to default highlight on error
    Color::from_str(&s).or_else(|_| Ok(Color::from_str(DEFAULT_HIGHLIGHT_COLOR).unwrap()))
}

/// Theme configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct ThemeConfig {
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_background"
    )]
    pub background: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_foreground"
    )]
    pub foreground: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_highlight"
    )]
    pub highlight: Color,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            background: Color::from_str(DEFAULT_BACKGROUND_COLOR).unwrap(),
            foreground: Color::from_str(DEFAULT_FOREGROUND_COLOR).unwrap(),
            highlight: Color::from_str(DEFAULT_HIGHLIGHT_COLOR).unwrap(),
        }
    }
}
