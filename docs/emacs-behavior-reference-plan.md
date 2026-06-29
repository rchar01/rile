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
  as case conversion, transpose, whitespace cleanup, fill/reflow, commenting,
  paragraph/sentence movement, revert, save-all, and window resizing.

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

- [ ] Should the first reference batch include only commands we expect to
  implement soon, or should it include a broader Rile 1.0 compatibility matrix?
- [ ] Should region case conversion be implemented in the same slice as word
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
- [ ] Add Emacs `core` scenarios for transpose point movement and undo behavior.
- [ ] Add Emacs `core` scenarios for fill/comment prompts and resulting text.
- [x] Update `tools/reference/emacs/README.md` with the case-conversion scenario
  names.
- [x] Update `tools/reference/emacs/README.md` with the whitespace scenario names.
- [ ] Update `tools/reference/emacs/README.md` with later Phase 3 scenario names.

Validation gate:

- [x] Run targeted captures for `case-word-core` and `case-region-core`, then
  inspect generated frames under `artifacts/reference/emacs/`.
- [x] Run targeted captures for `whitespace-spacing-core` and
  `whitespace-cleanup-core`, then inspect generated frames under
  `artifacts/reference/emacs/`.
- [ ] Run targeted captures for later Phase 3 scenarios with `make
  reference-capture REF_EDITOR=emacs REF_SCENARIO=<scenario>` and inspect
  generated frames under `artifacts/reference/emacs/`.

## Phase 4: Compare Against Rile

Goal: turn the reference into a concrete implementation backlog.

Tasks:

- [ ] Add or update a Rile gap table that maps Emacs commands to Rile command
  names, key bindings, implementation status, and priority.
- [ ] Rank missing features by usefulness, implementation size, and testability.
- [ ] Identify commands that should remain out of scope for Rile 1.0.

Validation gate:

- [ ] The backlog has a clear first implementation slice and explicit non-goals.

## Likely First Implementation Slice

Start with case conversion after the reference is written:

- `upcase-word`
- `downcase-word`
- `capitalize-word`
- `upcase-region`
- `downcase-region`

This slice is useful, bounded, easy to test without terminal I/O, and close to
the user's example gap.

## Validation

- [ ] Run `git diff --check` after documentation or scenario edits.
- [ ] Run targeted reference captures for new Emacs scenarios.
- [ ] Run focused Rile unit and PTY tests after any future implementation work.
- [ ] Run `make verify` before landing implementation changes.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
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
| 2026-06-29 | Target a documented plain-text subset for the first `fill-paragraph` implementation. | Full Emacs filling includes justification, language-specific breaking, fill prefixes, mode hooks, and comment filling that would be too large for the first Rile slice. |
| 2026-06-29 | Add reusable comment-syntax metadata before implementing comment editing commands. | Existing Rile comment markers are embedded in syntax highlighting and should not become the editing API by accident. |
| 2026-06-29 | Build a curated Emacs behavior reference before adding missing compatibility commands. | Avoid guessing from memory and avoid source-porting while preserving small-editor scope. |
