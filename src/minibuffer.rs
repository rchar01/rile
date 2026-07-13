// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use unicode_segmentation::UnicodeSegmentation;

use crate::text::{move_word_backward_byte, move_word_forward_byte};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MinibufferState {
    pub message: Option<String>,
    messages: Vec<String>,
    prompt: Option<PromptState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    DescribeFunction,
    DescribeVariable,
    ExtendedCommand,
    FindFile,
    FindFileReadOnly,
    GotoLine,
    HighlightLinesMatchingRegexp,
    HighlightPhrase,
    HighlightRegexp,
    InsertFile,
    IncrementalSearch,
    KillBuffer,
    KillDirtyBuffer,
    QueryReplaceRegexpReplacement,
    QueryReplaceRegexpSearch,
    QueryReplaceReplacement,
    QueryReplaceSearch,
    ReplaceRegexpReplacement,
    ReplaceRegexpSearch,
    RevertBuffer,
    SaveSomeBuffers,
    QuitDirtyBuffers,
    RectangleNumberFormat,
    RectangleNumberStart,
    ShellCommand,
    StringRectangle,
    SwitchToBuffer,
    UnhighlightRegexp,
    WriteFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptState {
    pub kind: PromptKind,
    pub label: String,
    pub input: String,
    pub cursor_byte: usize,
}

impl MinibufferState {
    pub fn set_message(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.messages.push(message.clone());
        self.message = Some(message);
        self.prompt = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.set_message(format!("Error: {}", message.into()));
    }

    pub fn start_prompt(&mut self, kind: PromptKind, label: impl Into<String>) {
        self.message = None;
        self.prompt = Some(PromptState {
            kind,
            label: label.into(),
            input: String::new(),
            cursor_byte: 0,
        });
    }

    pub fn prompt(&self) -> Option<&PromptState> {
        self.prompt.as_ref()
    }

    pub fn prompt_kind(&self) -> Option<PromptKind> {
        self.prompt.as_ref().map(|prompt| prompt.kind)
    }

    pub fn prompt_input(&self) -> Option<&str> {
        self.prompt.as_ref().map(|prompt| prompt.input.as_str())
    }

    pub fn set_prompt_label(&mut self, label: impl Into<String>) {
        if let Some(prompt) = &mut self.prompt {
            prompt.label = label.into();
        }
    }

    pub fn insert_prompt_text(&mut self, text: &str) {
        if let Some(prompt) = &mut self.prompt {
            prompt.input.insert_str(prompt.cursor_byte, text);
            prompt.cursor_byte += text.len();
        }
    }

    pub fn set_prompt_input(&mut self, input: impl Into<String>) {
        if let Some(prompt) = &mut self.prompt {
            prompt.input = input.into();
            prompt.cursor_byte = prompt.input.len();
        }
    }

    pub fn delete_prompt_grapheme_backward(&mut self) -> bool {
        let Some(prompt) = &mut self.prompt else {
            return false;
        };
        let Some((byte, _)) = prompt.input[..prompt.cursor_byte]
            .grapheme_indices(true)
            .next_back()
        else {
            return false;
        };
        prompt.input.drain(byte..prompt.cursor_byte);
        prompt.cursor_byte = byte;
        true
    }

    pub fn delete_prompt_grapheme_forward(&mut self) -> bool {
        let Some(prompt) = &mut self.prompt else {
            return false;
        };
        let Some(grapheme) = prompt.input[prompt.cursor_byte..].graphemes(true).next() else {
            return false;
        };
        let end = prompt.cursor_byte + grapheme.len();
        prompt.input.drain(prompt.cursor_byte..end);
        true
    }

    pub fn move_prompt_grapheme_forward(&mut self) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        let Some(grapheme) = prompt.input[prompt.cursor_byte..].graphemes(true).next() else {
            return;
        };
        prompt.cursor_byte += grapheme.len();
    }

    pub fn move_prompt_grapheme_backward(&mut self) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        let Some((byte, _)) = prompt.input[..prompt.cursor_byte]
            .grapheme_indices(true)
            .next_back()
        else {
            return;
        };
        prompt.cursor_byte = byte;
    }

    pub fn move_prompt_word_forward(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.cursor_byte = move_word_forward_byte(&prompt.input, prompt.cursor_byte);
        }
    }

    pub fn move_prompt_word_backward(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.cursor_byte = move_word_backward_byte(&prompt.input, prompt.cursor_byte);
        }
    }

    pub fn move_prompt_start(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.cursor_byte = 0;
        }
    }

    pub fn move_prompt_end(&mut self) {
        if let Some(prompt) = &mut self.prompt {
            prompt.cursor_byte = prompt.input.len();
        }
    }

    pub fn delete_prompt_to_end(&mut self) -> Option<String> {
        let prompt = self.prompt.as_mut()?;
        if prompt.cursor_byte >= prompt.input.len() {
            return None;
        }
        Some(prompt.input.drain(prompt.cursor_byte..).collect())
    }

    pub fn delete_prompt_word_forward(&mut self) -> Option<String> {
        let prompt = self.prompt.as_mut()?;
        let end = move_word_forward_byte(&prompt.input, prompt.cursor_byte);
        if end == prompt.cursor_byte {
            return None;
        }
        Some(prompt.input.drain(prompt.cursor_byte..end).collect())
    }

    pub fn delete_prompt_word_backward(&mut self) -> Option<String> {
        let prompt = self.prompt.as_mut()?;
        let start = move_word_backward_byte(&prompt.input, prompt.cursor_byte);
        if start == prompt.cursor_byte {
            return None;
        }
        let text = prompt.input.drain(start..prompt.cursor_byte).collect();
        prompt.cursor_byte = start;
        Some(text)
    }

    pub fn prompt_input_before_cursor(&self) -> Option<&str> {
        self.prompt
            .as_ref()
            .map(|prompt| &prompt.input[..prompt.cursor_byte])
    }

    pub fn take_prompt_input(&mut self) -> Option<(PromptKind, String)> {
        self.prompt.take().map(|prompt| (prompt.kind, prompt.input))
    }

    pub fn cancel_prompt(&mut self) -> bool {
        let had_prompt = self.prompt.take().is_some();
        if had_prompt {
            self.set_message("Quit");
        }
        had_prompt
    }

    pub fn display_text(&self) -> Option<String> {
        if let Some(prompt) = &self.prompt {
            return Some(format!("{}{}", prompt.label, prompt.input));
        }
        self.message.clone()
    }

    pub fn messages_text(&self) -> String {
        if self.messages.is_empty() {
            return "No messages.\n".to_owned();
        }

        let mut text = self.messages.join("\n");
        text.push('\n');
        text
    }

    pub fn clear(&mut self) {
        self.message = None;
        self.prompt = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{MinibufferState, PromptKind};

    #[test]
    fn prompt_display_combines_label_and_input() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ExtendedCommand, "M-x ");
        minibuffer.insert_prompt_text("save-buffer");

        assert_eq!(
            minibuffer.display_text().as_deref(),
            Some("M-x save-buffer")
        );
        assert_eq!(minibuffer.prompt_kind(), Some(PromptKind::ExtendedCommand));
        assert_eq!(minibuffer.prompt_input(), Some("save-buffer"));
    }

    #[test]
    fn prompt_backspace_removes_graphemes() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::FindFile, "Find file: ");
        minibuffer.insert_prompt_text("e\u{301}x");
        minibuffer.delete_prompt_grapheme_backward();
        minibuffer.delete_prompt_grapheme_backward();

        assert_eq!(minibuffer.prompt_input(), Some(""));
    }

    #[test]
    fn prompt_insert_and_backspace_use_cursor() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ExtendedCommand, "M-x ");
        minibuffer.insert_prompt_text("ac");
        minibuffer.move_prompt_grapheme_backward();
        minibuffer.insert_prompt_text("b");
        minibuffer.delete_prompt_grapheme_backward();
        minibuffer.insert_prompt_text("B");

        assert_eq!(minibuffer.prompt_input(), Some("aBc"));
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("aB"));
    }

    #[test]
    fn prompt_grapheme_movement_is_unicode_safe() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::FindFile, "Find file: ");
        minibuffer.insert_prompt_text("e\u{301}x");
        minibuffer.move_prompt_grapheme_backward();
        minibuffer.move_prompt_grapheme_backward();

        assert_eq!(minibuffer.prompt_input_before_cursor(), Some(""));

        minibuffer.move_prompt_grapheme_forward();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("e\u{301}"));
    }

    #[test]
    fn prompt_word_movement_uses_shared_boundaries() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ExtendedCommand, "M-x ");
        minibuffer.insert_prompt_text("one two_three");
        minibuffer.move_prompt_word_backward();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("one "));
        minibuffer.move_prompt_word_backward();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some(""));
        minibuffer.move_prompt_word_forward();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("one"));
    }

    #[test]
    fn prompt_word_movement_preserves_grapheme_boundaries() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ExtendedCommand, "M-x ");
        minibuffer.insert_prompt_text("e\u{301} next");
        while minibuffer.prompt_input_before_cursor() != Some("") {
            minibuffer.move_prompt_grapheme_backward();
        }

        minibuffer.move_prompt_word_forward();

        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("e\u{301}"));
    }

    #[test]
    fn prompt_start_end_and_forward_delete_use_cursor() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ExtendedCommand, "M-x ");
        minibuffer.insert_prompt_text("abc");
        minibuffer.move_prompt_start();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some(""));

        assert!(minibuffer.delete_prompt_grapheme_forward());
        assert_eq!(minibuffer.prompt_input(), Some("bc"));

        minibuffer.move_prompt_end();
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("bc"));
        assert!(!minibuffer.delete_prompt_grapheme_forward());
    }

    #[test]
    fn prompt_forward_delete_is_grapheme_safe() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::FindFile, "Find file: ");
        minibuffer.insert_prompt_text("e\u{301}x");
        minibuffer.move_prompt_start();

        assert!(minibuffer.delete_prompt_grapheme_forward());
        assert_eq!(minibuffer.prompt_input(), Some("x"));
    }

    #[test]
    fn prompt_kill_methods_return_deleted_text() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::ShellCommand, "Shell command: ");
        minibuffer.insert_prompt_text("one two three");
        minibuffer.move_prompt_word_backward();
        assert_eq!(
            minibuffer.delete_prompt_word_backward(),
            Some("two ".to_owned())
        );
        assert_eq!(minibuffer.prompt_input(), Some("one three"));
        assert_eq!(minibuffer.prompt_input_before_cursor(), Some("one "));

        assert_eq!(
            minibuffer.delete_prompt_word_forward(),
            Some("three".to_owned())
        );
        assert_eq!(minibuffer.prompt_input(), Some("one "));

        minibuffer.move_prompt_start();
        assert_eq!(minibuffer.delete_prompt_to_end(), Some("one ".to_owned()));
        assert_eq!(minibuffer.prompt_input(), Some(""));
        assert_eq!(minibuffer.delete_prompt_to_end(), None);
    }

    #[test]
    fn cancelling_prompt_sets_quit_message() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::FindFile, "Find file: ");
        assert!(minibuffer.cancel_prompt());

        assert_eq!(minibuffer.prompt(), None);
        assert_eq!(minibuffer.message.as_deref(), Some("Quit"));
        assert_eq!(minibuffer.messages_text(), "Quit\n");
    }

    #[test]
    fn messages_text_keeps_status_history() {
        let mut minibuffer = MinibufferState::default();

        assert_eq!(minibuffer.messages_text(), "No messages.\n");
        minibuffer.set_message("Saved alpha.txt");
        minibuffer.set_error("missing file name");
        minibuffer.clear();

        assert_eq!(
            minibuffer.messages_text(),
            "Saved alpha.txt\nError: missing file name\n"
        );
        assert_eq!(minibuffer.display_text(), None);
    }
}
