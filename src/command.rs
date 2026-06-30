// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::input::KeyEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    AboutRile,
    BackToIndentation,
    BackwardChar,
    BackwardKillWord,
    BackwardParagraph,
    BackwardWord,
    BeginningOfBuffer,
    BeginningOfLine,
    BufferListSelect,
    CallLastKeyboardMacro,
    CapitalizeWord,
    ClearRectangle,
    CommentDwim,
    CommentRegion,
    CopyRegionAsKill,
    CopyRectangleAsKill,
    CopyRectangleToRegister,
    CopyToRegister,
    DeleteBackwardChar,
    DeleteBlankLines,
    DeleteChar,
    DeleteHorizontalSpace,
    DeleteRectangle,
    DeleteTrailingWhitespace,
    DeleteOtherWindows,
    DeleteWindow,
    DescribeBuffer,
    DescribeBindings,
    DescribeFunction,
    DescribeKey,
    DescribeKeyBriefly,
    DescribeMode,
    DescribeVariable,
    DowncaseRegion,
    DowncaseWord,
    EndKeyboardMacro,
    EndOfBuffer,
    EndOfLine,
    ExchangePointAndMark,
    ExecuteExtendedCommand,
    FindFile,
    FindFileReadOnly,
    FillParagraph,
    ForwardChar,
    ForwardParagraph,
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
    QuitBufferList,
    QuitHelpWindow,
    QuitMessagesWindow,
    QuitShellOutputWindow,
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
    TransposeChars,
    TransposeLines,
    TransposeWords,
    Undo,
    UncommentRegion,
    UniversalArgument,
    UpcaseRegion,
    UpcaseWord,
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
    pub const fn label(self) -> &'static str {
        match self {
            Self::Movement => "Movement",
            Self::Editing => "Editing",
            Self::Files => "Files",
            Self::Buffers => "Buffers",
            Self::Windows => "Windows",
            Self::Search => "Search",
            Self::Shell => "Shell",
            Self::Registers => "Registers",
            Self::Rectangles => "Rectangles",
            Self::Macros => "Macros",
            Self::Commands => "Commands",
            Self::Help => "Help",
            Self::Configuration => "Configuration",
            Self::System => "System",
        }
    }

    const fn for_command(command: Command) -> Self {
        use Command::*;

        match command {
            BackToIndentation | BackwardChar | BackwardParagraph | BackwardWord
            | BeginningOfBuffer | BeginningOfLine | EndOfBuffer | EndOfLine | ForwardChar
            | ForwardParagraph | ForwardWord | GotoLine | NextLine | PreviousLine | Recenter
            | ScrollPageBackward | ScrollPageForward => Self::Movement,
            BackwardKillWord
            | CapitalizeWord
            | CommentDwim
            | CommentRegion
            | CopyRegionAsKill
            | DeleteBackwardChar
            | DeleteBlankLines
            | DeleteChar
            | DeleteHorizontalSpace
            | DeleteTrailingWhitespace
            | DowncaseRegion
            | DowncaseWord
            | ExchangePointAndMark
            | FillParagraph
            | JoinLine
            | KillLine
            | KillRegion
            | KillWord
            | MarkWholeBuffer
            | NewlineAndIndent
            | OpenLine
            | QuotedInsert
            | SetMarkCommand
            | TransposeChars
            | TransposeLines
            | TransposeWords
            | Undo
            | UncommentRegion
            | UpcaseRegion
            | UpcaseWord
            | Yank
            | YankPop => Self::Editing,
            FindFile | FindFileReadOnly | InsertFile | SaveBuffer | WriteFile => Self::Files,
            BufferListSelect | KillBuffer | ListBuffers | QuitBufferList | SwitchToBuffer => {
                Self::Buffers
            }
            DeleteOtherWindows | DeleteWindow | OtherWindow | SplitWindowBelow
            | SplitWindowRight => Self::Windows,
            IncrementalSearchBackward | IncrementalSearchForward | QueryReplace => Self::Search,
            ShellCommand | ShellCommandOnRegion | QuitShellOutputWindow => Self::Shell,
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
            AboutRile | DescribeBindings | DescribeBuffer | DescribeFunction | DescribeKey
            | DescribeKeyBriefly | DescribeMode | DescribeVariable | QuitHelpWindow
            | QuitMessagesWindow | ViewEchoAreaMessages => Self::Help,
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
        AboutRile => "Show version, build, terminal, config, and runtime path information.",
        BackToIndentation => {
            "Move point to the first non-whitespace character on the current line."
        }
        BackwardChar => "Move point one character toward the beginning of the buffer.",
        BackwardKillWord => "Kill the word before point and save the killed text in the kill ring.",
        BackwardParagraph => "Move point backward to the beginning of a paragraph.",
        BackwardWord => "Move point backward by one word.",
        BeginningOfBuffer => "Move point to the beginning of the current buffer.",
        BeginningOfLine => "Move point to the beginning of the current line.",
        BufferListSelect => "Visit the buffer named on the current buffer-list row.",
        CallLastKeyboardMacro => "Replay the most recently recorded keyboard macro.",
        CapitalizeWord => "Capitalize the following word or words.",
        ClearRectangle => "Replace the active rectangle contents with spaces.",
        CommentDwim => "Insert, comment, or uncomment line comments for the current mode.",
        CommentRegion => "Add line comment markers to the active region.",
        CopyRegionAsKill => "Copy the active region to the kill ring without deleting it.",
        CopyRectangleAsKill => "Copy the active rectangle to the rectangle kill ring.",
        CopyRectangleToRegister => "Copy the active rectangle into a prompted register.",
        CopyToRegister => "Copy the active region text into a prompted register.",
        DeleteBackwardChar => "Delete the character immediately before point.",
        DeleteBlankLines => "Delete redundant blank lines around point.",
        DeleteChar => "Delete the character at point.",
        DeleteHorizontalSpace => "Delete spaces and tabs around point.",
        DeleteRectangle => "Delete the active rectangle without saving it to the kill ring.",
        DeleteTrailingWhitespace => "Delete trailing spaces and tabs at line ends.",
        DeleteOtherWindows => "Delete every window except the selected window.",
        DeleteWindow => "Delete the selected window when another window is available.",
        DescribeBuffer => "Show detailed state for the current buffer.",
        DescribeBindings => "Show the active keymap stack and its key bindings.",
        DescribeFunction => "Prompt for an interactive command and show its help buffer.",
        DescribeKey => "Read a key sequence and describe the command bound to it.",
        DescribeKeyBriefly => "Read a key sequence and echo the command bound to it.",
        DescribeMode => "Show the active major, minor, and special-buffer modes.",
        DescribeVariable => "Prompt for a configuration option and show its help buffer.",
        DowncaseRegion => "Convert the active region to lower case.",
        DowncaseWord => "Convert the following word or words to lower case.",
        EndKeyboardMacro => "Finish recording the current keyboard macro.",
        EndOfBuffer => "Move point to the end of the current buffer.",
        EndOfLine => "Move point to the end of the current line.",
        ExchangePointAndMark => "Swap point with mark and reactivate the selected region.",
        ExecuteExtendedCommand => "Prompt for an interactive command name and run it.",
        FindFile => "Prompt for a file path and open it for editing.",
        FindFileReadOnly => "Prompt for a file path and open it read-only.",
        FillParagraph => "Reflow the current plain-text paragraph.",
        ForwardChar => "Move point one character toward the end of the buffer.",
        ForwardParagraph => "Move point forward to the end of a paragraph.",
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
        QuitBufferList => "Close the buffer list window or leave the buffer list buffer.",
        QuitHelpWindow => "Restore the buffer that was active before the help window opened.",
        QuitMessagesWindow => {
            "Restore the buffer that was active before the messages window opened."
        }
        QuitShellOutputWindow => {
            "Restore the buffer that was active before the shell output window opened."
        }
        QuotedInsert => "Read the next key and insert it literally when supported.",
        QueryReplace => "Prompt for search and replacement strings and replace interactively.",
        RectangleMarkMode => "Activate rectangle mark mode for column-oriented region commands.",
        RectangleNumberLines => "Insert formatted line numbers down the active rectangle.",
        Recenter => "Cycle point between center, top, and bottom of the selected window.",
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
        TransposeChars => "Transpose characters around point.",
        TransposeLines => "Transpose the previous line past the current line or lines.",
        TransposeWords => "Transpose the word before or containing point with another word.",
        Undo => "Undo the latest edit recorded for the current buffer.",
        UncommentRegion => "Remove line comment markers from the active region.",
        UniversalArgument => "Set or extend the numeric argument for the next command.",
        UpcaseRegion => "Convert the active region to upper case.",
        UpcaseWord => "Convert the following word or words to upper case.",
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
    MissingHandler(CommandId),
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
            if command.handler.is_none() {
                return Err(CommandRegistryError::MissingHandler(command.command));
            }
        }

        Ok(())
    }
}

pub fn default_commands() -> Vec<CommandSpec> {
    use Command::*;

    vec![
        CommandSpec::new("about-rile", "Show information about Rile", true, AboutRile)
            .with_handler(crate::editor::Editor::command_about_rile),
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
        )
        .with_handler(crate::editor::Editor::command_backward_kill_word),
        CommandSpec::new(
            "backward-paragraph",
            "Move cursor backward by paragraph",
            true,
            BackwardParagraph,
        )
        .with_handler(crate::editor::Editor::command_backward_paragraph),
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
            "buffer-list-select",
            "Visit buffer on current buffer-list row",
            true,
            BufferListSelect,
        )
        .with_handler(crate::editor::Editor::command_buffer_list_select),
        CommandSpec::new(
            "call-last-kbd-macro",
            "Execute the last keyboard macro",
            true,
            CallLastKeyboardMacro,
        )
        .with_handler(crate::editor::Editor::command_call_last_keyboard_macro),
        CommandSpec::new(
            "capitalize-word",
            "Capitalize word after cursor",
            true,
            CapitalizeWord,
        )
        .with_handler(crate::editor::Editor::command_capitalize_word),
        CommandSpec::new(
            "clear-rectangle",
            "Replace rectangle contents with spaces",
            true,
            ClearRectangle,
        )
        .with_handler(crate::editor::Editor::command_clear_rectangle),
        CommandSpec::new(
            "comment-dwim",
            "Insert or toggle line comments",
            true,
            CommentDwim,
        )
        .with_handler(crate::editor::Editor::command_comment_dwim),
        CommandSpec::new(
            "comment-region",
            "Comment active region",
            true,
            CommentRegion,
        )
        .with_handler(crate::editor::Editor::command_comment_region),
        CommandSpec::new(
            "copy-region-as-kill",
            "Copy active region to kill ring",
            true,
            CopyRegionAsKill,
        )
        .with_handler(crate::editor::Editor::command_copy_region_as_kill),
        CommandSpec::new(
            "copy-rectangle-as-kill",
            "Copy rectangle to kill ring",
            true,
            CopyRectangleAsKill,
        )
        .with_handler(crate::editor::Editor::command_copy_rectangle_as_kill),
        CommandSpec::new(
            "copy-rectangle-to-register",
            "Copy rectangle to a register",
            true,
            CopyRectangleToRegister,
        )
        .with_handler(crate::editor::Editor::command_copy_rectangle_to_register),
        CommandSpec::new(
            "copy-to-register",
            "Copy active region to a register",
            true,
            CopyToRegister,
        )
        .with_handler(crate::editor::Editor::command_copy_to_register),
        CommandSpec::new(
            "delete-backward-char",
            "Delete character before cursor",
            true,
            DeleteBackwardChar,
        )
        .with_handler(crate::editor::Editor::command_delete_backward_char),
        CommandSpec::new(
            "delete-blank-lines",
            "Delete redundant blank lines",
            true,
            DeleteBlankLines,
        )
        .with_handler(crate::editor::Editor::command_delete_blank_lines),
        CommandSpec::new(
            "delete-char",
            "Delete character at cursor",
            true,
            DeleteChar,
        )
        .with_handler(crate::editor::Editor::command_delete_char),
        CommandSpec::new(
            "delete-horizontal-space",
            "Delete spaces and tabs around cursor",
            true,
            DeleteHorizontalSpace,
        )
        .with_handler(crate::editor::Editor::command_delete_horizontal_space),
        CommandSpec::new(
            "delete-rectangle",
            "Delete rectangle without saving it",
            true,
            DeleteRectangle,
        )
        .with_handler(crate::editor::Editor::command_delete_rectangle),
        CommandSpec::new(
            "delete-trailing-whitespace",
            "Delete trailing spaces and tabs",
            true,
            DeleteTrailingWhitespace,
        )
        .with_handler(crate::editor::Editor::command_delete_trailing_whitespace),
        CommandSpec::new(
            "delete-other-windows",
            "Delete all other windows",
            true,
            DeleteOtherWindows,
        )
        .with_handler(crate::editor::Editor::command_delete_other_windows),
        CommandSpec::new("delete-window", "Delete current window", true, DeleteWindow)
            .with_handler(crate::editor::Editor::command_delete_window),
        CommandSpec::new(
            "downcase-region",
            "Convert region to lower case",
            true,
            DowncaseRegion,
        )
        .with_handler(crate::editor::Editor::command_downcase_region),
        CommandSpec::new(
            "downcase-word",
            "Convert word after cursor to lower case",
            true,
            DowncaseWord,
        )
        .with_handler(crate::editor::Editor::command_downcase_word),
        CommandSpec::new(
            "describe-buffer",
            "Describe current buffer",
            true,
            DescribeBuffer,
        )
        .with_handler(crate::editor::Editor::command_describe_buffer),
        CommandSpec::new(
            "describe-bindings",
            "Show active key bindings",
            true,
            DescribeBindings,
        )
        .with_handler(crate::editor::Editor::command_describe_bindings),
        CommandSpec::new(
            "describe-function",
            "Describe an interactive command",
            true,
            DescribeFunction,
        )
        .with_handler(crate::editor::Editor::command_describe_function),
        CommandSpec::new("describe-key", "Describe a key binding", true, DescribeKey)
            .with_handler(crate::editor::Editor::command_describe_key),
        CommandSpec::new(
            "describe-key-briefly",
            "Briefly describe a key binding",
            true,
            DescribeKeyBriefly,
        )
        .with_handler(crate::editor::Editor::command_describe_key_briefly),
        CommandSpec::new("describe-mode", "Describe active modes", true, DescribeMode)
            .with_handler(crate::editor::Editor::command_describe_mode),
        CommandSpec::new(
            "describe-variable",
            "Describe a configuration option",
            true,
            DescribeVariable,
        )
        .with_handler(crate::editor::Editor::command_describe_variable),
        CommandSpec::new(
            "end-kbd-macro",
            "Finish defining a keyboard macro",
            true,
            EndKeyboardMacro,
        )
        .with_handler(crate::editor::Editor::command_end_keyboard_macro),
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
        )
        .with_handler(crate::editor::Editor::command_exchange_point_and_mark),
        CommandSpec::new(
            "execute-extended-command",
            "Run command by name",
            true,
            ExecuteExtendedCommand,
        )
        .with_handler(crate::editor::Editor::command_execute_extended_command),
        CommandSpec::new("find-file", "Open file by path", true, FindFile)
            .with_handler(crate::editor::Editor::command_find_file),
        CommandSpec::new(
            "find-file-read-only",
            "Open file read-only by path",
            true,
            FindFileReadOnly,
        )
        .with_handler(crate::editor::Editor::command_find_file_read_only),
        CommandSpec::new(
            "fill-paragraph",
            "Reflow the current paragraph",
            true,
            FillParagraph,
        )
        .with_handler(crate::editor::Editor::command_fill_paragraph),
        CommandSpec::new("forward-char", "Move cursor right", true, ForwardChar)
            .with_handler(crate::editor::Editor::command_forward_char),
        CommandSpec::new(
            "forward-word",
            "Move cursor forward by word",
            true,
            ForwardWord,
        )
        .with_handler(crate::editor::Editor::command_forward_word),
        CommandSpec::new(
            "forward-paragraph",
            "Move cursor forward by paragraph",
            true,
            ForwardParagraph,
        )
        .with_handler(crate::editor::Editor::command_forward_paragraph),
        CommandSpec::new("goto-line", "Go to line or line:column", true, GotoLine)
            .with_handler(crate::editor::Editor::command_goto_line),
        CommandSpec::new(
            "isearch-backward",
            "Search backward incrementally",
            true,
            IncrementalSearchBackward,
        )
        .with_handler(crate::editor::Editor::command_incremental_search_backward),
        CommandSpec::new(
            "isearch-forward",
            "Search forward incrementally",
            true,
            IncrementalSearchForward,
        )
        .with_handler(crate::editor::Editor::command_incremental_search_forward),
        CommandSpec::new(
            "insert-file",
            "Insert file contents at point",
            true,
            InsertFile,
        )
        .with_handler(crate::editor::Editor::command_insert_file),
        CommandSpec::new(
            "increment-register",
            "Add numeric argument to a number register",
            true,
            IncrementRegister,
        )
        .with_handler(crate::editor::Editor::command_increment_register),
        CommandSpec::new(
            "insert-register",
            "Insert text, rectangle, or number from a register",
            true,
            InsertRegister,
        )
        .with_handler(crate::editor::Editor::command_insert_register),
        CommandSpec::new(
            "join-line",
            "Join current line to previous line",
            true,
            JoinLine,
        )
        .with_handler(crate::editor::Editor::command_join_line),
        CommandSpec::new(
            "jump-to-register",
            "Jump to a point register",
            true,
            JumpToRegister,
        )
        .with_handler(crate::editor::Editor::command_jump_to_register),
        CommandSpec::new("list-buffers", "List active buffers", true, ListBuffers)
            .with_handler(crate::editor::Editor::command_list_buffers),
        CommandSpec::new(
            "newline-and-indent",
            "Insert newline and indent according to mode",
            true,
            NewlineAndIndent,
        )
        .with_handler(crate::editor::Editor::command_newline_and_indent),
        CommandSpec::new("kill-buffer", "Kill a buffer by name", true, KillBuffer)
            .with_handler(crate::editor::Editor::command_kill_buffer),
        CommandSpec::new("kill-line", "Kill text to end of line", true, KillLine)
            .with_handler(crate::editor::Editor::command_kill_line),
        CommandSpec::new("kill-region", "Kill active region", true, KillRegion)
            .with_handler(crate::editor::Editor::command_kill_region),
        CommandSpec::new(
            "kill-rectangle",
            "Kill rectangle to kill ring",
            true,
            KillRectangle,
        )
        .with_handler(crate::editor::Editor::command_kill_rectangle),
        CommandSpec::new("kill-word", "Kill word after cursor", true, KillWord)
            .with_handler(crate::editor::Editor::command_kill_word),
        CommandSpec::new(
            "mark-whole-buffer",
            "Mark the whole buffer",
            true,
            MarkWholeBuffer,
        )
        .with_handler(crate::editor::Editor::command_mark_whole_buffer),
        CommandSpec::new("next-line", "Move cursor down", true, NextLine)
            .with_handler(crate::editor::Editor::command_next_line),
        CommandSpec::new(
            "number-to-register",
            "Store numeric argument in a register",
            true,
            NumberToRegister,
        )
        .with_handler(crate::editor::Editor::command_number_to_register),
        CommandSpec::new("open-line", "Insert newline after point", true, OpenLine)
            .with_handler(crate::editor::Editor::command_open_line),
        CommandSpec::new(
            "open-rectangle",
            "Insert blank space into rectangle columns",
            true,
            OpenRectangle,
        )
        .with_handler(crate::editor::Editor::command_open_rectangle),
        CommandSpec::new("other-window", "Select next window", true, OtherWindow)
            .with_handler(crate::editor::Editor::command_other_window),
        CommandSpec::new("previous-line", "Move cursor up", true, PreviousLine)
            .with_handler(crate::editor::Editor::command_previous_line),
        CommandSpec::new(
            "quoted-insert",
            "Insert the next key literally",
            true,
            QuotedInsert,
        )
        .with_handler(crate::editor::Editor::command_quoted_insert),
        CommandSpec::new(
            "quit-buffer-list",
            "Close the buffer list window",
            true,
            QuitBufferList,
        )
        .with_handler(crate::editor::Editor::command_quit_buffer_list),
        CommandSpec::new(
            "quit-help-window",
            "Restore the previous buffer from help",
            true,
            QuitHelpWindow,
        )
        .with_handler(crate::editor::Editor::command_quit_help_window),
        CommandSpec::new(
            "quit-messages-window",
            "Restore the previous buffer from messages",
            true,
            QuitMessagesWindow,
        )
        .with_handler(crate::editor::Editor::command_quit_messages_window),
        CommandSpec::new(
            "quit-shell-output-window",
            "Restore the previous buffer from shell output",
            true,
            QuitShellOutputWindow,
        )
        .with_handler(crate::editor::Editor::command_quit_shell_output_window),
        CommandSpec::new(
            "point-to-register",
            "Store point in a register",
            true,
            PointToRegister,
        )
        .with_handler(crate::editor::Editor::command_point_to_register),
        CommandSpec::new(
            "query-replace",
            "Interactively replace text",
            true,
            QueryReplace,
        )
        .with_handler(crate::editor::Editor::command_query_replace),
        CommandSpec::new(
            "rectangle-mark-mode",
            "Mark a rectangular region",
            true,
            RectangleMarkMode,
        )
        .with_handler(crate::editor::Editor::command_rectangle_mark_mode),
        CommandSpec::new(
            "rectangle-number-lines",
            "Insert line numbers at the rectangle left edge",
            true,
            RectangleNumberLines,
        )
        .with_handler(crate::editor::Editor::command_rectangle_number_lines),
        CommandSpec::new(
            "recenter",
            "Cycle cursor position in window",
            true,
            Recenter,
        )
        .with_handler(crate::editor::Editor::command_recenter),
        CommandSpec::new("save-buffer", "Save current buffer", true, SaveBuffer)
            .with_handler(crate::editor::Editor::command_save_buffer),
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
        )
        .with_handler(crate::editor::Editor::command_save_buffers_kill_terminal),
        CommandSpec::new(
            "set-mark-command",
            "Set mark at point",
            true,
            SetMarkCommand,
        )
        .with_handler(crate::editor::Editor::command_set_mark_command),
        CommandSpec::new("shell-command", "Run a shell command", true, ShellCommand)
            .with_handler(crate::editor::Editor::command_shell_command),
        CommandSpec::new(
            "shell-command-on-region",
            "Run a shell command with region as input",
            true,
            ShellCommandOnRegion,
        )
        .with_handler(crate::editor::Editor::command_shell_command_on_region),
        CommandSpec::new(
            "start-kbd-macro",
            "Start defining a keyboard macro",
            true,
            StartKeyboardMacro,
        )
        .with_handler(crate::editor::Editor::command_start_keyboard_macro),
        CommandSpec::new(
            "string-rectangle",
            "Replace rectangle contents with a string",
            true,
            StringRectangle,
        )
        .with_handler(crate::editor::Editor::command_string_rectangle),
        CommandSpec::new(
            "split-window-below",
            "Split current window horizontally",
            true,
            SplitWindowBelow,
        )
        .with_handler(crate::editor::Editor::command_split_window_below),
        CommandSpec::new(
            "split-window-right",
            "Split current window vertically",
            true,
            SplitWindowRight,
        )
        .with_handler(crate::editor::Editor::command_split_window_right),
        CommandSpec::new(
            "switch-to-buffer",
            "Switch to a buffer by name",
            true,
            SwitchToBuffer,
        )
        .with_handler(crate::editor::Editor::command_switch_to_buffer),
        CommandSpec::new(
            "toggle-line-numbers",
            "Toggle line numbers",
            true,
            ToggleLineNumbers,
        )
        .with_handler(crate::editor::Editor::command_toggle_line_numbers),
        CommandSpec::new(
            "toggle-read-only",
            "Toggle buffer read-only state",
            true,
            ToggleReadOnly,
        )
        .with_handler(crate::editor::Editor::command_toggle_read_only),
        CommandSpec::new(
            "toggle-search-highlighting",
            "Toggle search highlighting",
            true,
            ToggleSearchHighlighting,
        )
        .with_handler(crate::editor::Editor::command_toggle_search_highlighting),
        CommandSpec::new(
            "toggle-syntax-highlighting",
            "Toggle syntax highlighting",
            true,
            ToggleSyntaxHighlighting,
        )
        .with_handler(crate::editor::Editor::command_toggle_syntax_highlighting),
        CommandSpec::new(
            "transpose-chars",
            "Transpose characters around cursor",
            true,
            TransposeChars,
        )
        .with_handler(crate::editor::Editor::command_transpose_chars),
        CommandSpec::new(
            "transpose-lines",
            "Transpose lines around cursor",
            true,
            TransposeLines,
        )
        .with_handler(crate::editor::Editor::command_transpose_lines),
        CommandSpec::new(
            "transpose-words",
            "Transpose words around cursor",
            true,
            TransposeWords,
        )
        .with_handler(crate::editor::Editor::command_transpose_words),
        CommandSpec::new("undo", "Undo last edit", true, Undo)
            .with_handler(crate::editor::Editor::command_undo),
        CommandSpec::new(
            "uncomment-region",
            "Uncomment active region",
            true,
            UncommentRegion,
        )
        .with_handler(crate::editor::Editor::command_uncomment_region),
        CommandSpec::new(
            "universal-argument",
            "Set a numeric argument for the next command",
            true,
            UniversalArgument,
        )
        .with_handler(crate::editor::Editor::command_universal_argument),
        CommandSpec::new(
            "upcase-region",
            "Convert region to upper case",
            true,
            UpcaseRegion,
        )
        .with_handler(crate::editor::Editor::command_upcase_region),
        CommandSpec::new(
            "upcase-word",
            "Convert word after cursor to upper case",
            true,
            UpcaseWord,
        )
        .with_handler(crate::editor::Editor::command_upcase_word),
        CommandSpec::new(
            "view-echo-area-messages",
            "Show the message history",
            true,
            ViewEchoAreaMessages,
        )
        .with_handler(crate::editor::Editor::command_view_echo_area_messages),
        CommandSpec::new("write-file", "Write buffer to a new path", true, WriteFile)
            .with_handler(crate::editor::Editor::command_write_file),
        CommandSpec::new("yank", "Insert latest kill", true, Yank)
            .with_handler(crate::editor::Editor::command_yank),
        CommandSpec::new(
            "yank-rectangle",
            "Insert latest killed rectangle",
            true,
            YankRectangle,
        )
        .with_handler(crate::editor::Editor::command_yank_rectangle),
        CommandSpec::new("yank-pop", "Rotate the just-yanked kill", true, YankPop)
            .with_handler(crate::editor::Editor::command_yank_pop),
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
        assert!(registry.contains("about-rile"));
        assert!(registry.contains("back-to-indentation"));
        assert!(registry.contains("beginning-of-buffer"));
        assert!(registry.contains("backward-kill-word"));
        assert!(registry.contains("backward-word"));
        assert!(registry.contains("buffer-list-select"));
        assert!(registry.contains("call-last-kbd-macro"));
        assert!(registry.contains("capitalize-word"));
        assert!(registry.contains("clear-rectangle"));
        assert!(registry.contains("comment-dwim"));
        assert!(registry.contains("comment-region"));
        assert!(registry.contains("copy-rectangle-as-kill"));
        assert!(registry.contains("copy-rectangle-to-register"));
        assert!(registry.contains("copy-to-register"));
        assert!(registry.contains("end-of-buffer"));
        assert!(registry.contains("end-kbd-macro"));
        assert!(registry.contains("exchange-point-and-mark"));
        assert!(registry.contains("execute-extended-command"));
        assert!(registry.contains("find-file"));
        assert!(registry.contains("find-file-read-only"));
        assert!(registry.contains("fill-paragraph"));
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
        assert!(registry.contains("quit-buffer-list"));
        assert!(registry.contains("quit-help-window"));
        assert!(registry.contains("quit-messages-window"));
        assert!(registry.contains("quit-shell-output-window"));
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
        assert!(registry.contains("transpose-lines"));
        assert!(registry.contains("transpose-words"));
        assert!(registry.contains("uncomment-region"));
        assert!(registry.contains("upcase-region"));
        assert!(registry.contains("upcase-word"));
        assert!(registry.contains("write-file"));
        assert!(registry.contains("split-window-below"));
        assert!(registry.contains("split-window-right"));
        assert!(registry.contains("delete-rectangle"));
        assert!(registry.contains("delete-blank-lines"));
        assert!(registry.contains("delete-horizontal-space"));
        assert!(registry.contains("delete-trailing-whitespace"));
        assert!(registry.contains("delete-window"));
        assert!(registry.contains("delete-other-windows"));
        assert!(registry.contains("downcase-region"));
        assert!(registry.contains("downcase-word"));
        assert!(registry.contains("describe-buffer"));
        assert!(registry.contains("describe-bindings"));
        assert!(registry.contains("describe-function"));
        assert!(registry.contains("describe-key"));
        assert!(registry.contains("describe-key-briefly"));
        assert!(registry.contains("describe-mode"));
        assert!(registry.contains("describe-variable"));
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
            vec![
                "buffer-list-select",
                "list-buffers",
                "kill-buffer",
                "quit-buffer-list",
                "switch-to-buffer"
            ]
        );
    }

    #[test]
    fn default_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();

        for command in registry.commands() {
            assert!(
                command.handler.is_some(),
                "{} should have a handler",
                command.name
            );
        }
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
    fn editing_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();

        assert!(
            registry
                .commands_by_category(CommandCategory::Editing)
                .all(|command| command.handler.is_some())
        );
    }

    #[test]
    fn buffer_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();

        assert!(
            registry
                .commands_by_category(CommandCategory::Buffers)
                .all(|command| command.handler.is_some())
        );
    }

    #[test]
    fn phase_2_buffer_slice_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();
        let commands = [
            Command::BufferListSelect,
            Command::KillBuffer,
            Command::ListBuffers,
            Command::QuitBufferList,
            Command::SwitchToBuffer,
        ];

        for command in commands {
            let spec = registry
                .get_by_id(command)
                .expect("command should be registered");
            assert!(
                spec.handler.is_some(),
                "{} should have a handler",
                spec.name
            );
        }
    }

    #[test]
    fn phase_3_local_special_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();
        let commands = [
            Command::BufferListSelect,
            Command::QuitBufferList,
            Command::QuitHelpWindow,
            Command::QuitMessagesWindow,
            Command::QuitShellOutputWindow,
        ];

        for command in commands {
            let spec = registry
                .get_by_id(command)
                .expect("command should be registered");
            assert!(
                spec.handler.is_some(),
                "{} should have a handler",
                spec.name
            );
        }
    }

    #[test]
    fn window_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();

        assert!(
            registry
                .commands_by_category(CommandCategory::Windows)
                .all(|command| command.handler.is_some())
        );
    }

    #[test]
    fn phase_2_window_slice_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();
        let commands = [
            Command::DeleteOtherWindows,
            Command::DeleteWindow,
            Command::OtherWindow,
            Command::SplitWindowBelow,
            Command::SplitWindowRight,
        ];

        for command in commands {
            let spec = registry
                .get_by_id(command)
                .expect("command should be registered");
            assert!(
                spec.handler.is_some(),
                "{} should have a handler",
                spec.name
            );
        }
    }

    #[test]
    fn phase_2_editing_slice_commands_have_registered_handlers() {
        let registry = CommandRegistry::default();
        let commands = [
            Command::BackwardKillWord,
            Command::CapitalizeWord,
            Command::CommentDwim,
            Command::CommentRegion,
            Command::CopyRegionAsKill,
            Command::DeleteBackwardChar,
            Command::DeleteBlankLines,
            Command::DeleteChar,
            Command::DeleteHorizontalSpace,
            Command::DeleteTrailingWhitespace,
            Command::DowncaseRegion,
            Command::DowncaseWord,
            Command::ExchangePointAndMark,
            Command::FillParagraph,
            Command::JoinLine,
            Command::KillLine,
            Command::KillRegion,
            Command::KillWord,
            Command::MarkWholeBuffer,
            Command::NewlineAndIndent,
            Command::OpenLine,
            Command::QuotedInsert,
            Command::SetMarkCommand,
            Command::TransposeChars,
            Command::TransposeLines,
            Command::TransposeWords,
            Command::Undo,
            Command::UncommentRegion,
            Command::UpcaseRegion,
            Command::UpcaseWord,
            Command::Yank,
            Command::YankPop,
        ];

        for command in commands {
            let spec = registry
                .get_by_id(command)
                .expect("command should be registered");
            assert!(
                spec.handler.is_some(),
                "{} should have a handler",
                spec.name
            );
        }
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

    #[test]
    fn registry_rejects_missing_handlers() {
        let registry = CommandRegistry::try_new([CommandSpec::new(
            "save-buffer",
            "Save current buffer",
            true,
            Command::SaveBuffer,
        )]);

        assert_eq!(
            registry.err(),
            Some(CommandRegistryError::MissingHandler(Command::SaveBuffer))
        );
    }
}
