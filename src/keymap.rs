// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::command::{Command, CommandId};
use crate::input::{KeyEvent, SpecialKey};

pub type KeySeq = Vec<KeyEvent>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyMapId {
    Minibuffer,
    Transient,
    SpecialBuffer,
    MinorMode,
    MajorMode,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingTarget {
    Command(CommandId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub sequence: KeySeq,
    pub target: BindingTarget,
}

impl KeyBinding {
    pub fn new(sequence: impl Into<KeySeq>, command: CommandId) -> Self {
        Self {
            sequence: sequence.into(),
            target: BindingTarget::Command(command),
        }
    }

    pub fn command(&self) -> CommandId {
        match self.target {
            BindingTarget::Command(command) => command,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResolution {
    NoMatch,
    Prefix,
    Command(CommandId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStackResolution {
    NoMatch,
    Prefix,
    Command {
        keymap: KeyMapId,
        command: CommandId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStackBinding<'a> {
    pub keymap: KeyMapId,
    pub keymap_name: &'static str,
    pub binding: &'a KeyBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMap {
    id: KeyMapId,
    name: &'static str,
    bindings: Vec<KeyBinding>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new(default_bindings())
    }
}

impl KeyMap {
    pub fn new(bindings: impl Into<Vec<KeyBinding>>) -> Self {
        Self::named(KeyMapId::Global, "global-map", bindings)
    }

    pub fn named(id: KeyMapId, name: &'static str, bindings: impl Into<Vec<KeyBinding>>) -> Self {
        Self {
            id,
            name,
            bindings: bindings.into(),
        }
    }

    pub fn id(&self) -> KeyMapId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn resolve(&self, sequence: &[KeyEvent]) -> KeyResolution {
        let mut has_prefix = false;

        for binding in &self.bindings {
            if binding.sequence == sequence {
                return KeyResolution::Command(binding.command());
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

    pub fn bindings_for_command(&self, command: CommandId) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|binding| binding.command() == command)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMapStack<'a> {
    maps: Vec<&'a KeyMap>,
}

impl<'a> KeyMapStack<'a> {
    pub fn new(maps: impl Into<Vec<&'a KeyMap>>) -> Self {
        Self { maps: maps.into() }
    }

    pub fn global(global: &'a KeyMap) -> Self {
        Self::new([global])
    }

    pub fn maps(&self) -> &[&'a KeyMap] {
        &self.maps
    }

    pub fn resolve(&self, sequence: &[KeyEvent]) -> KeyStackResolution {
        for &keymap in &self.maps {
            match keymap.resolve(sequence) {
                KeyResolution::Command(command) => {
                    return KeyStackResolution::Command {
                        keymap: keymap.id(),
                        command,
                    };
                }
                KeyResolution::Prefix => return KeyStackResolution::Prefix,
                KeyResolution::NoMatch => {}
            }
        }

        KeyStackResolution::NoMatch
    }

    pub fn bindings_starting_with(&self, prefix: &[KeyEvent]) -> Vec<KeyStackBinding<'a>> {
        let mut bindings = Vec::new();
        for &keymap in &self.maps {
            bindings.extend(
                keymap
                    .bindings_starting_with(prefix)
                    .into_iter()
                    .filter(|binding| self.binding_is_active(keymap, binding))
                    .map(|binding| KeyStackBinding {
                        keymap: keymap.id(),
                        keymap_name: keymap.name(),
                        binding,
                    }),
            );
        }
        bindings
    }

    pub fn bindings_for_command(&self, command: CommandId) -> Vec<KeyStackBinding<'a>> {
        let mut bindings = Vec::new();
        for &keymap in &self.maps {
            bindings.extend(
                keymap
                    .bindings_for_command(command)
                    .into_iter()
                    .filter(|binding| self.binding_is_active(keymap, binding))
                    .map(|binding| KeyStackBinding {
                        keymap: keymap.id(),
                        keymap_name: keymap.name(),
                        binding,
                    }),
            );
        }
        bindings
    }

    pub fn keymap_name(&self, id: KeyMapId) -> Option<&'static str> {
        self.maps()
            .iter()
            .find(|keymap| keymap.id() == id)
            .map(|keymap| keymap.name())
    }

    fn binding_is_active(&self, keymap: &KeyMap, binding: &KeyBinding) -> bool {
        matches!(
            self.resolve(&binding.sequence),
            KeyStackResolution::Command {
                keymap: resolved_keymap,
                command,
            } if resolved_keymap == keymap.id() && command == binding.command()
        )
    }
}

pub(crate) fn format_key_sequence(sequence: &[KeyEvent]) -> String {
    sequence
        .iter()
        .map(format_key_event)
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_key_event(key: &KeyEvent) -> String {
    match key {
        KeyEvent::Ctrl(character) => format!("C-{character}"),
        KeyEvent::Meta(character) => format!("M-{character}"),
        KeyEvent::MetaSpecial(special) => format!("M-{}", format_special_key(*special)),
        KeyEvent::Text(text) if text == " " => "SPC".to_owned(),
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

pub fn default_bindings() -> Vec<KeyBinding> {
    use crate::input::SpecialKey;
    use Command::*;

    vec![
        KeyBinding::new([KeyEvent::Ctrl('b')], BackwardChar),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowLeft)], BackwardChar),
        KeyBinding::new([KeyEvent::Ctrl('f')], ForwardChar),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowRight)], ForwardChar),
        KeyBinding::new([KeyEvent::Meta('b')], BackwardWord),
        KeyBinding::new([KeyEvent::Meta('f')], ForwardWord),
        KeyBinding::new([KeyEvent::Meta('^')], JoinLine),
        KeyBinding::new([KeyEvent::Meta('d')], KillWord),
        KeyBinding::new([KeyEvent::Meta('m')], BackToIndentation),
        KeyBinding::new(
            [KeyEvent::MetaSpecial(SpecialKey::Backspace)],
            BackwardKillWord,
        ),
        KeyBinding::new([KeyEvent::Meta('<')], BeginningOfBuffer),
        KeyBinding::new([KeyEvent::Meta('>')], EndOfBuffer),
        KeyBinding::new([KeyEvent::Ctrl('p')], PreviousLine),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowUp)], PreviousLine),
        KeyBinding::new([KeyEvent::Ctrl('n')], NextLine),
        KeyBinding::new([KeyEvent::Special(SpecialKey::ArrowDown)], NextLine),
        KeyBinding::new([KeyEvent::Ctrl('v')], ScrollPageForward),
        KeyBinding::new([KeyEvent::Meta('v')], ScrollPageBackward),
        KeyBinding::new([KeyEvent::Special(SpecialKey::PageDown)], ScrollPageForward),
        KeyBinding::new([KeyEvent::Special(SpecialKey::PageUp)], ScrollPageBackward),
        KeyBinding::new([KeyEvent::Ctrl('a')], BeginningOfLine),
        KeyBinding::new([KeyEvent::Ctrl('l')], Recenter),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Home)], BeginningOfLine),
        KeyBinding::new([KeyEvent::Ctrl('e')], EndOfLine),
        KeyBinding::new([KeyEvent::Special(SpecialKey::End)], EndOfLine),
        KeyBinding::new([KeyEvent::Ctrl('d')], DeleteChar),
        KeyBinding::new([KeyEvent::Ctrl('j')], NewlineAndIndent),
        KeyBinding::new([KeyEvent::Ctrl('o')], OpenLine),
        KeyBinding::new([KeyEvent::Ctrl('q')], QuotedInsert),
        KeyBinding::new([KeyEvent::Ctrl('u')], UniversalArgument),
        KeyBinding::new([KeyEvent::Ctrl('@')], SetMarkCommand),
        KeyBinding::new(
            [KeyEvent::Ctrl('h'), KeyEvent::Text("e".to_owned())],
            ViewEchoAreaMessages,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('h'), KeyEvent::Text("f".to_owned())],
            DescribeFunction,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('h'), KeyEvent::Text("k".to_owned())],
            DescribeKey,
        ),
        KeyBinding::new([KeyEvent::Ctrl('_')], Undo),
        KeyBinding::new([KeyEvent::Ctrl('k')], KillLine),
        KeyBinding::new([KeyEvent::Ctrl('s')], IncrementalSearchForward),
        KeyBinding::new([KeyEvent::Ctrl('w')], KillRegion),
        KeyBinding::new([KeyEvent::Ctrl('r')], IncrementalSearchBackward),
        KeyBinding::new([KeyEvent::Ctrl('y')], Yank),
        KeyBinding::new([KeyEvent::Special(SpecialKey::Delete)], DeleteChar),
        KeyBinding::new(
            [KeyEvent::Special(SpecialKey::Backspace)],
            DeleteBackwardChar,
        ),
        KeyBinding::new([KeyEvent::Meta('x')], ExecuteExtendedCommand),
        KeyBinding::new([KeyEvent::Meta('y')], YankPop),
        KeyBinding::new(
            [KeyEvent::Meta('g'), KeyEvent::Text("g".to_owned())],
            GotoLine,
        ),
        KeyBinding::new([KeyEvent::Meta('%')], QueryReplace),
        KeyBinding::new([KeyEvent::Meta('!')], ShellCommand),
        KeyBinding::new([KeyEvent::Meta('|')], ShellCommandOnRegion),
        KeyBinding::new([KeyEvent::Meta('w')], CopyRegionAsKill),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("0".to_owned())],
            DeleteWindow,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("1".to_owned())],
            DeleteOtherWindows,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("2".to_owned())],
            SplitWindowBelow,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("3".to_owned())],
            SplitWindowRight,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("b".to_owned())],
            SwitchToBuffer,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("i".to_owned())],
            InsertFile,
        ),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('f')], FindFile),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('b')], ListBuffers),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('q')], ToggleReadOnly),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('r')], FindFileReadOnly),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('w')], WriteFile),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("(".to_owned())],
            StartKeyboardMacro,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text(")".to_owned())],
            EndKeyboardMacro,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("e".to_owned())],
            CallLastKeyboardMacro,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("h".to_owned())],
            MarkWholeBuffer,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text(" ".to_owned())],
            RectangleMarkMode,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text(" ".to_owned()),
            ],
            PointToRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("+".to_owned()),
            ],
            IncrementRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("N".to_owned()),
            ],
            RectangleNumberLines,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("c".to_owned()),
            ],
            ClearRectangle,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("d".to_owned()),
            ],
            DeleteRectangle,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("i".to_owned()),
            ],
            InsertRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("j".to_owned()),
            ],
            JumpToRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("k".to_owned()),
            ],
            KillRectangle,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("n".to_owned()),
            ],
            NumberToRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Meta('w'),
            ],
            CopyRectangleAsKill,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("o".to_owned()),
            ],
            OpenRectangle,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("r".to_owned()),
            ],
            CopyRectangleToRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("s".to_owned()),
            ],
            CopyToRegister,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("t".to_owned()),
            ],
            StringRectangle,
        ),
        KeyBinding::new(
            [
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("y".to_owned()),
            ],
            YankRectangle,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('x')],
            ExchangePointAndMark,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("k".to_owned())],
            KillBuffer,
        ),
        KeyBinding::new([KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')], SaveBuffer),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Text("o".to_owned())],
            OtherWindow,
        ),
        KeyBinding::new(
            [KeyEvent::Ctrl('x'), KeyEvent::Ctrl('c')],
            SaveBuffersKillTerminal,
        ),
    ]
}

pub fn help_keymap() -> KeyMap {
    use Command::*;

    KeyMap::named(
        KeyMapId::SpecialBuffer,
        "help-mode-map",
        [KeyBinding::new(
            [KeyEvent::Text("q".to_owned())],
            QuitHelpWindow,
        )],
    )
}

pub fn messages_keymap() -> KeyMap {
    use Command::*;

    KeyMap::named(
        KeyMapId::SpecialBuffer,
        "messages-mode-map",
        [KeyBinding::new(
            [KeyEvent::Text("q".to_owned())],
            QuitMessagesWindow,
        )],
    )
}

pub fn shell_output_keymap() -> KeyMap {
    use Command::*;

    KeyMap::named(
        KeyMapId::SpecialBuffer,
        "shell-output-mode-map",
        [KeyBinding::new(
            [KeyEvent::Text("q".to_owned())],
            QuitShellOutputWindow,
        )],
    )
}

pub fn buffer_list_keymap() -> KeyMap {
    use crate::input::SpecialKey;
    use Command::*;

    KeyMap::named(
        KeyMapId::SpecialBuffer,
        "buffer-list-mode-map",
        [
            KeyBinding::new([KeyEvent::Text("q".to_owned())], QuitBufferList),
            KeyBinding::new([KeyEvent::Special(SpecialKey::Enter)], BufferListSelect),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::{
        KeyBinding, KeyMap, KeyMapId, KeyMapStack, KeyResolution, KeyStackResolution,
        buffer_list_keymap, help_keymap, messages_keymap, shell_output_keymap,
    };
    use crate::command::{Command::*, CommandRegistry};
    use crate::input::{KeyEvent, SpecialKey};

    #[test]
    fn resolves_single_key_bindings() {
        let keymap = KeyMap::default();

        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('f')]),
            KeyResolution::Command(ForwardChar)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('f')]),
            KeyResolution::Command(ForwardWord)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('b')]),
            KeyResolution::Command(BackwardWord)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('d')]),
            KeyResolution::Command(KillWord)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('m')]),
            KeyResolution::Command(BackToIndentation)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::MetaSpecial(SpecialKey::Backspace)]),
            KeyResolution::Command(BackwardKillWord)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('<')]),
            KeyResolution::Command(BeginningOfBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('>')]),
            KeyResolution::Command(EndOfBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('s')]),
            KeyResolution::Command(IncrementalSearchForward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('v')]),
            KeyResolution::Command(ScrollPageForward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('v')]),
            KeyResolution::Command(ScrollPageBackward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Special(SpecialKey::PageDown)]),
            KeyResolution::Command(ScrollPageForward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Special(SpecialKey::PageUp)]),
            KeyResolution::Command(ScrollPageBackward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('l')]),
            KeyResolution::Command(Recenter)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('r')]),
            KeyResolution::Command(IncrementalSearchBackward)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('k')]),
            KeyResolution::Command(KillLine)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('j')]),
            KeyResolution::Command(NewlineAndIndent)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('o')]),
            KeyResolution::Command(OpenLine)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('^')]),
            KeyResolution::Command(JoinLine)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('q')]),
            KeyResolution::Command(QuotedInsert)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('w')]),
            KeyResolution::Command(KillRegion)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('y')]),
            KeyResolution::Command(Yank)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('_')]),
            KeyResolution::Command(Undo)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('@')]),
            KeyResolution::Command(SetMarkCommand)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('w')]),
            KeyResolution::Command(CopyRegionAsKill)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('%')]),
            KeyResolution::Command(QueryReplace)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('!')]),
            KeyResolution::Command(ShellCommand)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('|')]),
            KeyResolution::Command(ShellCommandOnRegion)
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
            keymap.resolve(&[KeyEvent::Ctrl('h'), KeyEvent::Text("e".to_owned())]),
            KeyResolution::Command(ViewEchoAreaMessages)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('h'), KeyEvent::Text("f".to_owned())]),
            KeyResolution::Command(DescribeFunction)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('h'), KeyEvent::Text("k".to_owned())]),
            KeyResolution::Command(DescribeKey)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Meta('g'), KeyEvent::Text("g".to_owned())]),
            KeyResolution::Command(GotoLine)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('s')]),
            KeyResolution::Command(SaveBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('f')]),
            KeyResolution::Command(FindFile)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('b')]),
            KeyResolution::Command(ListBuffers)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('r')]),
            KeyResolution::Command(FindFileReadOnly)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('q')]),
            KeyResolution::Command(ToggleReadOnly)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('w')]),
            KeyResolution::Command(WriteFile)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("(".to_owned())]),
            KeyResolution::Command(StartKeyboardMacro)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text(")".to_owned())]),
            KeyResolution::Command(EndKeyboardMacro)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("e".to_owned())]),
            KeyResolution::Command(CallLastKeyboardMacro)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("h".to_owned())]),
            KeyResolution::Command(MarkWholeBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text(" ".to_owned())]),
            KeyResolution::Command(RectangleMarkMode)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("r".to_owned())]),
            KeyResolution::Prefix
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text(" ".to_owned())
            ]),
            KeyResolution::Command(PointToRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("+".to_owned())
            ]),
            KeyResolution::Command(IncrementRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("N".to_owned())
            ]),
            KeyResolution::Command(RectangleNumberLines)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("c".to_owned())
            ]),
            KeyResolution::Command(ClearRectangle)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("d".to_owned())
            ]),
            KeyResolution::Command(DeleteRectangle)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("i".to_owned())
            ]),
            KeyResolution::Command(InsertRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("j".to_owned())
            ]),
            KeyResolution::Command(JumpToRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("k".to_owned())
            ]),
            KeyResolution::Command(KillRectangle)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("n".to_owned())
            ]),
            KeyResolution::Command(NumberToRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Meta('w')
            ]),
            KeyResolution::Command(CopyRectangleAsKill)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("o".to_owned())
            ]),
            KeyResolution::Command(OpenRectangle)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("r".to_owned())
            ]),
            KeyResolution::Command(CopyRectangleToRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("s".to_owned())
            ]),
            KeyResolution::Command(CopyToRegister)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("t".to_owned())
            ]),
            KeyResolution::Command(StringRectangle)
        );
        assert_eq!(
            keymap.resolve(&[
                KeyEvent::Ctrl('x'),
                KeyEvent::Text("r".to_owned()),
                KeyEvent::Text("y".to_owned())
            ]),
            KeyResolution::Command(YankRectangle)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Ctrl('x')]),
            KeyResolution::Command(ExchangePointAndMark)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("b".to_owned())]),
            KeyResolution::Command(SwitchToBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("i".to_owned())]),
            KeyResolution::Command(InsertFile)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("k".to_owned())]),
            KeyResolution::Command(KillBuffer)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("2".to_owned())]),
            KeyResolution::Command(SplitWindowBelow)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("3".to_owned())]),
            KeyResolution::Command(SplitWindowRight)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("0".to_owned())]),
            KeyResolution::Command(DeleteWindow)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("1".to_owned())]),
            KeyResolution::Command(DeleteOtherWindows)
        );
        assert_eq!(
            keymap.resolve(&[KeyEvent::Ctrl('x'), KeyEvent::Text("o".to_owned())]),
            KeyResolution::Command(OtherWindow)
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
    fn active_keymap_stack_resolves_global_bindings() {
        let keymap = KeyMap::default();
        let stack = KeyMapStack::global(&keymap);

        assert_eq!(stack.maps().len(), 1);
        assert_eq!(stack.maps()[0].id(), KeyMapId::Global);
        assert_eq!(stack.maps()[0].name(), "global-map");
        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('f')]),
            KeyStackResolution::Command {
                keymap: KeyMapId::Global,
                command: ForwardChar,
            }
        );
        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('x')]),
            KeyStackResolution::Prefix
        );
    }

    #[test]
    fn active_keymap_stack_respects_priority_order() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new([KeyEvent::Ctrl('f')], OtherWindow)],
        );
        let global = KeyMap::default();
        let stack = KeyMapStack::new([&special, &global]);

        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('f')]),
            KeyStackResolution::Command {
                keymap: KeyMapId::SpecialBuffer,
                command: OtherWindow,
            }
        );
        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('b')]),
            KeyStackResolution::Command {
                keymap: KeyMapId::Global,
                command: BackwardChar,
            }
        );
    }

    #[test]
    fn active_keymap_stack_prefix_shadows_lower_priority_command() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new(
                [KeyEvent::Ctrl('f'), KeyEvent::Text("q".to_owned())],
                OtherWindow,
            )],
        );
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [KeyBinding::new([KeyEvent::Ctrl('f')], ForwardChar)],
        );
        let stack = KeyMapStack::new([&special, &global]);

        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('f')]),
            KeyStackResolution::Prefix
        );
        assert_eq!(
            stack.resolve(&[KeyEvent::Ctrl('f'), KeyEvent::Text("q".to_owned())]),
            KeyStackResolution::Command {
                keymap: KeyMapId::SpecialBuffer,
                command: OtherWindow,
            }
        );
    }

    #[test]
    fn active_keymap_stack_reports_active_bindings_with_sources() {
        let special = KeyMap::named(
            KeyMapId::SpecialBuffer,
            "special-buffer-map",
            [KeyBinding::new(
                [KeyEvent::Text("q".to_owned())],
                OtherWindow,
            )],
        );
        let global = KeyMap::named(
            KeyMapId::Global,
            "global-map",
            [
                KeyBinding::new([KeyEvent::Text("q".to_owned())], ForwardChar),
                KeyBinding::new([KeyEvent::Ctrl('b')], BackwardChar),
            ],
        );
        let stack = KeyMapStack::new([&special, &global]);

        let other_window_bindings = stack.bindings_for_command(OtherWindow);
        assert_eq!(other_window_bindings.len(), 1);
        assert_eq!(other_window_bindings[0].keymap, KeyMapId::SpecialBuffer);
        assert_eq!(other_window_bindings[0].keymap_name, "special-buffer-map");
        assert_eq!(other_window_bindings[0].binding.command(), OtherWindow);

        assert!(stack.bindings_for_command(ForwardChar).is_empty());
        assert_eq!(stack.bindings_for_command(BackwardChar).len(), 1);
    }

    #[test]
    fn local_special_buffer_keymaps_resolve_local_commands() {
        assert_eq!(help_keymap().name(), "help-mode-map");
        assert_eq!(messages_keymap().name(), "messages-mode-map");
        assert_eq!(shell_output_keymap().name(), "shell-output-mode-map");
        assert_eq!(buffer_list_keymap().name(), "buffer-list-mode-map");

        assert_eq!(
            help_keymap().resolve(&[KeyEvent::Text("q".to_owned())]),
            KeyResolution::Command(QuitHelpWindow)
        );
        assert_eq!(
            messages_keymap().resolve(&[KeyEvent::Text("q".to_owned())]),
            KeyResolution::Command(QuitMessagesWindow)
        );
        assert_eq!(
            shell_output_keymap().resolve(&[KeyEvent::Text("q".to_owned())]),
            KeyResolution::Command(QuitShellOutputWindow)
        );
        assert_eq!(
            buffer_list_keymap().resolve(&[KeyEvent::Text("q".to_owned())]),
            KeyResolution::Command(QuitBufferList)
        );
        assert_eq!(
            buffer_list_keymap().resolve(&[KeyEvent::Special(SpecialKey::Enter)]),
            KeyResolution::Command(BufferListSelect)
        );
    }

    #[test]
    fn lists_bindings_starting_with_prefix() {
        let keymap = KeyMap::default();
        let bindings = keymap.bindings_starting_with(&[KeyEvent::Meta('g')]);

        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].command(), GotoLine);
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
                .map(|binding| binding.command()),
            Some(FindFile)
        );
        assert_eq!(keymap.bindings_for_command(FindFile).len(), 1);
        assert!(!keymap.bindings_for_command(SaveBuffer).is_empty());
    }

    #[test]
    fn default_bindings_target_registered_commands() {
        let keymap = KeyMap::default();
        let commands = CommandRegistry::default();

        for binding in &keymap.bindings {
            assert!(
                commands.get_by_id(binding.command()).is_some(),
                "{:?} should target a registered command",
                binding.command()
            );
        }
    }

    #[test]
    fn local_special_bindings_target_registered_commands() {
        let commands = CommandRegistry::default();
        let keymaps = [
            help_keymap(),
            messages_keymap(),
            shell_output_keymap(),
            buffer_list_keymap(),
        ];

        for keymap in keymaps {
            for binding in keymap.bindings_starting_with(&[]) {
                assert!(
                    commands.get_by_id(binding.command()).is_some(),
                    "{:?} in {} should target a registered command",
                    binding.command(),
                    keymap.name()
                );
            }
        }
    }
}
