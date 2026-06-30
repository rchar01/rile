<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Emacs Behavior Reference

## Goal

Create a curated reference for GNU Emacs command behavior before adding more
Emacs-compatible commands to Rile. The reference should capture command names,
default bindings, user-visible mechanics, edge cases, and intentional Rile
differences so implementation work can proceed from written requirements rather
than memory or source-porting.

## Scope

- Document practical Emacs commands that are relevant to Rile's lightweight
  terminal-editor goals.
- Prefer base terminal GNU Emacs behavior from the existing `core` reference
  profile unless a feature specifically concerns modern completion UX.
- Add focused reference scenarios when screenshots or terminal interaction are
  needed to understand prompts, point movement, messages, or visible output.
- Compare each command against Rile's current command registry and mark it as
  implemented, missing, partial, or intentionally different.
- Turn the reference into an implementation backlog for future Rile work.

## Non-Goals

- Do not copy, translate, mechanically port, or vendor Emacs implementation
  code.
- Do not attempt to replicate all of Emacs; Rile should stay a small terminal
  editor for source files, configuration files, Markdown, and UTF-8 text.
- Do not treat package-enabled modern Emacs behavior as canonical base behavior
  unless the command is explicitly about completion UX.
- Do not implement new editor commands before the reference for that command
  group is written and reviewed.

## Current Context

- `tools/reference/emacs/` already builds Emacs reference wrappers and captures
  behavior scenarios for many Rile features.
- Existing Emacs scenarios cover movement, kills, yanks, query replace,
  rectangles, registers, shells, macros, minibuffer completion, help, read-only
  state, and quit prompts.
- Rile's current command surface is defined in `src/command.rs` and default
  bindings are defined in `src/keymap.rs`.
- The current gap analysis shows Rile lacks common Emacs command families such
  as transpose, whitespace cleanup, fill/reflow, commenting, paragraph/sentence
  movement, revert, save-all, and window resizing.

## Assumptions

- The first compatibility target is observable base GNU Emacs behavior in a
  terminal, not every Lisp-level implementation detail.
- Reference captures are evidence for original Rile requirements and tests, not
  a correctness oracle by themselves.
- Command names and default key bindings should follow Emacs when Rile behavior
  is intentionally compatible.
- Exact edge cases may be simplified when full Emacs compatibility would make
  Rile significantly larger or harder to maintain; those differences should be
  explicit in the reference.

## Open Questions

- [x] Should the first reference batch include only commands we expect to
  implement soon, or should it include a broader Rile 1.0 compatibility matrix?
- [x] Should region case conversion be implemented in the same slice as word
  case conversion?
- [x] Should `comment-dwim` depend only on Rile's existing syntax modes, or
  should it introduce a separate comment-syntax table first?
- [x] Should `fill-paragraph` target exact Emacs defaults or a documented small
  subset suitable for plain text and comments?

## Phase 1: Define The Reference Format

Goal: make every future command note consistent and easy to turn into tests.

Tasks:

- [x] Create `docs/emacs-function-reference.md` with a compact command-entry
  template.
- [x] Include fields for command name, default binding, purpose, prompt flow,
  prefix-argument behavior, region behavior, point-after-command behavior, undo
  behavior, read-only behavior, messages, Rile status, and evidence.
- [x] Document how to cite evidence from Emacs manual pages, `describe-function`,
  and `tools/reference/emacs/scenarios/` captures.

Validation gate:

- [x] Review the template against at least two already implemented commands such
  as `join-line` and `query-replace`.

## Phase 2: Inventory The First Command Batch

Goal: write reference entries for high-value missing commands before
implementation.

Tasks:

- [x] Document case commands: `upcase-word`, `downcase-word`,
  `capitalize-word`, `upcase-region`, and `downcase-region`.
- [x] Document whitespace commands: `delete-horizontal-space`,
  `just-one-space`, `delete-blank-lines`, and `delete-trailing-whitespace`.
- [x] Document transpose commands: `transpose-chars`, `transpose-words`, and
  `transpose-lines`.
- [x] Document fill/comment commands: `fill-paragraph`, `comment-dwim`,
  `comment-region`, and `uncomment-region`.
- [x] Document navigation commands: `forward-paragraph`, `backward-paragraph`,
  `forward-sentence`, and `backward-sentence`.

Validation gate:

- [x] Each entry states whether Rile should match Emacs exactly, match a smaller
  subset, or intentionally differ.
- [x] Each entry has at least one evidence source.

## Phase 3: Add Focused Emacs Captures

Goal: collect terminal-visible behavior for commands where docs alone are not
enough.

Tasks:

- [x] Add Emacs `core` scenarios for word and region case conversion.
- [x] Add Emacs `core` scenarios for whitespace cleanup and spacing behavior.
- [x] Add Emacs `core` scenarios for transpose point movement and undo behavior.
- [x] Add Emacs `core` scenarios for fill/comment prompts and resulting text.
- [x] Update `tools/reference/emacs/README.md` with the case-conversion scenario
  names.
- [x] Update `tools/reference/emacs/README.md` with the whitespace scenario names.
- [x] Update `tools/reference/emacs/README.md` with the transpose scenario name.
- [x] Update `tools/reference/emacs/README.md` with later Phase 3 scenario names.

Validation gate:

- [x] Run targeted captures for `case-word-core` and `case-region-core`, then
  inspect generated frames under `artifacts/reference/emacs/`.
- [x] Run targeted captures for `whitespace-spacing-core` and
  `whitespace-cleanup-core`, then inspect generated frames under
  `artifacts/reference/emacs/`.
- [x] Run targeted captures for `transpose-core`, then inspect generated frames
  under `artifacts/reference/emacs/`.
- [x] Run targeted captures for `fill-paragraph-core` and `comment-commands-core`,
  then inspect generated frames under `artifacts/reference/emacs/`.

## Phase 4: Compare Against Rile

Goal: turn the reference into a concrete implementation backlog.

Tasks:

- [x] Add or update a Rile gap table that maps Emacs commands to Rile command
  names, key bindings, implementation status, and priority.
- [x] Rank missing features by usefulness, implementation size, and testability.
- [x] Identify commands that should remain out of scope for Rile 1.0.

Validation gate:

- [x] The backlog has a clear first implementation slice and explicit non-goals.

## Rile Compatibility Backlog

This table covers the curated command entries in
`docs/emacs-function-reference.md`. It intentionally does not claim to be a full
Emacs compatibility matrix.

Priority key: P0 is the next implementation slice, P1 is high-value follow-up, P2
is useful but larger or less urgent, and P3 is deferred for later Rile releases.

| Emacs command | Rile command name | Default binding target | Rile status | Priority | Implementation note |
| --- | --- | --- | --- | --- | --- |
| `join-line` / `delete-indentation` | `join-line` | `M-^` | Implemented subset | Done | Keep current no-prefix line-local behavior; defer prefix and region variants. |
| `query-replace` | `query-replace` | `M-%` | Implemented subset | Done | Keep current choice-key subset; broader Emacs query-replace keys are not part of this backlog. |
| `downcase-word` | `downcase-word` | `M-l` | Implemented subset | Done | Uses Rile word boundaries, UTF-8-safe edits, arguments, and single-command undo. |
| `upcase-word` | `upcase-word` | `M-u` | Implemented subset | Done | Shares the same word-span machinery as `downcase-word`. |
| `capitalize-word` | `capitalize-word` | `M-c` | Implemented subset | Done | Shares the case-word implementation and covers mixed ASCII/UTF-8 input in tests. |
| `downcase-region` | `downcase-region` | `C-x C-l` | Implemented subset | Done | Intentionally skips Emacs disabled-command confirmation. |
| `upcase-region` | `upcase-region` | `C-x C-u` | Implemented subset | Done | Preserves point/mark, keeps the active region adjusted, and has region undo tests. |
| `delete-horizontal-space` | `delete-horizontal-space` | `M-\` | Implemented subset | Done | Uses ASCII space/tab behavior, prefix backward-only deletion, read-only checks, and undo. |
| `just-one-space` | `just-one-space` | No active binding | Missing | P2 | Implement after deciding whether `M-SPC` should eventually map to `cycle-spacing` instead. |
| `delete-blank-lines` | `delete-blank-lines` | `C-x C-o` | Implemented subset | Done | Uses Rile's space/tab-only blank-line definition and covers blank runs, isolated blank lines, and nonblank lines before blanks. |
| `delete-trailing-whitespace` | `delete-trailing-whitespace` | None | Implemented subset | Done | Deletes ASCII spaces/tabs at physical line ends across the whole buffer or active-region bounds. |
| `transpose-chars` | `transpose-chars` | `C-t` | Implemented subset | Done | Uses UTF-8-safe grapheme transposition, repeat arguments, EOL behavior, read-only checks, and undo. |
| `transpose-words` | `transpose-words` | `M-t` | Implemented subset | Done | Uses Rile word boundaries, repeat arguments, punctuation preservation, read-only checks, and undo; defers mark-based zero-argument behavior. |
| `transpose-lines` | `transpose-lines` | `C-x C-t` | Implemented subset | Done | Moves the previous line past the current line or lines, supports repeat arguments, read-only checks, and undo; defers mark-line zero-argument behavior. |
| `fill-paragraph` | `fill-paragraph` | `M-q` | Missing | P2 | Implement plain-text reflow only after paragraph-boundary helpers exist. |
| `comment-dwim` | `comment-dwim` | `M-;` | Missing | P2 | Add reusable comment-syntax metadata first; target line comments before full Emacs DWIM behavior. |
| `comment-region` | `comment-region` | None globally | Missing | P2 | Build as a line-comment subset for known modes; avoid C-mode block-comment parity in the first version. |
| `uncomment-region` | `uncomment-region` | None | Missing | P2 | Implement as the inverse of the line-comment subset. |
| `forward-paragraph` | `forward-paragraph` | `M-}` | Implemented subset | Done | Uses blank-line-separated paragraph movement with spaces/tabs/formfeed separators. |
| `backward-paragraph` | `backward-paragraph` | `M-{` | Implemented subset | Done | Shares paragraph-boundary code with `forward-paragraph`; supports positive and negative arguments. |
| `forward-sentence` | `forward-sentence` | `M-e` | Missing | P3 | Useful but edge rules are subtler than paragraph movement. |
| `backward-sentence` | `backward-sentence` | `M-a` | Missing | P3 | Defer until the sentence-boundary subset is clearly worth the complexity. |

## Ranked Missing Work

1. Fill and comments: `fill-paragraph`, `comment-dwim`, `comment-region`, and
   `uncomment-region`. These are useful but need paragraph wrapping and reusable
   comment-syntax metadata to avoid coupling editing behavior to rendering.
2. Sentence movement: `forward-sentence` and `backward-sentence`. These remain
   deferred until Rile needs sentence-aware prose editing beyond paragraph moves.

Completed first slice: `downcase-word`, `upcase-word`, `capitalize-word`,
`downcase-region`, and `upcase-region` are implemented as documented subsets.

Completed second slice: `delete-horizontal-space`, `delete-blank-lines`, and
`delete-trailing-whitespace` are implemented as documented subsets.

Completed third slice: `forward-paragraph` and `backward-paragraph` are
implemented as documented subsets.

Completed fourth slice: `transpose-chars` is implemented as a documented subset.

Completed fifth slice: `transpose-words` and `transpose-lines` are implemented as
documented subsets.

## Rile 1.0 Non-Goals From This Batch

- Full Emacs disabled-command confirmation for `upcase-region` and
  `downcase-region`.
- Exact Unicode, locale, and syntax-table parity for Emacs case conversion.
- `cycle-spacing` parity or binding `M-SPC` before Rile has a documented spacing
  command decision.
- Zero-argument mark-based transpose variants.
- Full Emacs fill machinery, including justification, fill prefixes,
  sentence-end-double-space customization, CJK/kinsoku behavior, and mode hooks.
- Full Emacs comment machinery, including block comments, `comment-column`
  alignment, `comment-style`, delimiter-count prefix behavior, and mode-specific
  C-mode `C-c C-c` parity.
- Customizable `paragraph-start`, `paragraph-separate`, `sentence-end`, and
  mode-specific paragraph or sentence functions.

## First Implementation Slice

Case conversion is implemented:

- `upcase-word`
- `downcase-word`
- `capitalize-word`
- `upcase-region`
- `downcase-region`

This slice added command registry entries, default Emacs bindings, unit tests for
word and region spans, UTF-8 tests, argument tests, read-only tests, and PTY
coverage for key bindings and visible editing behavior.

The first slice explicitly excludes Emacs disabled-command confirmation for the
region commands and exact Emacs syntax-table or locale behavior.

## Second Implementation Slice

Whitespace cleanup is partially implemented:

- `delete-horizontal-space`
- `delete-blank-lines`
- `delete-trailing-whitespace`

This slice added command registry entries, default Emacs bindings, unit tests for
ASCII spaces/tabs, prefix backward-only deletion, blank-run cleanup, isolated
blank deletion, trailing spaces/tabs, active-region cleanup, read-only behavior,
undo, and PTY coverage for visible default-key behavior.

The second slice explicitly excludes `just-one-space`, `cycle-spacing`,
`delete-trailing-lines` customization, formfeed exceptions, and broader Unicode
whitespace rules.

## Validation

- [ ] Run `git diff --check` after documentation or scenario edits.
- [ ] Run targeted reference captures for new Emacs scenarios.
- [ ] Run focused Rile unit and PTY tests after any future implementation work.
- [ ] Run `make verify` before landing implementation changes.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-06-30 | Implemented the transpose follow-up slice. | `src/command.rs`, `src/keymap.rs`, and `src/editor.rs` now implement `transpose-words` and `transpose-lines`; unit tests cover punctuation, UTF-8, numeric arguments, undo, boundary failures, and read-only behavior; `tests/pty_insert.rs` covers visible `M-t` and `C-x C-t` behavior. |
| 2026-06-30 | Implemented the `transpose-chars` slice. | `src/command.rs`, `src/keymap.rs`, and `src/editor.rs` now implement `transpose-chars`; unit tests cover ordinary, end-of-line, UTF-8, argument, undo, and read-only behavior; `tests/pty_insert.rs` covers visible `C-t` behavior and undo. |
| 2026-06-30 | Implemented the paragraph movement slice. | `src/command.rs`, `src/keymap.rs`, and `src/editor.rs` now implement `forward-paragraph` and `backward-paragraph`; unit tests cover blank-line-separated paragraph movement, formfeed separators, and numeric arguments; `tests/pty_movement.rs` covers visible default-key behavior. |
| 2026-06-29 | Implemented the whitespace cleanup slice. | `src/command.rs`, `src/keymap.rs`, and `src/editor.rs` now implement `delete-horizontal-space`, `delete-blank-lines`, and `delete-trailing-whitespace`; unit tests cover spaces/tabs, prefix behavior, blank runs, isolated blank lines, trailing cleanup, active-region bounds, undo, and read-only behavior; `tests/pty_insert.rs` covers visible default-key behavior for the bound commands. |
| 2026-06-29 | Implemented the Phase 4 case-conversion slice. | `src/command.rs`, `src/keymap.rs`, and `src/editor.rs` now implement `downcase-word`, `upcase-word`, `capitalize-word`, `downcase-region`, and `upcase-region`; unit tests cover word arguments, UTF-8, region preservation, undo, and read-only behavior; `tests/pty_insert.rs` covers visible default-key behavior. |
| 2026-06-29 | Completed Phase 4 Rile comparison and backlog ranking. | `src/command.rs` and `src/keymap.rs` show that only `join-line` and `query-replace` from the curated reference are implemented; the new backlog ranks the remaining documented command families and defines case conversion as the first slice. |
| 2026-06-29 | Added Phase 3 Emacs fill/comment captures. | `tools/reference/emacs/scenarios/fill-paragraph-core.scenario` covers `M-q` fill and undo; `comment-commands-core.scenario` covers `M-;`, `comment-region`, and `uncomment-region` prompts/results in C mode. |
| 2026-06-29 | Added Phase 3 Emacs transpose captures. | `tools/reference/emacs/scenarios/transpose-core.scenario` covers `C-t`, `M-t`, `C-x C-t`, point placement, and undo frames. |
| 2026-06-29 | Added Phase 3 Emacs whitespace captures. | `tools/reference/emacs/scenarios/whitespace-spacing-core.scenario` covers `delete-horizontal-space` and `just-one-space`; `whitespace-cleanup-core.scenario` covers `delete-blank-lines` and `delete-trailing-whitespace`. |
| 2026-06-29 | Added Phase 3 Emacs case-conversion captures. | `tools/reference/emacs/scenarios/case-word-core.scenario` covers `M-l`, `M-u`, and `M-c`; `case-region-core.scenario` covers `C-x C-l`, `C-x C-u`, and disabled-command prompts. |
| 2026-06-29 | Completed Phase 2 first-batch inventory with navigation commands. | `docs/emacs-function-reference.md` now covers `forward-paragraph`, `backward-paragraph`, `forward-sentence`, and `backward-sentence` with Rile targets, Emacs manual evidence, `describe-function` output, and local batch probes. |
| 2026-06-29 | Documented Phase 2 fill/comment commands. | `docs/emacs-function-reference.md` now covers `fill-paragraph`, `comment-dwim`, `comment-region`, and `uncomment-region` with Rile targets, Emacs manual evidence, `describe-function` output, and local batch probes. |
| 2026-06-29 | Documented Phase 2 transpose commands. | `docs/emacs-function-reference.md` now covers `transpose-chars`, `transpose-words`, and `transpose-lines` with Rile targets, Emacs manual evidence, `describe-function` output, and local batch probes. |
| 2026-06-29 | Documented Phase 2 whitespace commands. | `docs/emacs-function-reference.md` now covers `delete-horizontal-space`, `just-one-space`, `delete-blank-lines`, and `delete-trailing-whitespace` with Rile targets and Emacs manual or `describe-function` evidence. |
| 2026-06-29 | Documented Phase 2 case-conversion commands. | `docs/emacs-function-reference.md` now covers `upcase-word`, `downcase-word`, `capitalize-word`, `upcase-region`, and `downcase-region` with Rile targets and Emacs manual evidence. |
| 2026-06-29 | Completed Phase 1 reference-format work. | `docs/emacs-function-reference.md` defines the entry template and validates it with `join-line` and `query-replace` entries. |
| 2026-06-29 | Plan created to pause implementation and write down the Emacs behavior-reference goal. | User requested writing down what we are trying to do before continuing. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-06-29 | Keep the first reference batch focused instead of expanding it into a full Rile 1.0 compatibility matrix. | The documented batch is already enough to drive implementation work; broader gaps such as revert, save-all, and window resizing should get their own evidence before entering the backlog. |
| 2026-06-29 | Implement word and region case conversion in the same first slice. | The commands share transformation, undo, read-only, and UTF-8 concerns, and the region variants are small when disabled-command confirmation is out of scope. |
| 2026-06-29 | Target a documented plain-text subset for the first `fill-paragraph` implementation. | Full Emacs filling includes justification, language-specific breaking, fill prefixes, mode hooks, and comment filling that would be too large for the first Rile slice. |
| 2026-06-29 | Add reusable comment-syntax metadata before implementing comment editing commands. | Existing Rile comment markers are embedded in syntax highlighting and should not become the editing API by accident. |
| 2026-06-29 | Build a curated Emacs behavior reference before adding missing compatibility commands. | Avoid guessing from memory and avoid source-porting while preserving small-editor scope. |
