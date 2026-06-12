<!--
SPDX-FileCopyrightText: 2026 Rile contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Rile

Rile Is Lightweight Emacs.

Rile is planned as a small, fast, terminal-native, Emacs-style text editor written in Rust. The v1 goal is practical daily editing of source files, config files, Markdown, and normal UTF-8 text.

## Status

Milestone 6 minibuffer prompting is implemented. The editor can insert text, move the cursor, delete text, save, quit, run exact-name `M-x` commands, and open files with `C-x C-f`.

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
- `C-x C-s` saves the current file.
- `C-x C-f` prompts for a file path and opens it.
- `C-x C-c` quits.
- `M-x` runs an exact command name.
- `C-g` cancels minibuffer prompts and prefix keys.

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
