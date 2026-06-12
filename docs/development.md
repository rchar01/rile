<!--
SPDX-FileCopyrightText: 2026 Rile contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Development Notes

## Repository Scope

`rile/` is the implementation project and intended distributable repository. The parent `rile-lab/` workspace is private research context and is not part of the Rile crate.

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
- movement commands support character, line, beginning-of-line, and end-of-line motion;
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

Current limitations: there is no scrolling, no prompt cursor movement, no file-name completion, no unsaved-changes quit confirmation, and no undo wiring yet.

## Line Ending Policy

The in-memory buffer model uses `\n` as the only line separator. `Buffer::from_text` splits on `\n`, `Buffer::serialize` joins lines with `\n`, and `Buffer::final_newline` records whether the serialized text ends with a final newline.

The current file policy preserves carriage return bytes as ordinary text. CRLF files therefore round-trip as CRLF when saved without editing those line endings, while newly inserted line breaks use `\n`. A later polishing milestone can add explicit line-ending detection and conversion controls if needed.

## Save Safety

Saves use a same-directory temporary file followed by `rename`, then best-effort parent-directory sync. This is intended to avoid partially written target files on common Unix filesystems. Permission, directory, and missing-parent errors propagate as `I/O error` values and failed saves keep the buffer dirty.

## Terminal Decision

Rile currently uses direct Unix termios and ANSI escape sequences with only the `libc` crate for platform bindings. This keeps behavior explicit and dependency count low while the terminal model is still small. A higher-level terminal crate can be reconsidered later if portability or feature needs outweigh the extra abstraction.

## License

Rile is licensed as `GPL-3.0-or-later`. Keep `COPYING` as the canonical GPLv3 license text and add SPDX identifiers to new source and documentation files.

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

The dev container provides:

| Tool | Purpose | Required |
| --- | --- | --- |
| `rustup` | Toolchain and component management. | Yes |
| `cargo` | Build, test, run, and package commands. | Yes |
| `rustfmt` | Rust formatting checks. | Yes |
| `clippy` | Rust lint checks. | Yes |
| `rust-analyzer` | Editor/LSP support. | Useful, not part of `verify` |
| `cargo-nextest` | Preferred test runner. | Yes, with `cargo test` fallback in `scripts/test` |
| `cargo-deny` | License, advisory, source, and dependency policy checks. | Yes |
| `cargo-audit` | Security advisory checks. | Yes |
| `cargo-machete` | Unused dependency detection. | Yes |

Current host status in this workspace: `cargo`, `podman`, and `make` are available; `rustup`, `rustfmt`, clippy, `rust-analyzer`, `cargo-nextest`, `cargo-deny`, `cargo-audit`, and `cargo-machete` are not. That is why the dev container is the canonical tooling environment.

## Dev Container Workflow

The dev image is defined in `Containerfile.dev`. It intentionally uses that name instead of plain `Containerfile` so tooling images are not confused with future runtime images.

Interactive development:

```sh
make shell
```

One-shot tasks:

```sh
make build
make fmt
make fmt-check
make test
make lint
make audit
make unused-deps
make verify
```

The Makefile delegates to scripts:

- `scripts/devshell` opens an interactive shell in the dev container.
- `scripts/in-container` runs one command in a fresh dev container.
- `scripts/build` runs `cargo build --locked`.
- `scripts/fmt` runs `cargo fmt` and updates Rust source formatting.
- `scripts/fmt-check` runs `cargo fmt --check` without modifying files.
- `scripts/test` runs `cargo nextest run --locked` when available, otherwise `cargo test --locked`.
- `scripts/test-cargo` always runs `cargo test --locked`.
- `scripts/lint` runs `scripts/fmt-check` and `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `scripts/audit` runs `cargo deny check` and `cargo audit`.
- `scripts/unused-deps` runs `cargo machete`.
- `scripts/verify` runs build, test, lint, audit, and unused dependency checks.
- `scripts/tools` prints the versions of expected tools.

`cargo-deny` reads policy from `deny.toml`. The current policy denies yanked crates, denies unknown registries and git sources, denies wildcard dependencies, warns on multiple dependency versions, and allows Rile's GPL license plus the permissive licenses used by current dependencies.

## Direct Host Workflow

Direct host development is supported if the same tools are installed locally. Use scripts directly:

```sh
./scripts/build
./scripts/fmt
./scripts/fmt-check
./scripts/test-cargo
./scripts/lint
./scripts/audit
./scripts/unused-deps
```

On this host, only `./scripts/build` and `./scripts/test-cargo` are expected to work until the missing Rust components and cargo subcommands are installed.

## CI Status

CI is deferred until `rile/` is initialized as a hosted Git repository. Milestone 1 inspection could not read Git status or remotes from `rile/` in this workspace. Future CI should call the same scripts used by `make verify`; it should not call `scripts/devshell`.
