// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MinibufferState {
    pub message: Option<String>,
    prompt: Option<PromptState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    ExtendedCommand,
    FindFile,
    IncrementalSearch,
    KillBuffer,
    SwitchToBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptState {
    pub kind: PromptKind,
    pub label: String,
    pub input: String,
}

impl MinibufferState {
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
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
            prompt.input.push_str(text);
        }
    }

    pub fn delete_prompt_grapheme_backward(&mut self) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        let Some((byte, _)) = prompt.input.grapheme_indices(true).next_back() else {
            return;
        };
        prompt.input.truncate(byte);
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
    fn cancelling_prompt_sets_quit_message() {
        let mut minibuffer = MinibufferState::default();

        minibuffer.start_prompt(PromptKind::FindFile, "Find file: ");
        assert!(minibuffer.cancel_prompt());

        assert_eq!(minibuffer.prompt(), None);
        assert_eq!(minibuffer.message.as_deref(), Some("Quit"));
    }
}
