// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use crate::completion::{CompletionConfig, CompletionMatching, CompletionStyle};
use crate::option::{OptionId, OptionRegistry, OptionValue};
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
    pub fill_column: usize,
    pub line_numbers: bool,
    pub syntax_highlighting: bool,
    pub search_highlighting: bool,
    pub backup_on_save: bool,
    pub backup_directory: Option<PathBuf>,
    pub auto_save: bool,
    pub auto_save_interval: usize,
    pub auto_save_timeout_seconds: usize,
    pub auto_save_directory: Option<PathBuf>,
    pub delete_auto_save_files: bool,
    pub theme: ThemeName,
    pub completion: CompletionConfig,
}

impl Default for Config {
    fn default() -> Self {
        let registry = OptionRegistry::default();
        let mut config = Self::empty_for_registry_defaults();
        for option in registry.options() {
            config
                .apply_option_value(option.id, option.default.clone())
                .expect("default option values should match config fields");
        }
        config
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
        let registry = OptionRegistry::default();
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
            let Some(option) = registry.get_by_config_key(key) else {
                return Err(config_error(line_number, format!("unknown key `{key}`")));
            };
            let value = option
                .parse_value(value)
                .map_err(|message| config_error(line_number, message))?;
            config
                .apply_option_value(option.id, value)
                .map_err(|message| config_error(line_number, message))?;
        }
        Ok(config)
    }

    pub fn option_value(&self, option: OptionId) -> OptionValue {
        match option {
            OptionId::TabWidth => OptionValue::Integer(self.tab_width),
            OptionId::FillColumn => OptionValue::Integer(self.fill_column),
            OptionId::LineNumbers => OptionValue::Boolean(self.line_numbers),
            OptionId::SyntaxHighlighting => OptionValue::Boolean(self.syntax_highlighting),
            OptionId::SearchHighlighting => OptionValue::Boolean(self.search_highlighting),
            OptionId::BackupOnSave => OptionValue::Boolean(self.backup_on_save),
            OptionId::BackupDirectory => OptionValue::String(
                self.backup_directory
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            ),
            OptionId::AutoSave => OptionValue::Boolean(self.auto_save),
            OptionId::AutoSaveInterval => OptionValue::Integer(self.auto_save_interval),
            OptionId::AutoSaveTimeoutSeconds => {
                OptionValue::Integer(self.auto_save_timeout_seconds)
            }
            OptionId::AutoSaveDirectory => OptionValue::String(
                self.auto_save_directory
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            ),
            OptionId::DeleteAutoSaveFiles => OptionValue::Boolean(self.delete_auto_save_files),
            OptionId::Theme => OptionValue::Choice(self.theme.name()),
            OptionId::CompletionStyle => OptionValue::Choice(self.completion.style.name()),
            OptionId::CompletionMaxCandidates => {
                OptionValue::Integer(self.completion.max_candidates)
            }
            OptionId::CompletionShowAnnotations => {
                OptionValue::Boolean(self.completion.show_annotations)
            }
            OptionId::CompletionMatching => OptionValue::Choice(self.completion.matching.name()),
        }
    }

    fn empty_for_registry_defaults() -> Self {
        Self {
            tab_width: 0,
            fill_column: 0,
            line_numbers: false,
            syntax_highlighting: false,
            search_highlighting: false,
            backup_on_save: false,
            backup_directory: None,
            auto_save: false,
            auto_save_interval: 0,
            auto_save_timeout_seconds: 0,
            auto_save_directory: None,
            delete_auto_save_files: false,
            theme: ThemeName::Default,
            completion: CompletionConfig {
                style: CompletionStyle::Vertical,
                max_candidates: 0,
                show_annotations: false,
                matching: CompletionMatching::Orderless,
            },
        }
    }

    fn apply_option_value(
        &mut self,
        option: OptionId,
        value: OptionValue,
    ) -> std::result::Result<(), &'static str> {
        match (option, value) {
            (OptionId::TabWidth, OptionValue::Integer(value)) => self.tab_width = value,
            (OptionId::FillColumn, OptionValue::Integer(value)) => self.fill_column = value,
            (OptionId::LineNumbers, OptionValue::Boolean(value)) => self.line_numbers = value,
            (OptionId::SyntaxHighlighting, OptionValue::Boolean(value)) => {
                self.syntax_highlighting = value;
            }
            (OptionId::SearchHighlighting, OptionValue::Boolean(value)) => {
                self.search_highlighting = value;
            }
            (OptionId::BackupOnSave, OptionValue::Boolean(value)) => self.backup_on_save = value,
            (OptionId::BackupDirectory, OptionValue::String(value)) => {
                self.backup_directory = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            (OptionId::AutoSave, OptionValue::Boolean(value)) => self.auto_save = value,
            (OptionId::AutoSaveInterval, OptionValue::Integer(value)) => {
                self.auto_save_interval = value;
            }
            (OptionId::AutoSaveTimeoutSeconds, OptionValue::Integer(value)) => {
                self.auto_save_timeout_seconds = value;
            }
            (OptionId::AutoSaveDirectory, OptionValue::String(value)) => {
                self.auto_save_directory = if value.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(value))
                };
            }
            (OptionId::DeleteAutoSaveFiles, OptionValue::Boolean(value)) => {
                self.delete_auto_save_files = value;
            }
            (OptionId::Theme, OptionValue::Choice("default")) => self.theme = ThemeName::Default,
            (OptionId::Theme, OptionValue::Choice("mono")) => self.theme = ThemeName::Mono,
            (OptionId::CompletionStyle, OptionValue::Choice("vertical")) => {
                self.completion.style = CompletionStyle::Vertical;
            }
            (OptionId::CompletionStyle, OptionValue::Choice("completions-buffer")) => {
                self.completion.style = CompletionStyle::CompletionsBuffer;
            }
            (OptionId::CompletionStyle, OptionValue::Choice("ido")) => {
                self.completion.style = CompletionStyle::Ido;
            }
            (OptionId::CompletionMaxCandidates, OptionValue::Integer(value)) => {
                self.completion.max_candidates = value;
            }
            (OptionId::CompletionShowAnnotations, OptionValue::Boolean(value)) => {
                self.completion.show_annotations = value;
            }
            (OptionId::CompletionMatching, OptionValue::Choice("prefix")) => {
                self.completion.matching = CompletionMatching::Prefix;
            }
            (OptionId::CompletionMatching, OptionValue::Choice("orderless")) => {
                self.completion.matching = CompletionMatching::Orderless;
            }
            (OptionId::CompletionMatching, OptionValue::Choice("substring")) => {
                self.completion.matching = CompletionMatching::Substring;
            }
            _ => return Err("option value does not match option type"),
        }
        Ok(())
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

fn config_error(line_number: usize, message: impl Into<String>) -> RileError {
    RileError::InvalidInput(format!("config line {line_number}: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{CompletionMatching, CompletionStyle, Config, ThemeName};
    use crate::option::{OptionId, OptionRegistry, OptionValue};

    #[test]
    fn parses_minimal_toml_subset() {
        let config = Config::parse(
            r#"
            [editor]
            tab_width = 2
            fill_column = 72
            line_numbers = true
            syntax_highlighting = false
            search_highlighting = false
            backup_on_save = true
            backup_directory = "/tmp/rile-backups"
            auto_save = true
            auto_save_interval = 300
            auto_save_timeout_seconds = 30
            auto_save_directory = "/tmp/rile-auto-save"
            delete_auto_save_files = false
            theme = "mono"
            completion_style = "ido"
            completion_max_candidates = 5
            completion_show_annotations = false
            completion_matching = "orderless"
            "#,
        )
        .expect("config should parse");

        assert_eq!(config.tab_width, 2);
        assert_eq!(config.fill_column, 72);
        assert!(config.line_numbers);
        assert!(!config.syntax_highlighting);
        assert!(!config.search_highlighting);
        assert!(config.backup_on_save);
        assert_eq!(
            config.backup_directory,
            Some(PathBuf::from("/tmp/rile-backups"))
        );
        assert!(config.auto_save);
        assert_eq!(config.auto_save_interval, 300);
        assert_eq!(config.auto_save_timeout_seconds, 30);
        assert_eq!(
            config.auto_save_directory,
            Some(PathBuf::from("/tmp/rile-auto-save"))
        );
        assert!(!config.delete_auto_save_files);
        assert_eq!(config.theme, ThemeName::Mono);
        assert_eq!(config.completion.style, CompletionStyle::Ido);
        assert_eq!(config.completion.max_candidates, 5);
        assert!(!config.completion.show_annotations);
        assert_eq!(config.completion.matching, CompletionMatching::Orderless);
    }

    #[test]
    fn rejects_invalid_config_values() {
        assert!(Config::parse("tab_width = 0").is_err());
        assert!(Config::parse("fill_column = 19").is_err());
        assert!(Config::parse("line_numbers = yes").is_err());
        assert!(Config::parse("backup_on_save = sometimes").is_err());
        assert!(Config::parse("auto_save = sometimes").is_err());
        assert!(Config::parse("auto_save_interval = nope").is_err());
        assert!(Config::parse("auto_save_timeout_seconds = nope").is_err());
        assert!(Config::parse("delete_auto_save_files = sometimes").is_err());
        assert!(Config::parse("theme = \"solarized\"").is_err());
        assert!(Config::parse("completion_style = \"popup\"").is_err());
        assert!(Config::parse("completion_max_candidates = 0").is_err());
        assert!(Config::parse("completion_matching = \"basic-substring\"").is_err());
        assert!(Config::parse("completion_matching = \"fuzzy\"").is_err());
        assert!(Config::parse("unknown = true").is_err());
    }

    #[test]
    fn default_values_come_from_option_registry() {
        let config = Config::default();
        let registry = OptionRegistry::default();

        for option in registry.options() {
            assert_eq!(
                config.option_value(option.id),
                option.default,
                "{} default should match registry",
                option.name
            );
        }
    }

    #[test]
    fn exposes_current_option_values() {
        let config = Config::parse(
            r#"
            tab_width = 2
            fill_column = 80
            completion_matching = "substring"
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.option_value(OptionId::TabWidth),
            OptionValue::Integer(2)
        );
        assert_eq!(
            config.option_value(OptionId::FillColumn),
            OptionValue::Integer(80)
        );
        assert_eq!(
            config.option_value(OptionId::CompletionMatching),
            OptionValue::Choice("substring")
        );
    }
}
