<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Testing Guide

Rile uses automated Rust tests, real-terminal PTY integration tests, parsed VT100 snapshots, and optional VHS visual review. Automated PTY and snapshot tests are the correctness gates. GIF and PNG outputs are review evidence only.

## Quick Commands

Run the full development verification suite:

```sh
make verify
```

Common targeted commands:

```sh
make test
make test-cargo
make snapshot-test
make perf-smoke
make visual-demos
make visual-frames
```

`make verify` runs inside the dev container and covers build, tests, parsed-screen snapshot checks, formatting/lints, advisory/license/dependency policy checks, and unused dependency checks. It intentionally does not run VHS visual tooling or optional performance smoke tests.

## Test Layers

Unit tests cover internal editor behavior without a terminal. They live beside Rust modules and should be preferred for buffer, editor, keymap, render, syntax, file, and configuration logic that does not require terminal I/O.

Source architecture tests live under ordinary integration tests when a repository
invariant is easier to validate from source structure than from runtime behavior.
For example, `tests/source_architecture.rs` keeps Rile's built-in regexp engine
private to `src/search_pattern.rs` and rejects adding an external regex crate
dependency without an intentional test update.

PTY integration tests live under `tests/pty_*.rs`. They spawn the compiled `rile` binary in a pseudo-terminal with `expectrl`, send real key input, parse terminal output with `vt100`, and assert screen contents, cursor position, status text, scrolling, splits, and save behavior.

Parsed-screen snapshots live under `tests/snapshots/`. They are generated from normalized VT100 screen state, not raw ANSI bytes. Snapshots include terminal size, cursor position, visible rows, and a caret marker.

Optional visual demos live under `demos/*.tape`. They run Rile through VHS and write ignored GIF and PNG artifacts under `artifacts/` for human or multimodal review.

Optional performance smoke tests run through `make perf-smoke`. They compare
Rile, GNU Emacs, GNU Zile, kg, and Debian `vi` on generated large-file and
long-line fixtures. They write ignored timing evidence under `artifacts/perf/`
and are documented in `docs/performance.md`.

## Metadata And Help Tests

Registry metadata is tested at the unit-test layer so new interactive commands,
key bindings, options, and modes cannot silently skip required names, summaries,
documentation, handlers, validation, or keymap coverage. Keep those invariant
tests close to the registry modules they protect.

Help rendering should be covered first with unit tests using stable text
fragments for command, key, option, mode, buffer, and about output. Use PTY tests
for representative end-to-end help flows that depend on real terminal input,
minibuffer prompts, local keymaps, or help-window restoration. Current PTY help
coverage includes `C-h k`, `C-h f`, `C-h v`, `C-h m`, `C-h C-a`,
`M-x describe-buffer`, `C-h e`, and prefix help.

## Deterministic Terminal Mode

Use `--visual-test` to make terminal output deterministic for PTY tests and VHS demos. Visual-test mode uses default configuration rather than user config and renders a verbose mode line with stable test-oriented state.

Use `--test-size WIDTHxHEIGHT` to render with a fixed terminal size instead of querying the host terminal. Test sizes are passed as columns by rows, for example `--test-size 80x24`.

The PTY harness also uses a temporary `HOME`, `TERM=xterm-256color`, and `NO_COLOR=1` to avoid local configuration and environment drift.

## PTY Test Rules

Keep PTY tests focused on terminal integration and user-visible rendering. Do not move ordinary unit-test coverage into PTY tests unless the behavior depends on real terminal I/O, raw mode, cursor placement, splits, scrolling, or visible status/minibuffer output.

PTY cursor assertions use zero-based terminal coordinates from `vt100::Screen::cursor_position()`, not one-based user-facing mode-line coordinates. For single-width fixture text at viewport origin, logical buffer line and display column map directly to terminal row and column. If a viewport has scrolled or a split pane starts below or to the right of the terminal origin, add the viewport row or column offset before comparing with the parsed terminal cursor.

Failure output should stay readable without raw ANSI inspection. The harness should include the scenario action, expected and actual cursor positions when relevant, normalized visible rows, and a caret marker.

## Fixtures

Visual and PTY fixtures live under `fixtures/visual/`. Keep them as UTF-8 text with LF line endings.

`wide.txt` intentionally includes accented Latin, Greek, CJK, emoji, and mixed-width text for Unicode rendering checks. `numbered.txt`, `long_lines.txt`, `split_left.txt`, and `split_right.txt` intentionally stay ASCII so cursor-column and clipping assertions remain simple and deterministic.

Do not use CRLF fixture edits for visual tests. CRLF behavior is covered separately by file tests.

## Snapshot Workflow

Check committed parsed-screen snapshots:

```sh
make snapshot-test
```

`make verify` also runs the check-only snapshot target. Verification must never update snapshots automatically.

To intentionally update snapshots:

```sh
INSTA_UPDATE=always RILE_SNAPSHOT_TEST=1 cargo test --locked --test pty_snapshots
make snapshot-test
```

The direct cargo-insta check-only equivalent is:

```sh
RILE_SNAPSHOT_TEST=1 cargo insta test --check --test pty_snapshots
```

Snapshot names should include the scenario and terminal size, such as `open_numbered_50x10`. Snapshots should avoid local temporary paths; visual-test output should display stable file names or normalized text.

## Visual Review Workflow

Generate every VHS demo:

```sh
make visual-demos
```

Generate every demo and verify named PNG frames plus any scripted file-output checks:

```sh
make visual-frames
```

Run one demo by passing `ARGS`:

```sh
make visual-frames ARGS='demos/search.tape'
```

Current demos cover movement, open/edit/save, deterministic resize rendering, incremental search, and split panes.

Visual review checklist:

- The cursor remains visible after movement, split changes, prompts, and clean quit.
- Mode lines show stable visual-test state, buffer names, active-window state, position, and dirty marker.
- Split separators are drawn consistently and do not overwrite buffer text.
- The active pane is clear after `C-x o`.
- Scrolling and horizontal clipping avoid stale text.
- Minibuffer prompts are readable while opening files, searching, saving, and quitting.

Generated GIFs and PNGs are intentionally ignored. Do not treat them as pixel-perfect snapshot tests unless a future tool explicitly adds reviewed image comparisons.

## Troubleshooting

If a PTY test is flaky, prefer waiting for expected screen text over adding blind sleeps. Keep crate-specific PTY behavior isolated in `tests/support/pty.rs` so the public test API remains stable if the backend changes.

If `C-s` does not reach Rile, verify the PTY/raw-mode path first. `C-s` can be intercepted by terminal flow control outside raw mode.

If snapshots churn unexpectedly, check for local paths, terminal-size drift, theme/config drift, cursor blink/state changes, or trailing whitespace in normalized rows.

If visual tooling fails, remember that it runs in the separate visual container from `Containerfile.visual`. The normal dev container and `make verify` should remain independent of VHS, ttyd, Chromium, and ffmpeg.
