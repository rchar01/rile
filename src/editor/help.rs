// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::command::{Command, CommandRegistry, CommandSpec};
use crate::input::KeyEvent;
use crate::keymap::{
    KeyBinding, KeyMap, KeyMapId, KeyMapStack, KeyStackResolution, format_key_sequence,
};
use crate::mode::{ModeId, ModeRegistry, ModeSpec};
use crate::option::{OptionSpec, OptionValue};

use super::{AboutRileInfo, ActiveModes, BufferDescription};

pub(super) const HELP_FILL_WIDTH: usize = 70;

pub(super) fn format_key_prefix_help(
    commands: &CommandRegistry,
    keymaps: &KeyMapStack<'_>,
    prefix: &[KeyEvent],
) -> String {
    let title = if keymaps.maps().len() == 1 && keymaps.maps()[0].id() == KeyMapId::Global {
        "Global Bindings"
    } else {
        "Active Bindings"
    };
    let mut text = String::new();
    append_help_heading(
        &mut text,
        &format!("{title} Starting With {}:", format_key_sequence(prefix)),
    );
    append_key_table_header(&mut text);
    text.push('\n');

    for binding in keymaps.bindings_starting_with(prefix) {
        let command = commands.get_by_id(binding.binding.command());
        let name = command.map(|command| command.name).unwrap_or("<unknown>");
        let description = command.map(|command| command.summary).unwrap_or("");
        append_key_table_row(
            &mut text,
            &format_key_sequence(&binding.binding.sequence),
            name,
            description,
        );
    }

    text
}

pub(super) fn format_describe_bindings_help(
    commands: &CommandRegistry,
    keymaps: &KeyMapStack<'_>,
) -> String {
    let mut text = String::new();
    append_help_heading(&mut text, "Active Key Bindings:");
    text.push_str("Keymap Stack:\n");
    for keymap in keymaps.maps() {
        text.push_str(&format!("- {}\n", keymap.name()));
    }
    text.push('\n');

    for keymap in keymaps.maps() {
        append_help_heading(&mut text, &format!("{}:", keymap.name()));
        append_key_table_header(&mut text);

        let mut has_bindings = false;
        for binding in keymap.bindings_starting_with(&[]) {
            has_bindings = true;
            let command = commands.get_by_id(binding.command());
            let name = command.map(|command| command.name).unwrap_or("<unknown>");
            let mut description = command
                .map(|command| command.summary)
                .unwrap_or("")
                .to_owned();
            if let Some(shadowed_by) = describe_shadowing(commands, keymaps, keymap, binding) {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(&shadowed_by);
            }
            append_key_table_row(
                &mut text,
                &format_key_sequence(&binding.sequence),
                name,
                &description,
            );
        }
        if !has_bindings {
            text.push_str("(No active bindings)\n");
        }
        text.push('\n');
    }

    text
}

fn describe_shadowing(
    commands: &CommandRegistry,
    keymaps: &KeyMapStack<'_>,
    keymap: &KeyMap,
    binding: &KeyBinding,
) -> Option<String> {
    match keymaps.resolve(&binding.sequence) {
        KeyStackResolution::Command {
            keymap: resolved_keymap,
            command,
        } if resolved_keymap == keymap.id() && command == binding.command() => None,
        KeyStackResolution::Command {
            keymap: resolved_keymap,
            command,
        } => {
            let keymap_name = keymaps.keymap_name(resolved_keymap).unwrap_or("<unknown>");
            let command_name = commands
                .get_by_id(command)
                .map(|command| command.name)
                .unwrap_or("<unknown>");
            Some(format!("(shadowed by {keymap_name} {command_name})"))
        }
        KeyStackResolution::Prefix => Some("(shadowed by higher-priority prefix)".to_owned()),
        KeyStackResolution::NoMatch => Some("(shadowed)".to_owned()),
    }
}

pub(super) fn format_describe_key_help(
    commands: &CommandRegistry,
    keymaps: &KeyMapStack<'_>,
    sequence: &[KeyEvent],
    keymap: KeyMapId,
    command: Command,
) -> String {
    let command = commands.get_by_id(command);
    let name = command.map(|command| command.name).unwrap_or("<unknown>");
    let source = keymaps
        .keymap_name(keymap)
        .map(|keymap_name| format!(" (found in {keymap_name})"))
        .unwrap_or_default();
    let mut text = format!(
        "{} runs the command {}{}.\n\n",
        format_key_sequence(sequence),
        name,
        source
    );
    append_shadowed_key_bindings(&mut text, commands, keymaps, sequence, keymap);
    if command.is_some() {
        append_command_help_body(&mut text, keymaps, command);
    }
    text
}

fn append_shadowed_key_bindings(
    text: &mut String,
    commands: &CommandRegistry,
    keymaps: &KeyMapStack<'_>,
    sequence: &[KeyEvent],
    source_keymap: KeyMapId,
) {
    let mut found_source = false;
    let mut rows = Vec::new();

    for keymap in keymaps.maps() {
        if keymap.id() == source_keymap {
            found_source = true;
            continue;
        }
        if !found_source {
            continue;
        }
        if let Some(binding) = keymap.binding_for_sequence(sequence) {
            let command_name = commands
                .get_by_id(binding.command())
                .map(|command| command.name)
                .unwrap_or("<unknown>");
            rows.push(format!("- {}: {}\n", keymap.name(), command_name));
        }
    }

    if !rows.is_empty() {
        text.push_str("Shadowed lower-priority bindings:\n");
        for row in rows {
            text.push_str(&row);
        }
        text.push('\n');
    }
}

pub(super) fn format_describe_key_brief_message(
    commands: &CommandRegistry,
    sequence: &[KeyEvent],
    command: Command,
) -> String {
    let name = commands
        .get_by_id(command)
        .map(|command| command.name)
        .unwrap_or("<unknown>");
    format!(
        "{} runs the command `{}`.",
        format_key_sequence(sequence),
        name
    )
}

pub(super) fn format_describe_function_help(
    keymaps: &KeyMapStack<'_>,
    command: CommandSpec,
) -> String {
    format_command_help(keymaps, Some(command), command.name)
}

pub(super) fn format_describe_variable_help(
    option: &OptionSpec,
    current_value: OptionValue,
) -> String {
    let mut text = String::new();
    append_help_heading(
        &mut text,
        &format!("{} is a configuration variable.", option.name),
    );
    append_option_field(&mut text, "Name", option.name);
    append_option_field(&mut text, "Config key", option.config_key);
    append_option_field(&mut text, "Current value", current_value);
    append_option_field(&mut text, "Default value", option.default);
    append_option_field(&mut text, "Type", option.value_type.label());
    append_option_field(&mut text, "Valid values", option.valid_values);
    append_option_field(&mut text, "Summary", option.summary);
    text.push('\n');
    append_wrapped_prose(&mut text, option.doc);
    text
}

pub(super) fn format_about_rile_help(info: &AboutRileInfo) -> String {
    let mut text = String::new();
    append_help_heading(&mut text, "About Rile:");
    append_option_field(&mut text, "Version", info.version);
    append_option_field(&mut text, "Build profile", info.build_profile);
    append_option_field(&mut text, "Enabled features", info.enabled_features);
    append_option_field(&mut text, "Terminal backend", info.terminal_backend);
    append_option_field(
        &mut text,
        "Config path",
        info.config_path.as_deref().unwrap_or("none"),
    );
    append_option_field(
        &mut text,
        "Current directory",
        info.current_directory.as_deref().unwrap_or("unknown"),
    );
    text.push('\n');
    append_wrapped_prose(
        &mut text,
        "Rile is a small UTF-8-capable terminal editor with Emacs-style key bindings. Runtime diagnostics that users can act on are reported through the echo area and can be reviewed with C-h e.",
    );
    text
}

pub(super) fn format_describe_mode_help(modes: &ActiveModes, registry: &ModeRegistry) -> String {
    let mut text = String::new();
    append_help_heading(&mut text, "Active Modes:");
    append_option_field(&mut text, "Major mode", mode_name(registry, modes.major));
    append_option_field(&mut text, "Syntax mode", mode_name(registry, modes.syntax));
    append_option_field(&mut text, "Minor modes", mode_list(registry, &modes.minor));
    append_option_field(
        &mut text,
        "Special buffer mode",
        modes
            .special
            .map(|mode| mode_name(registry, mode))
            .unwrap_or("none"),
    );
    text.push('\n');
    append_help_heading(&mut text, "Mode Details:");

    let mut detail_ids = vec![modes.major, modes.syntax];
    detail_ids.extend(modes.minor.iter().copied());
    if let Some(special) = modes.special {
        detail_ids.push(special);
    }
    for id in detail_ids {
        append_mode_spec_help(&mut text, required_mode(registry, id));
    }
    text
}

pub(super) fn format_describe_buffer_help(
    description: &BufferDescription,
    registry: &ModeRegistry,
) -> String {
    let mut text = String::new();
    append_help_heading(
        &mut text,
        &format!("{} is the current buffer.", description.name),
    );
    append_option_field(&mut text, "Name", description.name.as_str());
    append_option_field(
        &mut text,
        "Path",
        description.path.as_deref().unwrap_or("none"),
    );
    append_option_field(&mut text, "Kind", description.kind);
    append_option_field(&mut text, "Modified", yes_no(description.modified));
    append_option_field(&mut text, "Read only", yes_no(description.read_only));
    append_option_field(
        &mut text,
        "Point",
        format!(
            "line {}, column {}",
            description.point_line, description.point_column
        ),
    );
    append_option_field(&mut text, "Encoding", description.encoding);
    append_option_field(&mut text, "Line ending", description.line_ending);
    append_option_field(
        &mut text,
        "Final newline",
        yes_no(description.final_newline),
    );
    append_option_field(
        &mut text,
        "Major mode",
        mode_name(registry, description.modes.major),
    );
    append_option_field(
        &mut text,
        "Syntax mode",
        mode_name(registry, description.modes.syntax),
    );
    append_option_field(
        &mut text,
        "Minor modes",
        mode_list(registry, &description.modes.minor),
    );
    append_option_field(
        &mut text,
        "Special buffer mode",
        description
            .modes
            .special
            .map(|mode| mode_name(registry, mode))
            .unwrap_or("none"),
    );
    text
}

fn append_mode_spec_help(text: &mut String, mode: &ModeSpec) {
    append_help_heading(text, &format!("{}:", mode.name));
    append_option_field(text, "Name", mode.name);
    append_option_field(text, "Kind", mode.kind.label());
    append_option_field(text, "Summary", mode.summary);
    append_option_field(text, "Keymap", mode.keymap.unwrap_or("none"));
    text.push('\n');
    append_wrapped_prose(text, mode.doc);
    text.push('\n');
}

fn mode_name(registry: &ModeRegistry, id: ModeId) -> &'static str {
    required_mode(registry, id).name
}

fn mode_list(registry: &ModeRegistry, ids: &[ModeId]) -> String {
    if ids.is_empty() {
        return "none".to_owned();
    }
    ids.iter()
        .map(|id| mode_name(registry, *id))
        .collect::<Vec<_>>()
        .join(", ")
}

fn required_mode(registry: &ModeRegistry, id: ModeId) -> &ModeSpec {
    registry
        .get(id)
        .expect("active mode should be present in default mode registry")
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(super) fn format_unbound_key_help(sequence: &[KeyEvent]) -> String {
    format!("{} is undefined.\n", format_key_sequence(sequence))
}

pub(super) fn format_unbound_key_message(sequence: &[KeyEvent]) -> String {
    format!(
        "{} is not bound to any command.",
        format_key_sequence(sequence)
    )
}

fn format_command_help(
    keymaps: &KeyMapStack<'_>,
    command: Option<CommandSpec>,
    name: &str,
) -> String {
    let mut text = match command {
        Some(command) => format!("{} is an interactive command.\n\n", command.name),
        None => return format!("{} is not a known interactive command.\n", name),
    };
    append_command_help_body(&mut text, keymaps, command);
    text
}

fn append_command_help_body(
    text: &mut String,
    keymaps: &KeyMapStack<'_>,
    command: Option<CommandSpec>,
) {
    let bindings = command
        .map(|command| keymaps.bindings_for_command(command.command))
        .unwrap_or_default();
    if bindings.is_empty() {
        text.push_str("It is not bound to any key.\n\n");
    } else {
        let keys = bindings
            .iter()
            .map(|binding| format_key_sequence(&binding.binding.sequence))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!("It is bound to {}.\n\n", keys));
    }
    if let Some(command) = command {
        text.push_str(command.summary);
        text.push_str("\n\n");
        append_wrapped_prose(text, command.doc);
    }
}

fn append_help_heading(text: &mut String, heading: &str) {
    text.push_str(heading);
    text.push_str("\n\n");
}

fn append_key_table_header(text: &mut String) {
    text.push_str("Key             Binding                        Description\n");
    text.push_str("---             -------                        -----------\n");
}

fn append_key_table_row(text: &mut String, key: &str, binding: &str, description: &str) {
    text.push_str(&format!("{key:<15} {binding:<30} {description}\n"));
}

fn append_option_field(text: &mut String, label: &str, value: impl std::fmt::Display) {
    text.push_str(&format!("{label}: {value}\n"));
}

pub(super) fn append_wrapped_prose(text: &mut String, prose: &str) {
    for (index, block) in prose.split("\n\n").enumerate() {
        if index > 0 {
            text.push('\n');
        }
        if is_preformatted_help_block(block) {
            text.push_str(block.trim_end());
            text.push('\n');
        } else {
            append_wrapped_paragraph(text, block);
        }
    }
}

fn is_preformatted_help_block(block: &str) -> bool {
    block.lines().any(|line| {
        line.starts_with(' ')
            || line.starts_with('\t')
            || line.contains('|')
            || line.trim_start().starts_with("---")
    })
}

fn append_wrapped_paragraph(text: &mut String, paragraph: &str) {
    let mut line = String::new();
    for word in paragraph.split_whitespace() {
        let next_len = if line.is_empty() {
            word.len()
        } else {
            line.len() + 1 + word.len()
        };
        if next_len > HELP_FILL_WIDTH && !line.is_empty() {
            text.push_str(&line);
            text.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        text.push_str(&line);
        text.push('\n');
    }
}
