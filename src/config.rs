// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use crate::completion::{CompletionConfig, CompletionMatching, CompletionStyle};
use crate::{Result, RileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Default,
    Mono,
}

impl ThemeName {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Mono => "mono",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub tab_width: usize,
    pub line_numbers: bool,
    pub syntax_highlighting: bool,
    pub search_highlighting: bool,
    pub backup_on_save: bool,
    pub theme: ThemeName,
    pub completion: CompletionConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tab_width: 4,
            line_numbers: false,
            syntax_highlighting: true,
            search_highlighting: true,
            backup_on_save: false,
            theme: ThemeName::Default,
            completion: CompletionConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let Some(path) = default_config_path() else {
            return Ok(Self::default());
        };
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut config = Self::default();
        for (line_index, raw_line) in text.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || is_section_header(line) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(config_error(line_number, "expected key = value"));
            };
            let key = key.trim();
            let value = strip_inline_comment(value.trim()).trim();
            match key {
                "tab_width" => config.tab_width = parse_tab_width(value, line_number)?,
                "line_numbers" => config.line_numbers = parse_bool(value, line_number)?,
                "syntax_highlighting" => {
                    config.syntax_highlighting = parse_bool(value, line_number)?;
                }
                "search_highlighting" => {
                    config.search_highlighting = parse_bool(value, line_number)?;
                }
                "backup_on_save" => config.backup_on_save = parse_bool(value, line_number)?,
                "theme" => config.theme = parse_theme(value, line_number)?,
                "completion_style" => {
                    config.completion.style = parse_completion_style(value, line_number)?;
                }
                "completion_max_candidates" => {
                    config.completion.max_candidates =
                        parse_completion_max_candidates(value, line_number)?;
                }
                "completion_show_annotations" => {
                    config.completion.show_annotations = parse_bool(value, line_number)?;
                }
                "completion_matching" => {
                    config.completion.matching = parse_completion_matching(value, line_number)?;
                }
                _ => return Err(config_error(line_number, format!("unknown key `{key}`"))),
            }
        }
        Ok(config)
    }
}

pub fn default_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("rile")
            .join("config.toml")
    })
}

fn is_section_header(line: &str) -> bool {
    line.starts_with('[') && line.ends_with(']')
}

fn strip_inline_comment(value: &str) -> &str {
    let mut in_string = false;
    for (byte, character) in value.char_indices() {
        match character {
            '"' => in_string = !in_string,
            '#' if !in_string => return &value[..byte],
            _ => {}
        }
    }
    value
}

fn parse_tab_width(value: &str, line_number: usize) -> Result<usize> {
    let width = value
        .parse::<usize>()
        .map_err(|_| config_error(line_number, "tab_width must be an integer"))?;
    if !(1..=16).contains(&width) {
        return Err(config_error(
            line_number,
            "tab_width must be between 1 and 16",
        ));
    }
    Ok(width)
}

fn parse_bool(value: &str, line_number: usize) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(config_error(line_number, "expected true or false")),
    }
}

fn parse_theme(value: &str, line_number: usize) -> Result<ThemeName> {
    match unquote(value) {
        "default" => Ok(ThemeName::Default),
        "mono" => Ok(ThemeName::Mono),
        _ => Err(config_error(
            line_number,
            "theme must be `default` or `mono`",
        )),
    }
}

fn parse_completion_style(value: &str, line_number: usize) -> Result<CompletionStyle> {
    match unquote(value) {
        "vertical" => Ok(CompletionStyle::Vertical),
        "completions-buffer" => Ok(CompletionStyle::CompletionsBuffer),
        "ido" => Ok(CompletionStyle::Ido),
        _ => Err(config_error(
            line_number,
            "completion_style must be `vertical`, `completions-buffer`, or `ido`",
        )),
    }
}

fn parse_completion_max_candidates(value: &str, line_number: usize) -> Result<usize> {
    let max = value
        .parse::<usize>()
        .map_err(|_| config_error(line_number, "completion_max_candidates must be an integer"))?;
    if !(1..=20).contains(&max) {
        return Err(config_error(
            line_number,
            "completion_max_candidates must be between 1 and 20",
        ));
    }
    Ok(max)
}

fn parse_completion_matching(value: &str, line_number: usize) -> Result<CompletionMatching> {
    match unquote(value) {
        "prefix" => Ok(CompletionMatching::Prefix),
        "substring" => Ok(CompletionMatching::Substring),
        _ => Err(config_error(
            line_number,
            "completion_matching must be `prefix` or `substring`",
        )),
    }
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn config_error(line_number: usize, message: impl Into<String>) -> RileError {
    RileError::InvalidInput(format!("config line {line_number}: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::{CompletionMatching, CompletionStyle, Config, ThemeName};

    #[test]
    fn parses_minimal_toml_subset() {
        let config = Config::parse(
            r#"
            [editor]
            tab_width = 2
            line_numbers = true
            syntax_highlighting = false
            search_highlighting = false
            backup_on_save = true
            theme = "mono"
            completion_style = "ido"
            completion_max_candidates = 5
            completion_show_annotations = false
            completion_matching = "substring"
            "#,
        )
        .expect("config should parse");

        assert_eq!(config.tab_width, 2);
        assert!(config.line_numbers);
        assert!(!config.syntax_highlighting);
        assert!(!config.search_highlighting);
        assert!(config.backup_on_save);
        assert_eq!(config.theme, ThemeName::Mono);
        assert_eq!(config.completion.style, CompletionStyle::Ido);
        assert_eq!(config.completion.max_candidates, 5);
        assert!(!config.completion.show_annotations);
        assert_eq!(config.completion.matching, CompletionMatching::Substring);
    }

    #[test]
    fn rejects_invalid_config_values() {
        assert!(Config::parse("tab_width = 0").is_err());
        assert!(Config::parse("line_numbers = yes").is_err());
        assert!(Config::parse("backup_on_save = sometimes").is_err());
        assert!(Config::parse("theme = \"solarized\"").is_err());
        assert!(Config::parse("completion_style = \"popup\"").is_err());
        assert!(Config::parse("completion_max_candidates = 0").is_err());
        assert!(Config::parse("completion_matching = \"fuzzy\"").is_err());
        assert!(Config::parse("unknown = true").is_err());
    }
}
