// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::input::KeyEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub sequence: Vec<KeyEvent>,
    pub command: &'static str,
}

impl KeyBinding {
    pub fn new(sequence: impl Into<Vec<KeyEvent>>, command: &'static str) -> Self {
        Self {
            sequence: sequence.into(),
            command,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResolution {
    NoMatch,
    Prefix,
    Command(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMap {
    bindings: Vec<KeyBinding>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new(default_bindings())
    }
}

impl KeyMap {
    pub fn new(bindings: impl Into<Vec<KeyBinding>>) -> Self {
        Self {
            bindings: bindings.into(),
        }
    }

    pub fn resolve(&self, sequence: &[KeyEvent]) -> KeyResolution {
        let mut has_prefix = false;

        for binding in &self.bindings {
            if binding.sequence == sequence {
                return KeyResolution::Command(binding.command);
            }
            if binding.sequence.starts_with(sequence) {
                has_prefix = true;
            }
        }

        if has_prefix {
            KeyResolution::Prefix
        } else {
            KeyResolution::NoMatch
        }
    }
}

pub fn default_bindings() -> Vec<KeyBinding> {
    use crate::input::SpecialKey;

    vec![
        KeyBinding::new([KeyEvent::Ctrl('b')], "backward-char"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowLeft)], "backward-char"),
        KeyBinding::new([KeyEvent::Ctrl('f')], "forward-char"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowRight)], "forward-char"),
        KeyBinding::new([KeyEvent::Ctrl('p')], "previous-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowUp)], "previous-line"),
        KeyBinding::new([KeyEvent::Ctrl('n')], "next-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowDown)], "next-line"),
        KeyBinding::new([KeyEvent::Ctrl('a')], "beginning-of-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Home)], "beginning-of-line"),
        KeyBinding::new([KeyEvent::Ctrl('e')], "end-of-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::End)], "end-of-line"),
        KeyBinding::new([KeyEvent::Ctrl('d')], "delete-char"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Delete)], "delete-char"),
        KeyBinding::new(
            [KeyEvent::Special(SpecialKey::Backspace)],
            "delete-backward-char",
        ),
        KeyBinding::new([KeyEvent::Meta('x')], "execute-extended-command"),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')], "save-buffer"),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('c')],
            "save-buffers-kill-terminal",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{KeyMap, KeyResolution};
    use crate::input::KeyEvent;

    #[test]
    fn resolves_single_key_bindings() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('f')]),
            KeyResolution::Command("forward-char")
        );
    }

    #[test]
    fn resolves_prefix_and_complete_binding() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x')]),
            KeyResolution::Prefix
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')]),
            KeyResolution::Command("save-buffer")
        );
    }

    #[test]
    fn rejects_unbound_sequence() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('z')]),
            KeyResolution::NoMatch
        );
    }
}
