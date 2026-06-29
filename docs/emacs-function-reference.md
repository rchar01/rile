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

## Missing Reference Entries

### `downcase-word`

Status: `missing`.

Default binding: `M-l`.

Purpose: convert the following word, or words selected by a numeric argument, to
lower case.

Prompt flow: no prompt.

Prefix argument behavior: positive numeric arguments convert that many following
words and move point past the converted text. Negative arguments convert words
before point and leave point where it started. `M-- M-l` is the common one-word
backward form.

Region behavior: not a region command. If point is in the middle of a word, only
the part of the word after point is converted; with a negative argument, only the
part before point is converted.

Point after command: positive forms move point to the end of the converted text.
Negative forms keep point at its original position.

Undo behavior: the command should be one undoable edit for the converted span.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit. Missing text to
convert should be a no-op or use Rile's existing command-status style if a later
implementation needs feedback.

Rile target: implement a small compatible subset first: positive arguments,
negative arguments, middle-of-word spans, UTF-8-safe edits, point placement, and
single-command undo. Exact Emacs Unicode case-mapping edge cases can be deferred
until behavior evidence requires them.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-l` and grouped word
case-conversion behavior; Rile command registry currently has no `downcase-word`
entry.

Notes: Rile already has word movement and word-kill boundaries. The case commands
should reuse or deliberately refine those boundaries rather than invent a third
word model.

### `upcase-word`

Status: `missing`.

Default binding: `M-u`.

Purpose: convert the following word, or words selected by a numeric argument, to
upper case.

Prompt flow: no prompt.

Prefix argument behavior: positive numeric arguments convert that many following
words and move point past the converted text. Negative arguments convert words
before point and leave point where it started. `M-- M-u` is the common one-word
backward form.

Region behavior: not a region command. If point is in the middle of a word, only
the part of the word after point is converted; with a negative argument, only the
part before point is converted.

Point after command: positive forms move point to the end of the converted text.
Negative forms keep point at its original position.

Undo behavior: the command should be one undoable edit for the converted span.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implement the same first-slice semantics as `downcase-word`, with
case conversion changed to upper case.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-u` and grouped word
case-conversion behavior; Rile command registry currently has no `upcase-word`
entry.

Notes: Case conversion may expand some Unicode characters. Preserve valid UTF-8
and make point movement deterministic if converted text changes byte length.

### `capitalize-word`

Status: `missing`.

Default binding: `M-c`.

Purpose: convert the following word, or words selected by a numeric argument, to
capitalized form.

Prompt flow: no prompt.

Prefix argument behavior: positive numeric arguments capitalize that many
following words and move point past the converted text. Negative arguments
capitalize words before point and leave point where it started. `M-- M-c` is the
common one-word backward form.

Region behavior: not a region command. If point is in the middle of a word, only
the part of the word after point is converted; with a negative argument, only the
part before point is converted.

Point after command: positive forms move point to the end of the converted text.
Negative forms keep point at its original position.

Undo behavior: the command should be one undoable edit for the converted span.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implement a small compatible subset with first cased character upper
case and remaining cased characters lower case for each affected word. Exact Emacs
syntax-table and locale edge cases can be deferred.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-c` and grouped word
case-conversion behavior; Rile command registry currently has no
`capitalize-word` entry.

Notes: The implementation should define capitalization in terms of Unicode scalar
values and documented Rile word boundaries, then cover mixed ASCII and UTF-8 text
in unit tests.

### `downcase-region`

Status: `missing`.

Default binding: `C-x C-l`.

Purpose: convert the active region to lower case without moving point or mark.

Prompt flow: base Emacs normally treats this command as disabled and asks for
confirmation the first time it is used. Rile does not currently have a disabled
command system.

Prefix argument behavior: not meaningful for the first Rile implementation.

Region behavior: converts the text between point and mark. Point and mark remain
in place.

Point after command: point and mark remain where they were before the command.

Undo behavior: the whole region conversion should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Missing or inactive region should use
the same no-region behavior as other Rile region commands.

Rile target: intentionally differ from base Emacs by not adding disabled-command
confirmation for this one command family. Match the region transformation,
point/mark preservation, read-only behavior, and undo granularity.

Evidence: GNU Emacs manual, Case Conversion Commands, `C-x C-l`; Rile command
registry currently has no `downcase-region` entry.

Notes: This should share the same region-range and read-only validation path as
`kill-region` and `copy-region-as-kill` where practical.

### `upcase-region`

Status: `missing`.

Default binding: `C-x C-u`.

Purpose: convert the active region to upper case without moving point or mark.

Prompt flow: base Emacs normally treats this command as disabled and asks for
confirmation the first time it is used. Rile does not currently have a disabled
command system.

Prefix argument behavior: not meaningful for the first Rile implementation.

Region behavior: converts the text between point and mark. Point and mark remain
in place.

Point after command: point and mark remain where they were before the command.

Undo behavior: the whole region conversion should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Missing or inactive region should use
the same no-region behavior as other Rile region commands.

Rile target: intentionally differ from base Emacs by not adding disabled-command
confirmation for this one command family. Match the region transformation,
point/mark preservation, read-only behavior, and undo granularity.

Evidence: GNU Emacs manual, Case Conversion Commands, `C-x C-u`; Rile command
registry currently has no `upcase-region` entry.

Notes: Case conversion may change byte length for some Unicode text, so region
restoration and undo tests should cover non-ASCII input.
