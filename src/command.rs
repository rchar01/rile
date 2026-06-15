// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    BackToIndentation,
    BackwardChar,
    BackwardKillWord,
    BackwardWord,
    BeginningOfBuffer,
    BeginningOfLine,
    CallLastKeyboardMacro,
    CopyRegionAsKill,
    DeleteBackwardChar,
    DeleteChar,
    DeleteOtherWindows,
    DeleteWindow,
    DescribeFunction,
    DescribeKey,
    EndKeyboardMacro,
    EndOfBuffer,
    EndOfLine,
    ExchangePointAndMark,
    ExecuteExtendedCommand,
    FindFile,
    FindFileReadOnly,
    ForwardChar,
    ForwardWord,
    GotoLine,
    IncrementalSearchBackward,
    IncrementalSearchForward,
    InsertFile,
    JoinLine,
    ListBuffers,
    KillLine,
    KillBuffer,
    KillRegion,
    KillWord,
    MarkWholeBuffer,
    NextLine,
    OpenLine,
    PreviousLine,
    QuotedInsert,
    QueryReplace,
    Recenter,
    SaveBuffer,
    SaveBuffersKillTerminal,
    SetMarkCommand,
    StartKeyboardMacro,
    OtherWindow,
    SplitWindowBelow,
    SplitWindowRight,
    SwitchToBuffer,
    ScrollPageBackward,
    ScrollPageForward,
    ToggleLineNumbers,
    ToggleReadOnly,
    ToggleSearchHighlighting,
    ToggleSyntaxHighlighting,
    Undo,
    UniversalArgument,
    WriteFile,
    Yank,
    YankPop,
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
        CommandSpec::new(
            "back-to-indentation",
            "Move cursor to first non-whitespace character on line",
            true,
            BackToIndentation,
        ),
        CommandSpec::new("backward-char", "Move cursor left", true, BackwardChar),
        CommandSpec::new(
            "backward-kill-word",
            "Kill word before cursor",
            true,
            BackwardKillWord,
        ),
        CommandSpec::new(
            "backward-word",
            "Move cursor backward by word",
            true,
            BackwardWord,
        ),
        CommandSpec::new(
            "beginning-of-buffer",
            "Move cursor to beginning of buffer",
            true,
            BeginningOfBuffer,
        ),
        CommandSpec::new(
            "beginning-of-line",
            "Move cursor to beginning of line",
            true,
            BeginningOfLine,
        ),
        CommandSpec::new(
            "call-last-kbd-macro",
            "Execute the last keyboard macro",
            true,
            CallLastKeyboardMacro,
        ),
        CommandSpec::new(
            "copy-region-as-kill",
            "Copy active region to kill ring",
            true,
            CopyRegionAsKill,
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
        CommandSpec::new(
            "delete-other-windows",
            "Delete all other windows",
            true,
            DeleteOtherWindows,
        ),
        CommandSpec::new("delete-window", "Delete current window", true, DeleteWindow),
        CommandSpec::new(
            "describe-function",
            "Describe an interactive command",
            true,
            DescribeFunction,
        ),
        CommandSpec::new("describe-key", "Describe a key binding", true, DescribeKey),
        CommandSpec::new(
            "end-kbd-macro",
            "Finish defining a keyboard macro",
            true,
            EndKeyboardMacro,
        ),
        CommandSpec::new(
            "end-of-buffer",
            "Move cursor to end of buffer",
            true,
            EndOfBuffer,
        ),
        CommandSpec::new("end-of-line", "Move cursor to end of line", true, EndOfLine),
        CommandSpec::new(
            "exchange-point-and-mark",
            "Exchange cursor and mark",
            true,
            ExchangePointAndMark,
        ),
        CommandSpec::new(
            "execute-extended-command",
            "Run command by name",
            true,
            ExecuteExtendedCommand,
        ),
        CommandSpec::new("find-file", "Open file by path", true, FindFile),
        CommandSpec::new(
            "find-file-read-only",
            "Open file read-only by path",
            true,
            FindFileReadOnly,
        ),
        CommandSpec::new("forward-char", "Move cursor right", true, ForwardChar),
        CommandSpec::new(
            "forward-word",
            "Move cursor forward by word",
            true,
            ForwardWord,
        ),
        CommandSpec::new("goto-line", "Go to line or line:column", true, GotoLine),
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
        CommandSpec::new(
            "insert-file",
            "Insert file contents at point",
            true,
            InsertFile,
        ),
        CommandSpec::new(
            "join-line",
            "Join current line to previous line",
            true,
            JoinLine,
        ),
        CommandSpec::new("list-buffers", "List active buffers", true, ListBuffers),
        CommandSpec::new("kill-buffer", "Kill a buffer by name", true, KillBuffer),
        CommandSpec::new("kill-line", "Kill text to end of line", true, KillLine),
        CommandSpec::new("kill-region", "Kill active region", true, KillRegion),
        CommandSpec::new("kill-word", "Kill word after cursor", true, KillWord),
        CommandSpec::new(
            "mark-whole-buffer",
            "Mark the whole buffer",
            true,
            MarkWholeBuffer,
        ),
        CommandSpec::new("next-line", "Move cursor down", true, NextLine),
        CommandSpec::new("open-line", "Insert newline after point", true, OpenLine),
        CommandSpec::new("other-window", "Select next window", true, OtherWindow),
        CommandSpec::new("previous-line", "Move cursor up", true, PreviousLine),
        CommandSpec::new(
            "quoted-insert",
            "Insert the next key literally",
            true,
            QuotedInsert,
        ),
        CommandSpec::new(
            "query-replace",
            "Interactively replace text",
            true,
            QueryReplace,
        ),
        CommandSpec::new("recenter", "Center cursor in window", true, Recenter),
        CommandSpec::new("save-buffer", "Save current buffer", true, SaveBuffer),
        CommandSpec::new(
            "scroll-page-backward",
            "Scroll one page backward",
            true,
            ScrollPageBackward,
        ),
        CommandSpec::new(
            "scroll-page-forward",
            "Scroll one page forward",
            true,
            ScrollPageForward,
        ),
        CommandSpec::new(
            "save-buffers-kill-terminal",
            "Quit Rile",
            true,
            SaveBuffersKillTerminal,
        ),
        CommandSpec::new(
            "set-mark-command",
            "Set mark at point",
            true,
            SetMarkCommand,
        ),
        CommandSpec::new(
            "start-kbd-macro",
            "Start defining a keyboard macro",
            true,
            StartKeyboardMacro,
        ),
        CommandSpec::new(
            "split-window-below",
            "Split current window horizontally",
            true,
            SplitWindowBelow,
        ),
        CommandSpec::new(
            "split-window-right",
            "Split current window vertically",
            true,
            SplitWindowRight,
        ),
        CommandSpec::new(
            "switch-to-buffer",
            "Switch to a buffer by name",
            true,
            SwitchToBuffer,
        ),
        CommandSpec::new(
            "toggle-line-numbers",
            "Toggle line numbers",
            true,
            ToggleLineNumbers,
        ),
        CommandSpec::new(
            "toggle-read-only",
            "Toggle buffer read-only state",
            true,
            ToggleReadOnly,
        ),
        CommandSpec::new(
            "toggle-search-highlighting",
            "Toggle search highlighting",
            true,
            ToggleSearchHighlighting,
        ),
        CommandSpec::new(
            "toggle-syntax-highlighting",
            "Toggle syntax highlighting",
            true,
            ToggleSyntaxHighlighting,
        ),
        CommandSpec::new("undo", "Undo last edit", true, Undo),
        CommandSpec::new(
            "universal-argument",
            "Set a numeric argument for the next command",
            true,
            UniversalArgument,
        ),
        CommandSpec::new("write-file", "Write buffer to a new path", true, WriteFile),
        CommandSpec::new("yank", "Insert latest kill", true, Yank),
        CommandSpec::new("yank-pop", "Rotate the just-yanked kill", true, YankPop),
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
        assert!(registry.contains("back-to-indentation"));
        assert!(registry.contains("beginning-of-buffer"));
        assert!(registry.contains("backward-kill-word"));
        assert!(registry.contains("backward-word"));
        assert!(registry.contains("call-last-kbd-macro"));
        assert!(registry.contains("end-of-buffer"));
        assert!(registry.contains("end-kbd-macro"));
        assert!(registry.contains("exchange-point-and-mark"));
        assert!(registry.contains("execute-extended-command"));
        assert!(registry.contains("find-file"));
        assert!(registry.contains("find-file-read-only"));
        assert!(registry.contains("forward-word"));
        assert!(registry.contains("goto-line"));
        assert!(registry.contains("isearch-forward"));
        assert!(registry.contains("isearch-backward"));
        assert!(registry.contains("insert-file"));
        assert!(registry.contains("join-line"));
        assert!(registry.contains("list-buffers"));
        assert!(registry.contains("quoted-insert"));
        assert!(registry.contains("switch-to-buffer"));
        assert!(registry.contains("kill-buffer"));
        assert!(registry.contains("kill-line"));
        assert!(registry.contains("kill-region"));
        assert!(registry.contains("kill-word"));
        assert!(registry.contains("mark-whole-buffer"));
        assert!(registry.contains("copy-region-as-kill"));
        assert!(registry.contains("yank"));
        assert!(registry.contains("yank-pop"));
        assert!(registry.contains("undo"));
        assert!(registry.contains("universal-argument"));
        assert!(registry.contains("query-replace"));
        assert!(registry.contains("recenter"));
        assert!(registry.contains("scroll-page-backward"));
        assert!(registry.contains("scroll-page-forward"));
        assert!(registry.contains("set-mark-command"));
        assert!(registry.contains("start-kbd-macro"));
        assert!(registry.contains("toggle-line-numbers"));
        assert!(registry.contains("toggle-read-only"));
        assert!(registry.contains("toggle-search-highlighting"));
        assert!(registry.contains("toggle-syntax-highlighting"));
        assert!(registry.contains("write-file"));
        assert!(registry.contains("split-window-below"));
        assert!(registry.contains("split-window-right"));
        assert!(registry.contains("delete-window"));
        assert!(registry.contains("delete-other-windows"));
        assert!(registry.contains("describe-function"));
        assert!(registry.contains("describe-key"));
        assert!(registry.contains("other-window"));
        assert!(!registry.contains("save"));
    }
}
