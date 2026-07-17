<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# AGENTS.md

## Repository Scope

- `rile/` is the public implementation repository; parent `rile-lab/` is private research context and is not part of the crate.
- Official repository: <https://codeberg.org/rch/rile>.
- Rile is `GPL-3.0-or-later`; add SPDX headers to new source and documentation files.
- Do not copy, translate, mechanically port, or vendor reference editor code. Zile and kg tooling under `tools/reference/` is behavior evidence only; update `NOTICE.md` before any intentional third-party code import.

## Agent Workflow Expectations

- Read relevant code before editing.
- Prefer minimal changes that match existing patterns.
- Keep `README.md`, `AGENTS.md`, and skill docs current when repository behavior changes.
- If your runtime provides specialized tools or subagents for codebase exploration, use them when the repository structure, ownership boundaries, or relevant files are unclear.
- If your runtime provides specialized tools or subagents for verification, use them for non-trivial test runs, runtime-backed checks, or command-heavy validation.
- If your runtime provides specialized tools or subagents for review, use them after substantial edits to catch regressions, missing updates, or doc/code drift.
- If your runtime provides specialized tools or subagents for research, use them when behavior depends on external tooling or upstream docs.
- Prefer local repository docs, scripts, and configuration first; use web research when local sources are insufficient or freshness matters.
- Summarize any specialist-tool or subagent findings you rely on.
- Do not revert unrelated worktree changes.

## Optional Private Workspace Context

- When this repository is checked out inside a parent `rile-lab/` workspace, that
  parent may contain private planning notes, reference captures, and upstream
  research material.
- Use parent-workspace context only when the user explicitly grants access or the
  active workspace already includes it.
- Treat `rile/` source, tests, scripts, and documentation as authoritative for
  implementation decisions.
- Do not copy, translate, or vendor code from private reference directories into
  `rile/`; use them only as behavioral evidence.

## First Files To Read

- Start with `README.md`, `docs/development.md`, `docs/testing.md`, `Makefile`, `scripts/verify`, `Cargo.toml`, `deny.toml`, and `.gitmessage`.
- If behavior involves terminal rendering or input, inspect representative files in `src/terminal/`, `src/input.rs`, `src/editor.rs`, and `tests/support/pty.rs` before editing.
- Trust executable sources over prose when they conflict: scripts, `Makefile`, Cargo config, tests, and container files are the source of truth.

## Commands

- Preferred workflow uses Podman and `make`; the host only needs `podman` and `make`.
- Full quality gate: `make verify`. It runs build, tests, parsed-screen snapshots, fmt/clippy, audit/license checks, and unused dependency checks inside the dev container.
- Formatting: `make fmt`; check-only formatting is included in `make lint` and `make verify`.
- Focused test: `./scripts/in-container cargo test --locked <filter>`.
- Focused PTY target: `./scripts/in-container cargo test --locked --test <pty_target> <filter>`.
- Snapshot check: `make snapshot-test`. Do not update snapshots during verification.
- Intentional snapshot update: `./scripts/in-container env INSTA_UPDATE=always RILE_SNAPSHOT_TEST=1 cargo test --locked --test pty_snapshots`, then `make snapshot-test`.
- Optional visual review: `make visual-demos` or `make visual-frames`; these are not part of `make verify`.
- Interactive dev shell: `make shell`; it sets Podman's detach keys to `Ctrl-]` so `C-p` reaches Rile.

## Testing Notes

- Prefer unit tests beside modules for editor, buffer, keymap, render, syntax, file, and config logic that does not require terminal I/O.
- Put terminal behavior in `tests/pty_*.rs`; PTY tests spawn the real binary, parse `vt100` screen state, and assert visible output/cursor state.
- PTY cursor positions are zero-based terminal coordinates, not one-based mode-line coordinates.
- Use `--visual-test` and `--test-size WIDTHxHEIGHT` for deterministic terminal behavior in PTY, snapshot, and VHS scenarios.
- Generated `artifacts/` and `target/` are ignored; visual GIF/PNG outputs are review evidence, not correctness gates.

## Architecture Notes

- `src/main.rs` delegates CLI parsing to `src/app.rs`, then starts terminal editing through `src/terminal/`.
- `src/editor.rs` owns interactive editor state and command behavior; `src/command.rs` maps exact command names; `src/keymap.rs` maps key sequences.
- `src/buffer/` and `src/file.rs` handle UTF-8 text storage and file-backed documents; saves use same-directory temp files and `rename`.
- `src/render/` centralizes face spans and priority merging used by region, search, query-replace, syntax, mode-line, and minibuffer rendering.
- `src/window.rs` manages split layout and per-window viewport state; terminal drawing must keep point visible in the selected window.
- `src/shell.rs` owns bounded nonblocking shell jobs; `TerminalSession` polls and
  cancels them while `Editor` retains only logical foreground command targets.

## Documentation And History

- Keep `README.md` current for user-visible behavior and commands.
- Keep `docs/development.md` current for implementation milestones, limits, and workflow details.
- Keep `docs/testing.md` current for test layers, snapshot workflow, and visual-review rules.
- Maintain user-visible release notes in `NEWS` and GNU-style file-level history in `ChangeLog`.

## Commit Style

- Follow `.gitmessage`: Conventional Commit header, imperative subject, capitalized subject text, no trailing period.
- Keep the full header at most 68 characters and body lines wrapped at 72 characters.
- One commit should be one logical change.
