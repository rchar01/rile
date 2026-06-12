// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buffer::{Position, TextRange};
use crate::command::{Command, CommandRegistry};
use crate::file::Document;
use crate::input::{KeyEvent, SpecialKey};
use crate::keymap::{KeyMap, KeyResolution};
use crate::minibuffer::MinibufferState;
use crate::{Result, RileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorOutcome {
    Continue,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Normal,
    ExtendedCommand { input: String },
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
    input_mode: InputMode,
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
            input_mode: InputMode::Normal,
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
        if matches!(self.input_mode, InputMode::ExtendedCommand { .. }) {
            return self.handle_extended_command_key(key);
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

    fn handle_extended_command_key(&mut self, key: KeyEvent) -> Result<EditorOutcome> {
        match key {
            KeyEvent::Special(SpecialKey::Enter) => {
                let InputMode::ExtendedCommand { input } =
                    std::mem::replace(&mut self.input_mode, InputMode::Normal)
                else {
                    unreachable!("extended command mode checked above")
                };
                self.minibuffer.clear();
                self.execute_command_by_name(input.trim())
            }
            KeyEvent::Special(SpecialKey::Escape) | KeyEvent::Ctrl('g') => {
                self.input_mode = InputMode::Normal;
                self.minibuffer.set_message("Quit");
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Special(SpecialKey::Backspace) => {
                if let InputMode::ExtendedCommand { input } = &mut self.input_mode {
                    input.pop();
                    self.minibuffer.set_message(format!("M-x {input}"));
                }
                Ok(EditorOutcome::Continue)
            }
            KeyEvent::Text(text) => {
                if let InputMode::ExtendedCommand { input } = &mut self.input_mode {
                    input.push_str(&text);
                    self.minibuffer.set_message(format!("M-x {input}"));
                }
                Ok(EditorOutcome::Continue)
            }
            _ => Ok(EditorOutcome::Continue),
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
            ForwardChar => self.move_forward(),
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
            Ok(()) => self.minibuffer.set_message("Wrote file"),
            Err(error) => self.minibuffer.set_message(format!("Save failed: {error}")),
        }
        Ok(())
    }

    fn start_extended_command(&mut self) -> Result<()> {
        self.input_mode = InputMode::ExtendedCommand {
            input: String::new(),
        };
        self.minibuffer.set_message("M-x ");
        Ok(())
    }

    fn clear_key_sequence(&mut self) {
        self.key_sequence.clear();
    }
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
}
