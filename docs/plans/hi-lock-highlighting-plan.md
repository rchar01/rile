<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Hi-Lock Style Highlighting

## Goal

Add basic persistent, buffer-local highlighting commands in the Emacs hi-lock
family: `highlight-regexp`, `highlight-phrase`,
`highlight-lines-matching-regexp`, and `unhighlight-regexp`.

The first implementation should reuse Rile's existing line-local regexp and
decoration infrastructure. It should be useful in normal editing without adding
a full overlay system, face-name completion, or file-local hi-lock persistence.

## Scope

- Add interactive commands for regexp, phrase, whole-line, and removal
  highlighting.
- Add Emacs-compatible key bindings under `M-s h` where possible.
- Store user highlights per buffer and render them through `render::Span` and
  `DecorationProvider`.
- Use Rile's existing line-local `SearchPattern` matching and smart-case rules.
- Add a small set of user-highlight faces and terminal ANSI mappings.
- Document the behavior in user docs, development notes, release notes, and
  `ChangeLog`.

## Non-Goals

- Do not implement Emacs overlays as a general editing primitive.
- Do not support persisted `Hi-lock:` file comments in the first pass.
- Do not prompt for arbitrary Emacs face names in the first pass.
- Do not support multiline regexp highlighting before Rile has multiline regexp
  matching.
- Do not import, translate, or mechanically port Emacs hi-lock implementation
  code.

## Cached Emacs 30.2 Behavior

Captured locally with `emacs --batch --quick` on 2026-07-13 using GNU Emacs
30.2 and `(require 'hi-lock)`. This is behavior evidence only.

Command bindings:

| Command | Default key | Purpose |
| --- | --- | --- |
| `highlight-regexp` | `M-s h r` | Highlight regexp matches in the current buffer. |
| `highlight-phrase` | `M-s h p` | Highlight phrase/regexp matches with whitespace folding. |
| `highlight-lines-matching-regexp` | `M-s h l` | Highlight entire lines containing regexp matches. |
| `unhighlight-regexp` | `M-s h u` | Remove one hi-lock pattern, or all with universal argument. |

Observed command behavior:

| Command | Input | Observed stored behavior | Observed highlight |
| --- | --- | --- | --- |
| `highlight-phrase` | `foo bar` | Stores flexible whitespace equivalent to `[ \t]+`; matching is case-folded for lowercase input. | `foo bar` and `Foo   bar` both highlighted. |
| `highlight-regexp` | `TODO` | Stores raw regexp text with smart-case behavior. | Only `TODO` highlighted. |
| `highlight-lines-matching-regexp` | `plain` | Stores a full-line pattern around the regexp. | The whole `plain\n` line highlighted. |

Representative overlay evidence from the capture:

```text
overlay:"foo bar":1-8:hi-yellow
overlay:"Foo   bar":9-18:hi-yellow
overlay:"TODO":25-29:hi-pink
overlay:"plain\n":35-41:hi-green
```

Docstring-derived behavior to preserve where feasible:

- `highlight-regexp` and `highlight-lines-matching-regexp` prompt for a regexp
  and a face in Emacs; Rile's first pass should use automatic built-in faces
  instead of a face prompt.
- `highlight-regexp` can restrict highlighting to a subexpression with a prefix
  argument in Emacs; Rile should defer this until prefix arguments and capture
  highlighting need it.
- `unhighlight-regexp` interactively offers previously inserted hi-lock regexps;
  Rile's first pass can prompt for text and remove matching stored entries.
- Emacs highlights via font lock when available and overlays otherwise; Rile
  should use its existing decoration-provider rendering path.

## Current Rile Context

- `src/render/mod.rs` defines `Face`, `Span`, `DecorationProvider`, and priority
  merging.
- `src/editor.rs` already composes syntax, region, query-replace, and active
  search decorators in `spans_for_buffer_line`.
- `SearchDecorator` in `src/editor.rs` already maps `SearchPattern` matches to
  spans and can serve as the model for persistent user highlights.
- `src/search_pattern.rs` provides line-local literal and regexp matching with
  smart case.
- `src/minibuffer.rs` and `src/editor/prompt_history.rs` provide prompt kinds and
  history plumbing for command prompts.
- `M-s h` is currently unused in Rile's default keymap.

## Design

Represent each user highlight as a buffer-local entry:

```text
original_input: String
pattern: SearchPattern
kind: Match | Line
face: Face
```

Rendering model:

- Add a `UserHighlightDecorator` to the provider list in `spans_for_buffer_line`.
- Match highlights produce spans for every non-empty match in the line.
- Line highlights produce one span from byte `0` to `line.len()` when the pattern
  matches anywhere on the line.
- User highlights should have higher priority than syntax highlighting and lower
  priority than region, active search, and query-replace.
- Highlights should apply to non-current visible windows for the same buffer.

Face model:

- Add `Face::UserHighlight`, `Face::UserHighlightAlt`, and
  `Face::UserHighlightLine`.
- Cycle match highlights between `UserHighlight` and `UserHighlightAlt`.
- Use `UserHighlightLine` for line highlights.
- Map default-theme faces to distinct ANSI emphasis; map mono-theme faces to
  conservative underline/reverse/dim combinations.

Prompt model:

- Add prompt kinds for highlight regexp, highlight phrase, highlight lines, and
  unhighlight regexp.
- Add prompt labels:
  - `Highlight regexp: `
  - `Highlight phrase: `
  - `Highlight lines matching regexp: `
  - `Unhighlight regexp: `
- Add prompt history for these prompt kinds.
- On invalid regexp input, report `Error: invalid regexp` and do not add an
  entry.
- On empty input, report a normal command error and do not add an entry.

Command model:

- Register `highlight-regexp`, `highlight-phrase`,
  `highlight-lines-matching-regexp`, and `unhighlight-regexp` in
  `src/command.rs`.
- Add default key bindings:
  - `M-s h r` -> `highlight-regexp`
  - `M-s h p` -> `highlight-phrase`
  - `M-s h l` -> `highlight-lines-matching-regexp`
  - `M-s h u` -> `unhighlight-regexp`
- Consider adding `hi-lock-mode` only as a documentation-only future command, not
  as part of the first implementation.

Phrase transformation:

- Compile phrases as regexps after replacing runs of literal ASCII whitespace
  with `[ \t]+`.
- Keep other regexp syntax as typed, matching Emacs' documented "PHRASE can be
  any REGEXP" behavior within Rile's supported regexp subset.
- Preserve smart-case matching through `SearchPattern::compile`.

Removal behavior:

- First pass: remove stored entries whose `original_input` exactly matches the
  submitted text.
- If no matching entry exists, report `No highlight for <input>`.
- Defer completion over active patterns and universal-argument removal of all
  highlights to a later pass unless implementation discovers this is cheap.

## Open Questions

- [x] Should `unhighlight-regexp` remove all matching entries across match and
  line-highlight kinds, or only entries created by `highlight-regexp` and
  `highlight-phrase`? Decision: remove all entries whose original prompt text
  matches, regardless of highlight kind.
- [x] Should the first pass include an `unhighlight-all` command or universal
  argument support, or defer all-removal until prefix arguments are broader?
  Decision: defer all-removal.
- [x] Should user highlights be saved in sessions/config later, or remain purely
  ephemeral like the first implementation proposes? Decision: keep them
  ephemeral for the first pass.

## Phase 1: Reference And Architecture Plan

Goal: Lock down the intended subset before implementation.

Tasks:

- [x] Capture GNU Emacs 30.2 command bindings and representative behavior.
- [x] Inspect Rile render/decorator/search/prompt infrastructure.
- [x] Add the hi-lock command entries to `docs/emacs-function-reference.md` as
  durable behavior notes.

Validation gate:

- [x] Behavior notes clearly separate Emacs behavior from Rile first-pass scope.

## Phase 2: Core Data Model And Rendering

Goal: Add buffer-local persistent highlights without changing command behavior
yet.

Tasks:

- [x] Add user-highlight storage to editor/buffer state without persisting it to
  disk.
- [x] Add `Face` variants and terminal ANSI mappings for user highlights.
- [x] Add `UserHighlightDecorator` and wire it into `spans_for_buffer_line`.
- [x] Ensure user-highlight priority sits above syntax and below region/search.

Validation gate:

- [x] Unit tests cover span production, line highlighting, and priority merging.
- [x] Terminal render unit tests cover new faces.

## Phase 3: Commands And Prompts

Goal: Expose the feature through Emacs-style commands and key bindings.

Depends on: Phase 2.

Tasks:

- [x] Add command registry entries and handlers for the four hi-lock commands.
- [x] Add prompt kinds, prompt labels, submission handlers, and prompt history.
- [x] Add default key bindings under `M-s h`.
- [x] Implement regexp highlight creation using `SearchPattern::compile`.
- [x] Implement phrase transformation and highlight creation.
- [x] Implement whole-line highlight creation.
- [x] Implement exact-input unhighlight removal.

Validation gate:

- [x] Unit tests cover command submission, invalid input, empty input, history,
  and unhighlight removal.
- [x] Keymap and command registry tests cover the new commands and bindings.

## Phase 4: Integration Tests And Docs

Goal: Verify end-to-end command flow and document the user-visible subset.

Depends on: Phase 3.

Tasks:

- [x] Add PTY tests for `M-s h r`, `M-s h p`, `M-s h l`, and `M-s h u` prompt
  flows. Rendering effects are covered by span and ANSI unit tests because PTY
  tests parse normalized screen text, not raw color attributes.
- [x] Update `README.md` with commands, key bindings, and current limits.
- [x] Update `NEWS` with the user-visible feature entry.
- [x] Update `ChangeLog` with GNU-style source-history entries.
- [x] Update `docs/development.md` with implementation notes and limitations.

Validation gate:

- [x] Focused unit tests pass.
- [x] Focused PTY tests pass.
- [x] `git diff --check` passes.
- [x] `make verify` passes before final completion.

## Risks

- Whole-line spans over empty lines need careful handling because empty spans are
  ignored by the renderer.
- Rile's regexp subset is line-local and smaller than Emacs; docs must avoid
  implying full Emacs regexp support.
- Prompting for faces too early would require face-name completion and more UI
  design than the core feature needs.
- Overlap semantics must preserve active search and region visibility.
- PTY tests parse visible screen state and normally set `NO_COLOR=1`; color
  correctness should remain unit-tested through spans and ANSI rendering.

## Validation Summary

- [x] Behavior cache reviewed against GNU Emacs 30.2 evidence.
- [x] Core rendering/unit tests pass.
- [x] Command/prompt/keymap tests pass.
- [x] PTY command-flow tests pass.
- [x] Documentation updated.
- [x] `make verify` passes.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-07-13 | Plan created with cached GNU Emacs 30.2 hi-lock behavior. | Local `emacs --batch --quick` checks; `src/render/mod.rs`, `src/editor.rs`, `src/minibuffer.rs`, and `src/keymap.rs` inspected. |
| 2026-07-13 | Implemented buffer-local hi-lock style highlights and PTY coverage. | Commits `79d6fc4` and `dcd1d3d`; `./scripts/in-container cargo test --locked --lib` and focused `pty_search hi_lock_highlight_commands_use_emacs_style_keys` passed. |
| 2026-07-13 | Updated user docs, reference notes, release notes, ChangeLog, and plan progress. | `README.md`, `NEWS`, `ChangeLog`, `docs/development.md`, `docs/emacs-function-reference.md`, and this plan. |
| 2026-07-13 | Completed full verification and reviewed phrase metacharacter behavior. | `make verify` passed; patch review flagged phrase regexp syntax, but GNU Emacs 30.2 `highlight-phrase "a.b"` highlights both `a.b` and `axb`, so Rile preserves regexp metacharacters and adds a unit test for that behavior. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-13 | First pass uses Rile decorations instead of a general overlay subsystem. | Existing span rendering already supports overlapping highlights and terminal output. |
| 2026-07-13 | First pass uses automatic built-in highlight faces instead of prompting for face names. | This delivers the core feature without adding face-name completion or arbitrary face parsing. |
| 2026-07-13 | `unhighlight-regexp` removes every current-buffer entry whose original prompt text matches. | This keeps removal simple while covering regexp, phrase, and line highlights consistently. |
| 2026-07-13 | Preserve regexp metacharacters in `highlight-phrase` input. | GNU Emacs documents that a phrase can be any regexp, and local Emacs 30.2 behavior confirms `a.b` matches `axb`. |
