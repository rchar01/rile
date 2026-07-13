<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Hi-Lock Unhighlight And Face Selection

## Goal

Refine Rile's hi-lock implementation so `unhighlight-regexp` behaves closer to
GNU Emacs and user highlights remain readable across common terminal themes.

Add a practical first pass at face selection for `highlight-regexp`,
`highlight-phrase`, and `highlight-lines-matching-regexp` without introducing a
full Emacs face system.

## Scope

- Make `unhighlight-regexp` offer an editable default highlight pattern.
- Add universal-argument support so `C-u M-s h u` removes all current-buffer
  hi-lock highlights.
- Fix default-theme line highlight contrast by setting an explicit foreground.
- Add named hi-lock face choices backed by Rile's existing render faces or a
  small extension of them.
- Update docs, release notes, ChangeLog, and tests for user-visible behavior.

## Non-Goals

- Do not implement file-local `Hi-lock:` persistence.
- Do not implement arbitrary Emacs face definitions or all Emacs face
  attributes.
- Do not implement subexpression-only highlighting for `highlight-regexp`.
- Do not change the line-local regexp matching model.

## Verified Emacs Behavior

- GNU Emacs 30.2 `unhighlight-regexp` removes all highlights when called with a
  universal argument, passing `REGEXP = t` internally.
- Plain interactive `unhighlight-regexp` prompts for a previously inserted
  hi-lock regexp. The default is inferred from point where possible, otherwise
  from stored hi-lock patterns.
- Pressing `RET` at the prompt accepts that default pattern; empty input does
  not mean remove all.
- GNU Emacs 30.2 `highlight-regexp` prompts for a face by default in `emacs -Q`
  because `hi-lock-auto-select-face` is nil.
- GNU Emacs default face candidates include `hi-yellow`, `hi-pink`, `hi-green`,
  `hi-blue`, `hi-salmon`, `hi-aquamarine`, `hi-black-b`, `hi-blue-b`,
  `hi-red-b`, `hi-green-b`, and `hi-black-hb`.

## Rile Context At Plan Creation

- `src/editor.rs` stores user highlights per `BufferId` in `user_highlights`.
- `src/editor.rs` rejected empty `unhighlight-regexp` input as
  `Error: empty highlight regexp`.
- `command_unhighlight_regexp` ignored `CommandContext::argument`.
- `src/minibuffer.rs` supports starting prompts and pre-filling prompt input.
- `src/terminal/mod.rs` mapped `Face::UserHighlightLine` to `\x1b[43m`, which
  does not force a contrasting foreground.
- `src/completion.rs` has command, file, buffer, and option completion sources;
  face-name completion would require either a small new source or a simpler
  no-completion prompt.

## Design

### Unhighlight Defaults

- When `M-s h u` starts, pre-fill the prompt with a default active highlight
  pattern instead of leaving it blank.
- Prefer the highlight pattern at point when cheap to compute from existing
  line-local matching data.
- If no highlight at point is found, use the most recently added current-buffer
  highlight pattern.
- Let the user edit the pre-filled text before pressing `RET`.
- If the prompt is cleared and submitted blank, accept the stored default pattern
  rather than treating blank input as all-removal.
- On `RET`, remove all current-buffer entries whose original prompt text matches
  the submitted input, preserving the current exact-input removal behavior.
- If there are no active highlights, show `No highlighting to remove` and do not
  start a prompt.

### Remove All

- When `unhighlight-regexp` is invoked with a universal argument, remove every
  current-buffer hi-lock highlight immediately.
- Use singular/plural messages matching existing wording:
  `Removed 1 highlight`, `Removed N highlights`, or `No highlighting to remove`.
- Do not treat blank prompt input as remove-all; reserve all-removal for the
  universal-argument path to match Emacs.

### Face Selection

- Add a face prompt after the regexp or phrase prompt:
  `Highlight using face: `.
- Pre-fill the face prompt with the next default face name.
- Accept a small Emacs-named palette first:
  `hi-yellow`, `hi-pink`, `hi-green`, `hi-blue`, `hi-salmon`,
  `hi-aquamarine`, `hi-black-b`, `hi-blue-b`, `hi-red-b`, `hi-green-b`, and
  `hi-black-hb`.
- Map supported face names to Rile `Face` variants and ANSI-safe terminal
  styles. Add new `Face` variants only if the existing three user-highlight
  faces are insufficient for readable distinct choices.
- Treat empty face input as accepting the pre-filled default.
- Reject unknown face names with `Error: unknown highlight face`.
- Keep face selection buffer-local and ephemeral with the highlight entry.

### Contrast Fix

- Change default-theme `UserHighlightLine` rendering from yellow-background-only
  to yellow background with black foreground.
- Ensure any newly added hi-lock faces set both foreground and background when a
  background color is used.

## Tasks

### Phase 1: Unhighlight Behavior

- [x] Add a helper to choose the default current-buffer highlight pattern.
- [x] Pre-fill `Unhighlight regexp: ` with the chosen default pattern.
- [x] Show `No highlighting to remove` when there are no active highlights.
- [x] Add universal-argument all-removal to `command_unhighlight_regexp`.
- [x] Add unit tests for default selection, editable deletion, no-highlight
  behavior, and all-removal.
- [x] Add a regression test for clearing the pre-filled default and submitting
  blank input.
- [x] Add or update PTY coverage for `M-s h u RET` removing the pre-filled
  default and `C-u M-s h u` removing all highlights.

### Phase 2: Highlight Face Selection

- [x] Add state for pending highlight regexp/phrase/line input while asking for
  a face.
- [x] Add `PromptKind` support for the highlight face prompt and prompt history
  if appropriate.
- [x] Implement the supported hi-lock face-name palette and mapping to render
  faces.
- [x] Add the face prompt to `highlight-regexp`, `highlight-phrase`, and
  `highlight-lines-matching-regexp`.
- [x] Add tests for accepting the default face, choosing a non-default face, and
  rejecting an unknown face.

### Phase 3: Rendering And Documentation

- [x] Fix `Face::UserHighlightLine` default-theme ANSI contrast.
- [x] Add or extend terminal rendering tests for all supported hi-lock face
  mappings.
- [x] Update `README.md`, `NEWS`, `ChangeLog`, `docs/development.md`, and
  `docs/emacs-function-reference.md`.
- [x] Update this plan's Progress Log and Decision Log as implementation
  decisions are finalized.

## Validation

- [x] Run `./scripts/in-container cargo test --locked --lib`.
- [x] Run `./scripts/in-container cargo test --locked --test pty_search hi_lock`.
- [x] Run `make verify` before considering the work complete.

## Risks

- Adding a second prompt changes the current quick highlight flow and needs
  careful prompt-cancellation handling.
- Face-name completion may require extending the completion source model; the
  first implementation can ship without completion if that keeps the change
  smaller and safer.
- Some terminal color palettes vary; tests should assert emitted ANSI codes, not
  perceived color quality.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-13 | Plan created. | User requested plan under `docs/plans/`; Emacs 30.2 behavior verified with local `emacs -Q --batch` docstrings/source metadata. |
| 2026-07-13 | Implemented unhighlight defaults, face selection, and contrast fixes. | Commits `15dc18b` and `1e6799a`; `./scripts/in-container cargo test --locked --lib`, `./scripts/in-container cargo test --locked --lib highlight`, `./scripts/in-container cargo test --locked --lib unhighlight`, and `./scripts/in-container cargo test --locked --test pty_search hi_lock` passed. |
| 2026-07-13 | Completed full verification after fixing a Clippy lint. | `make verify` passed with 830/830 Rust tests and 4/4 snapshots after removing a needless borrow in `tests/pty_search.rs`. |
| 2026-07-13 | Fixed blank unhighlight submission semantics and reverified. | Added unit and PTY coverage so clearing the pre-filled prompt and pressing Enter accepts the stored default pattern instead of erroring or removing all highlights. `./scripts/in-container cargo test --locked --lib unhighlight`, `./scripts/in-container cargo test --locked --test pty_search hi_lock`, and final `make verify` passed with 831 Rust tests and 4/4 snapshots. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-13 | Empty `unhighlight-regexp` input should not remove all. | GNU Emacs uses universal argument for all-removal; plain prompt accepts an editable default pattern. |
| 2026-07-13 | Use a pre-filled default pattern for `unhighlight-regexp`. | This gives Emacs-like default deletion while still letting the user edit the regexp before submission. |
| 2026-07-13 | Blank `unhighlight-regexp` submission accepts the stored default. | This preserves Emacs-style default acceptance while avoiding a blank-input all-removal shortcut. |
| 2026-07-13 | Add face selection as a small named palette. | It matches Emacs' prompt shape without requiring a full face system. |
