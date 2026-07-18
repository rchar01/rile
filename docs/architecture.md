<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Architecture

Rile is a single Rust crate for a terminal-native Emacs-style editor. The code is
organized around concrete modules and explicit state ownership rather than a
plugin system, async runtime, or broad abstraction layers.

The current architecture favors small, testable helper modules while keeping
`Editor` as the application coordinator. Executable sources are the source of
truth when this guide and code disagree.

## Runtime Flow

`src/main.rs` starts the binary, delegates argument parsing to `src/app.rs`, and
then enters the terminal editor path. Interactive editing requires a terminal;
metadata commands such as `--help` and `--version` can run without one.

The terminal loop reads key events, asks `Editor` to handle them, then redraws
the current frame. Most user-visible behavior is tested either through unit tests
on the owning modules or through PTY tests that spawn the real binary.

## Main Modules

- `src/app.rs`: CLI parsing and application startup decisions.
- `src/terminal/mod.rs`: raw terminal integration, ANSI drawing, screen layout,
  completion display, mode-line drawing, and cursor placement.
- `src/input.rs`: terminal byte-to-key parsing for control, Meta, text, and
  special keys.
- `src/editor.rs`: interactive editor state, command handlers, prompt workflows,
  special-buffer behavior, and coordination between buffers, windows, search,
  registers, rectangles, macros, and rendering.
- `src/command.rs`: command IDs, names, categories, summaries, docs, handlers,
  and registry validation.
- `src/keymap.rs`: global and local keymaps, active-stack resolution, prefix
  discovery, and binding formatting.
- `src/buffer/`: UTF-8 text storage, positions, ranges, editing primitives,
  display-width helpers, and undo records.
- `src/file.rs`: file-backed documents, special documents, UTF-8 validation,
  read-only state, and safe save/write-file operations.
- `src/buffers.rs`: stable buffer IDs, buffer names, file reuse, dirty-buffer
  checks, and buffer-list document creation.
- `src/window.rs`: split tree, selected window, per-window buffer/cursor/viewport
  state, and layout calculations.
- `src/render/`: face spans, decoration providers, span priority merging, and
  clipping helpers shared by region, search, query replace, syntax, mode lines,
  and minibuffer rendering.
- `src/completion.rs`: completion candidates, matching, ranking, and file-category
  behavior.
- `src/minibuffer.rs`: prompt state, editable prompt text, cursor movement, and
  minibuffer messages.
- `src/shell.rs`: foreground shell-job spawning, bounded incremental pipe
  transfer, output capture, deadlines, cancellation, and process-group cleanup.
- `src/syntax.rs`: major-mode selection and lightweight syntax highlighting.
- `src/config.rs`, `src/option.rs`, and `src/mode.rs`: user configuration and
  inspectable option/mode metadata.

## Editor State

`Editor` owns the live editing session. It coordinates buffer selection, window
selection, prompt state, transient messages, command dispatch, kill-ring state,
registers, rectangles, keyboard macros, search state, query-replace state,
configuration options, and special-buffer return slots.

Helper modules under `src/editor/` hold pure or narrowly scoped behavior that was
split out of the main editor body:

- `src/editor/help.rs`: generated help/about/describe text.
- `src/editor/search.rs`: buffer-position search helper logic.
- `src/search_pattern.rs`: literal and built-in regexp pattern matching.
- `src/editor/prompt_history.rs`: per-prompt-kind history storage and navigation.
- `src/editor/completion_policy.rs`: completion prompt Enter, `M-RET`, Tab,
  directory descent, and exact-input acceptance policy.

`Editor` remains the place where those helpers are connected to mutable editor
state and user-visible command workflows.

## Commands And Key Dispatch

Interactive commands are registered in `src/command.rs` with stable typed IDs,
user-facing names, summaries, full help text, categories, and concrete `Editor`
handler function pointers. Registry tests reject missing handlers, duplicate
IDs, duplicate names, and missing metadata.

`src/keymap.rs` maps key sequences to command IDs. Global bindings handle normal
editing, while local maps handle special buffers such as `*Help*`, `*Messages*`,
`*Shell Command Output*`, and `*Buffer List*`. Prefix help and describe-key
commands inspect the same active keymap stack that dispatch uses.

## Shell Commands

`Editor` collects the command, optional region input, working directory, and
explicit insertion or replacement target. `TerminalSession` owns the resulting
`ShellJob`; no OS process handle is stored in cloneable editor state. The job
places `/bin/sh` in a dedicated process group and incrementally polls nonblocking
stdin, output, and child status with fixed per-poll work budgets. Display jobs
connect stdout and stderr to one stream and incrementally decode UTF-8 into the
generated output buffer. Mutation jobs retain separate stdout and stderr until
successful completion. Both modes share an 8 MiB cumulative byte budget, and
execution has a 30-second deadline.

Shell commands remain logically foreground: normal editor keys do not execute
while a job is active, but the terminal loop continues polling, redrawing, and
reading input. Terminal-owned suppression remains active through a quiet input
boundary so bytes queued behind Enter, completion, or cancellation cannot become
later edits. `C-g` discards partial output and sends `SIGINT` to the process group;
a second `C-g` or a 250 ms grace-period expiry sends `SIGKILL`. `C-x C-c` cancels
before following normal clean or dirty-buffer quit behavior, and suspension is
rejected until the command is cancelled.

Streaming display output is appended internally without entering a user undo
history. Partial transcripts remain visible after timeout, overflow,
cancellation, or decoding failure. Completion applies mutation output only to
the captured buffer and position or range.
If that target is unavailable or invalid, Rile preserves successful output in
`*Shell Command Output*` instead of guessing another target. Timeout, output
overflow, cancellation, and decoding failures never apply partial output.

Reaping is event-loop driven rather than a blocking cleanup sleep. A shared
reaper starts before the first shell child, and shell startup fails if that
service cannot start. Emergency drop and setup-failure cleanup close pipes,
signal the group and direct child, and transfer the unreaped child without
waiting on the terminal thread. Process-group cleanup covers ordinary
descendants; a process that deliberately starts a new session can escape that
group and may keep inherited output descriptors open until the command deadline.
Timeout or cancellation closes Rile's local pipes, so detached descendants
cannot hold capture open indefinitely.

## Buffers And Documents

Text is stored as `Vec<String>` lines in `Buffer`. Positions use line plus UTF-8
byte column, and editing helpers validate UTF-8 boundaries. Display-column
helpers account for tabs, grapheme clusters, and double-width characters where
rendering needs them.

`Document` wraps a buffer with document kind, path, read-only state, dirty state,
and file metadata. Existing files open as strict UTF-8, missing files create clean
named buffers, NUL-containing binary files are rejected, and saves write through a
same-directory temporary file before rename.

`BufferManager` owns all buffers and preserves stable `BufferId` values. Opening
an already-open path reuses the existing buffer instead of creating a duplicate.

## Windows And Viewports

`WindowSet` stores a split tree with stable window IDs. Each window records the
buffer it displays plus cursor and viewport state. Splits can be stacked or
side-by-side, and terminal drawing computes a layout rectangle for each visible
window.

Viewport state tracks the first visible line and horizontal scroll. Rendering and
movement keep point visible, including horizontal recentring for clipped long
lines. Split and buffer-switch behavior is covered by PTY tests because terminal
coordinates and viewport restoration are subtle.

## Rendering

Rendering is terminal-oriented. `src/terminal/mod.rs` prepares the visible rows,
draws buffer text, gutters, mode lines, minibuffer prompts, completion candidates,
and special-buffer contents, then places the terminal cursor.

Dynamic display text crosses a control-escaping boundary before ANSI output.
Tabs retain tab-stop expansion, while other C0 and C1 control characters render
as visible ASCII escapes. Only renderer-owned cursor, clearing, and face
sequences use the raw ANSI path. Display-width, clipping, span, and cursor
calculations account for the expanded control representation.

Normal buffer rows project source text directly into the horizontal viewport
instead of expanding the complete logical line. Projection retains source-byte
face boundaries, expands tabs and controls incrementally, and uses a bounded
source-character budget. Exhausting that budget produces a conservative right
truncation marker. Syntax, search, and persistent-highlight providers still
prepare source spans before terminal projection and may inspect the complete
logical line.

Decorations are expressed as face spans. `src/render/` merges overlapping spans by
priority and clips them to the visible byte range. This lets syntax, region,
search, query-replace, mode-line, minibuffer, warning, and error styling share
one rendering path.

## Minibuffer And Completion

The minibuffer supports messages and active prompts. Prompt state tracks editable
input, cursor byte offset, prompt kind, completion session, and prompt history.

Completion sources cover commands, options, files, buffers, write-file prompts,
and describe prompts. Matching and ranking live in `src/completion.rs`, while
prompt-specific acceptance policy lives in `src/editor/completion_policy.rs`.
File prompts use file-category behavior rather than global command-style
orderless matching.

## Search, Replace, Registers, Rectangles, And Macros

Incremental search and query replace are coordinated by `Editor` because they
span prompts, cursor movement, highlights, wrapping/failure state, and undo.
Buffer traversal is kept in `src/editor/search.rs`; literal and line-local
regexp pattern matching lives in `src/search_pattern.rs`. The regexp parser
compiles its AST into a size-limited ordered Thompson program. An iterative
Pike-style VM keeps one highest-priority thread per instruction and input slot,
preserving leftmost-first greedy matching and captures without recursive
backtracking. Forward and backward scans inject candidate starts into the same
bounded thread lists instead of restarting the matcher at every character.

Registers support point, text, rectangle, and number values. Rectangles support
mark mode, kill/copy/yank, delete, clear, open, string replacement, and line
number insertion. Keyboard macros record key events and replay through normal
editor input handling so prompts and commands use the same path as user input.

## Configuration And Modes

Configuration is loaded from `~/.config/rile/config.toml` when present. The option
registry describes supported keys, defaults, validation, and `describe-variable`
metadata.

Major modes and syntax modes are selected from buffer paths and document kinds.
Mode metadata is inspectable through `describe-mode`; special-buffer modes are
reported as active modes without replacing the normal major-mode slot in the mode
line.

## Testing Architecture

Unit tests live beside modules and cover pure logic and editor behavior that does
not require terminal I/O. PTY tests under `tests/pty_*.rs` spawn the real binary,
send key input, parse VT100 screen state, and assert visible text, cursor
position, scrolling, splits, prompts, and save behavior. Parsed-screen snapshots
cover selected deterministic terminal layouts.

The canonical quality gate is `make verify`. Focused development usually starts
with targeted `./scripts/in-container cargo test --locked ...` commands before
running the full gate.

## Known Hotspots

- `src/editor.rs` remains the main coordination hotspot. It is large because it
  owns session state and command workflows, even though several pure helpers have
  already been extracted.
- View-state ownership is split between editor, windows, and terminal drawing.
  This is behaviorally important but still more coupled than ideal.
- `src/command.rs` combines command metadata with concrete handler pointers. This
  is simple and validated, but command growth may eventually justify a cleaner
  dispatch boundary.
- Buffer storage is intentionally simple. A storage rewrite should be driven by
  measured editing or rendering limits, not by architecture preference alone.
