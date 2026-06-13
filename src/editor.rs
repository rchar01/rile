// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buffer::undo::UndoRecord;
use crate::buffer::{BufferId, Position, TextRange};
use crate::buffers::BufferManager;
use crate::command::{Command, CommandRegistry};
use crate::config::{Config, ThemeName};
use crate::file::Document;
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
    keymap: KeyMap,
    commands: CommandRegistry,
    minibuffer: MinibufferState,
    help_return: Option<Viewport>,
    search: Option<SearchState>,
    query_replace: Option<QueryReplaceState>,
    region: Option<RegionState>,
    kill_ring: Vec<String>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UndoEntry {
    buffer: BufferId,
    record: UndoRecord,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchState {
    direction: SearchDirection,
    origin: Position,
    current: Option<TextRange>,
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
            keymap: KeyMap::default(),
            commands: CommandRegistry::default(),
            minibuffer: MinibufferState::default(),
            help_return: None,
            search: None,
            query_replace: None,
            region: None,
            kill_ring: Vec::new(),
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

    pub fn ensure_current_window_contains_cursor(
        &mut self,
        text_rows: usize,
        text_columns: usize,
        cursor_display_column: usize,
    ) {
        self.sync_current_window();
        let viewport = self.windows.current_mut().viewport_mut();

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
        if self.minibuffer.prompt().is_some() {
            return self.handle_prompt_key(key);
        }

        if self.query_replace.is_some() {
            return self.handle_query_replace_key(key);
        }

        if key == KeyEvent::Ctrl('g') {
            self.clear_key_sequence();
            self.deactivate_region();
            self.clear_insert_group();
            self.minibuffer.set_message("Quit");
            return Ok(EditorOutcome::Continue);
        }

        if self.document().is_help() && key == KeyEvent::Text("q".to_owned()) {
            return Ok(self.restore_help_buffer());
        }

        if !self.key_sequence.is_empty() {
            return self.handle_bound_key(key);
        }

        match key {
            KeyEvent::Text(text) => {
                self.clear_key_sequence();
                self.insert_text(&text, true)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Enter) => {
                self.clear_key_sequence();
                self.insert_text("\n", false)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => {
                self.clear_key_sequence();
                self.insert_text("\t", false)?;
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

        self.execute_command(command.command)
    }

    fn handle_bound_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if !self.key_sequence.is_empty() && is_key_prefix_help(&key) {
            return Ok(self.show_key_prefix_help());
        }

        self.key_sequence.push(key);

        match self.keymap.resolve(&self.key_sequence) {
            KeyResolution::NoMatch => {
                self.clear_key_sequence();
                self.minibuffer.set_message("Key is not bound");
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Prefix => {
                self.minibuffer
                    .set_message(format_key_prefix_message(&self.key_sequence));
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Command(name) => {
                self.clear_key_sequence();
                self.execute_command_by_name(name)
            }
        }
    }

    fn show_key_prefix_help(&mut self) -> EditorOutcome {
        let prefix = self.key_sequence.clone();
        let text = format_key_prefix_help(&self.keymap, &prefix);
        self.sync_current_window();
        if !self.document().is_help() || self.help_return.is_none() {
            self.help_return = Some(*self.windows.current().viewport());
        }
        let help = self.buffers.open_help(text);

        self.clear_key_sequence();
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

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if self.minibuffer.prompt_kind() == Some(PromptKind::IncrementalSearch) {
            return self.handle_search_prompt_key(key);
        }

        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let Some((kind, input)) = self.minibuffer.take_prompt_input() else {
                    return Ok(EditorOutcome::Continue);
                };
                self.minibuffer.clear();
                self.submit_prompt(kind, &input)
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
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
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                self.minibuffer.insert_prompt_text(&text);
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => Ok(EditorOutcome::Continue),
            _ => Ok(EditorOutcome::Continue),
        }
    }

    fn submit_prompt(&mut self, kind: PromptKind, input: &str) -> Result<EditorOutcome> {
        match kind {
            PromptKind::ExtendedCommand => self.execute_command_by_name(input.trim()),
            PromptKind::FindFile => self.find_file(input.trim()),
            PromptKind::GotoLine => self.goto_line(input.trim()),
            PromptKind::IncrementalSearch => Ok(EditorOutcome::Continue),
            PromptKind::KillBuffer => self.kill_buffer(input.trim()),
            PromptKind::QueryReplaceReplacement => self.submit_query_replace_replacement(input),
            PromptKind::QueryReplaceSearch => self.submit_query_replace_search(input),
            PromptKind::SwitchToBuffer => self.switch_to_buffer(input.trim()),
        }
    }

    fn execute_command(&mut self, command: Command) -> Result<EditorOutcome> {
        use Command::*;

        match command {
            BackwardChar => self.move_backward(),
            BackwardWord => self.move_word_backward(),
            BeginningOfBuffer => self.move_beginning_of_buffer(),
            BeginningOfLine => self.move_beginning_of_line(),
            CopyRegionAsKill => self.copy_region_as_kill(),
            DeleteBackwardChar => self.delete_backward_char(),
            DeleteChar => self.delete_char(),
            DeleteOtherWindows => self.delete_other_windows(),
            DeleteWindow => self.delete_window(),
            EndOfBuffer => self.move_end_of_buffer(),
            EndOfLine => self.move_end_of_line(),
            ExecuteExtendedCommand => self.start_extended_command(),
            FindFile => self.start_find_file(),
            ForwardChar => self.move_forward(),
            ForwardWord => self.move_word_forward(),
            GotoLine => self.start_goto_line(),
            IncrementalSearchBackward => self.start_incremental_search(SearchDirection::Backward),
            IncrementalSearchForward => self.start_incremental_search(SearchDirection::Forward),
            KillLine => self.kill_line(),
            KillRegion => self.kill_region(),
            NextLine => self.move_line(1),
            OpenLine => self.open_line(),
            PreviousLine => self.move_line(-1),
            QueryReplace => self.start_query_replace(),
            SaveBuffer => self.save_buffer(),
            SaveBuffersKillTerminal => return Ok(EditorOutcome::Quit),
            SetMarkCommand => self.set_mark_command(),
            KillBuffer => self.start_kill_buffer(),
            OtherWindow => self.other_window(),
            SwitchToBuffer => self.start_switch_to_buffer(),
            SplitWindowBelow => self.split_window(SplitAxis::Horizontal),
            SplitWindowRight => self.split_window(SplitAxis::Vertical),
            ToggleLineNumbers => self.toggle_line_numbers(),
            ToggleSearchHighlighting => self.toggle_search_highlighting(),
            ToggleSyntaxHighlighting => self.toggle_syntax_highlighting(),
            Undo => self.undo(),
            Yank => self.yank(),
        }?;

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
        });
        self.minibuffer.set_message("Mark set");
        Ok(())
    }

    fn copy_region_as_kill(&mut self) -> Result<()> {
        self.clear_insert_group();
        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let text = self.document().buffer().text_in_range(range)?;
        self.push_kill(text);
        self.deactivate_region();
        self.minibuffer.set_message("Copied region");
        Ok(())
    }

    fn kill_region(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(range) = self.active_region_range() else {
            self.minibuffer.set_error("no active region");
            return Ok(());
        };
        let cursor_before = self.cursor;
        let text = self.document_mut().buffer_mut().delete_range(range)?;
        self.push_kill(text.clone());
        self.cursor = range.start;
        self.goal_display_column = None;
        self.record_delete(range, text, cursor_before, self.cursor);
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed region");
        Ok(())
    }

    fn yank(&mut self) -> Result<()> {
        if !self.ensure_buffer_editable() {
            return Ok(());
        }
        self.clear_insert_group();
        let Some(text) = self.kill_ring.last().cloned() else {
            self.minibuffer.set_error("kill ring is empty");
            return Ok(());
        };
        let cursor_before = self.cursor;
        self.cursor = self
            .document_mut()
            .buffer_mut()
            .insert(cursor_before, &text)?;
        self.record_insert(cursor_before, self.cursor, &text, false);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Yanked");
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
        self.push_kill(text.clone());
        self.record_delete(range, text, cursor_before, self.cursor);
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Killed line");
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
        match entry.record {
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
        self.goal_display_column = None;
        self.deactivate_region();
        self.sync_current_window();
        self.minibuffer.set_message("Undone");
        Ok(())
    }

    fn save_buffer(&mut self) -> Result<()> {
        match self.document_mut().save() {
            Ok(()) => self
                .minibuffer
                .set_message(format!("Wrote {}", self.document().display_name())),
            Err(error) => self.minibuffer.set_error(format!("save failed: {error}")),
        }
        Ok(())
    }

    fn start_extended_command(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::ExtendedCommand, "M-x ");
        Ok(())
    }

    fn start_find_file(&mut self) -> Result<()> {
        self.minibuffer
            .start_prompt(PromptKind::FindFile, "Find file: ");
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
        Ok(())
    }

    fn start_kill_buffer(&mut self) -> Result<()> {
        let label = format!("Kill buffer (default {}): ", self.current_buffer_name());
        self.minibuffer.start_prompt(PromptKind::KillBuffer, label);
        Ok(())
    }

    fn find_file(&mut self, path: &str) -> Result<EditorOutcome> {
        if path.is_empty() {
            self.minibuffer.set_error("missing file name");
            return Ok(EditorOutcome::Continue);
        }

        match self
            .buffers
            .open_path_with_backup(path, self.backup_on_save)
        {
            Ok(opened) => {
                self.current_buffer = opened.id;
                self.cursor = Position::new(0, 0);
                self.goal_display_column = None;
                self.search = None;
                self.query_replace = None;
                self.deactivate_region();
                self.clear_insert_group();
                self.sync_current_window();
                self.minibuffer
                    .set_message(format!("Opened {}", self.document().display_name()));
            }
            Err(error) => self.minibuffer.set_error(format!("open failed: {error}")),
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
        if name.is_empty() {
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
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.label());
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
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
        let start = match (direction, search.current) {
            (SearchDirection::Forward, Some(range)) => {
                search_start_after(self.document().buffer(), range.start)?
            }
            (SearchDirection::Backward, Some(range)) => range.start,
            (_, None) => search.origin,
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
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.sync_current_window();
            self.minibuffer.set_prompt_label(direction.label());
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
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
        if !region.active || region.buffer != self.current_buffer || region.mark == self.cursor {
            return None;
        }
        let (start, end) = if region.mark < self.cursor {
            (region.mark, self.cursor)
        } else {
            (self.cursor, region.mark)
        };
        Some(TextRange::new(start, end))
    }

    fn deactivate_region(&mut self) {
        if let Some(region) = &mut self.region {
            region.active = false;
        }
    }

    fn push_kill(&mut self, text: String) {
        if !text.is_empty() {
            self.kill_ring.push(text);
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
    }

    fn clear_insert_group(&mut self) {
        self.grouping_insert = false;
    }

    fn ensure_buffer_editable(&mut self) -> bool {
        if self.document().is_read_only() {
            self.minibuffer.set_error("buffer is read-only");
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

fn format_key_event(key: &KeyEvent) -> String {
    match key {
        KeyEvent::Ctrl(character) => format!("C-{character}"),
        KeyEvent::Meta(character) => format!("M-{character}"),
        KeyEvent::Text(text) => text.clone(),
        KeyEvent::Special(SpecialKey::Backspace) => "Backspace".to_owned(),
        KeyEvent::Special(SpecialKey::Delete) => "Delete".to_owned(),
        KeyEvent::Special(SpecialKey::Enter) => "Enter".to_owned(),
        KeyEvent::Special(SpecialKey::Tab) => "Tab".to_owned(),
        KeyEvent::Special(SpecialKey::Escape) => "Esc".to_owned(),
        KeyEvent::Special(SpecialKey::ArrowUp) => "Up".to_owned(),
        KeyEvent::Special(SpecialKey::ArrowDown) => "Down".to_owned(),
        KeyEvent::Special(SpecialKey::ArrowLeft) => "Left".to_owned(),
        KeyEvent::Special(SpecialKey::ArrowRight) => "Right".to_owned(),
        KeyEvent::Special(SpecialKey::Home) => "Home".to_owned(),
        KeyEvent::Special(SpecialKey::End) => "End".to_owned(),
        KeyEvent::Special(SpecialKey::PageUp) => "PageUp".to_owned(),
        KeyEvent::Special(SpecialKey::PageDown) => "PageDown".to_owned(),
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
}

impl DecorationProvider for RegionDecorator {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{Editor, EditorOutcome};
    use crate::buffer::Position;
    use crate::config::{Config, ThemeName};
    use crate::file::Document;
    use crate::input::{KeyEvent, SpecialKey};
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
            Some("Error: buffer is read-only")
        );

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
}
