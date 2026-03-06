use ratatui::style::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

// Default variables
const DEFAULT_PAGE_COLOR: &str = "#F3E5AB";
const DEFAULT_LIGHT_SHADOW_COLOR: &str = "#BFA37F";
const DEFAULT_MEDIUM_SHADOW_COLOR: &str = "#563F35";
const DEFAULT_DARK_SHADOW_COLOR: &str = "#2B1810";
const DEFAULT_VERY_DARK_SHADOW_COLOR: &str = "#1E0D04";
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
    Color::from_str(&s)
        .or_else(|_| Ok(Color::from_str(DEFAULT_PAGE_COLOR).expect("valid default page color")))
}

/// Deserialize foreground color with fallback to default
fn deserialize_foreground<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try to parse as hex color, fall back to default foreground on error
    Color::from_str(&s).or_else(|_| {
        Ok(Color::from_str(DEFAULT_DARK_SHADOW_COLOR).expect("valid default dark shadow color"))
    })
}

/// Deserialize highlight color with fallback to default
fn deserialize_highlight<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try to parse as hex color, fall back to default highlight on error
    Color::from_str(&s).or_else(|_| {
        Ok(Color::from_str(DEFAULT_HIGHLIGHT_COLOR).expect("valid default highlight color"))
    })
}

/// Theme configuration
#[derive(Deserialize, Serialize, Clone)]
pub struct ThemeConfig {
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_background"
    )]
    pub page: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_foreground"
    )]
    pub light_shadow: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_foreground"
    )]
    pub medium_shadow: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_foreground"
    )]
    pub dark_shadow: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_foreground"
    )]
    pub very_dark_shadow: Color,
    #[serde(
        serialize_with = "serialize_color",
        deserialize_with = "deserialize_highlight"
    )]
    pub highlight: Color,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            page: Color::from_str(DEFAULT_PAGE_COLOR).expect("valid default page color"),
            light_shadow: Color::from_str(DEFAULT_LIGHT_SHADOW_COLOR)
                .expect("valid default light shadow color"),
            medium_shadow: Color::from_str(DEFAULT_MEDIUM_SHADOW_COLOR)
                .expect("valid default medium shadow color"),
            dark_shadow: Color::from_str(DEFAULT_DARK_SHADOW_COLOR)
                .expect("valid default dark shadow color"),
            very_dark_shadow: Color::from_str(DEFAULT_VERY_DARK_SHADOW_COLOR)
                .expect("valid default very dark shadow color"),
            highlight: Color::from_str(DEFAULT_HIGHLIGHT_COLOR)
                .expect("valid default highlight color"),
        }
    }
}
