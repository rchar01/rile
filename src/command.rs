// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    BackwardChar,
    BeginningOfLine,
    DeleteBackwardChar,
    DeleteChar,
    EndOfLine,
    ExecuteExtendedCommand,
    FindFile,
    ForwardChar,
    IncrementalSearchBackward,
    IncrementalSearchForward,
    NextLine,
    PreviousLine,
    SaveBuffer,
    SaveBuffersKillTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub interactive: bool,
    pub command: Command,
}

impl CommandSpec {
    pub const fn new(
        name: &'static str,
        description: &'static str,
        interactive: bool,
        command: Command,
    ) -> Self {
        Self {
            name,
            description,
            interactive,
            command,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: Vec<CommandSpec>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new(default_commands())
    }
}

impl CommandRegistry {
    pub fn new(commands: impl Into<Vec<CommandSpec>>) -> Self {
        Self {
            commands: commands.into(),
        }
    }

    pub fn get(&self, name: &str) -> Option<CommandSpec> {
        self.commands
            .iter()
            .copied()
            .find(|command| command.name == name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }
}

pub fn default_commands() -> Vec<CommandSpec> {
    use Command::*;

    vec![
        CommandSpec::new("backward-char", "Move cursor left", true, BackwardChar),
        CommandSpec::new(
            "beginning-of-line",
            "Move cursor to beginning of line",
            true,
            BeginningOfLine,
        ),
        CommandSpec::new(
            "delete-backward-char",
            "Delete character before cursor",
            true,
            DeleteBackwardChar,
        ),
        CommandSpec::new(
            "delete-char",
            "Delete character at cursor",
            true,
            DeleteChar,
        ),
        CommandSpec::new("end-of-line", "Move cursor to end of line", true, EndOfLine),
        CommandSpec::new(
            "execute-extended-command",
            "Run command by exact name",
            true,
            ExecuteExtendedCommand,
        ),
        CommandSpec::new("find-file", "Open file by path", true, FindFile),
        CommandSpec::new("forward-char", "Move cursor right", true, ForwardChar),
        CommandSpec::new(
            "isearch-backward",
            "Search backward incrementally",
            true,
            IncrementalSearchBackward,
        ),
        CommandSpec::new(
            "isearch-forward",
            "Search forward incrementally",
            true,
            IncrementalSearchForward,
        ),
        CommandSpec::new("next-line", "Move cursor down", true, NextLine),
        CommandSpec::new("previous-line", "Move cursor up", true, PreviousLine),
        CommandSpec::new("save-buffer", "Save current buffer", true, SaveBuffer),
        CommandSpec::new(
            "save-buffers-kill-terminal",
            "Quit Rile",
            true,
            SaveBuffersKillTerminal,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandRegistry};

    #[test]
    fn default_registry_resolves_exact_command_names() {
        let registry = CommandRegistry::default();

        assert_eq!(
            registry.get("save-buffer").map(|spec| spec.command),
            Some(Command::SaveBuffer)
        );
        assert!(registry.contains("execute-extended-command"));
        assert!(registry.contains("find-file"));
        assert!(registry.contains("isearch-forward"));
        assert!(registry.contains("isearch-backward"));
        assert!(!registry.contains("save"));
    }
}
