#![allow(clippy::missing_errors_doc)]

use crate::ColorOverrides;
use ratatui::style::Color;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub focus_minutes: u16,
    pub short_break_minutes: u16,
    pub long_break_minutes: u16,
    pub rounds_per_cycle: u8,
    pub theme: String,
    pub language: String,
    pub reminder_enabled: bool,
    pub sound: Option<PathBuf>,
    pub volume: u8,
    pub animation: Option<PathBuf>,
    pub colors: ThemeColors,
    pub keybindings: Keybindings,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ThemeColors {
    pub background: Option<String>,
    pub surface: Option<String>,
    pub text: Option<String>,
    pub muted: Option<String>,
    pub accent: Option<String>,
    pub gold: Option<String>,
    pub good: Option<String>,
    pub border: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Keybindings {
    pub down: String,
    pub up: String,
    pub next_view: String,
    pub previous_view: String,
    pub skip: String,
    pub toggle_session: String,
    pub stop: String,
    pub palette: String,
    pub settings: String,
    pub help: String,
    pub quit: String,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            down: "j".into(),
            up: "k".into(),
            next_view: "l".into(),
            previous_view: "h".into(),
            skip: "K".into(),
            toggle_session: " ".into(),
            stop: "X".into(),
            palette: ":".into(),
            settings: "s".into(),
            help: "?".into(),
            quit: "q".into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            rounds_per_cycle: 4,
            theme: "Vermilion Paper Dark".into(),
            language: "en".into(),
            reminder_enabled: true,
            sound: None,
            volume: 100,
            animation: None,
            colors: ThemeColors::default(),
            keybindings: Keybindings::default(),
        }
    }
}

pub fn parse(source: &str) -> Result<Config, String> {
    let config: Config = toml::from_str(source).map_err(|error| error.to_string())?;
    if config.focus_minutes == 0 {
        return Err("focus_minutes must be nonzero".into());
    }
    if config.short_break_minutes == 0 {
        return Err("short_break_minutes must be nonzero".into());
    }
    if config.long_break_minutes == 0 {
        return Err("long_break_minutes must be nonzero".into());
    }
    if config.rounds_per_cycle == 0 {
        return Err("rounds_per_cycle must be nonzero".into());
    }
    if config.volume > 100 {
        return Err("volume must be between 0 and 100".into());
    }
    if !matches!(
        config.theme.as_str(),
        "Vermilion Paper Light" | "Vermilion Paper Dark" | "Ran Paper Light" | "Ran Paper Dark"
    ) {
        return Err(
            "theme must be Vermilion Paper Light, Vermilion Paper Dark, Ran Paper Light, or Ran Paper Dark"
                .into(),
        );
    }
    if !matches!(config.language.as_str(), "en" | "zh-CN") {
        return Err("language must be en or zh-CN".into());
    }
    for (name, key) in [
        ("keybindings.down", &config.keybindings.down),
        ("keybindings.up", &config.keybindings.up),
        ("keybindings.next_view", &config.keybindings.next_view),
        (
            "keybindings.previous_view",
            &config.keybindings.previous_view,
        ),
        ("keybindings.skip", &config.keybindings.skip),
        (
            "keybindings.toggle_session",
            &config.keybindings.toggle_session,
        ),
        ("keybindings.stop", &config.keybindings.stop),
        ("keybindings.palette", &config.keybindings.palette),
        ("keybindings.settings", &config.keybindings.settings),
        ("keybindings.help", &config.keybindings.help),
        ("keybindings.quit", &config.keybindings.quit),
    ] {
        if key.chars().count() != 1 {
            return Err(format!("{name} must contain exactly one character"));
        }
    }
    config.color_overrides()?;
    Ok(config)
}

impl Config {
    pub fn color_overrides(&self) -> Result<ColorOverrides, String> {
        Ok(ColorOverrides {
            background: parse_optional_color(
                "colors.background",
                self.colors.background.as_deref(),
            )?,
            surface: parse_optional_color("colors.surface", self.colors.surface.as_deref())?,
            text: parse_optional_color("colors.text", self.colors.text.as_deref())?,
            muted: parse_optional_color("colors.muted", self.colors.muted.as_deref())?,
            accent: parse_optional_color("colors.accent", self.colors.accent.as_deref())?,
            gold: parse_optional_color("colors.gold", self.colors.gold.as_deref())?,
            good: parse_optional_color("colors.good", self.colors.good.as_deref())?,
            border: parse_optional_color("colors.border", self.colors.border.as_deref())?,
        })
    }
}

fn parse_optional_color(name: &str, value: Option<&str>) -> Result<Option<Color>, String> {
    value.map(|value| parse_color(name, value)).transpose()
}

fn parse_color(name: &str, value: &str) -> Result<Color, String> {
    let Some(hex) = value.strip_prefix('#').filter(|hex| hex.len() == 6) else {
        return Err(format!("{name} must use #RRGGBB"));
    };
    let red = u8::from_str_radix(&hex[0..2], 16).map_err(|_| format!("{name} must use #RRGGBB"))?;
    let green =
        u8::from_str_radix(&hex[2..4], 16).map_err(|_| format!("{name} must use #RRGGBB"))?;
    let blue =
        u8::from_str_radix(&hex[4..6], 16).map_err(|_| format!("{name} must use #RRGGBB"))?;
    Ok(Color::Rgb(red, green, blue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_documented_defaults() {
        assert_eq!(parse("").expect("defaults"), Config::default());
    }

    #[test]
    fn invalid_field_names_and_values_are_diagnostic() {
        assert!(
            parse("focus_minutes = 0")
                .expect_err("zero")
                .contains("focus_minutes")
        );
        assert!(
            parse("mystery = true")
                .expect_err("unknown")
                .contains("mystery")
        );
        assert!(
            parse("volume = 101")
                .expect_err("volume")
                .contains("volume")
        );
        assert!(
            parse("theme = \"blue\"")
                .expect_err("theme")
                .contains("theme")
        );
        assert!(
            parse("language = \"fr\"")
                .expect_err("language")
                .contains("language")
        );
        assert!(
            parse("[colors]\naccent = \"red\"")
                .expect_err("color")
                .contains("colors.accent")
        );
    }

    #[test]
    fn ran_theme_and_partial_color_overrides_are_supported() {
        let config =
            parse("theme = \"Ran Paper Dark\"\n[colors]\naccent = \"#A6231F\"\ngood = \"#315C78\"")
                .expect("theme");
        let overrides = config.color_overrides().expect("colors");
        assert_eq!(overrides.accent, Some(Color::Rgb(166, 35, 31)));
        assert_eq!(overrides.good, Some(Color::Rgb(49, 92, 120)));
        assert_eq!(overrides.background, None);
    }
}
