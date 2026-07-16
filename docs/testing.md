<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Testing Guide

Rile uses automated Rust tests, real-terminal PTY integration tests, parsed VT100
snapshots, and optional VHS visual review. Automated PTY and snapshot tests are
the correctness gates. GIF and PNG outputs are review evidence only. See
[External Projects](external-projects.md) for links to the testing, visual, and
reference tools named here.

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
For example, `tests/source_architecture.rs` guards that Rile's built-in regexp
engine type stays private to `src/search_pattern.rs` and flags common external
regex crate dependencies for intentional review.

Regexp unit tests also count test-only VM thread steps for hostile optional,
ambiguous-alternative, and repeated-atom patterns. They assert work stays within
the compiled instruction count times the line's character slots, avoiding
flaky wall-clock thresholds. A representative PTY regression sends a failing
optional-chain regexp through incremental search and confirms redraw and later
input remain responsive.

Sentence movement unit tests cover UTF-8 byte positions, formfeed paragraph
separators, retained CRLF behavior, and saturated numeric arguments that must
stop at buffer edges. PTY coverage exercises the default `M-a` and `M-e` paths.

Paragraph filling unit tests verify that edits and undo records contain only the
affected paragraph range, including multi-paragraph undo and redo, no-op fills,
large collections of short words, retained carriage returns, and UTF-8 cursor
mapping. PTY coverage exercises the default `M-q` path and configured fill
columns.

Character-transposition unit tests cover combining grapheme clusters, saturated
positive and negative arguments, and range-local edit and undo data on a long
line. Buffer tests preserve newline-free insertion and final-newline behavior.
PTY coverage exercises real `C-t` dispatch and combining-grapheme undo and redo.

Input parser unit tests bound incomplete numeric CSI parameter streams and use a
reader that fails if decoding requests bytes beyond the limit. PTY coverage
sends an overlong numeric CSI prefix without a terminator and confirms the
editor falls back, processes the retained text, and remains responsive.

File-completion unit tests use synthetic iterators to verify raw directory
results, including errors, stop at the scan limit and that bounded top-ranked
retention preserves score, directory, and lexical ordering. Filesystem-backed
tests cover every matching mode and exact directory fallback. PTY coverage
confirms a candidate-overflow directory remains responsive and visibly marks
its results as partial in vertical, ido, and completions-buffer styles.

Terminal projection unit tests count inspected source characters rather than
asserting wall-clock timing. Million-character ASCII and pathological
zero-width lines verify bounded normal-row projection. Differential cases check
tabs, C0/C1 escapes, wide and combining characters, source spans, and hidden
edge flags against the prior full-line pipeline below the budget. A direct
budget-exhaustion regression verifies a right marker replaces trailing
zero-width characters without exceeding the viewport. Existing PTY scrolling
tests cover visible horizontal markers and cursor placement.

Shell-runner unit tests use small injected output budgets and deadlines to cover
the exact combined-output boundary, infinite producers, UTF-8 byte boundaries,
silent-command timeout, ordinary descendant cleanup, early stdin closure, and
simultaneous multi-megabyte stdin/stdout transfer. Synthetic always-ready pipes
verify the deadline is checked inside drain loops. Editor coverage verifies the
production 8 MiB failure leaves prefix insertion and region replacement targets
unchanged. A PTY regression sends a 2 MiB region through `M-|` and `cat`,
exceeding normal pipe capacity so sequential stdin/output handling would
deadlock.

Buffer-manager tests verify generated special buffers use document-kind identity
and cannot replace normal files with colliding names. Terminal and PTY regressions
open a normal file named `*Messages*`, exercise redraw and `C-h e`, restore the
file, and confirm unsaved edits can still be written to its original path.
Minibuffer unit tests verify message retention evicts the oldest entries at both
the count and byte limits and truncates oversized messages at valid UTF-8
boundaries. Editor and terminal tests verify visible message buffers refresh
after history changes while hidden buffers defer materialization until reopened.
Editor regressions cover visibility in a non-selected split and killing then
recreating the generated buffer.

Auto-revert editor tests combine a failed binary-file reload, a successful
reload, and an unrelated dirty buffer in one poll to verify per-buffer error
isolation and duplicate-error suppression. PTY coverage changes a watched file
to binary contents and confirms the minibuffer reports the failure while the
real editor remains interactive.

PTY integration tests live under `tests/pty_*.rs`. They spawn the compiled `rile`
binary in a pseudo-terminal with `expectrl`, send real key input, parse terminal
output with `vt100`, and assert screen contents, cursor position, status text,
scrolling, splits, and save behavior. The harness also retains raw output for
narrow security assertions that untrusted terminal control sequences were not
emitted.

Parsed-screen snapshots live under `tests/snapshots/`. They are generated from normalized VT100 screen state, not raw ANSI bytes. Snapshots include terminal size, cursor position, visible rows, and a caret marker.

Optional visual demos live under `demos/*.tape`. They run Rile through VHS and write ignored GIF and PNG artifacts under `artifacts/` for human or multimodal review.

Optional performance smoke tests run through `make perf-smoke`. They compare
Rile, GNU Emacs, GNU Zile, kg, and Debian `vi` on generated large-file and
long-line fixtures, including explicit redraw-at-column-zero timing. They write
ignored timing evidence under `artifacts/perf/` and are documented in
`docs/performance.md`.

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
`M-x describe-buffer`, `C-h e`, and prefix help. Hostile metadata regressions
also verify that buffer names, file paths, config paths, and working directories
cannot inject structured help rows, and that raw PTY output excludes embedded
terminal control sequences.

## Deterministic Terminal Mode

Use `--visual-test` to make terminal output deterministic for PTY tests and VHS demos. Visual-test mode uses default configuration rather than user config and renders a verbose mode line with stable test-oriented state.

Use `--test-size WIDTHxHEIGHT` to render with a fixed terminal size instead of querying the host terminal. Test sizes are passed as columns by rows, for example `--test-size 80x24`.

The PTY harness also uses a temporary `HOME`, `TERM=xterm-256color`, and `NO_COLOR=1` to avoid local configuration and environment drift.

## PTY Test Rules

Keep PTY tests focused on terminal integration and user-visible rendering. Do not move ordinary unit-test coverage into PTY tests unless the behavior depends on real terminal I/O, raw mode, cursor placement, splits, scrolling, or visible status/minibuffer output.

PTY cursor assertions use zero-based terminal coordinates from `vt100::Screen::cursor_position()`, not one-based user-facing mode-line coordinates. For single-width fixture text at viewport origin, logical buffer line and display column map directly to terminal row and column. If a viewport has scrolled or a split pane starts below or to the right of the terminal origin, add the viewport row or column offset before comparing with the parsed terminal cursor.

Failure output should stay readable without general raw ANSI inspection. Prefer
parsed-screen assertions; inspect retained raw bytes only when the emitted
control sequence itself is the behavior under test. The harness should include
the scenario action, expected and actual cursor positions when relevant,
normalized visible rows, and a caret marker.

File-prompt security tests cover C0 and C1 controls, OSC sequences terminated by
BEL and string terminators, and the shared `find-file`, `find-file-read-only`,
and `insert-file` display boundary. Unix coverage also verifies that invalid
filename bytes are emitted only through the lossy UTF-8 replacement character;
it does not treat those lossy prompt paths as round-trippable filesystem names.
Narrow control-escape clipping remains a terminal unit test because its exact
viewport boundary is more deterministic below the PTY layer.

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
