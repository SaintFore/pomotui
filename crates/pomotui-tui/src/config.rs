#![allow(clippy::missing_errors_doc)]

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
    pub reminder_enabled: bool,
    pub sound: Option<PathBuf>,
    pub volume: u8,
    pub animation: Option<PathBuf>,
    pub keybindings: Keybindings,
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
            reminder_enabled: true,
            sound: None,
            volume: 100,
            animation: None,
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
        "Vermilion Paper Light" | "Vermilion Paper Dark"
    ) {
        return Err("theme must be Vermilion Paper Light or Vermilion Paper Dark".into());
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
    Ok(config)
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
    }
}
