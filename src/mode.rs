// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::file::DocumentKind;
use crate::syntax::{MajorMode, SyntaxMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModeId {
    Fundamental,
    Text,
    Rust,
    C,
    Shell,
    Markdown,
    Toml,
    PlainTextSyntax,
    RustSyntax,
    CSyntax,
    ShellSyntax,
    MarkdownSyntax,
    TomlSyntax,
    Welcome,
    Help,
    Messages,
    Completions,
    BufferList,
    ShellOutput,
    LineNumbers,
    SyntaxHighlighting,
    SearchHighlighting,
}

impl ModeId {
    pub const fn for_major_mode(mode: MajorMode) -> Self {
        match mode {
            MajorMode::Fundamental => Self::Fundamental,
            MajorMode::Text => Self::Text,
            MajorMode::Rust => Self::Rust,
            MajorMode::C => Self::C,
            MajorMode::Shell => Self::Shell,
            MajorMode::Markdown => Self::Markdown,
            MajorMode::Toml => Self::Toml,
        }
    }

    pub const fn for_syntax_mode(mode: SyntaxMode) -> Self {
        match mode {
            SyntaxMode::PlainText => Self::PlainTextSyntax,
            SyntaxMode::Rust => Self::RustSyntax,
            SyntaxMode::C => Self::CSyntax,
            SyntaxMode::Shell => Self::ShellSyntax,
            SyntaxMode::Markdown => Self::MarkdownSyntax,
            SyntaxMode::Toml => Self::TomlSyntax,
        }
    }

    pub const fn for_document_kind(kind: DocumentKind) -> Option<Self> {
        match kind {
            DocumentKind::Normal => None,
            DocumentKind::Welcome => Some(Self::Welcome),
            DocumentKind::Help => Some(Self::Help),
            DocumentKind::Messages => Some(Self::Messages),
            DocumentKind::Completions => Some(Self::Completions),
            DocumentKind::BufferList => Some(Self::BufferList),
            DocumentKind::ShellOutput => Some(Self::ShellOutput),
        }
    }
}

const ALL_MODE_IDS: &[ModeId] = &[
    ModeId::Fundamental,
    ModeId::Text,
    ModeId::Rust,
    ModeId::C,
    ModeId::Shell,
    ModeId::Markdown,
    ModeId::Toml,
    ModeId::PlainTextSyntax,
    ModeId::RustSyntax,
    ModeId::CSyntax,
    ModeId::ShellSyntax,
    ModeId::MarkdownSyntax,
    ModeId::TomlSyntax,
    ModeId::Welcome,
    ModeId::Help,
    ModeId::Messages,
    ModeId::Completions,
    ModeId::BufferList,
    ModeId::ShellOutput,
    ModeId::LineNumbers,
    ModeId::SyntaxHighlighting,
    ModeId::SearchHighlighting,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeKind {
    Major,
    Syntax,
    Special,
    Minor,
}

impl ModeKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Major => "major mode",
            Self::Syntax => "syntax mode",
            Self::Special => "special buffer mode",
            Self::Minor => "minor mode",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModeSpec {
    pub id: ModeId,
    pub name: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub kind: ModeKind,
    pub keymap: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct ModeRegistry {
    specs: Vec<ModeSpec>,
}

impl ModeRegistry {
    pub fn new(specs: Vec<ModeSpec>) -> std::result::Result<Self, String> {
        let registry = Self { specs };
        registry.validate()?;
        Ok(registry)
    }

    pub fn modes(&self) -> impl Iterator<Item = &ModeSpec> {
        self.specs.iter()
    }

    pub fn get(&self, id: ModeId) -> Option<&ModeSpec> {
        self.specs.iter().find(|spec| spec.id == id)
    }

    pub fn validate(&self) -> std::result::Result<(), String> {
        for (index, spec) in self.specs.iter().enumerate() {
            if spec.name.is_empty() || spec.summary.is_empty() || spec.doc.is_empty() {
                return Err(format!("mode `{:?}` is missing required metadata", spec.id));
            }
            for other in self.specs.iter().skip(index + 1) {
                if spec.id == other.id {
                    return Err(format!("duplicate mode id `{}`", spec.name));
                }
                if spec.name == other.name {
                    return Err(format!("duplicate mode name `{}`", spec.name));
                }
            }
        }
        for id in ALL_MODE_IDS {
            if self.get(*id).is_none() {
                return Err(format!("missing mode spec for `{id:?}`"));
            }
        }
        Ok(())
    }
}

impl Default for ModeRegistry {
    fn default() -> Self {
        Self::new(default_modes()).expect("default mode registry should be valid")
    }
}

fn default_modes() -> Vec<ModeSpec> {
    vec![
        mode(ModeSpec {
            id: ModeId::Fundamental,
            name: "fundamental-mode",
            summary: "Fundamental mode",
            doc: "Default editing mode used when no file type-specific mode is known.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Text,
            name: "text-mode",
            summary: "Text mode",
            doc: "Editing mode for plain text files.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Rust,
            name: "rust-mode",
            summary: "Rust mode",
            doc: "Editing mode selected for Rust source files.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::C,
            name: "c-mode",
            summary: "C mode",
            doc: "Editing mode selected for C source and header files.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Shell,
            name: "shell-script-mode",
            summary: "Shell script mode",
            doc: "Editing mode selected for shell script files.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Markdown,
            name: "markdown-mode",
            summary: "Markdown mode",
            doc: "Editing mode selected for Markdown documents.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Toml,
            name: "toml-mode",
            summary: "TOML mode",
            doc: "Editing mode selected for TOML configuration files.",
            kind: ModeKind::Major,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::PlainTextSyntax,
            name: "plain-text-syntax-mode",
            summary: "Plain text syntax",
            doc: "Syntax mode that leaves buffer text undecorated.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::RustSyntax,
            name: "rust-syntax-mode",
            summary: "Rust syntax",
            doc: "Syntax mode that highlights Rust keywords, strings, and comments.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::CSyntax,
            name: "c-syntax-mode",
            summary: "C syntax",
            doc: "Syntax mode that highlights C keywords, strings, comments, and preprocessor lines.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::ShellSyntax,
            name: "shell-syntax-mode",
            summary: "Shell syntax",
            doc: "Syntax mode that highlights shell keywords, strings, and comments.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::MarkdownSyntax,
            name: "markdown-syntax-mode",
            summary: "Markdown syntax",
            doc: "Syntax mode that highlights Markdown headings and inline code spans.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::TomlSyntax,
            name: "toml-syntax-mode",
            summary: "TOML syntax",
            doc: "Syntax mode that highlights TOML sections, keys, strings, and comments.",
            kind: ModeKind::Syntax,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Welcome,
            name: "welcome-mode",
            summary: "Welcome buffer mode",
            doc: "Special buffer mode used by the startup welcome buffer.",
            kind: ModeKind::Special,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::Help,
            name: "help-mode",
            summary: "Help buffer mode",
            doc: "Special buffer mode for read-only help text. Press q to restore the previous buffer.",
            kind: ModeKind::Special,
            keymap: Some("help-mode-map"),
        }),
        mode(ModeSpec {
            id: ModeId::Messages,
            name: "messages-mode",
            summary: "Messages buffer mode",
            doc: "Special buffer mode for reviewing recent echo-area status and error messages.",
            kind: ModeKind::Special,
            keymap: Some("messages-mode-map"),
        }),
        mode(ModeSpec {
            id: ModeId::Completions,
            name: "completions-mode",
            summary: "Completions buffer mode",
            doc: "Special buffer mode used to display minibuffer completion candidates.",
            kind: ModeKind::Special,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::BufferList,
            name: "buffer-list-mode",
            summary: "Buffer list mode",
            doc: "Special buffer mode for selecting and inspecting open editor buffers.",
            kind: ModeKind::Special,
            keymap: Some("buffer-list-mode-map"),
        }),
        mode(ModeSpec {
            id: ModeId::ShellOutput,
            name: "shell-output-mode",
            summary: "Shell output mode",
            doc: "Special buffer mode for command output produced by shell-command operations.",
            kind: ModeKind::Special,
            keymap: Some("shell-output-mode-map"),
        }),
        mode(ModeSpec {
            id: ModeId::LineNumbers,
            name: "line-number-mode",
            summary: "Line number mode",
            doc: "Minor mode that shows a left line-number gutter in normal editing buffers.",
            kind: ModeKind::Minor,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::SyntaxHighlighting,
            name: "syntax-highlight-mode",
            summary: "Syntax highlight mode",
            doc: "Minor mode that enables syntax decorations for supported major modes.",
            kind: ModeKind::Minor,
            keymap: None,
        }),
        mode(ModeSpec {
            id: ModeId::SearchHighlighting,
            name: "search-highlight-mode",
            summary: "Search highlight mode",
            doc: "Minor mode that highlights active search and query-replace matches.",
            kind: ModeKind::Minor,
            keymap: None,
        }),
    ]
}

fn mode(spec: ModeSpec) -> ModeSpec {
    spec
}

#[cfg(test)]
mod tests {
    use super::{ModeId, ModeRegistry};
    use crate::file::DocumentKind;
    use crate::syntax::{MajorMode, SyntaxMode};

    #[test]
    fn default_mode_registry_is_valid() {
        let registry = ModeRegistry::default();

        assert_eq!(registry.validate(), Ok(()));
        assert_eq!(registry.modes().count(), 22);
    }

    #[test]
    fn every_mode_has_required_metadata() {
        for mode in ModeRegistry::default().modes() {
            assert!(!mode.name.is_empty());
            assert!(!mode.summary.is_empty());
            assert!(!mode.doc.is_empty());
            assert!(!mode.kind.label().is_empty());
        }
    }

    #[test]
    fn default_registry_covers_every_mode_id() {
        let registry = ModeRegistry::default();

        for id in super::ALL_MODE_IDS {
            assert!(registry.get(*id).is_some(), "missing mode spec for {id:?}");
        }
    }

    #[test]
    fn maps_existing_mode_state_to_mode_ids() {
        assert_eq!(ModeId::for_major_mode(MajorMode::Rust), ModeId::Rust);
        assert_eq!(
            ModeId::for_syntax_mode(SyntaxMode::Toml),
            ModeId::TomlSyntax
        );
        assert_eq!(
            ModeId::for_document_kind(DocumentKind::Help),
            Some(ModeId::Help)
        );
        assert_eq!(ModeId::for_document_kind(DocumentKind::Normal), None);
    }
}
