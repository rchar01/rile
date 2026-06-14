<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Development Notes

## Repository Scope

`rile/` is the implementation project and intended distributable repository. The parent `rile-lab/` workspace is private research context and is not part of the Rile crate.

The official repository is <https://codeberg.org/rch/rile>. Rile is copyrighted by Robert Charusta <rch-public@posteo.net>.

## Current Scope

Milestone 1 established project hygiene:

- license files and notice;
- README and project-local docs;
- library plus binary crate structure;
- initial module boundaries;
- crate-local error type;
- minimal `rile [file]` CLI parsing;
- smoke tests and quality commands.

Milestone 2 adds the terminal core:

- direct Unix termios raw mode through `libc`;
- drop-based raw-mode restoration;
- alternate-screen and cursor cleanup guard;
- terminal size query;
- buffered ANSI output wrapper;
- key parsing for Ctrl, Meta/ESC, printable UTF-8, arrows, Home/End, PageUp/PageDown, Backspace, Delete, Enter, and Tab;
- minimal fullscreen draw path that exits with `C-q`.

Milestone 3 adds the UTF-8 buffer core:

- `Vec<String>` line storage behind `buffer::Buffer`;
- UTF-8-boundary-checked `Position` and `TextRange` validation;
- load from string and serialize to string;
- insert, delete, and text extraction by range;
- grapheme-aware horizontal movement;
- display-column-preserving vertical movement;
- word movement;
- display width and visible byte-range helpers;
- dirty flag and final-newline tracking;
- undo record shape for insert, delete, replace, and cursor restoration.

Milestone 4 adds file-backed documents:

- `file::Document` wraps a `Buffer` with an optional path;
- existing files open as strict UTF-8;
- missing files create named clean buffers;
- invalid UTF-8 is rejected without lossy conversion;
- save and save-as write through a same-directory temporary file and rename;
- successful saves mark the buffer clean;
- failed saves leave the buffer dirty;
- the terminal shell opens the requested document and displays a basic mode line.

Milestone 5 adds basic editor commands and keymaps:

- `command::CommandRegistry` maps exact command names to internal commands;
- `keymap::KeyMap` resolves single-key and prefix key sequences;
- `C-x` prefix handling supports `C-x C-s` save and `C-x C-c` quit;
- movement commands support character, word, line, beginning-of-line, and end-of-line motion;
- printable UTF-8 text, Enter, and Tab insert into the current buffer;
- Backspace, Delete, and `C-d` delete text around point;
- minimal `M-x` accepts an exact command name and executes it;
- `editor::Editor` owns interactive editor state and is testable without a terminal;
- the terminal loop delegates key handling to `Editor` and redraws the current buffer.

Milestone 6 adds minibuffer prompt transitions:

- `minibuffer::MinibufferState` stores either a status/error message or an active prompt;
- prompt state records prompt kind, label, and editable input;
- prompt backspace deletes by grapheme cluster;
- `M-x` uses the shared minibuffer prompt path;
- `C-x C-f` prompts for a file path and opens existing or missing files;
- successful operations set status messages;
- errors use explicit `Error: ...` messages;
- `C-g` cancels prompts and prefix keys;
- tests cover prompt editing, command prompts, file prompts, status/error messages, and cancellation.

Milestone 7 adds incremental search:

- `C-s` starts forward incremental search;
- `C-r` starts backward incremental search;
- typed query text updates the current match live;
- repeated `C-s` and `C-r` jump to the next or previous match;
- repeating search at a buffer boundary first reports a failing search, and the
  next repeat wraps to the first or last match with a wrapped-search prompt;
- Enter accepts the current match;
- `C-g` cancels search and restores the original cursor position;
- active search uses `render::Face::CurrentSearchMatch` and `render::Face::SearchMatch` spans;
- the terminal renderer displays active search spans with ANSI highlighting;
- tests cover UTF-8 matches, repeated search, wrapping, cancellation, failed
  search, and ANSI span rendering.

Milestone 8 adds multiple buffers:

- `buffers::BufferManager` owns stable `BufferId` values and buffer entries;
- each buffer entry records a user-facing name and a file-backed `Document`;
- `find-file` reuses an existing buffer for the same path instead of opening duplicates;
- `C-x b` and `switch-to-buffer` prompt for an existing buffer name;
- `C-x k` and `kill-buffer` prompt for a buffer name, with empty input killing the current buffer;
- dirty buffers cannot be killed until saved or made clean by later explicit workflows;
- switching or killing the current buffer resets point to the start of the selected buffer;
- tests cover buffer reuse, switching, killing, and dirty-buffer protection.

Milestone 9 adds windows and splits:

- `window::WindowSet` stores a split tree, stable `WindowId` values, and per-window `Viewport` state;
- `C-x 2` and `split-window-below` split the current window into stacked viewports;
- `C-x 3` and `split-window-right` split the current window into side-by-side viewports;
- `C-x 0` and `delete-window` delete the current window when more than one exists;
- `C-x 1` and `delete-other-windows` collapse back to the selected window;
- `C-x o` and `other-window` cycle through windows and restore each window's cursor;
- terminal drawing lays out all windows, draws one mode line per window, and places point in the selected window;
- layout tests cover horizontal and vertical splitting, deletion, cycling, and per-window viewport state.

Milestone 10 adds region, kill/yank, and undo:

- `C-@` and `set-mark-command` set an active mark at point;
- active regions render through `render::Face::Region` and terminal ANSI highlighting;
- `C-w` and `kill-region` delete the active region into the kill ring;
- `M-w` and `copy-region-as-kill` copy the active region without deleting it;
- `C-y` and `yank` insert the latest kill-ring entry;
- `C-k` and `kill-line` delete to end of line or delete the line break at end of line;
- `C-o` and `open-line` insert a newline at point without moving point;
- `C-_` and `undo` reverse current-buffer insert/delete/yank/kill operations;
- normal printable typing is grouped into a single undo record until another command interrupts it;
- tests cover Unicode-safe region highlighting, kill/yank, kill-line, grouped typing undo, and new control-key parsing.

Milestone 11 adds query replace:

- `M-%` and `query-replace` start an interactive replacement workflow;
- the minibuffer prompts first for the search string, then for the replacement string;
- the current candidate is highlighted with the current-search face;
- choice keys support `y` to replace, `n` to skip, `!` to replace all remaining candidates, and `q`/Escape/`C-g` to quit;
- replacements are UTF-8-safe and reuse the buffer range validation path;
- each replacement records an undo entry so `C-_` can restore replaced text;
- tests cover UTF-8 replacement, skip/all behavior, missing input, highlighting, and undo.

Milestone 12 cleans up the face and decoration architecture:

- `render::Face` now defines stable priority values for overlapping spans;
- `render::Span` has shared construction and validation helpers;
- `render::DecorationProvider` remains the common line-decoration interface;
- `render::collect_spans_for_line`, `merge_spans`, and `clip_spans` centralize decoration collection, priority merging, and viewport clipping;
- region, incremental-search, and query-replace highlights are implemented as editor decoration providers instead of one ad hoc span builder;
- terminal rendering applies mode-line, minibuffer, warning, and error faces through the same face-to-ANSI path;
- tests cover provider collection, UTF-8 boundary rejection, span priority splitting, clipping, and fixed-width faced terminal output.

Milestone 13 adds syntax highlighting:

- `syntax::Highlighter` defines the line-highlighting interface;
- `syntax::MajorMode` selects Emacs-style major-mode names by file extension, including `Fundamental` fallback and `Text` for `.txt` files;
- `syntax::SyntaxMode` is derived from the major mode for highlighting, with a plain-text fallback;
- simple line-local highlighters cover Rust, C, shell, Markdown, and TOML;
- syntax spans use shared `Face::SyntaxKeyword`, `Face::SyntaxString`, and `Face::SyntaxComment` faces;
- syntax spans flow through the same decoration collection and priority merge path as region, search, and query-replace spans;
- syntax highlighting is enabled by default and can be toggled with `M-x toggle-syntax-highlighting`;
- the mode line displays the major mode in parentheses, independent of whether syntax highlighting is enabled;
- tests cover mode selection, language span output, syntax/search/region merge priority, and the toggle command.

Milestone 14 adds configuration and polish:

- `config::Config` loads `~/.config/rile/config.toml` when present and otherwise uses defaults;
- the config parser supports a small TOML subset with `tab_width`, `line_numbers`, `syntax_highlighting`, `search_highlighting`, and `theme` keys;
- `tab_width` controls terminal tab expansion and cursor column calculation for tabs;
- optional line numbers render in a left gutter with `Face::LineNumber`;
- syntax and search highlighting can start disabled from config and can be toggled with `M-x toggle-syntax-highlighting` and `M-x toggle-search-highlighting`;
- line numbers can be toggled with `M-x toggle-line-numbers`;
- `theme = "default"` keeps colored faces and `theme = "mono"` uses mostly monochrome ANSI emphasis;
- tests cover config parsing, invalid config values, editor option application, toggles, tab expansion, and line-number rendering.

Post-Milestone 14 UX polish adds a clean read-only `*Rile*` welcome buffer for no-file launches, blank unused rows instead of Vim-like `~` markers, and compact mode-line position text such as `All (1,0)` alongside the major mode.

Post-Milestone 14 navigation polish adds `M-g g` and `goto-line` with `line` or
`line:column` minibuffer input, clamping out-of-range targets to the current
buffer bounds. It also adds `M-<` and `M->` for moving to the beginning and end
of the current buffer, `C-v`/PageDown and `M-v`/PageUp for visible-page
scrolling with one-line overlap, and `C-l` for recentering point in the current
window. Pending key prefixes echo the current sequence with a `C-h` help hint,
and `C-h` opens a generated read-only `*Help*` buffer for that prefix. Help
buffers display `Type q in help window to restore previous buffer.` and `q`
restores the previous buffer in the current window. Attempts to edit a read-only
special buffer report `Buffer is read-only: <buffer>`, and transient messages
clear on the next non-prompt command unless that command writes a new message.

Post-Milestone 14 region polish adds `exchange-point-and-mark` on `C-x C-x`.
It swaps point with the current buffer's mark, reactivates the region, and
reports `No mark set in this buffer` when no mark exists for the current buffer.

Post-Milestone 14 word-kill polish adds `kill-word` on `M-d` and
`backward-kill-word` on `M-Backspace`, using the same Unicode-aware word
boundaries as `M-f` and `M-b`. Word kills use the existing kill-ring behavior:
each kill is a separate entry, and consecutive kill coalescing remains deferred.

Post-Milestone 14 file polish adds `write-file` on `C-x C-w`, prompting with
`Write file: `, saving the current buffer to the entered path, and making that
path the visited file. Empty input reports `Error: missing file name`.

Post-Milestone 14 minibuffer polish adds command completion for `M-x`, file
completion for `C-x C-f`, and buffer-name completion for `C-x b`. The completion
core is separate from the UI style and supports command-name, file-name, and
buffer-name sources, prefix or substring matching, common-prefix Tab completion,
selected candidate movement with `C-n`/Down and `C-p`/Up, and Enter acceptance.
File completion resolves relative candidates against the current buffer's
directory when available, keeps raw missing-file input working, and descends
into selected directories. Buffer completion keeps exact existing buffer names
working and otherwise requires explicit selection or Tab completion before
switching. The default
`completion_style = "vertical"` reserves rows above the minibuffer and shows
candidate annotations. `completion_style = "completions-buffer"` opens a
temporary read-only `*Completions*` buffer and restores the previous viewport on
accept/cancel. `completion_style = "ido"` is an experimental compact inline
minibuffer display. Supported completion config keys are `completion_style`,
`completion_max_candidates`, `completion_show_annotations`, and
`completion_matching`.

Post-Milestone 14 prompt-history polish adds in-session `M-p` and `M-n` history
navigation for command, file, buffer, write-file, and goto-line minibuffer
prompts. Prompt history is stored per prompt kind, preserves the current draft
while navigating, avoids consecutive duplicate entries, and refreshes completion
candidates after recalling history in completion-enabled prompts. Incremental
search and query-replace history remain deferred because they have separate
interaction models.

Post-Milestone 14 help polish adds `C-h k` and `C-h f` using Rile's existing
read-only `*Help*` special buffer. `C-h k` reads a complete key sequence and
shows the bound command plus its description. `C-h f` uses the shared command
completion source, then shows the selected interactive command's description and
current key bindings. Terminal input parsing uses the original termios erase
byte, so `0x08` remains Backspace on `stty erase ^H` terminals and otherwise
works as `C-h`; `M-Backspace` accepts both `Esc 0x7f` and `Esc 0x08`.

Post-Milestone 14 file polish adds `C-x C-r` / `find-file-read-only`, reusing
the shared file-completion source and relative-path resolution from `C-x C-f`.
Normal file-backed documents now carry an explicit read-only flag. Read-only
file buffers show `RO` in the normal mode line, block editing through the same
read-only guard used for special buffers, and reject save/write-file attempts.
`C-x C-q` / `toggle-read-only` toggles that flag for normal buffers; special
buffers such as `*Rile*`, `*Help*`, and `*Completions*` remain structurally
read-only.

Post-Milestone 14 file polish also adds `C-x i` / `insert-file`, prompting
with `Insert file: ` and using the shared file-completion and relative-path
resolution path. Inserted files use the same UTF-8 and binary-file validation
as file opening, insert at point, mark the current buffer dirty, and record an
undo entry. Empty input reports `Error: missing file name` to match Rile's
existing file prompts, even though base Emacs defaults empty `insert-file` input
to the current file.

Current limitations: there is no prompt cursor movement, no kill-buffer prompt
completion, no incremental-search/query-replace prompt history, no
unsaved-changes quit confirmation, and no redo or advanced Emacs undo traversal
yet. Search and query replace are exact line-local substring matching; they do
not wrap around the buffer and do not match across line breaks.

Milestone 15 hardening has started with binary-file detection: files containing NUL bytes are rejected before UTF-8 decoding so accidental binary opens fail with an explicit message.
The optional `backup_on_save = true` config setting writes the previous contents of an existing file to a sibling `file~` backup before saving the new contents.

Visual terminal testing has started with `--visual-test` and `--test-size WIDTHxHEIGHT`. Visual-test mode uses default config instead of user config and renders deterministic mode-line text for PTY, snapshot, and VHS review. PTY tests assert parsed `vt100` screen state instead of raw escape bytes.

## Line Ending Policy

The in-memory buffer model uses `\n` as the only line separator. `Buffer::from_text` splits on `\n`, `Buffer::serialize` joins lines with `\n`, and `Buffer::final_newline` records whether the serialized text ends with a final newline.

The current file policy preserves carriage return bytes as ordinary text. CRLF files therefore round-trip as CRLF when saved without editing those line endings, while newly inserted line breaks use `\n`. A later polishing milestone can add explicit line-ending detection and conversion controls if needed.

## Save Safety

Saves use a same-directory temporary file followed by `rename`, then best-effort parent-directory sync. This is intended to avoid partially written target files on common Unix filesystems. Permission, directory, and missing-parent errors propagate as `I/O error` values and failed saves keep the buffer dirty.

## Terminal Decision

Rile currently uses direct Unix termios and ANSI escape sequences with only the `libc` crate for platform bindings. This keeps behavior explicit and dependency count low while the terminal model is still small. A higher-level terminal crate can be reconsidered later if portability or feature needs outweigh the extra abstraction.

## License

Rile is copyrighted by Robert Charusta <rch-public@posteo.net> and licensed as `GPL-3.0-or-later`. Keep `COPYING` as the canonical GPLv3 license text and add SPDX identifiers to new source and documentation files.

## Release History Files

Maintain `NEWS` for user-visible release notes, with newest releases first. Keep entries concise and focused on behavior users need to know about.

Maintain `ChangeLog` in GNU-style plain text for file-level maintenance history, with newest entries first. Git remains the detailed history; `ChangeLog` should summarize coherent changes rather than mechanically duplicating every commit.

## Testing

See [Testing Guide](testing.md) for unit, integration, PTY, parsed-screen snapshot, and optional VHS visual-review workflows.

In short, `make verify` is the canonical quality gate. PTY tests assert parsed VT100 screen state from the real `rile` binary, parsed-screen snapshots live under `tests/snapshots/`, and optional GIF/PNG visual artifacts under ignored `artifacts/` are review evidence rather than the pass/fail oracle.

## Quality Gate

The preferred quality gate is:

```sh
make verify
```

`make verify` builds the dev container and runs the project scripts inside it. The scripts are the source of truth for CI and local verification; the Makefile is the friendly command interface.

## Required Tools

Host requirements:

| Tool | Purpose |
| --- | --- |
| `podman` | Builds and runs the dev container. |
| `make` | Provides stable local command targets. |

The dev container in `Containerfile.dev` provides:

| Tool | Purpose | Required |
| --- | --- | --- |
| `rustup` | Toolchain and component management. | Yes |
| `cargo` | Build, test, run, and package commands. | Yes |
| `rustfmt` | Rust formatting checks. | Yes |
| `clippy` | Rust lint checks. | Yes |
| `rust-analyzer` | Editor/LSP support. | Useful, not part of `verify` |
| `cargo-nextest` | Preferred test runner. | Yes, with `cargo test` fallback in `scripts/test` |
| `cargo-insta` | Parsed-screen snapshot checks. | Yes, used by `make verify` |
| `cargo-deny` | License, advisory, source, and dependency policy checks. | Yes |
| `cargo-audit` | Security advisory checks. | Yes |
| `cargo-machete` | Unused dependency detection. | Yes |

Current host status in this workspace: `cargo`, `podman`, and `make` are available; `rustup`, `rustfmt`, clippy, `rust-analyzer`, `cargo-nextest`, `cargo-insta`, `cargo-deny`, `cargo-audit`, and `cargo-machete` are not. That is why the dev container is the canonical tooling environment.

The visual tooling container in `Containerfile.visual` provides `vhs`, `ttyd`, `ffmpeg`, Chromium, and Rust for optional visual artifact generation. It is separate from the normal dev container so `make verify` stays smaller, faster, and independent of browser/video tooling.

## Dev Container Workflow

The dev image is defined in `Containerfile.dev`. It intentionally uses that name instead of plain `Containerfile` so tooling images are not confused with future runtime images.

Interactive development:

```sh
make shell
```

`make shell` sets Podman's interactive detach key sequence to `Ctrl-]` so
Emacs-style `C-p` movement reaches terminal editors instead of being held as
the first byte of Podman's default `Ctrl-p Ctrl-q` detach sequence. Override it
with `PODMAN_DETACH_KEYS`, for example `PODMAN_DETACH_KEYS=ctrl-^ make shell`.

One-shot tasks:

```sh
make build
make fmt
make fmt-check
make test
make snapshot-test
make lint
make audit
make unused-deps
make verify
make visual-demos
make visual-frames
```

The Makefile delegates to scripts:

- `scripts/devshell` opens an interactive shell in the dev container.
- `scripts/in-container` runs one command in a fresh dev container.
- `scripts/build` runs `cargo build --locked`.
- `scripts/fmt` runs `cargo fmt` and updates Rust source formatting.
- `scripts/fmt-check` runs `cargo fmt --check` without modifying files.
- `scripts/test` runs `cargo nextest run --locked` when available, otherwise `cargo test --locked`.
- `scripts/test-cargo` always runs `cargo test --locked`.
- `scripts/snapshot-test` runs check-only parsed-screen snapshot tests through `cargo insta test`.
- `scripts/lint` runs `scripts/fmt-check` and `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `scripts/audit` runs `cargo deny check` and `cargo audit`.
- `scripts/unused-deps` runs `cargo machete`.
- `scripts/verify` runs build, tests, snapshot checks, lint, audit, and unused dependency checks.
- `scripts/visual-demos` validates VHS tapes, builds Rile once, and records optional GIFs.
- `scripts/visual-frames` regenerates visual demos and verifies named PNG screenshots.
- `scripts/tools` prints the versions of expected tools.

`cargo-deny` reads policy from `deny.toml`. The current policy denies yanked crates, denies unknown registries and git sources, denies wildcard dependencies, warns on multiple dependency versions, and allows Rile's GPL license plus the permissive licenses used by current dependencies.

## Direct Host Workflow

Direct host development is supported if the same tools are installed locally. Use scripts directly:

```sh
./scripts/build
./scripts/fmt
./scripts/fmt-check
./scripts/test-cargo
./scripts/snapshot-test
./scripts/lint
./scripts/audit
./scripts/unused-deps
```

On this host, only `./scripts/build` and `./scripts/test-cargo` are expected to work until the missing Rust components and cargo subcommands are installed.

## CI Status

CI is deferred until it is configured for the official repository. Future CI should call the same scripts used by `make verify`; it should not call `scripts/devshell`.

Optional hosted CI visual artifact generation should be a separate, non-blocking job from `make verify`. That job may run `make visual-frames` in the visual tooling container and upload ignored files from `artifacts/` for review. GIFs and PNGs should remain review evidence only; PTY assertions and parsed-screen snapshots remain the correctness gates.
