// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::undo::UndoRecord;
use crate::buffer::{Buffer, BufferId, Position, RectangleEdit, TextRange};
use crate::buffers::BufferManager;
use crate::command::{
    Command, CommandContext, CommandOutcome, CommandRegistry, CommandSpec, Invocation,
};
use crate::completion::{CompletionConfig, CompletionSession, CompletionStyle};
use crate::config::{Config, ThemeName, default_config_path};
use crate::file::{Document, DocumentKind};
use crate::input::{KeyEvent, SpecialKey};
use crate::keymap::{
    KeyMap, KeyMapStack, KeyStackResolution, buffer_list_keymap, format_key_sequence, help_keymap,
    messages_keymap, shell_output_keymap,
};
use crate::minibuffer::{MinibufferState, PromptKind};
use crate::mode::{ModeId, ModeRegistry};
use crate::option::{OptionId, OptionRegistry, OptionValue};
use crate::render::{DecorationProvider, Face, Span, collect_spans_for_line};
use crate::shell::{ShellCommandOutput, run_shell_command};
use crate::syntax::{CommentSyntax, Highlighter, MajorMode, SyntaxHighlighter, SyntaxMode};
use crate::text::is_word_character;
use crate::window::{SplitAxis, Viewport, WindowId, WindowLayout, WindowSet};
use crate::{Result, RileError};

mod completion_policy;
mod help;
mod prompt_history;
mod search;

use completion_policy::{
    CompletionAcceptContext, accepted_completion_input, directory_completion_to_enter,
    raw_completion_input, tab_completion_input,
};
use help::{
    format_about_rile_help, format_describe_bindings_help, format_describe_buffer_help,
    format_describe_function_help, format_describe_key_brief_message, format_describe_key_help,
    format_describe_mode_help, format_describe_variable_help, format_key_prefix_help,
    format_unbound_key_help, format_unbound_key_message,
};
use prompt_history::PromptHistoryStore;
use search::{find_match, search_start_after};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    Continue,
    Quit,
    Suspend,
}

#[derive(Debug, Clone)]
pub struct Editor {
    buffers: BufferManager,
    windows: WindowSet,
    buffer_viewports: HashMap<BufferId, Viewport>,
    current_buffer: BufferId,
    previous_buffer: Option<BufferId>,
    cursor: Position,
    goal_display_column: Option<usize>,
    key_sequence: Vec<KeyEvent>,
    current_command_sequence: Option<Vec<KeyEvent>>,
    keyboard_macro_prompt_start: Option<usize>,
    keymap: KeyMap,
    help_keymap: KeyMap,
    messages_keymap: KeyMap,
    shell_output_keymap: KeyMap,
    buffer_list_keymap: KeyMap,
    commands: CommandRegistry,
    minibuffer: MinibufferState,
    help_return: Option<Viewport>,
    messages_return: Option<Viewport>,
    shell_output_return: Option<Viewport>,
    buffer_list_rows: Vec<Option<BufferId>>,
    auto_revert_buffers: HashSet<BufferId>,
    global_auto_revert: bool,
    describe_key: Option<DescribeKeyState>,
    completion: Option<CompletionSession>,
    completion_return: Option<Viewport>,
    completion_config: CompletionConfig,
    prompt_history: PromptHistoryStore,
    recording_keyboard_macro: Option<Vec<KeyEvent>>,
    last_keyboard_macro: Option<Vec<KeyEvent>>,
    replaying_keyboard_macro: bool,
    universal_argument: Option<UniversalArgumentState>,
    search: Option<SearchState>,
    query_replace: Option<QueryReplaceState>,
    rectangle_number_prompt: Option<RectangleNumberPromptState>,
    shell_command_prompt: Option<ShellCommandPromptState>,
    pending_kill_buffer: Option<BufferId>,
    save_some_buffers: Option<SaveSomeBuffersState>,
    pending_register: Option<PendingRegisterCommand>,
    quoted_insert: bool,
    region: Option<RegionState>,
    registers: HashMap<char, RegisterValue>,
    kill_ring: Vec<KillEntry>,
    yank_state: Option<YankState>,
    recenter_cycle_index: usize,
    window_line_cycle_index: usize,
    last_command_was_kill: bool,
    kill_recorded_this_command: bool,
    undo_stack: Vec<UndoEntry>,
    grouping_insert: bool,
    syntax_enabled: bool,
    search_highlighting: bool,
    line_numbers: bool,
    tab_width: usize,
    fill_column: usize,
    backup_on_save: bool,
    theme: ThemeName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegionState {
    buffer: BufferId,
    mark: Position,
    active: bool,
    shape: RegionShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DescribeKeyState {
    sequence: Vec<KeyEvent>,
    mode: DescribeKeyMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DescribeKeyMode {
    Help,
    Brief,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveModes {
    major: ModeId,
    syntax: ModeId,
    special: Option<ModeId>,
    minor: Vec<ModeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BufferDescription {
    name: String,
    path: Option<String>,
    kind: &'static str,
    modified: bool,
    read_only: bool,
    point_line: usize,
    point_column: usize,
    encoding: &'static str,
    line_ending: &'static str,
    final_newline: bool,
    modes: ActiveModes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AboutRileInfo {
    version: &'static str,
    build_profile: &'static str,
    enabled_features: &'static str,
    terminal_backend: &'static str,
    config_path: Option<String>,
    current_directory: Option<String>,
}

impl AboutRileInfo {
    fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            build_profile: build_profile(),
            enabled_features: "not reported by this build",
            terminal_backend: "ANSI terminal",
            config_path: default_config_path().map(|path| path.display().to_string()),
            current_directory: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
        }
    }
}

impl DescribeKeyMode {
    fn prompt(self) -> &'static str {
        match self {
            Self::Help => "Describe key:",
            Self::Brief => "Describe key briefly:",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionShape {
    Linear,
    Rectangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseTransform {
    Lower,
    Upper,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentAction {
    Comment,
    Uncomment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RectangleBounds {
    start_line: usize,
    end_line: usize,
    start_column: usize,
    end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransposeEdit {
    range: TextRange,
    replacement: String,
    cursor_after: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WordSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KillEntry {
    Text(String),
    Rectangle(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegisterValue {
    Point {
        buffer: BufferId,
        position: Position,
    },
    Text(String),
    Rectangle(Vec<String>),
    Number(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingRegisterCommand {
    CopyRectangle,
    CopyText,
    IncrementNumber { amount: i32 },
    Insert,
    Jump,
    Number { value: i32 },
    Point,
}

impl KillEntry {
    fn is_empty(&self) -> bool {
        match self {
            Self::Text(text) => text.is_empty(),
            Self::Rectangle(lines) => lines.iter().all(String::is_empty),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UndoEntry {
    buffer: BufferId,
    record: UndoRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct YankState {
    buffer: BufferId,
    range: TextRange,
    kill_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UniversalArgumentState {
    value: i32,
    entered_digits: bool,
    negative: bool,
}

impl UniversalArgumentState {
    fn new() -> Self {
        Self {
            value: 4,
            entered_digits: false,
            negative: false,
        }
    }

    fn multiply(&mut self) {
        if !self.entered_digits {
            self.value = self.value.saturating_mul(4);
        }
    }

    fn push_digit(&mut self, digit: u32) {
        if !self.entered_digits {
            self.value = 0;
            self.entered_digits = true;
        }
        self.value = self.value.saturating_mul(10).saturating_add(digit as i32);
    }

    fn negate(&mut self) {
        self.negative = !self.negative;
    }

    fn value(self) -> i32 {
        if self.negative {
            -self.value
        } else {
            self.value
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KillDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptEditOutcome {
    Unhandled,
    Handled { changed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryReplaceState {
    query: String,
    replacement: String,
    current: Option<TextRange>,
    replacements: usize,
    visited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RectangleNumberPromptState {
    bounds: RectangleBounds,
    start_at: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellCommandPromptState {
    action: ShellCommandAction,
    stdin: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SaveSomeBuffersState {
    pending: Vec<BufferId>,
    saved: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellCommandAction {
    Display,
    Insert,
    ReplaceRegion { range: TextRange },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchDirection {
    Forward,
    Backward,
}

impl SearchDirection {
    fn label(self) -> &'static str {
        match self {
            Self::Forward => "I-search: ",
            Self::Backward => "I-search backward: ",
        }
    }

    fn failing_label(self) -> &'static str {
        match self {
            Self::Forward => "Failing I-search: ",
            Self::Backward => "Failing I-search backward: ",
        }
    }

    fn wrapped_label(self) -> &'static str {
        match self {
            Self::Forward => "Wrapped I-search: ",
            Self::Backward => "Wrapped I-search backward: ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchState {
    direction: SearchDirection,
    origin: Position,
    current: Option<TextRange>,
    failed_direction: Option<SearchDirection>,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self::with_config(document, Config::default())
    }

    pub fn with_config(mut document: Document, config: Config) -> Self {
        document.set_backup_on_save(config.backup_on_save);
        let buffers = BufferManager::new(document);
        let current_buffer = buffers.entries()[0].id();
        let mut buffer_viewports = HashMap::new();
        buffer_viewports.insert(current_buffer, Viewport::new(current_buffer));
        Self {
            windows: WindowSet::new(current_buffer),
            buffers,
            buffer_viewports,
            current_buffer,
            previous_buffer: None,
            cursor: Position::new(0, 0),
            goal_display_column: None,
            key_sequence: Vec::new(),
            current_command_sequence: None,
            keyboard_macro_prompt_start: None,
            keymap: KeyMap::default(),
            help_keymap: help_keymap(),
            messages_keymap: messages_keymap(),
            shell_output_keymap: shell_output_keymap(),
            buffer_list_keymap: buffer_list_keymap(),
            commands: CommandRegistry::default(),
            minibuffer: MinibufferState::default(),
            help_return: None,
            messages_return: None,
            shell_output_return: None,
            buffer_list_rows: Vec::new(),
            auto_revert_buffers: HashSet::new(),
            global_auto_revert: false,
            describe_key: None,
            completion: None,
            completion_return: None,
            completion_config: config.completion,
            prompt_history: PromptHistoryStore::new(),
            recording_keyboard_macro: None,
            last_keyboard_macro: None,
            replaying_keyboard_macro: false,
            universal_argument: None,
            search: None,
            query_replace: None,
            rectangle_number_prompt: None,
            shell_command_prompt: None,
            pending_kill_buffer: None,
            save_some_buffers: None,
            pending_register: None,
            quoted_insert: false,
            region: None,
            registers: HashMap::new(),
            kill_ring: Vec::new(),
            yank_state: None,
            recenter_cycle_index: 0,
            window_line_cycle_index: 0,
            last_command_was_kill: false,
            kill_recorded_this_command: false,
            undo_stack: Vec::new(),
            grouping_insert: false,
            syntax_enabled: config.syntax_highlighting,
            search_highlighting: config.search_highlighting,
            line_numbers: config.line_numbers,
            tab_width: config.tab_width,
            fill_column: config.fill_column,
            backup_on_save: config.backup_on_save,
            theme: config.theme,
        }
    }

    pub fn document(&self) -> &Document {
        self.buffers
            .document(self.current_buffer)
            .expect("current buffer must exist")
    }

    pub fn current_buffer_id(&self) -> BufferId {
        self.current_buffer
    }

    pub fn current_buffer_name(&self) -> &str {
        self.buffers
            .name(self.current_buffer)
            .expect("current buffer must exist")
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    pub fn current_window_id(&self) -> WindowId {
        self.windows.current_id()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn window_layouts(&self, rows: usize, columns: usize) -> Vec<WindowLayout> {
        self.windows.layouts(rows, columns)
    }

    pub fn window_viewport(&self, id: WindowId) -> Option<&Viewport> {
        self.windows.window(id).map(|window| window.viewport())
    }

    pub fn set_window_text_rows(&mut self, id: WindowId, text_rows: usize) {
        if let Some(window) = self.windows.window_mut(id) {
            window.viewport_mut().text_rows = text_rows.max(1);
        }
    }

    pub fn ensure_current_window_contains_cursor(
        &mut self,
        text_rows: usize,
        text_columns: usize,
        cursor_display_column: usize,
    ) {
        self.sync_current_window();
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.text_rows = text_rows.max(1);

        if text_rows > 0 {
            if self.cursor.line < viewport.first_visible_line {
                viewport.first_visible_line = self.cursor.line;
            } else if self.cursor.line >= viewport.first_visible_line + text_rows {
                viewport.first_visible_line = self.cursor.line + 1 - text_rows;
            }
        }

        if text_columns > 0 {
            // Match Emacs' default hscroll-margin=5 and hscroll-step=0.
            const HORIZONTAL_SCROLL_MARGIN: usize = 5;
            let margin = HORIZONTAL_SCROLL_MARGIN.min(text_columns.saturating_sub(1));
            let right_margin = viewport
                .first_visible_column
                .saturating_add(text_columns.saturating_sub(margin));
            let should_scroll = cursor_display_column < viewport.first_visible_column
                || cursor_display_column
                    >= viewport.first_visible_column.saturating_add(text_columns)
                || cursor_display_column < viewport.first_visible_column.saturating_add(margin)
                || cursor_display_column >= right_margin;
            if should_scroll {
                viewport.first_visible_column =
                    cursor_display_column.saturating_sub(text_columns / 2);
            }
        }
    }

    pub fn document_for_buffer(&self, id: BufferId) -> Option<&Document> {
        self.buffers.document(id)
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    pub fn minibuffer(&self) -> &MinibufferState {
        &self.minibuffer
    }

    pub(crate) fn refresh_messages_buffer(&mut self) {
        let Some(messages) = self.buffers.find_by_name("*Messages*") else {
            return;
        };
        let text = self.minibuffer.messages_text();
        let Some(document) = self.buffers.document(messages) else {
            return;
        };
        if document.buffer().serialize() == text {
            return;
        }
        if let Some(document) = self.buffers.document_mut(messages) {
            *document = Document::messages(text);
        }
    }

    pub fn minibuffer_display_text(&self) -> Option<String> {
        let Some(completion) = &self.completion else {
            return self.minibuffer.display_text();
        };
        if completion.style() == CompletionStyle::Vertical {
            let text = self.minibuffer.display_text()?;
            let selected = completion.selected_match_number().unwrap_or(0);
            return Some(format!("{selected}/{}  {text}", completion.match_count()));
        }
        if completion.style() != CompletionStyle::Ido {
            return self.minibuffer.display_text();
        }
        let prompt = self.minibuffer.prompt()?;
        if !matches!(
            prompt.kind,
            PromptKind::DescribeFunction
                | PromptKind::DescribeVariable
                | PromptKind::ExtendedCommand
                | PromptKind::FindFile
                | PromptKind::FindFileReadOnly
                | PromptKind::InsertFile
                | PromptKind::KillBuffer
                | PromptKind::SwitchToBuffer
        ) {
            return self.minibuffer.display_text();
        }
        let candidates = if completion.has_matches() {
            completion
                .view_items()
                .into_iter()
                .map(|item| item.candidate.value.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            "No match".to_owned()
        };
        Some(format!(
            "{}{}  [{}]",
            prompt.label, prompt.input, candidates
        ))
    }

    pub fn completion(&self) -> Option<&CompletionSession> {
        self.completion.as_ref()
    }

    pub fn syntax_enabled(&self) -> bool {
        self.syntax_enabled
    }

    pub fn search_highlighting(&self) -> bool {
        self.search_highlighting
    }

    pub fn line_numbers(&self) -> bool {
        self.line_numbers
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
    }

    pub fn theme(&self) -> ThemeName {
        self.theme
    }

    pub fn syntax_mode_for_buffer(&self, id: BufferId) -> SyntaxMode {
        self.major_mode_for_buffer(id).syntax_mode()
    }

    pub fn major_mode_for_buffer(&self, id: BufferId) -> MajorMode {
        self.buffers
            .document(id)
            .map(|document| MajorMode::for_path(document.path()))
            .unwrap_or(MajorMode::Fundamental)
    }

    fn active_modes_for_document(&self, document: &Document) -> ActiveModes {
        let major = MajorMode::for_path(document.path());
        let syntax = major.syntax_mode();
        let mut minor = Vec::new();
        if self.line_numbers {
            minor.push(ModeId::LineNumbers);
        }
        if self.syntax_enabled {
            minor.push(ModeId::SyntaxHighlighting);
        }
        if self.search_highlighting {
            minor.push(ModeId::SearchHighlighting);
        }
        ActiveModes {
            major: ModeId::for_major_mode(major),
            syntax: ModeId::for_syntax_mode(syntax),
            special: ModeId::for_document_kind(document.kind()),
            minor,
        }
    }

    fn current_buffer_description(&self) -> BufferDescription {
        let document = self.document();
        let point_column = document
            .buffer()
            .display_column(self.cursor)
            .unwrap_or(self.cursor.byte);
        BufferDescription {
            name: self.current_buffer_name().to_owned(),
            path: document.path().map(|path| path.display().to_string()),
            kind: document_kind_label(document.kind()),
            modified: document.is_dirty(),
            read_only: document.is_read_only(),
            point_line: self.cursor.line + 1,
            point_column,
            encoding: "UTF-8",
            line_ending: "LF",
            final_newline: document.buffer().final_newline(),
            modes: self.active_modes_for_document(document),
        }
    }

    pub fn spans_for_buffer_line(
        &self,
        buffer: BufferId,
        line_index: usize,
        line: &str,
    ) -> Vec<Span> {
        let syntax = SyntaxDecorator {
            enabled: self.syntax_enabled,
            mode: self.syntax_mode_for_buffer(buffer),
        };
        if buffer != self.current_buffer {
            let providers: [&dyn DecorationProvider; 1] = [&syntax];
            return collect_spans_for_line(&providers, line_index, line);
        }

        let region = RegionDecorator {
            range: self.active_region_range(),
            rectangle: self.active_rectangle_bounds(),
        };
        let query_replace = QueryReplaceDecorator {
            enabled: self.search_highlighting,
            current: self.query_replace.as_ref().and_then(|state| state.current),
        };
        let search = SearchDecorator {
            enabled: self.search_highlighting,
            search: self.search.as_ref(),
            query: self.minibuffer.prompt_input(),
        };
        let providers: [&dyn DecorationProvider; 4] = [&syntax, &region, &query_replace, &search];
        collect_spans_for_line(&providers, line_index, line)
    }

    pub fn region_highlights_line_end_space(&self, buffer: BufferId, line_index: usize) -> bool {
        let Some(region) = self.region else {
            return false;
        };
        if !region.active || region.buffer != buffer || region.shape != RegionShape::Linear {
            return false;
        }
        let Some(range) = self.active_region_range() else {
            return false;
        };
        line_index >= range.start.line && line_index < range.end.line
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        self.record_keyboard_macro_key(&key);

        if self.minibuffer.prompt().is_some() {
            self.reset_recenter_cycle();
            return self.handle_prompt_key(key);
        }

        self.clear_transient_message();

        if self.query_replace.is_some() {
            self.reset_recenter_cycle();
            return self.handle_query_replace_key(key);
        }

        if self.quoted_insert {
            self.reset_recenter_cycle();
            return self.handle_quoted_insert_key(key);
        }

        if key == KeyEvent::Ctrl('g') {
            self.reset_recenter_cycle();
            self.quit_current_operation();
            return Ok(EditorOutcome::Continue);
        }

        if self.pending_register.is_some() {
            self.reset_recenter_cycle();
            return self.handle_pending_register_key(key);
        }

        if self.describe_key.is_some() {
            self.reset_recenter_cycle();
            return Ok(self.handle_describe_key(key));
        }

        if !self.key_sequence.is_empty() {
            return self.handle_bound_key(key);
        }

        if self.universal_argument.is_some() && self.handle_universal_argument_key(&key) {
            return Ok(EditorOutcome::Continue);
        }

        if self.active_keymaps().resolve(std::slice::from_ref(&key)) != KeyStackResolution::NoMatch
        {
            return self.handle_bound_key(key);
        }

        match key {
            KeyEvent::Special(SpecialKey::Escape) => {
                self.clear_key_sequence();
                self.universal_argument = None;
                self.clear_insert_group();
                self.reset_recenter_cycle();
                self.last_command_was_kill = false;
                self.yank_state = None;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                self.clear_key_sequence();
                self.reset_recenter_cycle();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument(&text, true, argument)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Enter) => {
                self.clear_key_sequence();
                self.reset_recenter_cycle();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument("\n", false, argument)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => {
                self.clear_key_sequence();
                self.reset_recenter_cycle();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument("\t", false, argument)?;
                Ok(EditorOutcome::Continue)
            }
            key => self.handle_bound_key(key),
        }
    }

    pub fn poll_auto_revert(&mut self) -> Result<bool> {
        if self.minibuffer.prompt().is_some() {
            return Ok(false);
        }
        let enabled_buffers = self.auto_revert_buffers.clone();
        let mut reverted = Vec::new();
        for entry in self.buffers.entries_mut() {
            let id = entry.id();
            if !self.global_auto_revert && !enabled_buffers.contains(&id) {
                continue;
            }
            let document = entry.document_mut();
            if document.kind() != DocumentKind::Normal
                || document.path().is_none()
                || document.is_dirty()
                || !document.file_changed_on_disk()?
            {
                continue;
            }
            document.reload_from_disk()?;
            reverted.push(id);
        }

        if reverted.is_empty() {
            return Ok(false);
        }
        self.undo_stack
            .retain(|entry| !reverted.contains(&entry.buffer));
        for buffer in &reverted {
            if *buffer == self.current_buffer {
                self.cursor = clamp_position_to_buffer(self.document().buffer(), self.cursor);
                self.sync_current_window();
            } else if let Some(document) = self.buffers.document(*buffer)
                && let Some(viewport) = self.buffer_viewports.get_mut(buffer)
            {
                viewport.cursor = clamp_position_to_buffer(document.buffer(), viewport.cursor);
            }
        }
        self.refresh_visible_buffer_list();
        if reverted.len() == 1 {
            let name = self
                .buffers
                .name(reverted[0])
                .unwrap_or("buffer")
                .to_owned();
            self.minibuffer
                .set_message(format!("Reverted {name} from disk"));
        } else {
            self.minibuffer
                .set_message(format!("Reverted {} buffers from disk", reverted.len()));
        }
        Ok(true)
    }

    pub fn execute_command_by_name(&mut self, name: &str) -> Result<EditorOutcome> {
        let Some(command) = self.commands.get(name) else {
            self.minibuffer
                .set_message(format!("No such command: {name}"));
            return Ok(EditorOutcome::Continue);
        };

        self.execute_command_spec(command)
    }

    fn execute_command_by_id(&mut self, id: Command) -> Result<EditorOutcome> {
        let Some(command) = self.commands.get_by_id(id) else {
            self.minibuffer
                .set_message(format!("No such command: {id:?}"));
            return Ok(EditorOutcome::Continue);
        };

        self.execute_command_spec(command)
    }

    fn execute_command_spec(&mut self, command: CommandSpec) -> Result<EditorOutcome> {
        let argument = if command.command == Command::UniversalArgument {
            None
        } else {
            self.take_universal_argument()
        };
        self.execute_command(command, argument)
    }

    fn handle_bound_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if !self.key_sequence.is_empty() && is_key_prefix_help(&key) {
            return Ok(self.show_key_prefix_help());
        }

        self.key_sequence.push(key);

        let resolution = self.active_keymaps().resolve(&self.key_sequence);
        match resolution {
            KeyStackResolution::NoMatch => {
                self.clear_key_sequence();
                self.universal_argument = None;
                self.reset_recenter_cycle();
                self.last_command_was_kill = false;
                self.yank_state = None;
                self.minibuffer.set_message("Key is not bound");
                Ok(EditorOutcome::Continue)
            }
            KeyStackResolution::Prefix => {
                self.minibuffer
                    .set_message(format_key_prefix_message(&self.key_sequence));
                Ok(EditorOutcome::Continue)
            }
            KeyStackResolution::Command { command, .. } => {
                let command_sequence = self.key_sequence.clone();
                self.clear_key_sequence();
                self.current_command_sequence = Some(command_sequence);
                let result = self.execute_command_by_id(command);
                self.current_command_sequence = None;
                result
            }
        }
    }

    fn active_keymaps(&self) -> KeyMapStack<'_> {
        match self.document().kind() {
            DocumentKind::Help => KeyMapStack::new([&self.help_keymap, &self.keymap]),
            DocumentKind::Messages => KeyMapStack::new([&self.messages_keymap, &self.keymap]),
            DocumentKind::ShellOutput => {
                KeyMapStack::new([&self.shell_output_keymap, &self.keymap])
            }
            DocumentKind::BufferList => KeyMapStack::new([&self.buffer_list_keymap, &self.keymap]),
            DocumentKind::Normal | DocumentKind::Welcome | DocumentKind::Completions => {
                KeyMapStack::global(&self.keymap)
            }
        }
    }

    fn show_key_prefix_help(&mut self) -> EditorOutcome {
        let prefix = self.key_sequence.clone();
        let text = format_key_prefix_help(&self.commands, &self.active_keymaps(), &prefix);
        self.clear_key_sequence();
        self.open_help_buffer(text)
    }

    fn open_help_buffer(&mut self, text: impl AsRef<str>) -> EditorOutcome {
        self.sync_current_window();
        let current_is_help = self.document().is_help();
        let current_viewport = *self.windows.current().viewport();
        remember_returnable_special_buffer_return(
            &mut self.help_return,
            current_viewport,
            current_is_help,
        );
        let help = self.buffers.open_help(text);

        self.show_returnable_special_buffer(help);
        self.minibuffer
            .set_message("Type q in help window to restore previous buffer.");

        EditorOutcome::Continue
    }

    fn restore_help_buffer(&mut self) -> EditorOutcome {
        let Some(viewport) = self.help_return.take() else {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        };
        if self.buffers.document(viewport.buffer).is_none() {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        }

        self.restore_returnable_special_buffer(viewport, true);

        EditorOutcome::Continue
    }

    fn open_messages_buffer(&mut self) -> Result<()> {
        self.sync_current_window();
        let current_is_messages = self.document().is_messages();
        let current_viewport = *self.windows.current().viewport();
        remember_returnable_special_buffer_return(
            &mut self.messages_return,
            current_viewport,
            current_is_messages,
        );
        let messages = self.buffers.open_messages(self.minibuffer.messages_text());

        self.show_returnable_special_buffer(messages);
        self.minibuffer
            .set_message("Type q in messages window to restore previous buffer.");
        Ok(())
    }

    fn restore_messages_buffer(&mut self) -> EditorOutcome {
        let Some(viewport) = self.messages_return.take() else {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        };
        if self.buffers.document(viewport.buffer).is_none() {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        }

        self.restore_returnable_special_buffer(viewport, true);

        EditorOutcome::Continue
    }

    fn open_shell_output_buffer(&mut self, text: impl AsRef<str>) -> EditorOutcome {
        self.sync_current_window();
        let current_is_shell_output = self.document().is_shell_output();
        let current_viewport = *self.windows.current().viewport();
        remember_returnable_special_buffer_return(
            &mut self.shell_output_return,
            current_viewport,
            current_is_shell_output,
        );
        let output = self.buffers.open_shell_output(text);

        self.show_returnable_special_buffer(output);

        EditorOutcome::Continue
    }

    fn restore_shell_output_buffer(&mut self) -> EditorOutcome {
        let Some(viewport) = self.shell_output_return.take() else {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        };
        if self.buffers.document(viewport.buffer).is_none() {
            self.minibuffer.set_message("No previous buffer");
            return EditorOutcome::Continue;
        }

        self.restore_returnable_special_buffer(viewport, true);

        EditorOutcome::Continue
    }

    fn show_returnable_special_buffer(&mut self, buffer: BufferId) {
        self.current_buffer = buffer;
        self.cursor = Position::new(0, 0);
        self.clear_returnable_special_buffer_state();
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.first_visible_line = 0;
        viewport.first_visible_column = 0;
        self.sync_current_window();
    }

    fn restore_returnable_special_buffer(&mut self, viewport: Viewport, clear_minibuffer: bool) {
        self.current_buffer = viewport.buffer;
        self.cursor = viewport.cursor;
        self.clear_returnable_special_buffer_state();
        *self.windows.current_mut().viewport_mut() = viewport;
        if clear_minibuffer {
            self.minibuffer.clear();
        }
    }

    fn clear_returnable_special_buffer_state(&mut self) {
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
    }

    fn close_buffer_list_window(&mut self) -> EditorOutcome {
        if self.windows.len() > 1 {
            self.sync_current_window();
            self.windows.delete_current();
            self.load_current_window();
            return EditorOutcome::Continue;
        }

        let fallback = self
            .buffers
            .entries()
            .iter()
            .find(|entry| !entry.document().is_buffer_list())
            .map(|entry| entry.id());
        let buffer = fallback.unwrap_or_else(|| {
            self.buffers
                .kill(self.current_buffer)
                .expect("scratch fallback should be created")
        });
        self.current_buffer = buffer;
        self.cursor = Position::new(0, 0);
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        self.sync_current_window();

        EditorOutcome::Continue
    }

    fn open_selected_buffer_list_entry(&mut self) -> EditorOutcome {
        let Some(buffer) = self
            .buffer_list_rows
            .get(self.cursor.line)
            .copied()
            .flatten()
        else {
            self.minibuffer.set_message("No buffer on this line");
            return EditorOutcome::Continue;
        };

        if self.buffers.document(buffer).is_none() {
            self.minibuffer.set_error("buffer no longer exists");
            return EditorOutcome::Continue;
        }

        self.sync_current_window();
        self.restore_buffer_in_current_window(buffer);
        self.remember_buffer_transition(buffer);
        self.current_buffer = buffer;
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        self.refresh_visible_buffer_list();
        let name = self.current_buffer_name().to_owned();
        self.minibuffer
            .set_message(format!("Switched to buffer {name}"));

        EditorOutcome::Continue
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if self.minibuffer.prompt_kind() == Some(PromptKind::IncrementalSearch) {
            return self.handle_search_prompt_key(key);
        }
        if self.minibuffer.prompt_kind() == Some(PromptKind::KillDirtyBuffer) {
            return Ok(self.handle_kill_dirty_buffer_key(key));
        }
        if self.completion.is_some() {
            return self.handle_completion_prompt_key(key);
        }

        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                self.prompt_history.record(kind, &input);
                self.minibuffer.clear();
                self.submit_prompt(kind, &input)
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
                self.reset_current_prompt_history_navigation();
                self.keyboard_macro_prompt_start = None;
                if matches!(
                    self.minibuffer.prompt_kind(),
                    Some(PromptKind::QueryReplaceSearch | PromptKind::QueryReplaceReplacement)
                ) {
                    self.query_replace = None;
                }
                if matches!(
                    self.minibuffer.prompt_kind(),
                    Some(PromptKind::RectangleNumberFormat | PromptKind::RectangleNumberStart)
                ) {
                    self.rectangle_number_prompt = None;
                }
                if self.minibuffer.prompt_kind() == Some(PromptKind::ShellCommand) {
                    self.shell_command_prompt = None;
                }
                if self.minibuffer.prompt_kind() == Some(PromptKind::KillDirtyBuffer) {
                    self.pending_kill_buffer = None;
                }
                if self.minibuffer.prompt_kind() == Some(PromptKind::SaveSomeBuffers) {
                    self.save_some_buffers = None;
                }
                self.minibuffer.cancel_prompt();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                if self.minibuffer.delete_prompt_grapheme_backward() {
                    self.reset_current_prompt_history_navigation();
                }
                self.record_prompt_non_kill_key();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Meta('p') => {
                self.recall_prompt_history(-1);
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Meta('n') => {
                self.recall_prompt_history(1);
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                self.minibuffer.insert_prompt_text(&text);
                self.reset_current_prompt_history_navigation();
                self.record_prompt_non_kill_key();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => Ok(EditorOutcome::Continue),
            key => {
                if let PromptEditOutcome::Handled { changed } = self.handle_prompt_edit_key(&key)
                    && changed
                {
                    self.reset_current_prompt_history_navigation();
                }
                Ok(EditorOutcome::Continue)
            }
        }
    }

    fn handle_kill_dirty_buffer_key(&mut self, key: KeyEvent) -> EditorOutcome {
        match key {
            KeyEvent::Text(text) => self.submit_kill_dirty_buffer(&text),
            KeyEvent::Special(SpecialKey::Enter)
            | KeyEvent::Special(SpecialKey::Escape)
            | KeyEvent::Ctrl('g') => self.submit_kill_dirty_buffer(""),
            _ => EditorOutcome::Continue,
        }
    }

    fn handle_completion_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                if let Some(directory) =
                    directory_completion_to_enter(kind, self.completion.as_ref(), &input)
                {
                    self.prompt_history.reset(kind);
                    self.minibuffer.start_prompt(kind, prompt_label(kind));
                    self.minibuffer.set_prompt_input(directory);
                    self.update_completion_from_prompt();
                    return Ok(EditorOutcome::Continue);
                }
                let input = self.completion_accept_input(kind, &input);
                self.prompt_history.record(kind, &input);
                self.minibuffer.clear();
                self.finish_completion_buffer();
                self.completion = None;
                self.submit_prompt(kind, &input)
            }
            KeyEvent::MetaSpecial(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                let input = raw_completion_input(&input);
                self.prompt_history.record(kind, &input);
                self.minibuffer.clear();
                self.finish_completion_buffer();
                self.completion = None;
                self.submit_prompt(kind, &input)
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
                self.reset_current_prompt_history_navigation();
                self.keyboard_macro_prompt_start = None;
                self.minibuffer.cancel_prompt();
                self.finish_completion_buffer();
                self.completion = None;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                if self.minibuffer.delete_prompt_grapheme_backward() {
                    self.reset_current_prompt_history_navigation();
                    self.update_completion_from_prompt();
                }
                self.record_prompt_non_kill_key();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => {
                self.complete_prompt_common_prefix();
                self.reset_current_prompt_history_navigation();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Meta('p') => {
                self.recall_prompt_history(-1);
                self.update_completion_from_prompt();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Meta('n') => {
                self.recall_prompt_history(1);
                self.update_completion_from_prompt();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Ctrl('n') | KeyEvent::Special(SpecialKey::ArrowDown) => {
                if let Some(completion) = &mut self.completion {
                    completion.move_selection(1);
                }
                self.update_completion_buffer();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Ctrl('p') | KeyEvent::Special(SpecialKey::ArrowUp) => {
                if let Some(completion) = &mut self.completion {
                    completion.move_selection(-1);
                }
                self.update_completion_buffer();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Ctrl('v') | KeyEvent::Special(SpecialKey::PageDown) => {
                if let Some(completion) = &mut self.completion {
                    completion.move_selection_page(1);
                }
                self.update_completion_buffer();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Meta('v') | KeyEvent::Special(SpecialKey::PageUp) => {
                if let Some(completion) = &mut self.completion {
                    completion.move_selection_page(-1);
                }
                self.update_completion_buffer();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                self.insert_completion_prompt_text(&text);
                self.reset_current_prompt_history_navigation();
                self.update_completion_from_prompt();
                self.record_prompt_non_kill_key();
                Ok(EditorOutcome::Continue)
            }
            key => {
                if let PromptEditOutcome::Handled { changed } = self.handle_prompt_edit_key(&key)
                    && changed
                {
                    self.reset_current_prompt_history_navigation();
                    self.update_completion_from_prompt();
                }
                Ok(EditorOutcome::Continue)
            }
        }
    }

    fn handle_prompt_edit_key(&mut self, key: &KeyEvent) -> PromptEditOutcome {
        match key {
            KeyEvent::Ctrl('f') | KeyEvent::Special(SpecialKey::ArrowRight) => {
                self.minibuffer.move_prompt_grapheme_forward();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Ctrl('b') | KeyEvent::Special(SpecialKey::ArrowLeft) => {
                self.minibuffer.move_prompt_grapheme_backward();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Meta('f') => {
                self.minibuffer.move_prompt_word_forward();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Meta('b') => {
                self.minibuffer.move_prompt_word_backward();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Ctrl('a') | KeyEvent::Special(SpecialKey::Home) => {
                self.minibuffer.move_prompt_start();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Ctrl('e') | KeyEvent::Special(SpecialKey::End) => {
                self.minibuffer.move_prompt_end();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed: false }
            }
            KeyEvent::Ctrl('d') | KeyEvent::Special(SpecialKey::Delete) => {
                let changed = self.minibuffer.delete_prompt_grapheme_forward();
                self.record_prompt_non_kill_key();
                PromptEditOutcome::Handled { changed }
            }
            KeyEvent::Ctrl('k') => {
                let killed = self.minibuffer.delete_prompt_to_end();
                self.record_prompt_kill(killed, KillDirection::Forward)
            }
            KeyEvent::Meta('d') => {
                let killed = self.minibuffer.delete_prompt_word_forward();
                self.record_prompt_kill(killed, KillDirection::Forward)
            }
            KeyEvent::MetaSpecial(SpecialKey::Backspace)
            | KeyEvent::CtrlSpecial(SpecialKey::Backspace) => {
                let killed = self.minibuffer.delete_prompt_word_backward();
                self.record_prompt_kill(killed, KillDirection::Backward)
            }
            _ => PromptEditOutcome::Unhandled,
        }
    }

    fn insert_completion_prompt_text(&mut self, text: &str) {
        if text.starts_with(MAIN_SEPARATOR) && self.file_prompt_is_at_default_base() {
            self.minibuffer.set_prompt_input(text.to_owned());
        } else {
            self.minibuffer.insert_prompt_text(text);
        }
    }

    fn file_prompt_is_at_default_base(&self) -> bool {
        if !matches!(
            self.minibuffer.prompt_kind(),
            Some(PromptKind::FindFile | PromptKind::FindFileReadOnly | PromptKind::InsertFile)
        ) {
            return false;
        }
        let Some(input) = self.minibuffer.prompt_input() else {
            return false;
        };
        self.minibuffer.prompt_input_before_cursor() == Some(input)
            && input == file_prompt_base_input(&self.find_file_base_dir())
    }

    fn record_prompt_non_kill_key(&mut self) {
        self.last_command_was_kill = false;
        self.kill_recorded_this_command = false;
        self.yank_state = None;
    }

    fn record_prompt_kill(
        &mut self,
        killed: Option<String>,
        direction: KillDirection,
    ) -> PromptEditOutcome {
        let Some(text) = killed else {
            self.record_prompt_non_kill_key();
            return PromptEditOutcome::Handled { changed: false };
        };
        self.yank_state = None;
        self.push_command_kill(KillEntry::Text(text), direction);
        self.last_command_was_kill = true;
        self.kill_recorded_this_command = false;
        PromptEditOutcome::Handled { changed: true }
    }

    fn completion_accept_input(&self, kind: PromptKind, input: &str) -> String {
        let trimmed = input.trim();
        accepted_completion_input(CompletionAcceptContext {
            kind,
            input,
            completion: self.completion.as_ref(),
            command_exists: self.commands.contains(trimmed),
            option_exists: OptionRegistry::default().contains(trimmed),
            exact_file_exists: self.find_file_input_is_exact_file(trimmed),
            buffer_exists: self.buffers.find_by_name(input).is_some(),
            switch_buffer_default: self.switch_buffer_default_name(),
        })
    }

    fn recall_prompt_history(&mut self, direction: isize) {
        let Some(kind) = self.minibuffer.prompt_kind() else {
            return;
        };
        let current = self
            .minibuffer
            .prompt_input()
            .unwrap_or_default()
            .to_owned();
        if let Some(input) = self.prompt_history.recall(kind, &current, direction) {
            self.minibuffer.set_prompt_input(input);
        }
    }

    fn reset_current_prompt_history_navigation(&mut self) {
        if let Some(kind) = self.minibuffer.prompt_kind() {
            self.prompt_history.reset(kind);
        }
    }

    fn complete_prompt_common_prefix(&mut self) {
        let input = self
            .minibuffer
            .prompt_input()
            .unwrap_or_default()
            .to_owned();
        let Some(completion) = self.completion.as_ref() else {
            return;
        };
        if let Some(next_input) = tab_completion_input(completion, &input) {
            self.minibuffer.set_prompt_input(next_input);
            self.update_completion_from_prompt();
        }
    }

    fn update_completion_from_prompt(&mut self) {
        let input = self
            .minibuffer
            .prompt_input()
            .unwrap_or_default()
            .to_owned();
        if let Some(completion) = &mut self.completion {
            completion.update(&input);
        }
        self.update_completion_buffer();
    }

    fn update_completion_buffer(&mut self) {
        let Some(completion) = &self.completion else {
            return;
        };
        if completion.style() != CompletionStyle::CompletionsBuffer {
            return;
        }
        let text = format_completion_buffer(completion);
        if self.completion_return.is_none() {
            self.sync_current_window();
            self.completion_return = Some(*self.windows.current().viewport());
        }
        let completions = self.buffers.open_completions(text);
        self.show_returnable_special_buffer(completions);
    }

    fn finish_completion_buffer(&mut self) {
        let Some(viewport) = self.completion_return.take() else {
            return;
        };
        if self.buffers.document(viewport.buffer).is_none() {
            return;
        }
        self.restore_returnable_special_buffer(viewport, false);
    }

    fn submit_prompt(&mut self, kind: PromptKind, input: &str) -> Result<EditorOutcome> {
        match kind {
            PromptKind::DescribeFunction => Ok(self.describe_function(input.trim())),
            PromptKind::DescribeVariable => Ok(self.describe_variable(input.trim())),
            PromptKind::ExtendedCommand => self.submit_extended_command(input.trim()),
            PromptKind::FindFile => self.find_file(input.trim()),
            PromptKind::FindFileReadOnly => self.find_file_read_only(input.trim()),
            PromptKind::GotoLine => self.goto_line(input.trim()),
            PromptKind::InsertFile => self.insert_file(input.trim()),
            PromptKind::IncrementalSearch => Ok(EditorOutcome::Continue),
            PromptKind::KillBuffer => self.kill_buffer(input),
            PromptKind::KillDirtyBuffer => Ok(self.submit_kill_dirty_buffer(input.trim())),
            PromptKind::QueryReplaceReplacement => self.submit_query_replace_replacement(input),
            PromptKind::QueryReplaceSearch => self.submit_query_replace_search(input),
            PromptKind::RevertBuffer => Ok(self.submit_revert_buffer(input.trim())),
            PromptKind::SaveSomeBuffers => Ok(self.submit_save_some_buffers(input.trim())),
            PromptKind::QuitDirtyBuffers => Ok(self.submit_quit_dirty_buffers(input.trim())),
            PromptKind::RectangleNumberFormat => self.submit_rectangle_number_format(input),
            PromptKind::RectangleNumberStart => self.submit_rectangle_number_start(input.trim()),
            PromptKind::ShellCommand => self.submit_shell_command(input.trim()),
            PromptKind::StringRectangle => self.submit_string_rectangle(input),
            PromptKind::SwitchToBuffer => self.switch_to_buffer(input),
            PromptKind::WriteFile => self.write_file(input.trim()),
        }
    }

    fn submit_extended_command(&mut self, name: &str) -> Result<EditorOutcome> {
        if self
            .commands
            .get(name)
            .is_some_and(|spec| is_keyboard_macro_control_command(spec.command))
        {
            self.trim_keyboard_macro_prompt_invocation();
        } else {
            self.clear_keyboard_macro_prompt_start();
        }
        self.execute_command_by_name(name)
    }

    fn execute_command(
        &mut self,
        command: CommandSpec,
        argument: Option<i32>,
    ) -> Result<EditorOutcome> {
        let command_id = command.command;
        if command_id != Command::Recenter {
            self.reset_recenter_cycle();
        }
        if command_id != Command::MoveToWindowLineTopBottom {
            self.reset_window_line_cycle();
        }
        let kill_command = is_kill_command(command_id);
        let yank_command = is_yank_command(command_id);
        if kill_command {
            self.kill_recorded_this_command = false;
        } else {
            self.last_command_was_kill = false;
        }
        if !yank_command {
            self.yank_state = None;
        }

        let handler = command
            .handler
            .expect("registered commands should have handlers");
        let context = self.command_context(argument);
        let outcome = handler(self, context)?;

        if kill_command {
            self.last_command_was_kill = self.kill_recorded_this_command;
            self.kill_recorded_this_command = false;
        }

        Ok(editor_outcome_for_command_outcome(outcome))
    }

    fn command_context(&self, argument: Option<i32>) -> CommandContext {
        let invoked_by = match &self.current_command_sequence {
            Some(sequence) => Invocation::Key(sequence.clone()),
            None => Invocation::ExtendedCommand,
        };

        CommandContext {
            argument,
            invoked_by,
        }
    }

    pub(crate) fn command_about_rile(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        let text = format_about_rile_help(&AboutRileInfo::current());
        self.open_help_buffer(text);
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_auto_revert_mode(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.toggle_auto_revert_mode();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_global_auto_revert_mode(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.global_auto_revert = !self.global_auto_revert;
        let state = if self.global_auto_revert { "on" } else { "off" };
        self.minibuffer
            .set_message(format!("Global auto-revert is {state}"));
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_back_to_indentation(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_back_to_indentation()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_backward_char(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(context.argument, Self::move_backward, Self::move_forward)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_backward_kill_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed_kill(context.argument, Self::backward_kill_word, Self::kill_word)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_backward_paragraph(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_paragraph_backward,
            Self::move_paragraph_forward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_backward_sentence(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_sentence_backward,
            Self::move_sentence_forward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_backward_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_word_backward,
            Self::move_word_forward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_beginning_of_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_beginning_of_buffer()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_beginning_of_line(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_beginning_of_line()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_buffer_list_select(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.open_selected_buffer_list_entry();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_call_last_keyboard_macro(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.call_last_keyboard_macro(context.argument)
            .map(command_outcome_for_editor_outcome)
    }

    pub(crate) fn command_capitalize_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.case_word(context.argument, CaseTransform::Capitalize)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_clear_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.clear_rectangle()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_comment_dwim(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.comment_dwim()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_comment_region(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.comment_region()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_copy_region_as_kill(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.copy_region_as_kill()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_copy_rectangle_as_kill(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.copy_rectangle_as_kill()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_copy_rectangle_to_register(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_copy_rectangle_to_register()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_copy_to_register(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_copy_to_register()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_delete_backward_char(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::delete_backward_char,
            Self::delete_char,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_blank_lines(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_blank_lines()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_char(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::delete_char,
            Self::delete_backward_char,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_horizontal_space(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_horizontal_space(context.argument.is_some())?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_trailing_whitespace(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_trailing_whitespace()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_other_windows(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_other_windows()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_window(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_window()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_downcase_region(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.case_region(CaseTransform::Lower)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_downcase_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.case_word(context.argument, CaseTransform::Lower)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_delete_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.delete_rectangle_command()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_describe_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        let registry = ModeRegistry::default();
        let description = self.current_buffer_description();
        let text = format_describe_buffer_help(&description, &registry);
        self.open_help_buffer(text);
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_describe_function(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_describe_function()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_describe_key(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_describe_key(DescribeKeyMode::Help)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_describe_key_briefly(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_describe_key(DescribeKeyMode::Brief)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_describe_mode(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        let registry = ModeRegistry::default();
        let modes = self.active_modes_for_document(self.document());
        let text = format_describe_mode_help(&modes, &registry);
        self.open_help_buffer(text);
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_describe_variable(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_describe_variable()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_describe_bindings(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        let text = format_describe_bindings_help(&self.commands, &self.active_keymaps());
        self.open_help_buffer(text);
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_end_keyboard_macro(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.end_keyboard_macro()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_end_or_call_keyboard_macro(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        if self.recording_keyboard_macro.is_some() {
            self.end_keyboard_macro()?;
            Ok(CommandOutcome::Continue)
        } else {
            self.call_last_keyboard_macro(context.argument)
                .map(command_outcome_for_editor_outcome)
        }
    }

    pub(crate) fn command_end_of_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_end_of_buffer()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_end_of_line(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_end_of_line()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_exchange_point_and_mark(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.exchange_point_and_mark()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_execute_extended_command(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_extended_command()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_find_file(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.start_find_file()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_find_file_read_only(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_find_file_read_only()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_fill_paragraph(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.fill_paragraph()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_forward_char(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(context.argument, Self::move_forward, Self::move_backward)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_forward_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_word_forward,
            Self::move_word_backward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_forward_paragraph(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_paragraph_forward,
            Self::move_paragraph_backward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_forward_sentence(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_signed(
            context.argument,
            Self::move_sentence_forward,
            Self::move_sentence_backward,
        )?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_move_to_window_line_top_bottom(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_to_window_line_top_bottom()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_goto_line(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.start_goto_line()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_increment_register(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_increment_register(context.argument)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_incremental_search_backward(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_incremental_search(SearchDirection::Backward)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_incremental_search_forward(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_incremental_search(SearchDirection::Forward)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_insert_file(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_insert_file()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_insert_register(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_insert_register()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_join_line(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.join_line()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_just_one_space(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.just_one_space(context.argument)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_jump_to_register(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_jump_to_register()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_kill_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_kill_buffer()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_kill_line(&mut self, context: CommandContext) -> Result<CommandOutcome> {
        self.repeat_positive_kill(context.argument, Self::kill_line)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_kill_region(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.kill_region()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_not_modified(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.not_modified()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_kill_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.kill_rectangle_command()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_kill_word(&mut self, context: CommandContext) -> Result<CommandOutcome> {
        self.repeat_signed_kill(context.argument, Self::kill_word, Self::backward_kill_word)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_list_buffers(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.list_buffers()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_mark_whole_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.mark_whole_buffer()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_newline_and_indent(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.repeat_positive(context.argument, Self::newline_and_indent)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_next_line(&mut self, context: CommandContext) -> Result<CommandOutcome> {
        self.move_line_by_argument(context.argument, 1)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_number_to_register(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_number_to_register(context.argument)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_open_line(&mut self, context: CommandContext) -> Result<CommandOutcome> {
        self.repeat_positive(context.argument, Self::open_line)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_open_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.open_rectangle()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_other_window(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.other_window()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_previous_line(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.move_line_by_argument(context.argument, -1)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_point_to_register(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_point_to_register()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_query_replace(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_query_replace()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_quoted_insert(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_quoted_insert()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_quit_buffer_list(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.close_buffer_list_window();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_quit_help_window(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.restore_help_buffer();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_quit_messages_window(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.restore_messages_buffer();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_quit_shell_output_window(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.restore_shell_output_buffer();
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_rectangle_mark_mode(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.rectangle_mark_mode()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_rectangle_number_lines(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.rectangle_number_lines(context.argument)?;
        if context.argument.is_some() {
            Ok(CommandOutcome::StartedPrompt)
        } else {
            Ok(CommandOutcome::Continue)
        }
    }

    pub(crate) fn command_recenter(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.recenter()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_revert_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        Ok(command_outcome_for_editor_outcome(
            self.start_revert_buffer()?,
        ))
    }

    pub(crate) fn command_save_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.save_buffer()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_save_some_buffers(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        Ok(command_outcome_for_editor_outcome(
            self.start_save_some_buffers(),
        ))
    }

    pub(crate) fn command_save_buffers_kill_terminal(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        Ok(command_outcome_for_editor_outcome(
            self.save_buffers_kill_terminal(),
        ))
    }

    pub(crate) fn command_scroll_page_backward(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.scroll_page_backward()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_scroll_page_forward(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.scroll_page_forward()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_set_mark_command(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.set_mark_command()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_shell_command(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_shell_command(context.argument)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_shell_command_on_region(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_shell_command_on_region(context.argument)?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_start_keyboard_macro(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_keyboard_macro()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_string_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_string_rectangle()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_split_window_below(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.split_window(SplitAxis::Horizontal)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_split_window_right(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.split_window(SplitAxis::Vertical)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_suspend_frame(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        Ok(CommandOutcome::Suspend)
    }

    pub(crate) fn command_switch_to_buffer(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_switch_to_buffer()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_toggle_line_numbers(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.toggle_line_numbers()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_toggle_read_only(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.toggle_read_only()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_toggle_search_highlighting(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.toggle_search_highlighting()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_toggle_syntax_highlighting(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.toggle_syntax_highlighting()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_transpose_chars(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.transpose_chars(context.argument)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_transpose_lines(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.transpose_lines(context.argument)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_transpose_words(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.transpose_words(context.argument)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_undo(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.undo()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_uncomment_region(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.uncomment_region()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_universal_argument(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.extend_universal_argument()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_upcase_region(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.case_region(CaseTransform::Upper)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_upcase_word(
        &mut self,
        context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.case_word(context.argument, CaseTransform::Upper)?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_view_echo_area_messages(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.open_messages_buffer()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_what_cursor_position(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.what_cursor_position()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_write_file(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.start_write_file()?;
        Ok(CommandOutcome::StartedPrompt)
    }

    pub(crate) fn command_yank(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.yank()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_yank_rectangle(
        &mut self,
        _context: CommandContext,
    ) -> Result<CommandOutcome> {
        self.yank_rectangle_command()?;
        Ok(CommandOutcome::Continue)
    }

    pub(crate) fn command_yank_pop(&mut self, _context: CommandContext) -> Result<CommandOutcome> {
        self.yank_pop()?;
        Ok(CommandOutcome::Continue)
    }

    fn insert_text(&mut self, text: &str, group_with_previous: bool) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        let cursor = self.cursor;
        self.cursor = self.document_mut().buffer_mut().insert(cursor, text)?;
        self.record_insert(cursor, self.cursor, text, group_with_previous);
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn insert_text_with_argument(
        &mut self,
        text: &str,
        group_with_previous: bool,
        argument: Option<i32>,
    ) -> Result<()> {
        let count = positive_argument_count(argument);
        if count == 0 {
            return Ok(());
        }
        self.insert_text(&text.repeat(count), group_with_previous)
    }

    fn repeat_positive(
        &mut self,
        argument: Option<i32>,
        action: fn(&mut Self) -> Result<()>,
    ) -> Result<()> {
        for _ in 0..positive_argument_count(argument) {
            action(self)?;
        }
        Ok(())
    }

    fn repeat_positive_kill(
        &mut self,
        argument: Option<i32>,
        action: fn(&mut Self) -> Result<()>,
    ) -> Result<()> {
        for _ in 0..positive_argument_count(argument) {
            action(self)?;
            if self.kill_recorded_this_command {
                self.last_command_was_kill = true;
            }
        }
        Ok(())
    }

    fn repeat_signed(
        &mut self,
        argument: Option<i32>,
        positive: fn(&mut Self) -> Result<()>,
        negative: fn(&mut Self) -> Result<()>,
    ) -> Result<()> {
        let argument = argument.unwrap_or(1);
        let count = argument.unsigned_abs() as usize;
        let action = if argument >= 0 { positive } else { negative };
        for _ in 0..count {
            action(self)?;
        }
        Ok(())
    }

    fn repeat_signed_kill(
        &mut self,
        argument: Option<i32>,
        positive: fn(&mut Self) -> Result<()>,
        negative: fn(&mut Self) -> Result<()>,
    ) -> Result<()> {
        let argument = argument.unwrap_or(1);
        let count = argument.unsigned_abs() as usize;
        let action = if argument >= 0 { positive } else { negative };
        for _ in 0..count {
            action(self)?;
            if self.kill_recorded_this_command {
                self.last_command_was_kill = true;
            }
        }
        Ok(())
    }

    fn move_line_by_argument(&mut self, argument: Option<i32>, direction: isize) -> Result<()> {
        let count = argument.unwrap_or(1).saturating_mul(direction as i32);
        self.move_line(count as isize)
    }

    fn extend_universal_argument(&mut self) -> Result<()> {
        if let Some(argument) = &mut self.universal_argument {
            argument.multiply();
        } else {
            self.universal_argument = Some(UniversalArgumentState::new());
        }
        self.set_universal_argument_message();
        Ok(())
    }

    fn handle_universal_argument_key(&mut self, key: &KeyEvent) -> bool {
        match key {
            KeyEvent::Ctrl('u') => {
                if let Some(argument) = &mut self.universal_argument {
                    argument.multiply();
                }
                self.set_universal_argument_message();
                true
            }
            KeyEvent::Text(text) if text.chars().count() == 1 => {
                let character = text.chars().next().expect("text must contain one char");
                if character == '-' {
                    if let Some(argument) = &mut self.universal_argument {
                        argument.negate();
                    }
                    self.set_universal_argument_message();
                    return true;
                }
                if let Some(digit) = character.to_digit(10) {
                    if let Some(argument) = &mut self.universal_argument {
                        argument.push_digit(digit);
                    }
                    self.set_universal_argument_message();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn set_universal_argument_message(&mut self) {
        let Some(argument) = self.universal_argument else {
            return;
        };
        if argument == UniversalArgumentState::new() {
            self.minibuffer.set_message("C-u-");
        } else {
            self.minibuffer
                .set_message(format!("C-u {}-", argument.value()));
        }
    }

    fn take_universal_argument(&mut self) -> Option<i32> {
        self.universal_argument
            .take()
            .map(UniversalArgumentState::value)
    }

    fn record_keyboard_macro_key(&mut self, key: &KeyEvent) {
        if self.replaying_keyboard_macro {
            return;
        }
        if let Some(keys) = &mut self.recording_keyboard_macro {
            keys.push(key.clone());
        }
    }

    fn trim_current_command_from_keyboard_macro(&mut self) {
        let Some(sequence) = &self.current_command_sequence else {
            return;
        };
        let Some(keys) = &mut self.recording_keyboard_macro else {
            return;
        };
        for _ in 0..sequence.len().min(keys.len()) {
            keys.pop();
        }
    }

    fn mark_keyboard_macro_prompt_start(&mut self) {
        let Some(sequence) = &self.current_command_sequence else {
            return;
        };
        let Some(keys) = &self.recording_keyboard_macro else {
            return;
        };
        self.keyboard_macro_prompt_start = Some(keys.len().saturating_sub(sequence.len()));
    }

    fn trim_keyboard_macro_prompt_invocation(&mut self) {
        let Some(start) = self.keyboard_macro_prompt_start.take() else {
            return;
        };
        let Some(keys) = &mut self.recording_keyboard_macro else {
            return;
        };
        keys.truncate(start.min(keys.len()));
    }

    fn clear_keyboard_macro_prompt_start(&mut self) {
        self.keyboard_macro_prompt_start = None;
    }

    fn start_keyboard_macro(&mut self) -> Result<()> {
        if self.replaying_keyboard_macro {
            self.minibuffer
                .set_error("Cannot define keyboard macro while executing one");
            return Ok(());
        }
        if self.recording_keyboard_macro.is_some() {
            self.trim_current_command_from_keyboard_macro();
            self.minibuffer.set_error("Already defining keyboard macro");
            return Ok(());
        }

        self.recording_keyboard_macro = Some(Vec::new());
        self.minibuffer.set_message("Defining keyboard macro...");
        Ok(())
    }

    fn end_keyboard_macro(&mut self) -> Result<()> {
        if self.recording_keyboard_macro.is_none() {
            self.minibuffer.set_error("Not defining keyboard macro");
            return Ok(());
        }

        self.trim_current_command_from_keyboard_macro();
        let keys = self
            .recording_keyboard_macro
            .take()
            .expect("recording macro should exist");
        if keys.is_empty() {
            self.last_keyboard_macro = None;
            self.minibuffer.set_message("Ignored empty keyboard macro");
        } else {
            let count = keys.len();
            self.last_keyboard_macro = Some(keys);
            self.minibuffer
                .set_message(format!("Keyboard macro defined ({count} keys)"));
        }
        Ok(())
    }

    fn call_last_keyboard_macro(&mut self, argument: Option<i32>) -> Result<EditorOutcome> {
        if self.recording_keyboard_macro.is_some() {
            self.trim_current_command_from_keyboard_macro();
            self.minibuffer
                .set_error("Cannot execute keyboard macro while defining one");
            return Ok(EditorOutcome::Continue);
        }
        if self.replaying_keyboard_macro {
            self.minibuffer
                .set_error("Cannot execute keyboard macro recursively");
            return Ok(EditorOutcome::Continue);
        }

        let Some(keys) = self.last_keyboard_macro.clone() else {
            self.minibuffer.set_error("No keyboard macro defined");
            return Ok(EditorOutcome::Continue);
        };

        let repeat_count = positive_argument_count(argument);
        if repeat_count == 0 {
            self.minibuffer
                .set_message("Keyboard macro repeated 0 times");
            return Ok(EditorOutcome::Continue);
        }

        self.replaying_keyboard_macro = true;
        let replay_result: Result<EditorOutcome> = (|| {
            for _ in 0..repeat_count {
                for key in &keys {
                    let outcome = self.handle_key(key.clone())?;
                    if outcome == EditorOutcome::Quit {
                        return Ok(EditorOutcome::Quit);
                    }
                }
            }
            Ok(EditorOutcome::Continue)
        })();
        self.replaying_keyboard_macro = false;

        if replay_result? == EditorOutcome::Quit {
            return Ok(EditorOutcome::Quit);
        }

        self.minibuffer
            .set_message(format!("Keyboard macro executed ({repeat_count} times)"));
        Ok(EditorOutcome::Continue)
    }

    fn start_quoted_insert(&mut self) -> Result<()> {
        self.clear_insert_group();
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.quoted_insert = true;
        self.minibuffer.set_message("C-q-");
        Ok(())
    }

    fn handle_quoted_insert_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        self.quoted_insert = false;
        match key {
            KeyEvent::Ctrl('g') => self.quit_current_operation(),
            KeyEvent::Text(text) => self.insert_text(&text, false)?,
            KeyEvent::Special(SpecialKey::Enter) => self.insert_text("\n", false)?,
            KeyEvent::Special(SpecialKey::Tab) => self.insert_text("\t", false)?,
            KeyEvent::Ctrl('@') => self
                .minibuffer
                .set_error("quoted NUL insertion is not supported"),
            KeyEvent::Ctrl(_)
            | KeyEvent::CtrlMeta(_)
            | KeyEvent::CtrlSpecial(_)
            | KeyEvent::Meta(_)
            | KeyEvent::MetaSpecial(_)
            | KeyEvent::Special(_) => self
                .minibuffer
                .set_error("quoted control insertion is not supported"),
        }
        Ok(EditorOutcome::Continue)
    }

    fn save_buffers_kill_terminal(&mut self) -> EditorOutcome {
        if !self.has_dirty_normal_buffers() {
            return EditorOutcome::Quit;
        }

        self.minibuffer.start_prompt(
            PromptKind::QuitDirtyBuffers,
            "Modified buffers exist; exit anyway? (yes or no) ",
        );
        EditorOutcome::Continue
    }

    fn submit_quit_dirty_buffers(&mut self, input: &str) -> EditorOutcome {
        match input.to_ascii_lowercase().as_str() {
            "yes" => EditorOutcome::Quit,
            "no" | "" => {
                self.minibuffer.set_message("Quit");
                EditorOutcome::Continue
            }
            _ => {
                self.minibuffer.start_prompt(
                    PromptKind::QuitDirtyBuffers,
                    "Modified buffers exist; exit anyway? (yes or no) ",
                );
                EditorOutcome::Continue
            }
        }
    }

    fn start_revert_buffer(&mut self) -> Result<EditorOutcome> {
        self.clear_insert_group();
        if self.document().kind() != DocumentKind::Normal || self.document().path().is_none() {
            self.minibuffer
                .set_error("revert-buffer requires a file-backed normal buffer");
            return Ok(EditorOutcome::Continue);
        }
        if self.document().is_dirty() {
            self.minibuffer.start_prompt(
                PromptKind::RevertBuffer,
                "Buffer modified; revert anyway? (yes or no) ",
            );
            return Ok(EditorOutcome::Continue);
        }
        self.revert_current_buffer()
    }

    fn submit_revert_buffer(&mut self, input: &str) -> EditorOutcome {
        match input.to_ascii_lowercase().as_str() {
            "yes" => self.revert_current_buffer().unwrap_or_else(|error| {
                self.minibuffer.set_error(format!("revert failed: {error}"));
                EditorOutcome::Continue
            }),
            "no" | "" => {
                self.minibuffer.set_message("Revert canceled");
                EditorOutcome::Continue
            }
            _ => {
                self.minibuffer.start_prompt(
                    PromptKind::RevertBuffer,
                    "Buffer modified; revert anyway? (yes or no) ",
                );
                EditorOutcome::Continue
            }
        }
    }

    fn revert_current_buffer(&mut self) -> Result<EditorOutcome> {
        let name = self.current_buffer_name().to_owned();
        self.document_mut().reload_from_disk()?;
        self.cursor = clamp_position_to_buffer(self.document().buffer(), self.cursor);
        self.goal_display_column = None;
        self.region = None;
        self.undo_stack.clear();
        self.refresh_visible_buffer_list();
        self.sync_current_window();
        self.minibuffer.set_message(format!("Reverted {name}"));
        Ok(EditorOutcome::Continue)
    }

    fn has_dirty_normal_buffers(&self) -> bool {
        self.buffers.entries().iter().any(|entry| {
            entry.document().kind() == DocumentKind::Normal && entry.document().is_dirty()
        })
    }

    fn quit_current_operation(&mut self) {
        self.quoted_insert = false;
        self.describe_key = None;
        self.pending_register = None;
        self.clear_key_sequence();
        self.universal_argument = None;
        self.deactivate_region();
        self.clear_insert_group();
        self.minibuffer.set_message("Quit");
    }

    fn move_backward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = self
            .document()
            .buffer()
            .move_grapheme_backward(self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_forward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = self
            .document()
            .buffer()
            .move_grapheme_forward(self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_word_backward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = self.document().buffer().move_word_backward(self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_word_forward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = self.document().buffer().move_word_forward(self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_paragraph_backward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = paragraph_backward_position(self.document().buffer(), self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_paragraph_forward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = paragraph_forward_position(self.document().buffer(), self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_sentence_backward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = sentence_backward_position(self.document().buffer(), self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_sentence_forward(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = sentence_forward_position(self.document().buffer(), self.cursor)?;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_line(&mut self, delta: isize) -> Result<()> {
        self.clear_insert_group();
        let (position, goal) =
            self.document()
                .buffer()
                .move_line(self.cursor, delta, self.goal_display_column)?;
        self.cursor = position;
        self.goal_display_column = Some(goal);
        self.sync_current_window();
        Ok(())
    }

    fn scroll_page_forward(&mut self) -> Result<()> {
        self.scroll_page(1)
    }

    fn scroll_page_backward(&mut self) -> Result<()> {
        self.scroll_page(-1)
    }

    fn scroll_page(&mut self, direction: isize) -> Result<()> {
        self.clear_insert_group();
        let text_rows = self.windows.current().viewport().text_rows.max(1);
        let amount = text_rows.saturating_sub(1).max(1);
        let old_first_visible_line = self.windows.current().viewport().first_visible_line;
        let delta = if direction.is_negative() {
            -(amount as isize)
        } else {
            amount as isize
        };
        let (position, goal) =
            self.document()
                .buffer()
                .move_line(self.cursor, delta, self.goal_display_column)?;

        self.cursor = position;
        self.goal_display_column = Some(goal);
        self.sync_current_window();

        let line_count = self.document().buffer().line_count();
        let max_first_visible_line = line_count.saturating_sub(text_rows);
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.first_visible_line = if direction.is_negative() {
            old_first_visible_line.saturating_sub(amount)
        } else {
            old_first_visible_line.saturating_add(amount)
        }
        .min(max_first_visible_line);
        Ok(())
    }

    fn recenter(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.sync_current_window();
        let text_rows = self.windows.current().viewport().text_rows.max(1);
        let line_count = self.document().buffer().line_count();
        let first_visible_line = match self.recenter_cycle_index % 3 {
            0 => self.cursor.line.saturating_sub(text_rows / 2),
            1 => self.cursor.line,
            _ => self.cursor.line.saturating_add(1).saturating_sub(text_rows),
        };
        self.recenter_cycle_index = (self.recenter_cycle_index + 1) % 3;
        self.windows.current_mut().viewport_mut().first_visible_line =
            first_visible_line.min(line_count.saturating_sub(1));
        Ok(())
    }

    fn reset_recenter_cycle(&mut self) {
        self.recenter_cycle_index = 0;
    }

    fn move_to_window_line_top_bottom(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.sync_current_window();
        let buffer = self.document().buffer();
        let viewport = *self.windows.current().viewport();
        let text_rows = viewport.text_rows.max(1);
        let line_count = buffer.line_count();
        let offset = match self.window_line_cycle_index % 3 {
            0 => text_rows / 2,
            1 => 0,
            _ => text_rows.saturating_sub(1),
        };
        let target_line = viewport
            .first_visible_line
            .saturating_add(offset)
            .min(line_count.saturating_sub(1));

        self.cursor = Position::new(target_line, 0);
        self.goal_display_column = None;
        self.window_line_cycle_index = (self.window_line_cycle_index + 1) % 3;
        self.sync_current_window();
        Ok(())
    }

    fn reset_window_line_cycle(&mut self) {
        self.window_line_cycle_index = 0;
    }

    fn what_cursor_position(&mut self) -> Result<()> {
        self.clear_insert_group();
        let buffer = self.document().buffer();
        let absolute = position_to_absolute(buffer, self.cursor)?;
        let total = buffer.serialize().len();
        let column = buffer.display_column(self.cursor)?;
        let percentage = absolute
            .saturating_mul(100)
            .checked_div(total)
            .unwrap_or(100);
        self.minibuffer.set_message(format!(
            "Line {}, column {}, point {} of {} ({}%)",
            self.cursor.line + 1,
            column,
            absolute + 1,
            total + 1,
            percentage
        ));
        Ok(())
    }

    fn move_beginning_of_buffer(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = Position::new(0, 0);
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_end_of_buffer(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = self.document().buffer().end_position();
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_beginning_of_line(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.cursor = Position::new(self.cursor.line, 0);
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_end_of_line(&mut self) -> Result<()> {
        self.clear_insert_group();
        let Some(line) = self.document().buffer().line(self.cursor.line) else {
            return Err(RileError::InvalidPosition(format!(
                "line {} is outside buffer",
                self.cursor.line
            )));
        };
        self.cursor = Position::new(self.cursor.line, line.len());
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn move_back_to_indentation(&mut self) -> Result<()> {
        self.clear_insert_group();
        let Some(line) = self.document().buffer().line(self.cursor.line) else {
            return Err(RileError::InvalidPosition(format!(
                "line {} is outside buffer",
                self.cursor.line
            )));
        };
        let target = line
            .char_indices()
            .find_map(|(byte, character)| (!character.is_whitespace()).then_some(byte))
            .unwrap_or(line.len());
        self.cursor = Position::new(self.cursor.line, target);
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn case_word(&mut self, argument: Option<i32>, transform: CaseTransform) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let range = self.word_case_range(argument.unwrap_or(1))?;
        if range.start == range.end {
            return Ok(());
        }

        let cursor_before = self.cursor;
        let old_text = self.document().buffer().text_in_range(range)?;
        let new_text = case_transform_text(&old_text, transform);
        let cursor_after = if old_text == new_text {
            range.end
        } else {
            self.replace_text_range(range, &new_text, cursor_before)?
        };

        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn word_case_range(&self, argument: i32) -> Result<TextRange> {
        let count = argument.unsigned_abs() as usize;
        if count == 0 {
            return Ok(TextRange::new(self.cursor, self.cursor));
        }

        let buffer = self.document().buffer();
        if argument >= 0 {
            let mut end = self.cursor;
            for _ in 0..count {
                end = buffer.move_word_forward(end)?;
            }
            Ok(TextRange::new(self.cursor, end))
        } else {
            let mut start = self.cursor;
            for _ in 0..count {
                start = buffer.move_word_backward(start)?;
            }
            Ok(TextRange::new(start, self.cursor))
        }
    }

    fn case_region(&mut self, transform: CaseTransform) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let cursor_at_start = self.cursor == range.start;
        let old_text = self.document().buffer().text_in_range(range)?;
        let new_text = case_transform_text(&old_text, transform);
        let replacement_end = if old_text == new_text {
            range.end
        } else {
            self.replace_text_range(range, &new_text, cursor_before)?
        };

        if cursor_at_start {
            self.cursor = range.start;
            if let Some(region) = &mut self.region {
                region.mark = replacement_end;
                region.active = true;
            }
        } else {
            self.cursor = replacement_end;
            if let Some(region) = &mut self.region {
                region.mark = range.start;
                region.active = true;
            }
        }
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.sync_current_window();
        Ok(())
    }

    fn comment_dwim(&mut self) -> Result<()> {
        let Some(syntax) = self.current_comment_syntax() else {
            self.minibuffer
                .set_error("No comment syntax for current mode");
            return Ok(());
        };

        if let Some(range) = self.active_region_range() {
            if self.region_lines_are_commented(range, syntax) {
                self.apply_comment_region(CommentAction::Uncomment, syntax, Some(range))
            } else {
                self.apply_comment_region(CommentAction::Comment, syntax, Some(range))
            }
        } else {
            self.comment_current_line(syntax)
        }
    }

    fn comment_region(&mut self) -> Result<()> {
        let Some(syntax) = self.current_comment_syntax() else {
            self.minibuffer
                .set_error("No comment syntax for current mode");
            return Ok(());
        };
        self.apply_comment_region(CommentAction::Comment, syntax, self.active_region_range())
    }

    fn uncomment_region(&mut self) -> Result<()> {
        let Some(syntax) = self.current_comment_syntax() else {
            self.minibuffer
                .set_error("No comment syntax for current mode");
            return Ok(());
        };
        self.apply_comment_region(CommentAction::Uncomment, syntax, self.active_region_range())
    }

    fn current_comment_syntax(&self) -> Option<CommentSyntax> {
        self.major_mode_for_buffer(self.current_buffer_id())
            .comment_syntax()
    }

    fn comment_current_line(&mut self, syntax: CommentSyntax) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let mut lines = self.document().buffer().lines().to_vec();
        let line_index = self.cursor.line.min(lines.len().saturating_sub(1));
        let indent = line_comment_indent(&lines[line_index]);
        let insertion = format!("{} ", syntax.line_start);
        lines[line_index].insert_str(indent, &insertion);
        let cursor_after = Position::new(line_index, indent + insertion.len());
        self.replace_buffer_text(lines.join("\n"), self.cursor, cursor_after)?;
        Ok(())
    }

    fn apply_comment_region(
        &mut self,
        action: CommentAction,
        syntax: CommentSyntax,
        range: Option<TextRange>,
    ) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let Some(range) = range else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let Some((start_line, end_line)) = region_line_bounds(range) else {
            return Ok(());
        };

        let cursor_before = self.cursor;
        let mut cursor_after = cursor_before;
        let mut lines = self.document().buffer().lines().to_vec();
        let end_line = end_line.min(lines.len().saturating_sub(1));
        for (line_index, line) in lines
            .iter_mut()
            .enumerate()
            .take(end_line + 1)
            .skip(start_line)
        {
            let edit = match action {
                CommentAction::Comment => comment_line(line, syntax),
                CommentAction::Uncomment => uncomment_line(line, syntax),
            };
            if let Some((byte, delta)) = edit {
                cursor_after =
                    adjust_position_after_line_delta(cursor_after, line_index, byte, delta);
            }
        }

        let replacement = lines.join("\n");
        if replacement == self.document().buffer().serialize() {
            self.deactivate_region();
            self.minibuffer.clear();
            return Ok(());
        }
        self.replace_buffer_text(replacement, cursor_before, cursor_after)?;
        Ok(())
    }

    fn region_lines_are_commented(&self, range: TextRange, syntax: CommentSyntax) -> bool {
        let Some((start_line, end_line)) = region_line_bounds(range) else {
            return false;
        };
        let lines = self.document().buffer().lines();
        let mut saw_non_empty = false;
        for line in lines
            .iter()
            .take(end_line.min(lines.len().saturating_sub(1)) + 1)
            .skip(start_line)
        {
            if line.trim().is_empty() {
                continue;
            }
            saw_non_empty = true;
            if !line_is_commented(line, syntax) {
                return false;
            }
        }
        saw_non_empty
    }

    fn fill_paragraph(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let region = self.active_region_range();
        let buffer = self.document().buffer();
        let lines = buffer.lines();
        let Some((first_line, last_line)) = fill_line_bounds(lines, self.cursor, region) else {
            return Ok(());
        };
        let runs = paragraph_runs_in_line_bounds(lines, first_line, last_line);
        if runs.is_empty() {
            return Ok(());
        }

        let cursor_before = self.cursor;
        let mut replacement_lines = lines.to_vec();
        let cursor_after = filled_cursor_position(lines, &runs, cursor_before, self.fill_column);
        for (run_start, run_end) in runs.into_iter().rev() {
            let filled =
                fill_plain_text_lines(&replacement_lines[run_start..=run_end], self.fill_column);
            replacement_lines.splice(run_start..=run_end, filled);
        }

        let replacement = replacement_lines.join("\n");
        if replacement == buffer.serialize() {
            self.minibuffer.clear();
            return Ok(());
        }

        let cursor_after = cursor_after
            .map(|position| clamp_position_to_lines(&replacement_lines, position))
            .unwrap_or_else(|| {
                if region.is_some() {
                    clamp_position_to_lines(&replacement_lines, cursor_before)
                } else {
                    Position::new(first_line, 0)
                }
            });
        self.replace_buffer_text(replacement, cursor_before, cursor_after)?;
        Ok(())
    }

    fn transpose_chars(&mut self, argument: Option<i32>) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let argument = argument.unwrap_or(1);
        if argument == 0 {
            self.minibuffer
                .set_error("zero-argument transpose-chars is not supported");
            return Ok(());
        }

        let Some(line) = self.document().buffer().line(self.cursor.line) else {
            return Ok(());
        };
        let Some(edit) = transpose_chars_edit(line, self.cursor, argument) else {
            self.minibuffer.set_error("Cannot transpose characters");
            return Ok(());
        };

        let cursor_before = self.cursor;
        let old_text = self.document_mut().buffer_mut().delete_range(edit.range)?;
        let replacement_end = self
            .document_mut()
            .buffer_mut()
            .insert(edit.range.start, &edit.replacement)?;
        self.record_replace(
            TextRange::new(edit.range.start, replacement_end),
            old_text,
            edit.replacement,
            cursor_before,
            edit.cursor_after,
        );
        self.cursor = edit.cursor_after;
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn transpose_words(&mut self, argument: Option<i32>) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let argument = argument.unwrap_or(1);
        if argument == 0 {
            self.minibuffer
                .set_error("zero-argument transpose-words is not supported");
            return Ok(());
        }

        let cursor_before = self.cursor;
        let mut replacement = self.document().buffer().serialize();
        let mut cursor = position_to_absolute(self.document().buffer(), self.cursor)?;
        for _ in 0..argument.unsigned_abs() {
            let Some((next_text, next_cursor)) =
                transpose_words_once(&replacement, cursor, argument)
            else {
                self.minibuffer.set_error("Cannot transpose words");
                return Ok(());
            };
            replacement = next_text;
            cursor = next_cursor;
        }

        let cursor_after = absolute_to_position(&replacement, cursor);
        self.replace_buffer_text(replacement, cursor_before, cursor_after)?;
        Ok(())
    }

    fn transpose_lines(&mut self, argument: Option<i32>) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let argument = argument.unwrap_or(1);
        if argument == 0 {
            self.minibuffer
                .set_error("zero-argument transpose-lines is not supported");
            return Ok(());
        }

        let cursor_before = self.cursor;
        let Some((lines, cursor_after)) =
            transpose_lines_edit(self.document().buffer(), self.cursor, argument)
        else {
            self.minibuffer.set_error("Cannot transpose lines");
            return Ok(());
        };

        self.replace_buffer_text(lines.join("\n"), cursor_before, cursor_after)?;
        Ok(())
    }

    fn replace_buffer_text(
        &mut self,
        replacement: String,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Result<()> {
        let range = TextRange::new(Position::new(0, 0), self.document().buffer().end_position());
        let old_text = self.document_mut().buffer_mut().delete_range(range)?;
        let replacement_end = self
            .document_mut()
            .buffer_mut()
            .insert(Position::new(0, 0), &replacement)?;
        self.record_replace(
            TextRange::new(Position::new(0, 0), replacement_end),
            old_text,
            replacement,
            cursor_before,
            cursor_after,
        );
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn replace_text_range(
        &mut self,
        range: TextRange,
        replacement: &str,
        cursor_before: Position,
    ) -> Result<Position> {
        let old_text = self.document_mut().buffer_mut().delete_range(range)?;
        let cursor_after = self
            .document_mut()
            .buffer_mut()
            .insert(range.start, replacement)?;
        self.record_replace(
            TextRange::new(range.start, cursor_after),
            old_text,
            replacement.to_owned(),
            cursor_before,
            cursor_after,
        );
        Ok(cursor_after)
    }

    fn replace_text_range_with_cursor(
        &mut self,
        range: TextRange,
        replacement: &str,
        cursor_before: Position,
        cursor_after: Position,
    ) -> Result<()> {
        let old_text = self.document_mut().buffer_mut().delete_range(range)?;
        let replacement_end = self
            .document_mut()
            .buffer_mut()
            .insert(range.start, replacement)?;
        self.record_replace(
            TextRange::new(range.start, replacement_end),
            old_text,
            replacement.to_owned(),
            cursor_before,
            cursor_after,
        );
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.deactivate_region();
        self.minibuffer.clear();
        self.sync_current_window();
        Ok(())
    }

    fn delete_text_range(&mut self, range: TextRange, cursor_after: Position) -> Result<bool> {
        if range.start == range.end {
            return Ok(false);
        }

        let cursor_before = self.cursor;
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        if text.is_empty() {
            return Ok(false);
        }
        self.cursor = cursor_after;
        self.record_delete(range, text, cursor_before, cursor_after);
        self.goal_display_column = None;
        self.deactivate_region();
        self.minibuffer.clear();
        self.sync_current_window();
        Ok(true)
    }

    fn delete_backward_char(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let start = self
            .document()
            .buffer()
            .move_grapheme_backward(self.cursor)?;
        if start == self.cursor {
            return Ok(());
        }
        let cursor = self.cursor;
        let range = TextRange::new(start, cursor);
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.record_delete(range, text, cursor, start);
        self.cursor = start;
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn delete_char(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let end = self
            .document()
            .buffer()
            .move_grapheme_forward(self.cursor)?;
        if end == self.cursor {
            return Ok(());
        }
        let cursor = self.cursor;
        let range = TextRange::new(cursor, end);
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.record_delete(range, text, cursor, cursor);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn delete_horizontal_space(&mut self, backward_only: bool) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let Some(line) = self.document().buffer().line(self.cursor.line) else {
            return Ok(());
        };
        let bytes = line.as_bytes();
        let mut start = self.cursor.byte;
        while start > 0 && is_horizontal_space_byte(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = self.cursor.byte;
        if !backward_only {
            while end < bytes.len() && is_horizontal_space_byte(bytes[end]) {
                end += 1;
            }
        }

        self.delete_text_range(
            TextRange::new(
                Position::new(self.cursor.line, start),
                Position::new(self.cursor.line, end),
            ),
            Position::new(self.cursor.line, start),
        )?;
        Ok(())
    }

    fn just_one_space(&mut self, argument: Option<i32>) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let count = argument.unwrap_or(1);
        let replacement_count = count.unsigned_abs() as usize;
        let include_newlines = count < 0;
        let text = self.document().buffer().serialize();
        let cursor_absolute = position_to_absolute(self.document().buffer(), self.cursor)?;
        let (start, end) = whitespace_run_around(&text, cursor_absolute, include_newlines);
        let replacement = " ".repeat(replacement_count);
        if text[start..end] == replacement {
            self.minibuffer.clear();
            return Ok(());
        }
        let cursor_after_absolute = start + replacement.len();
        let range = TextRange::new(
            absolute_to_position(&text, start),
            absolute_to_position(&text, end),
        );
        let cursor_after = absolute_to_position(
            &format!("{}{}{}", &text[..start], replacement, &text[end..]),
            cursor_after_absolute,
        );
        self.replace_text_range_with_cursor(range, &replacement, self.cursor, cursor_after)?;
        Ok(())
    }

    fn delete_blank_lines(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let buffer = self.document().buffer();
        let line_count = buffer.line_count();
        let line_index = self.cursor.line;
        let Some(line) = buffer.line(line_index) else {
            return Ok(());
        };

        let (range, cursor_after) = if is_blank_line(line) {
            let (run_start, run_end) = blank_line_run(buffer.lines(), line_index);
            if run_start == 0 && run_end + 1 == line_count {
                (
                    TextRange::new(Position::new(0, 0), buffer.end_position()),
                    Position::new(0, 0),
                )
            } else if run_end > run_start {
                let delete_start = if run_end + 1 < line_count {
                    Position::new(run_start + 1, 0)
                } else {
                    Position::new(run_start, buffer.line(run_start).unwrap_or("").len())
                };
                let delete_end = if run_end + 1 < line_count {
                    Position::new(run_end + 1, 0)
                } else {
                    buffer.end_position()
                };
                (
                    TextRange::new(delete_start, delete_end),
                    Position::new(run_start, 0),
                )
            } else if line_count == 1 {
                (
                    TextRange::new(Position::new(0, 0), Position::new(0, line.len())),
                    Position::new(0, 0),
                )
            } else if line_index + 1 < line_count {
                (
                    TextRange::new(
                        Position::new(line_index, 0),
                        Position::new(line_index + 1, 0),
                    ),
                    Position::new(line_index, 0),
                )
            } else {
                let previous_line = line_index - 1;
                let previous_len = buffer.line(previous_line).unwrap_or("").len();
                (
                    TextRange::new(
                        Position::new(previous_line, previous_len),
                        Position::new(line_index, line.len()),
                    ),
                    Position::new(previous_line, previous_len),
                )
            }
        } else {
            let blank_start = line_index + 1;
            if blank_start >= line_count || !is_blank_line(buffer.line(blank_start).unwrap_or("")) {
                return Ok(());
            }

            let mut blank_end = blank_start;
            while blank_end + 1 < line_count
                && is_blank_line(buffer.line(blank_end + 1).unwrap_or(""))
            {
                blank_end += 1;
            }
            let delete_end = if blank_end + 1 < line_count {
                Position::new(blank_end + 1, 0)
            } else {
                buffer.end_position()
            };
            (
                TextRange::new(Position::new(blank_start, 0), delete_end),
                self.cursor,
            )
        };

        self.delete_text_range(range, cursor_after)?;
        Ok(())
    }

    fn delete_trailing_whitespace(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();

        let region = self.active_region_range();
        let lines = self.document().buffer().lines().to_vec();
        let Some(last_line) = lines.len().checked_sub(1) else {
            return Ok(());
        };
        let start_line = region.map_or(0, |range| range.start.line);
        let end_line = region.map_or(last_line, |range| range.end.line.min(last_line));
        let mut ranges = Vec::new();

        for (line_index, line) in lines.iter().enumerate().take(end_line + 1).skip(start_line) {
            let line_end_in_scope = match region {
                None => true,
                Some(range) if line_index < range.end.line => true,
                Some(range) => range.end.byte >= line.len(),
            };
            if !line_end_in_scope {
                continue;
            }

            let lower_bound = match region {
                Some(range) if line_index == range.start.line => range.start.byte,
                _ => 0,
            };
            let start = trailing_horizontal_space_start(line).max(lower_bound);
            if start < line.len() {
                ranges.push(TextRange::new(
                    Position::new(line_index, start),
                    Position::new(line_index, line.len()),
                ));
            }
        }

        if ranges.is_empty() {
            return Ok(());
        }

        let cursor_before = self.cursor;
        let mut cursor_after = self.cursor;
        let mut deletes = Vec::new();
        for range in ranges {
            let text = self.document_mut().buffer_mut().delete_range(range)?;
            if !text.is_empty() {
                cursor_after = adjust_position_after_same_line_delete(cursor_after, range);
                deletes.push((range, text));
            }
        }

        if deletes.is_empty() {
            return Ok(());
        }

        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.record_batch_delete(deletes, cursor_before, cursor_after);
        self.deactivate_region();
        self.minibuffer.clear();
        self.sync_current_window();
        Ok(())
    }

    fn set_mark_command(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.region = Some(RegionState {
            buffer: self.current_buffer,
            mark: self.cursor,
            active: true,
            shape: RegionShape::Linear,
        });
        self.minibuffer.set_message("Mark set");
        Ok(())
    }

    fn rectangle_mark_mode(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.region = Some(RegionState {
            buffer: self.current_buffer,
            mark: self.cursor,
            active: true,
            shape: RegionShape::Rectangle,
        });
        self.minibuffer.set_message("Mark set (rectangle mode)");
        Ok(())
    }

    fn mark_whole_buffer(&mut self) -> Result<()> {
        self.clear_insert_group();
        let mark = self.document().buffer().end_position();
        self.cursor = Position::new(0, 0);
        self.region = Some(RegionState {
            buffer: self.current_buffer,
            mark,
            active: true,
            shape: RegionShape::Linear,
        });
        self.goal_display_column = None;
        self.sync_current_window();
        self.minibuffer.set_message("Mark set");
        Ok(())
    }

    fn exchange_point_and_mark(&mut self) -> Result<()> {
        self.clear_insert_group();
        let Some(region) = &mut self.region else {
            self.minibuffer.set_message("No mark set in this buffer");
            return Ok(());
        };
        if region.buffer != self.current_buffer {
            self.minibuffer.set_message("No mark set in this buffer");
            return Ok(());
        }

        std::mem::swap(&mut self.cursor, &mut region.mark);
        region.active = true;
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(())
    }

    fn start_point_to_register(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.pending_register = Some(PendingRegisterCommand::Point);
        self.minibuffer.set_message("Point to register: ");
        Ok(())
    }

    fn start_jump_to_register(&mut self) -> Result<()> {
        self.clear_insert_group();
        self.pending_register = Some(PendingRegisterCommand::Jump);
        self.minibuffer.set_message("Jump to register: ");
        Ok(())
    }

    fn start_copy_to_register(&mut self) -> Result<()> {
        self.clear_insert_group();
        if self.active_region_range().is_none() {
            self.minibuffer.set_error("no active region");
            return Ok(());
        }
        self.pending_register = Some(PendingRegisterCommand::CopyText);
        self.minibuffer.set_message("Copy to register: ");
        Ok(())
    }

    fn start_copy_rectangle_to_register(&mut self) -> Result<()> {
        self.clear_insert_group();
        if self.rectangle_bounds_from_mark().is_none() {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        }
        self.pending_register = Some(PendingRegisterCommand::CopyRectangle);
        self.minibuffer.set_message("Copy rectangle to register: ");
        Ok(())
    }

    fn start_insert_register(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        self.pending_register = Some(PendingRegisterCommand::Insert);
        self.minibuffer.set_message("Insert register: ");
        Ok(())
    }

    fn start_number_to_register(&mut self, argument: Option<i32>) -> Result<()> {
        self.clear_insert_group();
        self.pending_register = Some(PendingRegisterCommand::Number {
            value: argument.unwrap_or(0),
        });
        self.minibuffer.set_message("Number to register: ");
        Ok(())
    }

    fn start_increment_register(&mut self, argument: Option<i32>) -> Result<()> {
        self.clear_insert_group();
        self.pending_register = Some(PendingRegisterCommand::IncrementNumber {
            amount: argument.unwrap_or(1),
        });
        self.minibuffer.set_message("Increment register: ");
        Ok(())
    }

    fn handle_pending_register_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        let Some(command) = self.pending_register.take() else {
            return Ok(EditorOutcome::Continue);
        };
        let Some(register) = register_key_from_event(&key) else {
            self.minibuffer.set_error("invalid register key");
            return Ok(EditorOutcome::Continue);
        };

        match command {
            PendingRegisterCommand::CopyRectangle => self.copy_rectangle_to_register(register)?,
            PendingRegisterCommand::CopyText => self.copy_to_register(register)?,
            PendingRegisterCommand::IncrementNumber { amount } => {
                self.increment_register(register, amount)
            }
            PendingRegisterCommand::Insert => self.insert_register(register)?,
            PendingRegisterCommand::Jump => self.jump_to_register(register),
            PendingRegisterCommand::Number { value } => self.number_to_register(register, value),
            PendingRegisterCommand::Point => self.point_to_register(register),
        }
        Ok(EditorOutcome::Continue)
    }

    fn point_to_register(&mut self, register: char) {
        self.registers.insert(
            register,
            RegisterValue::Point {
                buffer: self.current_buffer,
                position: self.cursor,
            },
        );
        self.minibuffer
            .set_message(format!("Point saved to register {register}"));
    }

    fn jump_to_register(&mut self, register: char) {
        let Some(RegisterValue::Point { buffer, position }) =
            self.registers.get(&register).cloned()
        else {
            self.minibuffer
                .set_error("register does not contain a point");
            return;
        };
        let Some(document) = self.buffers.document(buffer) else {
            self.minibuffer.set_error("register buffer is gone");
            return;
        };
        if let Err(error) = document.buffer().validate_position(position) {
            self.minibuffer.set_error(error.to_string());
            return;
        }

        if buffer != self.current_buffer {
            self.sync_current_window();
            self.current_buffer = buffer;
        }
        self.restore_buffer_in_current_window(buffer);
        self.cursor = position;
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer
            .set_message(format!("Jumped to register {register}"));
    }

    fn copy_to_register(&mut self, register: char) -> Result<()> {
        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let text = self.document().buffer().text_in_range(range)?;
        self.registers.insert(register, RegisterValue::Text(text));
        self.deactivate_region();
        self.minibuffer
            .set_message(format!("Copied region to register {register}"));
        Ok(())
    }

    fn copy_rectangle_to_register(&mut self, register: char) -> Result<()> {
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        let text = self.document().buffer().text_in_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        self.registers
            .insert(register, RegisterValue::Rectangle(text));
        self.deactivate_region();
        self.minibuffer
            .set_message(format!("Copied rectangle to register {register}"));
        Ok(())
    }

    fn number_to_register(&mut self, register: char, value: i32) {
        self.registers
            .insert(register, RegisterValue::Number(value));
        self.minibuffer
            .set_message(format!("Number saved to register {register}"));
    }

    fn increment_register(&mut self, register: char, amount: i32) {
        let Some(RegisterValue::Number(value)) = self.registers.get_mut(&register) else {
            self.minibuffer
                .set_error("register does not contain a number");
            return;
        };
        *value = value.saturating_add(amount);
        self.minibuffer
            .set_message(format!("Incremented register {register}"));
    }

    fn insert_register(&mut self, register: char) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        let Some(value) = self.registers.get(&register).cloned() else {
            self.minibuffer.set_error("register is empty");
            return Ok(());
        };
        match value {
            RegisterValue::Text(text) => self.insert_register_text(register, &text)?,
            RegisterValue::Rectangle(rectangle) => {
                self.insert_register_rectangle(register, &rectangle)?
            }
            RegisterValue::Number(number) => {
                self.insert_register_text(register, &number.to_string())?;
            }
            RegisterValue::Point { .. } => {
                self.minibuffer.set_error("register does not contain text")
            }
        }
        Ok(())
    }

    fn insert_register_text(&mut self, register: char, text: &str) -> Result<()> {
        let cursor_before = self.cursor;
        self.cursor = self
            .document_mut()
            .buffer_mut()
            .insert(cursor_before, text)?;
        self.record_insert(cursor_before, self.cursor, text, false);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer
            .set_message(format!("Inserted register {register}"));
        Ok(())
    }

    fn insert_register_rectangle(&mut self, register: char, rectangle: &[String]) -> Result<()> {
        let cursor_before = self.cursor;
        let column = self.document().buffer().display_column(cursor_before)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            cursor_before,
            column,
            rectangle,
        )?;
        self.cursor = cursor_after;
        self.record_batch_insert(inserts, cursor_before, cursor_after);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer
            .set_message(format!("Inserted register {register}"));
        Ok(())
    }

    fn copy_region_as_kill(&mut self) -> Result<()> {
        self.clear_insert_group();
        if let Some(bounds) = self.active_rectangle_bounds() {
            let text = self.document().buffer().text_in_display_rectangle(
                bounds.start_line,
                bounds.end_line,
                bounds.start_column,
                bounds.end_column,
            )?;
            self.push_kill(KillEntry::Rectangle(text));
            self.deactivate_region();
            self.minibuffer.set_message("Copied rectangle");
            return Ok(());
        }

        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let text = self.document().buffer().text_in_range(range)?;
        self.push_kill(KillEntry::Text(text));
        self.deactivate_region();
        self.minibuffer.set_message("Copied region");
        Ok(())
    }

    fn copy_rectangle_as_kill(&mut self) -> Result<()> {
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        let text = self.document().buffer().text_in_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        self.push_kill(KillEntry::Rectangle(text));
        self.deactivate_region();
        self.minibuffer.set_message("Copied rectangle");
        Ok(())
    }

    fn kill_region(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        if let Some(bounds) = self.active_rectangle_bounds() {
            return self.kill_rectangle(bounds);
        }

        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.push_command_kill(KillEntry::Text(text.clone()), KillDirection::Forward);
        self.cursor = range.start;
        self.goal_display_column = None;
        self.record_delete(range, text, cursor_before, self.cursor);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed region");
        Ok(())
    }

    fn kill_rectangle_command(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        self.kill_rectangle(bounds)
    }

    fn kill_rectangle(&mut self, bounds: RectangleBounds) -> Result<()> {
        let cursor_before = self.cursor;
        let (text, deletes) = self.document_mut().buffer_mut().delete_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        self.push_command_kill(KillEntry::Rectangle(text), KillDirection::Forward);
        self.cursor = self.rectangle_position(bounds.end_line, bounds.start_column)?;
        self.goal_display_column = None;
        self.record_batch_delete(deletes, cursor_before, self.cursor);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed rectangle");
        Ok(())
    }

    fn delete_rectangle_command(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let (_, deletes) = self.document_mut().buffer_mut().delete_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        self.cursor = self.rectangle_position(bounds.end_line, bounds.start_column)?;
        self.goal_display_column = None;
        self.record_batch_delete(deletes, cursor_before, self.cursor);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Deleted rectangle");
        Ok(())
    }

    fn clear_rectangle(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let width = bounds.end_column - bounds.start_column;
        let replacement = vec![" ".repeat(width); bounds.end_line - bounds.start_line + 1];
        let (_, deletes) = self.document_mut().buffer_mut().delete_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        let at = self.rectangle_position(bounds.start_line, bounds.start_column)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            at,
            bounds.start_column,
            &replacement,
        )?;
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.record_rectangle_replace(deletes, inserts, cursor_before, cursor_after);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Cleared rectangle");
        Ok(())
    }

    fn open_rectangle(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let width = bounds.end_column - bounds.start_column;
        let blank = vec![" ".repeat(width); bounds.end_line - bounds.start_line + 1];
        let at = self.rectangle_position(bounds.start_line, bounds.start_column)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            at,
            bounds.start_column,
            &blank,
        )?;
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.record_batch_insert(inserts, cursor_before, cursor_after);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Opened rectangle");
        Ok(())
    }

    fn start_string_rectangle(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        if self.rectangle_bounds_from_mark().is_none() {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        }
        self.minibuffer
            .start_prompt(PromptKind::StringRectangle, "String rectangle: ");
        Ok(())
    }

    fn submit_string_rectangle(&mut self, replacement: &str) -> Result<EditorOutcome> {
        if !self.ensure_buffer_editable() {
            return Ok(EditorOutcome::Continue);
        }
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(EditorOutcome::Continue);
        };
        self.string_rectangle(bounds, replacement)?;
        Ok(EditorOutcome::Continue)
    }

    fn string_rectangle(&mut self, bounds: RectangleBounds, replacement: &str) -> Result<()> {
        let cursor_before = self.cursor;
        let replacement = vec![replacement.to_owned(); bounds.end_line - bounds.start_line + 1];
        let (_, deletes) = self.document_mut().buffer_mut().delete_display_rectangle(
            bounds.start_line,
            bounds.end_line,
            bounds.start_column,
            bounds.end_column,
        )?;
        let at = self.rectangle_position(bounds.start_line, bounds.start_column)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            at,
            bounds.start_column,
            &replacement,
        )?;
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.record_rectangle_replace(deletes, inserts, cursor_before, cursor_after);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("String rectangle replaced");
        Ok(())
    }

    fn rectangle_number_lines(&mut self, argument: Option<i32>) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(bounds) = self.rectangle_bounds_from_mark() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(());
        };
        if argument.is_some() {
            self.rectangle_number_prompt = Some(RectangleNumberPromptState {
                bounds,
                start_at: 1,
            });
            self.minibuffer
                .start_prompt(PromptKind::RectangleNumberStart, "Number to count from: ");
            return Ok(());
        }

        let format = default_rectangle_number_format(bounds, 1);
        self.insert_rectangle_numbers(bounds, 1, &format)
    }

    fn submit_rectangle_number_start(&mut self, input: &str) -> Result<EditorOutcome> {
        let Some(mut state) = self.rectangle_number_prompt else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(EditorOutcome::Continue);
        };
        let start_at = if input.is_empty() {
            1
        } else {
            match input.parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    self.rectangle_number_prompt = None;
                    self.minibuffer.set_error("invalid number");
                    return Ok(EditorOutcome::Continue);
                }
            }
        };
        state.start_at = start_at;
        self.rectangle_number_prompt = Some(state);
        let default_format = default_rectangle_number_format(state.bounds, start_at);
        self.minibuffer.start_prompt(
            PromptKind::RectangleNumberFormat,
            format!("Format string (default {default_format}): "),
        );
        Ok(EditorOutcome::Continue)
    }

    fn submit_rectangle_number_format(&mut self, input: &str) -> Result<EditorOutcome> {
        let Some(state) = self.rectangle_number_prompt.take() else {
            self.minibuffer.set_error("no rectangle selected");
            return Ok(EditorOutcome::Continue);
        };
        let format = if input.is_empty() {
            default_rectangle_number_format(state.bounds, state.start_at)
        } else {
            input.to_owned()
        };
        if let Err(error) = self.insert_rectangle_numbers(state.bounds, state.start_at, &format) {
            self.minibuffer.set_error(error.to_string());
        }
        Ok(EditorOutcome::Continue)
    }

    fn insert_rectangle_numbers(
        &mut self,
        bounds: RectangleBounds,
        start_at: i32,
        format: &str,
    ) -> Result<()> {
        let mut numbers = Vec::new();
        for offset in 0..=(bounds.end_line - bounds.start_line) {
            numbers.push(format_rectangle_number(
                format,
                start_at.saturating_add(offset as i32),
            )?);
        }

        let cursor_before = self.cursor;
        let at = self.rectangle_position(bounds.start_line, bounds.start_column)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            at,
            bounds.start_column,
            &numbers,
        )?;
        self.cursor = cursor_after;
        self.goal_display_column = None;
        self.record_batch_insert(inserts, cursor_before, cursor_after);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Numbered rectangle");
        Ok(())
    }

    fn yank(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(kill_index) = self.kill_ring.len().checked_sub(1) else {
            self.minibuffer.set_error("kill ring is empty");
            return Ok(());
        };
        let entry = self.kill_ring[kill_index].clone();
        let KillEntry::Text(text) = entry else {
            return self.yank_rectangle(kill_index);
        };
        let cursor_before = self.cursor;
        self.cursor = self
            .document_mut()
            .buffer_mut()
            .insert(cursor_before, &text)?;
        self.record_insert(cursor_before, self.cursor, &text, false);
        self.yank_state = Some(YankState {
            buffer: self.current_buffer,
            range: TextRange::new(cursor_before, self.cursor),
            kill_index,
        });
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Yanked");
        Ok(())
    }

    fn yank_rectangle_command(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(kill_index) = self.latest_rectangle_kill_index() else {
            self.minibuffer.set_error("no rectangle to yank");
            return Ok(());
        };
        self.yank_rectangle(kill_index)
    }

    fn yank_rectangle(&mut self, kill_index: usize) -> Result<()> {
        let KillEntry::Rectangle(rectangle) = self.kill_ring[kill_index].clone() else {
            return Ok(());
        };
        let cursor_before = self.cursor;
        let column = self.document().buffer().display_column(cursor_before)?;
        let (inserts, cursor_after) = self.document_mut().buffer_mut().insert_display_rectangle(
            cursor_before,
            column,
            &rectangle,
        )?;
        self.cursor = cursor_after;
        self.record_batch_insert(inserts, cursor_before, cursor_after);
        self.yank_state = None;
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Yanked rectangle");
        Ok(())
    }

    fn yank_pop(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        if self.kill_ring.is_empty() {
            self.minibuffer.set_error("Kill ring is empty");
            return Ok(());
        }
        let Some(state) = self.yank_state else {
            self.minibuffer.set_error("Previous command was not a yank");
            return Ok(());
        };
        if state.buffer != self.current_buffer {
            self.yank_state = None;
            self.minibuffer.set_error("Previous command was not a yank");
            return Ok(());
        }

        let Some(kill_index) = self.previous_text_kill_index(state.kill_index) else {
            self.minibuffer.set_error("No text kill to yank-pop");
            return Ok(());
        };
        let KillEntry::Text(text) = self.kill_ring[kill_index].clone() else {
            unreachable!("previous_text_kill_index returns text entries");
        };
        let cursor_before = self.cursor;
        let old_text = self.document_mut().buffer_mut().delete_range(state.range)?;
        let cursor_after = self
            .document_mut()
            .buffer_mut()
            .insert(state.range.start, &text)?;
        self.cursor = cursor_after;
        self.record_replace(
            TextRange::new(state.range.start, cursor_after),
            old_text,
            text,
            cursor_before,
            cursor_after,
        );
        self.yank_state = Some(YankState {
            buffer: self.current_buffer,
            range: TextRange::new(state.range.start, cursor_after),
            kill_index,
        });
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Yanked previous kill");
        Ok(())
    }

    fn kill_line(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        let Some(line) = self.document().buffer().line(self.cursor.line) else {
            return Ok(());
        };
        let end = if self.cursor.byte < line.len() {
            Position::new(self.cursor.line, line.len())
        } else if self.cursor.line + 1 < self.document().buffer().line_count() {
            Position::new(self.cursor.line + 1, 0)
        } else {
            return Ok(());
        };
        let range = TextRange::new(self.cursor, end);
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.push_command_kill(KillEntry::Text(text.clone()), KillDirection::Forward);
        self.record_delete(range, text, cursor_before, self.cursor);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed line");
        Ok(())
    }

    fn kill_word(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        let end = self.document().buffer().move_word_forward(self.cursor)?;
        if end == self.cursor {
            return Ok(());
        }

        let range = TextRange::new(self.cursor, end);
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.push_command_kill(KillEntry::Text(text.clone()), KillDirection::Forward);
        self.record_delete(range, text, cursor_before, self.cursor);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed word");
        Ok(())
    }

    fn backward_kill_word(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        let start = self.document().buffer().move_word_backward(self.cursor)?;
        if start == self.cursor {
            return Ok(());
        }

        let range = TextRange::new(start, self.cursor);
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.push_command_kill(KillEntry::Text(text.clone()), KillDirection::Backward);
        self.cursor = start;
        self.record_delete(range, text, cursor_before, self.cursor);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed word");
        Ok(())
    }

    fn open_line(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        let end = self
            .document_mut()
            .buffer_mut()
            .insert(cursor_before, "\n")?;
        self.record_insert(cursor_before, end, "\n", false);
        self.cursor = cursor_before;
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn newline_and_indent(&mut self) -> Result<()> {
        self.insert_text("\n", false)
    }

    fn join_line(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        if self.cursor.line == 0 {
            self.minibuffer.set_message("Beginning of buffer");
            return Ok(());
        }

        let previous_line_index = self.cursor.line - 1;
        let previous_line = self
            .document()
            .buffer()
            .line(previous_line_index)
            .unwrap_or("");
        let current_line = self
            .document()
            .buffer()
            .line(self.cursor.line)
            .unwrap_or("");

        let previous_trimmed_end = previous_line
            .char_indices()
            .rev()
            .find_map(|(byte, character)| {
                (!character.is_whitespace()).then_some(byte + character.len_utf8())
            })
            .unwrap_or(0);
        let current_trimmed_start = current_line
            .char_indices()
            .find_map(|(byte, character)| (!character.is_whitespace()).then_some(byte))
            .unwrap_or(current_line.len());

        let previous_has_text = previous_trimmed_end > 0;
        let current_has_text = current_trimmed_start < current_line.len();
        let replacement = if previous_has_text && current_has_text {
            " "
        } else {
            ""
        };

        let range = TextRange::new(
            Position::new(previous_line_index, previous_trimmed_end),
            Position::new(self.cursor.line, current_trimmed_start),
        );
        let cursor_before = self.cursor;
        let old_text = self.document_mut().buffer_mut().delete_range(range)?;
        let cursor_after = self
            .document_mut()
            .buffer_mut()
            .insert(range.start, replacement)?;
        self.cursor = cursor_after;
        self.record_replace(
            TextRange::new(range.start, cursor_after),
            old_text,
            replacement.to_owned(),
            cursor_before,
            cursor_after,
        );
        self.goal_display_column = None;
        self.minibuffer.clear();
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn undo(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(index) = self
            .undo_stack
            .iter()
            .rposition(|entry| entry.buffer == self.current_buffer)
        else {
            self.minibuffer.set_message("No undo information");
            return Ok(());
        };
        let entry = self.undo_stack.remove(index);
        self.undo_record(entry.record)?;
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.refresh_visible_buffer_list();
        self.minibuffer.set_message("Undone");
        Ok(())
    }

    fn undo_record(&mut self, record: UndoRecord) -> Result<()> {
        match record {
            UndoRecord::Batch(records) => {
                for record in records.into_iter().rev() {
                    self.undo_record(record)?;
                }
            }
            UndoRecord::Insert {
                range,
                cursor_before,
                ..
            } => {
                self.document_mut().buffer_mut().delete_range(range)?;
                self.cursor = cursor_before;
            }
            UndoRecord::Delete {
                range,
                text,
                cursor_before,
                ..
            } => {
                self.document_mut()
                    .buffer_mut()
                    .insert(range.start, &text)?;
                self.cursor = cursor_before;
            }
            UndoRecord::Replace {
                range,
                old_text,
                cursor_before,
                ..
            } => {
                self.document_mut().buffer_mut().delete_range(range)?;
                self.document_mut()
                    .buffer_mut()
                    .insert(range.start, &old_text)?;
                self.cursor = cursor_before;
            }
        }
        Ok(())
    }

    fn save_buffer(&mut self) -> Result<()> {
        match self.document_mut().save() {
            Ok(()) => {
                self.refresh_visible_buffer_list();
                self.minibuffer
                    .set_message(format!("Wrote {}", self.document().display_name()));
            }
            Err(error) => self.minibuffer.set_error(format!("save failed: {error}")),
        }
        Ok(())
    }

    fn start_save_some_buffers(&mut self) -> EditorOutcome {
        let pending = self
            .buffers
            .entries()
            .iter()
            .filter(|entry| {
                entry.document().kind() == DocumentKind::Normal
                    && entry.document().path().is_some()
                    && entry.document().is_dirty()
                    && !entry.document().is_read_only()
            })
            .map(|entry| entry.id())
            .collect::<Vec<_>>();

        if pending.is_empty() {
            self.minibuffer.set_message("No modified file buffers");
            return EditorOutcome::Continue;
        }

        self.save_some_buffers = Some(SaveSomeBuffersState { pending, saved: 0 });
        self.prompt_next_save_some_buffer()
    }

    fn submit_save_some_buffers(&mut self, input: &str) -> EditorOutcome {
        match input.to_ascii_lowercase().as_str() {
            "yes" => {
                let Some(buffer) = self.pop_pending_save_some_buffer() else {
                    return self.finish_save_some_buffers();
                };
                match self.save_buffer_by_id(buffer) {
                    Ok(()) => {
                        if let Some(state) = &mut self.save_some_buffers {
                            state.saved += 1;
                        }
                        self.prompt_next_save_some_buffer()
                    }
                    Err(error) => {
                        self.save_some_buffers = None;
                        self.refresh_visible_buffer_list();
                        self.minibuffer.set_error(format!("save failed: {error}"));
                        EditorOutcome::Continue
                    }
                }
            }
            "no" | "" => {
                let _ = self.pop_pending_save_some_buffer();
                self.prompt_next_save_some_buffer()
            }
            _ => self.prompt_next_save_some_buffer(),
        }
    }

    fn prompt_next_save_some_buffer(&mut self) -> EditorOutcome {
        loop {
            let Some(buffer) = self
                .save_some_buffers
                .as_ref()
                .and_then(|state| state.pending.first().copied())
            else {
                return self.finish_save_some_buffers();
            };
            if let Some(name) = self.buffers.name(buffer).map(str::to_owned) {
                self.minibuffer.start_prompt(
                    PromptKind::SaveSomeBuffers,
                    format!("Save file {name}? (yes or no) "),
                );
                return EditorOutcome::Continue;
            }
            let _ = self.pop_pending_save_some_buffer();
        }
    }

    fn pop_pending_save_some_buffer(&mut self) -> Option<BufferId> {
        let state = self.save_some_buffers.as_mut()?;
        if state.pending.is_empty() {
            None
        } else {
            Some(state.pending.remove(0))
        }
    }

    fn finish_save_some_buffers(&mut self) -> EditorOutcome {
        let saved = self
            .save_some_buffers
            .take()
            .map(|state| state.saved)
            .unwrap_or_default();
        self.refresh_visible_buffer_list();
        match saved {
            0 => self.minibuffer.set_message("No buffers saved"),
            1 => self.minibuffer.set_message("Saved 1 buffer"),
            count => self
                .minibuffer
                .set_message(format!("Saved {count} buffers")),
        }
        EditorOutcome::Continue
    }

    fn save_buffer_by_id(&mut self, buffer: BufferId) -> Result<()> {
        let Some(document) = self.buffers.document_mut(buffer) else {
            return Ok(());
        };
        document.save()
    }

    fn not_modified(&mut self) -> Result<()> {
        if self.document().kind() != DocumentKind::Normal {
            self.minibuffer
                .set_error("not-modified requires a normal buffer");
            return Ok(());
        }
        self.document_mut().mark_clean();
        self.refresh_visible_buffer_list();
        self.minibuffer.set_message("Modification flag cleared");
        Ok(())
    }

    fn toggle_auto_revert_mode(&mut self) {
        if self.document().kind() != DocumentKind::Normal || self.document().path().is_none() {
            self.minibuffer
                .set_error("auto-revert-mode requires a file-backed normal buffer");
            return;
        }
        let enabled = if self.auto_revert_buffers.contains(&self.current_buffer) {
            self.auto_revert_buffers.remove(&self.current_buffer);
            false
        } else {
            self.auto_revert_buffers.insert(self.current_buffer);
            true
        };
        let state = if enabled { "on" } else { "off" };
        let name = self.current_buffer_name().to_owned();
        self.minibuffer
            .set_message(format!("Auto-revert for {name} is {state}"));
    }

    fn start_write_file(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::WriteFile, "Write file: ");
        Ok(())
    }

    fn start_extended_command(&mut self) -> Result<()> {
        self.mark_keyboard_macro_prompt_start();
        self.minibuffer
            .start_prompt(PromptKind::ExtendedCommand, "M-x ");
        self.completion = Some(CompletionSession::commands(
            &self.commands,
            &self.keymap,
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_describe_function(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::DescribeFunction, "Describe function: ");
        self.completion = Some(CompletionSession::commands(
            &self.commands,
            &self.keymap,
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_describe_variable(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::DescribeVariable, "Describe variable: ");
        self.completion = Some(CompletionSession::options(
            &OptionRegistry::default(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_describe_key(&mut self, mode: DescribeKeyMode) -> Result<()> {
        self.describe_key = Some(DescribeKeyState {
            sequence: Vec::new(),
            mode,
        });
        self.minibuffer.set_message(format!("{} ", mode.prompt()));
        Ok(())
    }

    fn handle_describe_key(&mut self, key: KeyEvent) -> EditorOutcome {
        let Some(state) = &mut self.describe_key else {
            return EditorOutcome::Continue;
        };
        state.sequence.push(key);
        let sequence = state.sequence.clone();
        let mode = state.mode;

        match self.active_keymaps().resolve(&sequence) {
            KeyStackResolution::Prefix => {
                self.minibuffer.set_message(format!(
                    "{} {}-",
                    mode.prompt(),
                    format_key_sequence(&sequence)
                ));
                EditorOutcome::Continue
            }
            KeyStackResolution::Command { keymap, command } => {
                self.describe_key = None;
                match mode {
                    DescribeKeyMode::Help => {
                        let keymaps = self.active_keymaps();
                        let text = format_describe_key_help(
                            &self.commands,
                            &keymaps,
                            &sequence,
                            keymap,
                            command,
                        );
                        self.open_help_buffer(text)
                    }
                    DescribeKeyMode::Brief => {
                        let text =
                            format_describe_key_brief_message(&self.commands, &sequence, command);
                        self.minibuffer.set_message(text);
                        EditorOutcome::Continue
                    }
                }
            }
            KeyStackResolution::NoMatch => {
                self.describe_key = None;
                match mode {
                    DescribeKeyMode::Help => {
                        let text = format_unbound_key_help(&sequence);
                        self.open_help_buffer(text)
                    }
                    DescribeKeyMode::Brief => {
                        self.minibuffer
                            .set_message(format_unbound_key_message(&sequence));
                        EditorOutcome::Continue
                    }
                }
            }
        }
    }

    fn describe_function(&mut self, name: &str) -> EditorOutcome {
        let Some(command) = self.commands.get(name) else {
            self.minibuffer
                .set_message(format!("No such command: {name}"));
            return EditorOutcome::Continue;
        };
        let text = format_describe_function_help(&self.active_keymaps(), command);
        self.open_help_buffer(text)
    }

    fn describe_variable(&mut self, name: &str) -> EditorOutcome {
        let registry = OptionRegistry::default();
        let Some(option) = registry.get(name) else {
            self.minibuffer
                .set_message(format!("No such variable: {name}"));
            return EditorOutcome::Continue;
        };
        let text = format_describe_variable_help(option, self.option_value(option.id));
        self.open_help_buffer(text)
    }

    fn option_value(&self, option: OptionId) -> OptionValue {
        match option {
            OptionId::TabWidth => OptionValue::Integer(self.tab_width),
            OptionId::FillColumn => OptionValue::Integer(self.fill_column),
            OptionId::LineNumbers => OptionValue::Boolean(self.line_numbers),
            OptionId::SyntaxHighlighting => OptionValue::Boolean(self.syntax_enabled),
            OptionId::SearchHighlighting => OptionValue::Boolean(self.search_highlighting),
            OptionId::BackupOnSave => OptionValue::Boolean(self.backup_on_save),
            OptionId::Theme => OptionValue::Choice(self.theme.name()),
            OptionId::CompletionStyle => OptionValue::Choice(self.completion_config.style.name()),
            OptionId::CompletionMaxCandidates => {
                OptionValue::Integer(self.completion_config.max_candidates)
            }
            OptionId::CompletionShowAnnotations => {
                OptionValue::Boolean(self.completion_config.show_annotations)
            }
            OptionId::CompletionMatching => {
                OptionValue::Choice(self.completion_config.matching.name())
            }
        }
    }

    fn start_find_file(&mut self) -> Result<()> {
        let base_dir = self.find_file_base_dir();
        self.minibuffer
            .start_prompt(PromptKind::FindFile, "Find file: ");
        self.minibuffer
            .set_prompt_input(file_prompt_base_input(&base_dir));
        self.completion = Some(CompletionSession::files(base_dir, self.completion_config));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_find_file_read_only(&mut self) -> Result<()> {
        let base_dir = self.find_file_base_dir();
        self.minibuffer
            .start_prompt(PromptKind::FindFileReadOnly, "Find file read-only: ");
        self.minibuffer
            .set_prompt_input(file_prompt_base_input(&base_dir));
        self.completion = Some(CompletionSession::files(base_dir, self.completion_config));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_insert_file(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        let base_dir = self.find_file_base_dir();
        self.minibuffer
            .start_prompt(PromptKind::InsertFile, "Insert file: ");
        self.minibuffer
            .set_prompt_input(file_prompt_base_input(&base_dir));
        self.completion = Some(CompletionSession::files(base_dir, self.completion_config));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_goto_line(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::GotoLine, "Goto line: ");
        Ok(())
    }

    fn start_shell_command(&mut self, argument: Option<i32>) -> Result<()> {
        let action = if argument.is_some() {
            if !self.ensure_buffer_editable() {
                return Ok(());
            }
            ShellCommandAction::Insert
        } else {
            ShellCommandAction::Display
        };
        self.shell_command_prompt = Some(ShellCommandPromptState {
            action,
            stdin: String::new(),
        });
        self.minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        Ok(())
    }

    fn start_shell_command_on_region(&mut self, argument: Option<i32>) -> Result<()> {
        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        if argument.is_some() && !self.ensure_buffer_editable() {
            return Ok(());
        }
        let stdin = self.document().buffer().text_in_range(range)?;
        let action = if argument.is_some() {
            ShellCommandAction::ReplaceRegion { range }
        } else {
            ShellCommandAction::Display
        };
        self.shell_command_prompt = Some(ShellCommandPromptState { action, stdin });
        self.minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command on region: ");
        Ok(())
    }

    fn submit_shell_command(&mut self, command: &str) -> Result<EditorOutcome> {
        let Some(state) = self.shell_command_prompt.take() else {
            return Ok(EditorOutcome::Continue);
        };
        if command.is_empty() {
            self.minibuffer.set_message("Quit");
            return Ok(EditorOutcome::Continue);
        }

        let current_dir = match self.shell_command_current_dir() {
            Ok(current_dir) => current_dir,
            Err(error) => {
                self.minibuffer.set_error(error.to_string());
                return Ok(EditorOutcome::Continue);
            }
        };
        let output = match run_shell_command(command, &state.stdin, &current_dir) {
            Ok(output) => output,
            Err(error) => {
                self.minibuffer
                    .set_error(format!("shell command failed: {error}"));
                return Ok(EditorOutcome::Continue);
            }
        };

        if output.success() {
            return self.handle_successful_shell_command_output(state.action, output);
        }

        let text = format_shell_command_output(command, &output);
        self.open_shell_output_buffer(text);
        self.minibuffer.set_error(format!(
            "Shell command failed with code {}",
            format_shell_status(output.status_code)
        ));
        Ok(EditorOutcome::Continue)
    }

    fn handle_successful_shell_command_output(
        &mut self,
        action: ShellCommandAction,
        output: ShellCommandOutput,
    ) -> Result<EditorOutcome> {
        match action {
            ShellCommandAction::Display => {
                let text = format_shell_command_output("", &output);
                self.open_shell_output_buffer(text);
                self.minibuffer
                    .set_message(format_shell_success_message(&output));
            }
            ShellCommandAction::Insert => {
                if output.stdout.is_empty() {
                    self.minibuffer
                        .set_message("Shell command produced no output");
                } else {
                    self.insert_shell_stdout(&output.stdout)?;
                    let message = format_shell_mutation_message(
                        "Inserted",
                        output.stdout.len(),
                        output.stderr.len(),
                    );
                    self.minibuffer.set_message(message);
                }
            }
            ShellCommandAction::ReplaceRegion { range } => {
                if output.stdout.is_empty() {
                    self.minibuffer
                        .set_message("Shell command produced no output");
                } else {
                    self.replace_region_with_shell_stdout(range, &output.stdout)?;
                    let message = format_shell_mutation_message(
                        "Replaced region with",
                        output.stdout.len(),
                        output.stderr.len(),
                    );
                    self.minibuffer.set_message(message);
                }
            }
        }
        Ok(EditorOutcome::Continue)
    }

    fn insert_shell_stdout(&mut self, stdout: &str) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        self.cursor = self
            .document_mut()
            .buffer_mut()
            .insert(cursor_before, stdout)?;
        self.record_insert(cursor_before, self.cursor, stdout, false);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn replace_region_with_shell_stdout(&mut self, range: TextRange, stdout: &str) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let cursor_before = self.cursor;
        let old_text = self.document_mut().buffer_mut().delete_range(range)?;
        let cursor_after = self
            .document_mut()
            .buffer_mut()
            .insert(range.start, stdout)?;
        self.cursor = cursor_after;
        self.record_replace(
            TextRange::new(range.start, cursor_after),
            old_text,
            stdout.to_owned(),
            cursor_before,
            cursor_after,
        );
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        Ok(())
    }

    fn shell_command_current_dir(&self) -> Result<PathBuf> {
        let Some(path) = self.document().path() else {
            return Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        };
        let Some(parent) = path.parent() else {
            return Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        };
        if parent.as_os_str().is_empty() {
            return Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        }
        if !parent.is_dir() {
            return Err(RileError::InvalidInput(format!(
                "shell command directory does not exist: {}",
                parent.display()
            )));
        }
        Ok(parent.to_path_buf())
    }

    fn start_switch_to_buffer(&mut self) -> Result<()> {
        let label = self
            .switch_buffer_default_name()
            .map(|name| format!("Switch to buffer (default {name}): "))
            .unwrap_or_else(|| "Switch to buffer: ".to_owned());
        self.minibuffer
            .start_prompt(PromptKind::SwitchToBuffer, label);
        self.completion = Some(CompletionSession::buffers(
            self.switch_buffer_completion_names(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn switch_buffer_default_name(&self) -> Option<&str> {
        self.switch_buffer_default_id()
            .and_then(|id| self.buffers.name(id))
    }

    fn switch_buffer_default_id(&self) -> Option<BufferId> {
        if let Some(previous) = self.previous_buffer
            && previous != self.current_buffer
            && self.buffers.document(previous).is_some()
        {
            return Some(previous);
        }
        self.buffers
            .entries()
            .iter()
            .find(|entry| entry.id() != self.current_buffer)
            .map(|entry| entry.id())
            .or_else(|| {
                self.buffers
                    .document(self.current_buffer)
                    .map(|_| self.current_buffer)
            })
    }

    fn switch_buffer_completion_names(&self) -> Vec<String> {
        let default = self.switch_buffer_default_id();
        let mut names = Vec::new();
        if let Some(default) = default
            && let Some(name) = self.buffers.name(default)
        {
            names.push(name.to_owned());
        }
        names.extend(
            self.buffers
                .entries()
                .iter()
                .filter(|entry| Some(entry.id()) != default)
                .map(|entry| entry.name().to_owned()),
        );
        names
    }

    fn start_kill_buffer(&mut self) -> Result<()> {
        let label = format!("Kill buffer (default {}): ", self.current_buffer_name());
        self.minibuffer.start_prompt(PromptKind::KillBuffer, label);
        self.completion = Some(CompletionSession::buffers_with_title(
            self.kill_buffer_completion_names(),
            self.completion_config,
            "Kill buffer",
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn kill_buffer_completion_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(name) = self.buffers.name(self.current_buffer) {
            names.push(name.to_owned());
        }
        names.extend(
            self.buffers
                .entries()
                .iter()
                .filter(|entry| entry.id() != self.current_buffer)
                .map(|entry| entry.name().to_owned()),
        );
        names
    }

    fn list_buffers(&mut self) -> Result<()> {
        self.sync_current_window();
        let (text, rows) = self.buffer_list_contents();
        let buffer_list = self.buffers.open_buffer_list(text);
        self.buffer_list_rows = rows;
        let target = self
            .windows
            .window_showing_buffer(buffer_list)
            .unwrap_or_else(|| {
                if self.windows.len() == 1 {
                    self.windows.split_current(SplitAxis::Horizontal)
                } else {
                    self.windows.next_window_id()
                }
            });

        if let Some(window) = self.windows.window_mut(target) {
            *window.viewport_mut() = Viewport::new(buffer_list);
        }
        self.minibuffer.clear();
        Ok(())
    }

    fn buffer_list_contents(&self) -> (String, Vec<Option<BufferId>>) {
        let mut text =
            String::from("CRM Buffer                           Size Mode         File\n");
        text.push_str("--- ------                           ---- ----         ----\n");
        let mut rows = vec![None, None];
        for entry in self
            .buffers
            .entries()
            .iter()
            .filter(|entry| !entry.document().is_buffer_list())
        {
            let document = entry.document();
            let current = if entry.id() == self.current_buffer {
                '.'
            } else {
                ' '
            };
            let read_only = if document.is_read_only() { '%' } else { ' ' };
            let modified = if document.is_dirty() { '*' } else { ' ' };
            let size = document.buffer().serialize().len();
            let mode = MajorMode::for_path(document.path()).name();
            let file = document
                .path()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            text.push_str(&format!(
                "{current}{read_only}{modified} {:<32} {:>4} {:<12} {file}\n",
                entry.name(),
                size,
                mode,
            ));
            rows.push(Some(entry.id()));
        }
        (text, rows)
    }

    fn refresh_visible_buffer_list(&mut self) {
        let Some(buffer_list) = self.buffers.find_by_name("*Buffer List*") else {
            return;
        };
        if self.windows.window_showing_buffer(buffer_list).is_none() {
            return;
        }
        let (text, rows) = self.buffer_list_contents();
        self.buffers.open_buffer_list(text);
        self.buffer_list_rows = rows;
    }

    fn find_file(&mut self, path: &str) -> Result<EditorOutcome> {
        self.open_file_path(path, false)
    }

    fn find_file_read_only(&mut self, path: &str) -> Result<EditorOutcome> {
        self.open_file_path(path, true)
    }

    fn open_file_path(&mut self, path: &str, read_only: bool) -> Result<EditorOutcome> {
        if path.is_empty() {
            self.minibuffer.set_error("missing file name");
            return Ok(EditorOutcome::Continue);
        }

        let path = self.resolve_find_file_path(path);
        let result = if read_only {
            self.buffers.open_path_read_only(&path, self.backup_on_save)
        } else {
            self.buffers
                .open_path_with_backup(&path, self.backup_on_save)
        };
        match result {
            Ok(opened) => {
                self.remember_buffer_transition(opened.id);
                self.current_buffer = opened.id;
                self.cursor = Position::new(0, 0);
                self.goal_display_column = None;
                self.search = None;
                self.query_replace = None;
                self.deactivate_region();
                self.clear_insert_group();
                self.sync_current_window();
                self.refresh_visible_buffer_list();
                let mode = if read_only { " read-only" } else { "" };
                self.minibuffer
                    .set_message(format!("Opened{mode} {}", self.document().display_name()));
            }
            Err(error) => self.minibuffer.set_error(format!("open failed: {error}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn find_file_base_dir(&self) -> PathBuf {
        self.document()
            .path()
            .and_then(Path::parent)
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn resolve_find_file_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.find_file_base_dir().join(path)
        }
    }

    fn find_file_input_is_exact_file(&self, input: &str) -> bool {
        self.resolve_find_file_path(input).is_file()
    }

    fn write_file(&mut self, path: &str) -> Result<EditorOutcome> {
        if path.is_empty() {
            self.minibuffer.set_error("missing file name");
            return Ok(EditorOutcome::Continue);
        }

        match self.document_mut().save_as(path) {
            Ok(()) => {
                self.refresh_visible_buffer_list();
                self.minibuffer
                    .set_message(format!("Wrote {}", self.document().display_name()));
            }
            Err(error) => self.minibuffer.set_error(format!("save failed: {error}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn insert_file(&mut self, path: &str) -> Result<EditorOutcome> {
        if path.is_empty() {
            self.minibuffer.set_error("missing file name");
            return Ok(EditorOutcome::Continue);
        }
        if !self.ensure_buffer_editable() {
            return Ok(EditorOutcome::Continue);
        }

        let path = self.resolve_find_file_path(path);
        match crate::file::read_text_file(&path) {
            Ok(text) => {
                let cursor_before = self.cursor;
                self.cursor = self
                    .document_mut()
                    .buffer_mut()
                    .insert(cursor_before, &text)?;
                self.record_insert(cursor_before, self.cursor, &text, false);
                self.goal_display_column = None;
                self.deactivate_region();
                self.sync_current_window();
                self.minibuffer
                    .set_message(format!("Inserted {}", path.display()));
            }
            Err(error) => self.minibuffer.set_error(format!("insert failed: {error}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn goto_line(&mut self, input: &str) -> Result<EditorOutcome> {
        let Ok((line, column)) = parse_goto_line_input(input) else {
            self.minibuffer.set_error(if input.is_empty() {
                "missing line number"
            } else {
                "invalid line number"
            });
            return Ok(EditorOutcome::Continue);
        };

        let buffer = self.document().buffer();
        let target_line = (line - 1).min(buffer.line_count() - 1);
        let target_byte = buffer.byte_for_display_column(target_line, column)?;

        self.clear_insert_group();
        self.cursor = Position::new(target_line, target_byte);
        self.goal_display_column = None;
        self.sync_current_window();
        Ok(EditorOutcome::Continue)
    }

    fn switch_to_buffer(&mut self, name: &str) -> Result<EditorOutcome> {
        if name.trim().is_empty() {
            self.minibuffer.set_error("missing buffer name");
            return Ok(EditorOutcome::Continue);
        }

        match self.buffers.find_by_name(name) {
            Some(id) => {
                self.sync_current_window();
                self.restore_buffer_in_current_window(id);
                self.remember_buffer_transition(id);
                self.current_buffer = id;
                self.goal_display_column = None;
                self.search = None;
                self.query_replace = None;
                self.deactivate_region();
                self.clear_insert_group();
                self.refresh_visible_buffer_list();
                self.minibuffer
                    .set_message(format!("Switched to buffer {name}"));
            }
            None => self.minibuffer.set_error(format!("no such buffer: {name}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn kill_buffer(&mut self, name: &str) -> Result<EditorOutcome> {
        let target = if name.is_empty() {
            self.current_buffer
        } else if let Some(id) = self.buffers.find_by_name(name) {
            id
        } else {
            self.minibuffer.set_error(format!("no such buffer: {name}"));
            return Ok(EditorOutcome::Continue);
        };
        let target_name = self
            .buffers
            .name(target)
            .expect("target buffer must exist")
            .to_owned();

        if self
            .buffers
            .document(target)
            .expect("target buffer must exist")
            .is_dirty()
        {
            self.finish_completion_buffer();
            self.completion = None;
            self.pending_kill_buffer = Some(target);
            self.minibuffer
                .start_prompt(PromptKind::KillDirtyBuffer, dirty_kill_prompt(&target_name));
            return Ok(EditorOutcome::Continue);
        }

        self.finish_kill_buffer(target, false)
    }

    fn submit_kill_dirty_buffer(&mut self, input: &str) -> EditorOutcome {
        match input.to_ascii_lowercase().as_str() {
            "y" => {
                let Some(target) = self.pending_kill_buffer.take() else {
                    self.minibuffer.set_error("no pending buffer kill");
                    return EditorOutcome::Continue;
                };
                let Some(target_name) = self.buffers.name(target).map(str::to_owned) else {
                    self.minibuffer.set_error("buffer no longer exists");
                    return EditorOutcome::Continue;
                };
                let message = format!("{}y", dirty_kill_prompt(&target_name));
                if let Err(error) =
                    self.finish_kill_buffer_with_message(target, true, Some(message))
                {
                    self.minibuffer.set_error(format!("kill failed: {error}"));
                }
                EditorOutcome::Continue
            }
            "n" | "" => {
                self.pending_kill_buffer = None;
                self.minibuffer.set_message("Quit");
                EditorOutcome::Continue
            }
            _ => {
                if let Some(name) = self
                    .pending_kill_buffer
                    .and_then(|id| self.buffers.name(id))
                    .map(str::to_owned)
                {
                    self.minibuffer
                        .start_prompt(PromptKind::KillDirtyBuffer, dirty_kill_prompt(&name));
                } else {
                    self.pending_kill_buffer = None;
                    self.minibuffer.set_error("buffer no longer exists");
                }
                EditorOutcome::Continue
            }
        }
    }

    fn finish_kill_buffer(&mut self, target: BufferId, confirmed: bool) -> Result<EditorOutcome> {
        self.finish_kill_buffer_with_message(target, confirmed, None)
    }

    fn finish_kill_buffer_with_message(
        &mut self,
        target: BufferId,
        confirmed: bool,
        message: Option<String>,
    ) -> Result<EditorOutcome> {
        let target_name = self
            .buffers
            .name(target)
            .expect("target buffer must exist")
            .to_owned();

        let result = if confirmed {
            self.buffers.kill_confirmed(target)
        } else {
            self.buffers.kill(target)
        };

        match result {
            Ok(next_current) => {
                self.buffer_viewports.remove(&target);
                self.windows.replace_buffer(target, next_current);
                if target == self.current_buffer {
                    self.current_buffer = next_current;
                    self.cursor = Position::new(0, 0);
                    self.goal_display_column = None;
                    self.search = None;
                    self.query_replace = None;
                    self.deactivate_region();
                    self.clear_insert_group();
                    self.sync_current_window();
                }
                self.refresh_visible_buffer_list();
                self.minibuffer
                    .set_message(message.unwrap_or_else(|| format!("Killed buffer {target_name}")));
            }
            Err(error) => self.minibuffer.set_error(format!("kill failed: {error}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn start_incremental_search(&mut self, direction: SearchDirection) -> Result<()> {
        self.sync_current_window();
        self.query_replace = None;
        self.search = Some(SearchState {
            direction,
            origin: self.cursor,
            current: None,
            failed_direction: None,
        });
        self.minibuffer
            .start_prompt(PromptKind::IncrementalSearch, direction.label());
        Ok(())
    }

    fn start_query_replace(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.minibuffer
            .start_prompt(PromptKind::QueryReplaceSearch, "Query replace: ");
        Ok(())
    }

    fn submit_query_replace_search(&mut self, query: &str) -> Result<EditorOutcome> {
        if query.is_empty() {
            self.query_replace = None;
            self.minibuffer.set_error("missing search string");
            return Ok(EditorOutcome::Continue);
        }

        self.query_replace = Some(QueryReplaceState {
            query: query.to_owned(),
            replacement: String::new(),
            current: None,
            replacements: 0,
            visited: false,
        });
        self.minibuffer.start_prompt(
            PromptKind::QueryReplaceReplacement,
            format!("Query replace {query} with: "),
        );
        Ok(EditorOutcome::Continue)
    }

    fn submit_query_replace_replacement(&mut self, replacement: &str) -> Result<EditorOutcome> {
        let Some(query_replace) = &mut self.query_replace else {
            self.minibuffer.set_error("query replace is not active");
            return Ok(EditorOutcome::Continue);
        };
        query_replace.replacement = replacement.to_owned();
        self.advance_query_replace(self.cursor)?;
        Ok(EditorOutcome::Continue)
    }

    fn handle_query_replace_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        match key {
            KeyEvent::Text(text) if text == "y" || text == " " => {
                let next_start = self.replace_query_replace_current()?;
                self.advance_query_replace(next_start)?;
            }
            KeyEvent::Text(text) if text == "n" => {
                if let Some(current) = self.query_replace.as_ref().and_then(|state| state.current) {
                    self.cursor = current.end;
                    self.goal_display_column = None;
                    self.sync_current_window();
                    self.advance_query_replace(current.end)?;
                } else {
                    self.finish_query_replace(false);
                }
            }
            KeyEvent::Text(text) if text == "!" => {
                while self
                    .query_replace
                    .as_ref()
                    .and_then(|state| state.current)
                    .is_some()
                {
                    let next_start = self.replace_query_replace_current()?;
                    self.advance_query_replace(next_start)?;
                }
            }
            KeyEvent::Text(text) if text == "q" => self.finish_query_replace(false),
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
                self.finish_query_replace(true);
            }
            _ => self.set_query_replace_choice_message(),
        }
        Ok(EditorOutcome::Continue)
    }

    fn advance_query_replace(&mut self, start: Position) -> Result<()> {
        let Some(query) = self.query_replace.as_ref().map(|state| state.query.clone()) else {
            return Ok(());
        };

        let found = find_match(
            self.document().buffer(),
            &query,
            start,
            SearchDirection::Forward,
        )?;
        if let Some(range) = found {
            if let Some(state) = &mut self.query_replace {
                state.current = Some(range);
                state.visited = true;
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.sync_current_window();
            self.set_query_replace_choice_message();
        } else if self
            .query_replace
            .as_ref()
            .is_some_and(|state| state.replacements == 0 && !state.visited)
        {
            self.query_replace = None;
            self.minibuffer
                .set_message(format!("No matches for {query}"));
        } else {
            self.finish_query_replace(false);
        }
        Ok(())
    }

    fn set_query_replace_choice_message(&mut self) {
        let Some(state) = &self.query_replace else {
            self.minibuffer
                .set_message("Query replace: type y, n, !, or q");
            return;
        };
        self.minibuffer.set_message(format!(
            "Query replacing {} with {}: (y, n, !, q)?",
            state.query, state.replacement
        ));
    }

    fn replace_query_replace_current(&mut self) -> Result<Position> {
        if !self.ensure_buffer_editable() {
            return Ok(self.cursor);
        }
        let Some((old_range, replacement)) = self.query_replace.as_ref().and_then(|state| {
            state
                .current
                .map(|current| (current, state.replacement.clone()))
        }) else {
            return Ok(self.cursor);
        };

        let old_text = self.document_mut().buffer_mut().delete_range(old_range)?;
        let new_end = self
            .document_mut()
            .buffer_mut()
            .insert(old_range.start, &replacement)?;
        self.cursor = new_end;
        self.goal_display_column = None;
        self.record_replace(
            TextRange::new(old_range.start, new_end),
            old_text,
            replacement,
            old_range.start,
            new_end,
        );
        if let Some(state) = &mut self.query_replace {
            state.current = None;
            state.replacements += 1;
        }
        self.sync_current_window();
        Ok(new_end)
    }

    fn finish_query_replace(&mut self, cancelled: bool) {
        let replacements = self
            .query_replace
            .take()
            .map(|state| state.replacements)
            .unwrap_or(0);
        let noun = if replacements == 1 {
            "replacement"
        } else {
            "replacements"
        };
        if cancelled {
            self.minibuffer
                .set_message(format!("Quit query replace ({replacements} {noun})"));
        } else {
            let noun = if replacements == 1 {
                "occurrence"
            } else {
                "occurrences"
            };
            self.minibuffer
                .set_message(format!("Replaced {replacements} {noun}"));
        }
    }

    fn handle_search_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                self.minibuffer.clear();
                self.search = None;
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
                if let Some(search) = self.search.take() {
                    self.cursor = search.origin;
                    self.goal_display_column = None;
                    self.sync_current_window();
                }
                self.minibuffer.cancel_prompt();
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                if self.minibuffer.delete_prompt_grapheme_backward() {
                    self.update_incremental_search()?;
                }
                self.record_prompt_non_kill_key();
            }
            KeyEvent::Ctrl('s') => self.repeat_incremental_search(SearchDirection::Forward)?,
            KeyEvent::Ctrl('r') => self.repeat_incremental_search(SearchDirection::Backward)?,
            KeyEvent::Text(text) => {
                self.minibuffer.insert_prompt_text(&text);
                self.update_incremental_search()?;
                self.record_prompt_non_kill_key();
            }
            KeyEvent::Special(SpecialKey::Tab) => {}
            key => {
                if let PromptEditOutcome::Handled { changed } = self.handle_prompt_edit_key(&key)
                    && changed
                {
                    self.update_incremental_search()?;
                }
            }
        }
        Ok(EditorOutcome::Continue)
    }

    fn update_incremental_search(&mut self) -> Result<()> {
        let Some(query) = self.minibuffer.prompt_input().map(str::to_owned) else {
            return Ok(());
        };
        let Some(search) = self.search.as_ref() else {
            return Ok(());
        };

        let origin = search.origin;
        let direction = search.direction;
        if query.is_empty() {
            if let Some(search) = &mut self.search {
                search.current = None;
                search.failed_direction = None;
            }
            self.cursor = origin;
            self.goal_display_column = None;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.label());
            return Ok(());
        }

        let found = find_match(self.document().buffer(), &query, origin, direction)?;
        if let Some(range) = found {
            if let Some(search) = &mut self.search {
                search.current = Some(range);
                search.failed_direction = None;
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.label());
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
                search.failed_direction = Some(direction);
            }
            self.cursor = origin;
            self.goal_display_column = None;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.failing_label());
        }
        Ok(())
    }

    fn repeat_incremental_search(&mut self, direction: SearchDirection) -> Result<()> {
        let Some(query) = self.minibuffer.prompt_input().map(str::to_owned) else {
            return Ok(());
        };
        let Some(search) = self.search.as_ref() else {
            return Ok(());
        };

        let previous_cursor = self.cursor;
        let is_wrapping = search.current.is_none() && search.failed_direction == Some(direction);
        let start = match (direction, search.current, is_wrapping) {
            (SearchDirection::Forward, Some(range), _) => {
                search_start_after(self.document().buffer(), range.start)?
            }
            (SearchDirection::Backward, Some(range), _) => range.start,
            (SearchDirection::Forward, None, true) => Position::new(0, 0),
            (SearchDirection::Backward, None, true) => self.document().buffer().end_position(),
            (_, None, false) => search.origin,
        };

        if let Some(search) = &mut self.search {
            search.direction = direction;
        }

        if query.is_empty() {
            self.minibuffer.set_prompt_label(direction.label());
            return Ok(());
        }

        let found = find_match(self.document().buffer(), &query, start, direction)?;
        if let Some(range) = found {
            if let Some(search) = &mut self.search {
                search.current = Some(range);
                search.failed_direction = None;
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.sync_current_window();
            let label = if is_wrapping {
                direction.wrapped_label()
            } else {
                direction.label()
            };
            self.minibuffer.set_prompt_label(label);
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
                search.failed_direction = Some(direction);
            }
            self.cursor = previous_cursor;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.failing_label());
        }
        Ok(())
    }

    fn split_window(&mut self, axis: SplitAxis) -> Result<()> {
        self.sync_current_window();
        self.windows.split_current(axis);
        self.load_current_window();
        self.minibuffer.set_message(match axis {
            SplitAxis::Horizontal => "Split window below",
            SplitAxis::Vertical => "Split window right",
        });
        Ok(())
    }

    fn delete_window(&mut self) -> Result<()> {
        if self.windows.len() <= 1 {
            self.minibuffer.set_message("Only one window");
            return Ok(());
        }
        self.sync_current_window();
        self.windows.delete_current();
        self.load_current_window();
        self.minibuffer.set_message("Deleted window");
        Ok(())
    }

    fn delete_other_windows(&mut self) -> Result<()> {
        self.sync_current_window();
        self.windows.delete_others();
        self.load_current_window();
        self.minibuffer.set_message("Deleted other windows");
        Ok(())
    }

    fn other_window(&mut self) -> Result<()> {
        self.sync_current_window();
        self.windows.other_window();
        self.load_current_window();
        self.minibuffer.set_message("Selected other window");
        Ok(())
    }

    fn toggle_syntax_highlighting(&mut self) -> Result<()> {
        self.syntax_enabled = !self.syntax_enabled;
        let status = if self.syntax_enabled {
            "enabled"
        } else {
            "disabled"
        };
        self.minibuffer
            .set_message(format!("Syntax highlighting {status}"));
        Ok(())
    }

    fn toggle_search_highlighting(&mut self) -> Result<()> {
        self.search_highlighting = !self.search_highlighting;
        let status = if self.search_highlighting {
            "enabled"
        } else {
            "disabled"
        };
        self.minibuffer
            .set_message(format!("Search highlighting {status}"));
        Ok(())
    }

    fn toggle_line_numbers(&mut self) -> Result<()> {
        self.line_numbers = !self.line_numbers;
        let status = if self.line_numbers {
            "enabled"
        } else {
            "disabled"
        };
        self.minibuffer
            .set_message(format!("Line numbers {status}"));
        Ok(())
    }

    fn toggle_read_only(&mut self) -> Result<()> {
        if self.document().kind() != DocumentKind::Normal {
            let name = self.current_buffer_name().to_owned();
            self.minibuffer
                .set_message(format!("Buffer is read-only: {name}"));
            return Ok(());
        }

        let read_only = !self.document().is_read_only();
        self.document_mut().set_read_only(read_only);
        let status = if read_only { "read-only" } else { "writable" };
        self.refresh_visible_buffer_list();
        self.minibuffer
            .set_message(format!("Buffer is now {status}"));
        Ok(())
    }

    fn clear_key_sequence(&mut self) {
        self.key_sequence.clear();
    }
}

impl Editor {
    fn document_mut(&mut self) -> &mut Document {
        self.buffers
            .document_mut(self.current_buffer)
            .expect("current buffer must exist")
    }

    fn active_region_range(&self) -> Option<TextRange> {
        let region = self.region?;
        if !region.active
            || region.buffer != self.current_buffer
            || region.mark == self.cursor
            || region.shape != RegionShape::Linear
        {
            return None;
        }
        let (start, end) = if region.mark < self.cursor {
            (region.mark, self.cursor)
        } else {
            (self.cursor, region.mark)
        };
        Some(TextRange::new(start, end))
    }

    fn active_rectangle_bounds(&self) -> Option<RectangleBounds> {
        let region = self.region?;
        if !region.active
            || region.buffer != self.current_buffer
            || region.mark == self.cursor
            || region.shape != RegionShape::Rectangle
        {
            return None;
        }

        let buffer = self.document().buffer();
        let mark_column = buffer.display_column(region.mark).ok()?;
        let cursor_column = buffer.display_column(self.cursor).ok()?;
        let start_column = mark_column.min(cursor_column);
        let end_column = mark_column.max(cursor_column);
        if start_column == end_column {
            return None;
        }

        Some(RectangleBounds {
            start_line: region.mark.line.min(self.cursor.line),
            end_line: region.mark.line.max(self.cursor.line),
            start_column,
            end_column,
        })
    }

    fn rectangle_bounds_from_mark(&self) -> Option<RectangleBounds> {
        let region = self.region?;
        if region.buffer != self.current_buffer || region.mark == self.cursor {
            return None;
        }

        let buffer = self.document().buffer();
        let mark_column = buffer.display_column(region.mark).ok()?;
        let cursor_column = buffer.display_column(self.cursor).ok()?;
        let start_column = mark_column.min(cursor_column);
        let end_column = mark_column.max(cursor_column);
        if start_column == end_column {
            return None;
        }

        Some(RectangleBounds {
            start_line: region.mark.line.min(self.cursor.line),
            end_line: region.mark.line.max(self.cursor.line),
            start_column,
            end_column,
        })
    }

    fn rectangle_position(&self, line: usize, column: usize) -> Result<Position> {
        Ok(Position::new(
            line,
            self.document()
                .buffer()
                .byte_for_display_column(line, column)?,
        ))
    }

    fn deactivate_region(&mut self) {
        if let Some(region) = &mut self.region {
            region.active = false;
        }
    }

    fn push_kill(&mut self, entry: KillEntry) {
        if !entry.is_empty() {
            self.kill_ring.push(entry);
        }
    }

    fn push_command_kill(&mut self, entry: KillEntry, direction: KillDirection) {
        if entry.is_empty() {
            return;
        }

        if self.last_command_was_kill
            && let Some(previous) = self.kill_ring.last_mut()
            && let (KillEntry::Text(previous), KillEntry::Text(text)) = (previous, &entry)
        {
            match direction {
                KillDirection::Forward => previous.push_str(text),
                KillDirection::Backward => {
                    let mut combined = text.clone();
                    combined.push_str(previous);
                    *previous = combined;
                }
            }
        } else {
            self.kill_ring.push(entry);
        }

        self.kill_recorded_this_command = true;
    }

    fn record_batch_delete(
        &mut self,
        deletes: Vec<RectangleEdit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        let records = deletes
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(range, text)| UndoRecord::Delete {
                range,
                text,
                cursor_before,
                cursor_after,
            })
            .collect::<Vec<_>>();
        self.record_batch(records);
    }

    fn record_batch_insert(
        &mut self,
        inserts: Vec<RectangleEdit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        let records = inserts
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(range, text)| UndoRecord::Insert {
                range,
                text,
                cursor_before,
                cursor_after,
            })
            .collect::<Vec<_>>();
        self.record_batch(records);
    }

    fn record_rectangle_replace(
        &mut self,
        deletes: Vec<RectangleEdit>,
        inserts: Vec<RectangleEdit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        let mut records = deletes
            .into_iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(range, text)| UndoRecord::Delete {
                range,
                text,
                cursor_before,
                cursor_after,
            })
            .collect::<Vec<_>>();
        records.extend(
            inserts
                .into_iter()
                .filter(|(_, text)| !text.is_empty())
                .map(|(range, text)| UndoRecord::Insert {
                    range,
                    text,
                    cursor_before,
                    cursor_after,
                }),
        );
        self.record_batch(records);
    }

    fn record_batch(&mut self, records: Vec<UndoRecord>) {
        if records.is_empty() {
            return;
        }
        self.undo_stack.push(UndoEntry {
            buffer: self.current_buffer,
            record: UndoRecord::Batch(records),
        });
        self.clear_insert_group();
        self.refresh_visible_buffer_list();
    }

    fn latest_rectangle_kill_index(&self) -> Option<usize> {
        self.kill_ring
            .iter()
            .rposition(|entry| matches!(entry, KillEntry::Rectangle(_)))
    }

    fn previous_text_kill_index(&self, from: usize) -> Option<usize> {
        if self.kill_ring.is_empty() || from >= self.kill_ring.len() {
            return None;
        }

        let mut index = if from == 0 {
            self.kill_ring.len() - 1
        } else {
            from - 1
        };
        loop {
            if matches!(self.kill_ring[index], KillEntry::Text(_)) {
                return Some(index);
            }
            if index == from {
                return None;
            }
            index = if index == 0 {
                self.kill_ring.len() - 1
            } else {
                index - 1
            };
        }
    }

    fn record_insert(
        &mut self,
        start: Position,
        end: Position,
        text: &str,
        group_with_previous: bool,
    ) {
        if text.is_empty() {
            return;
        }
        let can_group = group_with_previous && self.grouping_insert && !text.contains('\n');
        if can_group
            && let Some(UndoEntry {
                buffer,
                record:
                    UndoRecord::Insert {
                        range,
                        text: existing_text,
                        cursor_after,
                        ..
                    },
            }) = self.undo_stack.last_mut()
            && *buffer == self.current_buffer
            && *cursor_after == start
        {
            range.end = end;
            existing_text.push_str(text);
            *cursor_after = end;
            self.grouping_insert = true;
            self.refresh_visible_buffer_list();
            return;
        }
        self.undo_stack.push(UndoEntry {
            buffer: self.current_buffer,
            record: UndoRecord::Insert {
                range: TextRange::new(start, end),
                text: text.to_owned(),
                cursor_before: start,
                cursor_after: end,
            },
        });
        self.grouping_insert = group_with_previous && !text.contains('\n');
        self.refresh_visible_buffer_list();
    }

    fn record_delete(
        &mut self,
        range: TextRange,
        text: String,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if text.is_empty() {
            return;
        }
        self.undo_stack.push(UndoEntry {
            buffer: self.current_buffer,
            record: UndoRecord::Delete {
                range,
                text,
                cursor_before,
                cursor_after,
            },
        });
        self.clear_insert_group();
        self.refresh_visible_buffer_list();
    }

    fn record_replace(
        &mut self,
        range: TextRange,
        old_text: String,
        new_text: String,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if old_text == new_text {
            return;
        }
        self.undo_stack.push(UndoEntry {
            buffer: self.current_buffer,
            record: UndoRecord::Replace {
                range,
                old_text,
                new_text,
                cursor_before,
                cursor_after,
            },
        });
        self.clear_insert_group();
        self.refresh_visible_buffer_list();
    }

    fn clear_insert_group(&mut self) {
        self.grouping_insert = false;
    }

    fn clear_transient_message(&mut self) {
        if self.minibuffer.prompt().is_none() {
            self.minibuffer.clear();
        }
    }

    fn ensure_buffer_editable(&mut self) -> bool {
        if self.document().is_read_only() {
            let name = self.current_buffer_name().to_owned();
            self.minibuffer
                .set_message(format!("Buffer is read-only: {name}"));
            false
        } else {
            true
        }
    }

    fn sync_current_window(&mut self) {
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.buffer = self.current_buffer;
        viewport.cursor = self.cursor;
        self.buffer_viewports.insert(self.current_buffer, *viewport);
    }

    fn remember_buffer_transition(&mut self, next: BufferId) {
        if next != self.current_buffer && self.buffers.document(self.current_buffer).is_some() {
            self.previous_buffer = Some(self.current_buffer);
        }
    }

    fn restore_buffer_in_current_window(&mut self, buffer: BufferId) {
        let text_rows = self.windows.current().viewport().text_rows;
        let mut viewport = self.saved_viewport_for_buffer(buffer);
        viewport.buffer = buffer;
        viewport.text_rows = text_rows;
        self.cursor = viewport.cursor;
        *self.windows.current_mut().viewport_mut() = viewport;
        self.buffer_viewports.insert(buffer, viewport);
    }

    fn saved_viewport_for_buffer(&self, buffer: BufferId) -> Viewport {
        self.buffer_viewports
            .get(&buffer)
            .copied()
            .or_else(|| {
                self.windows
                    .window_showing_buffer(buffer)
                    .and_then(|window| self.windows.window(window).map(|window| *window.viewport()))
            })
            .unwrap_or_else(|| Viewport::new(buffer))
    }

    fn load_current_window(&mut self) {
        let viewport = *self.windows.current().viewport();
        self.current_buffer = viewport.buffer;
        self.cursor = viewport.cursor;
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.clear_insert_group();
    }
}

fn parse_goto_line_input(input: &str) -> std::result::Result<(usize, usize), ()> {
    let input = input.trim();
    if input.is_empty() {
        return Err(());
    }

    let (line, column) = match input.split_once(':') {
        Some((line, column)) => (line.trim(), Some(column.trim())),
        None => (input, None),
    };

    let line = line.parse::<usize>().map_err(|_| ())?;
    if line == 0 {
        return Err(());
    }

    let column = match column {
        Some("") => return Err(()),
        Some(column) => column.parse::<usize>().map_err(|_| ())?,
        None => 0,
    };

    Ok((line, column))
}

fn is_kill_command(command: Command) -> bool {
    matches!(
        command,
        Command::BackwardKillWord
            | Command::KillLine
            | Command::KillRegion
            | Command::KillRectangle
            | Command::KillWord
    )
}

fn is_horizontal_space_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

fn trailing_horizontal_space_start(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut start = bytes.len();
    while start > 0 && is_horizontal_space_byte(bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn whitespace_run_around(text: &str, byte: usize, include_newlines: bool) -> (usize, usize) {
    let mut start = byte;
    while start > 0 && is_collapsible_space(text.as_bytes()[start - 1], include_newlines) {
        start -= 1;
    }

    let mut end = byte;
    while end < text.len() && is_collapsible_space(text.as_bytes()[end], include_newlines) {
        end += 1;
    }
    (start, end)
}

fn is_collapsible_space(byte: u8, include_newlines: bool) -> bool {
    is_horizontal_space_byte(byte) || (include_newlines && byte == b'\n')
}

fn adjust_position_after_same_line_delete(position: Position, range: TextRange) -> Position {
    if position.line != range.start.line || position.byte <= range.start.byte {
        return position;
    }

    if position.byte >= range.end.byte {
        Position::new(
            position.line,
            position.byte - (range.end.byte - range.start.byte),
        )
    } else {
        Position::new(position.line, range.start.byte)
    }
}

fn is_blank_line(line: &str) -> bool {
    line.bytes().all(is_horizontal_space_byte)
}

fn is_paragraph_separator_line(line: &str) -> bool {
    line.bytes()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\x0c'))
}

fn paragraph_forward_position(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let lines = buffer.lines();
    let mut line = position.line;

    if is_paragraph_separator_line(&lines[line]) {
        while line < lines.len() && is_paragraph_separator_line(&lines[line]) {
            line += 1;
        }
    }

    while line < lines.len() && !is_paragraph_separator_line(&lines[line]) {
        line += 1;
    }

    if line < lines.len() {
        Ok(Position::new(line, 0))
    } else {
        Ok(buffer.end_position())
    }
}

fn paragraph_backward_position(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let lines = buffer.lines();
    let mut line = position.line;

    if is_paragraph_separator_line(&lines[line]) {
        while line > 0 && is_paragraph_separator_line(&lines[line]) {
            line -= 1;
        }
        if is_paragraph_separator_line(&lines[line]) {
            return Ok(Position::new(0, 0));
        }
        while line > 0 && !is_paragraph_separator_line(&lines[line - 1]) {
            line -= 1;
        }
        return Ok(Position::new(line, 0));
    }

    if position.byte > 0 || (line > 0 && !is_paragraph_separator_line(&lines[line - 1])) {
        while line > 0 && !is_paragraph_separator_line(&lines[line - 1]) {
            line -= 1;
        }
        return Ok(Position::new(line, 0));
    }

    if line == 0 {
        return Ok(Position::new(0, 0));
    }

    line -= 1;
    while line > 0 && is_paragraph_separator_line(&lines[line]) {
        line -= 1;
    }
    if is_paragraph_separator_line(&lines[line]) {
        return Ok(Position::new(0, 0));
    }
    while line > 0 && !is_paragraph_separator_line(&lines[line - 1]) {
        line -= 1;
    }
    Ok(Position::new(line, 0))
}

fn sentence_forward_position(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let text = buffer.serialize();
    let absolute = position_to_absolute(buffer, position)?;
    let sentence_end = next_sentence_end(&text, absolute).unwrap_or(text.len());
    let paragraph_end = next_paragraph_boundary(&text, absolute)
        .filter(|boundary| *boundary > absolute)
        .unwrap_or(text.len());
    Ok(absolute_to_position(&text, sentence_end.min(paragraph_end)))
}

fn sentence_backward_position(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let text = buffer.serialize();
    let absolute = position_to_absolute(buffer, position)?;
    let start = previous_sentence_start(&text, absolute).unwrap_or(0);
    Ok(absolute_to_position(&text, start))
}

fn next_sentence_end(text: &str, from: usize) -> Option<usize> {
    for (offset, character) in text[from..].char_indices() {
        if !matches!(character, '.' | '?' | '!') {
            continue;
        }
        let punctuation_end = from + offset + character.len_utf8();
        let end = sentence_end_after_closers(text, punctuation_end);
        if sentence_boundary_after(text, end) {
            return Some(end);
        }
    }
    None
}

fn previous_sentence_start(text: &str, from: usize) -> Option<usize> {
    let mut starts = vec![0];
    for end in sentence_end_positions_before(text, from) {
        let start = skip_sentence_space(text, end);
        if start < text.len() {
            starts.push(start);
        }
    }
    for start in paragraph_start_positions_before(text, from) {
        starts.push(start);
    }
    starts.into_iter().filter(|start| *start < from).max()
}

fn sentence_end_positions_before(text: &str, before: usize) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut search = 0;
    while search < before {
        let Some(end) = next_sentence_end(text, search) else {
            break;
        };
        if end >= before {
            break;
        }
        positions.push(end);
        search = end;
    }
    positions
}

fn sentence_end_after_closers(text: &str, mut byte: usize) -> usize {
    while byte < text.len() {
        let character = text[byte..]
            .chars()
            .next()
            .expect("byte before text end has a character");
        if !matches!(character, '"' | '\'' | ')' | ']' | '}') {
            break;
        }
        byte += character.len_utf8();
    }
    byte
}

fn sentence_boundary_after(text: &str, byte: usize) -> bool {
    if byte >= text.len() {
        return true;
    }
    if text[byte..].starts_with("  ") {
        return true;
    }

    let bytes = text.as_bytes();
    let mut cursor = byte;
    while cursor < bytes.len() && is_horizontal_space_byte(bytes[cursor]) {
        cursor += 1;
    }
    cursor >= bytes.len() || bytes[cursor] == b'\n'
}

fn skip_sentence_space(text: &str, mut byte: usize) -> usize {
    while byte < text.len() {
        let character = text[byte..]
            .chars()
            .next()
            .expect("byte before text end has a character");
        if !character.is_whitespace() {
            break;
        }
        byte += character.len_utf8();
    }
    byte
}

fn next_paragraph_boundary(text: &str, from: usize) -> Option<usize> {
    paragraph_boundaries(text)
        .into_iter()
        .map(|(boundary, _)| boundary)
        .find(|boundary| *boundary >= from)
}

fn paragraph_start_positions_before(text: &str, before: usize) -> Vec<usize> {
    paragraph_boundaries(text)
        .into_iter()
        .filter_map(|(boundary, start)| (boundary < before && start < before).then_some(start))
        .collect()
}

fn paragraph_boundaries(text: &str) -> Vec<(usize, usize)> {
    let mut boundaries = Vec::new();
    let mut separator_start = None;
    let mut line_start = 0;

    while line_start <= text.len() {
        let line_end = text[line_start..]
            .find('\n')
            .map(|offset| line_start + offset)
            .unwrap_or(text.len());
        let line = &text[line_start..line_end];

        if is_paragraph_separator_line(line) {
            separator_start.get_or_insert_with(|| line_start.saturating_sub(1));
        } else if let Some(boundary) = separator_start.take() {
            boundaries.push((boundary, line_start));
        }

        if line_end == text.len() {
            break;
        }
        line_start = line_end + 1;
    }

    if let Some(boundary) = separator_start {
        boundaries.push((boundary, text.len()));
    }

    boundaries
}

fn fill_line_bounds(
    lines: &[String],
    cursor: Position,
    region: Option<TextRange>,
) -> Option<(usize, usize)> {
    if lines.is_empty() {
        return None;
    }

    if let Some(region) = region {
        let mut start = region.start.line.min(lines.len() - 1);
        let mut end = region.end.line.min(lines.len() - 1);
        if region.end.byte == 0 && end > start {
            end -= 1;
        }
        while start <= end && is_paragraph_separator_line(&lines[start]) {
            start += 1;
        }
        while end >= start && is_paragraph_separator_line(&lines[end]) {
            if end == 0 {
                break;
            }
            end -= 1;
        }
        if start > end {
            return None;
        }
        while start > 0 && !is_paragraph_separator_line(&lines[start - 1]) {
            start -= 1;
        }
        while end + 1 < lines.len() && !is_paragraph_separator_line(&lines[end + 1]) {
            end += 1;
        }
        return (start <= end).then_some((start, end));
    }

    let mut line = cursor.line.min(lines.len() - 1);
    if is_paragraph_separator_line(&lines[line]) {
        while line < lines.len() && is_paragraph_separator_line(&lines[line]) {
            line += 1;
        }
        if line == lines.len() {
            return None;
        }
    }

    let mut start = line;
    while start > 0 && !is_paragraph_separator_line(&lines[start - 1]) {
        start -= 1;
    }
    let mut end = line;
    while end + 1 < lines.len() && !is_paragraph_separator_line(&lines[end + 1]) {
        end += 1;
    }
    Some((start, end))
}

fn paragraph_runs_in_line_bounds(
    lines: &[String],
    first_line: usize,
    last_line: usize,
) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut line = first_line;
    while line <= last_line && line < lines.len() {
        while line <= last_line && line < lines.len() && is_paragraph_separator_line(&lines[line]) {
            line += 1;
        }
        if line > last_line || line >= lines.len() {
            break;
        }
        let start = line;
        while line <= last_line && line < lines.len() && !is_paragraph_separator_line(&lines[line])
        {
            line += 1;
        }
        runs.push((start, line - 1));
    }
    runs
}

fn fill_plain_text_lines(lines: &[String], fill_column: usize) -> Vec<String> {
    let words = lines
        .iter()
        .flat_map(|line| line.split_whitespace())
        .collect::<Vec<_>>();
    if words.is_empty() {
        return vec![String::new()];
    }

    let mut filled = Vec::new();
    let mut current = String::new();
    for word in words {
        let next_len = if current.is_empty() {
            UnicodeWidthStr::width(word)
        } else {
            UnicodeWidthStr::width(current.as_str()) + 1 + UnicodeWidthStr::width(word)
        };
        if next_len > fill_column && !current.is_empty() {
            filled.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        filled.push(current);
    }
    filled
}

fn filled_cursor_position(
    lines: &[String],
    runs: &[(usize, usize)],
    cursor: Position,
    fill_column: usize,
) -> Option<Position> {
    let mut line_delta = 0isize;
    for &(run_start, run_end) in runs {
        if cursor.line < run_start {
            if lines
                .get(cursor.line)
                .is_some_and(|line| is_paragraph_separator_line(line))
            {
                return Some(Position::new(run_start.checked_add_signed(line_delta)?, 0));
            }
            return Some(Position::new(
                cursor.line.checked_add_signed(line_delta)?,
                cursor.byte,
            ));
        }
        let original_len = run_end - run_start + 1;
        let filled = fill_plain_text_lines(&lines[run_start..=run_end], fill_column);
        let output_start = run_start.checked_add_signed(line_delta)?;
        if (run_start..=run_end).contains(&cursor.line) {
            return cursor_in_filled_run(
                &lines[run_start..=run_end],
                Position::new(cursor.line - run_start, cursor.byte),
                &filled,
                output_start,
            );
        }
        line_delta += filled.len() as isize - original_len as isize;
    }
    Some(Position::new(
        cursor.line.checked_add_signed(line_delta)?,
        cursor.byte,
    ))
}

fn cursor_in_filled_run(
    original: &[String],
    cursor: Position,
    filled: &[String],
    output_start_line: usize,
) -> Option<Position> {
    let original_text = original.join("\n");
    let cursor_absolute = position_to_absolute_in_lines(original, cursor)?;
    let anchor = word_anchor_for_absolute(&original_text, cursor_absolute);
    let filled_text = filled.join("\n");
    let filled_absolute = absolute_for_word_anchor(&filled_text, anchor);
    let relative = absolute_to_position(&filled_text, filled_absolute);
    Some(Position::new(
        output_start_line + relative.line,
        relative.byte,
    ))
}

fn position_to_absolute_in_lines(lines: &[String], position: Position) -> Option<usize> {
    let line = lines.get(position.line)?;
    if position.byte > line.len() || !line.is_char_boundary(position.byte) {
        return None;
    }
    let prefix = lines
        .iter()
        .take(position.line)
        .map(|line| line.len() + 1)
        .sum::<usize>();
    Some(prefix + position.byte)
}

fn word_anchor_for_absolute(text: &str, absolute: usize) -> (usize, usize) {
    let words = word_spans(text);
    for (index, word) in words.iter().enumerate() {
        if absolute < word.start {
            return (index, 0);
        }
        if absolute <= word.end {
            return (index, absolute - word.start);
        }
    }
    (words.len(), 0)
}

fn absolute_for_word_anchor(text: &str, anchor: (usize, usize)) -> usize {
    let words = word_spans(text);
    let (index, offset) = anchor;
    if index >= words.len() {
        return text.len();
    }
    let word = words[index];
    (word.start + offset).min(word.end)
}

fn word_spans(text: &str) -> Vec<WordSpan> {
    let mut words = Vec::new();
    let mut start = None;
    for (byte, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(word_start) = start.take() {
                words.push(WordSpan {
                    start: word_start,
                    end: byte,
                });
            }
        } else if start.is_none() {
            start = Some(byte);
        }
    }
    if let Some(word_start) = start {
        words.push(WordSpan {
            start: word_start,
            end: text.len(),
        });
    }
    words
}

fn clamp_position_to_lines(lines: &[String], position: Position) -> Position {
    let line = position.line.min(lines.len().saturating_sub(1));
    let byte = lines
        .get(line)
        .map(|line_text| position.byte.min(line_text.len()))
        .unwrap_or(0);
    Position::new(line, byte)
}

fn region_line_bounds(range: TextRange) -> Option<(usize, usize)> {
    let start = range.start.line;
    let mut end = range.end.line;
    if range.end.byte == 0 && end > start {
        end -= 1;
    }
    (start <= end).then_some((start, end))
}

fn line_comment_indent(line: &str) -> usize {
    line.char_indices()
        .find_map(|(byte, character)| (!matches!(character, ' ' | '\t')).then_some(byte))
        .unwrap_or(line.len())
}

fn line_is_commented(line: &str, syntax: CommentSyntax) -> bool {
    let indent = line_comment_indent(line);
    line[indent..].starts_with(syntax.line_start)
}

fn comment_line(line: &mut String, syntax: CommentSyntax) -> Option<(usize, isize)> {
    if line.trim().is_empty() {
        return None;
    }
    let indent = line_comment_indent(line);
    let insertion = format!("{} ", syntax.line_start);
    line.insert_str(indent, &insertion);
    Some((indent, insertion.len() as isize))
}

fn uncomment_line(line: &mut String, syntax: CommentSyntax) -> Option<(usize, isize)> {
    if line.trim().is_empty() {
        return None;
    }
    let indent = line_comment_indent(line);
    if !line[indent..].starts_with(syntax.line_start) {
        return None;
    }
    let mut end = indent + syntax.line_start.len();
    if line[end..].starts_with(' ') {
        end += 1;
    }
    line.replace_range(indent..end, "");
    Some((indent, -((end - indent) as isize)))
}

fn adjust_position_after_line_delta(
    position: Position,
    line_index: usize,
    edit_byte: usize,
    delta: isize,
) -> Position {
    if delta == 0 || position.line != line_index || position.byte < edit_byte {
        return position;
    }
    let indent_adjusted = position.byte.saturating_add_signed(delta);
    Position::new(position.line, indent_adjusted)
}

fn position_to_absolute(buffer: &Buffer, position: Position) -> Result<usize> {
    buffer.validate_position(position)?;
    let prefix_len = buffer
        .lines()
        .iter()
        .take(position.line)
        .map(|line| line.len() + 1)
        .sum::<usize>();
    Ok(prefix_len + position.byte)
}

fn clamp_position_to_buffer(buffer: &Buffer, position: Position) -> Position {
    let line = position.line.min(buffer.line_count().saturating_sub(1));
    let byte = buffer
        .line(line)
        .map(|text| position.byte.min(text.len()))
        .unwrap_or(0);
    Position::new(line, byte)
}

fn absolute_to_position(text: &str, absolute: usize) -> Position {
    let absolute = absolute.min(text.len());
    let mut line_start = 0;
    let mut line = 0;
    for (byte, character) in text.char_indices() {
        if byte >= absolute {
            break;
        }
        if character == '\n' {
            line += 1;
            line_start = byte + 1;
        }
    }
    Position::new(line, absolute - line_start)
}

fn transpose_words_once(text: &str, cursor: usize, argument: i32) -> Option<(String, usize)> {
    if argument > 0 {
        let source = word_at_or_before(text, cursor)?;
        let target = next_word_after(text, source.end)?;
        Some(transpose_word_spans(text, source, target))
    } else {
        let source = word_at_or_before(text, cursor)?;
        let target = previous_word_before(text, source.start)?;
        let (replacement, _) = transpose_word_spans(text, target, source);
        Some((replacement, target.start + (source.end - source.start)))
    }
}

fn transpose_word_spans(text: &str, first: WordSpan, second: WordSpan) -> (String, usize) {
    debug_assert!(first.end <= second.start);
    let first_text = &text[first.start..first.end];
    let middle = &text[first.end..second.start];
    let second_text = &text[second.start..second.end];

    let mut replacement = String::with_capacity(text.len());
    replacement.push_str(&text[..first.start]);
    replacement.push_str(second_text);
    replacement.push_str(middle);
    replacement.push_str(first_text);
    replacement.push_str(&text[second.end..]);

    (
        replacement,
        first.start + second_text.len() + middle.len() + first_text.len(),
    )
}

fn word_at_or_before(text: &str, cursor: usize) -> Option<WordSpan> {
    if cursor < text.len() && text.is_char_boundary(cursor) {
        let character = text[cursor..].chars().next()?;
        if is_word_character(character) {
            return word_around(text, cursor);
        }
    }

    previous_word_before(text, cursor)
}

fn word_around(text: &str, byte: usize) -> Option<WordSpan> {
    let mut start = byte;
    for (offset, character) in text[..byte].char_indices().rev() {
        if !is_word_character(character) {
            break;
        }
        start = offset;
    }

    let mut end = byte;
    for (offset, character) in text[byte..].char_indices() {
        if !is_word_character(character) {
            break;
        }
        end = byte + offset + character.len_utf8();
    }

    (start < end).then_some(WordSpan { start, end })
}

fn next_word_after(text: &str, byte: usize) -> Option<WordSpan> {
    let mut start = None;
    for (offset, character) in text[byte..].char_indices() {
        let absolute = byte + offset;
        if is_word_character(character) {
            start = Some(absolute);
            break;
        }
    }
    let start = start?;
    word_around(text, start)
}

fn previous_word_before(text: &str, byte: usize) -> Option<WordSpan> {
    let mut end = None;
    for (offset, character) in text[..byte].char_indices().rev() {
        if is_word_character(character) {
            end = Some(offset + character.len_utf8());
            break;
        }
    }
    let end = end?;

    let mut start = end;
    for (offset, character) in text[..end].char_indices().rev() {
        if !is_word_character(character) {
            break;
        }
        start = offset;
    }

    Some(WordSpan { start, end })
}

fn transpose_lines_edit(
    buffer: &Buffer,
    position: Position,
    argument: i32,
) -> Option<(Vec<String>, Position)> {
    let lines = buffer.lines();
    let effective_len =
        if buffer.final_newline() && lines.last().is_some_and(|line| line.is_empty()) {
            lines.len().saturating_sub(1)
        } else {
            lines.len()
        };
    if effective_len < 2 || position.line >= lines.len() {
        return None;
    }

    let current_line = position.line.min(effective_len - 1);
    if current_line == 0 {
        return None;
    }

    let source = current_line - 1;
    let mut replacement = lines[..effective_len].to_vec();
    if argument > 0 {
        let target = source
            .checked_add(argument as usize)?
            .min(effective_len - 1);
        if target == source {
            return None;
        }
        let moved = replacement.remove(source);
        replacement.insert(target, moved);
        if effective_len < lines.len() {
            replacement.extend_from_slice(&lines[effective_len..]);
        }
        let cursor_byte = replacement[target].len();
        Some((replacement, Position::new(target, cursor_byte)))
    } else {
        let distance = argument.unsigned_abs() as usize;
        let target = source.checked_sub(distance)?;
        let moved = replacement.remove(source);
        replacement.insert(target, moved);
        if effective_len < lines.len() {
            replacement.extend_from_slice(&lines[effective_len..]);
        }
        let cursor_byte = replacement[target].len();
        Some((replacement, Position::new(target, cursor_byte)))
    }
}

fn transpose_chars_edit(line: &str, position: Position, argument: i32) -> Option<TransposeEdit> {
    if !line.is_char_boundary(position.byte) {
        return None;
    }

    let graphemes = line.grapheme_indices(true).collect::<Vec<_>>();
    if graphemes.len() < 2 {
        return None;
    }

    let cursor_index = graphemes
        .iter()
        .take_while(|(byte, _)| *byte < position.byte)
        .count();
    if cursor_index == 0 {
        return None;
    }

    if argument > 0 {
        if cursor_index == graphemes.len() {
            if argument == 1 {
                return transpose_grapheme_range(line, position.line, graphemes.len() - 2, 1, 1);
            }
            return None;
        }

        let source = cursor_index - 1;
        let distance = (argument as usize).min(graphemes.len() - 1 - source);
        if distance == 0 {
            return None;
        }
        transpose_grapheme_range(line, position.line, source, distance, 1)
    } else {
        let source = cursor_index - 1;
        let distance = argument.unsigned_abs() as usize;
        let target = source.saturating_sub(distance);
        if target == source {
            return None;
        }
        transpose_grapheme_range(line, position.line, target, source - target, -1)
    }
}

fn transpose_grapheme_range(
    line: &str,
    line_index: usize,
    range_start_index: usize,
    distance: usize,
    direction: i32,
) -> Option<TransposeEdit> {
    let graphemes = line.grapheme_indices(true).collect::<Vec<_>>();
    let source = if direction >= 0 {
        range_start_index
    } else {
        range_start_index + distance
    };
    let range_end_index = range_start_index + distance + 1;
    let range_start = graphemes.get(range_start_index)?.0;
    let range_end = if range_end_index < graphemes.len() {
        graphemes[range_end_index].0
    } else {
        line.len()
    };
    let source_start = graphemes.get(source)?.0;
    let source_end = if source + 1 < graphemes.len() {
        graphemes[source + 1].0
    } else {
        line.len()
    };
    let dragged = &line[source_start..source_end];

    let mut replacement = String::new();
    let cursor_byte = if direction >= 0 {
        replacement.push_str(&line[range_start..source_start]);
        replacement.push_str(&line[source_end..range_end]);
        replacement.push_str(dragged);
        range_start + replacement.len()
    } else {
        replacement.push_str(dragged);
        let cursor_byte = range_start + replacement.len();
        replacement.push_str(&line[range_start..source_start]);
        replacement.push_str(&line[source_end..range_end]);
        cursor_byte
    };

    Some(TransposeEdit {
        range: TextRange::new(
            Position::new(line_index, range_start),
            Position::new(line_index, range_end),
        ),
        replacement,
        cursor_after: Position::new(line_index, cursor_byte),
    })
}

fn blank_line_run(lines: &[String], line_index: usize) -> (usize, usize) {
    let mut start = line_index;
    while start > 0 && is_blank_line(&lines[start - 1]) {
        start -= 1;
    }

    let mut end = line_index;
    while end + 1 < lines.len() && is_blank_line(&lines[end + 1]) {
        end += 1;
    }

    (start, end)
}

fn is_yank_command(command: Command) -> bool {
    matches!(command, Command::Yank | Command::YankPop)
}

fn editor_outcome_for_command_outcome(outcome: CommandOutcome) -> EditorOutcome {
    match outcome {
        CommandOutcome::Continue | CommandOutcome::StartedPrompt => EditorOutcome::Continue,
        CommandOutcome::Exit => EditorOutcome::Quit,
        CommandOutcome::Suspend => EditorOutcome::Suspend,
    }
}

fn command_outcome_for_editor_outcome(outcome: EditorOutcome) -> CommandOutcome {
    match outcome {
        EditorOutcome::Continue => CommandOutcome::Continue,
        EditorOutcome::Quit => CommandOutcome::Exit,
        EditorOutcome::Suspend => CommandOutcome::Suspend,
    }
}

fn is_keyboard_macro_control_command(command: Command) -> bool {
    matches!(
        command,
        Command::CallLastKeyboardMacro | Command::EndKeyboardMacro | Command::StartKeyboardMacro
    )
}

fn positive_argument_count(argument: Option<i32>) -> usize {
    argument.unwrap_or(1).max(0) as usize
}

fn case_transform_text(text: &str, transform: CaseTransform) -> String {
    match transform {
        CaseTransform::Lower => text.chars().flat_map(char::to_lowercase).collect(),
        CaseTransform::Upper => text.chars().flat_map(char::to_uppercase).collect(),
        CaseTransform::Capitalize => capitalize_words(text),
    }
}

fn capitalize_words(text: &str) -> String {
    let mut output = String::new();
    let mut cased_in_word = false;

    for character in text.chars() {
        if is_word_character(character) {
            if character.is_alphabetic() {
                if cased_in_word {
                    output.extend(character.to_lowercase());
                } else {
                    output.extend(character.to_uppercase());
                    cased_in_word = true;
                }
            } else {
                output.push(character);
            }
        } else {
            cased_in_word = false;
            output.push(character);
        }
    }

    output
}

fn register_key_from_event(key: &KeyEvent) -> Option<char> {
    match key {
        KeyEvent::Text(text) if text.chars().count() == 1 => text.chars().next(),
        _ => None,
    }
}

fn format_key_prefix_message(sequence: &[KeyEvent]) -> String {
    format!("{}- (C-h for help)", format_key_sequence(sequence))
}

fn is_key_prefix_help(key: &KeyEvent) -> bool {
    matches!(
        key,
        KeyEvent::Ctrl('h') | KeyEvent::Special(SpecialKey::Backspace)
    )
}

fn document_kind_label(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Normal => "normal",
        DocumentKind::Welcome => "welcome",
        DocumentKind::Help => "help",
        DocumentKind::Messages => "messages",
        DocumentKind::Completions => "completions",
        DocumentKind::BufferList => "buffer-list",
        DocumentKind::ShellOutput => "shell-output",
    }
}

fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

impl DecorationProvider for Editor {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
        self.spans_for_buffer_line(self.current_buffer, line_index, line)
    }
}

struct SyntaxDecorator {
    enabled: bool,
    mode: SyntaxMode,
}

impl DecorationProvider for SyntaxDecorator {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
        if !self.enabled || self.mode == SyntaxMode::PlainText {
            return Vec::new();
        }
        SyntaxHighlighter::new(self.mode).highlight_line(line_index, line)
    }
}

struct RegionDecorator {
    range: Option<TextRange>,
    rectangle: Option<RectangleBounds>,
}

impl DecorationProvider for RegionDecorator {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
        if let Some(rectangle) = self.rectangle {
            if line_index < rectangle.start_line || line_index > rectangle.end_line {
                return Vec::new();
            }
            let start = byte_for_display_column_in_line(line, rectangle.start_column);
            let end = byte_for_display_column_in_line(line, rectangle.end_column);
            if start == end {
                return Vec::new();
            }
            return vec![Span::new(start, end, Face::Region)];
        }

        let Some(range) = self.range else {
            return Vec::new();
        };
        if line_index < range.start.line || line_index > range.end.line {
            return Vec::new();
        }
        let start = if line_index == range.start.line {
            range.start.byte
        } else {
            0
        };
        let end = if line_index == range.end.line {
            range.end.byte
        } else {
            line.len()
        };
        if start == end {
            return Vec::new();
        }
        vec![Span::new(start, end, Face::Region)]
    }
}

fn byte_for_display_column_in_line(line: &str, target_column: usize) -> usize {
    let mut column = 0;
    for (byte, character) in line.char_indices() {
        let width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if column + width > target_column {
            return byte;
        }
        column += width;
    }
    line.len()
}

struct QueryReplaceDecorator {
    enabled: bool,
    current: Option<TextRange>,
}

impl DecorationProvider for QueryReplaceDecorator {
    fn spans_for_line(&self, line_index: usize, _line: &str) -> Vec<Span> {
        if !self.enabled {
            return Vec::new();
        }
        let Some(range) = self.current else {
            return Vec::new();
        };
        if range.start.line != line_index || range.end.line != line_index {
            return Vec::new();
        }
        vec![Span::new(
            range.start.byte,
            range.end.byte,
            Face::CurrentSearchMatch,
        )]
    }
}

struct SearchDecorator<'a> {
    enabled: bool,
    search: Option<&'a SearchState>,
    query: Option<&'a str>,
}

impl DecorationProvider for SearchDecorator<'_> {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
        if !self.enabled {
            return Vec::new();
        }
        let Some(search) = self.search else {
            return Vec::new();
        };
        let Some(query) = self.query else {
            return Vec::new();
        };
        if query.is_empty() {
            return Vec::new();
        }

        line.match_indices(query)
            .map(|(start, match_text)| {
                let end = start + match_text.len();
                let face = if search.current
                    == Some(TextRange::new(
                        Position::new(line_index, start),
                        Position::new(line_index, end),
                    )) {
                    Face::CurrentSearchMatch
                } else {
                    Face::SearchMatch
                };
                Span::new(start, end, face)
            })
            .collect()
    }
}

fn remember_returnable_special_buffer_return(
    return_viewport: &mut Option<Viewport>,
    current_viewport: Viewport,
    current_is_special: bool,
) {
    if !current_is_special || return_viewport.is_none() {
        *return_viewport = Some(current_viewport);
    }
}

fn format_completion_buffer(completion: &CompletionSession) -> String {
    let title = format!("Possible Completions for {}:", completion.title());
    let mut text = format!("{title}\n\n");
    let items = completion.view_items();
    if items.is_empty() {
        text.push_str("No match\n");
        return text;
    }
    for item in items {
        let marker = if item.selected { ">" } else { " " };
        if completion.show_annotations() && !item.candidate.annotation.is_empty() {
            text.push_str(&format!(
                "{marker} {:<32} {}\n",
                item.candidate.value, item.candidate.annotation
            ));
        } else {
            text.push_str(&format!("{marker} {}\n", item.candidate.value));
        }
    }
    text
}

fn default_rectangle_number_format(bounds: RectangleBounds, start_at: i32) -> String {
    let lines = (bounds.end_line - bounds.start_line + 1) as i32;
    let end = start_at.saturating_add(lines.saturating_sub(1));
    let width = start_at.to_string().len().max(end.to_string().len());
    format!("%{width}d ")
}

fn format_rectangle_number(format: &str, number: i32) -> Result<String> {
    let mut output = String::new();
    let mut chars = format.chars().peekable();
    let mut formatted_number = false;

    while let Some(character) = chars.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        if matches!(chars.peek(), Some('%')) {
            chars.next();
            output.push('%');
            continue;
        }
        if formatted_number {
            return Err(RileError::InvalidInput(
                "format string must contain one %d directive".to_owned(),
            ));
        }

        let zero_pad = if matches!(chars.peek(), Some('0')) {
            chars.next();
            true
        } else {
            false
        };
        let mut width = String::new();
        while let Some(character) = chars.peek().copied() {
            if !character.is_ascii_digit() {
                break;
            }
            width.push(character);
            chars.next();
        }
        if chars.next() != Some('d') {
            return Err(RileError::InvalidInput(
                "format string must contain one %d directive".to_owned(),
            ));
        }

        let width = width.parse::<usize>().unwrap_or(0);
        let number_text = number.to_string();
        let sign_width = usize::from(number < 0);
        if width > number_text.len() {
            let padding = width - number_text.len();
            if zero_pad && number < 0 {
                output.push('-');
                output.extend(std::iter::repeat_n('0', padding));
                output.push_str(&number_text[sign_width..]);
            } else {
                output.extend(std::iter::repeat_n(
                    if zero_pad { '0' } else { ' ' },
                    padding,
                ));
                output.push_str(&number_text);
            }
        } else {
            output.push_str(&number_text);
        }
        formatted_number = true;
    }

    if !formatted_number {
        return Err(RileError::InvalidInput(
            "format string must contain one %d directive".to_owned(),
        ));
    }
    Ok(output)
}

fn prompt_label(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::DescribeFunction => "Describe function: ",
        PromptKind::DescribeVariable => "Describe variable: ",
        PromptKind::ExtendedCommand => "M-x ",
        PromptKind::FindFile => "Find file: ",
        PromptKind::FindFileReadOnly => "Find file read-only: ",
        PromptKind::GotoLine => "Goto line: ",
        PromptKind::InsertFile => "Insert file: ",
        PromptKind::IncrementalSearch => "I-search: ",
        PromptKind::KillBuffer => "Kill buffer: ",
        PromptKind::KillDirtyBuffer => "Kill buffer anyway? ",
        PromptKind::QueryReplaceReplacement => "Query replace with: ",
        PromptKind::QueryReplaceSearch => "Query replace: ",
        PromptKind::RevertBuffer => "Buffer modified; revert anyway? (yes or no) ",
        PromptKind::SaveSomeBuffers => "Save file? (yes or no) ",
        PromptKind::QuitDirtyBuffers => "Modified buffers exist; exit anyway? (yes or no) ",
        PromptKind::RectangleNumberFormat => "Format string: ",
        PromptKind::RectangleNumberStart => "Number to count from: ",
        PromptKind::ShellCommand => "Shell command: ",
        PromptKind::StringRectangle => "String rectangle: ",
        PromptKind::SwitchToBuffer => "Switch to buffer: ",
        PromptKind::WriteFile => "Write file: ",
    }
}

fn file_prompt_base_input(base_dir: &Path) -> String {
    let mut input = base_dir.display().to_string();
    if !input.ends_with(MAIN_SEPARATOR) {
        input.push(MAIN_SEPARATOR);
    }
    input
}

fn dirty_kill_prompt(name: &str) -> String {
    format!("Buffer {name} modified; kill anyway? (y or n) ")
}

fn format_shell_command_output(command: &str, output: &ShellCommandOutput) -> String {
    let mut text = String::new();
    if !command.is_empty() {
        text.push_str("Command: ");
        text.push_str(command);
        text.push_str("\n\n");
    }

    if output.stdout.is_empty() {
        text.push_str("(No stdout)\n");
    } else {
        text.push_str(&output.stdout);
        if !output.stdout.ends_with('\n') {
            text.push('\n');
        }
    }

    if !output.stderr.is_empty() {
        text.push_str("\nstderr:\n");
        text.push_str(&output.stderr);
        if !output.stderr.ends_with('\n') {
            text.push('\n');
        }
    }

    if !output.success() {
        text.push_str("\nExit status: ");
        text.push_str(&format_shell_status(output.status_code));
        text.push('\n');
    }

    text
}

fn format_shell_status(status_code: Option<i32>) -> String {
    status_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_owned())
}

fn format_shell_success_message(output: &ShellCommandOutput) -> String {
    if output.stderr.is_empty() {
        format!("Shell command completed ({} bytes)", output.stdout.len())
    } else {
        format!(
            "Shell command completed ({} bytes stdout, {} bytes stderr)",
            output.stdout.len(),
            output.stderr.len()
        )
    }
}

fn format_shell_mutation_message(action: &str, stdout_bytes: usize, stderr_bytes: usize) -> String {
    if stderr_bytes == 0 {
        format!("{action} {stdout_bytes} bytes from shell command")
    } else {
        format!("{action} {stdout_bytes} bytes from shell command ({stderr_bytes} bytes stderr)")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::help::{
        HELP_FILL_WIDTH, append_wrapped_prose, format_about_rile_help,
        format_describe_bindings_help, format_describe_buffer_help, format_describe_key_help,
        format_describe_mode_help, format_describe_variable_help,
    };
    use super::{
        AboutRileInfo, ActiveModes, BufferDescription, Editor, EditorOutcome, KillEntry,
        file_prompt_base_input, format_rectangle_number,
    };
    use crate::buffer::{BufferId, Position, TextRange};
    use crate::command::{Command, CommandRegistry};
    use crate::completion::{
        CompletionConfig, CompletionMatching, CompletionSession, CompletionStyle,
    };
    use crate::config::{Config, ThemeName};
    use crate::file::Document;
    use crate::input::{KeyEvent, SpecialKey};
    use crate::keymap::{KeyBinding, KeyMap, KeyMapId, KeyMapStack};
    use crate::minibuffer::PromptKind;
    use crate::mode::{ModeId, ModeRegistry};
    use crate::option::{OptionId, OptionRegistry, OptionValue};
    use crate::render::{DecorationProvider, Face, Span};
    use crate::syntax::{MajorMode, SyntaxMode};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    #[test]
    fn help_prose_wraps_to_fill_width() {
        let mut help = String::new();

        append_wrapped_prose(
            &mut help,
            "This paragraph is intentionally long enough to require wrapping at the help fill width while preserving word boundaries for readable terminal help output.",
        );

        assert!(help.lines().count() > 1);
        for line in help.lines() {
            assert!(
                line.len() <= HELP_FILL_WIDTH,
                "line should be wrapped: {line:?}"
            );
        }
        let normalized = help.replace('\n', " ");
        assert!(normalized.contains("word boundaries for readable terminal help output."));
    }

    #[test]
    fn help_prose_preserves_preformatted_blocks() {
        let mut help = String::new();

        append_wrapped_prose(
            &mut help,
            "Intro paragraph.\n\nKey             Binding\n---             -------\nC-x C-f         find-file\n\nTrailing paragraph.",
        );

        assert!(help.contains(
            "Key             Binding\n---             -------\nC-x C-f         find-file"
        ));
        assert!(help.ends_with("Trailing paragraph.\n"));
    }

    #[test]
    fn about_rile_opens_runtime_help() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("about-rile should open help");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("About Rile:"));
        assert!(help.contains(concat!("Version: ", env!("CARGO_PKG_VERSION"))));
        assert!(help.contains("Build profile:"));
        assert!(help.contains("Terminal backend: ANSI terminal"));
        assert!(help.contains("Config path:"));
        assert!(help.contains("Current directory:"));
        assert!(help.contains("reviewed with C-h e"));
    }

    #[test]
    fn about_rile_help_formats_stable_fields() {
        let info = AboutRileInfo {
            version: "test-version",
            build_profile: "test-profile",
            enabled_features: "not reported by this build",
            terminal_backend: "test-terminal",
            config_path: Some("/tmp/rile/config.toml".to_owned()),
            current_directory: Some("/tmp/rile".to_owned()),
        };

        let help = format_about_rile_help(&info);

        assert!(help.contains("Version: test-version"));
        assert!(help.contains("Build profile: test-profile"));
        assert!(help.contains("Enabled features: not reported by this build"));
        assert!(help.contains("Terminal backend: test-terminal"));
        assert!(help.contains("Config path: /tmp/rile/config.toml"));
        assert!(help.contains("Current directory: /tmp/rile"));
    }

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rile-editor-test-{}-{counter}", std::process::id()));
            fs::create_dir(&path).expect("test directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn current_dir_prompt_input() -> String {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        file_prompt_base_input(&current_dir)
    }

    fn send_c_x_r(editor: &mut Editor, key: KeyEvent) {
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x prefix should start");
        editor
            .handle_key(KeyEvent::Text("r".to_owned()))
            .expect("C-x r prefix should start");
        editor.handle_key(key).expect("C-x r command should run");
    }

    fn editor_with_small_completion_page(document: Document) -> Editor {
        Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    max_candidates: 2,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        )
    }

    fn selected_completion_value(editor: &Editor) -> Option<&str> {
        editor
            .completion()
            .and_then(|completion| completion.selected())
            .map(|candidate| candidate.value.as_str())
    }

    fn assert_completion_movement_keys(
        editor: &mut Editor,
        first: &str,
        next: &str,
        page_forward: &str,
    ) {
        assert_eq!(selected_completion_value(editor), Some(first));
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("C-n should select next candidate");
        assert_eq!(selected_completion_value(editor), Some(next));
        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("C-p should select previous candidate");
        assert_eq!(selected_completion_value(editor), Some(first));
        editor
            .handle_key(KeyEvent::Ctrl('v'))
            .expect("C-v should page forward");
        assert_eq!(selected_completion_value(editor), Some(page_forward));
        editor
            .handle_key(KeyEvent::Meta('v'))
            .expect("M-v should page backward");
        assert_eq!(selected_completion_value(editor), Some(first));
    }

    fn start_test_completion_prompt(
        editor: &mut Editor,
        kind: PromptKind,
        completion: CompletionSession,
        input: &str,
    ) {
        editor
            .minibuffer
            .start_prompt(kind, super::prompt_label(kind));
        editor.completion = Some(completion);
        editor.minibuffer.set_prompt_input(input);
        editor.update_completion_from_prompt();
    }

    fn assert_completion_prompt_edit_refreshes_matches(
        initial_input: &str,
        setup_keys: &[KeyEvent],
        edit_key: KeyEvent,
        expected_input: &str,
    ) {
        let mut editor = Editor::new(Document::scratch());
        let completion = CompletionSession::commands(
            &CommandRegistry::default(),
            &KeyMap::default(),
            editor.completion_config,
        );
        start_test_completion_prompt(
            &mut editor,
            PromptKind::ExtendedCommand,
            completion,
            initial_input,
        );
        assert_eq!(
            editor.completion().map(CompletionSession::match_count),
            Some(0)
        );

        for key in setup_keys {
            editor
                .handle_key(key.clone())
                .expect("setup key should be handled");
        }
        editor
            .handle_key(edit_key)
            .expect("prompt edit should refresh completion");

        assert_eq!(editor.minibuffer().prompt_input(), Some(expected_input));
        assert_eq!(
            editor.completion().map(CompletionSession::match_count),
            Some(1)
        );
    }

    fn assert_incremental_search_prompt_edit_updates_live_search(
        initial_input: &str,
        setup_keys: &[KeyEvent],
        edit_key: KeyEvent,
        expected_input: &str,
    ) {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), " alpha beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        editor
            .handle_key(KeyEvent::Text(initial_input.to_owned()))
            .expect("search input should update");
        let failing_message = format!("Failing I-search: {initial_input}");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some(failing_message.as_str())
        );

        for key in setup_keys {
            editor
                .handle_key(key.clone())
                .expect("setup key should be handled");
        }
        editor
            .handle_key(edit_key)
            .expect("prompt edit should update search");

        assert_eq!(editor.minibuffer().prompt_input(), Some(expected_input));
        assert_eq!(editor.cursor(), Position::new(0, 0));
        let found_message = format!("I-search: {expected_input}");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some(found_message.as_str())
        );
    }

    fn mark_columns_one_to_three_across_two_lines(editor: &mut Editor) {
        editor.cursor = Position::new(0, 0);
        editor.deactivate_region();
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
    }

    #[test]
    fn inserts_printable_text_enter_and_tab() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("é".to_owned()))
            .expect("text should insert");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should insert newline");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should insert tab");

        assert_eq!(editor.document().buffer().serialize(), "é\n\t");
        assert_eq!(editor.cursor(), Position::new(1, 1));
    }

    #[test]
    fn moves_and_deletes_text_with_bindings() {
        let mut editor = Editor::new(Document::scratch());
        for text in ["a", "b", "c"] {
            editor
                .handle_key(KeyEvent::Text(text.to_owned()))
                .expect("text should insert");
        }

        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("cursor should move backward");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("backspace should delete");
        editor
            .handle_key(KeyEvent::Ctrl('d'))
            .expect("delete should delete at cursor");

        assert_eq!(editor.document().buffer().serialize(), "a");
        assert_eq!(editor.cursor(), Position::new(0, 1));
    }

    #[test]
    fn moves_by_words_with_meta_bindings() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('f'))
            .expect("M-f should move forward by word");
        assert_eq!(editor.cursor(), Position::new(0, "one".len()));

        editor
            .handle_key(KeyEvent::Meta('f'))
            .expect("M-f should move forward by next word");
        assert_eq!(editor.cursor(), Position::new(0, "one two".len()));

        editor
            .handle_key(KeyEvent::Meta('b'))
            .expect("M-b should move backward by word");
        assert_eq!(editor.cursor(), Position::new(0, "one ".len()));

        editor
            .handle_key(KeyEvent::Meta('b'))
            .expect("M-b should move backward by previous word");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn moves_by_paragraphs_with_meta_bindings() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(
                Position::new(0, 0),
                "one\ntwo\n\nthree\nfour\n \t\x0c\nfive",
            )
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} should move to paragraph boundary");
        assert_eq!(editor.cursor(), Position::new(2, 0));

        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} should skip separators and move to next boundary");
        assert_eq!(editor.cursor(), Position::new(5, 0));

        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} should move to end of final paragraph");
        assert_eq!(editor.cursor(), Position::new(6, "five".len()));

        editor.cursor = Position::new(5, 0);
        editor
            .handle_key(KeyEvent::Meta('{'))
            .expect("M-{ should move to previous paragraph start");
        assert_eq!(editor.cursor(), Position::new(3, 0));

        editor
            .handle_key(KeyEvent::Meta('{'))
            .expect("M-{ at paragraph start should move to previous paragraph");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn paragraph_movement_supports_prefix_arguments() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\n\ntwo\n\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("argument digit should be accepted");
        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} should repeat by argument");
        assert_eq!(editor.cursor(), Position::new(3, 0));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative argument should be accepted");
        editor
            .handle_key(KeyEvent::Text("1".to_owned()))
            .expect("argument digit should be accepted");
        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("negative M-} should move backward");
        assert_eq!(editor.cursor(), Position::new(2, 0));
    }

    #[test]
    fn paragraph_movement_clamps_at_buffer_edges_and_separator_runs() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "  \n\t\x0c\ntext\n\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('{'))
            .expect("M-{ at buffer start should stay at start");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor.cursor = Position::new(0, 1);
        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} should skip leading separators");
        assert_eq!(editor.cursor(), Position::new(3, 0));

        editor.cursor = Position::new(4, 0);
        editor
            .handle_key(KeyEvent::Meta('{'))
            .expect("M-{ should cross trailing separators");
        assert_eq!(editor.cursor(), Position::new(2, 0));

        editor.cursor = editor.document().buffer().end_position();
        editor
            .handle_key(KeyEvent::Meta('}'))
            .expect("M-} at buffer end should stay at end");
        assert_eq!(editor.cursor(), Position::new(4, 0));
    }

    #[test]
    fn sentence_movement_uses_default_boundaries() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "One. Two?  Three!\nFour.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should skip single-space abbreviation-like boundary");
        assert_eq!(editor.cursor(), Position::new(0, "One. Two?".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should move to next sentence end");
        assert_eq!(editor.cursor(), Position::new(0, "One. Two?  Three!".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should accept newline boundary");
        assert_eq!(editor.cursor(), Position::new(1, "Four.".len()));

        editor
            .handle_key(KeyEvent::Meta('a'))
            .expect("M-a should move to sentence start");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn sentence_movement_accepts_spaces_before_newline_boundaries() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "One. \n\"Two.\" \nThree.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should treat space before newline as a boundary");
        assert_eq!(editor.cursor(), Position::new(0, "One.".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should include closing quote before newline boundary");
        assert_eq!(editor.cursor(), Position::new(1, "\"Two.\"".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should accept end of buffer as a boundary");
        assert_eq!(editor.cursor(), Position::new(2, "Three.".len()));
    }

    #[test]
    fn sentence_movement_handles_closers_paragraphs_and_arguments() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "He said \"go.\"  Next.\n\nFinal.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should include closing quote");
        assert_eq!(editor.cursor(), Position::new(0, "He said \"go.\"".len()));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("C-u 2 M-e should stop at paragraph boundary then final sentence");
        assert_eq!(editor.cursor(), Position::new(2, "Final.".len()));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("negative prefix should start");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative sign should be recorded");
        editor
            .handle_key(KeyEvent::Text("1".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("negative M-e should move backward");
        assert_eq!(editor.cursor(), Position::new(2, 0));
    }

    #[test]
    fn sentence_movement_uses_whitespace_only_paragraph_boundaries() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "First.\n \nSecond.\n\t\nThird.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should stop before space-only separator");
        assert_eq!(editor.cursor(), Position::new(0, "First.".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should continue after space-only separator");
        assert_eq!(editor.cursor(), Position::new(2, "Second.".len()));

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should continue after tab-only separator");
        assert_eq!(editor.cursor(), Position::new(4, "Third.".len()));

        editor
            .handle_key(KeyEvent::Meta('a'))
            .expect("M-a should move to current sentence start");
        assert_eq!(editor.cursor(), Position::new(4, 0));

        editor
            .handle_key(KeyEvent::Meta('a'))
            .expect("M-a should cross tab-only separator");
        assert_eq!(editor.cursor(), Position::new(2, 0));
    }

    #[test]
    fn sentence_movement_handles_separator_start_and_punctuation_free_paragraphs() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "No punctuation\n \nNext sentence.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should stop at paragraph boundary without punctuation");
        assert_eq!(editor.cursor(), Position::new(0, "No punctuation".len()));

        editor.cursor = Position::new(1, 0);
        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should move forward from a separator line");
        assert_eq!(editor.cursor(), Position::new(2, "Next sentence.".len()));

        editor.cursor = Position::new(1, 0);
        editor
            .handle_key(KeyEvent::Meta('a'))
            .expect("M-a should move backward from a separator line");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor.cursor = Position::new(2, 0);
        editor
            .handle_key(KeyEvent::Meta('a'))
            .expect("M-a should cross punctuation-free paragraph");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn sentence_movement_works_in_read_only_buffers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "One.  Two.")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");

        editor
            .handle_key(KeyEvent::Meta('e'))
            .expect("M-e should move in read-only buffer");
        assert_eq!(editor.cursor(), Position::new(0, "One.".len()));
    }

    #[test]
    fn fill_paragraph_wraps_current_paragraph_and_undoes() {
        let text = "alpha   beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau\n\nnext untouched";
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), text)
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha   beta gamma".len());

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q should fill paragraph");
        let filled = editor.document().buffer().serialize();
        assert!(filled.contains("\n\nnext untouched"));
        assert!(!filled.contains("alpha   beta"));
        assert!(
            filled
                .lines()
                .take_while(|line| !line.is_empty())
                .all(|line| line.len() <= 70)
        );
        assert_eq!(editor.cursor().line, 0);
        assert!(editor.cursor().byte > 0);

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore unfilled paragraph");
        assert_eq!(editor.document().buffer().serialize(), text);
        assert_eq!(
            editor.cursor(),
            Position::new(0, "alpha   beta gamma".len())
        );
    }

    #[test]
    fn fill_paragraph_uses_configured_fill_column() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha beta gamma delta epsilon")
            .expect("fixture should insert");
        let config = Config {
            fill_column: 20,
            ..Config::default()
        };
        let mut editor = Editor::with_config(document, config);
        editor.cursor = Position::new(0, "alpha beta gamma delta".len());

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q should fill paragraph with configured column");

        assert_eq!(
            editor.document().buffer().serialize(),
            "alpha beta gamma\ndelta epsilon"
        );
        assert_eq!(editor.cursor(), Position::new(1, "delta".len()));
    }

    #[test]
    fn fill_paragraph_fills_active_region_paragraphs() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(
                Position::new(0, 0),
                "one   two\n\nthree   four\n\nfive   six",
            )
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(2, "three   four".len());
        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q should fill region paragraphs");

        assert_eq!(
            editor.document().buffer().serialize(),
            "one two\n\nthree four\n\nfive   six"
        );
        assert_eq!(editor.active_region_range(), None);
    }

    #[test]
    fn fill_paragraph_expands_partial_region_to_whole_paragraph() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(
                Position::new(0, 0),
                "one two\nthree   four\nfive six\n\nnext",
            )
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, "three".len());
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(1, "three   fo".len());

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q should fill whole overlapped paragraph");
        assert_eq!(
            editor.document().buffer().serialize(),
            "one two three four five six\n\nnext"
        );
        assert_eq!(editor.cursor().line, 0);
        assert!(editor.cursor().byte > "one two ".len());
    }

    #[test]
    fn fill_paragraph_region_on_separator_does_not_fill_neighbors() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one   two\n\nthree   four")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(2, 0);

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q on separator-only region should not edit");
        assert_eq!(
            editor.document().buffer().serialize(),
            "one   two\n\nthree   four"
        );
    }

    #[test]
    fn fill_paragraph_maps_region_cursor_on_trailing_separator() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(
                Position::new(0, 0),
                "one two\nthree   four\nfive six\n\nnext",
            )
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(3, 0);

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q should fill paragraph before separator");
        assert_eq!(
            editor.document().buffer().serialize(),
            "one two three four five six\n\nnext"
        );
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn fill_paragraph_on_separator_fills_next_paragraph() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "intro\n\nalpha   beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("M-q on blank line should fill next paragraph");
        assert_eq!(
            editor.document().buffer().serialize(),
            "intro\n\nalpha beta"
        );
        assert_eq!(editor.cursor(), Position::new(2, 0));
    }

    #[test]
    fn fill_paragraph_respects_read_only_buffers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha   beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");

        editor
            .handle_key(KeyEvent::Meta('q'))
            .expect("read-only M-q should not edit");
        assert_eq!(editor.document().buffer().serialize(), "alpha   beta");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn comment_dwim_inserts_current_line_comment_and_undoes() {
        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        fs::write(&path, "    let value = 1;\n").expect("fixture should write");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));

        editor
            .handle_key(KeyEvent::Meta(';'))
            .expect("M-; should insert line comment");
        assert_eq!(
            editor.document().buffer().serialize(),
            "    // let value = 1;\n"
        );
        assert_eq!(editor.cursor(), Position::new(0, "    // ".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore uncommented line");
        assert_eq!(
            editor.document().buffer().serialize(),
            "    let value = 1;\n"
        );
    }

    #[test]
    fn comment_dwim_toggles_active_region_line_comments() {
        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        fs::write(&path, "one\n  two\n\nthree\n").expect("fixture should write");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));

        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(2, 0);
        editor
            .handle_key(KeyEvent::Meta(';'))
            .expect("M-; should comment active region");
        assert_eq!(
            editor.document().buffer().serialize(),
            "// one\n  // two\n\nthree\n"
        );

        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set again");
        editor.cursor = Position::new(2, 0);
        editor
            .handle_key(KeyEvent::Meta(';'))
            .expect("M-; should uncomment fully commented region");
        assert_eq!(
            editor.document().buffer().serialize(),
            "one\n  two\n\nthree\n"
        );
    }

    #[test]
    fn comment_region_and_uncomment_region_use_mode_markers() {
        let directory = TestDir::new();
        let path = directory.path().join("config.toml");
        fs::write(&path, "title = \"demo\"\n[table]\n").expect("fixture should write");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));

        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(2, 0);
        editor
            .execute_command_by_name("comment-region")
            .expect("comment-region should run");
        assert_eq!(
            editor.document().buffer().serialize(),
            "# title = \"demo\"\n# [table]\n"
        );

        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set again");
        editor.cursor = Position::new(2, 0);
        editor
            .execute_command_by_name("uncomment-region")
            .expect("uncomment-region should run");
        assert_eq!(
            editor.document().buffer().serialize(),
            "title = \"demo\"\n[table]\n"
        );
    }

    #[test]
    fn comment_region_adds_marker_to_partially_commented_region_and_undoes() {
        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        fs::write(&path, "one\n// two\nthree\n").expect("fixture should write");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));

        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor.cursor = Position::new(3, 0);
        editor
            .execute_command_by_name("comment-region")
            .expect("comment-region should run");
        assert_eq!(
            editor.document().buffer().serialize(),
            "// one\n// // two\n// three\n"
        );

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore mixed region");
        assert_eq!(
            editor.document().buffer().serialize(),
            "one\n// two\nthree\n"
        );
    }

    #[test]
    fn comment_commands_report_missing_syntax_and_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "plain")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor
            .handle_key(KeyEvent::Meta(';'))
            .expect("M-; should report missing syntax");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: No comment syntax for current mode")
        );

        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        fs::write(&path, "let value = 1;\n").expect("fixture should write");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .handle_key(KeyEvent::Meta(';'))
            .expect("read-only M-; should not edit");
        assert_eq!(editor.document().buffer().serialize(), "let value = 1;\n");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: main.rs")
        );
    }

    #[test]
    fn moves_back_to_indentation() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "    alpha\n  beta\n    \n\tomega")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor.cursor = Position::new(0, "    alpha".len());
        editor
            .handle_key(KeyEvent::Meta('m'))
            .expect("M-m should move to first non-whitespace character");
        assert_eq!(editor.cursor(), Position::new(0, 4));

        editor.cursor = Position::new(1, "  beta".len());
        editor
            .execute_command_by_name("back-to-indentation")
            .expect("back-to-indentation should execute by name");
        assert_eq!(editor.cursor(), Position::new(1, 2));

        editor.cursor = Position::new(2, 0);
        editor
            .handle_key(KeyEvent::Meta('m'))
            .expect("M-m should move to end of all-whitespace line");
        assert_eq!(editor.cursor(), Position::new(2, 4));

        editor.cursor = Position::new(3, "\tomega".len());
        editor
            .handle_key(KeyEvent::Meta('m'))
            .expect("M-m should treat tabs as indentation");
        assert_eq!(editor.cursor(), Position::new(3, 1));
    }

    #[test]
    fn moves_to_beginning_and_end_of_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "first\nmiddle\nlast")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('>'))
            .expect("M-> should move to end of buffer");
        assert_eq!(editor.cursor(), Position::new(2, "last".len()));

        editor
            .handle_key(KeyEvent::Meta('<'))
            .expect("M-< should move to beginning of buffer");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .execute_command_by_name("end-of-buffer")
            .expect("end-of-buffer should execute by name");
        assert_eq!(editor.cursor(), Position::new(2, "last".len()));

        editor
            .execute_command_by_name("beginning-of-buffer")
            .expect("beginning-of-buffer should execute by name");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn page_scroll_moves_by_visible_text_rows_with_overlap() {
        let text = (0..20)
            .map(|line| format!("line {line:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), &text)
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.ensure_current_window_contains_cursor(6, 80, 0);

        editor
            .handle_key(KeyEvent::Ctrl('v'))
            .expect("C-v should scroll forward");
        assert_eq!(editor.cursor(), Position::new(5, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            5
        );

        editor
            .handle_key(KeyEvent::Meta('v'))
            .expect("M-v should scroll backward");
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            0
        );
    }

    #[test]
    fn quoted_insert_inserts_supported_literal_keys() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for next key");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("C-q-"));
        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("quoted printable should insert");
        assert_eq!(editor.document().buffer().serialize(), "z");
        assert_eq!(editor.cursor(), Position::new(0, 1));

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for tab");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("quoted tab should insert");
        assert_eq!(editor.document().buffer().serialize(), "z\t");

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for enter");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("quoted enter should insert newline");
        assert_eq!(editor.document().buffer().serialize(), "z\t\n");

        editor
            .execute_command_by_name("undo")
            .expect("undo should remove quoted newline");
        assert_eq!(editor.document().buffer().serialize(), "z\t");
    }

    #[test]
    fn quoted_insert_cancel_uses_quit_cleanup() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "xy")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        assert!(editor.active_region_range().is_some());

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for cancel");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel quoted insert");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
        assert!(editor.active_region_range().is_none());

        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("next key should insert normally");
        assert_eq!(editor.document().buffer().serialize(), "xzy");
    }

    #[test]
    fn quoted_insert_rejects_control_keys_and_read_only_buffers() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for next key");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("unsupported control should be reported");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: quoted control insertion is not supported")
        );

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for next key after rejection");
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("quoted NUL should be reported separately");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: quoted NUL insertion is not supported")
        );

        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("C-q should wait for cancel");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel quoted insert");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("x should insert normally after cancel");
        assert_eq!(editor.document().buffer().serialize(), "x");

        editor
            .execute_command_by_name("undo")
            .expect("undo should remove x before read-only check");
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("read-only quoted insert should not error");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("x should not be consumed by quoted insert after read-only guard");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn extended_command_completion_accepts_selected_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should execute selected completion");

        assert!(!editor.search_highlighting());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Search highlighting disabled")
        );
    }

    #[test]
    fn extended_command_completion_keeps_exact_name_entry_working() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-search-highlighting".to_owned()))
            .expect("exact command name should update prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should execute exact command name");

        assert!(!editor.search_highlighting());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Search highlighting disabled")
        );
    }

    #[test]
    fn extended_command_completion_prefers_exact_name_over_selected_substring() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc def")
            .expect("fixture text should insert");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    matching: CompletionMatching::Substring,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("kill-word".to_owned()))
            .expect("exact command name should update prompt");
        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some("backward-kill-word")
        );

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should execute the exact command name");

        assert_eq!(editor.document().buffer().serialize(), " def");
    }

    #[test]
    fn extended_command_completion_explicit_selection_overrides_exact_name() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("find-file".to_owned()))
            .expect("exact command name should update prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next command");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should execute explicitly selected command");

        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::FindFileReadOnly)
        );
        assert!(
            editor
                .minibuffer()
                .display_text()
                .as_deref()
                .is_some_and(|text| text.starts_with("Find file read-only: "))
        );
        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some(current_dir_prompt_input().as_str())
        );
    }

    #[test]
    fn extended_command_empty_input_accepts_selected_completion() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some("about-rile")
        );
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should execute selected completion");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("About Rile:")
        );
    }

    #[test]
    fn extended_command_completion_reports_no_match_as_raw_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("nosuchcommand".to_owned()))
            .expect("unknown command should update prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should submit raw command name");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No such command: nosuchcommand")
        );
    }

    #[test]
    fn extended_command_completion_meta_ret_submits_raw_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("file find".to_owned()))
            .expect("orderless input should update completion");
        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some("find-file")
        );
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Enter))
            .expect("M-RET should submit raw input");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No such command: file find")
        );
    }

    #[test]
    fn extended_command_tab_inserts_selected_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should insert selected command");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some("toggle-search-highlighting")
        );
    }

    #[test]
    fn find_file_completion_tab_inserts_selected_file() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-note.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(directory.path().join("alphabet-note.txt"), "alphabet")
            .expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alp".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should insert selected file");

        let expected = alpha.to_string_lossy();
        assert_eq!(editor.minibuffer().prompt_input(), Some(expected.as_ref()));
    }

    #[test]
    fn find_file_completion_explicit_selection_overrides_exact_file() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-note.txt");
        let extra = directory.path().join("alpha-note.txt-extra");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&extra, "extra").expect("extra fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-note.txt".to_owned()))
            .expect("exact file input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select prefixed file");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open explicitly selected file");

        assert_eq!(editor.document().path(), Some(extra.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "extra");
    }

    #[test]
    fn find_file_completion_accepts_selected_sibling_file() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-note.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-n".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should complete selected file");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open selected file");

        assert_eq!(editor.document().buffer().serialize(), "alpha");
        assert_eq!(editor.document().path(), Some(alpha.as_path()));
    }

    #[test]
    fn find_file_completion_ret_accepts_smart_case_selected_file() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let readme = directory.path().join("README.md");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&readme, "upper").expect("readme fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("readme.md".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should accept selected file");

        assert_eq!(editor.document().path(), Some(readme.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "upper");
    }

    #[test]
    fn find_file_completion_meta_ret_keeps_raw_input() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let readme = directory.path().join("README.md");
        let raw = directory.path().join("readme.md");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&readme, "upper").expect("readme fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("readme.md".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Enter))
            .expect("M-RET should submit raw input");

        assert_eq!(editor.document().path(), Some(raw.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn find_file_completion_keeps_raw_missing_file_input() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let missing = directory.path().join("new-note.txt");
        fs::write(&start, "start").expect("start fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("new-note.txt".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open raw missing file");

        assert_eq!(editor.document().path(), Some(missing.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn find_file_completion_accepts_default_substring_match() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-note.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("ote".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should accept selected substring match");

        assert_eq!(editor.document().path(), Some(alpha.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn substring_file_completion_accepts_selected_substring_file() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-note.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    matching: CompletionMatching::Substring,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("note".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open selected substring file");

        assert_eq!(editor.document().path(), Some(alpha.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn find_file_completion_keeps_raw_input_when_substring_directory_is_selected() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let missing = directory.path().join("note");
        fs::write(&start, "start").expect("start fixture should write");
        fs::create_dir(directory.path().join("alpha-note-dir")).expect("directory should create");
        fs::write(directory.path().join("beta-note.txt"), "beta")
            .expect("beta fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    matching: CompletionMatching::Substring,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("note".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should keep raw directory-ambiguous input");

        assert_eq!(editor.document().path(), Some(missing.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn find_file_completion_resolves_relative_paths_from_current_buffer_directory() {
        let directory = TestDir::new();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("nested directory should create");
        let start = nested.join("start.txt");
        let sibling = nested.join("sibling.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&sibling, "sibling").expect("sibling fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        let expected_base = file_prompt_base_input(&nested);
        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some(expected_base.as_str())
        );
        editor
            .handle_key(KeyEvent::Text("sibling.txt".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open sibling file");

        assert_eq!(editor.document().path(), Some(sibling.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "sibling");
    }

    #[test]
    fn find_file_completion_enters_selected_directory() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::create_dir(directory.path().join("alpha-dir")).expect("directory should create");
        fs::write(directory.path().join("alpha-dir").join("note.txt"), "note")
            .expect("nested fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-dir".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should descend into directory");

        let expected = file_prompt_base_input(&directory.path().join("alpha-dir"));
        assert_eq!(editor.minibuffer().prompt_input(), Some(expected.as_str()));
        assert!(editor.completion().is_some());
    }

    #[test]
    fn find_file_completion_enters_selected_child_directory() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let parent = directory.path().join("aaa-dir");
        fs::write(&start, "start").expect("start fixture should write");
        fs::create_dir(&parent).expect("parent directory should create");
        fs::create_dir(parent.join("child-dir")).expect("child directory should create");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty enter should descend into selected directory");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("second enter should descend into selected child directory");

        let expected = file_prompt_base_input(&parent.join("child-dir"));
        assert_eq!(editor.minibuffer().prompt_input(), Some(expected.as_str()));
        assert!(editor.completion().is_some());
    }

    #[test]
    fn ido_file_completion_renders_candidates_in_minibuffer() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(directory.path().join("alpha-note.txt"), "alpha")
            .expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");

        let text = editor
            .minibuffer_display_text()
            .expect("ido should render minibuffer text");
        assert!(text.contains("Find file: "));
        assert!(text.contains("alpha"));
        assert!(text.contains("alpha-note.txt"));
    }

    #[test]
    fn completions_buffer_file_completion_uses_file_title() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(directory.path().join("alpha-note.txt"), "alpha")
            .expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");

        assert_eq!(editor.current_buffer_name(), "*Completions*");
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("Possible Completions for Find file:")
        );
    }

    #[test]
    fn buffer_completion_extends_unique_prefix_and_switches() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-b".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should complete unique buffer name");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-buffer.txt"));

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to completed buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn switch_buffer_preserves_buffer_point() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "one\ntwo\nthree").expect("alpha fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("next-line should move in alpha");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
        assert_eq!(editor.cursor(), Position::new(1, 0));

        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");
        assert_eq!(editor.current_buffer_name(), "start.txt");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-buffer.txt".to_owned()))
            .expect("prompt input should name alpha");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to alpha");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn buffer_completion_enter_accepts_selected_default_candidate() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should accept selected buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Switched to buffer alpha-buffer.txt")
        );
    }

    #[test]
    fn buffer_completion_empty_input_switches_default_buffer() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");

        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Switch to buffer (default alpha-buffer.txt): ")
        );

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to the default buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Switched to buffer alpha-buffer.txt")
        );
    }

    #[test]
    fn ido_buffer_completion_empty_input_switches_default_buffer() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to the default buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
    }

    #[test]
    fn completions_buffer_buffer_completion_empty_input_switches_default_buffer() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::with_config(
            document,
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to the default buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt");
    }

    #[test]
    fn buffer_completion_tab_accepts_selected_default_candidate() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should accept selected default candidate");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-buffer.txt"));
    }

    #[test]
    fn buffer_completion_preserves_space_sensitive_exact_name() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let spaced = directory.path().join(" alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&spaced, "spaced alpha").expect("spaced fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(spaced.to_str().unwrap())
            .expect("spaced buffer should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text(" alpha-buffer.txt".to_owned()))
            .expect("exact space-sensitive name should update prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to exact space-sensitive buffer");

        assert_eq!(editor.current_buffer_name(), " alpha-buffer.txt");
        assert_eq!(editor.document().buffer().serialize(), "spaced alpha");
    }

    #[test]
    fn buffer_completion_explicit_selection_overrides_exact_name() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alpha-buffer.txt-extra");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-buffer.txt".to_owned()))
            .expect("exact buffer name should update prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select prefixed buffer");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should switch to explicitly selected buffer");

        assert_eq!(editor.current_buffer_name(), "alpha-buffer.txt-extra");
        assert_eq!(editor.document().buffer().serialize(), "alphabet");
    }

    #[test]
    fn buffer_completion_accepts_explicit_selection() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next buffer");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should accept selected buffer");

        assert_eq!(editor.current_buffer_name(), "alphabet-buffer.txt");
        assert_eq!(editor.document().buffer().serialize(), "alphabet");
    }

    #[test]
    fn ido_buffer_completion_renders_candidates_in_minibuffer() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");

        let text = editor
            .minibuffer_display_text()
            .expect("ido should render minibuffer text");
        assert!(text.contains("Switch to buffer (default *scratch*): "));
        assert!(text.contains("*scratch*"));
    }

    #[test]
    fn completions_buffer_buffer_completion_uses_buffer_title() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("switch-buffer should start prompt");

        assert_eq!(editor.current_buffer_name(), "*Completions*");
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("Possible Completions for Switch to buffer:")
        );
    }

    #[test]
    fn kill_buffer_completion_extends_unique_prefix_and_kills() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha-b".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should complete unique buffer name");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-buffer.txt"));

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should kill completed buffer");

        assert_eq!(editor.buffer_count(), 2);
        assert!(editor.buffers.find_by_name("alpha-buffer.txt").is_none());
        assert_eq!(editor.current_buffer_name(), "alphabet-buffer.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed buffer alpha-buffer.txt")
        );
    }

    #[test]
    fn kill_buffer_completion_tab_accepts_selected_default_candidate() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should accept selected default candidate");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-buffer.txt"));
    }

    #[test]
    fn kill_buffer_completion_enter_accepts_selected_default_candidate() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should kill selected default candidate");

        assert_eq!(editor.buffer_count(), 2);
        assert!(editor.buffers.find_by_name("alpha-buffer.txt").is_none());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed buffer alpha-buffer.txt")
        );
    }

    #[test]
    fn kill_buffer_completion_preserves_space_sensitive_exact_name() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let spaced = directory.path().join(" alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&spaced, "leading alpha").expect("spaced fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(spaced.to_str().unwrap())
            .expect("spaced buffer should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text(" alpha-buffer.txt".to_owned()))
            .expect("prompt input should keep leading space");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should kill exact buffer name");

        assert_eq!(editor.buffer_count(), 2);
        assert!(editor.buffers.find_by_name(" alpha-buffer.txt").is_none());
        assert!(editor.buffers.find_by_name("alphabet-buffer.txt").is_some());
        assert_eq!(editor.current_buffer_name(), "alphabet-buffer.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed buffer  alpha-buffer.txt")
        );
    }

    #[test]
    fn kill_buffer_completion_accepts_selected_candidate() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let alpha = directory.path().join("alpha-buffer.txt");
        let alphabet = directory.path().join("alphabet-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&alpha, "alpha").expect("alpha fixture should write");
        fs::write(&alphabet, "alphabet").expect("alphabet fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(alpha.to_str().unwrap())
            .expect("alpha should open");
        editor
            .find_file(alphabet.to_str().unwrap())
            .expect("alphabet should open");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next buffer");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should kill selected buffer");

        assert_eq!(editor.current_buffer_name(), "start.txt");
        assert!(editor.buffers.find_by_name("alphabet-buffer.txt").is_none());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed buffer alphabet-buffer.txt")
        );
    }

    #[test]
    fn ido_kill_buffer_completion_renders_candidates_in_minibuffer() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");

        let text = editor
            .minibuffer_display_text()
            .expect("ido should render minibuffer text");
        assert!(text.contains("Kill buffer (default *scratch*): "));
        assert!(text.contains("*scratch*"));
    }

    #[test]
    fn completions_buffer_kill_buffer_completion_uses_buffer_title() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");

        assert_eq!(editor.current_buffer_name(), "*Completions*");
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("Possible Completions for Kill buffer:")
        );
    }

    #[test]
    fn list_buffers_opens_read_only_buffer_list_in_other_window() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let source = directory.path().join("main.rs");
        fs::write(&start, "start\n").expect("start fixture should write");
        fs::write(&source, "fn main() {}\n").expect("source fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        let start_buffer = editor.current_buffer_id();
        editor
            .find_file(source.to_str().unwrap())
            .expect("source should open");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("buffers should list");

        assert_eq!(editor.current_buffer_id(), start_buffer);
        assert_eq!(editor.current_buffer_name(), "start.txt");
        assert_eq!(editor.window_count(), 2);

        let list_window = editor
            .window_layouts(12, 80)
            .into_iter()
            .find(|layout| layout.id != editor.current_window_id())
            .expect("other window should exist")
            .id;
        let list_buffer = editor
            .window_viewport(list_window)
            .expect("list window should have viewport")
            .buffer;
        let list_document = editor
            .document_for_buffer(list_buffer)
            .expect("list buffer should exist");
        let text = list_document.buffer().serialize();

        assert_eq!(list_document.display_name(), "*Buffer List*");
        assert!(list_document.is_read_only());
        assert!(text.contains("CRM Buffer"));
        assert!(text.contains(".   start.txt"));
        assert!(text.contains("main.rs"));
        assert!(text.contains("Rust"));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("buffers should refresh");
        assert_eq!(editor.window_count(), 2);
    }

    #[test]
    fn q_closes_selected_buffer_list_window() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        assert_eq!(editor.window_count(), 2);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");
        assert_eq!(editor.current_buffer_name(), "*Buffer List*");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should close buffer list window");

        assert_eq!(editor.window_count(), 1);
        assert_eq!(editor.current_buffer_name(), "*scratch*");
    }

    #[test]
    fn enter_on_buffer_list_row_switches_selected_window() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let source = directory.path().join("main.rs");
        fs::write(&start, "start\n").expect("start fixture should write");
        fs::write(&source, "fn main() {}\n").expect("source fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(source.to_str().unwrap())
            .expect("source should open");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");
        editor.cursor = Position::new(3, 0);

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open selected buffer");

        assert_eq!(editor.current_buffer_name(), "main.rs");
        assert_eq!(editor.window_count(), 2);
        assert_eq!(editor.document().buffer().serialize(), "fn main() {}\n");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Switched to buffer main.rs")
        );
    }

    #[test]
    fn enter_on_buffer_list_header_or_separator_keeps_list_selected() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");

        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("header enter should be ignored");
        assert_eq!(editor.current_buffer_name(), "*Buffer List*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No buffer on this line")
        );

        editor.cursor = Position::new(1, 0);
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("separator enter should be ignored");
        assert_eq!(editor.current_buffer_name(), "*Buffer List*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No buffer on this line")
        );
    }

    #[test]
    fn enter_on_stale_buffer_list_metadata_keeps_list_selected() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");
        editor.buffer_list_rows[2] = Some(BufferId(usize::MAX));
        editor.cursor = Position::new(2, 0);

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("stale enter should be ignored");

        assert_eq!(editor.current_buffer_name(), "*Buffer List*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: buffer no longer exists")
        );
    }

    #[test]
    fn visible_buffer_list_refreshes_after_opening_buffer() {
        let directory = TestDir::new();
        let source = directory.path().join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("source fixture should write");
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        let buffer_list = editor
            .buffers
            .find_by_name("*Buffer List*")
            .expect("buffer list should exist");
        assert!(
            !editor
                .document_for_buffer(buffer_list)
                .expect("buffer list should exist")
                .buffer()
                .serialize()
                .contains("main.rs")
        );

        editor
            .find_file(source.to_str().unwrap())
            .expect("source should open");

        let text = editor
            .document_for_buffer(buffer_list)
            .expect("buffer list should still exist")
            .buffer()
            .serialize();
        assert!(text.contains(".   main.rs"));
        assert!(text.contains("Rust"));
    }

    #[test]
    fn visible_buffer_list_refreshes_row_metadata_after_opening_buffer() {
        let directory = TestDir::new();
        let source = directory.path().join("main.rs");
        fs::write(&source, "fn main() {}\n").expect("source fixture should write");
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        editor
            .find_file(source.to_str().unwrap())
            .expect("source should open and refresh visible list");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");
        editor.cursor = Position::new(3, 0);

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("refreshed row should open buffer");

        assert_eq!(editor.current_buffer_name(), "main.rs");
        assert_eq!(editor.document().buffer().serialize(), "fn main() {}\n");
    }

    #[test]
    fn q_leaves_buffer_list_when_it_is_the_only_buffer() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("list-buffers")
            .expect("buffers should list");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other window should select buffer list");
        editor
            .execute_command_by_name("delete-other-windows")
            .expect("only buffer list window should remain");
        editor
            .kill_buffer("*scratch*")
            .expect("scratch should be killed");

        assert_eq!(editor.window_count(), 1);
        assert_eq!(editor.buffer_count(), 1);
        assert_eq!(editor.current_buffer_name(), "*Buffer List*");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should leave buffer list");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(editor.buffer_count(), 1);
    }

    #[test]
    fn prompt_history_navigation_covers_history_prompt_kinds() {
        let history_kinds = [
            PromptKind::ExtendedCommand,
            PromptKind::DescribeFunction,
            PromptKind::DescribeVariable,
            PromptKind::FindFile,
            PromptKind::FindFileReadOnly,
            PromptKind::GotoLine,
            PromptKind::InsertFile,
            PromptKind::KillBuffer,
            PromptKind::RectangleNumberFormat,
            PromptKind::RectangleNumberStart,
            PromptKind::ShellCommand,
            PromptKind::StringRectangle,
            PromptKind::SwitchToBuffer,
            PromptKind::WriteFile,
        ];

        for kind in history_kinds {
            assert!(super::prompt_history::prompt_kind_uses_history(kind));
            let mut editor = Editor::new(Document::scratch());
            editor.prompt_history.record(kind, "previous input");
            editor
                .minibuffer
                .start_prompt(kind, super::prompt_label(kind));
            editor.minibuffer.set_prompt_input("draft input");

            editor
                .handle_key(KeyEvent::Meta('p'))
                .expect("M-p should recall prompt history");
            assert_eq!(editor.minibuffer().prompt_input(), Some("previous input"));

            editor
                .handle_key(KeyEvent::Meta('n'))
                .expect("M-n should restore prompt draft");
            assert_eq!(editor.minibuffer().prompt_input(), Some("draft input"));
        }
    }

    #[test]
    fn plain_prompt_inserts_at_minibuffer_cursor() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");

        editor
            .handle_key(KeyEvent::Text("ac".to_owned()))
            .expect("prompt input should insert");
        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("C-b should move within prompt");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("prompt input should insert at cursor");

        assert_eq!(editor.minibuffer().prompt_input(), Some("abc"));
        assert_eq!(editor.minibuffer().prompt_input_before_cursor(), Some("ab"));
    }

    #[test]
    fn completion_prompt_inserts_at_minibuffer_cursor() {
        let mut editor = Editor::new(Document::scratch());
        let completion = CompletionSession::commands(
            &CommandRegistry::default(),
            &KeyMap::default(),
            editor.completion_config,
        );
        start_test_completion_prompt(
            &mut editor,
            PromptKind::ExtendedCommand,
            completion,
            "toggle-search-highlighting",
        );

        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("C-b should move within completion prompt");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("prompt input should insert at cursor");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some("toggle-search-highlightinxg")
        );
        assert_eq!(
            editor.minibuffer().prompt_input_before_cursor(),
            Some("toggle-search-highlightinx")
        );
    }

    #[test]
    fn prompt_word_movement_uses_minibuffer_cursor() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("one two_three");

        editor
            .handle_key(KeyEvent::Meta('b'))
            .expect("M-b should move by word in prompt");
        assert_eq!(
            editor.minibuffer().prompt_input_before_cursor(),
            Some("one ")
        );

        editor
            .handle_key(KeyEvent::Meta('f'))
            .expect("M-f should move by word in prompt");
        assert_eq!(
            editor.minibuffer().prompt_input_before_cursor(),
            Some("one two_three")
        );
    }

    #[test]
    fn prompt_start_end_and_forward_delete_use_minibuffer_cursor() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("abc");

        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("C-a should move to prompt start");
        assert_eq!(editor.minibuffer().prompt_input_before_cursor(), Some(""));

        editor
            .handle_key(KeyEvent::Ctrl('d'))
            .expect("C-d should delete after prompt point");
        assert_eq!(editor.minibuffer().prompt_input(), Some("bc"));

        editor
            .handle_key(KeyEvent::Special(SpecialKey::End))
            .expect("End should move to prompt end");
        assert_eq!(editor.minibuffer().prompt_input_before_cursor(), Some("bc"));
    }

    #[test]
    fn minibuffer_kill_line_yanks_into_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("echo suffix");

        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("C-a should move to prompt start");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("C-k should kill prompt suffix");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        editor
            .execute_command_by_name("yank")
            .expect("yank should insert minibuffer kill");

        assert_eq!(editor.document().buffer().serialize(), "echo suffix");
    }

    #[test]
    fn minibuffer_word_kills_yank_into_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("alpha beta");

        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Backspace))
            .expect("M-Backspace should kill previous prompt word");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        editor
            .execute_command_by_name("yank")
            .expect("yank should insert minibuffer kill");

        assert_eq!(editor.document().buffer().serialize(), "beta");

        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("gamma delta");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("C-a should move to prompt start");
        editor
            .handle_key(KeyEvent::Meta('d'))
            .expect("M-d should kill next prompt word");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        editor
            .execute_command_by_name("yank")
            .expect("yank should insert minibuffer kill");

        assert_eq!(editor.document().buffer().serialize(), "betagamma");

        editor
            .minibuffer
            .start_prompt(PromptKind::ShellCommand, "Shell command: ");
        editor.minibuffer.insert_prompt_text("omega psi");
        editor
            .handle_key(KeyEvent::CtrlSpecial(SpecialKey::Backspace))
            .expect("C-Backspace should kill previous prompt word when encoded");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        editor
            .execute_command_by_name("yank")
            .expect("yank should insert minibuffer kill");

        assert_eq!(editor.document().buffer().serialize(), "betagammapsi");
    }

    #[test]
    fn completion_prompt_deletion_refreshes_matches() {
        assert_completion_prompt_edit_refreshes_matches(
            "ztoggle-search-highlighting",
            &[KeyEvent::Ctrl('a')],
            KeyEvent::Ctrl('d'),
            "toggle-search-highlighting",
        );
    }

    #[test]
    fn completion_prompt_kill_commands_refresh_matches() {
        assert_completion_prompt_edit_refreshes_matches(
            "toggle-search-highlightingz",
            &[KeyEvent::Ctrl('b')],
            KeyEvent::Ctrl('k'),
            "toggle-search-highlighting",
        );
        assert_completion_prompt_edit_refreshes_matches(
            "z toggle-search-highlighting",
            &[KeyEvent::Ctrl('a')],
            KeyEvent::Meta('d'),
            " toggle-search-highlighting",
        );
        assert_completion_prompt_edit_refreshes_matches(
            "z toggle-search-highlighting",
            &[KeyEvent::Ctrl('a'), KeyEvent::Ctrl('f')],
            KeyEvent::MetaSpecial(SpecialKey::Backspace),
            " toggle-search-highlighting",
        );
        assert_completion_prompt_edit_refreshes_matches(
            "z toggle-search-highlighting",
            &[KeyEvent::Ctrl('a'), KeyEvent::Ctrl('f')],
            KeyEvent::CtrlSpecial(SpecialKey::Backspace),
            " toggle-search-highlighting",
        );
    }

    #[test]
    fn prompt_history_recalls_previous_m_x_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-line-numbers".to_owned()))
            .expect("prompt input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should submit command");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt again");
        editor
            .handle_key(KeyEvent::Meta('p'))
            .expect("M-p should recall history");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some("toggle-line-numbers")
        );

        editor
            .handle_key(KeyEvent::Meta('n'))
            .expect("M-n should return to draft");

        assert_eq!(editor.minibuffer().prompt_input(), Some(""));
    }

    #[test]
    fn prompt_history_preserves_current_draft() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-line-numbers".to_owned()))
            .expect("prompt input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should submit command");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt again");
        editor
            .handle_key(KeyEvent::Text("toggle".to_owned()))
            .expect("draft input should update");
        editor
            .handle_key(KeyEvent::Meta('p'))
            .expect("M-p should recall history");
        editor
            .handle_key(KeyEvent::Meta('n'))
            .expect("M-n should restore draft");

        assert_eq!(editor.minibuffer().prompt_input(), Some("toggle"));
    }

    #[test]
    fn prompt_history_is_per_prompt_kind() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-line-numbers".to_owned()))
            .expect("prompt input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should submit command");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Meta('p'))
            .expect("M-p should not recall M-x history");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some(current_dir_prompt_input().as_str())
        );
    }

    #[test]
    fn prompt_history_updates_completion_after_recall() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-line-numbers".to_owned()))
            .expect("prompt input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should submit command");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt again");
        editor
            .handle_key(KeyEvent::Meta('p'))
            .expect("M-p should recall history and update completion");

        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some("toggle-line-numbers")
        );
    }

    #[test]
    fn completion_prompt_navigation_uses_control_and_page_keys_for_all_sources() {
        for kind in [PromptKind::ExtendedCommand, PromptKind::DescribeFunction] {
            let mut editor = editor_with_small_completion_page(Document::scratch());
            let completion = CompletionSession::commands(
                &CommandRegistry::default(),
                &KeyMap::default(),
                editor.completion_config,
            );
            start_test_completion_prompt(&mut editor, kind, completion, "toggle-");
            assert_completion_movement_keys(
                &mut editor,
                "toggle-read-only",
                "toggle-line-numbers",
                "toggle-search-highlighting",
            );
        }

        let mut editor = editor_with_small_completion_page(Document::scratch());
        let completion =
            CompletionSession::options(&OptionRegistry::default(), editor.completion_config);
        start_test_completion_prompt(
            &mut editor,
            PromptKind::DescribeVariable,
            completion,
            "completion_",
        );
        assert_completion_movement_keys(
            &mut editor,
            "completion_style",
            "completion_matching",
            "completion_max_candidates",
        );

        let directory = TestDir::new();
        fs::write(directory.path().join("alpha-one.txt"), "one").expect("fixture should write");
        fs::write(directory.path().join("alpha-two.txt"), "two").expect("fixture should write");
        fs::write(directory.path().join("alpha-three.txt"), "three").expect("fixture should write");
        for kind in [
            PromptKind::FindFile,
            PromptKind::FindFileReadOnly,
            PromptKind::InsertFile,
        ] {
            let mut editor = editor_with_small_completion_page(Document::scratch());
            let completion = CompletionSession::files(directory.path(), editor.completion_config);
            start_test_completion_prompt(&mut editor, kind, completion, "alpha");
            assert_completion_movement_keys(
                &mut editor,
                "alpha-one.txt",
                "alpha-three.txt",
                "alpha-two.txt",
            );
        }

        for kind in [PromptKind::SwitchToBuffer, PromptKind::KillBuffer] {
            let mut editor = editor_with_small_completion_page(Document::scratch());
            let completion = CompletionSession::buffers(
                ["alpha-one", "alpha-two", "alpha-three"].map(str::to_owned),
                editor.completion_config,
            );
            start_test_completion_prompt(&mut editor, kind, completion, "alpha");
            assert_completion_movement_keys(&mut editor, "alpha-one", "alpha-two", "alpha-three");
        }
    }

    #[test]
    fn non_completion_prompts_ignore_completion_movement_keys() {
        let non_completion_kinds = [
            PromptKind::GotoLine,
            PromptKind::IncrementalSearch,
            PromptKind::KillDirtyBuffer,
            PromptKind::QueryReplaceReplacement,
            PromptKind::QueryReplaceSearch,
            PromptKind::QuitDirtyBuffers,
            PromptKind::RectangleNumberFormat,
            PromptKind::RectangleNumberStart,
            PromptKind::ShellCommand,
            PromptKind::StringRectangle,
            PromptKind::WriteFile,
        ];

        for kind in non_completion_kinds {
            let mut editor = Editor::new(Document::scratch());
            editor
                .minibuffer
                .start_prompt(kind, super::prompt_label(kind));
            editor.minibuffer.set_prompt_input("draft");

            for key in [
                KeyEvent::Ctrl('n'),
                KeyEvent::Ctrl('p'),
                KeyEvent::Ctrl('v'),
                KeyEvent::Meta('v'),
            ] {
                editor
                    .handle_key(key)
                    .expect("movement key should be ignored");
                assert_eq!(editor.minibuffer().prompt_kind(), Some(kind));
                assert_eq!(editor.minibuffer().prompt_input(), Some("draft"));
            }
        }
    }

    #[test]
    fn prompt_history_resets_when_file_completion_enters_directory() {
        let directory = TestDir::new();
        let alpha_dir = directory.path().join("alpha-dir");
        fs::create_dir(&alpha_dir).expect("directory fixture should create");
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .prompt_history
            .record(PromptKind::FindFile, "alpha-dir");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Meta('p'))
            .expect("M-p should recall directory history");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should descend into recalled directory");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-dir/"));

        editor
            .handle_key(KeyEvent::Meta('n'))
            .expect("M-n should not restore stale draft after descent");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-dir/"));
    }

    #[test]
    fn ido_completion_renders_candidates_in_minibuffer() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");

        let text = editor
            .minibuffer_display_text()
            .expect("ido should render minibuffer text");
        assert!(text.contains("M-x toggle-s"));
        assert!(text.contains("toggle-search-highlighting"));
        assert!(text.contains("toggle-syntax-highlighting"));
    }

    #[test]
    fn vertical_completion_minibuffer_uses_two_space_count_gap() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");

        assert_eq!(
            editor.minibuffer_display_text().as_deref(),
            Some("1/2  M-x toggle-s")
        );
    }

    #[test]
    fn completions_buffer_completion_restores_previous_buffer_on_cancel() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        assert_eq!(editor.current_buffer_name(), "*Completions*");
        assert!(
            editor
                .document_for_buffer(editor.current_buffer_id())
                .expect("current buffer should exist")
                .is_completions()
        );

        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn completions_buffer_refresh_keeps_original_return_buffer() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );
        editor
            .handle_key(KeyEvent::Text("buffer text".to_owned()))
            .expect("fixture text should insert");
        editor.cursor = Position::new(0, "buffer".len());
        let original = editor.current_buffer_id();

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt and completions buffer");
        assert_eq!(editor.current_buffer_name(), "*Completions*");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should refresh completions while inside completions buffer");
        assert_eq!(editor.current_buffer_name(), "*Completions*");

        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");

        assert_eq!(editor.current_buffer_id(), original);
        assert_eq!(editor.cursor(), Position::new(0, "buffer".len()));
        assert_eq!(editor.document().buffer().serialize(), "buffer text");
    }

    #[test]
    fn completions_buffer_completion_restores_previous_buffer_on_accept() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::CompletionsBuffer,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Text("buffer text".to_owned()))
            .expect("fixture text should insert");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion buffer");
        assert_eq!(editor.current_buffer_name(), "*Completions*");

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should accept selected command");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(editor.document().buffer().serialize(), "buffer text");
        assert!(!editor.search_highlighting());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Search highlighting disabled")
        );
    }

    #[test]
    fn recenter_cycles_viewport_without_moving_cursor() {
        let text = (0..20)
            .map(|line| format!("line {line:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), &text)
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.ensure_current_window_contains_cursor(6, 80, 0);

        for _ in 0..12 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
        }
        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("C-l should recenter");

        assert_eq!(editor.cursor(), Position::new(12, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            9
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("second C-l should put point at top");
        assert_eq!(editor.cursor(), Position::new(12, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            12
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("third C-l should put point at bottom");
        assert_eq!(editor.cursor(), Position::new(12, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            7
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("fourth C-l should cycle back to center");
        assert_eq!(editor.cursor(), Position::new(12, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            9
        );
    }

    #[test]
    fn recenter_top_cycle_can_leave_blank_space_below_short_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(
                Position::new(0, 0),
                "short 000\nshort 001\nshort 002\nshort 003\n",
            )
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.ensure_current_window_contains_cursor(10, 80, 0);
        for _ in 0..2 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
        }

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("first C-l should keep short buffer fully visible");
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            0
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("second C-l should put point at top");
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            2
        );
    }

    #[test]
    fn recenter_cycle_resets_after_other_commands() {
        let text = (0..20)
            .map(|line| format!("line {line:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), &text)
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.ensure_current_window_contains_cursor(6, 80, 0);
        for _ in 0..12 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
        }

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("first C-l should center");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("intervening command should reset recenter cycle");
        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("C-l after another command should center again");

        assert_eq!(editor.cursor(), Position::new(13, 0));
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            10
        );
    }

    #[test]
    fn recenter_center_cycle_can_leave_blank_space_at_end_of_buffer() {
        let text = (0..40)
            .map(|line| format!("line {line:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), &format!("{text}\n"))
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.ensure_current_window_contains_cursor(10, 80, 0);
        editor
            .handle_key(KeyEvent::Meta('>'))
            .expect("M-> should move to end of buffer");
        assert_eq!(editor.cursor(), Position::new(40, 0));

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("first C-l at EOF should center with blank space below");
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            35
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("second C-l at EOF should put point at top");
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            40
        );

        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("third C-l at EOF should put point at bottom");
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("current window should exist")
                .first_visible_line,
            31
        );
    }

    #[test]
    fn handles_c_x_prefix_save_and_quit() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "old").expect("file should be written");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));
        editor
            .handle_key(KeyEvent::Text("!".to_owned()))
            .expect("text should insert");

        assert_eq!(
            editor.handle_key(KeyEvent::Ctrl('x')).expect("prefix ok"),
            EditorOutcome::Continue
        );
        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("save should execute");
        assert_eq!(fs::read_to_string(&path).expect("file should read"), "!old");
        assert!(!editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should begin");
        assert_eq!(
            editor
                .handle_key(KeyEvent::Ctrl('c'))
                .expect("quit should execute"),
            EditorOutcome::Quit
        );
    }

    #[test]
    fn c_x_c_prompts_before_quitting_dirty_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "old").expect("file should be written");
        let mut editor = Editor::new(Document::open(&path).expect("document should open"));
        editor
            .handle_key(KeyEvent::Text("!".to_owned()))
            .expect("text should insert");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should begin");
        assert_eq!(
            editor
                .handle_key(KeyEvent::Ctrl('c'))
                .expect("dirty quit should prompt"),
            EditorOutcome::Continue
        );

        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Modified buffers exist; exit anyway? (yes or no) ")
        );
    }

    #[test]
    fn c_x_c_dirty_quit_cancel_keeps_editor_open() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");
        editor
            .execute_command_by_name("save-buffers-kill-terminal")
            .expect("dirty quit should prompt");

        assert_eq!(
            editor
                .handle_key(KeyEvent::Ctrl('g'))
                .expect("cancel should continue"),
            EditorOutcome::Continue
        );

        assert!(editor.document().is_dirty());
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn c_x_c_dirty_quit_no_answer_keeps_editor_open() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");
        editor
            .execute_command_by_name("save-buffers-kill-terminal")
            .expect("dirty quit should prompt");
        for character in "no".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("answer should type");
        }

        assert_eq!(
            editor
                .handle_key(KeyEvent::Special(SpecialKey::Enter))
                .expect("no should continue"),
            EditorOutcome::Continue
        );

        assert!(editor.document().is_dirty());
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn c_x_c_dirty_quit_yes_answer_exits() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");
        editor
            .execute_command_by_name("save-buffers-kill-terminal")
            .expect("dirty quit should prompt");
        for character in "yes".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("answer should type");
        }

        assert_eq!(
            editor
                .handle_key(KeyEvent::Special(SpecialKey::Enter))
                .expect("yes should quit"),
            EditorOutcome::Quit
        );
    }

    #[test]
    fn c_x_c_prompts_for_dirty_non_current_buffer() {
        let directory = TestDir::new();
        let dirty = directory.path().join("dirty.txt");
        let clean = directory.path().join("clean.txt");
        fs::write(&dirty, "dirty").expect("dirty fixture should write");
        fs::write(&clean, "clean").expect("clean fixture should write");
        let mut editor = Editor::new(Document::open(&dirty).expect("dirty should open"));
        editor
            .handle_key(KeyEvent::Text("!".to_owned()))
            .expect("dirty buffer should edit");
        editor
            .find_file(clean.to_str().unwrap())
            .expect("clean buffer should open");

        assert_eq!(editor.current_buffer_name(), "clean.txt");
        assert!(!editor.document().is_dirty());
        assert_eq!(
            editor
                .execute_command_by_name("save-buffers-kill-terminal")
                .expect("hidden dirty buffer should prompt"),
            EditorOutcome::Continue
        );
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Modified buffers exist; exit anyway? (yes or no) ")
        );
    }

    #[test]
    fn c_x_c_ignores_dirty_special_buffers() {
        let mut editor = Editor::new(Document::scratch());
        let help = editor.buffers.open_help("help");
        editor
            .buffers
            .document_mut(help)
            .expect("help buffer should exist")
            .buffer_mut()
            .insert(Position::new(0, 0), "dirty")
            .expect("help buffer mutation should work");

        assert_eq!(
            editor
                .execute_command_by_name("save-buffers-kill-terminal")
                .expect("dirty special buffer should not block quit"),
            EditorOutcome::Quit
        );
    }

    #[test]
    fn executes_exact_command_name_with_m_x() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should prompt");
        for character in "end-of-line".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("command input should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("command should execute");

        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn reports_unknown_m_x_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should prompt");
        for character in "missing-command".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("command input should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("unknown command should not error");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No such command: missing-command")
        );
    }

    #[test]
    fn edits_and_cancels_m_x_prompt() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should prompt");
        editor
            .handle_key(KeyEvent::Text("é".to_owned()))
            .expect("prompt input should update");
        assert_eq!(editor.minibuffer().display_text().as_deref(), Some("M-x é"));

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prompt backspace should edit");
        assert_eq!(editor.minibuffer().display_text().as_deref(), Some("M-x "));

        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
        assert_eq!(editor.minibuffer().prompt(), None);
    }

    #[test]
    fn c_g_cancels_key_prefix() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("C-x- (C-h for help)")
        );
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prefix");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert after cancel");

        assert_eq!(editor.minibuffer().message.as_deref(), None);
        assert_eq!(editor.document().buffer().serialize(), "x");
    }

    #[test]
    fn prefix_keys_echo_pending_sequence_without_prompt() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");

        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("M-g- (C-h for help)")
        );
        assert_eq!(editor.minibuffer().prompt(), None);

        editor
            .handle_key(KeyEvent::Text("g".to_owned()))
            .expect("goto-line should prompt");

        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Goto line: ")
        );
    }

    #[test]
    fn prefix_help_opens_help_buffer_for_pending_sequence() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Type q in help window to restore previous buffer.")
        );
        assert_eq!(
            editor.document().buffer().serialize(),
            "Global Bindings Starting With M-g:\n\n\
Key             Binding                        Description\n\
---             -------                        -----------\n\n\
M-g g           goto-line                      Go to line or line:column\n"
        );
    }

    #[test]
    fn prefix_help_displays_descriptions_and_space_keys() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("Key             Binding                        Description"));
        assert!(help.contains("C-x SPC"));
        assert!(help.contains("rectangle-mark-mode"));
        assert!(help.contains("Mark a rectangular region"));

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x prefix should start");
        editor
            .handle_key(KeyEvent::Text("r".to_owned()))
            .expect("C-x r prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("C-x r SPC"));
        assert!(help.contains("point-to-register"));
        assert!(help.contains("Store point in a register"));
    }

    #[test]
    fn view_echo_area_messages_opens_read_only_message_history() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x prefix should echo");
        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("unbound key should report message");
        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("messages buffer should open");

        assert_eq!(editor.current_buffer_name(), "*Messages*");
        assert!(editor.document().is_read_only());
        let messages = editor.document().buffer().serialize();
        assert!(messages.contains("C-x- (C-h for help)"));
        assert!(messages.contains("Key is not bound"));
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Type q in messages window to restore previous buffer.")
        );
    }

    #[test]
    fn q_restores_previous_buffer_from_messages_window() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("text should insert");
        editor.cursor = Position::new(0, 2);
        editor
            .execute_command_by_name("view-echo-area-messages")
            .expect("messages buffer should open");
        assert_eq!(editor.current_buffer_name(), "*Messages*");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(editor.cursor(), Position::new(0, 2));
        assert_eq!(editor.document().buffer().serialize(), "alpha");
        assert_eq!(editor.minibuffer().display_text(), None);
    }

    #[test]
    fn repeated_messages_open_keeps_original_return_buffer() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("text should insert");
        editor.cursor = Position::new(0, 2);
        let original = editor.current_buffer_id();
        editor
            .open_messages_buffer()
            .expect("messages buffer should open");
        assert_eq!(editor.current_buffer_name(), "*Messages*");
        editor
            .open_messages_buffer()
            .expect("messages buffer should refresh while inside messages buffer");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");

        assert_eq!(editor.current_buffer_id(), original);
        assert_eq!(editor.cursor(), Position::new(0, 2));
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn messages_buffer_refreshes_existing_buffer() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("missing-command")
            .expect("unknown command should set message");
        editor
            .execute_command_by_name("view-echo-area-messages")
            .expect("messages buffer should open");
        let first = editor.current_buffer_id();
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("No such command: missing-command")
        );
        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");
        editor
            .execute_command_by_name("another-missing-command")
            .expect("another unknown command should set message");
        editor
            .execute_command_by_name("view-echo-area-messages")
            .expect("messages buffer should reopen");

        assert_eq!(editor.current_buffer_id(), first);
        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("No such command: another-missing-command")
        );
    }

    #[test]
    fn describe_key_opens_help_for_complete_binding() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("describe-key should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("describe-key should read prefix");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Describe key: C-x-")
        );
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("describe-key should finish");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.starts_with("C-x C-f runs the command find-file (found in global-map)."));
        assert!(help.contains("\n\nIt is bound to C-x C-f.\n\n"));
        assert!(help.contains("Open file by path\n\n"));
        assert!(help.contains("Prompt for a file path and open it for editing."));
    }

    #[test]
    fn describe_key_displays_space_key_as_spc() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("describe-key should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("describe-key should read prefix");
        editor
            .handle_key(KeyEvent::Text(" ".to_owned()))
            .expect("describe-key should finish");

        let help = editor.document().buffer().serialize();
        assert!(help.contains("C-x SPC runs the command rectangle-mark-mode"));
        assert!(help.contains("It is bound to C-x SPC."));
    }

    #[test]
    fn describe_key_cancel_clears_pending_sequence() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("describe-key should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("describe-key should read prefix");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel describe-key");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert after cancel");

        assert_eq!(editor.minibuffer().display_text().as_deref(), None);
        assert_eq!(editor.document().buffer().serialize(), "x");
    }

    #[test]
    fn describe_key_briefly_echoes_complete_binding() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("c".to_owned()))
            .expect("describe-key-briefly should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("describe-key-briefly should read prefix");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Describe key briefly: C-x-")
        );
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("describe-key-briefly should finish");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("C-x C-f runs the command `find-file`.")
        );
    }

    #[test]
    fn describe_key_briefly_echoes_unbound_key() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("c".to_owned()))
            .expect("describe-key-briefly should start");
        editor
            .handle_key(KeyEvent::Ctrl(']'))
            .expect("describe-key-briefly should finish");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("C-] is not bound to any command.")
        );
    }

    #[test]
    fn describe_key_opens_help_for_unbound_key() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("describe-key should start");
        editor
            .handle_key(KeyEvent::Ctrl(']'))
            .expect("describe-key should finish");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        assert_eq!(
            editor.document().buffer().serialize(),
            "C-] is undefined.\n"
        );
    }

    #[test]
    fn suspend_frame_returns_suspend_outcome() {
        let mut editor = Editor::new(Document::scratch());

        let outcome = editor
            .execute_command_by_name("suspend-frame")
            .expect("suspend-frame should execute");

        assert_eq!(outcome, EditorOutcome::Suspend);
    }

    #[test]
    fn describe_key_briefly_cancel_clears_pending_sequence() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("c".to_owned()))
            .expect("describe-key-briefly should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("describe-key-briefly should read prefix");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel describe-key-briefly");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert after cancel");

        assert_eq!(editor.minibuffer().message.as_deref(), None);
        assert_eq!(editor.document().buffer().serialize(), "x");
    }

    #[test]
    fn describe_function_opens_help_for_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("f".to_owned()))
            .expect("describe-function should start");
        editor
            .handle_key(KeyEvent::Text("find-file".to_owned()))
            .expect("describe-function input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-function should submit");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.starts_with("find-file is an interactive command.\n\n"));
        assert!(help.contains("It is bound to C-x C-f.\n\n"));
        assert!(help.contains("Open file by path\n\n"));
        assert!(help.contains("Prompt for a file path and open it for editing."));
    }

    #[test]
    fn describe_function_completion_selects_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("f".to_owned()))
            .expect("describe-function should start");
        editor
            .handle_key(KeyEvent::Text("find-f".to_owned()))
            .expect("describe-function input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-function should accept selected completion");

        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .starts_with("find-file is an interactive command.")
        );
    }

    #[test]
    fn describe_function_completion_explicit_selection_overrides_exact_name() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("f".to_owned()))
            .expect("describe-function should start");
        editor
            .handle_key(KeyEvent::Text("find-file".to_owned()))
            .expect("exact command name should update input");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next command");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-function should accept explicit selection");

        let help = editor.document().buffer().serialize();
        assert!(help.contains("find-file-read-only is an interactive command."));
        assert!(help.contains("Open file read-only by path"));
    }

    #[test]
    fn describe_function_empty_input_accepts_selected_completion() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("f".to_owned()))
            .expect("describe-function should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next command");
        let selected = editor
            .completion()
            .and_then(|completion| completion.selected())
            .map(|candidate| candidate.value.clone())
            .expect("describe-function should have a selected command");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-function should accept empty selected completion");

        let help = editor.document().buffer().serialize();
        assert!(help.starts_with(&format!("{selected} is an interactive command.")));
        assert_ne!(
            editor.minibuffer().message.as_deref(),
            Some("No such command: ")
        );
    }

    #[test]
    fn describe_function_tab_inserts_selected_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("f".to_owned()))
            .expect("describe-function should start");
        editor
            .handle_key(KeyEvent::Text("find-file".to_owned()))
            .expect("command input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next command");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should insert selected command");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some("find-file-read-only")
        );
    }

    #[test]
    fn describe_mode_opens_help_for_active_modes() {
        let config = Config {
            line_numbers: true,
            ..Config::default()
        };
        let mut editor = Editor::with_config(Document::scratch(), config);

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("m".to_owned()))
            .expect("describe-mode should open help");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("Active Modes:"));
        assert!(help.contains("Major mode: fundamental-mode"));
        assert!(help.contains("Syntax mode: plain-text-syntax-mode"));
        assert!(help.contains(
            "Minor modes: line-number-mode, syntax-highlight-mode, search-highlight-mode"
        ));
        assert!(help.contains("Special buffer mode: none"));
        assert!(help.contains("fundamental-mode:"));
    }

    #[test]
    fn describe_mode_help_formats_special_buffer_mode() {
        let registry = ModeRegistry::default();
        let modes = ActiveModes {
            major: ModeId::Fundamental,
            syntax: ModeId::PlainTextSyntax,
            special: Some(ModeId::Help),
            minor: vec![ModeId::SyntaxHighlighting],
        };

        let help = format_describe_mode_help(&modes, &registry);

        assert!(help.contains("Special buffer mode: help-mode"));
        assert!(help.contains("help-mode:"));
        assert!(help.contains("Keymap: help-mode-map"));
    }

    #[test]
    fn describe_buffer_opens_help_for_current_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("abc".to_owned()))
            .expect("text should insert");
        editor
            .execute_command_by_name("describe-buffer")
            .expect("describe-buffer should open help");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("*scratch* is the current buffer."));
        assert!(help.contains("Name: *scratch*"));
        assert!(help.contains("Kind: normal"));
        assert!(help.contains("Modified: yes"));
        assert!(help.contains("Read only: no"));
        assert!(help.contains("Point: line 1, column 3"));
        assert!(help.contains("Encoding: UTF-8"));
        assert!(help.contains("Line ending: LF"));
        assert!(help.contains("Final newline: no"));
        assert!(help.contains("Major mode: fundamental-mode"));
    }

    #[test]
    fn describe_buffer_help_formats_typed_state() {
        let registry = ModeRegistry::default();
        let description = BufferDescription {
            name: "example.rs".to_owned(),
            path: Some("/tmp/example.rs".to_owned()),
            kind: "normal",
            modified: false,
            read_only: true,
            point_line: 2,
            point_column: 4,
            encoding: "UTF-8",
            line_ending: "LF",
            final_newline: true,
            modes: ActiveModes {
                major: ModeId::Rust,
                syntax: ModeId::RustSyntax,
                special: None,
                minor: vec![ModeId::SyntaxHighlighting],
            },
        };

        let help = format_describe_buffer_help(&description, &registry);

        assert!(help.contains("example.rs is the current buffer."));
        assert!(help.contains("Path: /tmp/example.rs"));
        assert!(help.contains("Read only: yes"));
        assert!(help.contains("Point: line 2, column 4"));
        assert!(help.contains("Major mode: rust-mode"));
        assert!(help.contains("Syntax mode: rust-syntax-mode"));
    }

    #[test]
    fn describe_variable_opens_help_for_option() {
        let config = Config {
            tab_width: 2,
            ..Config::default()
        };
        let mut editor = Editor::with_config(Document::scratch(), config);

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("v".to_owned()))
            .expect("describe-variable should start");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Describe variable: ")
        );
        editor
            .handle_key(KeyEvent::Text("tab_width".to_owned()))
            .expect("describe-variable input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-variable should submit");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("tab_width is a configuration variable."));
        assert!(help.contains("Name: tab_width"));
        assert!(help.contains("Config key: tab_width"));
        assert!(help.contains("Current value: 2"));
        assert!(help.contains("Default value: 4"));
        assert!(help.contains("Type: integer"));
        assert!(help.contains("Valid values: integer from 1 through 16"));
        assert!(help.contains("Display width used for tab characters."));
    }

    #[test]
    fn describe_variable_completion_selects_option() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("v".to_owned()))
            .expect("describe-variable should start");
        editor
            .handle_key(KeyEvent::Text("completion_mat".to_owned()))
            .expect("describe-variable input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-variable should accept selected completion");

        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("completion_matching is a configuration variable.")
        );
    }

    #[test]
    fn describe_variable_completion_accepts_explicit_selection() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("v".to_owned()))
            .expect("describe-variable should start");
        editor
            .handle_key(KeyEvent::Text("completion".to_owned()))
            .expect("describe-variable input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next option");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-variable should accept explicit selection");

        let help = editor.document().buffer().serialize();
        assert!(help.contains("completion_matching is a configuration variable."));
        assert!(help.contains("Name: completion_matching"));
    }

    #[test]
    fn describe_variable_empty_input_accepts_selected_completion() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("v".to_owned()))
            .expect("describe-variable should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next option");
        let selected = editor
            .completion()
            .and_then(|completion| completion.selected())
            .map(|candidate| candidate.value.clone())
            .expect("describe-variable should have a selected option");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("describe-variable should accept empty selected completion");

        let help = editor.document().buffer().serialize();
        assert!(help.contains(&format!("{selected} is a configuration variable.")));
        assert_ne!(
            editor.minibuffer().message.as_deref(),
            Some("No such variable: ")
        );
    }

    #[test]
    fn describe_variable_tab_inserts_selected_option() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("v".to_owned()))
            .expect("describe-variable should start");
        editor
            .handle_key(KeyEvent::Text("completion".to_owned()))
            .expect("describe-variable input should update");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::ArrowDown))
            .expect("down should select next option");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should insert selected option");

        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some("completion_matching")
        );
    }

    #[test]
    fn describe_variable_help_formats_option_metadata() {
        let registry = OptionRegistry::default();
        let option = registry
            .get("completion_matching")
            .expect("completion_matching option should exist");

        let help = format_describe_variable_help(option, OptionValue::Choice("substring"));

        assert!(help.contains("completion_matching is a configuration variable."));
        assert!(help.contains("Current value: substring"));
        assert!(help.contains("Default value: orderless"));
        assert!(help.contains("Valid values: orderless, prefix, or substring"));
    }

    #[test]
    fn editor_reports_current_option_values() {
        let config = Config {
            completion: CompletionConfig {
                matching: CompletionMatching::Substring,
                ..CompletionConfig::default()
            },
            ..Config::default()
        };
        let editor = Editor::with_config(Document::scratch(), config);

        assert_eq!(
            editor.option_value(OptionId::CompletionMatching),
            OptionValue::Choice("substring")
        );
    }

    #[test]
    fn describe_bindings_shows_global_keymap() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("describe-bindings should open help");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        let help = editor.document().buffer().serialize();
        assert!(help.contains("Active Key Bindings:"));
        assert!(help.contains("Keymap Stack:\n- global-map"));
        assert!(help.contains("global-map:"));
        assert!(help.contains("C-h b"));
        assert!(help.contains("describe-bindings"));
        assert!(help.contains("C-x C-f"));
        assert!(help.contains("find-file"));
    }

    #[test]
    fn describe_bindings_shows_special_buffer_local_map() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        assert_eq!(editor.current_buffer_name(), "*Help*");

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start in help buffer");
        editor
            .handle_key(KeyEvent::Text("b".to_owned()))
            .expect("describe-bindings should open help");

        let help = editor.document().buffer().serialize();
        assert!(help.contains("Keymap Stack:\n- help-mode-map\n- global-map"));
        assert!(help.contains("help-mode-map:"));
        assert!(help.contains("q               quit-help-window"));
        assert!(help.contains("global-map:"));
        assert!(help.contains("C-h b"));
        assert!(help.contains("describe-bindings"));
    }

    #[test]
    fn describe_bindings_marks_shadowed_bindings() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new(
                [KeyEvent::Text("q".to_owned())],
                Command::OtherWindow,
            )],
        );
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [KeyBinding::new(
                [KeyEvent::Text("q".to_owned())],
                Command::ForwardChar,
            )],
        );
        let stack = KeyMapStack::new([&special, &global]);

        let help = format_describe_bindings_help(&CommandRegistry::default(), &stack);

        assert!(help.contains("special-buffer-map:"));
        assert!(help.contains("q               other-window"));
        assert!(help.contains("global-map:"));
        assert!(help.contains("q               forward-char"));
        assert!(help.contains("(shadowed by special-buffer-map other-window)"));
    }

    #[test]
    fn describe_bindings_marks_bindings_shadowed_by_prefix() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new(
                [KeyEvent::Ctrl('f'), KeyEvent::Text("q".to_owned())],
                Command::OtherWindow,
            )],
        );
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [KeyBinding::new([KeyEvent::Ctrl('f')], Command::ForwardChar)],
        );
        let stack = KeyMapStack::new([&special, &global]);

        let help = format_describe_bindings_help(&CommandRegistry::default(), &stack);

        assert!(help.contains("C-f q"));
        assert!(help.contains("other-window"));
        assert!(help.contains("C-f             forward-char"));
        assert!(help.contains("(shadowed by higher-priority prefix)"));
    }

    #[test]
    fn describe_key_reports_shadowed_lower_priority_bindings() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new(
                [KeyEvent::Text("q".to_owned())],
                Command::OtherWindow,
            )],
        );
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [KeyBinding::new(
                [KeyEvent::Text("q".to_owned())],
                Command::ForwardChar,
            )],
        );
        let stack = KeyMapStack::new([&special, &global]);

        let help = format_describe_key_help(
            &CommandRegistry::default(),
            &stack,
            &[KeyEvent::Text("q".to_owned())],
            KeyMapId::SpecialBuffer,
            Command::OtherWindow,
        );

        assert!(help.starts_with("q runs the command other-window (found in special-buffer-map)."));
        assert!(help.contains("Shadowed lower-priority bindings:\n- global-map: forward-char"));
        assert!(help.contains("Select next window\n\n"));
    }

    #[test]
    fn describe_key_handles_missing_command_metadata() {
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [KeyBinding::new([KeyEvent::Ctrl('x')], Command::ForwardChar)],
        );
        let stack = KeyMapStack::global(&global);
        let help = format_describe_key_help(
            &CommandRegistry::new([]),
            &stack,
            &[KeyEvent::Ctrl('x')],
            KeyMapId::Global,
            Command::ForwardChar,
        );

        assert!(help.starts_with("C-x runs the command <unknown> (found in global-map)."));
        assert!(!help.contains("It is not bound to any key."));
    }

    #[test]
    fn q_in_help_restores_previous_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        let original = editor.current_buffer_id();

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        assert_eq!(editor.current_buffer_name(), "*Help*");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");

        assert_eq!(editor.current_buffer_id(), original);
        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert_eq!(editor.minibuffer().display_text(), None);
    }

    #[test]
    fn repeated_help_open_keeps_original_return_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        {
            let viewport = editor.windows.current_mut().viewport_mut();
            viewport.first_visible_line = 1;
            viewport.first_visible_column = 2;
        }
        let original = editor.current_buffer_id();
        editor.open_help_buffer("first help");
        assert_eq!(editor.current_buffer_name(), "*Help*");
        editor.open_help_buffer("second help");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");

        assert_eq!(editor.current_buffer_id(), original);
        assert_eq!(editor.cursor(), Position::new(1, 0));
        let restored_viewport = editor.windows.current().viewport();
        assert_eq!(restored_viewport.first_visible_line, 1);
        assert_eq!(restored_viewport.first_visible_column, 2);
        assert_eq!(editor.document().buffer().serialize(), "one\ntwo\nthree");
    }

    #[test]
    fn describe_key_reports_help_buffer_local_binding() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        assert_eq!(editor.current_buffer_name(), "*Help*");

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start in help buffer");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("describe-key should start");
        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("describe-key should describe local q");

        let help = editor.document().buffer().serialize();
        assert!(help.contains("q runs the command quit-help-window (found in help-mode-map)."));
        assert!(help.contains("It is bound to q."));
    }

    #[test]
    fn describe_key_briefly_reports_help_buffer_local_binding() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Backspace))
            .expect("prefix help should open");
        assert_eq!(editor.current_buffer_name(), "*Help*");

        editor
            .handle_key(KeyEvent::Ctrl('h'))
            .expect("help prefix should start in help buffer");
        editor
            .handle_key(KeyEvent::Text("c".to_owned()))
            .expect("describe-key-briefly should start");
        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("describe-key-briefly should describe local q");

        assert_eq!(editor.current_buffer_name(), "*Help*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("q runs the command `quit-help-window`.")
        );
    }

    #[test]
    fn special_buffers_are_read_only_but_normal_q_inserts() {
        let mut welcome = Editor::new(Document::welcome());

        welcome
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("read-only insert should not error");

        assert!(
            welcome
                .document()
                .buffer()
                .serialize()
                .contains("Welcome to Rile.")
        );
        assert!(!welcome.document().buffer().serialize().starts_with('x'));
        assert_eq!(
            welcome.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *Rile*")
        );

        welcome
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should replace read-only message");
        assert_eq!(welcome.minibuffer().message.as_deref(), Some("Quit"));

        welcome
            .handle_key(KeyEvent::Special(SpecialKey::Escape))
            .expect("Escape should quietly clear message");
        assert_eq!(welcome.minibuffer().message.as_deref(), None);

        welcome
            .handle_key(KeyEvent::Text("y".to_owned()))
            .expect("second read-only insert should not error");
        assert_eq!(
            welcome.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *Rile*")
        );
        welcome
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("movement should clear read-only message");
        assert_eq!(welcome.minibuffer().message.as_deref(), None);
        assert_eq!(welcome.cursor(), Position::new(0, 1));

        let mut normal = Editor::new(Document::scratch());
        normal
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("normal q should insert");

        assert_eq!(normal.document().buffer().serialize(), "q");
    }

    #[test]
    fn find_file_prompt_opens_existing_file() {
        let directory = TestDir::new();
        let path = directory.path().join("open.txt");
        fs::write(&path, "opened").expect("file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should prompt");
        let expected_base = current_dir_prompt_input();
        let expected_prompt = format!("Find file: {expected_base}");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some(expected_prompt.as_str())
        );
        editor
            .minibuffer
            .set_prompt_input(path.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("file prompt should open file");

        assert_eq!(editor.document().buffer().serialize(), "opened");
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("Opened "))
        );
    }

    #[test]
    fn find_file_absolute_path_replaces_default_prompt_base() {
        let directory = TestDir::new();
        let path = directory.path().join("absolute.txt");
        fs::write(&path, "absolute").expect("file should be written");
        let path_text = path.to_string_lossy().into_owned();
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should prompt");
        assert_eq!(
            editor.minibuffer().prompt_input(),
            Some(current_dir_prompt_input().as_str())
        );

        editor
            .handle_key(KeyEvent::Text(path_text.clone()))
            .expect("absolute path should replace prompt base");

        assert_eq!(editor.minibuffer().prompt_input(), Some(path_text.as_str()));
    }

    #[test]
    fn find_file_read_only_prompt_opens_read_only_file() {
        let directory = TestDir::new();
        let path = directory.path().join("readonly.txt");
        fs::write(&path, "locked").expect("file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('r'))
            .expect("read-only find-file should prompt");
        let expected_base = current_dir_prompt_input();
        let expected_prompt = format!("Find file read-only: {expected_base}");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some(expected_prompt.as_str())
        );
        editor
            .minibuffer
            .set_prompt_input(path.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("file prompt should open file read-only");

        assert_eq!(editor.document().buffer().serialize(), "locked");
        assert!(editor.document().is_read_only());
        assert!(editor.document().mode_line().contains("[noeol RO]"));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some(format!("Opened read-only {}", editor.document().display_name()).as_str())
        );

        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("read-only edit should not error");
        assert_eq!(editor.document().buffer().serialize(), "locked");
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("Buffer is read-only:"))
        );

        let written = directory.path().join("written.txt");
        editor
            .write_file(written.to_str().expect("path should be utf-8"))
            .expect("write-file should report read-only error");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: save failed: invalid input: buffer is read-only")
        );
        assert!(!written.exists());
    }

    #[test]
    fn find_file_empty_input_accepts_selected_completion() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let target = directory.path().join("aaa-target.txt");
        fs::write(&start, "start").expect("start file should be written");
        fs::write(&target, "target").expect("target file should be written");
        let document = Document::open(&start).expect("start file should open");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt");
        let expected = target.to_string_lossy();
        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some(expected.as_ref())
        );
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty selected file should open");

        assert_eq!(editor.document().path(), Some(target.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "target");
    }

    #[test]
    fn find_file_read_only_empty_input_accepts_selected_completion() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let target = directory.path().join("aaa-target.txt");
        fs::write(&start, "start").expect("start file should be written");
        fs::write(&target, "target").expect("target file should be written");
        let document = Document::open(&start).expect("start file should open");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("find-file-read-only")
            .expect("find-file-read-only should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty selected file should open read-only");

        assert_eq!(editor.document().path(), Some(target.as_path()));
        assert_eq!(editor.document().buffer().serialize(), "target");
        assert!(editor.document().is_read_only());
    }

    #[test]
    fn find_file_read_only_marks_existing_buffer_read_only() {
        let directory = TestDir::new();
        let path = directory.path().join("shared.txt");
        fs::write(&path, "shared").expect("file should be written");
        let document = Document::open(&path).expect("file should open");
        let mut editor = Editor::new(document);

        assert!(!editor.document().is_read_only());

        editor
            .find_file_read_only(path.to_str().expect("path should be utf-8"))
            .expect("read-only open should reuse buffer");

        assert!(editor.document().is_read_only());
    }

    #[test]
    fn toggle_read_only_flips_normal_buffer_editability() {
        let directory = TestDir::new();
        let path = directory.path().join("toggle.txt");
        fs::write(&path, "toggle").expect("file should be written");
        let document = Document::open(&path).expect("file should open");
        let mut editor = Editor::new(document);

        assert!(!editor.document().is_read_only());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("toggle-read-only should run");

        assert!(editor.document().is_read_only());
        assert!(editor.document().mode_line().contains(" RO]"));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is now read-only")
        );

        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("read-only edit should not error");
        assert_eq!(editor.document().buffer().serialize(), "toggle");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("toggle-read-only should run again");

        assert!(!editor.document().is_read_only());
        assert!(!editor.document().mode_line().contains(" RO]"));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is now writable")
        );

        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("writable edit should insert");
        assert_eq!(editor.document().buffer().serialize(), "xtoggle");
    }

    #[test]
    fn toggle_read_only_does_not_make_special_buffers_writable() {
        let mut editor = Editor::new(Document::welcome());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('q'))
            .expect("toggle-read-only should not error");

        assert!(editor.document().is_read_only());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *Rile*")
        );

        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("special buffer edit should not error");
        assert!(!editor.document().buffer().serialize().starts_with('x'));
    }

    #[test]
    fn find_file_prompt_creates_missing_named_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("missing.txt");
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt");
        editor
            .minibuffer
            .set_prompt_input(path.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("missing file should become buffer");

        assert_eq!(editor.document().path(), Some(path.as_path()));
        assert!(editor.document().missing_on_open());
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn find_file_prompt_meta_ret_reports_empty_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt");
        editor.minibuffer.set_prompt_input("");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Enter))
            .expect("raw empty input should be reported");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: missing file name")
        );
    }

    #[test]
    fn write_file_prompt_saves_to_new_path() {
        let directory = TestDir::new();
        let path = directory.path().join("written.txt");
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("saved".to_owned()))
            .expect("text should insert");
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('w'))
            .expect("write-file should prompt");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Write file: ")
        );
        submit_prompt_text(&mut editor, path.to_str().expect("path should be utf-8"));

        assert_eq!(editor.document().path(), Some(path.as_path()));
        assert!(!editor.document().is_dirty());
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "saved"
        );
        let expected_message = format!("Wrote {}", path.display());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some(expected_message.as_str())
        );
    }

    #[test]
    fn write_file_prompt_reports_empty_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("write-file")
            .expect("write-file should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty input should be reported");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: missing file name")
        );
        assert_eq!(editor.document().path(), None);
    }

    #[test]
    fn insert_file_prompt_inserts_text_at_point_and_undo_removes_it() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let source = directory.path().join("source.txt");
        fs::write(&start, "before\nafter\n").expect("start file should be written");
        fs::write(&source, "inserted\n").expect("source file should be written");
        let document = Document::open(&start).expect("start file should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move to second line");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("i".to_owned()))
            .expect("insert-file should prompt");
        let expected_base = file_prompt_base_input(directory.path());
        let expected_prompt = format!("Insert file: {expected_base}");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some(expected_prompt.as_str())
        );
        editor
            .minibuffer
            .set_prompt_input(source.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("insert-file prompt should submit");

        assert_eq!(
            editor.document().buffer().serialize(),
            "before\ninserted\nafter\n"
        );
        assert!(editor.document().is_dirty());
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.starts_with("Inserted "))
        );

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted file contents");
        assert_eq!(editor.document().buffer().serialize(), "before\nafter\n");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn insert_file_empty_input_accepts_selected_completion() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let source = directory.path().join("aaa-source.txt");
        fs::write(&start, "before\nafter\n").expect("start file should be written");
        fs::write(&source, "inserted\n").expect("source file should be written");
        let document = Document::open(&start).expect("start file should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move to second line");
        editor
            .execute_command_by_name("insert-file")
            .expect("insert-file should prompt");
        let expected = source.to_string_lossy();
        assert_eq!(
            editor
                .completion()
                .and_then(|completion| completion.selected())
                .map(|candidate| candidate.value.as_str()),
            Some(expected.as_ref())
        );
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty selected file should insert");

        assert_eq!(
            editor.document().buffer().serialize(),
            "before\ninserted\nafter\n"
        );
    }

    #[test]
    fn insert_file_prompt_meta_ret_reports_empty_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("insert-file")
            .expect("insert-file should prompt");
        editor.minibuffer.set_prompt_input("");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Enter))
            .expect("raw empty input should be reported");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: missing file name")
        );
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn insert_file_rejects_binary_input_without_modifying_buffer() {
        let directory = TestDir::new();
        let source = directory.path().join("binary.bin");
        fs::write(&source, b"text\0binary").expect("binary file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("safe".to_owned()))
            .expect("text should insert");
        editor
            .insert_file(source.to_str().expect("path should be utf-8"))
            .expect("binary input should be reported");

        assert_eq!(editor.document().buffer().serialize(), "safe");
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.contains("appears to be a binary file"))
        );
    }

    #[test]
    fn insert_file_rejects_invalid_utf8_without_modifying_buffer() {
        let directory = TestDir::new();
        let source = directory.path().join("invalid.txt");
        fs::write(&source, [0xff, b'a']).expect("invalid file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Text("safe".to_owned()))
            .expect("text should insert");
        editor
            .insert_file(source.to_str().expect("path should be utf-8"))
            .expect("invalid utf-8 should be reported");

        assert_eq!(editor.document().buffer().serialize(), "safe");
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.contains("not valid UTF-8"))
        );
    }

    #[test]
    fn insert_file_does_not_prompt_in_read_only_buffer() {
        let mut document = Document::scratch();
        document.set_read_only(true);
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("i".to_owned()))
            .expect("insert-file should be blocked");

        assert!(editor.minibuffer().prompt().is_none());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn find_file_reuses_existing_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("same.txt");
        fs::write(&path, "same").expect("file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt");
        editor
            .minibuffer
            .set_prompt_input(path.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("file should open");
        let first_id = editor.current_buffer_id();

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt again");
        editor
            .minibuffer
            .set_prompt_input(path.to_string_lossy().into_owned());
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("existing file buffer should be reused");

        assert_eq!(editor.current_buffer_id(), first_id);
        assert_eq!(editor.buffer_count(), 2);
    }

    #[test]
    fn goto_line_accepts_line_and_line_column() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "line 1\nline 2\nline 3 abc")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('g'))
            .expect("goto-line prefix should start");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("M-g- (C-h for help)")
        );
        editor
            .handle_key(KeyEvent::Text("g".to_owned()))
            .expect("goto-line should prompt");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Goto line: ")
        );
        submit_prompt_text(&mut editor, "3:7");

        assert_eq!(editor.cursor(), Position::new(2, 7));
        assert_eq!(editor.minibuffer().display_text(), None);

        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should prompt by name");
        submit_prompt_text(&mut editor, "2");

        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn goto_line_clamps_line_and_column() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "short\nlast")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should prompt");
        submit_prompt_text(&mut editor, "99:99");

        assert_eq!(editor.cursor(), Position::new(1, "last".len()));
    }

    #[test]
    fn goto_line_rejects_invalid_input_without_moving() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        assert_eq!(editor.cursor(), Position::new(1, 0));

        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should prompt");
        submit_prompt_text(&mut editor, "0");

        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: invalid line number")
        );

        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty prompt should submit");

        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: missing line number")
        );
    }

    #[test]
    fn switch_buffer_prompt_changes_current_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "notes").expect("file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .find_file(path.to_str().expect("path should be utf-8"))
            .expect("file should open");
        assert_eq!(editor.current_buffer_name(), "notes.txt");

        editor
            .execute_command_by_name("switch-to-buffer")
            .expect("switch should prompt");
        for character in "*scratch*".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("buffer prompt should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("switch should complete");

        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Switched to buffer *scratch*")
        );
    }

    #[test]
    fn kill_buffer_removes_clean_current_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("killme.txt");
        fs::write(&path, "kill me").expect("file should be written");
        let mut editor = Editor::new(Document::scratch());

        editor
            .find_file(path.to_str().expect("path should be utf-8"))
            .expect("file should open");
        assert_eq!(editor.buffer_count(), 2);

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("default kill should complete");

        assert_eq!(editor.buffer_count(), 1);
        assert_eq!(editor.current_buffer_name(), "*scratch*");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed buffer killme.txt")
        );
    }

    #[test]
    fn kill_buffer_prompts_before_killing_dirty_current_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");

        assert_eq!(editor.buffer_count(), 1);
        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::KillDirtyBuffer)
        );
        assert!(editor.completion.is_none());
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Buffer *scratch* modified; kill anyway? (y or n) ")
        );
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should not complete confirmation prompt");
        assert_eq!(editor.minibuffer().prompt_input(), Some(""));

        editor
            .handle_key(KeyEvent::Text("y".to_owned()))
            .expect("confirmation should kill immediately");

        assert_eq!(editor.buffer_count(), 1);
        assert!(!editor.document().is_dirty());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer *scratch* modified; kill anyway? (y or n) y")
        );
    }

    #[test]
    fn kill_buffer_dirty_current_cancel_keeps_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("cancellation should happen immediately");

        assert_eq!(editor.buffer_count(), 1);
        assert!(editor.document().is_dirty());
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn kill_buffer_dirty_current_c_g_cancels() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel dirty kill");

        assert_eq!(editor.buffer_count(), 1);
        assert!(editor.document().is_dirty());
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn kill_buffer_dirty_current_empty_answer_cancels() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty answer should cancel");

        assert_eq!(editor.buffer_count(), 1);
        assert!(editor.document().is_dirty());
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
    }

    #[test]
    fn kill_buffer_dirty_current_rejects_yes_no_words() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");
        editor
            .handle_key(KeyEvent::Text("yes".to_owned()))
            .expect("yes word should be rejected");

        assert_eq!(editor.buffer_count(), 1);
        assert!(editor.document().is_dirty());
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Buffer *scratch* modified; kill anyway? (y or n) ")
        );

        editor
            .handle_key(KeyEvent::Text("y".to_owned()))
            .expect("single y should confirm");

        assert_eq!(editor.buffer_count(), 1);
        assert!(!editor.document().is_dirty());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer *scratch* modified; kill anyway? (y or n) y")
        );
    }

    #[test]
    fn kill_buffer_completion_confirms_dirty_named_buffer() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let dirty = directory.path().join("dirty-buffer.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(&dirty, "dirty").expect("dirty fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor
            .find_file(dirty.to_str().unwrap())
            .expect("dirty buffer should open");
        editor
            .handle_key(KeyEvent::Text("!".to_owned()))
            .expect("dirty buffer should edit");
        editor
            .switch_to_buffer("start.txt")
            .expect("start buffer should switch");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("k".to_owned()))
            .expect("kill-buffer should start prompt");
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should complete dirty buffer name");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should prompt");

        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Buffer dirty-buffer.txt modified; kill anyway? (y or n) ")
        );
        editor
            .handle_key(KeyEvent::Text("y".to_owned()))
            .expect("confirmation should kill immediately");

        assert!(editor.buffers.find_by_name("dirty-buffer.txt").is_none());
        assert_eq!(editor.current_buffer_name(), "start.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer dirty-buffer.txt modified; kill anyway? (y or n) y")
        );
    }

    #[test]
    fn window_commands_split_cycle_and_restore_per_window_cursor() {
        let mut editor = Editor::new(Document::scratch());
        for text in ["a", "b", "c"] {
            editor
                .handle_key(KeyEvent::Text(text.to_owned()))
                .expect("text should insert");
        }

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("split should execute");
        assert_eq!(editor.window_count(), 2);
        assert_eq!(editor.cursor(), Position::new(0, 3));

        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("current split cursor should move");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("o".to_owned()))
            .expect("other-window should execute");
        assert_eq!(editor.cursor(), Position::new(0, 3));
    }

    #[test]
    fn universal_argument_repeats_movement_and_self_insert() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("C-f should move by argument");
        assert_eq!(editor.cursor(), Position::new(0, 4));

        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("cursor should move to beginning");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("3".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should repeat by argument");

        assert_eq!(editor.document().buffer().serialize(), "xxxabcdef");
        assert_eq!(editor.cursor(), Position::new(0, 3));
    }

    #[test]
    fn repeated_universal_argument_multiplies_by_four() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdefghijklmnopqr")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("first C-u should start argument");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("second C-u should multiply argument");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("C-f should move by multiplied argument");

        assert_eq!(editor.cursor(), Position::new(0, 16));
    }

    #[test]
    fn universal_argument_handles_negative_and_zero_counts() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        for _ in 0..4 {
            editor
                .handle_key(KeyEvent::Ctrl('f'))
                .expect("cursor should move forward");
        }
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("minus should negate argument");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("negative C-f should move backward");
        assert_eq!(editor.cursor(), Position::new(0, 2));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("0".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("zero self-insert should be a no-op");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("0".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Ctrl('d'))
            .expect("zero delete should be a no-op");

        assert_eq!(editor.document().buffer().serialize(), "abcdef");
        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn universal_argument_does_not_leak_after_prompt_cancel() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel prompt");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert once after prompt cancel");

        assert_eq!(editor.document().buffer().serialize(), "x");
        assert_eq!(editor.cursor(), Position::new(0, 1));
    }

    #[test]
    fn keyboard_macro_records_and_replays_raw_keys() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("(".to_owned()))
            .expect("macro should start");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert while recording");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("movement should record");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(")".to_owned()))
            .expect("macro should end");
        assert_eq!(editor.document().buffer().serialize(), "xabc");
        assert_eq!(editor.cursor(), Position::new(0, 2));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("macro should execute");

        assert_eq!(editor.document().buffer().serialize(), "xaxbc");
        assert_eq!(editor.cursor(), Position::new(0, 4));
    }

    #[test]
    fn keyboard_macro_records_prompt_input() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("(".to_owned()))
            .expect("macro should start");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should prompt");
        for text in "forward-char".chars() {
            editor
                .handle_key(KeyEvent::Text(text.to_string()))
                .expect("prompt input should record");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("prompt should submit");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(")".to_owned()))
            .expect("macro should end");
        assert_eq!(editor.cursor(), Position::new(0, 1));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("macro should replay prompt input");

        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn keyboard_macro_replay_propagates_quit() {
        let mut editor = Editor::new(Document::scratch());
        editor.last_keyboard_macro = Some(vec![KeyEvent::Ctrl('x'), KeyEvent::Ctrl('c')]);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        let outcome = editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("macro should execute");

        assert_eq!(outcome, EditorOutcome::Quit);
    }

    #[test]
    fn keyboard_macro_replay_clears_state_after_error() {
        let mut editor = Editor::new(Document::scratch());
        editor.cursor = Position::new(99, 0);
        editor.last_keyboard_macro = Some(vec![KeyEvent::Text("x".to_owned())]);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        assert!(editor.handle_key(KeyEvent::Text("e".to_owned())).is_err());

        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("(".to_owned()))
            .expect("macro should start after replay error");

        assert!(editor.recording_keyboard_macro.is_some());
        assert!(!editor.replaying_keyboard_macro);
    }

    #[test]
    fn keyboard_macro_trims_m_x_macro_control_command() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("(".to_owned()))
            .expect("macro should start");
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("text should insert while recording");
        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should prompt");
        for text in "end-kbd-macro".chars() {
            editor
                .handle_key(KeyEvent::Text(text.to_string()))
                .expect("prompt input should record before trimming");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("macro should end through M-x");
        assert_eq!(editor.document().buffer().serialize(), "x");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("macro should execute");

        assert_eq!(editor.document().buffer().serialize(), "xx");
    }

    #[test]
    fn universal_argument_repeats_keyboard_macro_execution() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("(".to_owned()))
            .expect("macro should start");
        editor
            .handle_key(KeyEvent::Text(">".to_owned()))
            .expect("text should insert while recording");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("next-line should record");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("beginning-of-line should record");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(")".to_owned()))
            .expect("macro should end");

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("e".to_owned()))
            .expect("macro should execute twice");

        assert_eq!(
            editor.document().buffer().serialize(),
            ">one\n>two\n>three\n"
        );
        assert_eq!(editor.cursor(), Position::new(3, 0));
    }

    #[test]
    fn universal_argument_preserves_pending_key_prefixes() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start key prefix");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("C-x 2 should split window");

        assert_eq!(editor.window_count(), 2);
    }

    #[test]
    fn universal_argument_repeats_kill_line_as_one_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("digit should update argument");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("C-k should repeat by argument");
        assert_eq!(editor.document().buffer().serialize(), "two\nthree");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should restore coalesced kill");
        assert_eq!(editor.document().buffer().serialize(), "one\ntwo\nthree");
    }

    #[test]
    fn delete_window_commands_update_window_count() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("split-window-below")
            .expect("split below should work");
        editor
            .execute_command_by_name("split-window-right")
            .expect("split right should work");
        assert_eq!(editor.window_count(), 3);
        assert_eq!(editor.window_layouts(12, 80).len(), 3);

        editor
            .execute_command_by_name("other-window")
            .expect("other window should work");
        assert_eq!(editor.window_count(), 3);

        editor
            .execute_command_by_name("delete-window")
            .expect("delete current should work");
        assert_eq!(editor.window_count(), 2);

        editor
            .execute_command_by_name("delete-other-windows")
            .expect("delete others should work");
        assert_eq!(editor.window_count(), 1);

        editor
            .execute_command_by_name("delete-window")
            .expect("single delete should be harmless");
        assert_eq!(editor.window_count(), 1);
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Only one window")
        );
    }

    #[test]
    fn mark_region_highlights_selected_text() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "éx")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move by grapheme");

        let spans = editor.spans_for_line(0, "éx");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].end_byte, "é".len());
        assert_eq!(spans[0].face, Face::Region);
    }

    #[test]
    fn exchange_point_and_mark_swaps_and_activates_region() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("point and mark should exchange");

        assert_eq!(editor.cursor(), Position::new(0, 2));
        let spans = editor.spans_for_line(0, "abcdef");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start_byte, 2);
        assert_eq!(spans[0].end_byte, 4);
        assert_eq!(spans[0].face, Face::Region);
    }

    #[test]
    fn mark_whole_buffer_sets_point_to_start_and_mark_to_end() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n  beta\nlast line\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move right");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("h".to_owned()))
            .expect("whole buffer should mark");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.active_region_range(),
            Some(TextRange::new(Position::new(0, 0), Position::new(3, 0)))
        );
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Mark set"));

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("point and mark should exchange");

        assert_eq!(editor.cursor(), Position::new(3, 0));
        assert_eq!(
            editor.active_region_range(),
            Some(TextRange::new(Position::new(0, 0), Position::new(3, 0)))
        );
    }

    #[test]
    fn mark_whole_buffer_does_not_modify_read_only_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        document.buffer_mut().mark_clean();
        document.set_read_only(true);
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("read-only buffer can be marked");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.active_region_range(),
            Some(TextRange::new(Position::new(0, 0), Position::new(0, 5)))
        );
        assert!(!editor.document().buffer().is_dirty());
    }

    #[test]
    fn exchange_point_and_mark_reports_missing_mark() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("missing mark should be reported");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No mark set in this buffer")
        );
    }

    #[test]
    fn kill_region_yank_and_undo_are_unicode_safe() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "éx")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move by grapheme");
        editor
            .handle_key(KeyEvent::Ctrl('w'))
            .expect("region should kill");
        assert_eq!(editor.document().buffer().serialize(), "x");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert latest kill");
        assert_eq!(editor.document().buffer().serialize(), "éx");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove yank");
        assert_eq!(editor.document().buffer().serialize(), "x");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore killed text");
        assert_eq!(editor.document().buffer().serialize(), "éx");
    }

    #[test]
    fn copy_region_yanks_without_deleting() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "ab")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Meta('w'))
            .expect("copy should populate kill ring");
        assert_eq!(editor.document().buffer().serialize(), "ab");

        editor
            .handle_key(KeyEvent::Ctrl('e'))
            .expect("cursor should move to end");
        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert copied text");
        assert_eq!(editor.document().buffer().serialize(), "aba");
    }

    #[test]
    fn rectangle_mark_mode_highlights_columns() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(" ".to_owned()))
            .expect("rectangle mark mode should start");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Mark set (rectangle mode)")
        );
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");

        assert_eq!(
            editor.spans_for_line(0, "abcdef"),
            vec![Span::new(1, 3, Face::Region)]
        );
        assert_eq!(
            editor.spans_for_line(1, "123456"),
            vec![Span::new(1, 3, Face::Region)]
        );
        assert_eq!(editor.active_region_range(), None);
    }

    #[test]
    fn kill_region_uses_rectangle_mark_mode_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(" ".to_owned()))
            .expect("rectangle mark mode should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        editor
            .handle_key(KeyEvent::Ctrl('w'))
            .expect("rectangle should kill");

        assert_eq!(
            editor.document().buffer().serialize(),
            "adef\n1456\nuvwxyz\n"
        );
        assert_eq!(editor.cursor(), Position::new(1, 1));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed rectangle")
        );

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
    }

    #[test]
    fn copy_region_and_yank_preserve_rectangle_shape() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text(" ".to_owned()))
            .expect("rectangle mark mode should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        editor
            .handle_key(KeyEvent::Meta('w'))
            .expect("rectangle should copy");

        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Copied rectangle")
        );

        editor
            .handle_key(KeyEvent::Ctrl('e'))
            .expect("cursor should move to line end");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("rectangle should yank");

        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyzbc\n      23"
        );
        assert_eq!(editor.cursor(), Position::new(3, 8));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove yanked rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
    }

    #[test]
    fn c_x_r_copy_and_yank_use_mark_rectangle_without_registers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Meta('w'));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Copied rectangle")
        );

        editor
            .handle_key(KeyEvent::Ctrl('e'))
            .expect("cursor should move to line end");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        send_c_x_r(&mut editor, KeyEvent::Text("y".to_owned()));

        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyzbc\n      23"
        );
        assert_eq!(editor.cursor(), Position::new(3, 8));
    }

    #[test]
    fn c_x_r_kill_delete_clear_and_open_are_undoable_rectangles() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("k".to_owned()));
        assert_eq!(
            editor.document().buffer().serialize(),
            "adef\n1456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Killed rectangle")
        );
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore killed rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("d".to_owned()));
        assert_eq!(
            editor.document().buffer().serialize(),
            "adef\n1456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Deleted rectangle")
        );
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore deleted rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("c".to_owned()));
        assert_eq!(
            editor.document().buffer().serialize(),
            "a  def\n1  456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Cleared rectangle")
        );
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore cleared rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("o".to_owned()));
        assert_eq!(
            editor.document().buffer().serialize(),
            "a  bcdef\n1  23456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Opened rectangle")
        );
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore opened rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
    }

    #[test]
    fn point_register_saves_and_jumps_to_point() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\nbeta\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        send_c_x_r(&mut editor, KeyEvent::Text(" ".to_owned()));
        editor
            .handle_key(KeyEvent::Text("p".to_owned()))
            .expect("register key should save point");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Point saved to register p")
        );

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("cursor should move to line start");
        assert_eq!(editor.cursor(), Position::new(1, 0));
        send_c_x_r(&mut editor, KeyEvent::Text("j".to_owned()));
        editor
            .handle_key(KeyEvent::Text("p".to_owned()))
            .expect("register key should jump");

        assert_eq!(editor.cursor(), Position::new(0, 2));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Jumped to register p")
        );
    }

    #[test]
    fn text_register_copies_inserts_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n------\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('@'))
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        send_c_x_r(&mut editor, KeyEvent::Text("s".to_owned()));
        editor
            .handle_key(KeyEvent::Text("t".to_owned()))
            .expect("register key should copy text");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Copied region to register t")
        );

        editor.cursor = Position::new(1, 0);
        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("t".to_owned()))
            .expect("register key should insert text");

        assert_eq!(editor.document().buffer().serialize(), "abcdef\nbc------\n");
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted register text");
        assert_eq!(editor.document().buffer().serialize(), "abcdef\n------\n");
    }

    #[test]
    fn rectangle_register_copies_inserts_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\n------\n------\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("r".to_owned()));
        editor
            .handle_key(KeyEvent::Text("r".to_owned()))
            .expect("register key should copy rectangle");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Copied rectangle to register r")
        );

        editor.cursor = Position::new(2, 0);
        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("r".to_owned()))
            .expect("register key should insert rectangle");

        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nbc------\n23------\n"
        );
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted register rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\n------\n------\n"
        );
    }

    #[test]
    fn number_register_stores_increments_inserts_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "value:\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('e'))
            .expect("cursor should move to end");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("7".to_owned()))
            .expect("argument should accept digit");
        send_c_x_r(&mut editor, KeyEvent::Text("n".to_owned()));
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("register key should store number");
        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("register key should insert number");

        assert_eq!(editor.document().buffer().serialize(), "value:7\n");

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Text("5".to_owned()))
            .expect("argument should accept digit");
        send_c_x_r(&mut editor, KeyEvent::Text("+".to_owned()));
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("register key should increment number");
        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("register key should insert incremented number");

        assert_eq!(editor.document().buffer().serialize(), "value:712\n");
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted number");
        assert_eq!(editor.document().buffer().serialize(), "value:7\n");
    }

    #[test]
    fn pending_register_can_be_cancelled_and_reports_invalid_key() {
        let mut editor = Editor::new(Document::scratch());

        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Insert register: ")
        );
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("C-g should cancel pending register command");
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("normal text should insert after cancel");
        assert_eq!(editor.document().buffer().serialize(), "x");

        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("invalid register key should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: invalid register key")
        );
    }

    #[test]
    fn insert_register_reports_empty_and_wrong_type_registers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("empty register should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: register is empty")
        );

        send_c_x_r(&mut editor, KeyEvent::Text(" ".to_owned()));
        editor
            .handle_key(KeyEvent::Text("p".to_owned()))
            .expect("register key should save point");
        send_c_x_r(&mut editor, KeyEvent::Text("i".to_owned()));
        editor
            .handle_key(KeyEvent::Text("p".to_owned()))
            .expect("point register should not insert as text");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: register does not contain text")
        );
        assert_eq!(editor.document().buffer().serialize(), "alpha\n");
    }

    #[test]
    fn string_rectangle_replaces_columns_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("t".to_owned()));
        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::StringRectangle)
        );
        editor
            .handle_key(KeyEvent::Text("XX".to_owned()))
            .expect("replacement should enter prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("string rectangle should submit");

        assert_eq!(
            editor.document().buffer().serialize(),
            "aXXdef\n1XX456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("String rectangle replaced")
        );

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore string rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
    }

    #[test]
    fn rectangle_number_lines_inserts_default_numbers_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        send_c_x_r(&mut editor, KeyEvent::Text("N".to_owned()));

        assert_eq!(
            editor.document().buffer().serialize(),
            "a1 bcdef\n12 23456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Numbered rectangle")
        );

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore numbered rectangle");
        assert_eq!(
            editor.document().buffer().serialize(),
            "abcdef\n123456\nuvwxyz\n"
        );
    }

    #[test]
    fn rectangle_number_lines_with_prefix_prompts_for_start_and_format() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        send_c_x_r(&mut editor, KeyEvent::Text("N".to_owned()));
        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::RectangleNumberStart)
        );

        editor
            .handle_key(KeyEvent::Text("7".to_owned()))
            .expect("start should enter prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("start prompt should submit");
        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::RectangleNumberFormat)
        );

        editor
            .handle_key(KeyEvent::Text("%03d:".to_owned()))
            .expect("format should enter prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("format prompt should submit");

        assert_eq!(
            editor.document().buffer().serialize(),
            "a007:bcdef\n1008:23456\nuvwxyz\n"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Numbered rectangle")
        );
    }

    #[test]
    fn rectangle_number_lines_prefix_empty_prompts_use_emacs_defaults() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcdef\n123456\nuvwxyz\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        mark_columns_one_to_three_across_two_lines(&mut editor);
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        send_c_x_r(&mut editor, KeyEvent::Text("N".to_owned()));
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty start should accept default");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty format should accept default");

        assert_eq!(
            editor.document().buffer().serialize(),
            "a1 bcdef\n12 23456\nuvwxyz\n"
        );
    }

    #[test]
    fn rectangle_number_format_supports_width_padding_and_rejects_bad_directives() {
        assert_eq!(
            format_rectangle_number("%03d:", 7).expect("format should work"),
            "007:"
        );
        assert_eq!(
            format_rectangle_number("%%%3d", -7).expect("format should work"),
            "% -7"
        );
        assert!(format_rectangle_number("plain", 7).is_err());
        assert!(format_rectangle_number("%d %d", 7).is_err());
        assert!(format_rectangle_number("%s", 7).is_err());
    }

    #[test]
    fn kill_line_and_undo_restore_text() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc\ndef")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("kill-line should delete to end of line");
        assert_eq!(editor.document().buffer().serialize(), "\ndef");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore line");
        assert_eq!(editor.document().buffer().serialize(), "abc\ndef");
    }

    #[test]
    fn consecutive_kill_lines_coalesce_for_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc\ndef")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("first C-k should kill text");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("second C-k should kill newline");
        assert_eq!(editor.document().buffer().serialize(), "def");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert coalesced line kill");
        assert_eq!(editor.document().buffer().serialize(), "abc\ndef");
    }

    #[test]
    fn consecutive_forward_word_kills_coalesce_for_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two three")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('d'))
            .expect("first M-d should kill word");
        editor
            .handle_key(KeyEvent::Meta('d'))
            .expect("second M-d should kill next word");
        assert_eq!(editor.document().buffer().serialize(), " three");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert coalesced word kill");
        assert_eq!(editor.document().buffer().serialize(), "one two three");
    }

    #[test]
    fn consecutive_backward_word_kills_prepend_for_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two three")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('>'))
            .expect("M-> should move to end");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Backspace))
            .expect("first M-Backspace should kill word backward");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Backspace))
            .expect("second M-Backspace should kill word backward");
        assert_eq!(editor.document().buffer().serialize(), "one ");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert coalesced backward word kill");
        assert_eq!(editor.document().buffer().serialize(), "one two three");
    }

    #[test]
    fn failed_repeat_kill_region_does_not_duplicate_kill_ring() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abc")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("cursor should move");
        editor
            .handle_key(KeyEvent::Ctrl('w'))
            .expect("region should kill");
        editor
            .handle_key(KeyEvent::Ctrl('w'))
            .expect("missing region should be reported");
        assert_eq!(editor.document().buffer().serialize(), "bc");

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should restore only the original region kill");
        assert_eq!(editor.document().buffer().serialize(), "abc");
    }

    #[test]
    fn yank_pop_rotates_previous_kills_after_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("first C-k should kill one");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("movement should break kill coalescing");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("second C-k should kill two");
        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("C-y should yank latest kill");
        assert_eq!(editor.document().buffer().serialize(), "\ntwo\nthree");

        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("M-y should rotate to previous kill");
        assert_eq!(editor.document().buffer().serialize(), "\none\nthree");

        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("M-y should wrap to latest kill");
        assert_eq!(editor.document().buffer().serialize(), "\ntwo\nthree");
    }

    #[test]
    fn yank_pop_skips_rectangle_entries_for_text_yanks() {
        let mut editor = Editor::new(Document::scratch());
        editor.kill_ring = vec![
            KillEntry::Text("old".to_owned()),
            KillEntry::Rectangle(vec!["rr".to_owned()]),
            KillEntry::Text("new".to_owned()),
        ];

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert latest text kill");
        assert_eq!(editor.document().buffer().serialize(), "new");

        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("yank-pop should skip rectangle entry");
        assert_eq!(editor.document().buffer().serialize(), "old");
    }

    #[test]
    fn yank_pop_requires_preceding_yank() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("empty kill ring should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Kill ring is empty")
        );

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("C-k should populate kill ring");
        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("M-y without yank should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Previous command was not a yank")
        );
    }

    #[test]
    fn movement_breaks_yank_pop_chain() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("first C-k should kill one");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("movement should break kill coalescing");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("second C-k should kill two");
        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("C-y should yank latest kill");
        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("movement should break yank-pop chain");
        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("M-y after movement should be reported");

        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Previous command was not a yank")
        );
        assert_eq!(editor.document().buffer().serialize(), "\ntwo");
    }

    #[test]
    fn yank_pop_handles_multiline_entries_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\nfour")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("first C-k should kill text");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("second C-k should coalesce newline");
        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("movement should break kill coalescing");
        editor
            .handle_key(KeyEvent::Ctrl('k'))
            .expect("third C-k should create another kill entry");
        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("C-y should yank latest kill");
        assert_eq!(editor.document().buffer().serialize(), "two\nthree\nfour");

        editor
            .handle_key(KeyEvent::Meta('y'))
            .expect("M-y should rotate to multi-line kill");
        assert_eq!(editor.document().buffer().serialize(), "two\none\n\nfour");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore previous yank text");
        assert_eq!(editor.document().buffer().serialize(), "two\nthree\nfour");
    }

    #[test]
    fn kill_word_and_backward_kill_word_update_kill_ring_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two three")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('d'))
            .expect("M-d should kill word forward");
        assert_eq!(editor.document().buffer().serialize(), " two three");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Meta('>'))
            .expect("M-> should move to end");
        editor
            .handle_key(KeyEvent::MetaSpecial(SpecialKey::Backspace))
            .expect("M-Backspace should kill word backward");
        assert_eq!(editor.document().buffer().serialize(), " two ");
        assert_eq!(editor.cursor(), Position::new(0, " two ".len()));

        editor
            .handle_key(KeyEvent::Ctrl('y'))
            .expect("yank should insert latest kill");
        assert_eq!(editor.document().buffer().serialize(), " two three");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove yank");
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore backward kill");
        assert_eq!(editor.document().buffer().serialize(), " two three");
    }

    #[test]
    fn transpose_chars_swaps_adjacent_graphemes_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcd")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 2);

        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-t should transpose adjacent chars");
        assert_eq!(editor.document().buffer().serialize(), "acbd");
        assert_eq!(editor.cursor(), Position::new(0, 3));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore transposed chars");
        assert_eq!(editor.document().buffer().serialize(), "abcd");
        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn transpose_chars_handles_end_of_line_and_utf8() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "aé界d")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "aé".len());

        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-t should transpose UTF-8 graphemes");
        assert_eq!(editor.document().buffer().serialize(), "a界éd");
        assert_eq!(editor.cursor(), Position::new(0, "a界é".len()));

        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcd")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = editor.document().buffer().end_position();
        let eol_edit = super::transpose_chars_edit("abcd", Position::new(0, "abcd".len()), 1)
            .expect("EOL transpose should be editable");
        assert_eq!(eol_edit.replacement, "dc");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-t at EOL should swap previous two chars");
        assert_eq!(editor.document().buffer().serialize(), "abdc");
        assert_eq!(editor.cursor(), Position::new(0, "abdc".len()));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("prefixed C-t at EOL should report boundary failure");
        assert_eq!(editor.document().buffer().serialize(), "abdc");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Cannot transpose characters")
        );
    }

    #[test]
    fn transpose_chars_honors_positive_and_negative_arguments() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcd")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 2);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-u 2 C-t should drag char forward");
        assert_eq!(editor.document().buffer().serialize(), "acdb");
        assert_eq!(editor.cursor(), Position::new(0, 4));

        editor.cursor = Position::new(0, 3);
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("negative prefix should start");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative prefix sign should be recorded");
        editor
            .handle_key(KeyEvent::Text("1".to_owned()))
            .expect("negative prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-u -1 C-t should drag char backward");
        assert_eq!(editor.document().buffer().serialize(), "adcb");
        assert_eq!(editor.cursor(), Position::new(0, 2));
    }

    #[test]
    fn transpose_chars_reports_zero_argument_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "abcd")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 2);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("0".to_owned()))
            .expect("zero prefix should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("zero C-t should be reported");
        assert_eq!(editor.document().buffer().serialize(), "abcd");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: zero-argument transpose-chars is not supported")
        );

        editor.cursor = Position::new(0, 0);
        editor
            .transpose_chars(None)
            .expect("C-t at BOL should report boundary failure");
        assert_eq!(editor.document().buffer().serialize(), "abcd");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Cannot transpose characters")
        );

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("read-only C-t should not edit");
        assert_eq!(editor.document().buffer().serialize(), "abcd");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn transpose_words_preserves_punctuation_and_undoes() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "FOO, BAR baz")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "FOO".len());

        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("M-t should transpose words");
        assert_eq!(editor.document().buffer().serialize(), "BAR, FOO baz");
        assert_eq!(editor.cursor(), Position::new(0, "BAR, FOO".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore transposed words");
        assert_eq!(editor.document().buffer().serialize(), "FOO, BAR baz");
        assert_eq!(editor.cursor(), Position::new(0, "FOO".len()));
    }

    #[test]
    fn transpose_words_honors_repeat_arguments_and_utf8() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "déjà two three")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "déjà".len());

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("C-u 2 M-t should drag word forward");
        assert_eq!(editor.document().buffer().serialize(), "two three déjà");
        assert_eq!(editor.cursor(), Position::new(0, "two three déjà".len()));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("negative prefix should start");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative prefix sign should be recorded");
        editor
            .handle_key(KeyEvent::Text("1".to_owned()))
            .expect("negative prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("C-u -1 M-t should drag word backward");
        assert_eq!(editor.document().buffer().serialize(), "two déjà three");
        assert_eq!(editor.cursor(), Position::new(0, "two déjà".len()));
    }

    #[test]
    fn transpose_words_reports_boundaries_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "one".len());

        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("M-t should report missing target word");
        assert_eq!(editor.document().buffer().serialize(), "one");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Cannot transpose words")
        );

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("0".to_owned()))
            .expect("zero prefix should be recorded");
        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("zero M-t should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: zero-argument transpose-words is not supported")
        );

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .handle_key(KeyEvent::Meta('t'))
            .expect("read-only M-t should not edit");
        assert_eq!(editor.document().buffer().serialize(), "one");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn transpose_lines_moves_previous_line_and_undoes() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-x C-t should transpose lines");
        assert_eq!(editor.document().buffer().serialize(), "two\none\nthree");
        assert_eq!(editor.cursor(), Position::new(1, "one".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore transposed lines");
        assert_eq!(editor.document().buffer().serialize(), "one\ntwo\nthree");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn transpose_lines_honors_arguments_utf8_and_boundaries() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "å\ntwo\nthree\nfour")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-u 2 C-x C-t should move line forward");
        assert_eq!(
            editor.document().buffer().serialize(),
            "two\nthree\nå\nfour"
        );
        assert_eq!(editor.cursor(), Position::new(2, "å".len()));

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("negative prefix should start");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative prefix sign should be recorded");
        editor
            .handle_key(KeyEvent::Text("1".to_owned()))
            .expect("negative prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-u -1 C-x C-t should move line backward");
        assert_eq!(
            editor.document().buffer().serialize(),
            "three\ntwo\nå\nfour"
        );
        assert_eq!(editor.cursor(), Position::new(0, "three".len()));

        editor.cursor = Position::new(0, 0);
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("first-line transpose should report boundary failure");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Cannot transpose lines")
        );
    }

    #[test]
    fn transpose_lines_preserves_final_newline_at_eof() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = editor.document().buffer().end_position();

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("C-x C-t at EOF should transpose real lines");
        assert_eq!(editor.document().buffer().serialize(), "two\none\n");
        assert_eq!(editor.cursor(), Position::new(1, "one".len()));
    }

    #[test]
    fn transpose_lines_reports_zero_argument_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("0".to_owned()))
            .expect("zero prefix should be recorded");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("zero C-x C-t should be reported");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: zero-argument transpose-lines is not supported")
        );

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('t'))
            .expect("read-only C-x C-t should not edit");
        assert_eq!(editor.document().buffer().serialize(), "one\ntwo");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn case_word_commands_transform_utf8_words_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "déjà_vu mixed")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('u'))
            .expect("M-u should upcase word");
        assert_eq!(editor.document().buffer().serialize(), "DÉJÀ_VU mixed");
        assert_eq!(editor.cursor(), Position::new(0, "DÉJÀ_VU".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore upcase-word");
        assert_eq!(editor.document().buffer().serialize(), "déjà_vu mixed");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Meta('c'))
            .expect("M-c should capitalize word");
        assert_eq!(editor.document().buffer().serialize(), "Déjà_vu mixed");
        assert_eq!(editor.cursor(), Position::new(0, "Déjà_vu".len()));

        editor
            .handle_key(KeyEvent::Meta('l'))
            .expect("M-l should downcase next word");
        assert_eq!(editor.document().buffer().serialize(), "Déjà_vu mixed");
        assert_eq!(editor.cursor(), Position::new(0, "Déjà_vu mixed".len()));
    }

    #[test]
    fn case_word_commands_honor_positive_and_negative_arguments() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one TWO THREE")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('c'))
            .expect("C-u 2 M-c should capitalize two words");
        assert_eq!(editor.document().buffer().serialize(), "One Two THREE");
        assert_eq!(editor.cursor(), Position::new(0, "One Two".len()));

        editor.cursor = editor.document().buffer().end_position();
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("negative prefix should start");
        editor
            .handle_key(KeyEvent::Text("-".to_owned()))
            .expect("negative prefix sign should be recorded");
        editor
            .handle_key(KeyEvent::Text("2".to_owned()))
            .expect("negative prefix digit should be recorded");
        editor
            .handle_key(KeyEvent::Meta('l'))
            .expect("C-u -2 M-l should downcase two words backward");
        assert_eq!(editor.document().buffer().serialize(), "One two three");
        assert_eq!(editor.cursor(), Position::new(0, "One two three".len()));
    }

    #[test]
    fn case_region_commands_preserve_region_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha βeta\nGamma")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("whole buffer should be marked");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-x C-u should upcase region");
        assert_eq!(editor.document().buffer().serialize(), "ALPHA ΒETA\nGAMMA");
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.active_region_range(),
            Some(TextRange::new(
                Position::new(0, 0),
                Position::new(1, "GAMMA".len())
            ))
        );

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('l'))
            .expect("C-x C-l should downcase region");
        assert_eq!(editor.document().buffer().serialize(), "alpha βeta\ngamma");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore downcase-region");
        assert_eq!(editor.document().buffer().serialize(), "ALPHA ΒETA\nGAMMA");
    }

    #[test]
    fn case_region_reports_missing_region_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("upcase-region")
            .expect("missing region should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: no active region")
        );

        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("whole buffer should be marked");
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .execute_command_by_name("upcase-region")
            .expect("read-only region case should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn open_line_inserts_newline_without_moving_point() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha beta\nsecond")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        for _ in 0..5 {
            editor
                .handle_key(KeyEvent::Ctrl('f'))
                .expect("cursor should move forward");
        }

        editor
            .handle_key(KeyEvent::Ctrl('o'))
            .expect("open-line should insert newline");

        assert_eq!(
            editor.document().buffer().serialize(),
            "alpha\n beta\nsecond"
        );
        assert_eq!(editor.cursor(), Position::new(0, "alpha".len()));
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted newline");
        assert_eq!(editor.document().buffer().serialize(), "alpha beta\nsecond");
        assert_eq!(editor.cursor(), Position::new(0, "alpha".len()));
    }

    #[test]
    fn newline_and_indent_inserts_newline_without_carrying_indentation() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "    alpha\n    \n  beta\nplain")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "    alpha".len());

        editor
            .handle_key(KeyEvent::Ctrl('j'))
            .expect("newline-and-indent should insert newline");

        assert_eq!(
            editor.document().buffer().serialize(),
            "    alpha\n\n    \n  beta\nplain"
        );
        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove inserted newline");
        assert_eq!(
            editor.document().buffer().serialize(),
            "    alpha\n    \n  beta\nplain"
        );
        assert_eq!(editor.cursor(), Position::new(0, "    alpha".len()));
    }

    #[test]
    fn join_line_joins_with_trimmed_separator_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha  \n  beta\nlast")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .handle_key(KeyEvent::Meta('^'))
            .expect("join-line should join current line to previous");
        assert_eq!(editor.document().buffer().serialize(), "alpha beta\nlast");
        assert_eq!(editor.cursor(), Position::new(0, "alpha ".len()));
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore joined line");
        assert_eq!(
            editor.document().buffer().serialize(),
            "alpha  \n  beta\nlast"
        );
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn join_line_handles_blank_previous_line_first_line_and_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n\n    gamma\nlast")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(2, 0);

        editor
            .execute_command_by_name("join-line")
            .expect("join-line command should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha\ngamma\nlast");
        assert_eq!(editor.cursor(), Position::new(1, 0));

        editor.cursor = Position::new(0, 0);
        editor
            .execute_command_by_name("join-line")
            .expect("first-line join-line should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha\ngamma\nlast");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Beginning of buffer")
        );

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor.cursor = Position::new(1, 0);
        editor
            .execute_command_by_name("join-line")
            .expect("read-only join-line should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha\ngamma\nlast");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn delete_horizontal_space_removes_spaces_and_tabs_around_point() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha \t  beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha \t".len());

        editor
            .handle_key(KeyEvent::Meta('\\'))
            .expect("delete-horizontal-space should run");
        assert_eq!(editor.document().buffer().serialize(), "alphabeta");
        assert_eq!(editor.cursor(), Position::new(0, "alpha".len()));
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore horizontal space");
        assert_eq!(editor.document().buffer().serialize(), "alpha \t  beta");
        assert_eq!(editor.cursor(), Position::new(0, "alpha \t".len()));
    }

    #[test]
    fn delete_horizontal_space_prefix_deletes_only_before_point_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha \t  beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha \t".len());

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("C-u should start argument");
        editor
            .handle_key(KeyEvent::Meta('\\'))
            .expect("prefixed delete-horizontal-space should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha  beta");
        assert_eq!(editor.cursor(), Position::new(0, "alpha".len()));

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor.cursor = Position::new(0, "alpha  ".len());
        editor
            .execute_command_by_name("delete-horizontal-space")
            .expect("read-only delete-horizontal-space should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha  beta");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn delete_horizontal_space_handles_utf8_neighbors_and_deactivates_region() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "é \t  beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor.cursor = Position::new(0, "é \t".len());

        editor
            .execute_command_by_name("delete-horizontal-space")
            .expect("delete-horizontal-space should run");
        assert_eq!(editor.document().buffer().serialize(), "ébeta");
        assert_eq!(editor.cursor(), Position::new(0, "é".len()));
        assert_eq!(editor.active_region_range(), None);
    }

    #[test]
    fn just_one_space_collapses_horizontal_space_and_undoes() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha \t  beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha \t".len());

        editor
            .execute_command_by_name("just-one-space")
            .expect("just-one-space should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha beta");
        assert_eq!(editor.cursor(), Position::new(0, "alpha ".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore whitespace");
        assert_eq!(editor.document().buffer().serialize(), "alpha \t  beta");
    }

    #[test]
    fn just_one_space_honors_numeric_and_negative_arguments() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n \t\nbeta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 1);

        editor
            .just_one_space(Some(-2))
            .expect("negative just-one-space should collapse newlines");
        assert_eq!(editor.document().buffer().serialize(), "alpha  beta");
        assert_eq!(editor.cursor(), Position::new(0, "alpha  ".len()));

        editor
            .just_one_space(Some(0))
            .expect("zero just-one-space should delete spaces");
        assert_eq!(editor.document().buffer().serialize(), "alphabeta");
    }

    #[test]
    fn just_one_space_respects_read_only_buffers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha   beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha ".len());
        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");

        editor
            .execute_command_by_name("just-one-space")
            .expect("read-only just-one-space should not edit");
        assert_eq!(editor.document().buffer().serialize(), "alpha   beta");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn just_one_space_noops_without_dirtying_clean_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha beta")
            .expect("fixture should insert");
        document.buffer_mut().mark_clean();
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha ".len());

        editor
            .execute_command_by_name("just-one-space")
            .expect("already-normalized just-one-space should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha beta");
        assert!(!editor.document().is_dirty());

        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alphabeta")
            .expect("fixture should insert");
        document.buffer_mut().mark_clean();
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, "alpha".len());
        editor
            .just_one_space(Some(0))
            .expect("zero-space no-op should run");
        assert_eq!(editor.document().buffer().serialize(), "alphabeta");
        assert!(!editor.document().is_dirty());
    }

    #[test]
    fn delete_blank_lines_handles_nonblank_runs_blank_runs_and_undo() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "header\n\n\nbody\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should start prefix");
        editor
            .handle_key(KeyEvent::Ctrl('o'))
            .expect("delete-blank-lines should run after nonblank line");
        assert_eq!(editor.document().buffer().serialize(), "header\nbody\n");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore blank run");
        assert_eq!(editor.document().buffer().serialize(), "header\n\n\nbody\n");
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor.cursor = Position::new(2, 0);
        editor
            .execute_command_by_name("delete-blank-lines")
            .expect("delete-blank-lines should collapse blank run");
        assert_eq!(editor.document().buffer().serialize(), "header\n\nbody\n");
        assert_eq!(editor.cursor(), Position::new(1, 0));
    }

    #[test]
    fn delete_blank_lines_deletes_isolated_blank_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n\nbeta\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .execute_command_by_name("delete-blank-lines")
            .expect("delete-blank-lines should delete isolated blank line");
        assert_eq!(editor.document().buffer().serialize(), "alpha\nbeta\n");
        assert_eq!(editor.cursor(), Position::new(1, 0));

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor.cursor = Position::new(0, 0);
        editor
            .execute_command_by_name("delete-blank-lines")
            .expect("read-only delete-blank-lines should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha\nbeta\n");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn delete_blank_lines_handles_eof_and_all_blank_buffers() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\n\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, 0);

        editor
            .execute_command_by_name("delete-blank-lines")
            .expect("delete-blank-lines should handle EOF blank line");
        assert_eq!(editor.document().buffer().serialize(), "alpha\n");
        assert_eq!(editor.cursor(), Position::new(1, 0));

        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "\n\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("delete-blank-lines")
            .expect("delete-blank-lines should handle all-blank buffer");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn delete_trailing_whitespace_cleans_buffer_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha  \nbeta\t\t\ngamma")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(1, "beta\t\t".len());

        editor
            .execute_command_by_name("delete-trailing-whitespace")
            .expect("delete-trailing-whitespace should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha\nbeta\ngamma");
        assert_eq!(editor.cursor(), Position::new(1, "beta".len()));
        assert!(editor.document().is_dirty());

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore trailing whitespace");
        assert_eq!(
            editor.document().buffer().serialize(),
            "alpha  \nbeta\t\t\ngamma"
        );
        assert_eq!(editor.cursor(), Position::new(1, "beta\t\t".len()));
    }

    #[test]
    fn delete_trailing_whitespace_respects_active_region_bounds() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha  \nbeta\t\ngamma  \n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 0);
        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor.cursor = Position::new(2, 0);

        editor
            .execute_command_by_name("delete-trailing-whitespace")
            .expect("region delete-trailing-whitespace should run");
        assert_eq!(
            editor.document().buffer().serialize(),
            "alpha\nbeta\ngamma  \n"
        );
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert_eq!(editor.active_region_range(), None);
    }

    #[test]
    fn delete_trailing_whitespace_noops_without_dirtying_and_respects_read_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\nbeta")
            .expect("fixture should insert");
        document.buffer_mut().mark_clean();
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("delete-trailing-whitespace")
            .expect("clean delete-trailing-whitespace should run");
        assert_eq!(editor.document().buffer().serialize(), "alpha\nbeta");
        assert!(!editor.document().is_dirty());

        editor
            .execute_command_by_name("toggle-read-only")
            .expect("toggle-read-only should run");
        editor
            .execute_command_by_name("delete-trailing-whitespace")
            .expect("read-only delete-trailing-whitespace should not error");
        assert_eq!(editor.document().buffer().serialize(), "alpha\nbeta");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Buffer is read-only: *scratch*")
        );
    }

    #[test]
    fn undo_groups_normal_typing() {
        let mut editor = Editor::new(Document::scratch());
        for text in ["a", "b", "c"] {
            editor
                .handle_key(KeyEvent::Text(text.to_owned()))
                .expect("text should insert");
        }
        assert_eq!(editor.document().buffer().serialize(), "abc");

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove grouped typing");
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn syntax_highlighting_uses_file_extension_and_can_toggle() {
        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        let mut document = Document::open(&path).expect("missing Rust file should open");
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "fn main()")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        assert_eq!(
            editor.syntax_mode_for_buffer(editor.current_buffer_id()),
            SyntaxMode::Rust
        );
        assert_eq!(
            editor.major_mode_for_buffer(editor.current_buffer_id()),
            MajorMode::Rust
        );
        assert!(editor.syntax_enabled());
        assert!(editor.spans_for_line(0, "fn main()").contains(&Span::new(
            0,
            2,
            Face::SyntaxKeyword
        )));

        editor
            .execute_command_by_name("toggle-syntax-highlighting")
            .expect("toggle should work");
        assert!(!editor.syntax_enabled());
        assert!(editor.spans_for_line(0, "fn main()").is_empty());
        assert_eq!(
            editor.major_mode_for_buffer(editor.current_buffer_id()),
            MajorMode::Rust
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Syntax highlighting disabled")
        );
    }

    #[test]
    fn major_mode_distinguishes_text_from_fundamental() {
        let scratch = Editor::new(Document::scratch());
        assert_eq!(
            scratch.major_mode_for_buffer(scratch.current_buffer_id()),
            MajorMode::Fundamental
        );

        let directory = TestDir::new();
        let text_path = directory.path().join("notes.txt");
        let unknown_path = directory.path().join("notes.unknown");

        let text_editor = Editor::new(Document::open(&text_path).expect("missing text opens"));
        assert_eq!(
            text_editor.major_mode_for_buffer(text_editor.current_buffer_id()),
            MajorMode::Text
        );
        assert_eq!(
            text_editor.syntax_mode_for_buffer(text_editor.current_buffer_id()),
            SyntaxMode::PlainText
        );

        let unknown_editor =
            Editor::new(Document::open(&unknown_path).expect("missing file opens"));
        assert_eq!(
            unknown_editor.major_mode_for_buffer(unknown_editor.current_buffer_id()),
            MajorMode::Fundamental
        );
    }

    #[test]
    fn viewport_scrolls_vertically_to_keep_cursor_visible() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\nfour")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        for _ in 0..3 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
        }
        editor.ensure_current_window_contains_cursor(2, 80, 0);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_line,
            2
        );

        for _ in 0..2 {
            editor
                .handle_key(KeyEvent::Ctrl('p'))
                .expect("cursor should move up");
        }
        editor.ensure_current_window_contains_cursor(2, 80, 0);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_line,
            1
        );
    }

    #[test]
    fn viewport_scrolls_horizontally_to_keep_cursor_visible() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("abcdef".to_owned()))
            .expect("text should insert");
        editor.ensure_current_window_contains_cursor(10, 3, 6);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_column,
            5
        );

        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("cursor should move to beginning");
        editor.ensure_current_window_contains_cursor(10, 3, 0);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_column,
            0
        );
    }

    #[test]
    fn viewport_scrolls_horizontally_like_default_emacs_hscroll() {
        let mut editor = Editor::new(Document::scratch());

        editor.ensure_current_window_contains_cursor(10, 32, 36);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_column,
            20
        );

        editor.ensure_current_window_contains_cursor(10, 32, 56);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_column,
            40
        );

        editor.ensure_current_window_contains_cursor(10, 32, 31);
        assert_eq!(
            editor
                .window_viewport(editor.current_window_id())
                .expect("viewport should exist")
                .first_visible_column,
            15
        );
    }

    #[test]
    fn editor_applies_config_options_and_toggle_commands() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                tab_width: 2,
                fill_column: 72,
                line_numbers: true,
                syntax_highlighting: false,
                search_highlighting: false,
                backup_on_save: true,
                theme: ThemeName::Mono,
                completion: Default::default(),
            },
        );

        assert_eq!(editor.tab_width(), 2);
        assert_eq!(
            editor.option_value(OptionId::FillColumn),
            OptionValue::Integer(72)
        );
        assert!(editor.line_numbers());
        assert!(!editor.syntax_enabled());
        assert!(!editor.search_highlighting());
        assert!(editor.document().backup_on_save());
        assert_eq!(editor.theme(), ThemeName::Mono);

        editor
            .execute_command_by_name("toggle-line-numbers")
            .expect("line toggle should work");
        assert!(!editor.line_numbers());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Line numbers disabled")
        );

        editor
            .execute_command_by_name("toggle-search-highlighting")
            .expect("search toggle should work");
        assert!(editor.search_highlighting());
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Search highlighting enabled")
        );
    }

    #[test]
    fn disabling_search_highlighting_keeps_search_motion() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two")
            .expect("fixture should insert");
        let mut editor = Editor::with_config(
            document,
            Config {
                search_highlighting: false,
                ..Config::default()
            },
        );

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        submit_prompt_text_without_enter(&mut editor, "two");

        assert_eq!(editor.cursor(), Position::new(0, "one ".len()));
        assert!(editor.spans_for_line(0, "one two").is_empty());
    }

    #[test]
    fn syntax_spans_merge_below_region_and_search_priority() {
        let directory = TestDir::new();
        let path = directory.path().join("main.rs");
        let mut document = Document::open(&path).expect("missing Rust file should open");
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "fn fn")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("end-of-line")
            .expect("cursor should move to end");
        editor
            .execute_command_by_name("set-mark-command")
            .expect("mark should set");
        editor
            .handle_key(KeyEvent::Ctrl('a'))
            .expect("cursor should move to beginning");
        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        submit_prompt_text_without_enter(&mut editor, "fn");

        assert_eq!(
            editor.spans_for_line(0, "fn fn"),
            vec![
                Span::new(0, 2, Face::CurrentSearchMatch),
                Span::new(2, 3, Face::Region),
                Span::new(3, 5, Face::SearchMatch),
            ]
        );
    }

    fn submit_prompt_text(editor: &mut Editor, text: &str) {
        for character in text.chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("prompt input should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("prompt should submit");
    }

    fn submit_prompt_text_without_enter(editor: &mut Editor, text: &str) {
        for character in text.chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("prompt input should update");
        }
    }

    #[test]
    fn shell_command_displays_output_buffer_and_returns() {
        let mut editor = Editor::new(Document::scratch());
        let original_buffer = editor.current_buffer_id();

        editor
            .handle_key(KeyEvent::Meta('!'))
            .expect("M-! should prompt");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Shell command: ")
        );
        submit_prompt_text(&mut editor, "printf 'shell-out\\n'");

        assert_eq!(editor.current_buffer_name(), "*Shell Command Output*");
        assert!(editor.document().buffer().serialize().contains("shell-out"));
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Shell command completed (10 bytes)")
        );

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");
        assert_eq!(editor.current_buffer_id(), original_buffer);
    }

    #[test]
    fn repeated_shell_output_open_keeps_original_return_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        editor.cursor = Position::new(0, 2);
        let original = editor.current_buffer_id();

        editor.open_shell_output_buffer("first output");
        assert_eq!(editor.current_buffer_name(), "*Shell Command Output*");
        editor.open_shell_output_buffer("second output");

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("q should restore previous buffer");

        assert_eq!(editor.current_buffer_id(), original);
        assert_eq!(editor.cursor(), Position::new(0, 2));
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn shell_command_with_prefix_inserts_stdout_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Meta('!'))
            .expect("C-u M-! should prompt");
        submit_prompt_text(&mut editor, "printf 'INSERTED'");

        assert_eq!(editor.document().buffer().serialize(), "INSERTEDalpha");
        assert_eq!(editor.cursor(), Position::new(0, "INSERTED".len()));

        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should remove shell insertion");
        assert_eq!(editor.document().buffer().serialize(), "alpha");
    }

    #[test]
    fn shell_command_on_region_with_prefix_replaces_stdout() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "b\na")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("whole buffer should mark");
        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Meta('|'))
            .expect("C-u M-| should prompt");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Shell command on region: ")
        );
        submit_prompt_text(&mut editor, "sort");

        assert_eq!(editor.document().buffer().serialize(), "a\nb\n");
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore region replacement");
        assert_eq!(editor.document().buffer().serialize(), "b\na");
    }

    #[test]
    fn nonzero_prefix_shell_command_does_not_mutate_buffer() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let original_buffer = editor.current_buffer_id();

        editor
            .handle_key(KeyEvent::Ctrl('u'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Meta('!'))
            .expect("C-u M-! should prompt");
        submit_prompt_text(&mut editor, "printf 'changed'; exit 2");

        assert_eq!(editor.current_buffer_name(), "*Shell Command Output*");
        assert_eq!(
            editor
                .document_for_buffer(original_buffer)
                .expect("original buffer should remain")
                .buffer()
                .serialize(),
            "alpha"
        );
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: Shell command failed with code 2")
        );
    }

    #[test]
    fn query_replace_replaces_utf8_and_undo_restores() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "é a é")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Meta('%'))
            .expect("query replace should prompt");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Query replace: ")
        );
        submit_prompt_text(&mut editor, "é");
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Query replace é with: ")
        );
        submit_prompt_text(&mut editor, "e");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Query replacing é with e: (y, n, !, q)?")
        );

        let spans = editor.spans_for_line(0, "é a é");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].end_byte, "é".len());
        assert_eq!(spans[0].face, Face::CurrentSearchMatch);

        editor
            .handle_key(KeyEvent::Text("y".to_owned()))
            .expect("yes should replace current candidate");
        assert_eq!(editor.document().buffer().serialize(), "e a é");
        assert_eq!(editor.cursor(), Position::new(0, "e a ".len()));

        editor
            .handle_key(KeyEvent::Text("q".to_owned()))
            .expect("quit should finish query replace");
        editor
            .handle_key(KeyEvent::Ctrl('_'))
            .expect("undo should restore replacement");
        assert_eq!(editor.document().buffer().serialize(), "é a é");
    }

    #[test]
    fn query_replace_skips_and_replaces_all_remaining() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "foo foo foo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .execute_command_by_name("query-replace")
            .expect("query replace should prompt");
        submit_prompt_text(&mut editor, "foo");
        submit_prompt_text(&mut editor, "bar");
        editor
            .handle_key(KeyEvent::Text("n".to_owned()))
            .expect("no should skip current candidate");
        editor
            .handle_key(KeyEvent::Text("!".to_owned()))
            .expect("bang should replace all remaining candidates");

        assert_eq!(editor.document().buffer().serialize(), "foo bar bar");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Replaced 2 occurrences")
        );
    }

    #[test]
    fn query_replace_reports_empty_and_missing_searches() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("query-replace")
            .expect("query replace should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty query should submit");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: missing search string")
        );

        editor
            .execute_command_by_name("query-replace")
            .expect("query replace should prompt");
        submit_prompt_text(&mut editor, "missing");
        submit_prompt_text(&mut editor, "replacement");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("No matches for missing")
        );
    }

    #[test]
    fn incremental_search_forward_updates_live_with_utf8_highlights() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha\nécho écho")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        editor
            .handle_key(KeyEvent::Text("é".to_owned()))
            .expect("search input should update");

        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("I-search: é")
        );

        let spans = editor.spans_for_line(1, "écho écho");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].start_byte, 0);
        assert_eq!(spans[0].end_byte, "é".len());
        assert_eq!(spans[0].face, Face::CurrentSearchMatch);
        assert_eq!(spans[1].face, Face::SearchMatch);

        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("search accept should keep match");
        assert_eq!(editor.cursor(), Position::new(1, 0));
        assert_eq!(editor.minibuffer().prompt(), None);
    }

    #[test]
    fn incremental_search_prompt_inserts_at_minibuffer_cursor() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha beta")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        editor
            .handle_key(KeyEvent::Text("alpa".to_owned()))
            .expect("search input should update");
        editor
            .handle_key(KeyEvent::Ctrl('b'))
            .expect("C-b should move within search prompt");
        editor
            .handle_key(KeyEvent::Text("h".to_owned()))
            .expect("search input should insert at cursor");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha"));
        assert_eq!(
            editor.minibuffer().prompt_input_before_cursor(),
            Some("alph")
        );
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn incremental_search_prompt_deletion_updates_live_search() {
        assert_incremental_search_prompt_edit_updates_live_search(
            "x alpha",
            &[KeyEvent::Ctrl('a')],
            KeyEvent::Ctrl('d'),
            " alpha",
        );
    }

    #[test]
    fn incremental_search_prompt_kill_commands_update_live_search() {
        assert_incremental_search_prompt_edit_updates_live_search(
            " alphax",
            &[KeyEvent::Ctrl('b')],
            KeyEvent::Ctrl('k'),
            " alpha",
        );
        assert_incremental_search_prompt_edit_updates_live_search(
            "x alpha",
            &[KeyEvent::Ctrl('a')],
            KeyEvent::Meta('d'),
            " alpha",
        );
        assert_incremental_search_prompt_edit_updates_live_search(
            "x alpha",
            &[KeyEvent::Ctrl('a'), KeyEvent::Ctrl('f')],
            KeyEvent::MetaSpecial(SpecialKey::Backspace),
            " alpha",
        );
        assert_incremental_search_prompt_edit_updates_live_search(
            "x alpha",
            &[KeyEvent::Ctrl('a'), KeyEvent::Ctrl('f')],
            KeyEvent::CtrlSpecial(SpecialKey::Backspace),
            " alpha",
        );
    }

    #[test]
    fn incremental_search_repeats_forward_and_backward() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "foo bar foo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        for character in "foo".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("query should update");
        }
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("repeat forward should move");
        assert_eq!(editor.cursor(), Position::new(0, "foo bar ".len()));

        editor
            .handle_key(KeyEvent::Ctrl('r'))
            .expect("repeat backward should move");
        assert_eq!(editor.cursor(), Position::new(0, 0));
    }

    #[test]
    fn incremental_search_wraps_forward_after_boundary_failure() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\none")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        for character in "one".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("query should update");
        }
        assert_eq!(editor.cursor(), Position::new(0, 0));

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("repeat forward should move to next match");
        assert_eq!(editor.cursor(), Position::new(2, 0));

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("repeat forward should report boundary failure");
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Failing I-search: one")
        );

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("repeat forward should wrap to first match");
        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Wrapped I-search: one")
        );
    }

    #[test]
    fn incremental_search_wraps_backward_after_boundary_failure() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\none")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('r'))
            .expect("search should prompt");
        for character in "one".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("query should update");
        }

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Failing I-search backward: one")
        );

        editor
            .handle_key(KeyEvent::Ctrl('r'))
            .expect("repeat backward should wrap to last match");
        assert_eq!(editor.cursor(), Position::new(2, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Wrapped I-search backward: one")
        );
    }

    #[test]
    fn incremental_search_cancel_restores_original_cursor() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one two")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        for character in "two".chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("query should update");
        }
        assert_eq!(editor.cursor(), Position::new(0, "one ".len()));

        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("cancel should restore point");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(editor.minibuffer().message.as_deref(), Some("Quit"));
        assert_eq!(editor.minibuffer().prompt(), None);
    }

    #[test]
    fn incremental_search_reports_failing_query() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("query should update");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Failing I-search: z")
        );
    }

    #[test]
    fn incremental_search_repeated_total_miss_stays_failing() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "alpha")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("search should prompt");
        editor
            .handle_key(KeyEvent::Text("z".to_owned()))
            .expect("query should update");
        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("repeat forward should keep failing");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Failing I-search: z")
        );

        editor
            .handle_key(KeyEvent::Ctrl('r'))
            .expect("repeat backward should keep failing");

        assert_eq!(editor.cursor(), Position::new(0, 0));
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Failing I-search backward: z")
        );
    }
}
