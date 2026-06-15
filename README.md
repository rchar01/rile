<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Rile

Rile Is Lightweight Emacs.

Rile is planned as a small, fast, terminal-native, Emacs-style text editor written in Rust. The v1 goal is practical daily editing of source files, config files, Markdown, and normal UTF-8 text.

Official repository: <https://codeberg.org/rch/rile>

## Status

Milestone 14 configuration and polish is implemented. The editor can insert text, move the cursor, delete text, save, quit, run `M-x` commands with command-name completion, open files with `C-x C-f`, switch and kill buffers, split/delete/select windows, search with active highlights, use basic region editing, run interactive query replacement, highlight common source/config formats, and load basic user preferences.

Current binary behavior:

```sh
cargo run -- [file]
```

Editing mode requires an interactive terminal. `--help` and `--version` work without one. When no file is provided, Rile opens a clean `*Rile*` welcome buffer. When a file path is provided, Rile opens it as UTF-8 before entering raw mode, rejects NUL-containing binary files, and shows file/dirty state, position, and major mode in the mode line.

Developer visual testing flags are available for deterministic terminal review: `--visual-test` uses deterministic defaults and a verbose visual-test mode line, while `--test-size WIDTHxHEIGHT` overrides terminal size during rendering.

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
- Backspace deletes before point; `C-d`/Delete deletes at point.
- `M-d` kills the next word; `M-Backspace` kills the previous word.
- `M-^` joins the current line to the previous line, trimming indentation around
  the join.
- `C-o` opens a line at point without moving point.
- `C-@` sets the mark at point.
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
- `C-x b` prompts for a buffer name and switches to it.
- `C-x C-b` shows a read-only `*Buffer List*` in another window.
- `C-x k` prompts for a buffer name and kills it; empty input kills the current buffer.
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
- `C-x C-c` quits.
- `M-x` runs a command by name with completion; `C-n`/Down and `C-p`/Up move
  through candidates, Tab completes a common prefix, and Enter accepts the
  selected candidate.
- `C-h k` describes a key binding, and `C-h f` describes an interactive
  command with command-name completion.
- `M-p` and `M-n` move through history in command, file, buffer, write-file,
  goto-line, and describe-command minibuffer prompts.
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
`C-x C-f`, `C-x C-r`, and `C-x i` file names, and `C-x b` buffer names.

## License

Rile is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

Rile is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See `COPYING` for details.

Copyright (c) 2026 Robert Charusta <rch-public@posteo.net>.

## Development

The preferred development workflow uses Podman, `make`, and the project dev container. See `docs/README.md` for maintainer documentation and `docs/testing.md` for the testing workflow. The host only needs:

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

Release notes are maintained in `NEWS`. GNU-style file-level maintenance history is maintained in `ChangeLog`; Git remains the detailed development history.

CI is deferred until it is configured for the official repository.

## Reference Policy

The repository includes optional reference-testing tooling for studying behavior of reference editors such as GNU Zile, kg, and GNU Emacs. Rile should use reference editors only for behavior and architecture lessons unless license implications are explicitly documented. Do not copy, translate, or mechanically port reference implementation code into Rile.

See `NOTICE.md` for the current third-party code status.
