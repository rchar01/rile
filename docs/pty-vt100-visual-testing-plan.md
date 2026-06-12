<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: PTY, VT100, Snapshot, and VHS Testing

## Goal

Add a deterministic terminal-integration testing system for Rile that lets automated tests and human reviewers verify real terminal behavior.

The automated path must spawn the real `rile` binary in a pseudo-terminal, send real input, parse terminal output with a VT100 model, and assert screen contents, cursor position, mode-line text, splits, and scrolling. The visual path must generate reproducible VHS GIFs or videos for UX review without treating GIF review as the correctness oracle.

## Scope

- Add deterministic CLI/test-mode support to Rile.
- Add integration-test support modules under `tests/support/`.
- Add fixture files under `fixtures/visual/`.
- Add PTY tests for open, movement, insert, save, status line, scrolling, splits, and terminal size behavior.
- Add parsed-screen `insta` snapshots.
- Add optional VHS tapes under `demos/` and generated artifacts under `artifacts/`.
- Add documentation and Makefile/script targets for structured PTY tests and optional visual review.

## Non-Goals

- Do not make VHS or GIF review part of the default correctness gate.
- Do not snapshot raw ANSI escape output.
- Do not move existing unit-test coverage into PTY tests.
- Do not use PTY tests for every editor command; reserve them for terminal integration and user-visible rendering.
- Do not implement compatibility testing for nano, Zile, or other editors in the first Rile test harness pass. The support layer should be designed so that future adapter-style comparison tests are possible.

## Current Context

- `src/app.rs` currently parses only `rile [file]`, `--help`, and `--version`; new flags require parser changes.
- `src/main.rs` passes only `options.file.as_deref()` to `terminal::run_basic_editor`; test mode requires passing richer options.
- `src/terminal/mod.rs` reads terminal size through `ioctl(TIOCGWINSZ)` in `TerminalSession::draw`; deterministic `--test-size` needs an override path.
- The renderer already has testable `draw_editor_frame`, `TerminalSize`, cursor-position extraction tests, mode-line position tests, and scroll-to-cursor behavior.
- Rile already supports splits, buffer switching, search, query replace, syntax highlighting, config, binary-file detection, and optional `backup_on_save`.
- There is no existing `tests/` integration-test tree, no `fixtures/`, no `demos/`, no `artifacts/`, and no snapshot workflow.
- Current canonical verification is `make verify`, backed by `scripts/verify` in the dev container.
- Proposed crate versions are available: `expectrl = 0.9.0`, `vt100 = 0.16.2`, `insta = 1.48.0`, `anyhow = 1.0.102`, `tempfile = 3.27.0`, `assert_cmd = 2.2.2`, and `predicates = 3.1.4`.

## Assumptions

- PTY tests will run on Unix-like systems only at first, matching Rile's current Unix terminal implementation.
- `vt100::Screen::cursor_position()` should be treated as the source of truth for terminal cursor coordinates, but Phase 1 must confirm whether it reports zero-based or one-based coordinates.
- Visual test mode should preserve normal editor semantics while making status text, screen dimensions, and debug display deterministic.
- VHS is optional local tooling and should not be required by `cargo test`, `make test`, or default CI.
- Snapshot tests must normalize trailing whitespace and include cursor metadata plus a caret marker.

## Open Questions

- [ ] Should `--visual-test` imply a specific theme and line-number setting, or should it preserve user config except for nondeterministic fields?
- [ ] Should PTY tests ignore user config by forcing a temporary `HOME`, or should Rile add an explicit `--no-config` test flag?
- [ ] Should `--test-size 80x24` be accepted only with `--visual-test`, or should it be a general developer/debug flag?
- [ ] Should split visual labels use internal `WindowId` values or deterministic left/right labels computed from layout order?
- [ ] Should snapshot files be committed immediately, or introduced after the first snapshot format stabilizes?

## Design Decisions To Validate Early

- Prefer `expectrl` first, because it matches the guide and is purpose-built for PTY interaction.
- Keep a fallback decision point for `portable-pty` if `expectrl` read/drain APIs make reliable VT100 parsing awkward.
- Add `--visual-test` and `--test-size WIDTHxHEIGHT` as first-class CLI options rather than test-only environment variables, because VHS needs to invoke the binary directly.
- Use parsed screen dumps for snapshots, not ANSI byte streams.
- Keep PTY failure output verbose: scenario, last action, expected/actual cursor, status line, full visible screen, and caret marker.

## Phase 1: CLI and Deterministic Render Mode

Goal: Make the real Rile binary deterministic enough for PTY and VHS runs.

Tasks:

- [x] Extend `app::CliOptions` with `visual_test: bool` and `test_size: Option<TerminalSize>` or a small CLI-owned size type. Evidence: `app::CliOptions` now carries `visual_test` and `test_size`.
- [x] Parse `--visual-test` and `--test-size WIDTHxHEIGHT` before the optional file argument. Evidence: `app::parse_args` handles `--visual-test`, `--test-size WIDTHxHEIGHT`, and `--test-size=WIDTHxHEIGHT`.
- [x] Update `usage()` and CLI unit tests for the new flags and invalid sizes. Evidence: app tests cover visual flags, equals-form size parsing, and invalid sizes.
- [x] Change `main.rs` and `terminal::run_basic_editor` to pass editor runtime options instead of only `Option<&Path>`. Evidence: `terminal::RuntimeOptions` carries file, visual-test, and test-size settings.
- [x] Add a terminal-size override so `TerminalSession::draw` uses `--test-size` when present and `ioctl` otherwise. Evidence: `TerminalSession` stores `test_size` and falls back to `terminal_size` only when unset.
- [x] Add a visual-test editor/render option that makes status text deterministic and visibly marks test mode. Evidence: `FrameOptions { visual_test }` renders `Rile VISUAL` mode-line text with window id, active state, file name, line, column, dirty flag, and position.
- [x] Decide whether visual-test mode ignores user config or forces a temporary config-free mode. Evidence: visual-test startup uses `Config::default()` instead of loading user config.

Validation gate:

- [x] `cargo run -- --help` documents both flags. Evidence: `make run ARGS='--help'` prints both flags.
- [ ] `cargo run -- --visual-test --test-size 80x24 fixtures/visual/numbered.txt` runs in an interactive terminal.
- [x] Existing unit tests continue to pass. Evidence: `make verify` passed after Phase 1.

## Phase 2: Fixtures and Harness Skeleton

Goal: Add stable inputs and a small support API before writing scenario tests.

Tasks:

- [x] Add `fixtures/visual/numbered.txt` with 20 numbered rows and repeated digit columns. Evidence: fixture file exists.
- [x] Add `fixtures/visual/wide.txt` with ASCII, accented, Greek, CJK, emoji, and mixed-width rows. Evidence: fixture file exists.
- [x] Add `fixtures/visual/long_lines.txt` for horizontal scrolling and clipping. Evidence: fixture file exists.
- [x] Add `fixtures/visual/split_left.txt` and `fixtures/visual/split_right.txt` for split-pane demos and tests. Evidence: fixture files exist.
- [x] Add `tests/support/mod.rs` and expose support modules. Evidence: `tests/support/mod.rs` exposes fixture, key, screen, and PTY helpers.
- [x] Add `tests/support/keys.rs` with common escape sequences and Emacs control-key helpers. Evidence: common control, meta, arrow, enter, backspace, and delete sequences are defined.
- [x] Add `tests/support/screen.rs` with screen dump normalization and caret rendering helpers. Evidence: parsed `vt100::Screen` text and dumps are normalized with cursor carets.
- [x] Add `tests/support/fixtures.rs` with fixture loaders and temporary-file helpers. Evidence: visual fixture lookup, fixture loading, temporary files, and temporary HOME helpers are available.

Validation gate:

- [x] `cargo test` compiles the empty integration-test support modules. Evidence: `cargo test --test pty_open` compiles support modules.
- [ ] Fixture line endings and Unicode contents are documented and stable.

## Phase 3: PTY Harness

Goal: Spawn the compiled Rile binary in a real PTY and expose readable assertion helpers.

Tasks:

- [x] Add dev dependencies: `anyhow`, `expectrl`, `vt100`, `insta`, `tempfile`, `assert_cmd`, and `predicates`. Evidence: dependencies are listed in `Cargo.toml` and locked in `Cargo.lock`.
- [x] Add `tests/support/pty.rs` with `RilePty` owning an `expectrl` session, `vt100::Parser`, `TempDir`, file path, rows, and columns. Evidence: `RilePty` owns the session, parser, temporary HOME, file path, rows, and columns; file path is passed through `Command` args instead of shell quoting.
- [x] Force deterministic environment variables in the harness, including `TERM`, `NO_COLOR` if useful, and a temporary `HOME` or test config path. Evidence: harness sets `TERM=xterm-256color`, `NO_COLOR=1`, and a temporary `HOME`.
- [x] Start Rile with `--visual-test --test-size WIDTHxHEIGHT` and a shell-safe path. Evidence: harness uses `std::process::Command` with separate args.
- [x] Implement `send`, `drain_for`, `wait_for_screen_contains`, `assert_screen_contains`, `assert_status_contains`, `cursor_position`, `assert_cursor`, `snapshot_text`, and `quit`. Evidence: these methods exist on `RilePty`.
- [x] Include the last action name in failure messages. Evidence: `RilePty` assertion failures include `last_action` and screen dumps.
- [ ] Confirm `expectrl` read APIs are reliable enough; if not, record a decision to switch the internals to `portable-pty` while preserving the public `RilePty` API.

Validation gate:

- [x] A smoke test can open a temp file, parse the visible screen, find the file name, and quit cleanly. Evidence: `cargo test --test pty_open` passes.
- [ ] A deliberately wrong cursor assertion prints a readable screen dump during local development.

## Phase 4: First Structured Tests

Goal: Prove real-terminal open, movement, insert, save, and status behavior with small tests.

Tasks:

- [x] Add `tests/pty_open.rs` with an open-file assertion and first parsed-screen snapshot. Evidence: `opens_visual_fixture_in_pty` asserts parsed screen contents, status text, and cursor position.
- [ ] Add `tests/pty_movement.rs` for `C-f`, `C-b`, `C-n`, `C-p`, arrow keys, `C-a`, `C-e`, `M-f`, and `M-b` where stable.
- [ ] Add `tests/pty_insert.rs` for printable ASCII, UTF-8 text, Enter, Backspace, and Delete.
- [ ] Add `tests/pty_save.rs` for modification and `C-x C-s`, including verifying file contents on disk.
- [ ] Add `tests/pty_statusline.rs` for clean/dirty state, save state, line/column changes, visual-test marker, and error messages.
- [ ] Keep expected cursor positions in terminal coordinates and document the conversion from logical buffer position to terminal position.

Validation gate:

- [x] `cargo test --test pty_open` passes locally. Evidence: command passed on 2026-06-12.
- [ ] `cargo test --test pty_movement` passes locally.
- [ ] Snapshot failure output is understandable without reading raw ANSI bytes.

## Phase 5: Scrolling, Resize, and Splits

Goal: Cover the areas most likely to regress visually in terminal use.

Tasks:

- [ ] Add `tests/pty_scrolling.rs` with vertical cursor visibility after repeated `C-n` and `C-p`.
- [ ] Add horizontal scrolling tests using `long_lines.txt` and a narrow test size.
- [ ] Add `tests/pty_resize.rs` using `--test-size` for small and narrow terminal snapshots.
- [ ] Add `tests/pty_split.rs` for `C-x 3`, `C-x 2`, `C-x o`, active pane marker, cursor-in-active-pane, and separator rendering.
- [ ] Add split snapshot tests only after visual-test pane labels are deterministic.

Validation gate:

- [ ] Cursor remains inside the parsed VT100 screen after scrolling and split navigation.
- [ ] Mode-line/status text remains visible in small terminal sizes.
- [ ] Split separators do not corrupt buffer text in snapshots.

## Phase 6: Snapshot Workflow

Goal: Make snapshot review useful and safe for agents and humans.

Tasks:

- [ ] Store snapshots under `tests/snapshots/` using `insta` defaults.
- [ ] Add a documented `cargo insta test` workflow.
- [ ] Add a `scripts/snapshot-test` or Makefile target if the workflow proves useful.
- [ ] Add snapshot naming rules that include scenario names and terminal size.
- [ ] Ensure snapshots avoid local temp paths by displaying only file names or normalized paths in visual-test mode.

Validation gate:

- [ ] `cargo insta test` passes with committed snapshots.
- [ ] Updating snapshots requires an intentional review step, not an automatic `make verify` side effect.

## Phase 7: VHS Visual Review

Goal: Add optional reproducible demos for humans and multimodal LLMs.

Tasks:

- [x] Add `demos/movement.tape` using `--visual-test --test-size 80x24 fixtures/visual/numbered.txt`.
- [x] Add `demos/split-pane.tape` using split and pane-switching commands.
- [ ] Add `demos/open-edit-save.tape`, `demos/search.tape`, and `demos/resize.tape` after the core demos work.
- [x] Add `artifacts/` to `.gitignore` unless artifacts are explicitly requested for distribution.
- [x] Add documentation that VHS output is review evidence only, not the pass/fail oracle.
- [x] Add a visual review checklist covering cursor visibility, status-line consistency, split separators, active pane clarity, scrolling, minibuffer readability, and clean quit.

Visual review checklist:

- The cursor remains visible after movement, split changes, prompts, and clean quit.
- Mode lines show stable visual-test state, buffer names, active-window state, position, and dirty marker.
- Split separators are drawn consistently and do not overwrite buffer text.
- The active pane is clear after `C-x o`.
- Scrolling and horizontal clipping avoid stale text.
- Minibuffer prompts are readable while opening files and quitting.

Validation gate:

- [ ] `vhs demos/movement.tape` generates `artifacts/movement.gif` on a machine with VHS installed.
- [ ] `vhs demos/split-pane.tape` generates `artifacts/split-pane.gif` on a machine with VHS installed.

## Phase 8: CI and Developer Workflow

Goal: Integrate structured tests into normal verification without making visual tooling mandatory.

Tasks:

- [ ] Add PTY tests to normal `cargo test` and `make test` once stable.
- [ ] Add `cargo insta test` to `make verify` only after snapshot churn is low and snapshots are committed.
- [x] Keep VHS out of default `make verify`.
- [x] Add optional `make demos` or `make visual-demos` target that checks for `vhs` and writes to `artifacts/`.
- [ ] Document optional CI artifact generation for hosted CI after Codeberg CI is configured.

Validation gate:

- [x] `make verify` passes on the dev container without requiring VHS.
- [x] Optional visual demo generation fails clearly when VHS is missing.

## Risks

- PTY timing can be flaky if tests rely on sleeps instead of wait-for-screen conditions.
- `expectrl` APIs may differ from the skeleton; isolate all crate-specific code in `tests/support/pty.rs`.
- `C-s` can be intercepted by terminal flow control outside raw mode; PTY tests should verify Rile's raw-mode setup handles it.
- Unicode cursor behavior must distinguish byte offsets, grapheme movement, display columns, and terminal columns.
- Snapshot churn can become noisy if visual-test mode still includes local paths, theme differences, or terminal-dependent behavior.
- `--test-size` could accidentally mask real resize behavior; use it for deterministic rendering and add true PTY resize tests later only if stable.

## Validation Summary

- [x] `make fmt` passes.
- [ ] `make test` passes with PTY tests enabled.
- [x] `make verify` passes without requiring VHS.
- [ ] `cargo insta test` passes after snapshots are committed.
- [ ] At least one deliberately failed PTY assertion produces a readable screen dump during local harness validation.
- [ ] At least one VHS movement GIF is generated manually under `artifacts/`.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-06-12 | Plan created from user-provided guide and current Rile codebase inspection. | `src/app.rs`, `src/main.rs`, `src/terminal/mod.rs`, `Cargo.toml`, and `Makefile` inspected; crate availability checked with `cargo search`. |
| 2026-06-12 | Phase 1 and visual fixtures started. | Added `--visual-test`, `--test-size`, deterministic visual mode-line rendering, and `fixtures/visual/*`. |
| 2026-06-12 | PTY harness skeleton and first smoke test added. | Added `tests/support/*`, dev dependencies, and passing `tests/pty_open.rs`. |
| 2026-06-12 | Optional VHS demo infrastructure added. | Added `demos/movement.tape`, `demos/split-pane.tape`, `scripts/visual-demos`, `make visual-demos`, and ignored `artifacts/`. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-06-12 | Treat VHS as optional visual review, not a correctness gate. | Structured PTY/VT100 assertions are required for exact behavior. |
| 2026-06-12 | Add CLI flags before PTY tests. | Current CLI and terminal-size flow cannot produce deterministic PTY snapshots without `--visual-test` and `--test-size`. |
| 2026-06-12 | Visual-test mode uses default config instead of user config. | PTY and snapshot output should not depend on a developer's `~/.config/rile/config.toml`. |
