// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::search_pattern::PatternKind;

#[derive(Debug, Clone, Default)]
pub(super) struct SearchHistoryStore {
    literal: SearchHistory,
    regexp: SearchHistory,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SearchHistory {
    entries: Vec<String>,
    position: Option<usize>,
    draft: String,
}

impl SearchHistoryStore {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn record(&mut self, kind: PatternKind, input: &str) {
        if input.trim().is_empty() {
            self.reset(kind);
            return;
        }

        let history = self.history_mut(kind);
        if history.entries.last().is_none_or(|entry| entry != input) {
            history.entries.push(input.to_owned());
        }
        history.position = None;
        history.draft.clear();
    }

    pub(super) fn recall(
        &mut self,
        kind: PatternKind,
        current: &str,
        direction: isize,
    ) -> Option<String> {
        let history = self.history_mut(kind);
        if history.entries.is_empty() {
            return None;
        }

        let next_position = match (history.position, direction.signum()) {
            (None, -1) => {
                history.draft = current.to_owned();
                Some(history.entries.len() - 1)
            }
            (Some(position), -1) => Some(position.saturating_sub(1)),
            (Some(position), 1) if position + 1 < history.entries.len() => Some(position + 1),
            (Some(_), 1) => None,
            _ => return None,
        };

        history.position = next_position;
        Some(
            next_position
                .map(|position| history.entries[position].clone())
                .unwrap_or_else(|| history.draft.clone()),
        )
    }

    pub(super) fn reset(&mut self, kind: PatternKind) {
        let history = self.history_mut(kind);
        history.position = None;
        history.draft.clear();
    }

    fn history_mut(&mut self, kind: PatternKind) -> &mut SearchHistory {
        match kind {
            PatternKind::Literal => &mut self.literal,
            PatternKind::Regexp => &mut self.regexp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_preserves_draft() {
        let mut history = SearchHistoryStore::new();
        history.record(PatternKind::Literal, "alpha");
        history.record(PatternKind::Literal, "beta");

        assert_eq!(
            history.recall(PatternKind::Literal, "draft", -1),
            Some("beta".to_owned())
        );
        assert_eq!(
            history.recall(PatternKind::Literal, "ignored", -1),
            Some("alpha".to_owned())
        );
        assert_eq!(
            history.recall(PatternKind::Literal, "ignored", 1),
            Some("beta".to_owned())
        );
        assert_eq!(
            history.recall(PatternKind::Literal, "ignored", 1),
            Some("draft".to_owned())
        );
    }

    #[test]
    fn literal_and_regexp_histories_are_separate() {
        let mut history = SearchHistoryStore::new();
        history.record(PatternKind::Literal, "foo");
        history.record(PatternKind::Regexp, "f.o");

        assert_eq!(
            history.recall(PatternKind::Literal, "", -1),
            Some("foo".to_owned())
        );
        assert_eq!(
            history.recall(PatternKind::Regexp, "", -1),
            Some("f.o".to_owned())
        );
    }

    #[test]
    fn record_skips_empty_input_and_adjacent_duplicates() {
        let mut history = SearchHistoryStore::new();
        history.record(PatternKind::Literal, "alpha");
        history.record(PatternKind::Literal, "alpha");
        history.record(PatternKind::Literal, "   ");

        assert_eq!(
            history.recall(PatternKind::Literal, "", -1),
            Some("alpha".to_owned())
        );
        assert_eq!(
            history.recall(PatternKind::Literal, "ignored", -1),
            Some("alpha".to_owned())
        );
    }
}
