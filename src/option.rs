// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionId {
    TabWidth,
    FillColumn,
    LineNumbers,
    SyntaxHighlighting,
    SearchHighlighting,
    BackupOnSave,
    BackupDirectory,
    AutoSave,
    AutoSaveInterval,
    AutoSaveTimeoutSeconds,
    AutoSaveDirectory,
    DeleteAutoSaveFiles,
    Theme,
    CompletionStyle,
    CompletionMaxCandidates,
    CompletionShowAnnotations,
    CompletionMatching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Boolean,
    Integer,
    String,
    Choice(&'static [&'static str]),
}

impl OptionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::String => "string",
            Self::Choice(_) => "choice",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    Boolean(bool),
    Integer(usize),
    String(String),
    Choice(&'static str),
}

impl fmt::Display for OptionValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => write!(formatter, "{value}"),
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::String(value) => write!(formatter, "{value}"),
            Self::Choice(value) => write!(formatter, "{value}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionSpec {
    pub id: OptionId,
    pub name: &'static str,
    pub config_key: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub value_type: OptionType,
    pub default: OptionValue,
    pub valid_values: &'static str,
    validator: fn(&OptionValue) -> bool,
    parse_error: &'static str,
    validation_error: &'static str,
}

impl OptionSpec {
    pub fn parse_value(&self, text: &str) -> std::result::Result<OptionValue, &'static str> {
        let value = match self.value_type {
            OptionType::Boolean => match text {
                "true" => OptionValue::Boolean(true),
                "false" => OptionValue::Boolean(false),
                _ => return Err(self.parse_error),
            },
            OptionType::Integer => text
                .parse::<usize>()
                .map(OptionValue::Integer)
                .map_err(|_| self.parse_error)?,
            OptionType::String => OptionValue::String(unquote(text).to_owned()),
            OptionType::Choice(choices) => {
                let unquoted = unquote(text);
                let Some(choice) = choices.iter().copied().find(|choice| *choice == unquoted)
                else {
                    return Err(self.validation_error);
                };
                OptionValue::Choice(choice)
            }
        };
        self.validate_value(&value)?;
        Ok(value)
    }

    pub fn validate_value(&self, value: &OptionValue) -> std::result::Result<(), &'static str> {
        if (self.validator)(value) {
            Ok(())
        } else {
            Err(self.validation_error)
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionRegistry {
    specs: Vec<OptionSpec>,
}

impl OptionRegistry {
    pub fn new(specs: Vec<OptionSpec>) -> std::result::Result<Self, String> {
        let registry = Self { specs };
        registry.validate()?;
        Ok(registry)
    }

    pub fn options(&self) -> impl Iterator<Item = &OptionSpec> {
        self.specs.iter()
    }

    pub fn get(&self, name: &str) -> Option<&OptionSpec> {
        self.specs.iter().find(|spec| spec.name == name)
    }

    pub fn get_by_config_key(&self, config_key: &str) -> Option<&OptionSpec> {
        self.specs.iter().find(|spec| spec.config_key == config_key)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn validate(&self) -> std::result::Result<(), String> {
        for (index, spec) in self.specs.iter().enumerate() {
            if spec.name.is_empty()
                || spec.config_key.is_empty()
                || spec.summary.is_empty()
                || spec.doc.is_empty()
                || spec.valid_values.is_empty()
            {
                return Err(format!(
                    "option `{}` is missing required metadata",
                    spec.name
                ));
            }
            spec.validate_value(&spec.default).map_err(|message| {
                format!("option `{}` default is invalid: {message}", spec.name)
            })?;
            for other in self.specs.iter().skip(index + 1) {
                if spec.id == other.id {
                    return Err(format!("duplicate option id `{}`", spec.name));
                }
                if spec.name == other.name {
                    return Err(format!("duplicate option name `{}`", spec.name));
                }
                if spec.config_key == other.config_key {
                    return Err(format!("duplicate option config key `{}`", spec.config_key));
                }
            }
        }
        Ok(())
    }
}

impl Default for OptionRegistry {
    fn default() -> Self {
        Self::new(default_options()).expect("default option registry should be valid")
    }
}

const THEME_VALUES: &[&str] = &["default", "mono"];
const COMPLETION_STYLE_VALUES: &[&str] = &["vertical", "completions-buffer", "ido"];
const COMPLETION_MATCHING_VALUES: &[&str] = &["orderless", "prefix", "substring"];

fn default_options() -> Vec<OptionSpec> {
    vec![
        option_spec(OptionSpecData {
            id: OptionId::TabWidth,
            name: "tab_width",
            summary: "Tab width",
            doc: "Display width used for tab characters.",
            value_type: OptionType::Integer,
            default: OptionValue::Integer(4),
            valid_values: "integer from 1 through 16",
            validator: valid_tab_width,
            parse_error: "tab_width must be an integer",
            validation_error: "tab_width must be between 1 and 16",
        }),
        option_spec(OptionSpecData {
            id: OptionId::FillColumn,
            name: "fill_column",
            summary: "Fill column",
            doc: "Display column used by fill-paragraph when wrapping plain-text paragraphs.",
            value_type: OptionType::Integer,
            default: OptionValue::Integer(70),
            valid_values: "integer from 20 through 200",
            validator: valid_fill_column,
            parse_error: "fill_column must be an integer",
            validation_error: "fill_column must be between 20 and 200",
        }),
        option_spec(OptionSpecData {
            id: OptionId::LineNumbers,
            name: "line_numbers",
            summary: "Line numbers",
            doc: "Whether normal editing buffers show a left line-number gutter.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(false),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::SyntaxHighlighting,
            name: "syntax_highlighting",
            summary: "Syntax highlighting",
            doc: "Whether supported major modes highlight syntax while rendering buffers.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(true),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::SearchHighlighting,
            name: "search_highlighting",
            summary: "Search highlighting",
            doc: "Whether active search matches are highlighted in the selected buffer.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(true),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::BackupOnSave,
            name: "backup_on_save",
            summary: "Backup on save",
            doc: "Whether saving an existing regular file writes one persistent backup for each buffer visit. Unix backups use mode 0600.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(false),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::BackupDirectory,
            name: "backup_directory",
            summary: "Backup directory",
            doc: "Directory for backup_on_save files. Empty keeps file~ backups beside the saved file. A non-empty directory is checked when a backup is written; backup files there use mapped path-based names.",
            value_type: OptionType::String,
            default: OptionValue::String(String::new()),
            valid_values: "directory path string, or empty string for sibling backups",
            validator: valid_string,
            parse_error: "backup_directory must be a string",
            validation_error: "backup_directory must be a string",
        }),
        option_spec(OptionSpecData {
            id: OptionId::AutoSave,
            name: "auto_save",
            summary: "Auto-save",
            doc: "Whether dirty file-visiting buffers write Emacs-style #file# auto-save files.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(false),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::AutoSaveInterval,
            name: "auto_save_interval",
            summary: "Auto-save interval",
            doc: "Number of handled key events between auto-save writes. Zero disables event-count auto-save.",
            value_type: OptionType::Integer,
            default: OptionValue::Integer(300),
            valid_values: "integer from 0 through 100000",
            validator: valid_auto_save_interval,
            parse_error: "auto_save_interval must be an integer",
            validation_error: "auto_save_interval must be between 0 and 100000",
        }),
        option_spec(OptionSpecData {
            id: OptionId::AutoSaveTimeoutSeconds,
            name: "auto_save_timeout_seconds",
            summary: "Auto-save idle timeout",
            doc: "Idle seconds before auto-save writes dirty buffers. Zero disables idle auto-save.",
            value_type: OptionType::Integer,
            default: OptionValue::Integer(30),
            valid_values: "integer from 0 through 3600",
            validator: valid_auto_save_timeout_seconds,
            parse_error: "auto_save_timeout_seconds must be an integer",
            validation_error: "auto_save_timeout_seconds must be between 0 and 3600",
        }),
        option_spec(OptionSpecData {
            id: OptionId::AutoSaveDirectory,
            name: "auto_save_directory",
            summary: "Auto-save directory",
            doc: "Directory for auto_save files. Empty keeps #file# auto-save files beside the visited file. A non-empty directory is checked when auto-save writes; files there use mapped path-based names wrapped in #...#.",
            value_type: OptionType::String,
            default: OptionValue::String(String::new()),
            valid_values: "directory path string, or empty string for sibling auto-save files",
            validator: valid_string,
            parse_error: "auto_save_directory must be a string",
            validation_error: "auto_save_directory must be a string",
        }),
        option_spec(OptionSpecData {
            id: OptionId::DeleteAutoSaveFiles,
            name: "delete_auto_save_files",
            summary: "Delete auto-save files",
            doc: "Whether a successful explicit save removes the buffer's current-session auto-save file.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(true),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::Theme,
            name: "theme",
            summary: "Theme",
            doc: "Color theme used for terminal faces and status text.",
            value_type: OptionType::Choice(THEME_VALUES),
            default: OptionValue::Choice("default"),
            valid_values: "default or mono",
            validator: valid_theme,
            parse_error: "theme must be `default` or `mono`",
            validation_error: "theme must be `default` or `mono`",
        }),
        option_spec(OptionSpecData {
            id: OptionId::CompletionStyle,
            name: "completion_style",
            summary: "Completion style",
            doc: "Display style used by completion-enabled minibuffer prompts.",
            value_type: OptionType::Choice(COMPLETION_STYLE_VALUES),
            default: OptionValue::Choice("vertical"),
            valid_values: "vertical, completions-buffer, or ido",
            validator: valid_completion_style,
            parse_error: "completion_style must be `vertical`, `completions-buffer`, or `ido`",
            validation_error: "completion_style must be `vertical`, `completions-buffer`, or `ido`",
        }),
        option_spec(OptionSpecData {
            id: OptionId::CompletionMaxCandidates,
            name: "completion_max_candidates",
            summary: "Completion candidate limit",
            doc: "Maximum number of vertical completion candidates shown at once.",
            value_type: OptionType::Integer,
            default: OptionValue::Integer(8),
            valid_values: "integer from 1 through 20",
            validator: valid_completion_max_candidates,
            parse_error: "completion_max_candidates must be an integer",
            validation_error: "completion_max_candidates must be between 1 and 20",
        }),
        option_spec(OptionSpecData {
            id: OptionId::CompletionShowAnnotations,
            name: "completion_show_annotations",
            summary: "Completion annotations",
            doc: "Whether completion candidates show descriptions or file-kind annotations.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(true),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        }),
        option_spec(OptionSpecData {
            id: OptionId::CompletionMatching,
            name: "completion_matching",
            summary: "Completion matching",
            doc: "Matching strategy used for command, option, and buffer completion candidates. File prompts use file-category prefix, partial-completion, and substring matching when this is `orderless`.",
            value_type: OptionType::Choice(COMPLETION_MATCHING_VALUES),
            default: OptionValue::Choice("orderless"),
            valid_values: "orderless, prefix, or substring",
            validator: valid_completion_matching,
            parse_error: "completion_matching must be `orderless`, `prefix`, or `substring`",
            validation_error: "completion_matching must be `orderless`, `prefix`, or `substring`",
        }),
    ]
}

struct OptionSpecData {
    id: OptionId,
    name: &'static str,
    summary: &'static str,
    doc: &'static str,
    value_type: OptionType,
    default: OptionValue,
    valid_values: &'static str,
    validator: fn(&OptionValue) -> bool,
    parse_error: &'static str,
    validation_error: &'static str,
}

fn option_spec(data: OptionSpecData) -> OptionSpec {
    OptionSpec {
        id: data.id,
        name: data.name,
        config_key: data.name,
        summary: data.summary,
        doc: data.doc,
        value_type: data.value_type,
        default: data.default,
        valid_values: data.valid_values,
        validator: data.validator,
        parse_error: data.parse_error,
        validation_error: data.validation_error,
    }
}

fn valid_boolean(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Boolean(_))
}

fn valid_string(value: &OptionValue) -> bool {
    matches!(value, OptionValue::String(_))
}

fn valid_tab_width(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Integer(width) if (1..=16).contains(width))
}

fn valid_fill_column(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Integer(column) if (20..=200).contains(column))
}

fn valid_theme(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Choice("default" | "mono"))
}

fn valid_completion_style(value: &OptionValue) -> bool {
    matches!(
        value,
        OptionValue::Choice("vertical" | "completions-buffer" | "ido")
    )
}

fn valid_completion_max_candidates(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Integer(max) if (1..=20).contains(max))
}

fn valid_auto_save_interval(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Integer(interval) if (0..=100000).contains(interval))
}

fn valid_auto_save_timeout_seconds(value: &OptionValue) -> bool {
    matches!(value, OptionValue::Integer(seconds) if (0..=3600).contains(seconds))
}

fn valid_completion_matching(value: &OptionValue) -> bool {
    matches!(
        value,
        OptionValue::Choice("orderless" | "prefix" | "substring")
    )
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::{OptionId, OptionRegistry, OptionSpec, OptionType, OptionValue, valid_boolean};

    #[test]
    fn default_option_registry_is_valid() {
        let registry = OptionRegistry::default();

        assert_eq!(registry.validate(), Ok(()));
        assert_eq!(registry.options().count(), 17);
    }

    #[test]
    fn every_option_has_required_metadata_and_valid_default() {
        for option in OptionRegistry::default().options() {
            assert!(!option.name.is_empty());
            assert!(!option.config_key.is_empty());
            assert!(!option.summary.is_empty());
            assert!(!option.doc.is_empty());
            assert!(!option.value_type.label().is_empty());
            assert!(!option.valid_values.is_empty());
            assert_eq!(option.validate_value(&option.default), Ok(()));
        }
    }

    #[test]
    fn registry_rejects_duplicate_ids_names_and_config_keys() {
        let option = OptionSpec {
            id: OptionId::LineNumbers,
            name: "line_numbers",
            config_key: "line_numbers",
            summary: "Line numbers",
            doc: "Whether to show line numbers.",
            value_type: OptionType::Boolean,
            default: OptionValue::Boolean(false),
            valid_values: "true or false",
            validator: valid_boolean,
            parse_error: "expected true or false",
            validation_error: "expected true or false",
        };

        assert!(OptionRegistry::new(vec![option.clone(), option]).is_err());
    }

    #[test]
    fn option_specs_parse_and_validate_values() {
        let registry = OptionRegistry::default();

        assert_eq!(
            registry
                .get("tab_width")
                .expect("tab_width option should exist")
                .parse_value("4"),
            Ok(OptionValue::Integer(4))
        );
        assert!(
            registry
                .get("tab_width")
                .expect("tab_width option should exist")
                .parse_value("0")
                .is_err()
        );
        assert_eq!(
            registry
                .get("fill_column")
                .expect("fill_column option should exist")
                .parse_value("20"),
            Ok(OptionValue::Integer(20))
        );
        assert_eq!(
            registry
                .get("fill_column")
                .expect("fill_column option should exist")
                .parse_value("200"),
            Ok(OptionValue::Integer(200))
        );
        assert_eq!(
            registry
                .get("fill_column")
                .expect("fill_column option should exist")
                .parse_value("72"),
            Ok(OptionValue::Integer(72))
        );
        assert!(
            registry
                .get("fill_column")
                .expect("fill_column option should exist")
                .parse_value("19")
                .is_err()
        );
        assert!(
            registry
                .get("fill_column")
                .expect("fill_column option should exist")
                .parse_value("201")
                .is_err()
        );
        assert_eq!(
            registry
                .get("theme")
                .expect("theme option should exist")
                .parse_value("\"mono\""),
            Ok(OptionValue::Choice("mono"))
        );
        assert_eq!(
            registry
                .get("backup_directory")
                .expect("backup_directory option should exist")
                .parse_value("\"/tmp/rile-backups\""),
            Ok(OptionValue::String("/tmp/rile-backups".to_owned()))
        );
        assert_eq!(
            registry
                .get("auto_save_interval")
                .expect("auto_save_interval option should exist")
                .parse_value("300"),
            Ok(OptionValue::Integer(300))
        );
        assert_eq!(
            registry
                .get("auto_save_directory")
                .expect("auto_save_directory option should exist")
                .parse_value("\"/tmp/rile-auto-save\""),
            Ok(OptionValue::String("/tmp/rile-auto-save".to_owned()))
        );
    }

    #[test]
    fn option_specs_return_validation_messages() {
        let registry = OptionRegistry::default();
        let cases = [
            ("line_numbers", "yes", "expected true or false"),
            ("tab_width", "wide", "tab_width must be an integer"),
            ("tab_width", "0", "tab_width must be between 1 and 16"),
            ("fill_column", "wide", "fill_column must be an integer"),
            (
                "fill_column",
                "19",
                "fill_column must be between 20 and 200",
            ),
            (
                "theme",
                "\"solarized\"",
                "theme must be `default` or `mono`",
            ),
            (
                "completion_max_candidates",
                "0",
                "completion_max_candidates must be between 1 and 20",
            ),
            (
                "completion_matching",
                "\"fuzzy\"",
                "completion_matching must be `orderless`, `prefix`, or `substring`",
            ),
        ];

        for (name, value, message) in cases {
            let option = registry.get(name).expect("option should exist");
            assert_eq!(option.parse_value(value), Err(message), "{name}");
        }
    }
}
