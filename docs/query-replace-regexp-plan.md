<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Add Query Replace Regexp

## Goal

Add a basic Emacs-style `query-replace-regexp` command that reuses Rile's
existing regexp matcher, interactive query-replace workflow, highlighting, undo,
and prompt history. Keep the regexp and replacement boundaries modular so later
work can add captures, replacement templates, and non-query `replace-regexp`
without rewriting the first implementation.

## Scope

- Add `query-replace-regexp` as an interactive command.
- Add the default Emacs-style binding `C-M-%`.
- Reuse the current line-local regexp subset from `src/search_pattern.rs`.
- Keep replacement text literal in the first implementation.
- Add tests, user docs, NEWS, and ChangeLog entries.

## Non-Goals

- Do not add `replace-regexp` in this phase.
- Do not add capture groups, backreferences, or replacement escapes such as
  `\1`, `\&`, `\,(...)`, or case-conversion directives.
- Do not add grouping, alternation, counted repetition, word-boundary syntax, or
  syntax classes to the regexp engine.
- Do not add multiline regexp matching.
- Do not expand the query-replace choice-key set beyond the current `y`, `n`,
  `!`, `q`, Escape, and `C-g` subset.

## Current Context

- `src/search_pattern.rs` already exposes `PatternKind::{Literal, Regexp}` and
  `SearchPattern::compile`.
- `src/editor/search.rs` already searches a compiled `SearchPattern` across the
  current buffer one line at a time.
- `src/editor.rs` currently implements literal `query-replace` by compiling
  `PatternKind::Literal` inside `advance_query_replace`.
- `src/render/mod.rs` already has `CurrentSearchMatch` and `SearchMatch` faces,
  and query-replace already highlights the current candidate.
- `src/command.rs` currently has `QueryReplace` but no `QueryReplaceRegexp`.
- `src/keymap.rs` binds `M-%` to literal `query-replace`; `C-M-%` is available.
- `docs/emacs-function-reference.md` documents `query-replace` and regexp
  isearch, but not `query-replace-regexp`.

## Assumptions

- The first regexp replacement subset should be honest and conservative:
  regexp matching, literal replacement text.
- Regexp query-replace search history should be separate from literal
  query-replace search history, matching the existing split between literal and
  regexp incremental-search history.
- Zero-width regexp replacements should be rejected initially instead of trying
  to emulate Emacs's nuanced advancement behavior.

## Open Questions

- [ ] Confirm whether invalid regexp messages should include parser details or
  keep the current terse `invalid regexp` style used by regexp isearch.
- [ ] Confirm whether replacement prompt history should be shared between
  literal and regexp query-replace. Recommended: keep it separate for now.

## Tasks

### Command And Prompt Wiring

- [ ] Add `Command::QueryReplaceRegexp` in `src/command.rs`.
- [ ] Register `query-replace-regexp` with a summary and editor handler.
- [ ] Add `C-M-%` key binding in `src/keymap.rs`.
- [ ] Add prompt kinds for regexp search and replacement, such as
  `QueryReplaceRegexpSearch` and `QueryReplaceRegexpReplacement`.
- [ ] Include the new prompt kinds in prompt history support.
- [ ] Route the new prompt kinds in `Editor::submit_prompt`.

### Query-Replace Refactor

- [ ] Refactor `start_query_replace` into a shared helper that accepts a
  `PatternKind` and prompt-label text.
- [ ] Store the selected `PatternKind` and compiled `SearchPattern` in
  `QueryReplaceState`.
- [ ] Change `advance_query_replace` to reuse the stored compiled pattern instead
  of recompiling a literal pattern on every search step.
- [ ] Preserve existing literal `query-replace` behavior and labels.
- [ ] Add regexp labels such as `Query replace regexp: ` and
  `Query replace regexp <pattern> with: `.

### Replacement Boundary

- [ ] Introduce a small replacement abstraction, for example
  `ReplacementTemplate::Literal(String)`.
- [ ] Use the abstraction from `replace_query_replace_current`, even though the
  initial implementation expands to the literal replacement unchanged.
- [ ] Keep capture/backreference expansion out of this phase, but leave the API
  shape ready to accept the matched text or match metadata later.

### Zero-Width Safety

- [ ] Add a helper on or near `SearchPattern` to detect whether a regexp can
  produce a zero-width match for replacement purposes.
- [ ] Reject zero-width regexp replacement patterns with a clear error before
  starting replacement.
- [ ] Cover examples such as `^`, `$`, `a*`, and `a?`.

### Tests

- [ ] Add command registry and keymap tests for `query-replace-regexp` and
  `C-M-%`.
- [ ] Add editor/unit tests showing regexp query-replace stores and reuses a
  compiled regexp pattern.
- [ ] Add tests for invalid regexp rejection.
- [ ] Add tests for zero-width regexp rejection.
- [ ] Add PTY coverage for `C-M-%` replacing matches such as `f.o`.
- [ ] Add PTY coverage for `!` replacing all remaining regexp matches.
- [ ] Add PTY coverage showing prompt history recall for regexp query-replace.
- [ ] Ensure existing literal query-replace tests still pass unchanged.

### Documentation

- [ ] Update `README.md` with `C-M-%` / `query-replace-regexp` and literal-only
  replacement text in the first subset.
- [ ] Update `docs/development.md` with the implemented subset and deferred
  regexp replacement features.
- [ ] Add a `query-replace-regexp` entry to `docs/emacs-function-reference.md`.
- [ ] Add a user-visible `NEWS` entry under the unreleased section.
- [ ] Add GNU-style `ChangeLog` entries.

## Validation

- [ ] Run `make fmt`.
- [ ] Run `./scripts/in-container cargo test --locked search_pattern::tests`.
- [ ] Run `./scripts/in-container cargo test --locked query_replace`.
- [ ] Run `./scripts/in-container cargo test --locked --test pty_search`.
- [ ] Run `make verify`.
- [ ] Run `git diff --check`.

## Risks

- Zero-width regexp matches can cause infinite replacement loops if not rejected
  or advanced carefully.
- Literal-only replacement text may surprise users who expect Emacs replacement
  escapes; documentation must say this explicitly.
- Storing compiled patterns in `QueryReplaceState` requires careful cancellation
  and prompt-transition cleanup so stale state does not survive failed prompts.
- Separate regexp prompt history adds prompt-kind boilerplate but avoids mixing
  literal strings and regexp patterns.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-11 | Plan created. | User requested a written implementation plan. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-11 | Implement `query-replace-regexp` before `replace-regexp`. | It reuses the existing query-replace interaction and gives safer incremental behavior before adding non-query bulk replacement. |
