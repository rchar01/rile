// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::input::KeyEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    BackToIndentation,
    BackwardChar,
    BackwardKillWord,
    BackwardWord,
    BeginningOfBuffer,
    BeginningOfLine,
    CallLastKeyboardMacro,
    ClearRectangle,
    CopyRegionAsKill,
    CopyRectangleAsKill,
    CopyRectangleToRegister,
    CopyToRegister,
    DeleteBackwardChar,
    DeleteChar,
    DeleteRectangle,
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
    InsertRegister,
    IncrementRegister,
    JoinLine,
    JumpToRegister,
    ListBuffers,
    NewlineAndIndent,
    KillLine,
    KillBuffer,
    KillRegion,
    KillRectangle,
    KillWord,
    MarkWholeBuffer,
    NextLine,
    NumberToRegister,
    OpenLine,
    OpenRectangle,
    PreviousLine,
    PointToRegister,
    QuotedInsert,
    QueryReplace,
    RectangleMarkMode,
    RectangleNumberLines,
    Recenter,
    SaveBuffer,
    SaveBuffersKillTerminal,
    SetMarkCommand,
    ShellCommand,
    ShellCommandOnRegion,
    StartKeyboardMacro,
    StringRectangle,
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
    ViewEchoAreaMessages,
    WriteFile,
    Yank,
    YankRectangle,
    YankPop,
}

pub type CommandId = Command;

pub type CommandHandler =
    fn(&mut crate::editor::Editor, CommandContext) -> crate::Result<CommandOutcome>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContext {
    pub argument: Option<i32>,
    pub invoked_by: Invocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Key(Vec<KeyEvent>),
    ExtendedCommand,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOutcome {
    Continue,
    Exit,
    StartedPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Movement,
    Editing,
    Files,
    Buffers,
    Windows,
    Search,
    Shell,
    Registers,
    Rectangles,
    Macros,
    Commands,
    Help,
    Configuration,
    System,
}

impl CommandCategory {
    const fn for_command(command: Command) -> Self {
        use Command::*;

        match command {
            BackToIndentation | BackwardChar | BackwardWord | BeginningOfBuffer
            | BeginningOfLine | EndOfBuffer | EndOfLine | ForwardChar | ForwardWord | GotoLine
            | NextLine | PreviousLine | Recenter | ScrollPageBackward | ScrollPageForward => {
                Self::Movement
            }
            BackwardKillWord | CopyRegionAsKill | DeleteBackwardChar | DeleteChar
            | ExchangePointAndMark | JoinLine | KillLine | KillRegion | KillWord
            | MarkWholeBuffer | NewlineAndIndent | OpenLine | QuotedInsert | SetMarkCommand
            | Undo | Yank | YankPop => Self::Editing,
            FindFile | FindFileReadOnly | InsertFile | SaveBuffer | WriteFile => Self::Files,
            KillBuffer | ListBuffers | SwitchToBuffer => Self::Buffers,
            DeleteOtherWindows | DeleteWindow | OtherWindow | SplitWindowBelow
            | SplitWindowRight => Self::Windows,
            IncrementalSearchBackward | IncrementalSearchForward | QueryReplace => Self::Search,
            ShellCommand | ShellCommandOnRegion => Self::Shell,
            CopyRectangleToRegister
            | CopyToRegister
            | IncrementRegister
            | InsertRegister
            | JumpToRegister
            | NumberToRegister
            | PointToRegister => Self::Registers,
            ClearRectangle | CopyRectangleAsKill | DeleteRectangle | KillRectangle
            | OpenRectangle | RectangleMarkMode | RectangleNumberLines | StringRectangle
            | YankRectangle => Self::Rectangles,
            CallLastKeyboardMacro | EndKeyboardMacro | StartKeyboardMacro => Self::Macros,
            ExecuteExtendedCommand | UniversalArgument => Self::Commands,
            DescribeFunction | DescribeKey | ViewEchoAreaMessages => Self::Help,
            ToggleLineNumbers
            | ToggleReadOnly
            | ToggleSearchHighlighting
            | ToggleSyntaxHighlighting => Self::Configuration,
            SaveBuffersKillTerminal => Self::System,
        }
    }
}

const fn default_doc_for_command(command: CommandId) -> &'static str {
    use Command::*;

    match command {
        BackToIndentation => {
            "Move point to the first non-whitespace character on the current line."
        }
        BackwardChar => "Move point one character toward the beginning of the buffer.",
        BackwardKillWord => "Kill the word before point and save the killed text in the kill ring.",
        BackwardWord => "Move point backward by one word.",
        BeginningOfBuffer => "Move point to the beginning of the current buffer.",
        BeginningOfLine => "Move point to the beginning of the current line.",
        CallLastKeyboardMacro => "Replay the most recently recorded keyboard macro.",
        ClearRectangle => "Replace the active rectangle contents with spaces.",
        CopyRegionAsKill => "Copy the active region to the kill ring without deleting it.",
        CopyRectangleAsKill => "Copy the active rectangle to the rectangle kill ring.",
        CopyRectangleToRegister => "Copy the active rectangle into a prompted register.",
        CopyToRegister => "Copy the active region text into a prompted register.",
        DeleteBackwardChar => "Delete the character immediately before point.",
        DeleteChar => "Delete the character at point.",
        DeleteRectangle => "Delete the active rectangle without saving it to the kill ring.",
        DeleteOtherWindows => "Delete every window except the selected window.",
        DeleteWindow => "Delete the selected window when another window is available.",
        DescribeFunction => "Prompt for an interactive command and show its help buffer.",
        DescribeKey => "Read a key sequence and describe the command bound to it.",
        EndKeyboardMacro => "Finish recording the current keyboard macro.",
        EndOfBuffer => "Move point to the end of the current buffer.",
        EndOfLine => "Move point to the end of the current line.",
        ExchangePointAndMark => "Swap point with mark and reactivate the selected region.",
        ExecuteExtendedCommand => "Prompt for an interactive command name and run it.",
        FindFile => "Prompt for a file path and open it for editing.",
        FindFileReadOnly => "Prompt for a file path and open it read-only.",
        ForwardChar => "Move point one character toward the end of the buffer.",
        ForwardWord => "Move point forward by one word.",
        GotoLine => "Prompt for a line or line:column location and move point there.",
        IncrementalSearchBackward => "Start backward incremental search from point.",
        IncrementalSearchForward => "Start forward incremental search from point.",
        InsertFile => "Prompt for a file path and insert its contents at point.",
        InsertRegister => "Insert the text, rectangle, or number stored in a register.",
        IncrementRegister => "Add the numeric argument to a prompted number register.",
        JoinLine => "Join the current line to the previous line, trimming surrounding space.",
        JumpToRegister => "Move point to the position stored in a prompted register.",
        ListBuffers => "Show a read-only buffer list in another window.",
        NewlineAndIndent => "Insert a newline using the current plain-text indentation policy.",
        KillLine => "Kill text from point to the line end, or kill the line break at EOL.",
        KillBuffer => "Prompt for a buffer name and kill the selected buffer.",
        KillRegion => "Kill the active region and save it in the kill ring.",
        KillRectangle => "Kill the active rectangle and save it for rectangle yanking.",
        KillWord => "Kill the word after point and save it in the kill ring.",
        MarkWholeBuffer => "Set point at buffer start and mark at buffer end.",
        NextLine => "Move point to the next visual line, preserving the goal column.",
        NumberToRegister => "Store the numeric argument in a prompted register.",
        OpenLine => "Insert a newline at point without moving point.",
        OpenRectangle => "Open blank columns across the active rectangle.",
        PreviousLine => "Move point to the previous visual line, preserving the goal column.",
        PointToRegister => "Store the current buffer and point in a prompted register.",
        QuotedInsert => "Read the next key and insert it literally when supported.",
        QueryReplace => "Prompt for search and replacement strings and replace interactively.",
        RectangleMarkMode => "Activate rectangle mark mode for column-oriented region commands.",
        RectangleNumberLines => "Insert formatted line numbers down the active rectangle.",
        Recenter => "Scroll the selected window so point is near the vertical center.",
        SaveBuffer => "Write the current file-backed buffer to disk.",
        SaveBuffersKillTerminal => "Quit Rile, prompting before exit when buffers are modified.",
        SetMarkCommand => "Set mark at point and activate the region.",
        ShellCommand => "Prompt for a shell command and display or insert its output.",
        ShellCommandOnRegion => "Run a shell command with the active region on standard input.",
        StartKeyboardMacro => "Begin recording subsequent input as a keyboard macro.",
        StringRectangle => "Replace each line of the active rectangle with prompted text.",
        OtherWindow => "Select the next window in the current frame layout.",
        SplitWindowBelow => "Split the selected window into upper and lower panes.",
        SplitWindowRight => "Split the selected window into left and right panes.",
        SwitchToBuffer => "Prompt for a buffer name and switch the selected window to it.",
        ScrollPageBackward => "Scroll backward by one visible page with overlap.",
        ScrollPageForward => "Scroll forward by one visible page with overlap.",
        ToggleLineNumbers => "Toggle line-number display for editor windows.",
        ToggleReadOnly => "Toggle read-only state for the current normal buffer.",
        ToggleSearchHighlighting => "Toggle visual highlights for search and query replace.",
        ToggleSyntaxHighlighting => "Toggle syntax highlighting for supported modes.",
        Undo => "Undo the latest edit recorded for the current buffer.",
        UniversalArgument => "Set or extend the numeric argument for the next command.",
        ViewEchoAreaMessages => "Open the read-only message history buffer.",
        WriteFile => "Prompt for a path and write the current buffer there.",
        Yank => "Insert the latest kill-ring entry at point.",
        YankRectangle => "Insert the latest killed rectangle at point.",
        YankPop => "Replace the just-yanked text with an earlier kill-ring entry.",
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub command: CommandId,
    pub name: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub category: CommandCategory,
    pub interactive: bool,
    pub handler: Option<CommandHandler>,
}

impl CommandSpec {
    pub const fn new(
        name: &'static str,
        summary: &'static str,
        interactive: bool,
        command: CommandId,
    ) -> Self {
        Self {
            command,
            name,
            summary,
            doc: default_doc_for_command(command),
            category: CommandCategory::for_command(command),
            interactive,
            handler: None,
        }
    }

    pub const fn with_handler(mut self, handler: CommandHandler) -> Self {
        self.handler = Some(handler);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRegistryError {
    DuplicateId(CommandId),
    DuplicateName(&'static str),
    MissingSummary(CommandId),
    MissingDoc(CommandId),
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
        Self::try_new(commands).expect("command registry should be valid")
    }

    pub fn try_new(
        commands: impl Into<Vec<CommandSpec>>,
    ) -> std::result::Result<Self, CommandRegistryError> {
        let registry = Self {
            commands: commands.into(),
        };
        registry.validate()?;
        Ok(registry)
    }

    pub fn get(&self, name: &str) -> Option<CommandSpec> {
        self.commands
            .iter()
            .copied()
            .find(|command| command.name == name)
    }

    pub fn get_by_id(&self, id: CommandId) -> Option<CommandSpec> {
        self.commands
            .iter()
            .copied()
            .find(|command| command.command == id)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn interactive_commands(&self) -> impl Iterator<Item = CommandSpec> + '_ {
        self.commands
            .iter()
            .copied()
            .filter(|command| command.interactive)
    }

    pub fn commands_by_category(
        &self,
        category: CommandCategory,
    ) -> impl Iterator<Item = CommandSpec> + '_ {
        self.commands
            .iter()
            .copied()
            .filter(move |command| command.category == category)
    }

    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    pub fn validate(&self) -> std::result::Result<(), CommandRegistryError> {
        for (index, command) in self.commands.iter().enumerate() {
            if command.summary.trim().is_empty() {
                return Err(CommandRegistryError::MissingSummary(command.command));
            }
            if command.doc.trim().is_empty() {
                return Err(CommandRegistryError::MissingDoc(command.command));
            }
            if self.commands[index + 1..]
                .iter()
                .any(|other| other.command == command.command)
            {
                return Err(CommandRegistryError::DuplicateId(command.command));
            }
            if self.commands[index + 1..]
                .iter()
                .any(|other| other.name == command.name)
            {
                return Err(CommandRegistryError::DuplicateName(command.name));
            }
        }

        Ok(())
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
        )
        .with_handler(crate::editor::Editor::command_back_to_indentation),
        CommandSpec::new("backward-char", "Move cursor left", true, BackwardChar)
            .with_handler(crate::editor::Editor::command_backward_char),
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
        )
        .with_handler(crate::editor::Editor::command_backward_word),
        CommandSpec::new(
            "beginning-of-buffer",
            "Move cursor to beginning of buffer",
            true,
            BeginningOfBuffer,
        )
        .with_handler(crate::editor::Editor::command_beginning_of_buffer),
        CommandSpec::new(
            "beginning-of-line",
            "Move cursor to beginning of line",
            true,
            BeginningOfLine,
        )
        .with_handler(crate::editor::Editor::command_beginning_of_line),
        CommandSpec::new(
            "call-last-kbd-macro",
            "Execute the last keyboard macro",
            true,
            CallLastKeyboardMacro,
        ),
        CommandSpec::new(
            "clear-rectangle",
            "Replace rectangle contents with spaces",
            true,
            ClearRectangle,
        ),
        CommandSpec::new(
            "copy-region-as-kill",
            "Copy active region to kill ring",
            true,
            CopyRegionAsKill,
        ),
        CommandSpec::new(
            "copy-rectangle-as-kill",
            "Copy rectangle to kill ring",
            true,
            CopyRectangleAsKill,
        ),
        CommandSpec::new(
            "copy-rectangle-to-register",
            "Copy rectangle to a register",
            true,
            CopyRectangleToRegister,
        ),
        CommandSpec::new(
            "copy-to-register",
            "Copy active region to a register",
            true,
            CopyToRegister,
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
            "delete-rectangle",
            "Delete rectangle without saving it",
            true,
            DeleteRectangle,
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
        )
        .with_handler(crate::editor::Editor::command_end_of_buffer),
        CommandSpec::new("end-of-line", "Move cursor to end of line", true, EndOfLine)
            .with_handler(crate::editor::Editor::command_end_of_line),
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
        CommandSpec::new("forward-char", "Move cursor right", true, ForwardChar)
            .with_handler(crate::editor::Editor::command_forward_char),
        CommandSpec::new(
            "forward-word",
            "Move cursor forward by word",
            true,
            ForwardWord,
        )
        .with_handler(crate::editor::Editor::command_forward_word),
        CommandSpec::new("goto-line", "Go to line or line:column", true, GotoLine)
            .with_handler(crate::editor::Editor::command_goto_line),
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
            "increment-register",
            "Add numeric argument to a number register",
            true,
            IncrementRegister,
        ),
        CommandSpec::new(
            "insert-register",
            "Insert text, rectangle, or number from a register",
            true,
            InsertRegister,
        ),
        CommandSpec::new(
            "join-line",
            "Join current line to previous line",
            true,
            JoinLine,
        ),
        CommandSpec::new(
            "jump-to-register",
            "Jump to a point register",
            true,
            JumpToRegister,
        ),
        CommandSpec::new("list-buffers", "List active buffers", true, ListBuffers),
        CommandSpec::new(
            "newline-and-indent",
            "Insert newline and indent according to mode",
            true,
            NewlineAndIndent,
        ),
        CommandSpec::new("kill-buffer", "Kill a buffer by name", true, KillBuffer),
        CommandSpec::new("kill-line", "Kill text to end of line", true, KillLine),
        CommandSpec::new("kill-region", "Kill active region", true, KillRegion),
        CommandSpec::new(
            "kill-rectangle",
            "Kill rectangle to kill ring",
            true,
            KillRectangle,
        ),
        CommandSpec::new("kill-word", "Kill word after cursor", true, KillWord),
        CommandSpec::new(
            "mark-whole-buffer",
            "Mark the whole buffer",
            true,
            MarkWholeBuffer,
        ),
        CommandSpec::new("next-line", "Move cursor down", true, NextLine)
            .with_handler(crate::editor::Editor::command_next_line),
        CommandSpec::new(
            "number-to-register",
            "Store numeric argument in a register",
            true,
            NumberToRegister,
        ),
        CommandSpec::new("open-line", "Insert newline after point", true, OpenLine),
        CommandSpec::new(
            "open-rectangle",
            "Insert blank space into rectangle columns",
            true,
            OpenRectangle,
        ),
        CommandSpec::new("other-window", "Select next window", true, OtherWindow),
        CommandSpec::new("previous-line", "Move cursor up", true, PreviousLine)
            .with_handler(crate::editor::Editor::command_previous_line),
        CommandSpec::new(
            "quoted-insert",
            "Insert the next key literally",
            true,
            QuotedInsert,
        ),
        CommandSpec::new(
            "point-to-register",
            "Store point in a register",
            true,
            PointToRegister,
        ),
        CommandSpec::new(
            "query-replace",
            "Interactively replace text",
            true,
            QueryReplace,
        ),
        CommandSpec::new(
            "rectangle-mark-mode",
            "Mark a rectangular region",
            true,
            RectangleMarkMode,
        ),
        CommandSpec::new(
            "rectangle-number-lines",
            "Insert line numbers at the rectangle left edge",
            true,
            RectangleNumberLines,
        ),
        CommandSpec::new("recenter", "Center cursor in window", true, Recenter)
            .with_handler(crate::editor::Editor::command_recenter),
        CommandSpec::new("save-buffer", "Save current buffer", true, SaveBuffer),
        CommandSpec::new(
            "scroll-page-backward",
            "Scroll one page backward",
            true,
            ScrollPageBackward,
        )
        .with_handler(crate::editor::Editor::command_scroll_page_backward),
        CommandSpec::new(
            "scroll-page-forward",
            "Scroll one page forward",
            true,
            ScrollPageForward,
        )
        .with_handler(crate::editor::Editor::command_scroll_page_forward),
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
        CommandSpec::new("shell-command", "Run a shell command", true, ShellCommand),
        CommandSpec::new(
            "shell-command-on-region",
            "Run a shell command with region as input",
            true,
            ShellCommandOnRegion,
        ),
        CommandSpec::new(
            "start-kbd-macro",
            "Start defining a keyboard macro",
            true,
            StartKeyboardMacro,
        ),
        CommandSpec::new(
            "string-rectangle",
            "Replace rectangle contents with a string",
            true,
            StringRectangle,
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
        CommandSpec::new(
            "view-echo-area-messages",
            "Show the message history",
            true,
            ViewEchoAreaMessages,
        ),
        CommandSpec::new("write-file", "Write buffer to a new path", true, WriteFile),
        CommandSpec::new("yank", "Insert latest kill", true, Yank),
        CommandSpec::new(
            "yank-rectangle",
            "Insert latest killed rectangle",
            true,
            YankRectangle,
        ),
        CommandSpec::new("yank-pop", "Rotate the just-yanked kill", true, YankPop),
    ]
}

#[cfg(test)]
mod tests {
    use super::{Command, CommandCategory, CommandRegistry, CommandRegistryError, CommandSpec};

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
        assert!(registry.contains("clear-rectangle"));
        assert!(registry.contains("copy-rectangle-as-kill"));
        assert!(registry.contains("copy-rectangle-to-register"));
        assert!(registry.contains("copy-to-register"));
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
        assert!(registry.contains("increment-register"));
        assert!(registry.contains("insert-file"));
        assert!(registry.contains("insert-register"));
        assert!(registry.contains("join-line"));
        assert!(registry.contains("jump-to-register"));
        assert!(registry.contains("list-buffers"));
        assert!(registry.contains("newline-and-indent"));
        assert!(registry.contains("number-to-register"));
        assert!(registry.contains("point-to-register"));
        assert!(registry.contains("quoted-insert"));
        assert!(registry.contains("switch-to-buffer"));
        assert!(registry.contains("kill-buffer"));
        assert!(registry.contains("kill-line"));
        assert!(registry.contains("kill-region"));
        assert!(registry.contains("kill-rectangle"));
        assert!(registry.contains("kill-word"));
        assert!(registry.contains("mark-whole-buffer"));
        assert!(registry.contains("open-rectangle"));
        assert!(registry.contains("copy-region-as-kill"));
        assert!(registry.contains("yank"));
        assert!(registry.contains("yank-rectangle"));
        assert!(registry.contains("yank-pop"));
        assert!(registry.contains("undo"));
        assert!(registry.contains("universal-argument"));
        assert!(registry.contains("view-echo-area-messages"));
        assert!(registry.contains("query-replace"));
        assert!(registry.contains("rectangle-mark-mode"));
        assert!(registry.contains("rectangle-number-lines"));
        assert!(registry.contains("recenter"));
        assert!(registry.contains("scroll-page-backward"));
        assert!(registry.contains("scroll-page-forward"));
        assert!(registry.contains("set-mark-command"));
        assert!(registry.contains("shell-command"));
        assert!(registry.contains("shell-command-on-region"));
        assert!(registry.contains("start-kbd-macro"));
        assert!(registry.contains("string-rectangle"));
        assert!(registry.contains("toggle-line-numbers"));
        assert!(registry.contains("toggle-read-only"));
        assert!(registry.contains("toggle-search-highlighting"));
        assert!(registry.contains("toggle-syntax-highlighting"));
        assert!(registry.contains("write-file"));
        assert!(registry.contains("split-window-below"));
        assert!(registry.contains("split-window-right"));
        assert!(registry.contains("delete-rectangle"));
        assert!(registry.contains("delete-window"));
        assert!(registry.contains("delete-other-windows"));
        assert!(registry.contains("describe-function"));
        assert!(registry.contains("describe-key"));
        assert!(registry.contains("other-window"));
        assert!(!registry.contains("save"));
    }

    #[test]
    fn default_registry_is_valid() {
        let registry = CommandRegistry::default();

        assert_eq!(registry.validate(), Ok(()));
    }

    #[test]
    fn registry_resolves_by_typed_id() {
        let registry = CommandRegistry::default();

        let command = registry
            .get_by_id(Command::SwitchToBuffer)
            .expect("switch-to-buffer command should be registered");

        assert_eq!(command.name, "switch-to-buffer");
        assert_eq!(command.summary, "Switch to a buffer by name");
        assert_eq!(
            command.doc,
            "Prompt for a buffer name and switch the selected window to it."
        );
        assert_eq!(command.category, CommandCategory::Buffers);
    }

    #[test]
    fn registry_keeps_summaries_distinct_from_full_docs() {
        let registry = CommandRegistry::default();
        let command = registry
            .get("save-buffer")
            .expect("save-buffer should be registered");

        assert_eq!(command.summary, "Save current buffer");
        assert_eq!(command.doc, "Write the current file-backed buffer to disk.");
        assert_ne!(command.summary, command.doc);
    }

    #[test]
    fn registry_filters_interactive_commands_and_categories() {
        let registry = CommandRegistry::default();

        assert!(
            registry
                .interactive_commands()
                .all(|command| command.interactive)
        );

        let buffers = registry
            .commands_by_category(CommandCategory::Buffers)
            .map(|command| command.name)
            .collect::<Vec<_>>();

        assert_eq!(
            buffers,
            vec!["list-buffers", "kill-buffer", "switch-to-buffer"]
        );
    }

    #[test]
    fn movement_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();

        assert!(
            registry
                .commands_by_category(CommandCategory::Movement)
                .all(|command| command.handler.is_some())
        );
    }

    #[test]
    fn registry_rejects_duplicate_command_ids() {
        let registry = CommandRegistry::try_new([
            CommandSpec::new(
                "save-buffer",
                "Save current buffer",
                true,
                Command::SaveBuffer,
            ),
            CommandSpec::new("save-file", "Save current file", true, Command::SaveBuffer),
        ]);

        assert_eq!(
            registry.err(),
            Some(CommandRegistryError::DuplicateId(Command::SaveBuffer))
        );
    }

    #[test]
    fn registry_rejects_duplicate_command_names() {
        let registry = CommandRegistry::try_new([
            CommandSpec::new(
                "save-buffer",
                "Save current buffer",
                true,
                Command::SaveBuffer,
            ),
            CommandSpec::new(
                "save-buffer",
                "Write current buffer",
                true,
                Command::WriteFile,
            ),
        ]);

        assert_eq!(
            registry.err(),
            Some(CommandRegistryError::DuplicateName("save-buffer"))
        );
    }

    #[test]
    fn registry_rejects_missing_summary_or_docs() {
        let missing_summary = CommandRegistry::try_new([CommandSpec::new(
            "save-buffer",
            "",
            true,
            Command::SaveBuffer,
        )]);

        assert_eq!(
            missing_summary.err(),
            Some(CommandRegistryError::MissingSummary(Command::SaveBuffer))
        );

        let mut command = CommandSpec::new(
            "save-buffer",
            "Save current buffer",
            true,
            Command::SaveBuffer,
        );
        command.doc = "";
        let missing_doc = CommandRegistry::try_new([command]);

        assert_eq!(
            missing_doc.err(),
            Some(CommandRegistryError::MissingDoc(Command::SaveBuffer))
        );
    }
}
