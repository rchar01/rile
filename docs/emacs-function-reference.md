<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Emacs Function Reference

This document records selected GNU Emacs command behavior that is relevant to
Rile. It is a behavior reference, not a source-porting guide. Use it to write
original Rile requirements and tests before implementing Emacs-compatible command
families.

Prefer base terminal Emacs evidence from the `core` reference profile. Use the
`modern` profile only for commands whose behavior is specifically about modern
completion UX.

## Entry Template

Use this template for new command entries:

```md
### `command-name`

Status: `missing`, `implemented`, `partial`, or `intentionally different`.

Default binding: `KEY`, or none.

Purpose: one short sentence.

Prompt flow: prompts, defaults, completion, and cancel behavior.

Prefix argument behavior: repeat count, direction, narrowing, or not meaningful.

Region behavior: active-region semantics, mark handling, or not meaningful.

Point after command: where point lands after success and after no-op cases.

Undo behavior: expected undo granularity and whether the command exits an active
interactive state.

Read-only behavior: expected behavior in read-only buffers or text.

Messages: user-visible minibuffer/status/help text that matters for tests.

Rile target: exact match, small subset, intentional difference, or undecided.

Evidence: manual sections, `describe-function` output, reference scenarios, and
Rile tests/docs.

Notes: open questions or implementation constraints.
```

Evidence should cite behavior sources rather than copy implementation code. Good
sources are GNU Emacs manual sections, `describe-function` output captured from a
reference Emacs build, and `tools/reference/emacs/scenarios/` visual captures.

## Implemented Reference Entries

### `join-line` / `delete-indentation`

Status: `implemented` in Rile as `join-line`.

Default binding: `M-^`.

Purpose: merge the current line with the previous line, cleaning indentation at
the join.

Prompt flow: no prompt.

Prefix argument behavior: base Emacs uses a prefix argument to join the current
line with the following line. Rile currently implements the no-prefix previous
line join only.

Region behavior: base Emacs can join lines in an active region when no prefix
argument is given. Rile currently implements the point-local line join only.

Point after command: for the no-prefix line-local case, point lands at the join
site on the merged previous line. Joining the second line of `alpha` and
indented `beta` leaves a single `alpha beta` line; joining across a blank line can
remove the blank boundary before the indented line.

Undo behavior: the merge is one edit from the user's perspective and should be
undoable as a single command result in Rile.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit. Read-only refusal
uses Rile's existing read-only error message.

Rile target: keep the implemented no-prefix behavior compatible. Prefix and
region variants are useful but are not required before the first case-conversion
slice.

Evidence: GNU Emacs manual, Indentation Commands, `M-^`; Emacs scenario
`tools/reference/emacs/scenarios/join-line-core.scenario`; Rile command registry
entry `join-line`; Rile docs in `README.md`.

Notes: Emacs exposes the underlying function as `delete-indentation`; Rile uses
the user-facing command name `join-line`, matching the familiar command name from
Zile/kg and current Rile docs.

### `query-replace`

Status: `implemented` in Rile as `query-replace`.

Default binding: `M-%`.

Purpose: prompt for a search string and replacement string, then ask before
replacing each match.

Prompt flow: `M-%` first prompts for the search string, then prompts for the
replacement string. After both prompts are accepted, point moves to the current
candidate and the minibuffer waits for a choice key. Rile supports `y` to replace,
`n` to skip, `!` to replace all remaining matches, and `q`, Escape, or `C-g` to
quit.

Prefix argument behavior: base Emacs uses numeric prefix arguments for word-bound
matching and negative prefix arguments for backward replacement. Rile currently
does not implement those variants.

Region behavior: base Emacs has broader replacement behavior through narrowing,
multi-buffer workflows, and related commands. Rile's current command operates
from point through the current buffer and does not wrap.

Point after command: point visits each candidate during the workflow. After a
replacement, point advances to the next candidate. After `!`, Rile replaces all
remaining matches and reports the replacement count.

Undo behavior: each replacement records undo information. Undo after the command
should restore replaced text through Rile's normal buffer-local undo path.

Read-only behavior: should refuse modifications in read-only buffers through the
normal Rile read-only guard.

Messages: prompts and choice status are user-visible. Rile reports completion
status after replacement, including the number of replacements made.

Rile target: keep the current subset. Do not attempt full Emacs query-replace
choice-key coverage before simpler missing command families.

Evidence: GNU Emacs manual, Query Replace; Emacs scenario
`tools/reference/emacs/scenarios/query-replace-core.scenario`; Rile command
registry entry `query-replace`; Rile PTY and unit tests for query replace.

Notes: Emacs supports many additional choice keys such as comma, period, undo,
recursive edit, replacement editing, and help. Those are out of scope for the
current Rile command unless a concrete user need appears.
