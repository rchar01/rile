<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Architecture Improvement Ideas

This document records possible architecture improvements for Rile. It is not an
active implementation checklist and does not replace `docs/architecture.md` as
the description of how the editor works today.

The default rule is to preserve user-visible behavior. Refactors should be small,
test-backed, and justified by concrete maintenance pain or a feature that needs a
cleaner seam.

## Current Pressure Points

- `src/editor.rs` is still the main hotspot for interactive workflows and mutable
  session coordination.
- View-state synchronization between buffers, windows, editor state, and terminal
  drawing is subtle and easy to regress.
- Command metadata and command dispatch are still coupled through concrete
  handler pointers in `src/command.rs`.
- Terminal rendering currently prepares output and also participates in keeping
  point visible.
- Buffer storage uses `Vec<String>` lines. It is simple and testable, but should
  be evaluated with benchmarks before large-file goals become stricter.

## Improvement Candidates

### Special-Buffer Workflows

Returnable special buffers such as help, messages, completions, and shell output
share return-slot and viewport-restoration behavior. Existing cleanup has reduced
duplication, but new special-buffer types should keep that helper path small and
explicit rather than adding a dynamic registry prematurely.

Buffer-list behavior is related but distinct because it refreshes visible rows
and interacts with window selection differently. Do not force it through a shared
helper unless the result remains clearer than the current direct code.

### Completion Prompt Policy

Completion matching and ranking should remain in `src/completion.rs`. Prompt
acceptance behavior should stay isolated in the editor-side completion policy
helper so file, buffer, command, option, and raw-input decisions can be tested
without spreading prompt rules through command handlers.

Future completion work should preserve that boundary unless a new prompt type
proves the current helper is too narrow.

### Command Metadata And Dispatch

`src/command.rs` should continue to own command IDs, names, categories, summaries,
full docs, and registry validation. If command growth makes handler coupling hard
to maintain, move concrete command-to-handler dispatch into an editor-side module.

Avoid trait objects or dynamic dispatch unless there is a real second command
implementation. The current concrete handler table is easy to validate and should
not be replaced only for aesthetics.

### View-State Ownership

Before changing view-state ownership, identify and test the canonical source of
truth for selected window, selected buffer, point, and viewport state. The riskiest
areas are split panes, buffer switching, help/completion return, horizontal
scrolling, and killing buffers shown in multiple windows.

Any cleanup here should start with characterization tests and targeted PTY tests.
Removing synchronization helpers without replacement invariants is high risk.

### Render Snapshot Boundary

A render snapshot could make terminal drawing consume a prepared view model
instead of mutating editor state during drawing. This should happen only if it
supports a concrete testing need, viewport-state cleanup, or future UI boundary.

Keep ANSI output in the terminal adapter. Do not introduce a generic frontend
abstraction before another frontend exists.

### Error Modeling

Rile currently uses simple errors in many paths. Add specific error variants only
when callers need to branch on the error. Do not convert every string error into
an enum variant by default.

User-facing message changes should be tested with focused unit or PTY coverage
when they affect visible behavior.

### Buffer Storage

Keep the current line-vector buffer until benchmarks or concrete editing goals
show it is insufficient. A rope, gap buffer, or piece table would be a large
behavioral and testing change.

Any storage proposal should first record representative measurements for opening,
editing, searching, moving, and rendering large files and long lines.

## Deferred Ideas

- Plugin system.
- Async runtime.
- Multi-crate workspace.
- Terminal backend replacement.
- Broad filesystem or shell trait abstraction.
- Buffer storage rewrite without benchmark evidence.

## Design Rules

- Prefer concrete structs and modules over traits.
- Add a trait only when there are at least two real implementations or a clear
  testing seam that cannot be handled with pure helper functions.
- Avoid `Arc`, `Mutex`, `RwLock`, channels, and async unless a feature requires
  concurrency.
- Keep ownership local and explicit; do not introduce shared mutable state to
  reduce borrow-checker friction.
- Move code before changing behavior. Do not combine mechanical extraction with
  semantic changes.

## Validation Expectations

- Use focused unit tests for helper extraction and pure logic.
- Use PTY tests for terminal-visible behavior, cursor placement, prompts, splits,
  scrolling, and special-buffer restoration.
- Use `make snapshot-test` for rendering or viewport changes.
- Use `make verify` before treating architecture changes as complete.
