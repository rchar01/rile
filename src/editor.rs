// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buffer::{Position, TextRange};
use crate::command::{Command, CommandRegistry};
use crate::file::Document;
use crate::input::{KeyEvent, SpecialKey};
use crate::keymap::{KeyMap, KeyResolution};
use crate::minibuffer::{MinibufferState, PromptKind};
use crate::render::{DecorationProvider, Face, Span};
use crate::{Result, RileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    Continue,
    Quit,
}

#[derive(Debug, Clone)]
pub struct Editor {
    document: Document,
    cursor: Position,
    goal_display_column: Option<usize>,
    key_sequence: Vec<KeyEvent>,
    keymap: KeyMap,
    commands: CommandRegistry,
    minibuffer: MinibufferState,
    search: Option<SearchState>,
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
        Self {
            document,
            cursor: Position::new(0, 0),
            goal_display_column: None,
            key_sequence: Vec::new(),
            keymap: KeyMap::default(),
            commands: CommandRegistry::default(),
            minibuffer: MinibufferState::default(),
            search: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    pub fn minibuffer(&self) -> &MinibufferState {
        &self.minibuffer
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        if self.minibuffer.prompt().is_some() {
            return self.handle_prompt_key(key);
        }

        if key == KeyEvent::Ctrl('g') {
            self.clear_key_sequence();
            self.minibuffer.set_message("Quit");
            return Ok(EditorOutcome::Continue);
        }

        match key {
            KeyEvent::Text(text) => {
                self.clear_key_sequence();
                self.insert_text(&text)?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Enter) => {
                self.clear_key_sequence();
                self.insert_text("\n")?;
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Tab) => {
                self.clear_key_sequence();
                self.insert_text("\t")?;
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
        self.key_sequence.push(key);

        match self.keymap.resolve(&self.key_sequence) {
            KeyResolution::NoMatch => {
                self.clear_key_sequence();
                self.minibuffer.set_message("Key is not bound");
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Prefix => {
                self.minibuffer.set_message("Prefix key");
                Ok(EditorOutcome::Continue)
            }
            KeyResolution::Command(name) => {
                self.clear_key_sequence();
                self.execute_command_by_name(name)
            }
        }
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
                self.submit_prompt(kind, input.trim())
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
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
            PromptKind::ExtendedCommand => self.execute_command_by_name(input),
            PromptKind::FindFile => self.find_file(input),
            PromptKind::IncrementalSearch => Ok(EditorOutcome::Continue),
        }
    }

    fn execute_command(&mut self, command: Command) -> Result<EditorOutcome> {
        use Command::*;

        match command {
            BackwardChar => self.move_backward(),
            BeginningOfLine => self.move_beginning_of_line(),
            DeleteBackwardChar => self.delete_backward_char(),
            DeleteChar => self.delete_char(),
            EndOfLine => self.move_end_of_line(),
            ExecuteExtendedCommand => self.start_extended_command(),
            FindFile => self.start_find_file(),
            ForwardChar => self.move_forward(),
            IncrementalSearchBackward => self.start_incremental_search(SearchDirection::Backward),
            IncrementalSearchForward => self.start_incremental_search(SearchDirection::Forward),
            NextLine => self.move_line(1),
            PreviousLine => self.move_line(-1),
            SaveBuffer => self.save_buffer(),
            SaveBuffersKillTerminal => return Ok(EditorOutcome::Quit),
        }?;

        Ok(EditorOutcome::Continue)
    }

    fn insert_text(&mut self, text: &str) -> Result<()> {
        self.cursor = self.document.buffer_mut().insert(self.cursor, text)?;
        self.goal_display_column = None;
        self.minibuffer.clear();
        Ok(())
    }

    fn move_backward(&mut self) -> Result<()> {
        self.cursor = self.document.buffer().move_grapheme_backward(self.cursor)?;
        self.goal_display_column = None;
        Ok(())
    }

    fn move_forward(&mut self) -> Result<()> {
        self.cursor = self.document.buffer().move_grapheme_forward(self.cursor)?;
        self.goal_display_column = None;
        Ok(())
    }

    fn move_line(&mut self, delta: isize) -> Result<()> {
        let (position, goal) =
            self.document
                .buffer()
                .move_line(self.cursor, delta, self.goal_display_column)?;
        self.cursor = position;
        self.goal_display_column = Some(goal);
        Ok(())
    }

    fn move_beginning_of_line(&mut self) -> Result<()> {
        self.cursor = Position::new(self.cursor.line, 0);
        self.goal_display_column = None;
        Ok(())
    }

    fn move_end_of_line(&mut self) -> Result<()> {
        let Some(line) = self.document.buffer().line(self.cursor.line) else {
            return Err(RileError::InvalidPosition(format!(
                "line {} is outside buffer",
                self.cursor.line
            )));
        };
        self.cursor = Position::new(self.cursor.line, line.len());
        self.goal_display_column = None;
        Ok(())
    }

    fn delete_backward_char(&mut self) -> Result<()> {
        let start = self.document.buffer().move_grapheme_backward(self.cursor)?;
        if start == self.cursor {
            return Ok(());
        }
        self.document
            .buffer_mut()
            .delete_range(TextRange::new(start, self.cursor))?;
        self.cursor = start;
        self.goal_display_column = None;
        Ok(())
    }

    fn delete_char(&mut self) -> Result<()> {
        let end = self.document.buffer().move_grapheme_forward(self.cursor)?;
        if end == self.cursor {
            return Ok(());
        }
        self.document
            .buffer_mut()
            .delete_range(TextRange::new(self.cursor, end))?;
        self.goal_display_column = None;
        Ok(())
    }

    fn save_buffer(&mut self) -> Result<()> {
        match self.document.save() {
            Ok(()) => self
                .minibuffer
                .set_message(format!("Wrote {}", self.document.display_name())),
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

    fn find_file(&mut self, path: &str) -> Result<EditorOutcome> {
        if path.is_empty() {
            self.minibuffer.set_error("missing file name");
            return Ok(EditorOutcome::Continue);
        }

        match Document::open(path) {
            Ok(document) => {
                self.document = document;
                self.cursor = Position::new(0, 0);
                self.goal_display_column = None;
                self.minibuffer
                    .set_message(format!("Opened {}", self.document.display_name()));
            }
            Err(error) => self.minibuffer.set_error(format!("open failed: {error}")),
        }
        Ok(EditorOutcome::Continue)
    }

    fn start_incremental_search(&mut self, direction: SearchDirection) -> Result<()> {
        self.search = Some(SearchState {
            direction,
            origin: self.cursor,
            current: None,
        });
        self.minibuffer
            .start_prompt(PromptKind::IncrementalSearch, direction.label());
        Ok(())
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
            self.minibuffer.set_prompt_label(direction.label());
            return Ok(());
        }

        let found = find_match(self.document.buffer(), &query, origin, direction)?;
        if let Some(range) = found {
            if let Some(search) = &mut self.search {
                search.current = Some(range);
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.minibuffer.set_prompt_label(direction.label());
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
            }
            self.cursor = origin;
            self.goal_display_column = None;
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
                search_start_after(self.document.buffer(), range.start)?
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

        let found = find_match(self.document.buffer(), &query, start, direction)?;
        if let Some(range) = found {
            if let Some(search) = &mut self.search {
                search.current = Some(range);
            }
            self.cursor = range.start;
            self.goal_display_column = None;
            self.minibuffer.set_prompt_label(direction.label());
        } else {
            if let Some(search) = &mut self.search {
                search.current = None;
            }
            self.cursor = previous_cursor;
            self.minibuffer.set_prompt_label(direction.failing_label());
        }
        Ok(())
    }

    fn clear_key_sequence(&mut self) {
        self.key_sequence.clear();
    }
}

impl DecorationProvider for Editor {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span> {
        let Some(search) = &self.search else {
            return Vec::new();
        };
        let Some(query) = self.minibuffer.prompt_input() else {
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
                Span {
                    start_byte: start,
                    end_byte: end,
                    face,
                }
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
    use crate::file::Document;
    use crate::input::{KeyEvent, SpecialKey};
    use crate::render::{DecorationProvider, Face};

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
