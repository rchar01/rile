// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::command::CommandRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStyle {
    Vertical,
    CompletionsBuffer,
    Ido,
}

impl CompletionStyle {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::CompletionsBuffer => "completions-buffer",
            Self::Ido => "ido",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionMatching {
    Prefix,
    Substring,
}

impl CompletionMatching {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prefix => "prefix",
            Self::Substring => "substring",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionConfig {
    pub style: CompletionStyle,
    pub max_candidates: usize,
    pub show_annotations: bool,
    pub matching: CompletionMatching,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            style: CompletionStyle::Vertical,
            max_candidates: 8,
            show_annotations: true,
            matching: CompletionMatching::Prefix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub value: String,
    pub annotation: String,
}

impl CompletionCandidate {
    pub fn new(value: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: annotation.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSource {
    Commands,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSession {
    source: CompletionSource,
    config: CompletionConfig,
    candidates: Vec<CompletionCandidate>,
    matches: Vec<usize>,
    selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionViewItem<'a> {
    pub candidate: &'a CompletionCandidate,
    pub selected: bool,
}

impl CompletionSession {
    pub fn commands(registry: &CommandRegistry, config: CompletionConfig) -> Self {
        let candidates = registry
            .commands()
            .iter()
            .filter(|command| command.interactive)
            .map(|command| CompletionCandidate::new(command.name, command.description))
            .collect::<Vec<_>>();
        let mut session = Self {
            source: CompletionSource::Commands,
            config,
            candidates,
            matches: Vec::new(),
            selected: 0,
        };
        session.update("");
        session
    }

    pub fn source(&self) -> CompletionSource {
        self.source
    }

    pub fn style(&self) -> CompletionStyle {
        self.config.style
    }

    pub fn max_candidates(&self) -> usize {
        self.config.max_candidates.max(1)
    }

    pub fn show_annotations(&self) -> bool {
        self.config.show_annotations
    }

    pub fn update(&mut self, input: &str) {
        self.matches = self
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| self.matches_input(candidate, input).then_some(index))
            .collect();
        if self.matches.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.matches.len() - 1);
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.matches.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.matches.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
    }

    pub fn selected(&self) -> Option<&CompletionCandidate> {
        self.matches
            .get(self.selected)
            .and_then(|index| self.candidates.get(*index))
    }

    pub fn common_prefix(&self, input: &str) -> Option<String> {
        let mut values = self.matches.iter().filter_map(|index| {
            self.candidates
                .get(*index)
                .map(|candidate| candidate.value.as_str())
        });
        let first = values.next()?;
        let mut prefix = first.to_owned();
        for value in values {
            prefix = common_prefix(&prefix, value);
            if prefix.is_empty() {
                break;
            }
        }
        (prefix.len() > input.len()).then_some(prefix)
    }

    pub fn view_items(&self) -> Vec<CompletionViewItem<'_>> {
        let max = self.max_candidates();
        let start = if self.selected >= max {
            self.selected + 1 - max
        } else {
            0
        };
        self.matches
            .iter()
            .enumerate()
            .skip(start)
            .take(max)
            .filter_map(|(match_index, candidate_index)| {
                self.candidates
                    .get(*candidate_index)
                    .map(|candidate| CompletionViewItem {
                        candidate,
                        selected: match_index == self.selected,
                    })
            })
            .collect()
    }

    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    fn matches_input(&self, candidate: &CompletionCandidate, input: &str) -> bool {
        match self.config.matching {
            CompletionMatching::Prefix => candidate.value.starts_with(input),
            CompletionMatching::Substring => candidate.value.contains(input),
        }
    }
}

fn common_prefix(left: &str, right: &str) -> String {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(character, _)| character)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CompletionConfig, CompletionSession};
    use crate::command::CommandRegistry;

    #[test]
    fn command_completion_filters_and_selects_candidates() {
        let registry = CommandRegistry::default();
        let mut session = CompletionSession::commands(&registry, CompletionConfig::default());

        session.update("toggle-s");

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("toggle-search-highlighting")
        );
        session.move_selection(1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("toggle-syntax-highlighting")
        );
    }

    #[test]
    fn command_completion_extends_common_prefix() {
        let registry = CommandRegistry::default();
        let mut session = CompletionSession::commands(&registry, CompletionConfig::default());

        session.update("toggle");

        assert_eq!(session.common_prefix("toggle").as_deref(), Some("toggle-"));
    }
}
