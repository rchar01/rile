// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::completion::{CompletionSession, CompletionSource};
use crate::minibuffer::PromptKind;

pub(super) struct CompletionAcceptContext<'a> {
    pub(super) kind: PromptKind,
    pub(super) input: &'a str,
    pub(super) completion: Option<&'a CompletionSession>,
    pub(super) command_exists: bool,
    pub(super) option_exists: bool,
    pub(super) exact_file_exists: bool,
    pub(super) buffer_exists: bool,
    pub(super) switch_buffer_default: Option<&'a str>,
}

pub(super) fn accepted_completion_input(context: CompletionAcceptContext<'_>) -> String {
    let input = context.input;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        if context.kind == PromptKind::SwitchToBuffer {
            return context
                .switch_buffer_default
                .map(str::to_owned)
                .unwrap_or_else(|| input.to_owned());
        }
        return input.to_owned();
    }

    match context.completion.map(CompletionSession::source) {
        Some(CompletionSource::Commands)
            if !selection_explicit(context.completion) && context.command_exists =>
        {
            return trimmed.to_owned();
        }
        Some(CompletionSource::Options)
            if !selection_explicit(context.completion) && context.option_exists =>
        {
            return trimmed.to_owned();
        }
        Some(CompletionSource::Files) => return accepted_file_completion_input(context),
        Some(CompletionSource::Buffers)
            if matches!(
                context.kind,
                PromptKind::KillBuffer | PromptKind::SwitchToBuffer
            ) =>
        {
            return accepted_buffer_completion_input(context);
        }
        _ => {}
    }

    selected_value(context.completion).unwrap_or_else(|| input.to_owned())
}

pub(super) fn directory_completion_to_enter(
    completion: Option<&CompletionSession>,
    input: &str,
) -> Option<String> {
    if input.trim().is_empty() {
        return None;
    }
    let completion =
        completion.filter(|completion| completion.source() == CompletionSource::Files)?;
    let candidate = completion
        .selected()
        .filter(|candidate| candidate.is_directory())?;
    if completion.has_matches()
        && (completion.selection_explicit()
            || candidate.value.trim_end_matches('/') == input.trim().trim_end_matches('/'))
    {
        return Some(candidate.value.clone());
    }
    None
}

pub(super) fn raw_completion_input(input: &str) -> String {
    input.to_owned()
}

pub(super) fn tab_completion_input(
    completion: &CompletionSession,
    current_input: &str,
) -> Option<String> {
    completion
        .selected()
        .map(|candidate| candidate.value.clone())
        .filter(|next_input| next_input != current_input)
}

fn accepted_file_completion_input(context: CompletionAcceptContext<'_>) -> String {
    let input = context.input;
    let trimmed = input.trim();
    if !selection_explicit(context.completion) && context.exact_file_exists {
        return trimmed.to_owned();
    }
    let Some(completion) = context.completion else {
        return input.to_owned();
    };
    if !completion.has_matches() {
        return input.to_owned();
    }
    if let Some(candidate) = completion.selected()
        && candidate.is_directory()
        && !completion.selection_explicit()
        && candidate.value.trim_end_matches('/') != trimmed.trim_end_matches('/')
    {
        return input.to_owned();
    }
    selected_value(context.completion).unwrap_or_else(|| input.to_owned())
}

fn accepted_buffer_completion_input(context: CompletionAcceptContext<'_>) -> String {
    if !selection_explicit(context.completion) && context.buffer_exists {
        return context.input.to_owned();
    }
    selected_value(context.completion).unwrap_or_else(|| context.input.to_owned())
}

fn selection_explicit(completion: Option<&CompletionSession>) -> bool {
    completion.is_some_and(CompletionSession::selection_explicit)
}

fn selected_value(completion: Option<&CompletionSession>) -> Option<String> {
    completion
        .and_then(CompletionSession::selected)
        .map(|candidate| candidate.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRegistry;
    use crate::completion::{CompletionConfig, CompletionSession};
    use crate::keymap::KeyMap;
    use crate::option::OptionRegistry;

    #[test]
    fn empty_switch_buffer_accepts_default_name() {
        let accepted = accepted_completion_input(CompletionAcceptContext {
            kind: PromptKind::SwitchToBuffer,
            input: "",
            completion: None,
            command_exists: false,
            option_exists: false,
            exact_file_exists: false,
            buffer_exists: false,
            switch_buffer_default: Some("previous.txt"),
        });

        assert_eq!(accepted, "previous.txt");
    }

    #[test]
    fn exact_command_accepts_raw_input_without_explicit_selection() {
        let completion = CompletionSession::commands(
            &CommandRegistry::default(),
            &KeyMap::default(),
            CompletionConfig::default(),
        );
        let accepted = accepted_completion_input(CompletionAcceptContext {
            kind: PromptKind::ExtendedCommand,
            input: "toggle-line-numbers",
            completion: Some(&completion),
            command_exists: true,
            option_exists: false,
            exact_file_exists: false,
            buffer_exists: false,
            switch_buffer_default: None,
        });

        assert_eq!(accepted, "toggle-line-numbers");
    }

    #[test]
    fn exact_option_accepts_raw_input_without_explicit_selection() {
        let completion =
            CompletionSession::options(&OptionRegistry::default(), CompletionConfig::default());
        let accepted = accepted_completion_input(CompletionAcceptContext {
            kind: PromptKind::DescribeVariable,
            input: "completion_style",
            completion: Some(&completion),
            command_exists: false,
            option_exists: true,
            exact_file_exists: false,
            buffer_exists: false,
            switch_buffer_default: None,
        });

        assert_eq!(accepted, "completion_style");
    }

    #[test]
    fn exact_buffer_accepts_raw_input_without_explicit_selection() {
        let completion =
            CompletionSession::buffers(["other-buffer".to_owned()], CompletionConfig::default());
        let accepted = accepted_completion_input(CompletionAcceptContext {
            kind: PromptKind::SwitchToBuffer,
            input: "exact-buffer",
            completion: Some(&completion),
            command_exists: false,
            option_exists: false,
            exact_file_exists: false,
            buffer_exists: true,
            switch_buffer_default: None,
        });

        assert_eq!(accepted, "exact-buffer");
    }

    #[test]
    fn file_completion_keeps_raw_mismatched_directory_prefix() {
        let temp = tempfile::tempdir().expect("tempdir should create");
        std::fs::create_dir(temp.path().join("alpha-dir")).expect("directory should create");
        let mut completion = CompletionSession::files(temp.path(), CompletionConfig::default());
        completion.update("alpha");

        let accepted = accepted_completion_input(CompletionAcceptContext {
            kind: PromptKind::FindFile,
            input: "alpha",
            completion: Some(&completion),
            command_exists: false,
            option_exists: false,
            exact_file_exists: false,
            buffer_exists: false,
            switch_buffer_default: None,
        });

        assert_eq!(accepted, "alpha");
    }

    #[test]
    fn directory_completion_enters_matching_selected_directory() {
        let temp = tempfile::tempdir().expect("tempdir should create");
        std::fs::create_dir(temp.path().join("alpha-dir")).expect("directory should create");
        let mut completion = CompletionSession::files(temp.path(), CompletionConfig::default());
        completion.update("alpha");

        assert_eq!(
            directory_completion_to_enter(Some(&completion), "alpha-dir/"),
            Some("alpha-dir/".to_owned())
        );
    }

    #[test]
    fn tab_completion_uses_selected_candidate_when_it_changes_input() {
        let mut completion =
            CompletionSession::buffers(["alpha-buffer".to_owned()], CompletionConfig::default());
        completion.update("alpha");

        assert_eq!(
            tab_completion_input(&completion, "alpha"),
            Some("alpha-buffer".to_owned())
        );
        assert_eq!(tab_completion_input(&completion, "alpha-buffer"), None);
    }

    #[test]
    fn raw_completion_input_preserves_minibuffer_text() {
        assert_eq!(raw_completion_input("readme.md"), "readme.md");
    }
}
