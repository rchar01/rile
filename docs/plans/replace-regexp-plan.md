<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Implement Replace Regexp

## Goal

Add `replace-regexp` as a non-interactive regexp replacement command. The first
implementation should reuse Rile's existing line-local regexp subset and keep
replacement text literal, matching the current `query-replace-regexp` behavior.

## Scope

- Add `M-x replace-regexp` command registration and editor dispatch.
- Prompt for a regexp and replacement string, then replace all matches from point
  to the end of the buffer.
- Reuse existing regexp compilation, matching, replacement, undo, and prompt
  history infrastructure where practical.
- Add unit and PTY coverage for successful replacement, errors, history, point
  scope, and undo.
- Update user-visible documentation, release notes, and source history.

## Non-Goals

- No default key binding in this pass.
- No Emacs replacement escapes such as `\&`, `\1`, `\2`, `\,(...)`, or case
  conversion escapes.
- No regexp capture groups, alternation, counted repetition, backreferences, or
  word-boundary syntax.
- No multiline regexp matching or replacements across line breaks.
- No region-limited or prefix-argument variants.

## Current Context

- `query-replace-regexp` is implemented with `C-M-%` and `M-x`.
- `src/search_pattern.rs` provides `SearchPattern::compile`, line-local matching,
  and `SearchPattern::can_match_empty`.
- `src/editor/search.rs::find_match` finds the next match in buffer order.
- `src/editor.rs` already has `ReplacementTemplate::Literal`,
  `replace_text_range`, query-replace prompt handling, and undo recording paths.
- `src/editor/prompt_history.rs` stores prompt history per `PromptKind`.
- Current docs state that regexp query replace uses literal replacement text and
  rejects regexps that can match empty text.

## Assumptions

- Literal replacement is the correct first step because it matches
  `query-replace-regexp` and avoids premature capture-engine work.
- `replace-regexp` should operate from point to end of buffer and not wrap.
- Replacement should stop after each inserted replacement's end position to avoid
  matching inside freshly inserted text.
- Empty-match-capable regexps should be rejected before replacement, using the
  same error as `query-replace-regexp`.

## Open Questions

- [x] Should this pass include replacement escapes or captures? Decision: no.
  Keep replacement text literal and defer replacement expansion to a separate
  regexp-engine enhancement.

## Tasks

- [ ] Add `Command::ReplaceRegexp` in `src/command.rs`, including category,
  description, registry entry, command-name test coverage, and an editor handler.
- [ ] Add `PromptKind::ReplaceRegexpSearch` and
  `PromptKind::ReplaceRegexpReplacement` in `src/minibuffer.rs` and label them as
  `Replace regexp: ` and `Replace regexp <regexp> with: ` in `src/editor.rs`.
- [ ] Add a small pending replacement state in `src/editor.rs` to carry the
  accepted regexp query and compiled `SearchPattern` between the search and
  replacement prompts.
- [ ] Start `replace-regexp` by checking editability, clearing conflicting search
  or query-replace state, deactivating the region, and opening the regexp prompt.
- [ ] Validate the regexp prompt in `src/editor.rs`: reject empty input, invalid
  regexps, and regexps that can match empty text before recording history or
  starting the replacement prompt.
- [ ] Implement replacement submission by recording accepted replacement history,
  iterating forward from point with `find_match`, replacing each match with a
  literal `ReplacementTemplate`, recording undo for every replacement, and
  reporting either `Replaced N occurrences` or `No matches for <regexp>`.
- [ ] Clear pending replace-regexp state on prompt cancel, buffer switches, and
  other relevant operation resets without disturbing unrelated state.
- [ ] Add replace-regexp prompt kinds to prompt history support while keeping
  them separate from query-replace-regexp histories.
- [ ] Add unit tests in `src/editor.rs` for successful regexp replacement,
  point-to-end scope, no-match behavior, invalid/zero-width rejection, history
  timing, and undo restoration.
- [ ] Add PTY tests in `tests/pty_search.rs` for `M-x replace-regexp` prompt flow,
  visible replacement results, and search/replacement history recall.
- [ ] Update `README.md`, `NEWS`, `ChangeLog`, `docs/development.md`, and
  `docs/emacs-function-reference.md` for the new command and its literal
  replacement limitation.

## Validation

- [ ] Run `./scripts/in-container cargo test --locked --lib replace_regexp`.
- [ ] Run `./scripts/in-container cargo test --locked --test pty_search replace_regexp`.
- [ ] Run `make verify`.
- [ ] Run `git diff --check`.
- [ ] Run patch verification for the final diff.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-12 | Plan created with literal replacement as the recommended first subset. | User requested a written implementation plan; current `query-replace-regexp` code and docs inspected. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-12 | Keep replacement text literal in this pass. | The current regexp engine has no capture API, and this keeps behavior consistent with `query-replace-regexp`. |
| 2026-07-12 | Do not add a default key binding for `replace-regexp`. | The requested command is useful through `M-x`; avoiding a new binding keeps the initial feature minimal. |
