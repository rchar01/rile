<!--
SPDX-FileCopyrightText: 2026 Rile contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Rile

Rile Is Lightweight Emacs.

Rile is planned as a small, fast, terminal-native, Emacs-style text editor written in Rust. The v1 goal is practical daily editing of source files, config files, Markdown, and normal UTF-8 text.

## Status

Milestone 11 query replace support is implemented. The editor can insert text, move the cursor, delete text, save, quit, run exact-name `M-x` commands, open files with `C-x C-f`, switch and kill buffers, split/delete/select windows, search with active highlights, use basic region editing, and run interactive query replacement.

Current binary behavior:

```sh
cargo run -- [file]
```

Editing mode requires an interactive terminal. `--help` and `--version` work without one. When a file path is provided, Rile opens it as UTF-8 before entering raw mode and shows basic file/dirty state in the mode line.

Basic editor keys:

- `C-f`/right arrow and `C-b`/left arrow move horizontally.
- `C-n`/down arrow and `C-p`/up arrow move vertically.
- `C-a`/Home and `C-e`/End move within the current line.
- Printable UTF-8 text inserts at point.
- Backspace deletes before point; `C-d`/Delete deletes at point.
- `C-@` sets the mark at point.
- `C-w` kills the active region.
- `M-w` copies the active region.
- `C-y` yanks the latest kill or copy.
- `C-k` kills to the end of the line, or the line break at end of line.
- `C-_` undoes the latest edit in the current buffer.
- `C-x C-s` saves the current file.
- `C-x C-f` prompts for a file path and opens it.
- `C-x b` prompts for a buffer name and switches to it.
- `C-x k` prompts for a buffer name and kills it; empty input kills the current buffer.
- `C-x 2` splits the current window below.
- `C-x 3` splits the current window right.
- `C-x 0` deletes the current window.
- `C-x 1` deletes other windows.
- `C-x o` selects the next window.
- `C-s` starts forward incremental search; repeat `C-s` jumps to the next match.
- `C-r` starts backward incremental search; repeat `C-r` jumps to the previous match.
- `M-%` starts query replace; enter search and replacement strings, then use `y` to replace, `n` to skip, `!` to replace all remaining matches, and `q` to quit.
- `C-x C-c` quits.
- `M-x` runs an exact command name.
- `C-g` cancels minibuffer prompts and prefix keys.

Current search and query replace use exact UTF-8 substring matching within individual lines. They do not wrap around the buffer and do not match across line breaks yet.
Window splitting currently stores per-window cursor state but does not scroll automatically yet.
Undo is buffer-local for current-buffer edits and groups normal typing, but does not yet provide redo or advanced Emacs undo traversal.

## License

Rile is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

Rile is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See `COPYING` for details.

## Development

The preferred development workflow uses Podman, `make`, and the project dev container. The host only needs:

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

CI is deferred until this project is initialized as a hosted Git repository. In this workspace, `git` did not recognize `rile/` as a repository during Milestone 1 inspection.

## Clean-Room Reference Policy

The surrounding private research workspace contains reference checkouts of GNU Zile and kg. Rile should use them only for behavior and architecture lessons unless license implications are explicitly documented.

See `NOTICE.md` for the current third-party code status.
