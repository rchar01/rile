<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Self-Documentation Architecture

Rile's self-documentation is implemented as typed metadata that is used by the
editor itself. User-visible commands, key bindings, options, modes, buffer
state, and runtime metadata should be inspectable from inside Rile without
maintaining separate hand-written command tables.

The canonical quality gate for this area is `make verify`.

## Source Of Truth

The source of truth is executable Rust metadata, not prose:

- `src/command.rs` owns command IDs, names, categories, summaries, full docs,
  handlers, completion metadata, and help command registration.
- `src/keymap.rs` owns global and special-buffer keymaps, active-stack
  resolution semantics, prefix handling, and binding-shadowing metadata.
- `src/option.rs` owns supported config keys, option types, defaults,
  validation, completion metadata, and help text.
- `src/mode.rs` owns major, syntax, minor, and special-buffer mode metadata.
- `src/editor.rs` renders help buffers, describes active editor state, dispatches
  commands, and refreshes special buffers such as `*Messages*`.
- `src/completion.rs` and `src/minibuffer.rs` connect command, option, file, and
  buffer completion sources to prompts.

README and maintainer docs should summarize behavior and point to these sources.
They should not duplicate exhaustive command, key, option, or mode tables that
Rile can generate from live metadata.

## Commands

Interactive commands use stable typed IDs and stable user-facing names. The
command registry backs:

- `M-x` command lookup and execution.
- command-name completion and annotations.
- command dispatch through registered handlers.
- `C-h f` / `describe-function` command help.
- command metadata coverage tests.

Rile keeps Emacs-compatible names when behavior matches familiar editor
commands. For example, `describe-function` remains the user-facing command for
registered interactive commands. Behavior that intentionally differs can use a
clearer Rile command name.

Command categories are a fixed `CommandCategory` enum so help labels and tests
use exhaustive, compile-time-checked metadata.

## Keymaps

Keymaps are named, typed metadata. `KeyMapStack` resolves bindings from active
maps in priority order:

- higher-priority exact command bindings win;
- higher-priority prefixes block lower-priority exact command bindings;
- lower-priority maps are used only when higher-priority maps have no match.

Special buffers use local maps instead of hard-coded input bypasses. For
example, `*Help*`, `*Messages*`, `*Buffer List*`, and shell-output buffers bind
their local `q` behavior through keymaps.

Help commands consume the same keymap stack as normal dispatch:

- `C-h k` / `describe-key` reads a complete key sequence and opens full help.
- `C-h c` / `describe-key-briefly` reports the command in the echo area.
- `C-h b` / `describe-bindings` lists active bindings and marks shadowed rows.
- prefix help lists bindings that continue the active prefix.

## Options

Configuration parsing is backed by the option registry. Each supported config
key has an `OptionSpec` with a name, summary, full documentation, type, default
value, valid values, and validator.

The option registry backs:

- config defaults;
- config parsing and validation;
- option-name completion for `C-h v`;
- `describe-variable` help with current and default values.

Option names intentionally remain the existing TOML-style `snake_case` config
keys. Hyphenated aliases should not be added unless a concrete compatibility
need appears.

## Modes And Buffers

Mode metadata is backed by the mode registry. Active modes are derived from
existing editor state:

- major mode from the selected buffer path;
- syntax mode from the major mode;
- special-buffer mode from `DocumentKind`;
- minor modes from editor settings such as line numbers, syntax highlighting,
  and search highlighting.

`C-h m` / `describe-mode` renders the active mode stack. `M-x describe-buffer`
renders typed buffer state, including name, path, kind, modified state,
read-only state, point, encoding, line-ending policy, final-newline state, and
active modes. Buffer names and paths are escaped before help construction so
filesystem control characters remain visible data rather than help structure.

Special-buffer modes are exposed through `describe-mode`, not by replacing the
normal mode-line major-mode slot. The mode line continues to show the derived
major mode.

## Help And Messages Buffers

Help output is displayed through the read-only `*Help*` special buffer. Generated
prose is wrapped near terminal-friendly widths while preserving tables and
preformatted sections.

`C-h e` / `view-echo-area-messages` opens the read-only `*Messages*` buffer with
the bounded recent history of minibuffer status and error messages. The history
retains the newest 1,000 entries within a 1 MiB UTF-8 payload budget. If the
buffer already exists, redraw refreshes it from the current message history so
new messages appear while it is visible. The local messages keymap binds `q` to
restore the previous buffer.

`about-rile` / `C-h C-a` renders editor-level runtime metadata through the same
help-buffer path. It reports version, build profile, feature-reporting status,
terminal backend, default config path, current directory, and diagnostic
guidance. Config paths and current directories use the same source-level
control escaping as buffer descriptions. Rile does not maintain a separate
diagnostics registry yet; actionable runtime diagnostics are the echo-area
status and error messages reviewable with `C-h e`.

## Test Coverage

Tests enforce that metadata does not silently drift:

- command registry tests reject duplicate IDs and names, missing docs, and
  missing handlers;
- default key-binding tests reject commands without registry entries;
- keymap tests cover active-stack priority, prefixes, and shadowed bindings;
- option registry tests cover defaults, parsing, validation, completion, and
  `describe-variable` output;
- mode registry tests cover metadata completeness and active mode discovery;
- help rendering tests cover command, key, option, mode, buffer, messages, and
  about output;
- PTY tests cover representative real-terminal help flows.

Run focused tests when changing one metadata area, then run `make verify` before
committing or merging self-documentation changes.
