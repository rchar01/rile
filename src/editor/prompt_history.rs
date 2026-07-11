// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::minibuffer::PromptKind;

#[derive(Debug, Clone, Default)]
pub(super) struct PromptHistoryStore {
    histories: Vec<PromptHistory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptHistory {
    kind: PromptKind,
    entries: Vec<String>,
    position: Option<usize>,
    draft: String,
}

impl PromptHistoryStore {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn record(&mut self, kind: PromptKind, input: &str) {
        if !prompt_kind_uses_history(kind) || input.trim().is_empty() {
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
        kind: PromptKind,
        current: &str,
        direction: isize,
    ) -> Option<String> {
        if !prompt_kind_uses_history(kind) {
            return None;
        }

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

    pub(super) fn reset(&mut self, kind: PromptKind) {
        if let Some(history) = self
            .histories
            .iter_mut()
            .find(|history| history.kind == kind)
        {
            history.position = None;
            history.draft.clear();
        }
    }

    fn history_mut(&mut self, kind: PromptKind) -> &mut PromptHistory {
        if let Some(index) = self
            .histories
            .iter()
            .position(|history| history.kind == kind)
        {
            return &mut self.histories[index];
        }

        self.histories.push(PromptHistory {
            kind,
            entries: Vec::new(),
            position: None,
            draft: String::new(),
        });
        self.histories
            .last_mut()
            .expect("history was just inserted")
    }
}

pub(super) fn prompt_kind_uses_history(kind: PromptKind) -> bool {
    matches!(
        kind,
        PromptKind::ExtendedCommand
            | PromptKind::DescribeFunction
            | PromptKind::DescribeVariable
            | PromptKind::FindFile
            | PromptKind::FindFileReadOnly
            | PromptKind::GotoLine
            | PromptKind::InsertFile
            | PromptKind::KillBuffer
            | PromptKind::QueryReplaceReplacement
            | PromptKind::QueryReplaceSearch
            | PromptKind::RectangleNumberFormat
            | PromptKind::RectangleNumberStart
            | PromptKind::ShellCommand
            | PromptKind::StringRectangle
            | PromptKind::SwitchToBuffer
            | PromptKind::WriteFile
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_restores_draft_after_history_navigation() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::ExtendedCommand, "first-command");
        history.record(PromptKind::ExtendedCommand, "second-command");

        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "draft", -1),
            Some("second-command".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", -1),
            Some("first-command".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", 1),
            Some("second-command".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", 1),
            Some("draft".to_owned())
        );
    }

    #[test]
    fn recall_history_is_per_prompt_kind() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::ExtendedCommand, "command-input");
        history.record(PromptKind::FindFile, "file-input");

        assert_eq!(
            history.recall(PromptKind::FindFile, "", -1),
            Some("file-input".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "", -1),
            Some("command-input".to_owned())
        );
    }

    #[test]
    fn record_skips_empty_input_and_adjacent_duplicates() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::ExtendedCommand, "repeat-command");
        history.record(PromptKind::ExtendedCommand, "repeat-command");
        history.record(PromptKind::ExtendedCommand, "   ");

        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "", -1),
            Some("repeat-command".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", -1),
            Some("repeat-command".to_owned())
        );
    }

    #[test]
    fn reset_clears_navigation_without_losing_entries() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::ExtendedCommand, "previous-command");

        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "stale draft", -1),
            Some("previous-command".to_owned())
        );
        history.reset(PromptKind::ExtendedCommand);
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", 1),
            None
        );

        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "fresh draft", -1),
            Some("previous-command".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::ExtendedCommand, "ignored", 1),
            Some("fresh draft".to_owned())
        );
    }

    #[test]
    fn query_replace_search_and_replacement_use_separate_histories() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::QueryReplaceSearch, "needle");
        history.record(PromptKind::QueryReplaceReplacement, "replacement");

        assert_eq!(
            history.recall(PromptKind::QueryReplaceSearch, "draft", -1),
            Some("needle".to_owned())
        );
        assert_eq!(
            history.recall(PromptKind::QueryReplaceReplacement, "draft", -1),
            Some("replacement".to_owned())
        );
    }

    #[test]
    fn unsupported_prompt_kind_has_no_history() {
        let mut history = PromptHistoryStore::new();
        history.record(PromptKind::IncrementalSearch, "query");

        assert_eq!(history.recall(PromptKind::IncrementalSearch, "", -1), None);
    }
}
