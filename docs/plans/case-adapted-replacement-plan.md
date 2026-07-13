<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Case-Adapted Replacement

## Goal

Implement Emacs-style case-adapted replacement for Rile replacement commands so
case-folded matches can preserve the matched text's casing in the inserted
replacement text.

## Scope

- Apply to literal `query-replace`, regexp `query-replace-regexp`, and
  non-interactive `replace-regexp`.
- Preserve existing smart-case search behavior from `SearchPattern`.
- Add focused unit tests for replacement casing helpers.
- Add PTY tests for visible replacement behavior.
- Update `README.md`, `NEWS`, `ChangeLog`, `docs/development.md`, and
  `docs/emacs-function-reference.md`.

## Non-Goals

- Do not add a user configuration variable for `case-replace` in the first pass
  unless discovery shows it is necessary for correctness.
- Do not implement full Emacs replacement expressions such as `\,(...)`.
- Do not implement explicit case-conversion replacement directives unless they
  are required for the default case-adaptation behavior.
- Do not change search matching semantics; smart-case search is already
  implemented.

## Current Context

- Smart-case matching was added in commits `89dc219`, `dbf7577`, and
  `cd4210e`.
- Replacement text is currently inserted exactly as typed.
- Docs intentionally call case-adapted replacement a future follow-up.
- Relevant implementation files:
  - `src/editor.rs`: `ReplacementTemplate`, `expand_regexp_replacement`,
    `replace_query_replace_current`, and `replace_regexp_matches`.
  - `src/search_pattern.rs`: shared literal/regexp matching and smart-case
    search behavior.
  - `tests/pty_search.rs`: terminal-level replacement coverage.
- Verification commands:
  - Focused unit tests: `./scripts/in-container cargo test --locked --lib editor`
  - Focused PTY tests: `./scripts/in-container cargo test --locked --test pty_search <filter>`
  - Full gate: `make verify`

## Assumptions

- Rile should follow Emacs's default `case-replace` behavior when the search is
  case-folded.
- Case adaptation should happen after regexp replacement expansion, so `\&` and
  `\1`-style captures participate in the final replacement text exactly as Emacs
  behavior confirms.
- If the search pattern is case-sensitive because it contains unescaped
  uppercase, replacement text should remain exactly as typed.
- Unicode casing should use Rust string case conversion consistently with Rile's
  existing case-conversion commands, while documented edge cases can remain
  deferred if Emacs has locale/syntax-table-specific behavior.

## Open Questions

- [x] Confirm whether default Emacs adapts only all-lowercase replacement text or
  also adapts mixed/uppercase replacement text.
- [x] Confirm how Emacs adapts regexp replacements containing `\&`, numbered
  captures, and literal surrounding text.
- [x] Confirm how Emacs treats mixed-case matches such as `StaTUS`.
- [x] Decide whether the first implementation should expose a `case_replace`
  configuration option or keep the Emacs default always on.

## Phase 1: Reference Discovery

Goal: Pin the user-visible behavior before changing replacement code.

Tasks:

- [x] Use local GNU Emacs batch checks or repository reference tooling to record
  literal `query-replace` behavior for `status`, `Status`, `STATUS`, and a
  mixed-case match.
- [x] Record regexp `replace-regexp` behavior for lowercase search/replacement
  against `Status status STATUS`.
- [x] Record regexp replacement behavior for `\&`, numbered captures, and
  literal text around captures.
- [x] Record whether uppercase or mixed-case replacement input is adapted.
- [x] Add findings to this plan's Progress Log before implementation.

Validation gate:

- [x] Reference findings are concrete enough to write expected test outputs.

Decision point: Confirm whether implementation proceeds with always-on default
case adaptation or a config-backed option.

## Phase 2: Unit-Level Implementation

Goal: Add the smallest shared helper that adapts replacement text consistently.

Depends on: Phase 1 findings.

Tasks:

- [ ] Add unit tests in `src/editor.rs` for lowercase, capitalized, all-uppercase,
  and mixed-case match inputs.
- [ ] Add unit tests for regexp replacement expansion followed by case
  adaptation.
- [ ] Implement a helper that receives the matched text, expanded replacement
  text, and search case mode, then returns the adapted replacement text.
- [ ] Apply the helper in both query-replace and `replace-regexp` replacement
  paths.
- [ ] Keep exact replacement behavior when the search is case-sensitive.

Validation gate:

- [ ] Run `./scripts/in-container cargo test --locked --lib editor`.

## Phase 3: PTY Coverage

Goal: Pin behavior through real terminal commands.

Depends on: Phase 2 implementation.

Tasks:

- [ ] Add `query-replace` PTY coverage for lowercase replacement adapting
  `status`, `Status`, and `STATUS`.
- [ ] Add `query-replace-regexp` PTY coverage for the same behavior.
- [ ] Add `replace-regexp` PTY coverage for the same behavior.
- [ ] Add at least one uppercase search PTY case proving exact replacement text
  still applies when the search is case-sensitive.

Validation gate:

- [ ] Run focused PTY filters in `tests/pty_search.rs`.

## Phase 4: Documentation And Release Notes

Goal: Update user-facing and implementation docs after behavior is verified.

Depends on: Phase 3 tests.

Tasks:

- [ ] Update `README.md` replacement command notes.
- [ ] Update `docs/development.md` current replacement behavior.
- [ ] Update `docs/emacs-function-reference.md` for `query-replace`,
  `query-replace-regexp`, and `replace-regexp`.
- [ ] Add a user-visible `NEWS` entry under unreleased changes.
- [ ] Add a GNU-style `ChangeLog` entry.

Validation gate:

- [ ] Run `git diff --check`.

## Phase 5: Final Verification And Cleanup

Goal: Ensure the implementation is merge-ready and the plan state is accurate.

Depends on: Phases 1 through 4.

Tasks:

- [ ] Run `make verify`.
- [ ] Run patch review over the case-adapted replacement commits.
- [ ] Address any review findings with focused tests and rerun relevant gates.
- [ ] Commit implementation, docs, and follow-ups in logical commits.
- [ ] Update this plan's Progress Log and Decision Log.

Validation gate:

- [ ] `make verify` passes.
- [ ] Patch review reports no must-fix findings.
- [ ] Worktree is clean after commits.

## Risks

- Emacs case adaptation may depend on `case-replace`, search mode, and
  replacement text shape in ways that are easy to oversimplify.
- Regexp capture expansion can produce mixed replacement text where adaptation
  order matters.
- Unicode casing can change byte length; replacement code must continue using
  valid UTF-8 positions and existing buffer APIs.
- Always-on behavior may later need a config option if users need exact
  replacement text while keeping case-folded search.

## Validation Summary

- [ ] Focused unit tests pass.
- [ ] Focused PTY tests pass.
- [ ] `git diff --check` passes.
- [ ] `make verify` passes.
- [ ] Patch review is clean or documented.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-13 | Plan created for Emacs-style case-adapted replacement follow-up. | User requested a plan under `docs/plans/` before implementation. |
| 2026-07-13 | Completed Emacs reference discovery. Lowercase `status` to `state` yields `state State STATE`; `StaTUS` maps to capitalized output, lowercase-first mixed case stays lowercase, all-uppercase matches upcase the full replacement, and regexp replacement adaptation happens after capture/whole-match expansion. | Local GNU Emacs `--batch --quick` checks using `replace-string` and `replace-regexp`. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-13 | Treat case-adapted replacement as separate from smart-case search. | Smart-case search is already implemented; replacement casing has additional Emacs-specific edge cases. |
| 2026-07-13 | Implement first pass as always-on default behavior instead of adding `case_replace` config. | This matches Emacs defaults and avoids introducing configuration before there is a user need for disabling it. |
