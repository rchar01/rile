// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use crate::buffer::undo::UndoRecord;
use crate::buffer::{BufferId, Position, RectangleEdit, TextRange};
use crate::buffers::BufferManager;
use crate::command::{Command, CommandRegistry};
use crate::completion::{CompletionConfig, CompletionSession, CompletionSource, CompletionStyle};
use crate::config::{Config, ThemeName};
use crate::file::{Document, DocumentKind};
use crate::input::{KeyEvent, SpecialKey};
use crate::keymap::{KeyMap, KeyResolution};
use crate::minibuffer::{MinibufferState, PromptKind};
use crate::render::{DecorationProvider, Face, Span, collect_spans_for_line};
use crate::syntax::{Highlighter, MajorMode, SyntaxHighlighter, SyntaxMode};
use crate::window::{SplitAxis, Viewport, WindowId, WindowLayout, WindowSet};
use crate::{Result, RileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    Continue,
    Quit,
}

#[derive(Debug, Clone)]
pub struct Editor {
    buffers: BufferManager,
    windows: WindowSet,
    current_buffer: BufferId,
    cursor: Position,
    goal_display_column: Option<usize>,
    key_sequence: Vec<KeyEvent>,
    current_command_sequence: Option<Vec<KeyEvent>>,
    keyboard_macro_prompt_start: Option<usize>,
    keymap: KeyMap,
    commands: CommandRegistry,
    minibuffer: MinibufferState,
    help_return: Option<Viewport>,
    describe_key: Option<Vec<KeyEvent>>,
    completion: Option<CompletionSession>,
    completion_return: Option<Viewport>,
    completion_config: CompletionConfig,
    prompt_histories: Vec<PromptHistory>,
    recording_keyboard_macro: Option<Vec<KeyEvent>>,
    last_keyboard_macro: Option<Vec<KeyEvent>>,
    replaying_keyboard_macro: bool,
    universal_argument: Option<UniversalArgumentState>,
    search: Option<SearchState>,
    query_replace: Option<QueryReplaceState>,
    quoted_insert: bool,
    region: Option<RegionState>,
    kill_ring: Vec<KillEntry>,
    yank_state: Option<YankState>,
    last_command_was_kill: bool,
    kill_recorded_this_command: bool,
    undo_stack: Vec<UndoEntry>,
    grouping_insert: bool,
    syntax_enabled: bool,
    search_highlighting: bool,
    line_numbers: bool,
    tab_width: usize,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionShape {
    Linear,
    Rectangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RectangleBounds {
    start_line: usize,
    end_line: usize,
    start_column: usize,
    end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KillEntry {
    Text(String),
    Rectangle(Vec<String>),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryReplaceState {
    query: String,
    replacement: String,
    current: Option<TextRange>,
    replacements: usize,
    visited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptHistory {
    kind: PromptKind,
    entries: Vec<String>,
    position: Option<usize>,
    draft: String,
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
        Self {
            windows: WindowSet::new(current_buffer),
            buffers,
            current_buffer,
            cursor: Position::new(0, 0),
            goal_display_column: None,
            key_sequence: Vec::new(),
            current_command_sequence: None,
            keyboard_macro_prompt_start: None,
            keymap: KeyMap::default(),
            commands: CommandRegistry::default(),
            minibuffer: MinibufferState::default(),
            help_return: None,
            describe_key: None,
            completion: None,
            completion_return: None,
            completion_config: config.completion,
            prompt_histories: Vec::new(),
            recording_keyboard_macro: None,
            last_keyboard_macro: None,
            replaying_keyboard_macro: false,
            universal_argument: None,
            search: None,
            query_replace: None,
            quoted_insert: false,
            region: None,
            kill_ring: Vec::new(),
            yank_state: None,
            last_command_was_kill: false,
            kill_recorded_this_command: false,
            undo_stack: Vec::new(),
            grouping_insert: false,
            syntax_enabled: config.syntax_highlighting,
            search_highlighting: config.search_highlighting,
            line_numbers: config.line_numbers,
            tab_width: config.tab_width,
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
            if cursor_display_column < viewport.first_visible_column {
                viewport.first_visible_column = cursor_display_column;
            } else if cursor_display_column >= viewport.first_visible_column + text_columns {
                viewport.first_visible_column = cursor_display_column + 1 - text_columns;
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

    pub fn minibuffer_display_text(&self) -> Option<String> {
        let Some(completion) = &self.completion else {
            return self.minibuffer.display_text();
        };
        if completion.style() != CompletionStyle::Ido {
            return self.minibuffer.display_text();
        }
        let prompt = self.minibuffer.prompt()?;
        if !matches!(
            prompt.kind,
            PromptKind::DescribeFunction
                | PromptKind::ExtendedCommand
                | PromptKind::FindFile
                | PromptKind::FindFileReadOnly
                | PromptKind::InsertFile
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

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        self.record_keyboard_macro_key(&key);

        if self.minibuffer.prompt().is_some() {
            return self.handle_prompt_key(key);
        }

        self.clear_transient_message();

        if self.query_replace.is_some() {
            return self.handle_query_replace_key(key);
        }

        if self.quoted_insert {
            return self.handle_quoted_insert_key(key);
        }

        if key == KeyEvent::Ctrl('g') {
            self.quit_current_operation();
            return Ok(EditorOutcome::Continue);
        }

        if self.describe_key.is_some() {
            return Ok(self.handle_describe_key(key));
        }

        if self.document().is_help() && key == KeyEvent::Text("q".to_owned()) {
            return Ok(self.restore_help_buffer());
        }

        if self.document().is_buffer_list() && key == KeyEvent::Text("q".to_owned()) {
            return Ok(self.close_buffer_list_window());
        }

        if !self.key_sequence.is_empty() {
            return self.handle_bound_key(key);
        }

        if self.universal_argument.is_some() && self.handle_universal_argument_key(&key) {
            return Ok(EditorOutcome::Continue);
        }

        match key {
            KeyEvent::Special(SpecialKey::Escape) => {
                self.clear_key_sequence();
                self.universal_argument = None;
                self.clear_insert_group();
                self.last_command_was_kill = false;
                self.yank_state = None;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                self.clear_key_sequence();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument(&text, true, argument)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Enter) => {
                self.clear_key_sequence();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument("\n", false, argument)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => {
                self.clear_key_sequence();
                self.last_command_was_kill = false;
                self.yank_state = None;
                let argument = self.take_universal_argument();
                self.insert_text_with_argument("\t", false, argument)?;
                Ok(EditorOutcome::Continue)
            }
            key => self.handle_bound_key(key),
        }
    }

    pub fn execute_command_by_name(&mut self, name: &str) -> Result<EditorOutcome> {
        let Some(command) = self.commands.get(name) else {
            self.minibuffer
                .set_message(format!("No such command: {name}"));
            return Ok(EditorOutcome::Continue);
        };

        if command.command == Command::UniversalArgument {
            self.extend_universal_argument()?;
            return Ok(EditorOutcome::Continue);
        }

        let argument = self.take_universal_argument();
        self.execute_command(command.command, argument)
    }

    fn handle_bound_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if !self.key_sequence.is_empty() && is_key_prefix_help(&key) {
            return Ok(self.show_key_prefix_help());
        }

        self.key_sequence.push(key);

        match self.keymap.resolve(&self.key_sequence) {
            KeyResolution::NoMatch => {
                self.clear_key_sequence();
                self.universal_argument = None;
                self.last_command_was_kill = false;
                self.yank_state = None;
                self.minibuffer.set_message("Key is not bound");
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Prefix => {
                self.minibuffer
                    .set_message(format_key_prefix_message(&self.key_sequence));
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Command(name) => {
                let command_sequence = self.key_sequence.clone();
                self.clear_key_sequence();
                self.current_command_sequence = Some(command_sequence);
                let result = self.execute_command_by_name(name);
                self.current_command_sequence = None;
                result
            }
        }
    }

    fn show_key_prefix_help(&mut self) -> EditorOutcome {
        let prefix = self.key_sequence.clone();
        let text = format_key_prefix_help(&self.keymap, &prefix);
        self.clear_key_sequence();
        self.open_help_buffer(text)
    }

    fn open_help_buffer(&mut self, text: impl AsRef<str>) -> EditorOutcome {
        self.sync_current_window();
        if !self.document().is_help() || self.help_return.is_none() {
            self.help_return = Some(*self.windows.current().viewport());
        }
        let help = self.buffers.open_help(text);

        self.current_buffer = help;
        self.cursor = Position::new(0, 0);
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.first_visible_line = 0;
        viewport.first_visible_column = 0;
        self.sync_current_window();
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

        self.current_buffer = viewport.buffer;
        self.cursor = viewport.cursor;
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        *self.windows.current_mut().viewport_mut() = viewport;
        self.minibuffer.clear();

        EditorOutcome::Continue
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

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if self.minibuffer.prompt_kind() == Some(PromptKind::IncrementalSearch) {
            return self.handle_search_prompt_key(key);
        }
        if self.completion.is_some() {
            return self.handle_completion_prompt_key(key);
        }

        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                self.record_prompt_history(kind, &input);
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
                self.minibuffer.cancel_prompt();
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                self.minibuffer.delete_prompt_grapheme_backward();
                self.reset_current_prompt_history_navigation();
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
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => Ok(EditorOutcome::Continue),
            _ => Ok(EditorOutcome::Continue),
        }
    }

    fn handle_completion_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                if self.completion_should_enter_selected_directory(&input) {
                    self.reset_prompt_history_navigation(kind);
                    self.minibuffer.start_prompt(kind, prompt_label(kind));
                    let directory = self
                        .completion
                        .as_ref()
                        .and_then(CompletionSession::selected)
                        .map(|candidate| candidate.value.clone())
                        .unwrap_or(input);
                    self.minibuffer.set_prompt_input(directory);
                    self.update_completion_from_prompt();
                    return Ok(EditorOutcome::Continue);
                }
                let input = self.completion_accept_input(&input);
                self.record_prompt_history(kind, &input);
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
                self.minibuffer.delete_prompt_grapheme_backward();
                self.reset_current_prompt_history_navigation();
                self.update_completion_from_prompt();
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
            KeyEvent::Text(text) => {
                self.minibuffer.insert_prompt_text(&text);
                self.reset_current_prompt_history_navigation();
                self.update_completion_from_prompt();
                Ok(EditorOutcome::Continue)
            }
            _ => Ok(EditorOutcome::Continue),
        }
    }

    fn completion_accept_input(&self, input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return input.to_owned();
        }
        match self.completion.as_ref().map(CompletionSession::source) {
            Some(CompletionSource::Commands) if self.commands.contains(trimmed) => {
                return trimmed.to_owned();
            }
            Some(CompletionSource::Files) => return self.file_completion_accept_input(input),
            Some(CompletionSource::Buffers) => return self.buffer_completion_accept_input(input),
            Some(_) => {}
            None => {}
        }
        self.completion
            .as_ref()
            .and_then(CompletionSession::selected)
            .map(|candidate| candidate.value.clone())
            .unwrap_or_else(|| input.to_owned())
    }

    fn file_completion_accept_input(&self, input: &str) -> String {
        let trimmed = input.trim();
        if self.find_file_input_is_exact_file(trimmed) {
            return trimmed.to_owned();
        }
        let Some(completion) = self.completion.as_ref() else {
            return input.to_owned();
        };
        if completion.selection_explicit() {
            return completion
                .selected()
                .map(|candidate| candidate.value.clone())
                .unwrap_or_else(|| input.to_owned());
        }
        input.to_owned()
    }

    fn buffer_completion_accept_input(&self, input: &str) -> String {
        if self.buffers.find_by_name(input).is_some() {
            return input.to_owned();
        }
        let Some(completion) = self.completion.as_ref() else {
            return input.to_owned();
        };
        if completion.selection_explicit() {
            return completion
                .selected()
                .map(|candidate| candidate.value.clone())
                .unwrap_or_else(|| input.to_owned());
        }
        input.to_owned()
    }

    fn completion_should_enter_selected_directory(&self, input: &str) -> bool {
        if input.trim().is_empty() {
            return false;
        }
        let Some(completion) = self
            .completion
            .as_ref()
            .filter(|completion| completion.source() == CompletionSource::Files)
        else {
            return false;
        };
        let Some(candidate) = completion
            .selected()
            .filter(|candidate| candidate.is_directory())
        else {
            return false;
        };
        completion.selection_explicit()
            || candidate.value.trim_end_matches('/') == input.trim().trim_end_matches('/')
    }

    fn record_prompt_history(&mut self, kind: PromptKind, input: &str) {
        if !prompt_kind_uses_history(kind) || input.trim().is_empty() {
            self.reset_prompt_history_navigation(kind);
            return;
        }
        let index = self.prompt_history_index(kind);
        let history = &mut self.prompt_histories[index];
        if history.entries.last().is_none_or(|entry| entry != input) {
            history.entries.push(input.to_owned());
        }
        history.position = None;
        history.draft.clear();
    }

    fn recall_prompt_history(&mut self, direction: isize) {
        let Some(kind) = self.minibuffer.prompt_kind() else {
            return;
        };
        if !prompt_kind_uses_history(kind) {
            return;
        }
        let current = self
            .minibuffer
            .prompt_input()
            .unwrap_or_default()
            .to_owned();
        let index = self.prompt_history_index(kind);
        let history = &mut self.prompt_histories[index];
        if history.entries.is_empty() {
            return;
        }

        let next_position = match (history.position, direction.signum()) {
            (None, -1) => {
                history.draft = current;
                Some(history.entries.len() - 1)
            }
            (Some(position), -1) => Some(position.saturating_sub(1)),
            (Some(position), 1) if position + 1 < history.entries.len() => Some(position + 1),
            (Some(_), 1) => None,
            _ => return,
        };

        history.position = next_position;
        let input = next_position
            .map(|position| history.entries[position].clone())
            .unwrap_or_else(|| history.draft.clone());
        self.minibuffer.set_prompt_input(input);
    }

    fn reset_current_prompt_history_navigation(&mut self) {
        if let Some(kind) = self.minibuffer.prompt_kind() {
            self.reset_prompt_history_navigation(kind);
        }
    }

    fn reset_prompt_history_navigation(&mut self, kind: PromptKind) {
        if let Some(history) = self
            .prompt_histories
            .iter_mut()
            .find(|history| history.kind == kind)
        {
            history.position = None;
            history.draft.clear();
        }
    }

    fn prompt_history_index(&mut self, kind: PromptKind) -> usize {
        if let Some(index) = self
            .prompt_histories
            .iter()
            .position(|history| history.kind == kind)
        {
            return index;
        }
        self.prompt_histories.push(PromptHistory {
            kind,
            entries: Vec::new(),
            position: None,
            draft: String::new(),
        });
        self.prompt_histories.len() - 1
    }

    fn complete_prompt_common_prefix(&mut self) {
        let input = self
            .minibuffer
            .prompt_input()
            .unwrap_or_default()
            .to_owned();
        let Some(prefix) = self
            .completion
            .as_ref()
            .and_then(|completion| completion.common_prefix(&input))
        else {
            return;
        };
        self.minibuffer.set_prompt_input(prefix);
        self.update_completion_from_prompt();
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
        self.current_buffer = completions;
        self.cursor = Position::new(0, 0);
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        let viewport = self.windows.current_mut().viewport_mut();
        viewport.first_visible_line = 0;
        viewport.first_visible_column = 0;
        self.sync_current_window();
    }

    fn finish_completion_buffer(&mut self) {
        let Some(viewport) = self.completion_return.take() else {
            return;
        };
        if self.buffers.document(viewport.buffer).is_none() {
            return;
        }
        self.current_buffer = viewport.buffer;
        self.cursor = viewport.cursor;
        self.goal_display_column = None;
        self.search = None;
        self.query_replace = None;
        self.deactivate_region();
        self.clear_insert_group();
        *self.windows.current_mut().viewport_mut() = viewport;
    }

    fn submit_prompt(&mut self, kind: PromptKind, input: &str) -> Result<EditorOutcome> {
        match kind {
            PromptKind::DescribeFunction => Ok(self.describe_function(input.trim())),
            PromptKind::ExtendedCommand => self.submit_extended_command(input.trim()),
            PromptKind::FindFile => self.find_file(input.trim()),
            PromptKind::FindFileReadOnly => self.find_file_read_only(input.trim()),
            PromptKind::GotoLine => self.goto_line(input.trim()),
            PromptKind::InsertFile => self.insert_file(input.trim()),
            PromptKind::IncrementalSearch => Ok(EditorOutcome::Continue),
            PromptKind::KillBuffer => self.kill_buffer(input.trim()),
            PromptKind::QueryReplaceReplacement => self.submit_query_replace_replacement(input),
            PromptKind::QueryReplaceSearch => self.submit_query_replace_search(input),
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
        command: Command,
        argument: Option<i32>,
    ) -> Result<EditorOutcome> {
        use Command::*;

        let kill_command = is_kill_command(command);
        let yank_command = is_yank_command(command);
        if kill_command {
            self.kill_recorded_this_command = false;
        } else {
            self.last_command_was_kill = false;
        }
        if !yank_command {
            self.yank_state = None;
        }

        match command {
            BackToIndentation => self.move_back_to_indentation(),
            BackwardChar => self.repeat_signed(argument, Self::move_backward, Self::move_forward),
            BackwardKillWord => {
                self.repeat_signed_kill(argument, Self::backward_kill_word, Self::kill_word)
            }
            BackwardWord => {
                self.repeat_signed(argument, Self::move_word_backward, Self::move_word_forward)
            }
            BeginningOfBuffer => self.move_beginning_of_buffer(),
            BeginningOfLine => self.move_beginning_of_line(),
            CallLastKeyboardMacro => return self.call_last_keyboard_macro(argument),
            CopyRegionAsKill => self.copy_region_as_kill(),
            DeleteBackwardChar => {
                self.repeat_signed(argument, Self::delete_backward_char, Self::delete_char)
            }
            DeleteChar => {
                self.repeat_signed(argument, Self::delete_char, Self::delete_backward_char)
            }
            DeleteOtherWindows => self.delete_other_windows(),
            DeleteWindow => self.delete_window(),
            DescribeFunction => self.start_describe_function(),
            DescribeKey => self.start_describe_key(),
            EndKeyboardMacro => self.end_keyboard_macro(),
            EndOfBuffer => self.move_end_of_buffer(),
            EndOfLine => self.move_end_of_line(),
            ExchangePointAndMark => self.exchange_point_and_mark(),
            ExecuteExtendedCommand => self.start_extended_command(),
            FindFile => self.start_find_file(),
            FindFileReadOnly => self.start_find_file_read_only(),
            ForwardChar => self.repeat_signed(argument, Self::move_forward, Self::move_backward),
            ForwardWord => {
                self.repeat_signed(argument, Self::move_word_forward, Self::move_word_backward)
            }
            GotoLine => self.start_goto_line(),
            IncrementalSearchBackward => self.start_incremental_search(SearchDirection::Backward),
            IncrementalSearchForward => self.start_incremental_search(SearchDirection::Forward),
            InsertFile => self.start_insert_file(),
            JoinLine => self.join_line(),
            ListBuffers => self.list_buffers(),
            KillLine => self.repeat_positive_kill(argument, Self::kill_line),
            KillRegion => self.kill_region(),
            KillWord => {
                self.repeat_signed_kill(argument, Self::kill_word, Self::backward_kill_word)
            }
            MarkWholeBuffer => self.mark_whole_buffer(),
            NextLine => self.move_line_by_argument(argument, 1),
            OpenLine => self.repeat_positive(argument, Self::open_line),
            PreviousLine => self.move_line_by_argument(argument, -1),
            QuotedInsert => self.start_quoted_insert(),
            QueryReplace => self.start_query_replace(),
            RectangleMarkMode => self.rectangle_mark_mode(),
            Recenter => self.recenter(),
            SaveBuffer => self.save_buffer(),
            SaveBuffersKillTerminal => return Ok(EditorOutcome::Quit),
            SetMarkCommand => self.set_mark_command(),
            StartKeyboardMacro => self.start_keyboard_macro(),
            KillBuffer => self.start_kill_buffer(),
            OtherWindow => self.other_window(),
            ScrollPageBackward => self.scroll_page_backward(),
            ScrollPageForward => self.scroll_page_forward(),
            SwitchToBuffer => self.start_switch_to_buffer(),
            SplitWindowBelow => self.split_window(SplitAxis::Horizontal),
            SplitWindowRight => self.split_window(SplitAxis::Vertical),
            ToggleLineNumbers => self.toggle_line_numbers(),
            ToggleReadOnly => self.toggle_read_only(),
            ToggleSearchHighlighting => self.toggle_search_highlighting(),
            ToggleSyntaxHighlighting => self.toggle_syntax_highlighting(),
            Undo => self.undo(),
            UniversalArgument => self.extend_universal_argument(),
            WriteFile => self.start_write_file(),
            Yank => self.yank(),
            YankPop => self.yank_pop(),
        }?;

        if kill_command {
            self.last_command_was_kill = self.kill_recorded_this_command;
            self.kill_recorded_this_command = false;
        }

        Ok(EditorOutcome::Continue)
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
            | KeyEvent::Meta(_)
            | KeyEvent::MetaSpecial(_)
            | KeyEvent::Special(_) => self
                .minibuffer
                .set_error("quoted control insertion is not supported"),
        }
        Ok(EditorOutcome::Continue)
    }

    fn quit_current_operation(&mut self) {
        self.quoted_insert = false;
        self.describe_key = None;
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
        let max_first_visible_line = line_count.saturating_sub(text_rows);
        let centered_first_visible_line = self.cursor.line.saturating_sub(text_rows / 2);
        self.windows.current_mut().viewport_mut().first_visible_line =
            centered_first_visible_line.min(max_first_visible_line);
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
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_describe_function(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::DescribeFunction, "Describe command: ");
        self.completion = Some(CompletionSession::commands(
            &self.commands,
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_describe_key(&mut self) -> Result<()> {
        self.describe_key = Some(Vec::new());
        self.minibuffer.set_message("Describe key: ");
        Ok(())
    }

    fn handle_describe_key(&mut self, key: KeyEvent) -> EditorOutcome {
        let Some(sequence) = &mut self.describe_key else {
            return EditorOutcome::Continue;
        };
        sequence.push(key);
        let sequence = sequence.clone();

        match self.keymap.resolve(&sequence) {
            KeyResolution::Prefix => {
                self.minibuffer
                    .set_message(format!("Describe key: {}-", format_key_sequence(&sequence)));
                EditorOutcome::Continue
            }
            KeyResolution::Command(command) => {
                self.describe_key = None;
                let text =
                    format_describe_key_help(&self.commands, &self.keymap, &sequence, command);
                self.open_help_buffer(text)
            }
            KeyResolution::NoMatch => {
                self.describe_key = None;
                let text = format_unbound_key_help(&sequence);
                self.open_help_buffer(text)
            }
        }
    }

    fn describe_function(&mut self, name: &str) -> EditorOutcome {
        let Some(command) = self.commands.get(name) else {
            self.minibuffer
                .set_message(format!("No such command: {name}"));
            return EditorOutcome::Continue;
        };
        let text = format_describe_function_help(&self.keymap, command.name, command.description);
        self.open_help_buffer(text)
    }

    fn start_find_file(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::FindFile, "Find file: ");
        self.completion = Some(CompletionSession::files(
            self.find_file_base_dir(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_find_file_read_only(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::FindFileReadOnly, "Find file read-only: ");
        self.completion = Some(CompletionSession::files(
            self.find_file_base_dir(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_insert_file(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.minibuffer
            .start_prompt(PromptKind::InsertFile, "Insert file: ");
        self.completion = Some(CompletionSession::files(
            self.find_file_base_dir(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn start_goto_line(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::GotoLine, "Goto line: ");
        Ok(())
    }

    fn start_switch_to_buffer(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::SwitchToBuffer, "Switch to buffer: ");
        self.completion = Some(CompletionSession::buffers(
            self.buffer_completion_names(),
            self.completion_config,
        ));
        self.update_completion_from_prompt();
        Ok(())
    }

    fn buffer_completion_names(&self) -> Vec<String> {
        self.buffers
            .entries()
            .iter()
            .map(|entry| entry.name().to_owned())
            .collect()
    }

    fn start_kill_buffer(&mut self) -> Result<()> {
        let label = format!("Kill buffer (default {}): ", self.current_buffer_name());
        self.minibuffer.start_prompt(PromptKind::KillBuffer, label);
        Ok(())
    }

    fn list_buffers(&mut self) -> Result<()> {
        self.sync_current_window();
        let text = self.format_buffer_list();
        let buffer_list = self.buffers.open_buffer_list(text);
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

    fn format_buffer_list(&self) -> String {
        let mut text =
            String::from("CRM Buffer                           Size Mode         File\n");
        text.push_str("--- ------                           ---- ----         ----\n");
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
        }
        text
    }

    fn refresh_visible_buffer_list(&mut self) {
        let Some(buffer_list) = self.buffers.find_by_name("*Buffer List*") else {
            return;
        };
        if self.windows.window_showing_buffer(buffer_list).is_none() {
            return;
        }
        let text = self.format_buffer_list();
        self.buffers.open_buffer_list(text);
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
                self.current_buffer = id;
                self.cursor = Position::new(0, 0);
                self.goal_display_column = None;
                self.search = None;
                self.query_replace = None;
                self.deactivate_region();
                self.clear_insert_group();
                self.sync_current_window();
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

        match self.buffers.kill(target) {
            Ok(next_current) => {
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
                    .set_message(format!("Killed buffer {target_name}"));
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
            _ => self
                .minibuffer
                .set_message("Query replace: type y, n, !, or q"),
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
            self.minibuffer
                .set_message("Query replace: type y, n, !, or q");
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
            self.minibuffer
                .set_message(format!("Query replace done ({replacements} {noun})"));
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
                self.minibuffer.delete_prompt_grapheme_backward();
                self.update_incremental_search()?;
            }
            KeyEvent::Ctrl('s') => self.repeat_incremental_search(SearchDirection::Forward)?,
            KeyEvent::Ctrl('r') => self.repeat_incremental_search(SearchDirection::Backward)?,
            KeyEvent::Text(text) => {
                self.minibuffer.insert_prompt_text(&text);
                self.update_incremental_search()?;
            }
            KeyEvent::Special(SpecialKey::Tab) => {}
            _ => {}
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
        Command::BackwardKillWord | Command::KillLine | Command::KillRegion | Command::KillWord
    )
}

fn is_yank_command(command: Command) -> bool {
    matches!(command, Command::Yank | Command::YankPop)
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

fn format_key_sequence(sequence: &[KeyEvent]) -> String {
    sequence
        .iter()
        .map(format_key_event)
        .collect::<Vec<_>>()
        .join(" ")
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

fn format_key_prefix_help(keymap: &KeyMap, prefix: &[KeyEvent]) -> String {
    let mut text = format!(
        "Global Bindings Starting With {}:\n\n",
        format_key_sequence(prefix)
    );
    text.push_str("Key             Binding\n");
    text.push_str("---             -------\n\n");

    for binding in keymap.bindings_starting_with(prefix) {
        text.push_str(&format!(
            "{:<15} {}\n",
            format_key_sequence(&binding.sequence),
            binding.command
        ));
    }

    text
}

fn format_describe_key_help(
    commands: &CommandRegistry,
    keymap: &KeyMap,
    sequence: &[KeyEvent],
    command: &str,
) -> String {
    let mut text = format!(
        "{} runs the command `{}`.\n\n",
        format_key_sequence(sequence),
        command
    );
    let description = commands.get(command).map(|command| command.description);
    text.push_str(&format_command_help(keymap, command, description));
    text
}

fn format_describe_function_help(keymap: &KeyMap, name: &str, description: &str) -> String {
    format_command_help(keymap, name, Some(description))
}

fn format_unbound_key_help(sequence: &[KeyEvent]) -> String {
    format!(
        "{} is not bound to any command.\n",
        format_key_sequence(sequence)
    )
}

fn format_command_help(keymap: &KeyMap, name: &str, description: Option<&str>) -> String {
    let mut text = match description {
        Some(description) => format!("{} is an interactive command.\n\n{}\n", name, description),
        None => format!("{} is not a known interactive command.\n", name),
    };
    let bindings = keymap.bindings_for_command(name);
    if bindings.is_empty() {
        text.push_str("\nIt is not bound to any key.\n");
    } else {
        let keys = bindings
            .iter()
            .map(|binding| format_key_sequence(&binding.sequence))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!("\nIt is bound to {}.\n", keys));
    }
    text
}

fn format_key_event(key: &KeyEvent) -> String {
    match key {
        KeyEvent::Ctrl(character) => format!("C-{character}"),
        KeyEvent::Meta(character) => format!("M-{character}"),
        KeyEvent::MetaSpecial(special) => format!("M-{}", format_special_key(*special)),
        KeyEvent::Text(text) => text.clone(),
        KeyEvent::Special(special) => format_special_key(*special),
    }
}

fn format_special_key(key: SpecialKey) -> String {
    match key {
        SpecialKey::Backspace => "Backspace".to_owned(),
        SpecialKey::Delete => "Delete".to_owned(),
        SpecialKey::Enter => "Enter".to_owned(),
        SpecialKey::Tab => "Tab".to_owned(),
        SpecialKey::Escape => "Esc".to_owned(),
        SpecialKey::ArrowUp => "Up".to_owned(),
        SpecialKey::ArrowDown => "Down".to_owned(),
        SpecialKey::ArrowLeft => "Left".to_owned(),
        SpecialKey::ArrowRight => "Right".to_owned(),
        SpecialKey::Home => "Home".to_owned(),
        SpecialKey::End => "End".to_owned(),
        SpecialKey::PageUp => "PageUp".to_owned(),
        SpecialKey::PageDown => "PageDown".to_owned(),
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

fn find_match(
    buffer: &crate::buffer::Buffer,
    query: &str,
    start: Position,
    direction: SearchDirection,
) -> Result<Option<TextRange>> {
    buffer.validate_position(start)?;
    if query.is_empty() {
        return Ok(None);
    }

    match direction {
        SearchDirection::Forward => find_forward(buffer, query, start),
        SearchDirection::Backward => find_backward(buffer, query, start),
    }
}

fn find_forward(
    buffer: &crate::buffer::Buffer,
    query: &str,
    start: Position,
) -> Result<Option<TextRange>> {
    for line_index in start.line..buffer.line_count() {
        let line = buffer.line(line_index).expect("line index is in range");
        let minimum_byte = if line_index == start.line {
            start.byte
        } else {
            0
        };
        if let Some((match_start, match_text)) = line
            .match_indices(query)
            .find(|(match_start, _)| *match_start >= minimum_byte)
        {
            return Ok(Some(TextRange::new(
                Position::new(line_index, match_start),
                Position::new(line_index, match_start + match_text.len()),
            )));
        }
    }
    Ok(None)
}

fn find_backward(
    buffer: &crate::buffer::Buffer,
    query: &str,
    start: Position,
) -> Result<Option<TextRange>> {
    for line_index in (0..=start.line).rev() {
        let line = buffer.line(line_index).expect("line index is in range");
        let maximum_byte = if line_index == start.line {
            start.byte
        } else {
            line.len()
        };
        if let Some((match_start, match_text)) = line
            .match_indices(query)
            .filter(|(match_start, _)| *match_start < maximum_byte)
            .last()
        {
            return Ok(Some(TextRange::new(
                Position::new(line_index, match_start),
                Position::new(line_index, match_start + match_text.len()),
            )));
        }
    }
    Ok(None)
}

fn search_start_after(buffer: &crate::buffer::Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let line = buffer.line(position.line).expect("line index is in range");
    if position.byte < line.len() {
        let character_width = line[position.byte..]
            .chars()
            .next()
            .expect("position before line end has a character")
            .len_utf8();
        return Ok(Position::new(
            position.line,
            position.byte + character_width,
        ));
    }
    if position.line + 1 < buffer.line_count() {
        return Ok(Position::new(position.line + 1, 0));
    }
    Ok(buffer.end_position())
}

fn format_completion_buffer(completion: &CompletionSession) -> String {
    let title = match completion.source() {
        CompletionSource::Commands => "Possible Completions for M-x:",
        CompletionSource::Files => "Possible Completions for Find file:",
        CompletionSource::Buffers => "Possible Completions for Switch to buffer:",
    };
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

fn prompt_label(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::DescribeFunction => "Describe command: ",
        PromptKind::ExtendedCommand => "M-x ",
        PromptKind::FindFile => "Find file: ",
        PromptKind::FindFileReadOnly => "Find file read-only: ",
        PromptKind::GotoLine => "Goto line: ",
        PromptKind::InsertFile => "Insert file: ",
        PromptKind::IncrementalSearch => "I-search: ",
        PromptKind::KillBuffer => "Kill buffer: ",
        PromptKind::QueryReplaceReplacement => "Query replace with: ",
        PromptKind::QueryReplaceSearch => "Query replace: ",
        PromptKind::SwitchToBuffer => "Switch to buffer: ",
        PromptKind::WriteFile => "Write file: ",
    }
}

fn prompt_kind_uses_history(kind: PromptKind) -> bool {
    matches!(
        kind,
        PromptKind::ExtendedCommand
            | PromptKind::DescribeFunction
            | PromptKind::FindFile
            | PromptKind::FindFileReadOnly
            | PromptKind::GotoLine
            | PromptKind::InsertFile
            | PromptKind::KillBuffer
            | PromptKind::SwitchToBuffer
            | PromptKind::WriteFile
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Editor, EditorOutcome, KillEntry};
    use crate::buffer::{Position, TextRange};
    use crate::completion::{CompletionConfig, CompletionMatching, CompletionStyle};
    use crate::config::{Config, ThemeName};
    use crate::file::Document;
    use crate::input::{KeyEvent, SpecialKey};
    use crate::minibuffer::PromptKind;
    use crate::render::{DecorationProvider, Face, Span};
    use crate::syntax::{MajorMode, SyntaxMode};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
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
    fn extended_command_tab_extends_common_prefix() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Tab))
            .expect("tab should complete common prefix");

        assert_eq!(editor.minibuffer().prompt_input(), Some("toggle-"));
    }

    #[test]
    fn find_file_completion_extends_common_prefix() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(directory.path().join("alpha-note.txt"), "alpha")
            .expect("alpha fixture should write");
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
            .expect("tab should complete common prefix");

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha"));
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
    fn find_file_completion_keeps_raw_ambiguous_missing_file_input() {
        let directory = TestDir::new();
        let start = directory.path().join("start.txt");
        let missing = directory.path().join("alpha");
        fs::write(&start, "start").expect("start fixture should write");
        fs::write(directory.path().join("alpha-note.txt"), "alpha")
            .expect("alpha fixture should write");
        fs::create_dir(directory.path().join("alpha-dir")).expect("directory should create");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix should start");
        editor
            .handle_key(KeyEvent::Ctrl('f'))
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("alpha".to_owned()))
            .expect("prompt input should update completion");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("enter should open raw missing file");

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

        assert_eq!(editor.minibuffer().prompt_input(), Some("alpha-dir/"));
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
        assert!(text.contains("Find file: alpha"));
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
    fn buffer_completion_keeps_ambiguous_raw_input() {
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
            .expect("enter should submit raw ambiguous input");

        assert_eq!(editor.current_buffer_name(), "alphabet-buffer.txt");
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Error: no such buffer: alpha")
        );
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
        assert!(text.contains("Switch to buffer: "));
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

        assert_eq!(editor.minibuffer().prompt_input(), Some(""));
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
    fn prompt_history_resets_when_file_completion_enters_directory() {
        let directory = TestDir::new();
        let alpha_dir = directory.path().join("alpha-dir");
        fs::create_dir(&alpha_dir).expect("directory fixture should create");
        let start = directory.path().join("start.txt");
        fs::write(&start, "start").expect("start fixture should write");
        let document = Document::open(&start).expect("start fixture should open");
        let mut editor = Editor::new(document);
        editor.record_prompt_history(PromptKind::FindFile, "alpha-dir");

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
    fn recenter_moves_viewport_without_moving_cursor() {
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
Key             Binding\n\
---             -------\n\n\
M-g g           goto-line\n"
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
        assert!(help.contains("C-x C-f runs the command `find-file`."));
        assert!(help.contains("find-file is an interactive command."));
        assert!(help.contains("Open file by path"));
        assert!(help.contains("It is bound to C-x C-f."));
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
        assert!(help.contains("find-file is an interactive command."));
        assert!(help.contains("Open file by path"));
        assert!(help.contains("It is bound to C-x C-f."));
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
                .contains("find-file is an interactive command.")
        );
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
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Find file: ")
        );
        for character in path.to_string_lossy().chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("file prompt should update");
        }
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
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Find file read-only: ")
        );
        for character in path.to_string_lossy().chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("file prompt should update");
        }
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
        for character in path.to_string_lossy().chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("file prompt should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("missing file should become buffer");

        assert_eq!(editor.document().path(), Some(path.as_path()));
        assert!(editor.document().missing_on_open());
        assert_eq!(editor.document().buffer().serialize(), "");
    }

    #[test]
    fn find_file_prompt_reports_empty_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty input should be reported");

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
        assert_eq!(
            editor.minibuffer().display_text().as_deref(),
            Some("Insert file: ")
        );
        submit_prompt_text(&mut editor, source.to_str().expect("path should be utf-8"));

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
    fn insert_file_prompt_reports_empty_input() {
        let mut editor = Editor::new(Document::scratch());

        editor
            .execute_command_by_name("insert-file")
            .expect("insert-file should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("empty input should be reported");

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
        for character in path.to_string_lossy().chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("file prompt should update");
        }
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("file should open");
        let first_id = editor.current_buffer_id();

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should prompt again");
        for character in path.to_string_lossy().chars() {
            editor
                .handle_key(KeyEvent::Text(character.to_string()))
                .expect("file prompt should update");
        }
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
    fn kill_buffer_refuses_dirty_current_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .handle_key(KeyEvent::Text("dirty".to_owned()))
            .expect("text should insert");

        editor
            .execute_command_by_name("kill-buffer")
            .expect("kill should prompt");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("dirty kill should be reported");

        assert_eq!(editor.buffer_count(), 1);
        assert!(editor.document().is_dirty());
        assert!(
            editor
                .minibuffer()
                .message
                .as_deref()
                .is_some_and(|message| message.contains("unsaved changes"))
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
            4
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
    fn editor_applies_config_options_and_toggle_commands() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                tab_width: 2,
                line_numbers: true,
                syntax_highlighting: false,
                search_highlighting: false,
                backup_on_save: true,
                theme: ThemeName::Mono,
                completion: Default::default(),
            },
        );

        assert_eq!(editor.tab_width(), 2);
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
            Some("Query replace done (2 replacements)")
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
