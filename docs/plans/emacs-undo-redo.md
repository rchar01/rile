<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Emacs-Style Undo And Redo

## Goal

Implement Emacs-like undo traversal for Rile. Repeated `undo` should walk
backward through edits, a non-undo command should close that undo sequence, and
later `undo` should be able to redo by undoing the recorded undo sequence.

## Scope

- Current-buffer undo and redo traversal.
- Undo sequence tracking across command boundaries.
- `undo`, `undo-only`, and `undo-redo` command behavior.
- Interaction with Rile's saved-state modified flag tracking.
- Reference scenarios, unit tests, PTY tests, user docs, `NEWS`, and
  `ChangeLog` updates.

## Non-Goals

- Selective active-region undo in the first implementation pass.
- Emacs Lisp-compatible `buffer-undo-list` internals.
- Undo history size limits and pruning, unless the new model requires a guard.
- Persisting undo history across editor sessions.
- External crates for undo management.

## Current Context

- `src/buffer/undo.rs` defines `Insert`, `Delete`, `Replace`, and `Batch`
  records.
- `src/editor.rs` stores a global undo stack tagged by `BufferId` and removes a
  matching current-buffer entry on `undo`.
- Normal printable typing is grouped in `record_insert` until another command
  interrupts it.
- Rile tracks a per-buffer clean undo depth so undoing back to saved contents
  clears the modified flag.
- `README.md` and `docs/development.md` currently document that redo and
  advanced Emacs undo traversal are not implemented.
- Emacs reference scenarios live under `tools/reference/emacs/scenarios/` and
  are behavior evidence only.

## Assumptions

- User-visible Emacs behavior is the compatibility target; Rile does not need to
  copy Emacs' internal undo-list representation.
- `undo` should remain buffer-local even though the storage is a global stack
  tagged by buffer.
- A normal command boundary, including movement and self-insert, is the point at
  which a just-performed undo sequence becomes redoable.
- The current clean-depth modified-state logic may need adjustment because
  recording an undo sequence can change undo depth without changing text.

## Edge Cases To Preserve

- A single edit followed by `undo`, harmless movement, then `undo` redoes the
  edit.
- Multiple undone edits redo in the same order Emacs shows after an undo-sequence
  break.
- Typing after undo starts a new branch: the next undo removes the new typing
  before any older undone edit is considered.
- `undo-only` immediately after `undo` continues undo traversal without redoing
  the just-undone edit.
- `undo-only` after a non-undo command boundary should not redo a just-undone
  edit; in the single-edit boundary case it reports no further undo information.
- `undo-redo` redoes a previous undo without adding another redo layer for
  itself.
- Repeated `undo-redo` should not oscillate by recording itself as undoable.
- Undo/redo should remain buffer-local after switching buffers.
- Undo sequence finalization must not create text changes by itself.
- Save, `write-file`, `not-modified`, revert, and opened-file clean points must
  keep accurate modified-state behavior through undo and redo.
- Undoing back to a clean point clears the modified flag; redoing away from it
  marks the buffer modified.
- If an undo sequence is finalized while the text is clean, the saved clean depth
  may need to move to the new metadata depth.
- Compound `Batch` edits must invert in the correct order.
- Failed or no-op commands should not create bogus redo records.
- Minibuffer prompts and command cancellation should not corrupt active undo
  sequence state.
- Read-only buffers should not perform undo or redo mutations.
- Killing a buffer should remove or ignore stale undo and redo sequence state for
  that buffer.

## Open Questions

- [ ] Should `undo-redo` have a default key binding in the first pass, or only be
  available as `M-x undo-redo`?
- [ ] Should Rile add aliases for `C-/` and `C-x u` at the same time as redo, or
  keep only the existing `C-_` binding initially?
- [ ] Should active-region selective undo be a follow-up plan or a later phase in
  this plan?

## Phase 1: Reference And Planning

Goal: Document behavior before changing Rile undo code.

Tasks:

- [x] Add this durable implementation plan.
- [x] Add a base Emacs scenario for one-edit undo, command-boundary break, and
  redo via `undo`.
- [x] Add a base Emacs scenario for multi-edit undo and redo order.
- [x] Add a base Emacs scenario for branch behavior when typing after undo.
- [x] Add a base Emacs scenario for `undo-only` and `undo-redo`.
- [x] List the new scenarios in `tools/reference/emacs/README.md`.
- [x] Run targeted reference captures for the new scenarios.

Validation gate:

- [x] `make reference-capture REF_EDITOR=emacs REF_SCENARIO=<scenario>` passes
  for each new scenario.
- [x] `git diff --check` passes.
- [x] The first-part diff contains only planning, reference evidence, and metadata,
  not Rile undo implementation code.

## Phase 2: Undo Record Inversion

Goal: Add internal helpers that can represent undoing an undo.

Tasks:

- [ ] Add a helper to invert one `UndoRecord`.
- [ ] Add `Batch` inversion that preserves user-visible command order.
- [ ] Add unit tests for each record kind and batch inversion.
- [ ] Keep record inversion independent from editor command dispatch.

Validation gate:

- [ ] Focused unit tests prove insert, delete, replace, and batch inversion.

## Phase 3: Undo Sequence State

Goal: Track active undo sequences without exposing redo commands yet.

Tasks:

- [ ] Add editor state for the currently active undo sequence.
- [ ] When `undo` applies a normal edit record, collect the inverse record in the
  active sequence instead of discarding it.
- [ ] Add a command-boundary helper that finalizes the active sequence into the
  undo stack when a non-undo command runs.
- [ ] Ensure buffer switching keeps per-buffer sequence state correct.
- [ ] Ensure buffer killing clears stale active sequence state.

Validation gate:

- [ ] Unit tests show a non-undo command after undo creates a redoable entry.

## Phase 4: Core Redo Via Undo

Goal: Make plain `undo` redo after an undo sequence is broken.

Tasks:

- [ ] Teach `undo` to apply finalized undo-sequence entries as redo operations.
- [ ] Preserve repeated redo order for multi-edit undo sequences.
- [ ] Keep new typing after undo as a branch that is undone before older redo
  entries.
- [ ] Keep normal typing grouping behavior intact.

Validation gate:

- [ ] Unit tests match the one-edit, multi-edit, and branch reference scenarios.
- [ ] PTY tests cover visible text and mode-line status for the main path.

## Phase 5: Modified-State Tracking

Goal: Preserve clean/dirty behavior through undo and redo metadata changes.

Tasks:

- [ ] Revisit `clean_undo_depths` after sequence finalization.
- [ ] Update clean depths when metadata depth changes but clean text does not.
- [ ] Verify save, `write-file`, `not-modified`, revert, opened file, and scratch
  replacement behavior through undo and redo.

Validation gate:

- [ ] Existing undo-to-clean tests still pass.
- [ ] New tests cover redo away from and back toward clean text.

## Phase 6: Undo-Only And Undo-Redo Commands

Goal: Add explicit Emacs command variants.

Tasks:

- [ ] Add `undo-only` command.
- [ ] Add `undo-redo` command.
- [ ] Decide and implement key bindings or keep the commands `M-x` only.
- [ ] Document command descriptions in the command registry.

Validation gate:

- [ ] Unit and PTY tests match the `undo-only` and `undo-redo` reference
  scenario.

## Phase 7: Documentation

Goal: Replace current limitation notes with implemented behavior.

Tasks:

- [ ] Update `README.md` undo behavior.
- [ ] Update `docs/development.md` undo architecture notes.
- [ ] Update `docs/emacs-function-reference.md` if command coverage changes.
- [ ] Add `NEWS` release note.
- [ ] Add `ChangeLog` entries.

Validation gate:

- [ ] Docs no longer claim redo is missing after implementation lands.

## Phase 8: Full Verification

Goal: Prove the feature is complete and repository-clean.

Tasks:

- [ ] Run focused unit tests for undo/redo.
- [ ] Run focused PTY tests for undo/redo and modified status.
- [ ] Run the new reference captures when scenario behavior changes.
- [ ] Run `git diff --check`.
- [ ] Run `make verify`.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-02 | Plan created before Rile undo code changes. | User requested all edge cases documented before implementation. |
| 2026-07-02 | Added and captured first-pass Emacs undo/redo reference scenarios. | `make reference-capture REF_EDITOR=emacs REF_SCENARIO=undo-redo-single-core`, `undo-redo-multiple-core`, `undo-redo-branch-core`, and `undo-family-core` passed. |
| 2026-07-02 | Updated `undo-only` edge cases after visual capture review. | `undo-family-core` shows immediate `undo-only` continuing to an earlier edit and boundary `undo-only` reporting no further undo in the single-edit case. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-02 | Plan first, then reference scenarios, then implementation. | Avoid coding redo mechanics before edge cases are explicit. |
| 2026-07-02 | Target user-visible Emacs behavior, not internal undo-list compatibility. | Rile can stay smaller while matching terminal editing behavior. |
