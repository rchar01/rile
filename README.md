<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

<div align="center">
  <img src="assets/brand/rile-forge-avatar-transparent-512.png" alt="Rile forge avatar" width="256">

  <p><strong>A small UTF-8-capable terminal Emacs-style editor written in Rust.</strong></p>
</div>

---

Rile is a terminal-native editor for practical daily editing of source files,
configuration files, Markdown, and normal UTF-8 text. It follows familiar Emacs
editing conventions while keeping the implementation small and Rust-native.

Official repository: <https://codeberg.org/rch/rile>

## Status

Rile is under active early development. It can edit UTF-8 text, open and save
files, use Emacs-style key bindings, run `M-x` commands with completion, manage
buffers and split windows, search and query-replace with highlights, run shell
commands, use registers and rectangles, show help buffers, highlight common
source/config formats, and load basic user preferences.

The v1 goal is a dependable lightweight editor for source files, config files,
Markdown, and other plain text. Some advanced Emacs behavior is intentionally
not implemented yet.

## Quick Start

Requirements for running from source:

- Rust 2024 toolchain when developing directly on the host.
- Or `podman` and `make` for the preferred dev-container workflow.

Current binary behavior:

```sh
cargo run -- [file]
```

Editing mode requires an interactive terminal. `--help` and `--version` work without one. When no file is provided, Rile opens a clean `*Rile*` welcome buffer. When a file path is provided, Rile opens it as UTF-8 before entering raw mode, rejects NUL-containing binary files, and shows file/dirty state, position, and major mode in the mode line.

Developer visual testing flags are available for deterministic terminal review: `--visual-test` uses deterministic defaults and a verbose visual-test mode line, while `--test-size WIDTHxHEIGHT` overrides terminal size during rendering.

## Usage

Basic editor keys:

- `C-f`/right arrow and `C-b`/left arrow move horizontally.
- `M-f` and `M-b` move forward and backward by word.
- `C-n`/down arrow and `C-p`/up arrow move vertically.
- `C-v`/PageDown and `M-v`/PageUp scroll by one visible page.
- `C-l` recenters the current line in the window.
- `C-a`/Home and `C-e`/End move within the current line.
- `M-<` and `M->` move to the beginning and end of the buffer.
- `M-g g` prompts for a line or `line:column` and moves point there.
- `M-m` moves to the first non-whitespace character on the current line, or to
  line end when the line contains only whitespace.
- `C-h` after a prefix such as `M-g` shows available bindings for that prefix;
  type `q` in the help buffer to restore the previous buffer.
- Printable UTF-8 text inserts at point.
- `C-q` quotes the next key, inserting printable text, Tab, or Enter literally.
- `C-u` supplies a numeric argument for the next repeatable command; repeated
  `C-u` multiplies by four, and digits enter an explicit count.
- `C-x (` starts recording a keyboard macro, `C-x )` ends it, and `C-x e`
  replays the latest macro. `C-u` before `C-x e` repeats macro execution.
- Backspace deletes before point; `C-d`/Delete deletes at point.
- `M-d` kills the next word; `M-Backspace` kills the previous word.
- `M-^` joins the current line to the previous line, trimming indentation around
  the join.
- `C-j` inserts a newline and leaves point at the start of the new line in the
  current plain-text mode.
- `C-o` opens a line at point without moving point.
- `C-@` sets the mark at point.
- `C-x SPC` starts rectangle mark mode; `C-w`, `M-w`, and `C-y` then kill,
  copy, and yank rectangular columns.
- `C-x r k`, `C-x r M-w`, and `C-x r y` kill, copy, and yank rectangles
  using mark and point. `C-x r d` deletes a rectangle without saving it,
  `C-x r c` clears it to spaces, `C-x r o` opens blank columns, `C-x r t`
  replaces it with a prompted string, and `C-x r N` inserts line numbers.
- Single printable-character registers support `C-x r SPC` to save point,
  `C-x r j` to jump, `C-x r s` to copy the active region, `C-x r r` to copy a
  rectangle, `C-x r i` to insert text, rectangle, or number values, and
  `C-x r n` / `C-x r +` to store and increment number registers.
- `M-!` prompts for a shell command and displays captured stdout/stderr in a
  read-only `*Shell Command Output*` buffer. `C-u M-!` inserts stdout at point
  after a successful command.
- `M-|` sends the active region to a shell command on stdin and displays
  captured output. `C-u M-|` replaces the active region with stdout after a
  successful command.
- `C-x h` marks the whole buffer, leaving point at the beginning.
- `C-x C-x` exchanges point and mark.
- `C-w` kills the active region.
- `M-w` copies the active region.
- `C-y` yanks the latest kill or copy; consecutive kill commands coalesce into
  one yankable entry.
- `M-y` immediately after `C-y` or another `M-y` rotates through earlier kill-ring
  entries.
- `C-k` kills to the end of the line, or the line break at end of line.
- `C-_` undoes the latest edit in the current buffer.
- `C-x C-s` saves the current file.
- `C-x C-w` prompts for a file path and writes the current buffer there.
- `C-x C-f` prompts for a file path with completion and opens it.
- `C-x i` prompts for a file path with completion and inserts its contents.
- `C-x C-r` prompts for a file path with completion and opens it read-only.
- `C-x C-q` toggles whether the current normal buffer is read-only.
- `C-x b` prompts for a buffer name with completion and switches to it,
  preserving each buffer's point; empty input switches to the default buffer,
  and Tab or Enter accepts the selected candidate when the input is not exact.
- `C-x C-b` shows a read-only `*Buffer List*` in another window.
- `C-x k` prompts for a buffer name with completion and kills it; empty input
  kills the current buffer, Tab or Enter accepts the selected candidate when the
  input is not exact, and buffers with unsaved changes use an Emacs-style
  `y-or-n-p` confirmation.
- `C-x 2` splits the current window below.
- `C-x 3` splits the current window right.
- `C-x 0` deletes the current window.
- `C-x 1` deletes other windows.
- `C-x o` selects the next window.
- `C-s` starts forward incremental search; repeat `C-s` jumps to the next match.
- `C-r` starts backward incremental search; repeat `C-r` jumps to the previous match.
- Repeating search at a buffer boundary first reports a failing search; repeating
  again wraps to the first or last match and shows a wrapped-search prompt.
- `M-%` starts query replace; enter search and replacement strings, then use `y` to replace, `n` to skip, `!` to replace all remaining matches, and `q` to quit.
- `M-x toggle-syntax-highlighting` toggles syntax highlighting on and off.
- `M-x toggle-search-highlighting` toggles search/query-replace highlights on and off.
- `M-x toggle-line-numbers` toggles line-number display on and off.
- `C-x C-c` quits, prompting for `yes` before exiting when normal buffers have
  unsaved changes.
- `M-x` runs a command by name with completion; `C-n`/Down and `C-p`/Up move
  through candidates, Tab completes a common prefix, and Enter accepts the
  selected candidate. Command candidates show the first known key binding when
  available.
- `C-h k` describes a key binding, `C-h f` (`describe-function`) prompts for
  an interactive command with command-name completion, and `C-h e` opens the
  read-only `*Messages*` message history.
- `M-p` and `M-n` move through history in command, file, buffer, write-file,
  goto-line, rectangle, shell-command, and describe-function minibuffer prompts.
- `C-g` cancels minibuffer prompts and prefix keys.

Current search and query replace use exact UTF-8 substring matching within individual lines. Incremental search wraps after an explicit boundary failure; query replace does not wrap, and neither command matches across line breaks yet.
Highlighting now flows through shared face spans and deterministic priority merging for region, search, query-replace, mode-line, minibuffer, and error faces.
Syntax modes are selected from file extensions for Rust, C, shell, Markdown, and TOML, with a plain-text fallback.
Window splitting stores per-window cursor state and scrolls automatically to keep point visible. Empty rows are left blank rather than filled with marker characters.
Undo is buffer-local for current-buffer edits and groups normal typing, but does not yet provide redo or advanced Emacs undo traversal.

## Configuration

Rile loads a minimal TOML-style config file from `~/.config/rile/config.toml` when it exists. Supported keys:

```toml
tab_width = 4
line_numbers = false
syntax_highlighting = true
search_highlighting = true
backup_on_save = false # when true, save previous contents to file~
theme = "default" # or "mono"
completion_style = "vertical" # "completions-buffer" or "ido"
completion_max_candidates = 8
completion_show_annotations = true
completion_matching = "prefix" # or "substring"
```

Completion currently applies to `M-x` command names, `C-h f` command names,
`C-x C-f`, `C-x C-r`, and `C-x i` file names, and `C-x b`/`C-x k` buffer
names.
Command completion candidates show the first known key binding, such as
`save-buffer (C-x C-s)`, when one exists.

## License

Rile is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

Rile is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See `COPYING` for details.

Copyright (c) 2026 Robert Charusta <rch-public@posteo.net>.

## Development

The preferred development workflow uses Podman, `make`, and the project dev container. See [docs/README.md](docs/README.md) for maintainer documentation and [docs/testing.md](docs/testing.md) for the testing workflow. The host only needs:

- `podman`
- `make`

The dev container provides the Rust toolchain and project quality tools: `rustup`, `cargo`, `rustfmt`, `clippy`, `rust-analyzer`, `cargo-nextest`, `cargo-deny`, `cargo-audit`, and `cargo-machete`.

Common commands:

```sh
make help
make shell
make build
make fmt
make test
make lint
make audit
make verify
```

For direct host development, install the same Rust tools locally and run the scripts under `scripts/` directly.

Release notes are maintained in [NEWS](NEWS). GNU-style file-level maintenance history is maintained in [ChangeLog](ChangeLog); Git remains the detailed development history.

CI is deferred until it is configured for the official repository.

## Reference Policy

The repository includes optional reference-testing tooling for studying behavior of reference editors such as GNU Zile, kg, and GNU Emacs. Rile should use reference editors only for behavior and architecture lessons unless license implications are explicitly documented. Do not copy, translate, or mechanically port reference implementation code into Rile.

See [NOTICE.md](NOTICE.md) for the current third-party code status.
