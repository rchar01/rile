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

## First-Batch Reference Entries

### `downcase-word`

Status: `implemented` in Rile as `downcase-word`.

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

Rile target: implemented as a small compatible subset: positive arguments,
negative arguments, middle-of-word spans, UTF-8-safe edits, point placement, and
single-command undo. Exact Emacs Unicode case-mapping edge cases remain deferred
until behavior evidence requires them.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-l` and grouped word
case-conversion behavior; Emacs scenario
`tools/reference/emacs/scenarios/case-word-core.scenario`; Rile command registry
entry `downcase-word`; Rile unit and PTY tests for case conversion.

Notes: Rile already has word movement and word-kill boundaries. The case commands
should reuse or deliberately refine those boundaries rather than invent a third
word model.

### `upcase-word`

Status: `implemented` in Rile as `upcase-word`.

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

Rile target: implemented with the same first-slice semantics as `downcase-word`,
with case conversion changed to upper case.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-u` and grouped word
case-conversion behavior; Emacs scenario
`tools/reference/emacs/scenarios/case-word-core.scenario`; Rile command registry
entry `upcase-word`; Rile unit and PTY tests for case conversion.

Notes: Case conversion may expand some Unicode characters. Preserve valid UTF-8
and make point movement deterministic if converted text changes byte length.

### `capitalize-word`

Status: `implemented` in Rile as `capitalize-word`.

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

Rile target: implemented as a small compatible subset with first cased character
upper case and remaining cased characters lower case for each affected word. Exact
Emacs syntax-table and locale edge cases remain deferred.

Evidence: GNU Emacs manual, Case Conversion Commands, `M-c` and grouped word
case-conversion behavior; Emacs scenario
`tools/reference/emacs/scenarios/case-word-core.scenario`; Rile command registry
entry `capitalize-word`; Rile unit and PTY tests for case conversion.

Notes: The implementation should define capitalization in terms of Unicode scalar
values and documented Rile word boundaries, then cover mixed ASCII and UTF-8 text
in unit tests.

### `downcase-region`

Status: `implemented` in Rile as `downcase-region`.

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

Rile target: implemented with an intentional difference from base Emacs: Rile does
not add disabled-command confirmation for this command family. It matches the
region transformation, point/mark preservation, read-only behavior, and undo
granularity subset.

Evidence: GNU Emacs manual, Case Conversion Commands, `C-x C-l`; Emacs scenario
`tools/reference/emacs/scenarios/case-region-core.scenario`; Rile command
registry entry `downcase-region`; Rile unit and PTY tests for case conversion.

Notes: This should share the same region-range and read-only validation path as
`kill-region` and `copy-region-as-kill` where practical.

### `upcase-region`

Status: `implemented` in Rile as `upcase-region`.

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

Rile target: implemented with an intentional difference from base Emacs: Rile does
not add disabled-command confirmation for this command family. It matches the
region transformation, point/mark preservation, read-only behavior, and undo
granularity subset.

Evidence: GNU Emacs manual, Case Conversion Commands, `C-x C-u`; Emacs scenario
`tools/reference/emacs/scenarios/case-region-core.scenario`; Rile command
registry entry `upcase-region`; Rile unit and PTY tests for case conversion.

Notes: Case conversion may change byte length for some Unicode text, so region
restoration and undo tests should cover non-ASCII input.

### `delete-horizontal-space`

Status: `implemented` in Rile as `delete-horizontal-space`.

Default binding: `M-\`.

Purpose: delete spaces and tabs around point.

Prompt flow: no prompt.

Prefix argument behavior: with a prefix argument, delete only spaces and tabs
before point. Without a prefix argument, delete adjacent spaces and tabs on both
sides of point.

Region behavior: not a region command.

Point after command: point stays at the boundary where the surrounding horizontal
space was removed. In the backward-only form, point moves left by the number of
deleted characters before point.

Undo behavior: the command should be one undoable edit for the deleted span.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implemented as a small Emacs-compatible subset for ASCII space and
tab characters. Newlines are not treated as horizontal space.

Evidence: GNU Emacs `describe-function` output for `delete-horizontal-space`;
current GNU Emacs key binding for `M-\`; Emacs scenario
`tools/reference/emacs/scenarios/whitespace-spacing-core.scenario`; Rile command
registry entry `delete-horizontal-space`; Rile unit and PTY tests for whitespace
cleanup.

Notes: This command is a good unit-test target because it does not depend on
terminal-visible prompts or mode-specific indentation rules.

### `just-one-space`

Status: `missing`.

Default binding: no active default binding in current GNU Emacs; active key lookup
resolves `M-SPC` to `cycle-spacing`.

Purpose: collapse spaces and tabs around point to a requested number of spaces.

Prompt flow: no prompt.

Prefix argument behavior: without a numeric argument, leave one space. With a
numeric argument `N`, leave `N` spaces. With a negative numeric argument, delete
newlines as well and leave `-N` spaces.

Region behavior: not a region command.

Point after command: point lands after the inserted replacement spaces, matching
the normal insertion point for the collapsed spacing.

Undo behavior: the collapse and replacement insertion should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implement a small subset before `cycle-spacing`: no-argument
one-space collapse and positive numeric arguments for horizontal space. Defer
negative-argument newline joining unless a focused scenario confirms it is worth
the extra behavior.

Evidence: GNU Emacs `describe-function` output for `just-one-space`; current GNU
Emacs key binding for `M-SPC` resolves to `cycle-spacing`; `M-x just-one-space`
advertises `M-SPC` after execution; Emacs scenario
`tools/reference/emacs/scenarios/whitespace-spacing-core.scenario`; Rile command
registry currently has no `just-one-space` entry.

Notes: The plan names this underlying command, but `M-SPC` has subtle current
Emacs behavior because `cycle-spacing` is the active binding while `M-x
just-one-space` still advertises `M-SPC`. Decide whether Rile should expose
`just-one-space` first or bind `M-SPC` to a documented subset of `cycle-spacing`
in a later slice.

### `delete-blank-lines`

Status: `implemented` in Rile as `delete-blank-lines`.

Default binding: `C-x C-o`.

Purpose: remove redundant blank lines around point.

Prompt flow: no prompt.

Prefix argument behavior: not meaningful for the first Rile implementation.

Region behavior: not a region command.

Point after command: on a run of multiple blank lines, point remains on the single
remaining blank line. On an isolated blank line, that line is deleted. On a
nonblank line, immediately following blank lines are deleted.

Undo behavior: the whole blank-line deletion should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implemented as a small compatible subset using Rile's existing
definition of blank lines as lines containing only spaces or tabs. The command
collapses multi-blank-line runs, deletes isolated blank lines, and deletes
following blank lines after a nonblank line.

Evidence: GNU Emacs manual, Blank Lines, `C-x C-o`; GNU Emacs
`describe-function` output for `delete-blank-lines`; Emacs scenario
`tools/reference/emacs/scenarios/whitespace-cleanup-core.scenario`; Rile command
registry entry `delete-blank-lines`; Rile unit and PTY tests for whitespace
cleanup.

Notes: Unit tests should cover point on a nonblank line before blank lines, point
inside a multi-blank-line run, and point on a single blank line.

### `delete-trailing-whitespace`

Status: `implemented` in Rile as `delete-trailing-whitespace`.

Default binding: none.

Purpose: delete trailing whitespace at line ends, and optionally trailing blank
lines at the end of the buffer.

Prompt flow: no prompt.

Prefix argument behavior: not meaningful for the first Rile implementation.

Region behavior: if the region is active, base Emacs uses the region bounds as
the cleanup range. Without an active region, it operates on the whole accessible
buffer.

Point after command: point should remain stable when possible. If deleted text is
before point or includes point, Rile should use its normal edit-adjustment rules
and cover the result with tests.

Undo behavior: the whole cleanup should be one undoable command result, even when
multiple lines are changed.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile target: implemented as a small compatible subset: delete ASCII spaces and
tabs at physical line ends within the active region bounds or whole buffer. Defer
Emacs's `delete-trailing-lines` customization and formfeed exception unless Rile
gains the corresponding customization surface.

Evidence: GNU Emacs manual, Useless Whitespace, `delete-trailing-whitespace`; GNU
Emacs `describe-function` output; Emacs scenario
`tools/reference/emacs/scenarios/whitespace-cleanup-core.scenario`; Rile command
registry entry `delete-trailing-whitespace`; Rile unit tests for whole-buffer,
active-region, undo, no-op, and read-only behavior.

Notes: This command is likely useful before full whitespace visualization support
because it can be implemented and tested independently of rendering faces.

### `transpose-chars`

Status: `implemented` in Rile as `transpose-chars`.

Default binding: `C-t`.

Purpose: transpose adjacent characters around point.

Prompt flow: no prompt.

Prefix argument behavior: a numeric argument is a repeat count. Positive
arguments drag the character before point forward across that many following
characters; negative arguments drag it backward. A zero argument has special
Emacs mark-based behavior and transposes the character ending after point with the
one ending after mark.

Region behavior: not a region command. The zero-argument form depends on mark,
but does not transform the active region as a region command.

Point after command: without a prefix argument, point moves forward one character.
At end of line, Emacs transposes the previous two characters instead of swapping a
character with the newline. At beginning of buffer or without enough text, Emacs
signals an error.

Undo behavior: the transposition should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Boundary failures should use Rile's
normal command-status style for failed edit commands.

Rile implementation: implements the first compatible subset: no-prefix
transposition, positive and negative repeat counts, end-of-line special case,
point movement, UTF-8-safe grapheme boundaries, and single-command undo. It
reports a boundary failure when a prefixed end-of-line request cannot be honored
within the line. It defers zero-argument mark-based transposition.

Evidence: GNU Emacs manual, Transposing Text, `C-t`; GNU Emacs
`describe-function` output for `transpose-chars`; local batch probes for ordinary,
end-of-line, zero-argument, and beginning-of-buffer behavior; Emacs scenario
`tools/reference/emacs/scenarios/transpose-core.scenario`; Rile command registry
entry `transpose-chars`; Rile unit and PTY tests for ordinary, end-of-line,
UTF-8, argument, undo, and read-only behavior.

Notes: Terminal `C-t` should be checked against Rile's input layer before binding
because control-key availability can vary by terminal mode.

### `transpose-words`

Status: `implemented` in Rile as `transpose-words`.

Default binding: `M-t`.

Purpose: transpose the word before or containing point with the next word.

Prompt flow: no prompt.

Prefix argument behavior: a numeric argument is a repeat count. Positive
arguments drag the word before or containing point forward across that many words;
negative arguments drag it backward. A zero argument transposes words around or
after point and mark.

Region behavior: not a region command. The zero-argument form depends on mark,
but does not transform the active region as a region command.

Point after command: after a successful positive transposition, point lands at the
end of the transposed words. Punctuation between words stays in place; for
example, `FOO, BAR` becomes `BAR, FOO`. At end of line, Emacs can transpose the
word before point with the first word on the next line. Without two words to
transpose, Emacs signals an error.

Undo behavior: the transposition should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Boundary failures should use Rile's
normal command-status style for failed edit commands.

Rile implementation: implements a small compatible subset using Rile's documented
word boundaries: no-prefix behavior, positive and negative repeat counts,
punctuation preservation between swapped words, point placement, read-only checks,
and one undo entry per command. It defers zero-argument mark-based behavior and
exact Emacs syntax-table edge cases.

Evidence: GNU Emacs manual, Transposing Text, `M-t`; GNU Emacs
`describe-function` output for `transpose-words`; local batch probes for
punctuation, zero-argument, and missing-word behavior; Emacs scenario
`tools/reference/emacs/scenarios/transpose-core.scenario`; Rile command registry
entry `transpose-words`; Rile unit and PTY tests for punctuation, UTF-8, numeric
arguments, undo, boundary failures, and read-only behavior.

Notes: This should reuse the same word-boundary model selected for case
conversion and word movement so Rile does not grow inconsistent command-specific
word rules.

### `transpose-lines`

Status: `implemented` in Rile as `transpose-lines`.

Default binding: `C-x C-t`.

Purpose: exchange the current line with the previous line.

Prompt flow: no prompt.

Prefix argument behavior: with a numeric argument, Emacs moves the previous line
past that many lines. Negative arguments move it backward. With argument zero,
Emacs interchanges the line containing point with the line containing mark.

Region behavior: not a region command. The zero-argument form depends on mark,
but does not transform the active region as a region command.

Point after command: without a prefix argument, point is left after both exchanged
lines. Local probes show `one\ntwo\n` with point on `two` becomes `two\none\n` and
point lands at the end of the exchanged pair.

Undo behavior: the line exchange should be one undoable edit.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Boundary failures should use Rile's
normal command-status style for failed edit commands.

Rile implementation: implements a small compatible subset: no-prefix previous-line
movement past the current line, numeric repeat counts, positive and negative line
movement, point placement, final-newline-safe editing, read-only checks, and one
undo entry per command. It defers zero-argument mark-line exchange unless a later
capture shows it is important for Rile's scope.

Evidence: GNU Emacs manual, Transposing Text, `C-x C-t`; GNU Emacs
`describe-function` output for `transpose-lines`; local batch probes for ordinary,
negative-argument, and zero-argument behavior; Emacs scenario
`tools/reference/emacs/scenarios/transpose-core.scenario`; Rile command registry
entry `transpose-lines`; Rile unit and PTY tests for no-prefix movement, numeric
arguments, UTF-8 lines, undo, boundary failures, and read-only behavior.

Notes: Tests should cover files without a trailing newline, the first line, the
last line, and multi-byte UTF-8 text so line-range replacement does not corrupt
buffer storage.

### `fill-paragraph`

Status: `implemented` in Rile as `fill-paragraph`.

Default binding: `M-q` globally and in text buffers. Current GNU Emacs
programming modes can remap `M-q` to mode-specific fill commands such as
`prog-fill-reindent-defun` or `c-fill-paragraph`.

Purpose: reflow the current paragraph, or paragraphs in the active region, to fit
within the fill column.

Prompt flow: no prompt.

Prefix argument behavior: a numeric argument tells Emacs to justify the filled
text as well as reflow it. The first Rile implementation should not implement
justification.

Region behavior: when called interactively with an active region, base Emacs fills
each paragraph in the region. Otherwise, it fills the paragraph at point, or the
paragraph after point when point is between paragraphs.

Point after command: base behavior generally preserves point relative to the
filled text when possible. Local text-mode probes show point at paragraph start
remaining at the start after a simple fill.

Undo behavior: filling the affected paragraph or region should be one undoable
command result.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required for the normal edit.

Rile implementation: implements a documented plain-text subset: fill paragraphs
by collapsing internal spaces and line breaks, wrapping at a fixed fill column,
and preserving blank-line paragraph boundaries. Active-region invocation fills
each paragraph overlapped by the region. It defers justification,
sentence-end-double-space rules, CJK/kinsoku behavior, fill prefixes,
mode-specific comment filling, and programmable fill hooks.

Evidence: GNU Emacs manual, Explicit Fill Commands, `M-q`; GNU Emacs
`describe-function` output for `fill-paragraph`; local batch probes for text-mode
paragraph and active-region filling; Emacs scenario
`tools/reference/emacs/scenarios/fill-paragraph-core.scenario`; Rile command
registry entry `fill-paragraph`; Rile unit and PTY tests for wrapping, active
regions, blank-line behavior, undo, and read-only behavior.

Notes: This should probably share wrapping code with help-buffer prose wrapping,
but editor-buffer filling needs separate undo, point-adjustment, region, and
read-only tests.

### `comment-dwim`

Status: `missing`.

Default binding: `M-;`.

Purpose: insert, align, comment, uncomment, or kill comments depending on point,
region, mode, and prefix argument.

Prompt flow: no prompt.

Prefix argument behavior: active-region numeric behavior is delegated through
Emacs comment-region logic; current GNU Emacs documentation describes numeric
`ARG` as controlling how many characters are removed from each comment delimiter.
Without an active region, a prefix argument kills comments on the current line, or
on multiple lines for numeric arguments.

Region behavior: with an active region, base Emacs comments the region unless all
lines are already comments, in which case it uncomments the region.

Point after command: without a region, adding or realigning a comment places point
after the comment start delimiter so the user can type comment text. Region
commenting should preserve useful point/mark positions, but Rile can define and
test a simpler deterministic result for its first subset.

Undo behavior: the chosen comment action should be one undoable command result.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. If the current mode has no comment
syntax, Rile should report a normal command error rather than guessing.

Rile target: implement a smaller line-comment subset first for modes with known
line comment markers, such as Rust/C `//` and shell/TOML `#`. Support active-
region comment/uncomment toggling and simple current-line comment insertion.
Defer alignment to `comment-column`, block comments, `comment-style`, comment
killing, numeric delimiter-count behavior, and mode-specific indentation rules.

Evidence: GNU Emacs manual, Comment Commands, `M-;`; GNU Emacs
`describe-function` output for `comment-dwim`; local key-binding checks for
`M-;`; Emacs scenario
`tools/reference/emacs/scenarios/comment-commands-core.scenario`; Rile command
registry currently has no `comment-dwim` entry and Rile syntax highlighting
currently stores comment markers only inside highlighter logic.

Notes: Before implementation, Rile should expose reusable comment syntax metadata
rather than deriving editing behavior from rendering-only highlighter internals.

### `comment-region`

Status: `missing`.

Default binding: none globally. In C mode and related modes, GNU Emacs binds
`C-c C-c` to `comment-region`.

Purpose: add comment delimiters to each line in a region.

Prompt flow: no prompt.

Prefix argument behavior: plain `C-u` uncomments each line in the region. Numeric
arguments specify how many comment characters to add; negative numeric arguments
remove that many comment characters.

Region behavior: operates on the supplied region even if the mark is inactive.
Base Emacs uses mode-specific `comment-start`, `comment-padding`, `comment-end`,
and `comment-style` settings; in C mode, local probes show default block-comment
wrapping such as `/* int x; */` for each line.

Point after command: point should remain stable when possible, adjusted through
normal edit rules if inserted delimiters occur before point.

Undo behavior: commenting the whole region should be one undoable command result.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Missing comment syntax should use
Rile's normal command-error status.

Rile target: intentionally differ from full Emacs and implement a line-comment
subset first. Add or remove one configured line-comment marker at each non-empty
line's indentation within the region. Defer block comments, configurable padding,
blank-line style variants, inactive-mark operation from `M-x`, and exact Emacs
mode-specific comment styles.

Evidence: GNU Emacs manual, Comment Commands, `comment-region`; GNU Emacs
`describe-function` output; local C-mode probes for `C-c C-c` and block-comment
behavior; Emacs scenario
`tools/reference/emacs/scenarios/comment-commands-core.scenario`; Rile command
registry currently has no `comment-region` entry.

Notes: Rile may still bind a future line-comment subset differently from C mode's
`C-c C-c` if that prefix is reserved for mode-specific keymaps later.

### `uncomment-region`

Status: `missing`.

Default binding: none.

Purpose: remove comment delimiters from each line in a region.

Prompt flow: no prompt.

Prefix argument behavior: a numeric argument can specify how many characters to
remove from comment delimiters.

Region behavior: operates on the supplied region. Base Emacs follows the current
mode's comment syntax; the first Rile subset should operate only on configured
line-comment markers.

Point after command: point should remain stable when possible, adjusted through
normal edit rules if removed delimiters occur before point.

Undo behavior: uncommenting the whole region should be one undoable command
result.

Read-only behavior: should refuse to edit a read-only buffer through the normal
Rile read-only guard.

Messages: no success message is required. Missing comment syntax should use
Rile's normal command-error status.

Rile target: implement as the inverse of Rile's first `comment-region` subset:
remove one configured line-comment marker and one optional following space at each
commented line's indentation. Defer numeric delimiter-count behavior and block
comment syntax.

Evidence: GNU Emacs manual, Comment Commands, `uncomment-region`; GNU Emacs
`describe-function` output; local C-mode probes for `comment-region` followed by
`uncomment-region`; Emacs scenario
`tools/reference/emacs/scenarios/comment-commands-core.scenario`; Rile command
registry currently has no `uncomment-region` entry.

Notes: Keep this command's parser strict enough that uncommenting a region does
not delete comment-like text in strings or later code columns unless it is at the
configured indentation position.

### `forward-paragraph`

Status: `implemented` in Rile as `forward-paragraph`.

Default binding: `M-}`.

Purpose: move point forward to the end of the current or next paragraph.

Prompt flow: no prompt.

Prefix argument behavior: numeric arguments repeat the movement. Negative
arguments move backward by paragraphs.

Region behavior: not a region command. If the region is active, movement should
deactivate it unless Rile's general movement-command policy says otherwise.

Point after command: base Emacs moves to the end of the current paragraph, or the
next paragraph when point is between paragraphs. In Fundamental and Text modes,
blank lines made of spaces, tabs, or formfeed characters separate paragraphs.
Local probes show point moving from the beginning of `one\ntwo\n\n` to the blank
line after `two`.

Undo behavior: movement only; no undo entry.

Read-only behavior: movement should work in read-only buffers.

Messages: no success message is required. Boundary cases should follow Rile's
normal movement-command style.

Rile implementation: implements the first compatible subset: blank-line-
separated paragraphs, positive and negative repeat counts, point visibility, and
no buffer mutation. Empty lines and lines containing only spaces, tabs, or
formfeed characters separate paragraphs. It defers Emacs's `paragraph-start`,
`paragraph-separate`, fill-prefix, and mode-specific paragraph customizations.

Evidence: GNU Emacs manual, Paragraphs, `M-}`; GNU Emacs `describe-function`
output for `forward-paragraph`; local batch probes for blank-line boundaries and
repeat counts; Rile command registry entry `forward-paragraph`; Rile unit and PTY
tests for blank-line-separated paragraph movement and numeric arguments.

Notes: The first implementation should share paragraph-boundary code with
`backward-paragraph` and, later, `fill-paragraph`.

### `backward-paragraph`

Status: `implemented` in Rile as `backward-paragraph`.

Default binding: `M-{`.

Purpose: move point backward to the beginning of the current or previous
paragraph.

Prompt flow: no prompt.

Prefix argument behavior: numeric arguments repeat the movement. Negative
arguments move forward by paragraphs.

Region behavior: not a region command. If the region is active, movement should
deactivate it unless Rile's general movement-command policy says otherwise.

Point after command: base Emacs moves to the beginning of the current or previous
paragraph. If a blank line precedes the paragraph, Emacs can place point on that
blank line. Local probes show moving backward from a paragraph boundary to the
beginning of the preceding paragraph.

Undo behavior: movement only; no undo entry.

Read-only behavior: movement should work in read-only buffers.

Messages: no success message is required. Boundary cases should follow Rile's
normal movement-command style.

Rile implementation: implements the inverse of the first `forward-paragraph`
subset: blank-line-separated paragraphs, positive and negative repeat counts,
point visibility, and no buffer mutation. Empty lines and lines containing only
spaces, tabs, or formfeed characters separate paragraphs. It defers customizable
paragraph regex behavior and fill-prefix interactions.

Evidence: GNU Emacs manual, Paragraphs, `M-{`; GNU Emacs `describe-function`
output for `backward-paragraph`; local batch probes for blank-line boundaries and
negative arguments; Rile command registry entry `backward-paragraph`; Rile unit
and PTY tests for blank-line-separated paragraph movement and numeric arguments.

Notes: Tests should cover beginning of buffer, consecutive blank lines, indented
nonblank lines, and buffers without a final newline.

### `forward-sentence`

Status: `missing`.

Default binding: `M-e`.

Purpose: move point forward to the end of the current or next sentence.

Prompt flow: no prompt.

Prefix argument behavior: numeric arguments repeat the movement. Negative
arguments move backward to sentence starts.

Region behavior: not a region command. If the region is active, movement should
deactivate it unless Rile's general movement-command policy says otherwise.

Point after command: base Emacs places point just after the punctuation ending the
sentence and before following whitespace. With default settings, Emacs treats `.`,
`?`, or `!` as ending a sentence only when followed by end of line or two spaces,
with closing delimiters allowed between punctuation and whitespace. Sentence
movement also stops at paragraph boundaries.

Undo behavior: movement only; no undo entry.

Read-only behavior: movement should work in read-only buffers.

Messages: no success message is required. Emacs signals end-of-buffer when a
forward sentence movement cannot complete; Rile can use its normal movement-error
status.

Rile target: implement a documented subset first: ASCII `.`, `?`, and `!`
sentence terminators, optional closing quotes/brackets, Emacs's default two-space
or end-of-line boundary rule, paragraph-boundary stops, positive and negative
repeat counts, and no buffer mutation. Defer customizable `sentence-end`,
`sentence-end-double-space`, language-specific sentence rules, and mode-specific
sentence functions.

Evidence: GNU Emacs manual, Sentences, `M-e`; GNU Emacs `describe-function`
output for `forward-sentence`; local batch probes for two-space boundaries,
single-space non-boundaries, paragraph stops, negative arguments, and
end-of-buffer behavior; Rile command registry currently has no `forward-sentence`
entry.

Notes: This command should not rely on word movement boundaries. Sentence
recognition has different punctuation and whitespace rules.

### `backward-sentence`

Status: `missing`.

Default binding: `M-a`.

Purpose: move point backward to the beginning of the current or previous sentence.

Prompt flow: no prompt.

Prefix argument behavior: numeric arguments repeat the movement. Negative
arguments move forward to sentence ends.

Region behavior: not a region command. If the region is active, movement should
deactivate it unless Rile's general movement-command policy says otherwise.

Point after command: base Emacs places point just before the first character of
the sentence. It does not move over the whitespace at the sentence boundary.
Sentence starts and ends are also recognized at paragraph boundaries.

Undo behavior: movement only; no undo entry.

Read-only behavior: movement should work in read-only buffers.

Messages: no success message is required. Boundary cases should follow Rile's
normal movement-command style.

Rile target: implement the inverse of the first `forward-sentence` subset:
default two-space sentence boundaries, paragraph-boundary stops, positive and
negative repeat counts, point visibility, and no buffer mutation. Defer
customizable sentence variables and mode-specific sentence functions.

Evidence: GNU Emacs manual, Sentences, `M-a`; GNU Emacs `describe-function`
output for `backward-sentence`; local batch probes for backward movement and
negative arguments; Rile command registry currently has no `backward-sentence`
entry.

Notes: Tests should cover punctuation followed by one space, two spaces, newline,
closing quotes, and paragraph boundaries.
