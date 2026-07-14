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
buffers and split windows, search and literal/regexp query-replace with
highlights, run shell commands, use registers and rectangles, show help buffers,
highlight common
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

Editing mode requires an interactive terminal. `--help` and `--version` work
without one. When no file is provided, Rile opens a clean `*Rile*` welcome
buffer. When a file path is provided, Rile opens it as UTF-8 before entering raw
mode, rejects NUL-containing binary files, and shows file/dirty state, position,
and major mode in the mode line. Control characters in terminal-visible text
render as visible escapes instead of terminal control sequences.

Developer visual testing flags are available for deterministic terminal review: `--visual-test` uses deterministic defaults and a verbose visual-test mode line, while `--test-size WIDTHxHEIGHT` overrides terminal size during rendering.

## Usage

Basic editor keys:

- `C-f`/right arrow and `C-b`/left arrow move horizontally.
- `M-f` and `M-b` move forward and backward by word.
- `C-n`/down arrow and `C-p`/up arrow move vertically.
- `C-v`/PageDown and `M-v`/PageUp scroll by one visible page.
- Repeated `C-l` cycles the current line through window center, top, and bottom.
- `M-r` cycles point through the middle, top, and bottom visible window line.
- Ctrl-Left/Right move by word, Ctrl-Up/Down move by paragraph, and
  Ctrl-Home/End move to the beginning or end of the buffer when the terminal
  sends standard modified-key escape sequences.
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
  replays the latest macro. `C-u` before `C-x e` repeats macro execution. F3
  starts recording and F4 ends recording or replays the latest macro when the
  terminal sends common function-key escape sequences.
- Backspace deletes before point; `C-d`/Delete deletes at point.
- `M-d` kills the next word; `M-Backspace` kills the previous word.
- `M-l`, `M-u`, and `M-c` downcase, upcase, and capitalize words. `C-x C-l`
  and `C-x C-u` downcase and upcase the active region.
- `M-\` deletes spaces and tabs around point; with `C-u`, it deletes only
  spaces and tabs before point. `C-x C-o` deletes redundant blank lines around
  point. `M-x delete-trailing-whitespace` removes trailing spaces and tabs from
  line ends.
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
- `M-x just-one-space` collapses spaces and tabs around point to one space;
  numeric arguments choose the number of spaces, and negative arguments also
  collapse newlines.
- `C-w` kills the active region.
- `M-w` copies the active region.
- `C-y` yanks the latest kill or copy; consecutive kill commands coalesce into
  one yankable entry.
- `M-y` immediately after `C-y` or another `M-y` rotates through earlier kill-ring
  entries.
- `C-k` kills to the end of the line, or the line break at end of line.
- `C-_` undoes the latest edit in the current buffer. After an undo sequence is
  broken by another command, `C-_` can redo by undoing that undo sequence.
  `M-x undo-only` continues undo without redo, and `M-x undo-redo` redoes a
  just-undone change. Undoing back to the saved contents clears the modified
  flag.
- `C-x C-s` saves the current file.
- `C-x s` offers to save each modified file-backed buffer, skipping special,
  read-only, and unnamed buffers.
- `C-x C-v` reverts the current file-backed buffer from disk, prompting before
  discarding unsaved changes.
- `M-x auto-revert-mode` and `M-x global-auto-revert-mode` reload changed files
  while idle, but only for clean file-backed buffers.
- `C-x C-w` prompts for a file path and writes the current buffer there.
- `C-x C-f` prompts for a file path with completion, starts from the current
  buffer's directory, and opens it; Tab inserts the selected path, Enter accepts
  the selected existing path, and no-match missing-file input opens a new buffer.
  Directory candidates descend after Tab, exact input, explicit selection, or a
  typed directory prefix.
- `C-x i` prompts for a file path with completion and inserts its contents;
  Enter accepts the selected existing path.
- `C-x C-r` prompts for a file path with completion and opens it read-only;
  Enter accepts the selected existing path.
- `C-x C-q` toggles whether the current normal buffer is read-only.
- `C-x b` prompts for a buffer name with completion and switches to it,
  preserving each buffer's point; empty input switches to the default buffer,
  and Tab or Enter accepts the selected candidate.
- `C-x C-b` shows a read-only `*Buffer List*` in another window.
- `C-x k` prompts for a buffer name with completion and kills it; empty input
  kills the current buffer, Tab or Enter accepts the selected candidate, and
  buffers with unsaved changes use an Emacs-style `y-or-n-p` confirmation.
- Minibuffer prompts support `C-a`/Home, `C-e`/End, `C-f`/Right,
  `C-b`/Left, `M-f`, and `M-b` cursor movement while editing prompt text.
  `C-d`/Delete deletes after point; `C-k`, `M-d`, `M-Backspace`, and
  terminal-encoded `C-Backspace` kill prompt text into the normal kill ring.
- `C-x 2` splits the current window below.
- `C-x 3` splits the current window right.
- `C-x 0` deletes the current window.
- `C-x 1` deletes other windows.
- `C-x o` selects the next window.
- `C-s` starts forward incremental search; repeat `C-s` jumps to the next match.
- `C-r` starts backward incremental search; repeat `C-r` jumps to the previous match.
  Search uses Emacs-style smart case: lowercase search text matches
  case-insensitively, while unescaped uppercase search text is case-sensitive.
- `C-M-s` and `C-M-r` start forward and backward regexp incremental search.
  The built-in regexp subset supports `.`, `*`, `+`, `?`, `^`, `$`,
  Emacs-style grouping `\(...\)`, alternation `\|`, counted repetition
  `\{m\}`, `\{m,\}`, and `\{m,n\}`, escaped metacharacters, and character
  classes such as `[abc]`, `[^abc]`, and ASCII ranges like `[a-z]`. It also
  supports word constructs `\<`, `\>`, `\b`, `\B`, `\w`, and `\W`, plus ASCII
  POSIX bracket classes `[[:alpha:]]`, `[[:digit:]]`, `[[:alnum:]]`, `[[:space:]]`,
  `[[:lower:]]`, and `[[:upper:]]`. Bare `(`, `)`, `{`, `}`, and `|` match
  literally; use the escaped Emacs forms for regexp operators. Regexp search uses
  the same smart-case rule as literal search; uppercase regexp characters escaped
  with `\` do not make the whole search case-sensitive. Word constructs use
  Rile's Unicode-aware word definition: alphanumeric characters plus underscore.
- `M-p` and `M-n` move through accepted search history while an
  incremental-search prompt is active. Accept a search with Enter to record it.
  Literal search and regexp search keep separate histories; forward and backward
  search share history within each kind.
- `M-}` and `M-{` move forward and backward by blank-line-separated paragraphs.
- `M-e` and `M-a` move forward and backward by sentence using Rile's documented
  default sentence-boundary subset.
- `M-q` fills the current plain-text paragraph, or paragraphs in the active
  region, by collapsing whitespace and wrapping at the configured fill column.
- `M-;` inserts a line comment for supported modes, or toggles line comments in
  an active region. `M-x comment-region` and `M-x uncomment-region` operate on
  active regions using the current mode's line-comment marker.
- `C-t` transposes characters around point, with UTF-8-safe grapheme handling.
- `M-t` transposes words around point and `C-x C-t` transposes lines.
- Repeating search at a buffer boundary first reports a failing search; repeating
  again wraps to the first or last match and shows a wrapped-search prompt.
- `M-%` starts literal query replace; enter search and replacement strings, then
  use `y` to replace, `n` to skip, `!` to replace all remaining matches, and `q`
  to quit. Matching uses smart case; matches found case-insensitively adapt the
  replacement text's case to the matched text.
- `C-M-%` starts regexp query replace using the same line-local regexp subset as
  regexp incremental search. Replacement text expands `\&` to the whole match,
  `\1` through `\9` to numbered captures, and `\\` to a literal backslash.
  Unmatched or missing captures expand to empty text, and unsupported backslash
  escapes are preserved literally. Regexps that can match empty text are
  rejected for replacement.
- `M-x replace-regexp` prompts for a regexp and replacement string, then replaces
  all matches from point to the end of the buffer without asking at each match.
  It uses the same line-local regexp subset and replacement expansion as
  `query-replace-regexp`.
- `M-s h r` (`highlight-regexp`) adds a persistent highlight for regexp matches
  in the current buffer. `M-s h p` (`highlight-phrase`) highlights phrase
  matches, folding spaces and tabs between words while leaving other regexp
  syntax active. `M-s h l`
  (`highlight-lines-matching-regexp`) highlights whole non-empty lines that
  match a regexp. Highlight commands open a selectable face list with names such
  as `hi-yellow`, `hi-pink`, `hi-green`, or `hi-blue`; press Enter to accept the
  default shown in the prompt.
  `M-s h u` (`unhighlight-regexp`) pre-fills an editable active highlight
  pattern to remove, and `C-u M-s h u` removes all current-buffer highlights.
- `M-p` and `M-n` recall accepted query-replace search and replacement prompt
  history while editing those prompts. Literal and regexp query-replace prompts
  keep separate histories, and `replace-regexp` keeps its own regexp search and
  replacement histories.
- `M-x toggle-syntax-highlighting` toggles syntax highlighting on and off.
- `M-x toggle-search-highlighting` toggles search/query-replace highlights on and off.
- `M-x toggle-line-numbers` toggles line-number display on and off.
- `M-x not-modified` clears the current normal buffer's modified flag without
  saving.
- `C-x C-c` quits, prompting for `yes` before exiting when normal buffers have
  unsaved changes.
- `M-x` runs a command by name with completion; `C-n`/Down and `C-p`/Up move
  through candidates, `C-v`/PageDown and `M-v`/PageUp page through candidates,
  Tab inserts the selected candidate, and Enter accepts the selected candidate.
  An explicitly moved selection wins over exact minibuffer text. Command
  candidates show the first known key binding when available.
- `C-h k` describes a key binding, `C-h f` (`describe-function`) prompts for
  an interactive command with command-name completion, `C-h v`
  (`describe-variable`) describes a configuration option with option-name
  completion, `C-h m` (`describe-mode`) describes active modes, and
  `M-x describe-buffer` describes the current buffer. `C-x =` reports the
  current line, column, and point location. `C-h C-a` (`about-rile`)
  shows version, build, terminal, config, and runtime path information, and
  `C-h e` opens the read-only `*Messages*` message history, which updates while
  it is visible.
- `C-z` suspends Rile and returns to the invoking shell on terminals with job
  control; resuming the process redraws the editor.
- `M-p` and `M-n` move through history in command, file, buffer, write-file,
  goto-line, rectangle, shell-command, describe-function, describe-variable, and
  incremental-search minibuffer prompts. Incremental-search history records
  searches accepted with Enter.
- `C-g` cancels minibuffer prompts and prefix keys.

Current search and replacement matching is line-local and uses smart case: lowercase search text matches case-insensitively, while unescaped uppercase search text is case-sensitive. Replacement commands adapt replacement text casing for case-insensitive matches, so replacing `status` with `state` can produce `state`, `State`, or `STATE` to match the original text. Regexp incremental search, regexp query replace, and `replace-regexp` use Rile's built-in line-local regexp subset without an external regex dependency, including Emacs-style grouping, alternation, counted repetition, word constructs, and ASCII POSIX bracket classes. Incremental search wraps after an explicit boundary failure; replacement commands do not wrap, and no search command matches across line breaks yet.
Highlighting now flows through shared face spans and deterministic priority merging for syntax, persistent user highlights, region, search, query-replace, mode-line, minibuffer, and error faces. Minibuffer completion counters and prompt labels use minibuffer styling while editable prompt input and ordinary messages stay in the default face. Region highlighting stays visible on horizontally clipped long lines and selected line-end space.
Syntax modes are selected from file extensions for Rust, C, shell, Markdown, and TOML, with a plain-text fallback.
Window splitting stores per-window cursor state and scrolls automatically to keep point visible, including Emacs-style horizontal recentering on clipped long lines. Side-by-side splits reserve a visible separator column. Empty rows are left blank rather than filled with marker characters.
Undo is buffer-local for current-buffer edits, groups normal typing, tracks the
saved state so undoing back to that point clears the modified flag, and supports
Emacs-style redo by undoing finalized undo sequences. `undo-only` and
`undo-redo` are available through `M-x`; `C-/`, `C-x u`, and selective region
undo remain deferred.

## Configuration

Rile loads a minimal TOML-style config file from `~/.config/rile/config.toml` when it exists. Supported keys:

```toml
tab_width = 4
fill_column = 70
line_numbers = false
syntax_highlighting = true
search_highlighting = true
backup_on_save = false # when true, save one backup per buffer visit
backup_directory = "" # empty uses sibling file~ backups
auto_save = false # when true, write Emacs-style #file# auto-save files
auto_save_interval = 300 # handled key events between auto-save writes
auto_save_timeout_seconds = 30 # idle seconds before auto-save writes
auto_save_directory = "" # empty uses sibling #file# auto-save files
delete_auto_save_files = true # remove auto-save files after successful save
theme = "default" # or "mono"
completion_style = "vertical" # "completions-buffer" or "ido"
completion_max_candidates = 8
completion_show_annotations = true
completion_matching = "orderless" # "orderless", "prefix", or "substring"
```

`fill_column` accepts integer values from 20 through 200.

Backups are disabled by default.  When `backup_on_save` is true, saving an
existing file first writes a persistent backup of the original contents.  With
an empty `backup_directory`, backups live beside the file as `file~`; otherwise
Rile writes path-based mapped backup names into the configured directory.  That
directory is checked when a backup is written and must exist before saving.

Auto-save is also disabled by default.  When `auto_save` is true, dirty
file-visiting buffers write Emacs-style auto-save files without marking the
buffer clean or changing the visited file.  Empty `auto_save_directory` uses
sibling `#file#` names; otherwise Rile writes mapped path-based names wrapped in
`#...#` into the configured directory.  On Unix, new auto-save files inherit the
visited file's permissions, and rewrites never make an existing recovery file
more permissive.  Successful explicit saves delete auto-save files written by
the current session when `delete_auto_save_files` is true; pre-existing recovery
files are preserved.  Opening a file with a newer auto-save file warns so the
auto-save file can be opened manually for recovery.

Completion currently applies to `M-x` command names, `C-h f` command names,
`C-h v` option names, `C-x C-f`, `C-x C-r`, and `C-x i` file names, and
`C-x b`/`C-x k` buffer names. Completion prompts use Vertico-style selected
candidate insertion on Tab. Enter accepts the selected candidate unless exact
typed command, option, buffer, or file text is deliberately preserved; an
explicitly moved selection wins over exact text. Use `M-RET` to submit the raw
minibuffer text instead; for `C-x C-f`, this opens or creates the typed path even
when a completion candidate is selected. The default `orderless` matching for
command, option, and buffer prompts splits input on spaces and requires every
component to match in any order. File prompts follow Emacs file-category behavior
instead: they use prefix, word-component, and substring matching by default, keep
directory descent separate from orderless command matching, and use `M-RET` for
raw missing-file input when a candidate is selected. Lowercase completion input
is matched case-insensitively, while uppercase input is case-sensitive,
including file-name completion.
File prompts show the current buffer's directory as editable minibuffer text, so
the active base path is visible before typing. Long file prompts scroll
horizontally to keep the cursor-side tail visible. Directory candidates descend
on Enter when selected from exact directory text, an explicitly moved selection,
or a typed prefix of the selected directory; substring-only directory matches
keep the raw typed path.
Command completion candidates show the first known key binding, such as
`save-buffer (C-x C-s)` or `save-some-buffers (C-x s)`, when one exists.

Current completion matching supports these forms:

| Query component | Status | Meaning |
| --- | --- | --- |
| `foo` | supported | Literal substring for `orderless`; prefix, word-component, or substring matching for files. Lowercase is case-insensitive, uppercase is case-sensitive. |
| `foo bar` | supported | All `orderless` components must match in any order. |
| `^foo` | supported | Simple literal anchor; matches the beginning of a candidate. |
| `foo$` | supported | Simple literal anchor; matches the end of a candidate. |
| `^foo$` | supported | Simple literal anchor; matches the whole candidate. |
| `f-f` | supported for files | File partial-completion word-prefix matching, such as `find-file`-style components in file names. |
| `tice` | supported for files | File substring matching, such as `NOTICE.md`. |
| `!foo` | supported | Negated `orderless` component; rejects candidates matching `foo`. |
| `=foo` | supported | Force-literal `orderless` component; treats `^` and `$` as literal text. Combine as `!=foo` for negated literal matching. |
| `foo|bar` | not supported | Full regular-expression syntax is not implemented; metacharacters other than simple anchors are literal text. |
| `ff` | not yet supported | Initialism matching is not implemented. |
| `f~` | not yet supported | Fuzzy/flex matching is not implemented. |

## License

Rile is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

Rile is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See `COPYING` for details.

Copyright (c) 2026 Robert Charusta <rch-public@posteo.net>.

## Development

The preferred development workflow uses Podman, `make`, and the project dev
container. See [docs/README.md](docs/README.md) for maintainer documentation,
[docs/self-documentation.md](docs/self-documentation.md) for the implemented
help/metadata architecture, [docs/testing.md](docs/testing.md) for the testing
workflow, and [docs/external-projects.md](docs/external-projects.md) for links
to external tooling and dependencies. The host only needs:

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
make perf-smoke
```

For direct host development, install the same Rust tools locally and run the scripts under `scripts/` directly.

`make perf-smoke` is optional and runs generated large-file and long-line
performance smoke comparisons against Rile, GNU Emacs, GNU Zile, kg, and Debian
`vi`. Results are ignored local artifacts under `artifacts/perf/`; see
`docs/performance.md`.

Release notes are maintained in [NEWS](NEWS). GNU-style file-level maintenance history is maintained in [ChangeLog](ChangeLog); Git remains the detailed development history.

CI is deferred until it is configured for the official repository.

## Reference Policy

The repository includes optional reference-testing tooling for studying behavior of reference editors such as GNU Zile, kg, and GNU Emacs. Rile should use reference editors only for behavior and architecture lessons unless license implications are explicitly documented. Do not copy, translate, or mechanically port reference implementation code into Rile.

See [NOTICE.md](NOTICE.md) for the current third-party code status.
