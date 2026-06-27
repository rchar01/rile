<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Remove Regex From Completion Matching

## Goal

Remove Rile's runtime `regex` dependency from completion matching while keeping
the useful Emacs-modern completion workflow: orderless literal components, smart
case, negation, force-literal components, selected-candidate interaction, raw
submission, and file-prompt special handling.

The migration should be explicit that Rile remains Emacs-modern-like for common
interactive completion, but no longer implements full Orderless regexp-component
semantics unless the project chooses to keep the dependency.

## Scope

- Replace `regex`-backed orderless components in `src/completion.rs` with a
  small string matcher.
- Preserve command, option, and buffer orderless completion semantics that do
  not require a regular-expression engine.
- Preserve file completion behavior, including the file-category override.
- Update unit tests, PTY tests, README/user docs, development docs, reference
  testing notes, `NEWS`, and `ChangeLog` to match the behavior change.
- Remove `regex = "1"` from runtime dependencies in `Cargo.toml` and refresh
  `Cargo.lock` through normal Cargo commands.

## Non-Goals

- Do not implement a custom regular-expression parser or engine.
- Do not change completion style selection, candidate rendering, prompt history,
  or Enter/Tab policy except where tests must be renamed for the regex removal.
- Do not change command names, option names, key bindings, or config keys.
- Do not remove `completion_matching = "orderless"`.
- Do not attempt to remove test-only transitive uses of `regex` pulled by dev
  dependencies such as `expectrl`, `assert_cmd`, or `predicates`.

## Current Context

- Runtime `regex` use is isolated to `src/completion.rs`.
- `regex` is currently used only for non-file orderless completion components.
- File prompts already override global orderless matching with file-category
  behavior.
- Current docs advertise regular-expression components in `README.md`,
  `docs/development.md`, and `docs/reference-testing.md`.
- Current tests cover regex behavior in `src/completion.rs` and
  `tests/pty_completion.rs`.
- `make verify` currently passes with the regex-backed implementation.

## Compatibility Target

The target is common Emacs-modern completion UX, not exact Emacs Orderless
regexp compatibility.

Preserve these behaviors:

- `completion_matching = "orderless"` remains the default for command, option,
  and buffer prompts.
- Space-separated components match in any order.
- Every positive component must match.
- `re md` matches `readme.md`.
- `md re` matches `readme.md`.
- `find file` matches `find-file`.
- `file find` matches `find-file`.
- Lowercase components match case-insensitively.
- Components containing uppercase letters match case-sensitively.
- Ranking continues to prefer exact, then prefix, then word-boundary, then
  substring matches.
- `!foo` rejects candidates matching the component `foo`.
- `=foo` forces literal treatment of the component.
- `!=foo` combines negation and force-literal matching.
- Backslash-escaped spaces continue to keep a space inside one component.
- Tab continues to insert or extend the selected candidate.
- Enter acceptance policy remains unchanged.
- `M-RET` continues to submit raw minibuffer input.
- Explicitly moved selections continue to win over exact typed text.
- Exact typed command, option, buffer, or file text is preserved where current
  policy already preserves it.

Preserve these file-prompt behaviors:

- File prompts continue to use file-category matching instead of global orderless
  command-style matching.
- Raw missing-file input continues to be accepted for file prompts through
  `M-RET` when a candidate is selected.
- Directory candidates continue to descend after Tab insertion, exact input, or
  explicit selection.
- Smart-case matching continues to apply to file prompts.
- Orderless command-style matching does not hijack missing file names.

## Intentional Behavior Change

Full regular-expression components stop being supported for completion matching.

These forms must no longer be documented as regex features:

- `foo\.txt`
- `f.nd`
- `foo|bar`
- `[abc]`
- `(foo|bar)`
- `.*`
- character classes
- groups
- alternation
- arbitrary regular-expression anchors

After migration, regex metacharacters should be literal text unless they are one
of the explicitly supported simple anchors described below.

## Simple Anchor Policy

Keep a small handwritten anchor convenience because it preserves common
interactive examples without keeping a regex engine.

- `^foo` means the candidate must start with literal `foo`.
- `foo$` means the candidate must end with literal `foo`.
- `^foo$` means the candidate must exactly equal literal `foo`.
- `=^foo` means literal text `^foo`, not an anchor.
- `=foo$` means literal text `foo$`, not an anchor.
- `^` by itself is literal `^` unless the implementation chooses to treat it as
  an empty prefix component with test coverage.
- `$` by itself is literal `$` unless the implementation chooses to treat it as
  an empty suffix component with test coverage.

The simple anchor matcher should use the same smart-case rule as literal
components.

## Proposed Implementation

### Phase 0: Pre-Migration Characterization

Goal: capture additional behavior that must survive the later matcher rewrite
before changing implementation code. Existing tests already characterize current
regex-specific behavior that Phase 3 will replace or intentionally change.

Tasks:

- [x] Add explicit unit coverage showing `re md` and `md re` both match
  `readme.md`.
- [x] Add explicit unit coverage showing backslash-escaped spaces remain part of
  one orderless component while other components still match orderlessly.
- [x] Add explicit unit coverage showing force-literal matching preserves anchor
  text such as `=^foo` and `=foo$`.
- [x] Confirm existing unit coverage captures current regex anchors, invalid
  regex fallback, and regex-metacharacter behavior that will change during the
  migration.

Validation gate:

- [x] Run focused completion tests before starting Phase 1.

### Phase 1: Matcher Design

Goal: replace regex-backed components with explicit string-match kinds.

Tasks:

- [x] Replace `OrderlessMatcher::Regex(Regex)` with a non-regex matcher enum in
  `src/completion.rs`.
- [x] Add matcher kinds for literal substring, literal prefix anchor, literal
  suffix anchor, and literal exact anchor.
- [x] Keep existing component flags for negation and force-literal matching.
- [x] Ensure `=foo` bypasses anchor parsing and treats all characters literally.
- [x] Keep component splitting behavior unchanged, including escaped spaces.

Validation gate:

- [x] Unit tests cover literal, negated, force-literal, escaped-space, and simple
  anchor components.

### Phase 2: Runtime Dependency Removal

Goal: remove `regex` from Rile's normal runtime dependency graph.

Tasks:

- [x] Remove `use regex::{Regex, RegexBuilder};` from `src/completion.rs`.
- [x] Remove `regex = "1"` from `[dependencies]` in `Cargo.toml`.
- [x] Refresh `Cargo.lock` with the normal locked/container workflow.
- [x] Confirm `cargo tree --locked -e normal -i regex` no longer points from
  `regex` to `rile` through normal dependencies.

Validation gate:

- [x] `cargo tree --locked -e normal` shows no normal runtime `regex` dependency
  under `rile`.

### Phase 3: Test Migration

Goal: prove preserved matching behavior and document intentional differences.

Tasks:

- [x] Replace `orderless_completion_supports_regex_and_literal_fallback` with
  tests for simple anchors and literal metacharacters.
- [x] Keep tests proving `find file` and `file find` both match `find-file`.
- [x] Add or keep tests proving `re md` and `md re` match `readme.md`.
- [x] Keep tests for smart case.
- [x] Keep tests for `!foo`, `=foo`, and `!=foo`.
- [x] Add tests proving `foo.txt` treats `.` as literal text after migration.
- [x] Add tests proving `=^foo` and `=foo$` are literal, not anchors.
- [x] Add tests proving bare `^` and `$` are literal text, not empty anchors.
- [x] Rename or replace `vertical_mx_completion_matches_regex_anchor` in
  `tests/pty_completion.rs` with a simple-anchor PTY test.
- [x] Keep relevant file-completion PTY tests unchanged unless behavior actually
  changes, which should be treated as a bug.

Validation gate:

- [x] Run `./scripts/in-container cargo test --locked completion -- --nocapture`.
- [x] Run `./scripts/in-container cargo test --locked --test pty_completion -- --nocapture`.

### Phase 4: Documentation And Release Notes

Goal: make public documentation match the new matching language.

Tasks:

- [x] Update `README.md` completion table to replace regular-expression
  components with simple literal anchors.
- [x] Update `docs/development.md` to remove claims that valid regular
  expressions are honored.
- [x] Update `docs/reference-testing.md` to stop saying reference captures cover
  regexp completion correctness.
- [x] Add a `NEWS` entry noting that completion matching now uses literal
  orderless components with simple anchors and no runtime regex engine.
- [x] Add a GNU-style `ChangeLog` entry for changed files.

Validation gate:

- [x] Search the repo for stale `regex`, `regexp`, and `regular expression`
  wording and either update it or confirm it refers to dev dependencies or
  historical context.

### Phase 5: Full Verification

Goal: verify the migration did not regress unrelated editor behavior.

Tasks:

- [x] Run `make fmt`.
- [x] Run `git diff --check`.
- [x] Run `make verify`.
- [x] Review `git diff` for accidental behavior changes outside completion and
  documentation.

Validation gate:

- [x] Full `make verify` passes.
- [x] Final dependency tree confirms `regex` is not a normal runtime dependency
  of `rile`.

## Risks

- Users who rely on true Orderless regexp components will lose that behavior.
- Documentation must be updated carefully because regex support is currently
  advertised in multiple places.
- `Cargo.lock` may still contain `regex` through dev dependencies; the success
  criterion is removing it from normal runtime dependencies, not necessarily
  removing every lockfile occurrence.
- Simple anchors may look like regex anchors, so docs and tests must be clear
  that only literal anchoring is supported.

## Open Questions

- [ ] Should `^` and `$` alone be treated as literal characters or as empty
  anchors? Recommended answer: literal characters, because empty anchors are not
  useful in completion prompts.
- [ ] Should `foo.txt` match only literal `foo.txt`, or should existing behavior
  where regex `.` matched any character be temporarily preserved with special
  compatibility code? Recommended answer: make `.` literal and document the
  intentional simplification.
- [ ] Is full Emacs Orderless regexp-component compatibility required for Rile
  1.0? Recommended answer: no, unless exact Emacs Orderless compatibility becomes
  an explicit release goal.

## Acceptance Criteria

- `regex` is absent from `[dependencies]` in `Cargo.toml`.
- `src/completion.rs` contains no `regex` crate imports or regex builders.
- Command, option, and buffer completion still support orderless literal
  components, smart case, negation, force-literal matching, and simple anchors.
- File completion behavior remains unchanged.
- Public docs no longer promise full regular-expression component support.
- Focused completion tests and `make verify` pass.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-06-27 | Plan created. | User requested a written migration plan before implementation. |
| 2026-06-27 | Added pre-migration characterization coverage to the plan. | `src/completion.rs` tests cover `re md`, `md re`, escaped spaces with another component, and force-literal anchor text. |
| 2026-06-27 | Verified pre-migration completion coverage. | `make fmt`, `./scripts/in-container cargo test --locked completion -- --nocapture`, and `git diff --check` passed. |
| 2026-06-27 | Removed runtime regex-backed completion matching. | `make verify` passed; `cargo tree --locked -e normal -i regex` reported no normal dependency path from `rile` to `regex`. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-06-27 | Plan targets Emacs-modern-like UX, not exact Orderless regexp compatibility. | Removing `regex` cannot preserve full regexp semantics without writing a custom regex engine. |
