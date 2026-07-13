<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Hi-Lock Face Completion

## Goal

Make the `Highlight using face:` prompt show a selectable face list like Rile's
existing `M-x`, buffer, file, and variable completion prompts.

The first pass should make Rile's supported hi-lock palette discoverable and
keyboard-selectable without claiming support for arbitrary Emacs face objects.

## Scope

- Add completion candidates for the supported hi-lock face palette.
- Start a completion session when the `Highlight using face:` prompt opens.
- Reuse the existing completion UI and navigation behavior rather than building a
  custom face picker.
- Show short annotations so users can distinguish face choices.
- Keep existing manual face-name entry working.
- Update tests and user-facing docs for the completion behavior.

## Non-Goals

- Do not implement arbitrary Emacs face names such as
  `font-lock-comment-delimiter-face` unless they are explicitly mapped to Rile
  render faces.
- Do not implement full Emacs face inheritance, attributes, or theme integration.
- Do not require color preview columns in the first pass; terminal ANSI previews
  can be a later improvement.
- Do not change the hi-lock highlighting storage, regexp matching, or
  unhighlight behavior.

## Current Context

- `PromptKind::HighlightFace` exists and uses prompt history.
- `src/editor.rs` currently pre-fills the face prompt with the next default face
  name and accepts manual input through `highlight_face_by_name`.
- Supported face names currently live in `HIGHLIGHT_FACE_SPECS` in
  `src/editor.rs`.
- `CompletionSession` currently supports `Commands`, `Files`, `Buffers`, and
  `Options` sources.
- `Editor::start_extended_command`, `Editor::start_describe_function`,
  `Editor::start_describe_variable`, file prompts, and buffer prompts already
  demonstrate how to attach completion to a minibuffer prompt.
- The current completion UI can show candidate values, annotations, and selected
  rows, which is close enough to Emacs' face-name list for a first pass.

## Desired Behavior

- After entering a regexp or phrase for a hi-lock command, Rile prompts:
  `Highlight using face (default hi-yellow):`.
- The completion area/list opens for the face prompt using Rile's configured
  completion style.
- Candidate values include at least the supported palette:
  `hi-yellow`, `hi-pink`, `hi-green`, `hi-blue`, `hi-salmon`,
  `hi-aquamarine`, `hi-black-b`, `hi-blue-b`, `hi-red-b`, `hi-green-b`, and
  `hi-black-hb`.
- Each candidate has a short annotation such as `Hi-lock yellow highlight` or
  `Hi-lock bold red highlight`.
- Pressing Enter with the pre-filled default still accepts the default face.
- Typing or selecting another supported face applies that face.
- Unknown face names still fail with `Error: unknown highlight face`.

## Design

### Completion Source

- Add `CompletionSource::Faces` or a narrower `CompletionSource::HighlightFaces`.
- Add a constructor such as `CompletionSession::highlight_faces(...)` that accepts
  face candidate metadata or returns candidates from a small built-in list.
- Prefer keeping candidate definitions close to the existing hi-lock face specs so
  face names, annotations, and render mappings do not drift.

### Face Metadata

- Extend the hi-lock face spec data with an annotation string.
- Keep the render `Face` mapping in the same data structure or expose a shared
  accessor from `src/editor.rs` if moving the specs is too invasive.
- If moving the specs out of `src/editor.rs`, choose a small internal module such
  as `src/highlight_face.rs` only if it avoids circular dependencies and keeps the
  edit smaller than duplicating names.

### Prompt Wiring

- When `start_user_highlight_face_prompt` starts `PromptKind::HighlightFace`, set
  `self.completion = Some(CompletionSession::highlight_faces(...))` and call
  `update_completion_from_prompt()`.
- Ensure accepting a completion flows through the existing
  `handle_completion_prompt_key` path and then `submit_highlight_face`.
- Ensure cancelling the face prompt clears both `pending_user_highlight` and the
  active completion session using existing cancellation paths.

### Completion Policy

- Update `completion_policy.rs` so `HighlightFaces` behaves like command, option,
  and buffer completions for acceptance.
- Exact typed face names should be accepted even if no selection was explicitly
  made.
- Tab/common-prefix behavior should work like other non-file completions.

## Tasks

### Phase 1: Completion Model

- [x] Add a face-oriented completion source to `CompletionSource`.
- [x] Add a `CompletionSession` constructor for hi-lock face candidates.
- [x] Add tests for matching, annotations, selection, and accepted input for the
  new source.

### Phase 2: Hi-Lock Prompt Wiring

- [x] Extend hi-lock face specs with annotations or provide a shared candidate
  list derived from the specs.
- [x] Start a face completion session from `start_user_highlight_face_prompt`.
- [x] Confirm prompt cancellation clears pending highlight state and completion
  state.
- [x] Add editor unit tests for default face acceptance, selecting a non-default
  face through completion, and unknown-face rejection.

### Phase 3: PTY And Docs

- [x] Add PTY coverage that opens a hi-lock command and sees the face candidate
  list.
- [x] Add PTY coverage for selecting a non-default face from the list.
- [x] Update `README.md`, `NEWS`, `ChangeLog`, `docs/development.md`, and
  `docs/emacs-function-reference.md`.
- [x] Update this plan's Progress Log and Decision Log as work completes.

## Validation

- [x] Run `./scripts/in-container cargo test --locked --lib completion`.
- [x] Run `./scripts/in-container cargo test --locked --lib highlight`.
- [x] Run `./scripts/in-container cargo test --locked --test pty_search hi_lock`.
- [x] Run `make verify` before considering the work complete.

## Risks

- Completion acceptance policy is shared by several prompt types; changes must not
  regress `M-x`, buffer, file, or option completion.
- The existing completion UI may not show colored previews. That is acceptable for
  the first pass, but candidate names and annotations must be clear.
- Keeping face names duplicated between render mapping, prompt validation, and
  completion candidates would create drift; prefer one source of truth.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-13 | Plan created. | User requested a plan for a selectable `Highlight using face:` list similar to `M-x` and other completion prompts. |
| 2026-07-13 | Implemented hi-lock face completion. | Commit `536a9d2`; targeted `completion`, `highlight`, and `pty_search hi_lock` tests passed. |
| 2026-07-13 | Preselected the displayed default face and completed full verification. | Added coverage for rotated defaults; `./scripts/in-container cargo test --locked --lib completion`, `./scripts/in-container cargo test --locked --lib highlight_regexp_face`, `./scripts/in-container cargo test --locked --test pty_search hi_lock`, and `make verify` passed with 838 Rust tests plus snapshots. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-13 | Use Rile's existing completion UI for hi-lock face selection. | It already supports selectable lists, annotations, keyboard navigation, and prompt acceptance. |
| 2026-07-13 | Limit first-pass candidates to Rile's supported hi-lock palette. | Rile does not implement arbitrary Emacs face definitions yet, so listing unsupported faces would be misleading. |
