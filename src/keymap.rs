// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
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

    pub fn bindings_starting_with(&self, prefix: &[KeyEvent]) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|binding| {
                binding.sequence.len() > prefix.len() && binding.sequence.starts_with(prefix)
            })
            .collect()
    }

    pub fn binding_for_sequence(&self, sequence: &[KeyEvent]) -> Option<&KeyBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.sequence == sequence)
    }

    pub fn bindings_for_command(&self, command: &str) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|binding| binding.command == command)
            .collect()
    }
}

pub fn default_bindings() -> Vec<KeyBinding> {
    use crate::input::SpecialKey;

    vec![
        KeyBinding::new([KeyEvent::Ctrl('b')], "backward-char"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowLeft)], "backward-char"),
        KeyBinding::new([KeyEvent::Ctrl('f')], "forward-char"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowRight)], "forward-char"),
        KeyBinding::new([KeyEvent::Meta('b')], "backward-word"),
        KeyBinding::new([KeyEvent::Meta('f')], "forward-word"),
        KeyBinding::new([KeyEvent::Meta('^')], "join-line"),
        KeyBinding::new([KeyEvent::Meta('d')], "kill-word"),
        KeyBinding::new([KeyEvent::Meta('m')], "back-to-indentation"),
        KeyBinding::new(
            [KeyEvent::MetaSpecial(SpecialKey::Backspace)],
            "backward-kill-word",
        ),
        KeyBinding::new([KeyEvent::Meta('<')], "beginning-of-buffer"),
        KeyBinding::new([KeyEvent::Meta('>')], "end-of-buffer"),
        KeyBinding::new([KeyEvent::Ctrl('p')], "previous-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowUp)], "previous-line"),
        KeyBinding::new([KeyEvent::Ctrl('n')], "next-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowDown)], "next-line"),
        KeyBinding::new([KeyEvent::Ctrl('v')], "scroll-page-forward"),
        KeyBinding::new([KeyEvent::Meta('v')], "scroll-page-backward"),
        KeyBinding::new(
            [KeyEvent::Special(SpecialKey::PageDown)],
            "scroll-page-forward",
        ),
        KeyBinding::new(
            [KeyEvent::Special(SpecialKey::PageUp)],
            "scroll-page-backward",
        ),
        KeyBinding::new([KeyEvent::Ctrl('a')], "beginning-of-line"),
        KeyBinding::new([KeyEvent::Ctrl('l')], "recenter"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Home)], "beginning-of-line"),
        KeyBinding::new([KeyEvent::Ctrl('e')], "end-of-line"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::End)], "end-of-line"),
        KeyBinding::new([KeyEvent::Ctrl('d')], "delete-char"),
        KeyBinding::new([KeyEvent::Ctrl('j')], "newline-and-indent"),
        KeyBinding::new([KeyEvent::Ctrl('o')], "open-line"),
        KeyBinding::new([KeyEvent::Ctrl('q')], "quoted-insert"),
        KeyBinding::new([KeyEvent::Ctrl('u')], "universal-argument"),
        KeyBinding::new([KeyEvent::Ctrl('@')], "set-mark-command"),
        KeyBinding::new(
            [KeyEvent::Ctrl('h'), KeyEvent::Text("f".to_owned())],
            "describe-function",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('h'), KeyEvent::Text("k".to_owned())],
            "describe-key",
        ),
        KeyBinding::new([KeyEvent::Ctrl('_')], "undo"),
        KeyBinding::new([KeyEvent::Ctrl('k')], "kill-line"),
        KeyBinding::new([KeyEvent::Ctrl('s')], "isearch-forward"),
        KeyBinding::new([KeyEvent::Ctrl('w')], "kill-region"),
        KeyBinding::new([KeyEvent::Ctrl('r')], "isearch-backward"),
        KeyBinding::new([KeyEvent::Ctrl('y')], "yank"),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Delete)], "delete-char"),
        KeyBinding::new(
            [KeyEvent::Special(SpecialKey::Backspace)],
            "delete-backward-char",
        ),
        KeyBinding::new([KeyEvent::Meta('x')], "execute-extended-command"),
        KeyBinding::new([KeyEvent::Meta('y')], "yank-pop"),
        KeyBinding::new(
            [KeyEvent::Meta('g'), KeyEvent::Text("g".to_owned())],
            "goto-line",
        ),
        KeyBinding::new([KeyEvent::Meta('%')], "query-replace"),
        KeyBinding::new([KeyEvent::Meta('w')], "copy-region-as-kill"),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("0".to_owned())],
            "delete-window",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("1".to_owned())],
            "delete-other-windows",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("2".to_owned())],
            "split-window-below",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("3".to_owned())],
            "split-window-right",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("b".to_owned())],
            "switch-to-buffer",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("i".to_owned())],
            "insert-file",
        ),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('f')], "find-file"),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('b')], "list-buffers"),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('q')],
            "toggle-read-only",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('r')],
            "find-file-read-only",
        ),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('w')], "write-file"),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("(".to_owned())],
            "start-kbd-macro",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text(")".to_owned())],
            "end-kbd-macro",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("e".to_owned())],
            "call-last-kbd-macro",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("h".to_owned())],
            "mark-whole-buffer",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text(" ".to_owned())],
            "rectangle-mark-mode",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("N".to_owned()),
            ],
            "rectangle-number-lines",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("c".to_owned()),
            ],
            "clear-rectangle",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("d".to_owned()),
            ],
            "delete-rectangle",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("k".to_owned()),
            ],
            "kill-rectangle",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Meta('w'),
            ],
            "copy-rectangle-as-kill",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("o".to_owned()),
            ],
            "open-rectangle",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("t".to_owned()),
            ],
            "string-rectangle",
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("y".to_owned()),
            ],
            "yank-rectangle",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('x')],
            "exchange-point-and-mark",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("k".to_owned())],
            "kill-buffer",
        ),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')], "save-buffer"),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("o".to_owned())],
            "other-window",
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('c')],
            "save-buffers-kill-terminal",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::{KeyMap, KeyResolution};
    use crate::input::{KeyEvent, SpecialKey};

    #[test]
    fn resolves_single_key_bindings() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('f')]),
            KeyResolution::Command("forward-char")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('f')]),
            KeyResolution::Command("forward-word")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('b')]),
            KeyResolution::Command("backward-word")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('d')]),
            KeyResolution::Command("kill-word")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('m')]),
            KeyResolution::Command("back-to-indentation")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::MetaSpecial(SpecialKey::Backspace)]),
            KeyResolution::Command("backward-kill-word")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('<')]),
            KeyResolution::Command("beginning-of-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('>')]),
            KeyResolution::Command("end-of-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('s')]),
            KeyResolution::Command("isearch-forward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('v')]),
            KeyResolution::Command("scroll-page-forward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('v')]),
            KeyResolution::Command("scroll-page-backward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Special(SpecialKey::PageDown)]),
            KeyResolution::Command("scroll-page-forward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Special(SpecialKey::PageUp)]),
            KeyResolution::Command("scroll-page-backward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('l')]),
            KeyResolution::Command("recenter")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('r')]),
            KeyResolution::Command("isearch-backward")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('k')]),
            KeyResolution::Command("kill-line")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('j')]),
            KeyResolution::Command("newline-and-indent")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('o')]),
            KeyResolution::Command("open-line")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('^')]),
            KeyResolution::Command("join-line")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('q')]),
            KeyResolution::Command("quoted-insert")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('w')]),
            KeyResolution::Command("kill-region")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('y')]),
            KeyResolution::Command("yank")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('_')]),
            KeyResolution::Command("undo")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('@')]),
            KeyResolution::Command("set-mark-command")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('w')]),
            KeyResolution::Command("copy-region-as-kill")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('%')]),
            KeyResolution::Command("query-replace")
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
            keymap.resolve(&[KeyEvent::Meta('g')]),
            KeyResolution::Prefix
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('h')]),
            KeyResolution::Prefix
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('h'), KeyEvent::Text("f".to_owned())]),
            KeyResolution::Command("describe-function")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('h'), KeyEvent::Text("k".to_owned())]),
            KeyResolution::Command("describe-key")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('g'), KeyEvent::Text("g".to_owned())]),
            KeyResolution::Command("goto-line")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')]),
            KeyResolution::Command("save-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('f')]),
            KeyResolution::Command("find-file")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('b')]),
            KeyResolution::Command("list-buffers")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('r')]),
            KeyResolution::Command("find-file-read-only")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('q')]),
            KeyResolution::Command("toggle-read-only")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('w')]),
            KeyResolution::Command("write-file")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("(".to_owned())]),
            KeyResolution::Command("start-kbd-macro")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text(")".to_owned())]),
            KeyResolution::Command("end-kbd-macro")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("e".to_owned())]),
            KeyResolution::Command("call-last-kbd-macro")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("h".to_owned())]),
            KeyResolution::Command("mark-whole-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text(" ".to_owned())]),
            KeyResolution::Command("rectangle-mark-mode")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("r".to_owned())]),
            KeyResolution::Prefix
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("N".to_owned())
            ]),
            KeyResolution::Command("rectangle-number-lines")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("c".to_owned())
            ]),
            KeyResolution::Command("clear-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("d".to_owned())
            ]),
            KeyResolution::Command("delete-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("k".to_owned())
            ]),
            KeyResolution::Command("kill-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Meta('w')
            ]),
            KeyResolution::Command("copy-rectangle-as-kill")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("o".to_owned())
            ]),
            KeyResolution::Command("open-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("t".to_owned())
            ]),
            KeyResolution::Command("string-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("y".to_owned())
            ]),
            KeyResolution::Command("yank-rectangle")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('x')]),
            KeyResolution::Command("exchange-point-and-mark")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("b".to_owned())]),
            KeyResolution::Command("switch-to-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("i".to_owned())]),
            KeyResolution::Command("insert-file")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("k".to_owned())]),
            KeyResolution::Command("kill-buffer")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("2".to_owned())]),
            KeyResolution::Command("split-window-below")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("3".to_owned())]),
            KeyResolution::Command("split-window-right")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("0".to_owned())]),
            KeyResolution::Command("delete-window")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("1".to_owned())]),
            KeyResolution::Command("delete-other-windows")
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("o".to_owned())]),
            KeyResolution::Command("other-window")
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

    #[test]
    fn lists_bindings_starting_with_prefix() {
        let keymap = KeyMap::default();
        let bindings = keymap.bindings_starting_with(&[KeyEvent::Meta('g')]);

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].command, "goto-line");
        assert_eq!(
            bindings[0].sequence,
            vec![KeyEvent::Meta('g'), KeyEvent::Text("g".to_owned())]
        );
    }

    #[test]
    fn finds_bindings_by_sequence_and_command() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap
                .binding_for_sequence(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('f')])
                .map(|binding| binding.command),
            Some("find-file")
        );
        assert_eq!(keymap.bindings_for_command("find-file").len(), 1);
        assert!(keymap.bindings_for_command("missing-command").is_empty());
    }
}
