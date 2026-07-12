<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Upgrade Regexp Module

## Goal

Upgrade Rile's built-in regexp engine from the current small search subset to a
practical Emacs-aligned subset for regexp search, `query-replace-regexp`, and
`replace-regexp`. The goal is functional compatibility for the most important
editing workflows, not a full clone of every GNU Emacs regexp feature.

## Scope

- Keep existing support for `.`, `*`, `+`, `?`, `^`, `$`, escaped literals, and
  character classes.
- Add Emacs-style grouping with `\(...\)`.
- Add Emacs-style alternation with `\|`.
- Add Emacs-style counted repetition with `\{m\}`, `\{m,\}`, and `\{m,n\}`.
- Track numbered captures for grouped subexpressions.
- Add replacement expansion for `\&` and `\1` through `\9`.
- Add common word constructs such as `\<`, `\>`, `\b`, `\B`, `\w`, and `\W`.
- Add basic POSIX bracket classes inside `[...]`, such as `[:alpha:]`,
  `[:digit:]`, and `[:space:]`.
- Preserve UTF-8-safe byte ranges and current line-local matching.
- Keep invalid-regexp diagnostics explicit enough for tests and user messages.

## Non-Goals

- No full Emacs syntax-table integration.
- No Emacs category constructs.
- No non-greedy operators.
- No multiline regexp matching.
- No locale-specific collation or equivalence classes.
- No named capture groups.
- No replacement expression evaluation such as `\,(...)`.
- No broad search behavior changes such as wrapping or region narrowing.

## Compatibility Direction

Rile should follow Emacs regexp syntax where it differs from common PCRE syntax:

- Grouping is `\(...\)`, not `(...)`.
- Alternation is `\|`, not `|`.
- Counted repetition is `\{m,n\}`, not `{m,n}`.
- Bare `(`, `)`, `{`, `}`, and `|` remain literals unless used through the
  Emacs escaped forms.

This keeps regexp behavior aligned with the editor model Rile is already
following and avoids accidentally exposing a second PCRE-like regexp language.

## Current Context

- `src/search_pattern.rs` now parses regexps into an expression/sequence AST
  foundation.
- The current matcher handles one line at a time and returns byte ranges.
- Existing regexp support includes `.`, `*`, `+`, `?`, `^`, `$`, Emacs-style
  grouping, alternation, counted repetition, escaped metacharacters, and
  character classes with ranges and negation.
- Current code tracks numbered captures internally but has no
  replacement-expansion API yet.
- `query-replace-regexp` and `replace-regexp` intentionally use literal
  replacement text today.

## Assumptions

- The small expression/sequence AST should remain the internal representation as
  replacement expansion is added.
- Matching should stay line-local until there is a separate design for multiline
  buffer-spanning matches.
- Captures should be represented as byte ranges into the original line text.
- `can_match_empty` remains required because replacement commands reject
  empty-match-capable regexps until Rile has a deliberate empty-match replacement
  policy.

## Open Questions

- [ ] Should `\s-`, `\sw`, and other Emacs syntax-class constructs be deferred
  entirely, or should a tiny hard-coded subset exist before syntax tables?
- [ ] Should replacement expansion treat unmatched captures as empty strings or
  preserve the typed escape literally? Emacs behavior should be checked before
  implementation.
- [ ] Should `\w` use Rust Unicode word-like semantics, ASCII plus underscore, or
  Rile's existing word-motion definition?

## Phase 1: Parser Refactor

Goal: Make the regexp representation extensible without changing user-visible
behavior yet.

Tasks:

- [x] Replace the flat piece list with a regexp expression/sequence AST
  foundation. Explicit grouping and alternation syntax remain Phase 2 work.
- [x] Preserve current successful pattern behavior for the existing subset.
- [x] Preserve or improve current invalid-pattern rejection for trailing escapes,
  repeated quantifiers, unterminated classes, empty classes, and invalid ranges.
- [x] Add parser tests for the existing subset so later phases can refactor with
  confidence.

Validation gate:

- [x] Run `./scripts/in-container cargo test --locked --lib search_pattern`.

## Phase 2: Alternation, Groups, And Counts

Goal: Add the most important missing Emacs regexp structure.

Tasks:

- [x] Add `\(...\)` grouping.
- [x] Add `\|` alternation with Emacs-style precedence.
- [x] Add counted repetition with `\{m\}`, `\{m,\}`, and `\{m,n\}`.
- [x] Keep bare `(`, `)`, `{`, `}`, and `|` as literal characters.
- [x] Add tests for nested groups, alternatives, quantified groups, and malformed
  escaped constructs.

Validation gate:

- [x] Run `./scripts/in-container cargo test --locked --lib search_pattern`.
- [x] Run focused regexp incremental-search tests.

## Phase 3: Match Objects And Captures

Goal: Return enough match information for replacement expansion.

Tasks:

- [x] Introduce a match result that includes the whole-match byte range and
  numbered capture ranges.
- [x] Preserve the existing public search-pattern APIs where callers only need a
  range, or migrate callers minimally.
- [x] Track captures through backtracking without exposing invalid UTF-8 byte
  boundaries.
- [x] Add tests for captures, unmatched captures, nested captures, and repeated
  captures.

Validation gate:

- [x] Run `./scripts/in-container cargo test --locked --lib search_pattern`.
- [x] Run existing search and query-replace focused tests.

## Phase 4: Replacement Expansion

Goal: Make regexp replacement text functionally useful for Emacs-style workflows.

Tasks:

- [ ] Add a replacement expansion API for regexp matches.
- [ ] Support `\&` for the whole match.
- [ ] Support `\1` through `\9` for numbered captures.
- [ ] Define and test escaping rules for literal backslashes and unsupported
  replacement escapes.
- [ ] Use replacement expansion in `query-replace-regexp`.
- [ ] Use replacement expansion in `replace-regexp`.

Examples:

- Pattern `\(foo\)-\(bar\)`, replacement `\2-\1`, input `foo-bar`, output
  `bar-foo`.
- Pattern `f.o`, replacement `[\&]`, input `foo fxo`, output `[foo] [fxo]`.

Validation gate:

- [ ] Run focused `query-replace-regexp` tests.
- [ ] Run focused `replace-regexp` tests after the command exists.
- [ ] Run PTY replacement tests for visible prompt and replacement behavior.

## Phase 5: Word And Class Constructs

Goal: Add the regexp forms that matter most in everyday editing searches.

Tasks:

- [ ] Add `\<` and `\>` for beginning and end of word.
- [ ] Add `\b` and `\B` for word-boundary and non-word-boundary matching.
- [ ] Add `\w` and `\W` for word and non-word characters.
- [ ] Add basic POSIX bracket classes inside character classes:
  `[:alpha:]`, `[:digit:]`, `[:alnum:]`, `[:space:]`, `[:lower:]`, and
  `[:upper:]`.
- [ ] Decide whether `[:word:]` should be supported as a Rile extension or only
  if reference Emacs behavior justifies it.

Examples:

- Pattern `\<cat\>` matches `cat` as a word, not the `cat` in `concatenate`.
- Pattern `[[:digit:]]\{2,4\}` matches two to four digits.

Validation gate:

- [ ] Run `./scripts/in-container cargo test --locked --lib search_pattern`.
- [ ] Run focused regexp isearch and replacement PTY tests.

## Phase 6: Documentation And Release Notes

Goal: Keep user-facing regexp behavior explicit and maintainable.

Tasks:

- [ ] Update `README.md` with the supported regexp subset and replacement
  expansion examples.
- [ ] Update `NEWS` with user-visible regexp improvements.
- [ ] Update `ChangeLog` with GNU-style source-history entries.
- [ ] Update `docs/development.md` current limitations.
- [ ] Update `docs/emacs-function-reference.md` for regexp search,
  `query-replace-regexp`, and `replace-regexp`.

Validation gate:

- [ ] Run `git diff --check`.

## Full Validation

- [ ] Run `./scripts/in-container cargo test --locked --lib search_pattern`.
- [ ] Run focused regexp isearch tests.
- [ ] Run focused `query-replace-regexp` tests.
- [ ] Run focused `replace-regexp` tests after that command exists.
- [ ] Run `./scripts/in-container cargo test --locked --test pty_search`.
- [ ] Run `make verify`.
- [ ] Run patch verification on the final diff.

## Risks

- Backtracking can become slow on pathological patterns; tests should include a
  few stress cases before broadening syntax further.
- Capture tracking can be subtly wrong around nested groups, repeated groups, and
  alternation branches.
- Emacs regexp syntax differs from PCRE syntax, so examples must be clear to
  avoid user confusion.
- Replacement expansion can break expectations if literal backslash handling is
  not documented and tested.
- Word-boundary behavior depends on what Rile treats as a word character; this
  should align with editor word-motion behavior unless Emacs evidence suggests a
  better local rule.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-12 | Phase 3 internal match objects and numbered capture ranges completed. | `src/search_pattern.rs`; `./scripts/in-container cargo test --locked --lib search_pattern` and `./scripts/in-container cargo test --locked --test pty_search regexp` passed. |
| 2026-07-12 | Phase 2 grouping, alternation, and counted repetition completed. | `src/search_pattern.rs`; `./scripts/in-container cargo test --locked --lib search_pattern` and `./scripts/in-container cargo test --locked --test pty_search regexp` passed. |
| 2026-07-12 | Phase 1 AST foundation completed without user-visible regexp changes. | `src/search_pattern.rs`; `./scripts/in-container cargo test --locked --lib search_pattern`, `./scripts/in-container cargo test --locked --lib regexp`, and `./scripts/in-container cargo test --locked --test pty_search regexp` passed. |
| 2026-07-12 | Plan created for a practical Emacs-aligned regexp subset. | User requested a durable plan in `docs/plans/`; current `src/search_pattern.rs` and regexp docs inspected. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-12 | Target Emacs regexp syntax rather than PCRE syntax. | Rile is an Emacs-style editor and existing commands are named after Emacs regexp commands. |
| 2026-07-12 | Keep matching line-local for this upgrade. | Current search infrastructure is line-local, and multiline matching needs a separate buffer-spanning design. |
| 2026-07-12 | Defer full syntax-table and replacement-expression support. | They are less important than groups, alternation, counts, captures, and ordinary replacement expansion. |
| 2026-07-12 | Split Phase 1 into an AST foundation before new syntax. | This keeps all existing regexp users stable before adding Emacs grouping, alternation, and captures. |
