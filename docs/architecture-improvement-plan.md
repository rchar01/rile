<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Architecture Roadmap To Rile 1.0

## Goal

Improve Rile's maintainability and testability on the road to the first stable
release without rewriting working editor behavior. The plan deliberately
separates low-risk Phase 1 cleanup from deeper Phase 2 architecture work so the
editor stays working throughout the refactor.

## Scope

- Document the safe Phase 1 cleanup work that should happen before larger design
  changes.
- Document Phase 2 candidates that may become worthwhile after Phase 1 tests and
  cleanup expose clearer seams.
- Preserve current user-visible behavior unless a later change explicitly says
  otherwise.
- Prefer concrete Rust modules and data types over new trait hierarchies.

## Non-Goals

- Do not split the project into multiple crates yet.
- Do not replace the direct termios and ANSI terminal backend yet.
- Do not introduce async, a plugin system, or broad filesystem/shell traits.
- Do not replace the current `Vec<String>` buffer storage without benchmark
  evidence and a concrete large-file goal.
- Do not change command names, key bindings, config keys, or documented editor
  behavior as part of cleanup-only work.

## Current Context

- Rile is a single Rust crate for a terminal-native Emacs-style editor.
- `src/editor.rs` is the main architectural hotspot. It owns editor state,
  command dispatch, prompts, completion policy, help formatting, special-buffer
  workflows, search, query replace, registers, rectangles, keyboard macros,
  undo coordination, and viewport synchronization.
- Lower-level modules such as `src/buffer/`, `src/file.rs`, `src/window.rs`,
  `src/render/`, `src/input.rs`, `src/syntax.rs`, `src/config.rs`, and metadata
  registries are already comparatively concrete and testable.
- `src/command.rs` combines command metadata with concrete `Editor` handler
  function pointers. This coupling is not ideal, but it is simple and should not
  be changed before lower-risk cleanup.
- `src/terminal/mod.rs` draws frames and currently mutates editor viewport state
  to keep point visible. That is a design smell, but it is also tied to subtle
  cursor and scrolling behavior.
- The canonical verification gate is `make verify`. Focused tests should use the
  existing unit tests and PTY tests before the full gate.

## Assumptions

- The Rile 1.0 goal remains a dependable lightweight terminal editor for source
  files, config files, Markdown, and UTF-8 text.
- Terminal UI remains the only concrete frontend target for now.
- Keeping current behavior stable is more important than reducing line count
  quickly.
- Additional abstractions should be introduced only when they simplify real code
  already under change.

## Open Questions

- [ ] Which feature area is most likely after cleanup: redo, larger-file
  performance, more Emacs compatibility, syntax/mode expansion, shell/process
  behavior, or configuration growth?
- [ ] Are command names and key bindings considered stable enough to treat as a
  user-facing compatibility surface?
- [ ] Is alternate frontend support a real future goal, or should terminal-only
  design stay explicit through Phase 2?

## Phase 1: Stabilize And Make Refactors Safe

Phase 1 should reduce risk and improve local reasoning while keeping `Editor` as
the application coordinator. It should avoid major ownership changes, command
system redesign, and new traits.

### Step 1.1 Add Characterization Coverage

Goal: protect behavior before moving code.

Tasks:

- [x] Add or confirm tests for switching buffers in split windows while
  preserving point and selected-window state.
- [x] Add or confirm tests for help, messages, shell-output, and completion
  buffer return behavior.
- [x] Add or confirm tests for horizontal scroll state across window and buffer
  switches.
- [x] Add or confirm tests for killing a buffer that is shown in one or more
  windows.

Likely files:

- `src/editor.rs`
- `tests/pty_split_pane.rs`
- `tests/pty_completion.rs`
- `tests/pty_scrolling.rs`
- `tests/pty_movement.rs`

Risk: low.

Validation gate:

- [x] Run focused editor tests with `./scripts/in-container cargo test --locked editor`.
- [x] Run relevant PTY tests with `./scripts/in-container cargo test --locked --test <target>`.

Next slice:

- Start Step 1.2 by identifying pure help/about/describe formatting helpers that
  can move without widening editor internals.

### Step 1.2 Extract Pure Help Formatting

Goal: reduce `src/editor.rs` size without changing mutable editor behavior.

Tasks:

- [x] Move help/about/describe formatting helpers into a small helper module only
  if the move does not require broad visibility changes.
- [x] Keep `Editor` responsible for opening help buffers and collecting current
  editor state.
- [x] Preserve command, key, option, mode, buffer, messages, and about output.

Likely files:

- `src/editor.rs`
- Possible new `src/editor/help.rs` or equivalent helper module
- `docs/self-documentation.md` only if the source-of-truth description changes

Risk: low.

Validation gate:

- [x] Run focused help and describe tests.
- [x] Run representative PTY help tests.

### Step 1.3 Extract Pure Search Helpers

Goal: isolate exact line-local search logic from editor workflow state.

Tasks:

- [x] Move pure search match helpers out of the main editor body.
- [x] Keep incremental-search prompt state and wrap/failure behavior in `Editor`
  until tests prove a cleaner state-machine seam.
- [x] Preserve exact UTF-8 substring matching and current wrapping semantics.

Likely files:

- `src/editor.rs`
- Possible new `src/editor/search.rs`
- `tests/pty_search.rs`

Risk: low to medium.

Validation gate:

- [x] Run search unit tests.
- [x] Run `./scripts/in-container cargo test --locked --test pty_search`.

### Step 1.4 Extract Prompt History If Clean

Goal: isolate prompt history behavior without moving prompt submission logic.

Tasks:

- [x] Move prompt history storage/navigation into a small concrete helper if it
  can be tested without exposing large editor internals.
- [x] Preserve draft restoration, duplicate suppression, per-prompt-kind history,
  and completion refresh after history recall.
- [x] Leave prompt submission and command-specific prompt behavior in `Editor`.

Likely files:

- `src/editor.rs`
- Possible new `src/editor/prompt_history.rs`
- `tests/pty_completion.rs`

Risk: medium.

Validation gate:

- [x] Run prompt-history and completion unit tests.
- [x] Run relevant completion PTY tests.

### Step 1.5 Reassess Before Larger Refactors

Goal: avoid continuing into architecture churn after small wins.

Tasks:

- [x] Review whether Phase 1 extractions reduced review pain without widening
  visibility too much.
- [x] Record any remaining repeated patterns with file references.
- [x] Decide whether Phase 2 work is justified by concrete pain or upcoming
  features.

Risk: low.

Validation gate:

- [x] Run `make verify` before treating Phase 1 cleanup as complete.
- [x] Update this plan's Progress Log with completed changes and evidence.

Reassessment:

- Phase 1 reduced `Editor` review scope without exposing editor internals outside
  the `editor` module tree. The extracted help, search, and prompt-history seams
  use `pub(super)` helpers rather than `pub(crate)` state access.
- Remaining repeated code is concentrated in returnable special-buffer opening
  and return paths: `open_help_buffer`, `open_messages_buffer`,
  `open_shell_output_buffer`, `update_completion_buffer`, and their
  restore/finish partners in `src/editor.rs`.
- Buffer-list handling is a related but distinct special-buffer workflow:
  `list_buffers` and `refresh_visible_buffer_list` share opening/refresh
  concerns, but not the same return-viewport mechanics.
- Step 2.1 is justified as the next small candidate because the returnable
  special-buffer paths repeat cursor, search, region, insert-group, viewport,
  and return-viewport handling. Buffer-list cleanup should be included only if
  it stays clear rather than forcing one helper over unlike workflows.
  Broader command dispatch, terminal, or buffer-storage refactors are not yet
  justified by the Phase 1 evidence.

## Phase 2: Architecture Readiness For Rile 1.0

Phase 2 work should start only after Phase 1 cleanup and tests are in place.
Each item is a candidate, not a commitment. Prefer the smallest candidate that
solves a real maintenance or release-readiness problem.

### Step 2.1 Simplify Special-Buffer Helpers

Goal: reduce repeated help/messages/completions/shell-output buffer opening and
return logic, then consider buffer-list refresh only if it fits without hiding
distinct behavior.

Tasks:

- [x] Introduce one small helper for returnable special buffers if repeated
  cursor, search, region, insert-group, viewport, and return-viewport handling
  is still visible after Phase 1.
- [x] Treat buffer-list opening and refresh as related but distinct unless the
  helper remains obvious and behavior-preserving.
- [x] Keep `DocumentKind` concrete.
- [x] Avoid a dynamic special-buffer registry unless new special-buffer types
  become frequent.

Risk: low to medium.

Validation gate:

- [x] Run help, messages, completion-buffer, and shell-output tests.
- [x] Run buffer-list tests too if buffer-list behavior is touched; not required
  for this cleanup because buffer-list behavior was left unchanged.

### Step 2.2 Extract Completion Prompt Policy

Goal: make completion prompt behavior easier to test without moving matching
logic.

Tasks:

- [ ] Keep matching and candidate ranking in `src/completion.rs`.
- [ ] Move Enter, `M-RET`, Tab, directory descent, exact-file acceptance, and
  default-buffer acceptance policy into a concrete editor-side helper.
- [ ] Avoid traits or generic prompt sources unless the helper remains too
  coupled after extraction.

Risk: medium.

Validation gate:

- [ ] Run completion unit tests.
- [ ] Run `./scripts/in-container cargo test --locked --test pty_completion`.

### Step 2.3 Separate Command Metadata From Dispatch

Goal: make `src/command.rs` a cleaner source of command metadata only if command
growth makes the current concrete handler table painful.

Tasks:

- [ ] Keep `Command`, `CommandSpec`, command docs, categories, and completion
  metadata in `src/command.rs`.
- [ ] Move the concrete command-to-handler mapping into an editor dispatch module
  only if this reduces coupling without adding trait objects or dynamic dispatch.
- [ ] Preserve registry validation that every interactive command has metadata
  and a handler.

Risk: medium.

Validation gate:

- [ ] Run command registry tests.
- [ ] Run keymap tests.
- [ ] Run representative `M-x`, key binding, and help PTY tests.

### Step 2.4 Clean Up View-State Ownership

Goal: reduce duplicated selected buffer, cursor, and viewport state after tests
cover the current behavior.

Tasks:

- [ ] Identify the canonical source of truth for selected window buffer and
  cursor state.
- [ ] Preserve per-buffer point restoration and per-window viewport behavior.
- [ ] Remove synchronization helpers only after replacement invariants are
  tested.

Likely risk areas:

- Split panes
- Buffer switching
- Help and completion return
- Horizontal scrolling
- Killing buffers shown in windows

Risk: medium to high.

Validation gate:

- [ ] Run movement, scrolling, split-pane, completion, and buffer-related PTY tests.
- [ ] Run parsed-screen snapshots with `make snapshot-test`.

### Step 2.5 Introduce A Render Snapshot Boundary If Needed

Goal: make terminal rendering consume a prepared view model instead of mutating
editor state while drawing.

Tasks:

- [ ] Move cursor-visibility adjustment before frame rendering.
- [ ] Introduce a small `RenderSnapshot` or equivalent only if it simplifies
  tests or supports a concrete future UI boundary.
- [ ] Keep ANSI output in the terminal adapter.

Risk: medium.

Validation gate:

- [ ] Run terminal unit tests.
- [ ] Run PTY rendering tests.
- [ ] Run `make snapshot-test`.

### Step 2.6 Improve Error Modeling Only Where Callers Branch

Goal: avoid stringly error handling in areas that need distinct recovery paths.

Tasks:

- [ ] Add specific error variants only when callers need to branch on the error.
- [ ] Preserve user-facing messages unless intentionally changed.
- [ ] Avoid converting every string error into an enum variant by default.

Risk: low.

Validation gate:

- [ ] Run unit tests for affected modules.
- [ ] Run focused PTY tests for user-visible error messages if output changes.

### Step 2.7 Evaluate Buffer Storage With Benchmarks Before Changing It

Goal: avoid a premature rope or gap-buffer rewrite.

Tasks:

- [ ] Add simple benchmarks or measurement scripts for representative editing,
  movement, search, and rendering workloads.
- [ ] Record concrete thresholds for acceptable Rile 1.0 file sizes.
- [ ] Consider alternate storage only if measurements show current storage blocks
  the product goal.

Risk: high if implemented without evidence.

Validation gate:

- [ ] Benchmark results are recorded before proposing a storage migration.
- [ ] Existing buffer, editor, PTY, and snapshot tests pass after any storage
  change.

## Deferred Until After Rile 1.0 Unless Goals Change

- Plugin system.
- Async runtime.
- Multi-crate workspace.
- Terminal backend replacement.
- Broad filesystem or shell trait abstraction.
- Buffer storage rewrite without benchmark evidence.

## Rust Design Rules For This Plan

- Prefer concrete structs and modules over traits.
- Add a trait only when there are at least two real implementations or a clear
  testing seam that cannot be handled with pure helper functions.
- Avoid `Arc`, `Mutex`, `RwLock`, channels, and async unless a feature requires
  concurrency.
- Keep ownership local and explicit; do not introduce shared mutable state to
  reduce borrow-checker friction.
- Move code before changing behavior. Do not combine mechanical extraction with
  semantic changes.

## Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Splitting `Editor` widens visibility too much | Extract pure helpers first; stop if many fields need `pub(crate)` access. |
| View-state cleanup breaks terminal behavior | Add characterization tests before changing ownership. |
| Command dispatch refactor adds indirection | Keep concrete handlers unless command growth proves painful. |
| Render snapshot becomes speculative | Do it only for a concrete testing or UI-boundary need. |
| Tests become slower or flaky | Keep pure behavior in unit tests and reserve PTY tests for terminal-visible behavior. |
| Documentation drifts from implementation | Update this plan's Progress Log and move durable conclusions into guides when work completes. |

## Validation Strategy

- Use focused unit tests for helper extraction and pure logic.
- Use PTY tests for terminal-visible behavior, cursor placement, prompts,
  splits, scrolling, and special-buffer restoration.
- Use `make snapshot-test` for rendering or viewport changes.
- Use `make verify` before treating a phase as complete.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-06-26 | Completed Step 2.1 returnable special-buffer helper cleanup. | `src/editor.rs` now uses shared helpers for returnable special-buffer return-slot preservation, special-buffer display, transient state clearing, and viewport restore; repeated-open unit tests cover help, messages, shell-output, and completions-buffer return slots; buffer-list behavior was left untouched; focused help, messages, completion, shell-command, and PTY movement/completion/shell-command tests passed. |
| 2026-06-26 | Completed Step 1.5 Phase 1 reassessment. | `make verify` passed after Step 1.5; review found extracted helper modules remain `pub(super)`, returnable special-buffer open/restore handling is the clearest repeated pattern, and buffer-list refresh should stay distinct unless a helper remains obvious. |
| 2026-06-26 | Completed Step 1.4 prompt history extraction. | `src/editor/prompt_history.rs` now owns per-prompt-kind history storage, duplicate suppression, navigation, and draft restoration while `Editor` still submits prompts and refreshes completion; `./scripts/in-container cargo test --locked prompt_history -- --nocapture`, `./scripts/in-container cargo test --locked completion -- --nocapture`, and `./scripts/in-container cargo test --locked --test pty_completion vertical_mx_prompt_history_recalls_previous_command -- --nocapture` passed. |
| 2026-06-26 | Completed Step 1.3 pure search helper extraction. | `src/editor/search.rs` now owns exact forward/backward match helpers and UTF-8-aware repeat-start advancement while `Editor` keeps incremental-search prompt, wrap, and failure state; `./scripts/in-container cargo test --locked editor::search -- --nocapture` and `./scripts/in-container cargo test --locked --test pty_search -- --nocapture` passed. |
| 2026-06-26 | Completed Step 1.2 pure help formatting extraction. | `src/editor/help.rs` now owns help/about/describe formatting helpers while `Editor` still collects state and opens help buffers; focused editor help tests and representative PTY movement help tests passed. |
| 2026-06-26 | Completed the remaining Step 1.1 characterization checks. | `tests/pty_scrolling.rs` covers horizontal scroll state across split-window and buffer switches; existing help, messages, shell-output, and completions-buffer return tests were confirmed with `./scripts/in-container cargo test --locked --test pty_scrolling -- --nocapture`, `./scripts/in-container cargo test --locked --test pty_movement -- --nocapture`, `./scripts/in-container cargo test --locked --test pty_shell_command -- --nocapture`, and the `editor::tests::completions_buffer_completion_restores_previous_buffer_on_cancel` / `editor::tests::completions_buffer_completion_restores_previous_buffer_on_accept` unit tests. |
| 2026-06-26 | Added Step 1.1 split-window switch-buffer coverage. | `tests/pty_split_pane.rs` covers switching buffers in the selected split while preserving per-buffer point and selected-window state; `./scripts/in-container cargo test --locked editor -- --nocapture` and `./scripts/in-container cargo test --locked --test pty_split_pane -- --nocapture` passed. |
| 2026-06-26 | Added the first Step 1.1 safety test for killing a buffer shown in multiple windows. | `tests/pty_split_pane.rs` covers replacing every window that displayed the killed buffer while preserving the selected window; `./scripts/in-container cargo test --locked editor -- --nocapture` and `./scripts/in-container cargo test --locked --test pty_split_pane -- --nocapture` passed. |
| 2026-06-26 | Plan created from architecture review and self-critique. | User requested a written phased improvement plan. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-06-26 | Close Phase 1; Step 2.1 is the only currently justified next candidate. | Full verification is green after the small extractions, and the remaining concrete pain is repeated returnable special-buffer open/restore logic rather than a need for broad `Editor` decomposition. |
| 2026-06-26 | Prefer Phase 1 safety-first cleanup before Phase 2 architecture work. | The current risk is editor centralization, but aggressive refactors could break subtle terminal behavior. |
