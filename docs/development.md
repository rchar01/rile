<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Development Notes

## Repository Scope

`rile/` is the implementation project and intended distributable repository. The parent `rile-lab/` workspace is private research context and is not part of the Rile crate.

The official repository is <https://codeberg.org/rch/rile>. Rile is copyrighted by Robert Charusta <rch-public@posteo.net>.

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
- movement commands support character, word, line, beginning-of-line, and end-of-line motion;
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

Milestone 7 adds incremental search:

- `C-s` starts forward incremental search;
- `C-r` starts backward incremental search;
- `C-M-s` and `C-M-r` start forward and backward regexp incremental search;
- typed query text updates the current match live;
- repeated `C-s` and `C-r` jump to the next or previous match;
- repeating search at a buffer boundary first reports a failing search, and the
  next repeat wraps to the first or last match with a wrapped-search prompt;
- Enter accepts the current match;
- `C-g` cancels search and restores the original cursor position;
- active search uses `render::Face::CurrentSearchMatch` and
  `render::Face::SearchMatch` spans;
- the terminal renderer displays active search spans with ANSI highlighting;
- tests cover UTF-8 matches, repeated search, wrapping, cancellation, failed
  search, regexp search, and ANSI span rendering.

Milestone 8 adds multiple buffers:

- `buffers::BufferManager` owns stable `BufferId` values and buffer entries;
- each buffer entry records a user-facing name and a file-backed `Document`;
- `find-file` reuses an existing buffer for the same path instead of opening duplicates;
- `C-x b` and `switch-to-buffer` prompt for an existing buffer name;
- `C-x k` and `kill-buffer` prompt for a buffer name, with empty input killing the current buffer;
- dirty buffers ask for Emacs-style `y-or-n-p` confirmation before `kill-buffer`
  removes them;
- switching buffers preserves each buffer's point; killing the current buffer
  selects the next buffer;
- tests cover buffer reuse, switching, killing, and dirty-buffer confirmation.

Milestone 9 adds windows and splits:

- `window::WindowSet` stores a split tree, stable `WindowId` values, and per-window `Viewport` state;
- `C-x 2` and `split-window-below` split the current window into stacked viewports;
- `C-x 3` and `split-window-right` split the current window into side-by-side viewports;
- `C-x 0` and `delete-window` delete the current window when more than one exists;
- `C-x 1` and `delete-other-windows` collapse back to the selected window;
- `C-x o` and `other-window` cycle through windows and restore each window's cursor;
- terminal drawing lays out all windows, draws one mode line per window, and places point in the selected window;
- horizontal auto-scrolling follows Emacs' default margin-and-recenter behavior
  for clipped long lines;
- layout tests cover horizontal and vertical splitting, deletion, cycling, and per-window viewport state.

Milestone 10 adds region, kill/yank, and undo:

- `C-@` and `set-mark-command` set an active mark at point;
- active regions render through `render::Face::Region` and terminal ANSI
  highlighting, including horizontally clipped long lines and selected line-end
  padding when the region crosses a line break;
- `C-w` and `kill-region` delete the active region into the kill ring;
- `M-w` and `copy-region-as-kill` copy the active region without deleting it;
- `C-y` and `yank` insert the latest kill-ring entry, including coalesced
  consecutive kills;
- `M-y` and `yank-pop` replace the just-yanked range with earlier kill-ring
  entries, wrapping through the ring while repeated immediately after `C-y` or
  `M-y`;
- `C-u` and `universal-argument` supply a numeric argument to the next
  repeatable command, including self-insert, line/character/word movement,
  character deletion, word kills, `C-k`, `C-j`, and `C-o`;
- `C-k` and `kill-line` delete to end of line or delete the line break at end of line;
- `C-j` and `newline-and-indent` insert a newline and leave point at the start
  of the new line in the current plain-text mode;
- `C-o` and `open-line` insert a newline at point without moving point;
- `C-_` and `undo` reverse current-buffer insert/delete/yank/kill operations;
- normal printable typing is grouped into a single undo record until another command interrupts it;
- tests cover Unicode-safe region highlighting, kill/yank, kill-line, grouped typing undo, and new control-key parsing.

Milestone 11 adds query replace:

- `M-%` and `query-replace` start an interactive replacement workflow;
- the minibuffer prompts first for the search string, then for the replacement string;
- the current candidate is highlighted with the current-search face;
- choice keys support `y` to replace, `n` to skip, `!` to replace all remaining candidates, and `q`/Escape/`C-g` to quit;
- replacements are UTF-8-safe and reuse the buffer range validation path;
- each replacement records an undo entry so `C-_` can restore replaced text;
- tests cover UTF-8 replacement, skip/all behavior, missing input, highlighting, and undo.

Milestone 12 cleans up the face and decoration architecture:

- `render::Face` now defines stable priority values for overlapping spans;
- `render::Span` has shared construction and validation helpers;
- `render::DecorationProvider` remains the common line-decoration interface;
- `render::collect_spans_for_line`, `merge_spans`, and `clip_spans` centralize decoration collection, priority merging, and viewport clipping;
- region, incremental-search, and query-replace highlights are implemented as editor decoration providers instead of one ad hoc span builder;
- terminal rendering applies mode-line, minibuffer, warning, and error faces through the same face-to-ANSI path;
- tests cover provider collection, UTF-8 boundary rejection, span priority splitting, clipping, and fixed-width faced terminal output.

Milestone 13 adds syntax highlighting:

- `syntax::Highlighter` defines the line-highlighting interface;
- `syntax::MajorMode` selects Emacs-style major-mode names by file extension, including `Fundamental` fallback and `Text` for `.txt` files;
- `syntax::SyntaxMode` is derived from the major mode for highlighting, with a plain-text fallback;
- simple line-local highlighters cover Rust, C, shell, Markdown, and TOML;
- syntax spans use shared `Face::SyntaxKeyword`, `Face::SyntaxString`, and `Face::SyntaxComment` faces;
- syntax spans flow through the same decoration collection and priority merge path as region, search, and query-replace spans;
- syntax highlighting is enabled by default and can be toggled with `M-x toggle-syntax-highlighting`;
- the mode line displays the major mode in parentheses, independent of whether syntax highlighting is enabled;
- tests cover mode selection, language span output, syntax/search/region merge priority, and the toggle command.

Milestone 14 adds configuration and polish:

- `config::Config` loads `~/.config/rile/config.toml` when present and otherwise uses defaults;
- the config parser supports a small TOML subset with `tab_width`, `fill_column`,
  `line_numbers`, `syntax_highlighting`, `search_highlighting`, and `theme` keys;
- `tab_width` controls terminal tab expansion and cursor column calculation for
  tabs;
- `fill_column` controls the display column used by `fill-paragraph`;
- optional line numbers render in a left gutter with `Face::LineNumber`;
- syntax and search highlighting can start disabled from config and can be toggled with `M-x toggle-syntax-highlighting` and `M-x toggle-search-highlighting`;
- line numbers can be toggled with `M-x toggle-line-numbers`;
- `theme = "default"` keeps colored faces and `theme = "mono"` uses mostly monochrome ANSI emphasis;
- tests cover config parsing, invalid config values, editor option application, toggles, tab expansion, and line-number rendering.

Post-Milestone 14 UX polish adds a clean read-only `*Rile*` welcome buffer for no-file launches, blank unused rows instead of Vim-like `~` markers, and compact mode-line position text such as `All (1,0)` alongside the major mode.

Post-Milestone 14 navigation polish adds `M-g g` and `goto-line` with `line` or
`line:column` minibuffer input, clamping out-of-range targets to the current
buffer bounds. It also adds `M-<` and `M->` for moving to the beginning and end
of the current buffer, `C-v`/PageDown and `M-v`/PageUp for visible-page
scrolling with one-line overlap, repeated `C-l` for cycling point through the
current window's center, top, and bottom, and `M-m` for moving to the current line's first non-whitespace
character or to line end on all-whitespace lines. Pending key prefixes echo the
current sequence with a `C-h` help hint,
and `C-h` opens a generated read-only `*Help*` buffer for that prefix. Help
buffers display `Type q in help window to restore previous buffer.` and `q`
restores the previous buffer in the current window. Attempts to edit a read-only
special buffer report `Buffer is read-only: <buffer>`, and transient messages
clear on the next non-prompt command unless that command writes a new message.

Post-Milestone 14 region polish adds `exchange-point-and-mark` on `C-x C-x`.
It swaps point with the current buffer's mark, reactivates the region, and
reports `No mark set in this buffer` when no mark exists for the current buffer.
`mark-whole-buffer` on `C-x h` moves point to the beginning of the buffer, sets
the mark at the buffer end, activates the region, reports `Mark set`, and does
not modify read-only or writable buffer contents.

Post-Milestone 14 word-kill polish adds `kill-word` on `M-d` and
`backward-kill-word` on `M-Backspace`, using the same Unicode-aware word
boundaries as `M-f` and `M-b`. Consecutive kill commands coalesce into one
kill-ring entry: forward kills append text and backward kills prepend text so
`C-y` restores the killed text in buffer order. A non-kill command or failed kill
breaks the coalescing chain.

Post-Milestone 14 movement polish adds `move-to-window-line-top-bottom` on
`M-r`. It cycles point through the middle, top, and bottom visible text rows in
the current window, places point at column zero, does not scroll, and leaves
buffer text unchanged. `what-cursor-position` on `C-x =` reports the current
one-based line, display column, and byte-oriented point position in the echo
area. Cursor-position and buffer-list size reporting compute serialized byte
lengths from stored lines without materializing a second copy of buffer text.

Post-Milestone 14 paragraph movement polish adds `forward-paragraph` on `M-}`
and `backward-paragraph` on `M-{`. The first subset treats empty lines and lines
containing only spaces, tabs, or formfeed characters as paragraph separators,
supports positive and negative numeric arguments, works in read-only buffers,
and does not mutate buffer text or create undo entries. It intentionally defers
customizable Emacs paragraph regexes, fill prefixes, and mode-specific paragraph
movement functions.

Post-Milestone 14 sentence movement polish adds `forward-sentence` on `M-e` and
`backward-sentence` on `M-a`. The first subset recognizes ASCII `.`, `?`, and
`!` terminators, optional closing quotes or brackets, Emacs's default two-space,
newline, or end-of-buffer sentence boundary rule with optional horizontal space
before newline or end of buffer, and paragraph-boundary stops.
It supports positive and negative numeric arguments, works in read-only buffers,
and does not mutate buffer text or create undo entries. Sentence traversal scans
stored lines directly with constant auxiliary memory, and repeated movement stops
when point no longer changes. It intentionally defers customizable sentence
variables, language-specific rules, and mode-specific sentence functions.

Post-Milestone 14 fill polish adds `fill-paragraph` on `M-q`. The first subset
fills plain-text paragraphs by collapsing internal whitespace and wrapping words
at the configured `fill_column` while preserving blank-line paragraph boundaries.
It also fills each paragraph overlapped by an active region. It respects
read-only buffers and records one undo entry for each command result. It
builds and compares only the affected line range, replaces that range directly,
and stores only that range in undo. Word wrapping and cursor mapping
stream paragraph words without an eager word list or duplicate fill pass. It
intentionally defers buffer-local fill columns, justification, fill prefixes,
CJK/kinsoku handling, mode-specific comment filling, and programmable fill hooks.

Post-Milestone 14 comment polish adds reusable line-comment metadata to major
modes and implements `comment-dwim` on `M-;`, plus `comment-region` and
`uncomment-region` as `M-x` commands. The first subset supports Rust and C `//`
comments and shell/TOML `#` comments. It inserts a current-line comment at
indentation, toggles active regions with `M-;`, skips blank lines for region
operations, respects read-only buffers, and records one undo entry per command.
It intentionally defers block comments, comment-column alignment, comment
killing, delimiter-count prefix behavior, and mode-specific comment styles.

Post-Milestone 14 transpose polish adds `transpose-chars` on `C-t`,
`transpose-words` on `M-t`, and `transpose-lines` on `C-x C-t`. The first
character subset transposes same-line UTF-8 grapheme clusters and handles the
Emacs end-of-line case by swapping the previous two graphemes. The word subset
uses Rile's Unicode-aware word boundaries and preserves punctuation between the
swapped words. The line subset moves the previous line past the current line or
lines. These commands support positive and negative numeric arguments, respect
read-only buffers, and record one undo entry per command. They intentionally
defer zero-argument mark-based transposition. Character transposition scans only
the grapheme boundaries needed to reach point and the target, uses constant
auxiliary metadata, limits replacement and undo strings to the affected range,
and bounds saturated numeric arguments by actual line progress. Newline-free
buffer insertion mutates the stored line directly rather than cloning unrelated
text around the insertion point.

Post-Milestone 14 case-conversion polish adds `downcase-word` on `M-l`,
`upcase-word` on `M-u`, `capitalize-word` on `M-c`, `downcase-region` on
`C-x C-l`, and `upcase-region` on `C-x C-u`. Word commands use Rile's existing
Unicode-aware word boundaries, support positive and negative numeric arguments,
and record one undo entry per command. Region commands preserve point and mark,
keep the adjusted active region, respect read-only buffers, and intentionally do
not implement Emacs disabled-command confirmation.

Post-Milestone 14 whitespace cleanup polish adds `delete-horizontal-space` on
`M-\`, `delete-blank-lines` on `C-x C-o`, `delete-trailing-whitespace` as an
unbound `M-x` command, and `just-one-space` as an unbound `M-x` command.
Horizontal cleanup deletes ASCII spaces and tabs around point, or only before
point with a prefix argument. `just-one-space` leaves a requested number of spaces
around point, and negative arguments also collapse newlines. Blank-line cleanup
uses Rile's space/tab-only blank-line definition, collapses blank runs to one
blank line, deletes isolated blank lines, and deletes following blank lines after
a nonblank line. Trailing cleanup deletes ASCII spaces and tabs at physical line
ends across the whole buffer or within active-region bounds. These commands
respect read-only buffers and record one undo entry for each command result.

Post-Milestone 14 file polish adds `write-file` on `C-x C-w`, prompting with
`Write file: `, saving the current buffer to the entered path, and making that
path the visited file. Empty input reports `Error: missing file name`.

Post-Milestone 14 minibuffer polish adds command completion for `M-x`, file
completion for `C-x C-f`, and buffer-name completion for `C-x b` and `C-x k`.
The completion core is separate from the UI style and supports command-name,
file-name, and buffer-name sources, orderless, prefix, or substring matching,
selected candidate movement with `C-n`/Down and `C-p`/Up, selected candidate
paging with `C-v`/PageDown and `M-v`/PageUp, and Enter acceptance. For
completion prompts, including `M-x`, `C-h f`, `C-h v`, `C-x C-f`, `C-x C-r`,
`C-x i`, `C-x b`, and `C-x k`, Tab inserts the selected candidate and an
explicitly moved selection wins over exact minibuffer text on Enter. Completion
prompts use Vertico-style raw exit:
`M-RET` submits the raw minibuffer input even when a completion candidate is
selected.
Minibuffer prompt editing tracks an input cursor: `C-a`/Home and `C-e`/End move
to input bounds, `C-f`/Right and `C-b`/Left move by grapheme, `M-f` and `M-b`
move by word, insertion occurs at point, Backspace deletes before point, and
`C-d`/Delete deletes after point. `C-k`, `M-d`, `M-Backspace`, and unambiguous
CSI-u `C-Backspace` kill prompt text into the normal kill ring.
Minibuffer completion counters and prompt labels use the minibuffer face, while
editable prompt input and ordinary echo-area messages use the default face;
errors keep the error face.
File prompts initialize the editable minibuffer input to the current buffer's
directory when available, making the base path visible before typing. Long
minibuffer prompt rendering follows the prompt cursor so the tail of deep paths
and filenames stays visible. File completion accepts selected existing candidates
on Enter, descends into selected directories, opens exact typed existing files,
and uses `M-RET` to keep raw missing-file input available when a completion
candidate is selected. Directory candidates descend after Tab insertion, exact
input, explicit selection, or a typed prefix of the selected directory;
substring-only directory matches keep the raw typed path. The default
`completion_matching = "orderless"` uses
component-based matching for non-file prompts: every space-separated component
must match in any order, lowercase components match case-insensitively, `^foo`,
`foo$`, and `^foo$` act as simple literal anchors, and other regexp
metacharacters are literal text. `!foo` negates an orderless component, `=foo`
forces literal matching, and `!=foo` combines both forms for negated literal
matching.
File prompts override the global orderless default with Emacs-style file-category
matching: literal prefixes, word-component partial completion, and substring
matches. Rile's file matching also uses smart case: lowercase input matches
case-insensitively, while uppercase input is case-sensitive.
C-x b empty input switches to the default previous buffer, while C-x k empty
input kills the default current buffer. The default `completion_style =
"vertical"` reserves rows above the minibuffer and shows candidate annotations.
Command completion rows
include the first known key binding in the candidate label, such as
`save-buffer (C-x C-s)`, and keep annotations aligned after the visible label
column. The `completions-buffer` style opens a temporary read-only
`*Completions*` buffer and restores the previous viewport on accept/cancel. The
`ido` style is an experimental compact inline minibuffer display. Supported
completion config keys are `completion_style`, `completion_max_candidates`,
`completion_show_annotations`, and `completion_matching`.

Post-Milestone 14 prompt-history polish adds in-session `M-p` and `M-n` history
navigation for command, file, buffer, write-file, goto-line, rectangle, shell
command, describe-function, describe-variable, incremental-search, and
query-replace minibuffer prompts. Prompt history is stored per prompt kind,
preserves the current draft while navigating, avoids consecutive duplicate
entries, and refreshes completion candidates after recalling history in
completion-enabled prompts. Incremental search records accepted searches when
Enter exits the prompt; canceled searches and invalid regexps are not recorded.
It uses separate literal and regexp histories, shared between forward and
backward search within each kind. Query-replace search and replacement prompts
also support history; literal and regexp query-replace prompts use separate
histories.

Undo dirty-state tracking stores a per-buffer undo save point. Opening, saving,
reverting, and `not-modified` record the current undo depth as clean; undoing
back to that depth clears the modified flag, while undoing past a saved edit
makes the buffer modified again. Undo traversal records active undo sequences as
redoable undo-stack entries when a non-undo command boundary is reached, and
records redo-generated undo entries back as ordinary edits so repeated
undo/redo traversal can return to the original saved text. Saving,
`not-modified`, and undo-sequence finalization also break normal typing undo
grouping so subsequent typing cannot merge into an earlier undo record. Explicit
`undo-only` and `undo-redo` commands are available through `M-x`; `C-/`, `C-x u`,
and selective region undo remain deferred.

Post-Milestone 14 self-documentation work made commands, keymaps, options,
modes, buffers, messages, and runtime metadata inspectable from inside Rile.
The implemented architecture is documented in
[Self-Documentation Architecture](self-documentation.md). Terminal input parsing
uses the original termios erase byte, so `0x08` remains Backspace on
`stty erase ^H` terminals and otherwise works as `C-h`; `M-Backspace` accepts
both `Esc 0x7f` and `Esc 0x08`.

Post-Milestone 14 file polish adds `C-x C-r` / `find-file-read-only`, reusing
the shared file-completion source and relative-path resolution from `C-x C-f`.
Normal file-backed documents now carry an explicit read-only flag. Read-only
file buffers show `RO` in the normal mode line, block editing through the same
read-only guard used for special buffers, and reject save/write-file attempts.
`C-x C-q` / `toggle-read-only` toggles that flag for normal buffers; special
buffers such as `*Rile*`, `*Help*`, `*Completions*`, and `*Buffer List*` remain
structurally read-only.

Post-Milestone 14 buffer polish adds `C-x C-b` / `list-buffers`. It refreshes a
read-only `*Buffer List*` special buffer with `CRM Buffer Size Mode File`
columns and displays it in another window while leaving the original buffer
selected. Repeating `C-x C-b` reuses the existing list window. If the buffer list
window is explicitly selected, `q` closes that window. Buffer names and file
paths render control characters as visible escapes so filesystem metadata is
not written to the terminal as control sequences.

Post-Milestone 14 file polish also adds `C-x i` / `insert-file`, prompting
with `Insert file: ` and using the shared file-completion and relative-path
resolution path. Inserted files use the same UTF-8 and binary-file validation
as file opening, insert at point, mark the current buffer dirty, and record an
undo entry. Empty input reports `Error: missing file name` to match Rile's
existing file prompts, even though base Emacs defaults empty `insert-file` input
to the current file.

Post-Milestone 14 file-state polish adds `revert-buffer` on `C-x C-v` and
`not-modified` as an unbound `M-x` command. Revert reloads file-backed normal
buffers through the same UTF-8 and binary validation as file open, prompts before
discarding dirty contents, preserves the buffer's read-only setting, clears undo
history, and clamps point to the reloaded contents. `not-modified` clears the
dirty flag without writing the file or changing buffer text.

Post-Milestone 14 save polish adds `save-some-buffers` on `C-x s`. It walks
modified file-backed normal buffers in buffer-list order, prompts before each
save, uses the same safe save path as `save-buffer`, and skips special,
read-only, and unnamed buffers.

Post-Milestone 14 file-state polish also adds `auto-revert-mode` and
`global-auto-revert-mode` as unbound `M-x` commands. Auto-revert uses idle input
timeouts to poll file size and modification timestamp changes, reloads only
clean file-backed normal buffers, and never replaces dirty buffer contents.
Metadata and reload failures retain the last valid buffer contents, report a
minibuffer error, and do not stop other watched buffers or the editor session.
Failed buffers retry after a short delay without repeatedly replacing newer
minibuffer messages with the same error. Reloaded buffers drop stale undo records
for that buffer and clamp saved point positions to the new text.

Post-Milestone 14 key-alias polish adds parser support for common F3/F4 escape
sequences and standard Ctrl-modified arrow/Home/End CSI sequences. F3 maps to
`start-kbd-macro`; F4 maps to `kmacro-end-or-call-macro`, which stops the active
macro recording or replays the last macro. Ctrl-Left/Right map to word movement,
Ctrl-Up/Down map to paragraph movement, and Ctrl-Home/End map to buffer
beginning/end.

Post-Milestone 14 terminal polish adds `suspend-frame` on `C-z`. The terminal
session leaves the alternate screen, restores cooked mode, raises `SIGTSTP`, and
re-enters raw mode plus the alternate screen after the process resumes. This is
intended for Unix terminals with job control; automated tests cover command
dispatch without sending a real suspend signal.

kg-inspired optional conveniences are not planned as default behavior unless a
clear Emacs-compatible requirement appears. Shift selection belongs behind an
explicit optional mode, CUA clipboard key aliases belong behind CUA-mode-style
behavior, and electric-pair insertion belongs behind an optional modern editing
mode rather than always-on base editing.

Post-Milestone 14 editing polish adds `C-q` / `quoted-insert`. It waits for the
next key with a `C-q-` minibuffer message, then inserts printable UTF-8 text,
Tab, or Enter literally. NUL and other control, Meta, or special keys are
rejected with explicit errors. Buffer text and other terminal-visible strings
that already contain C0 or C1 controls render them as visible escapes instead of
terminal control sequences. Read-only buffers block quoted insert before
entering the pending quoted state.

Post-Milestone 14 editing polish also adds `M-^` / `join-line`. It joins the
current line to the previous line, trims trailing whitespace before the newline
and leading indentation after it, inserts one separating space when both sides
contain text, places point at the join, and records a single undoable edit.

Post-Milestone 14 macro polish adds keyboard macros with `C-x (` /
`start-kbd-macro`, `C-x )` / `end-kbd-macro`, and `C-x e` /
`call-last-kbd-macro`. Rile records raw `KeyEvent` input after macro definition
starts, trims the terminating `C-x )`, and replays the saved keys through normal
editor key handling so prompt input and command dispatch behave like typed input.
Macro replay skips recording, rejects recursive or nested macro execution, and
honors `C-u` repeat counts before `C-x e`. Emacs-style `C-u 0 C-x e` repeat until
error behavior is deferred; Rile currently treats zero as zero executions.

Post-Milestone 14 rectangle polish adds `C-x SPC` / `rectangle-mark-mode`.
Rectangle mark mode stores a rectangular active region in display columns and
renders it with the normal region face. `C-w` and `M-w` on a rectangle-marked
region save typed rectangle kill-ring entries, and regular `C-y` yanks those
entries back as columns, padding shorter target lines with spaces. The explicit
rectangle command subset `C-x r k`, `C-x r M-w`, `C-x r y`, `C-x r d`,
`C-x r c`, `C-x r o`, `C-x r t`, and `C-x r N` uses mark and point for kill,
copy, yank, delete, clear, open, string replacement, and line numbering
operations. Rectangle kill, yank, delete, clear, open, string replacement, and
numbering operations are undoable as one grouped edit. Register-backed `C-x r`
commands support single printable-character point registers (`C-x r SPC`,
`C-x r j`), text registers (`C-x r s`, `C-x r i`), rectangle registers
(`C-x r r`, `C-x r i`), and number registers (`C-x r n`, `C-x r +`,
`C-x r i`). `M-y` rotation across rectangle entries is deferred.

Post-Milestone 14 shell-command polish adds `M-!` / `shell-command` and `M-|` /
`shell-command-on-region`. Rile runs `/bin/sh -c <command>` synchronously using
the current buffer file's parent directory when file-backed, otherwise the editor
launch directory. No-prefix `M-!` and `M-|` display captured stdout/stderr in a
read-only `*Shell Command Output*` buffer. `C-u M-!` inserts stdout at point, and
`C-u M-|` replaces the active linear region with stdout, but only after a
successful command exit; nonzero exits show output and do not mutate the edited
buffer. Output must decode as UTF-8. V1 deliberately does not support `M-&`,
process cancellation, live process buffers, interactive TTY subprocesses, remote
file handlers, configurable shells, coding-system prompts, or rectangle piping.

Post-Milestone 14 quit polish makes `C-x C-c` protect modified normal buffers.
Clean buffers exit immediately. If any normal buffer has unsaved changes, Rile
prompts `Modified buffers exist; exit anyway? (yes or no) `; `yes` exits and
`no` or `C-g` cancels. Generated special buffers are ignored for this decision.

Current limitations: there is no shell-command process timeout/cancellation, no
message-log retention limit or persistence across sessions, and no selective
region undo yet.
Literal search, regexp incremental search, query replace, regexp query replace,
and `replace-regexp` use Emacs-style smart-case matching: lowercase search text
matches case-insensitively, while unescaped uppercase search text is
case-sensitive. Regexp matching applies that rule to literal atoms, character
classes, ASCII alphabetic ranges, and supported POSIX classes; uppercase regexp
characters escaped with `\` do not make the whole search case-sensitive. Regexp
commands use Rile's built-in line-local subset: `.`, `*`, `+`, `?`, `^`, `$`,
Emacs-style grouping
`\(...\)`, alternation `\|`, counted repetition `\{m\}`, `\{m,\}`, and
`\{m,n\}`, escaped metacharacters, and character classes with ranges and
negation. It also supports word constructs `\<`, `\>`, `\b`, `\B`, `\w`, and
`\W` using Rile's Unicode-aware word-character definition, plus ASCII POSIX
bracket classes `[[:alpha:]]`, `[[:digit:]]`, `[[:alnum:]]`, `[[:space:]]`,
`[[:lower:]]`, and `[[:upper:]]`. Bare `(`, `)`, `{`, `}`, and `|` match
literally. Rile does not implement regexp backreferences, syntax-class escapes,
or multiline regexp matching; current unrecognized regexp escapes match the
escaped character literally unless they are explicitly invalid syntax. Regexp
compilation is bounded to 1,024 pattern characters, 32 captures, 64 nested
groups, and 4,096 VM instructions. Unbounded `*`, `+`, and `\{m,\}` repetition
is rejected when its atom can match empty text, avoiding ambiguous zero-width
cycles in the limited engine. The parser compiles accepted patterns to an
ordered Thompson program, and iterative Pike-style matching bounds work by the
compiled program size and line length instead of recursively backtracking. Regexp
replacement commands expand `\&` to the whole match, `\1` through `\9` to numbered
captures, and `\\` to a literal backslash. Unmatched or missing captures expand to
empty text, unsupported replacement backslash escapes are preserved literally,
and replacement text is case-adapted for case-insensitive matches after regexp
replacement expansion. Patterns that can match empty text are rejected. Search
wraps only after an explicit boundary failure, and replacement commands do not
wrap.

Hi-lock style user highlights are buffer-local and ephemeral. `highlight-regexp`,
`highlight-phrase`, and `highlight-lines-matching-regexp` store persistent
patterns for the current buffer and render them through the shared decoration
path. `highlight-phrase` folds spaces and tabs to `[ \t]+` before compiling the
pattern. Highlight commands prompt with `Highlight using face (default NAME):`
and attach the existing completion UI to a small Emacs-named face palette backed
by Rile render faces. `unhighlight-regexp` pre-fills an active highlight pattern,
preferring a highlight at point and otherwise the most recent pattern, and removes
entries whose original prompt text exactly matches the submitted input.
Submitting a blank unhighlight prompt accepts the stored default pattern, not
all-removal.
Universal-argument `unhighlight-regexp` removes all current-buffer highlights.
Rile does not yet persist `Hi-lock:` file comments, complete arbitrary face-name
support, or support subexpression-only highlighting.

Milestone 15 hardening has started with binary-file detection: files containing
NUL bytes are rejected before UTF-8 decoding so accidental binary opens fail
with an explicit message.  Backups remain disabled by default.  The optional
`backup_on_save = true` config setting writes one persistent backup per buffer
visit before the first successful save of an existing file.  Empty
`backup_directory` uses a sibling `file~` backup; a configured backup directory
uses mapped path-based names and is checked when the backup is written.  Backup
creation failures block the save so the original file contents remain intact.
Auto-save is a separate default-off feature.  When `auto_save = true`, dirty
file-visiting buffers write Emacs-style `#file#` auto-save files after the
configured handled-key interval or idle timeout.  Auto-save writes do not mark
buffers clean and do not modify visited files.  On Unix, recovery files inherit
visited-file permissions and rewrites intersect those permissions with any
existing recovery-file mode so auto-save cannot broaden access.  Explicit saves
delete matching current-session auto-save files by default while preserving
pre-existing recovery files, and opening a file with a newer auto-save file
emits a warning so the auto-save file can be opened manually for recovery.

Visual terminal testing has started with `--visual-test` and `--test-size
WIDTHxHEIGHT`. Visual-test mode uses default config instead of user config and
renders deterministic mode-line text for PTY, snapshot, and VHS review. PTY
tests normally assert parsed `vt100` screen state; targeted security tests also
inspect retained raw output for untrusted control sequences.

## Line Ending Policy

The in-memory buffer model uses `\n` as the only line separator. `Buffer::from_text` splits on `\n`, `Buffer::serialize` joins lines with `\n`, and `Buffer::final_newline` records whether the serialized text ends with a final newline.

The current file policy preserves carriage return bytes as ordinary text. CRLF
files therefore round-trip as CRLF when saved without editing those line
endings, while newly inserted line breaks use `\n`. Preserved carriage returns
display as visible `\r` escapes. A later polishing milestone can add explicit
line-ending detection and conversion controls if needed.

## Save Safety

Saves use a same-directory temporary file followed by `rename`, then best-effort
parent-directory sync.  This is intended to avoid partially written target files
on common Unix filesystems.  Rile preserves existing file permissions across the
replacement and retries stale temporary-name collisions before failing.
Permission, directory, and missing-parent errors propagate as `I/O error` values
and failed saves keep the buffer dirty.

## Terminal Decision

Rile currently uses direct Unix termios and ANSI escape sequences with only the `libc` crate for platform bindings. This keeps behavior explicit and dependency count low while the terminal model is still small. A higher-level terminal crate can be reconsidered later if portability or feature needs outweigh the extra abstraction.

## License

Rile is copyrighted by Robert Charusta <rch-public@posteo.net> and licensed as `GPL-3.0-or-later`. Keep `COPYING` as the canonical GPLv3 license text and add SPDX identifiers to new source and documentation files.

## Release History Files

Maintain `NEWS` for user-visible release notes, with newest releases first. Keep entries concise and focused on behavior users need to know about.

Maintain `ChangeLog` in GNU-style plain text for file-level maintenance history, with newest entries first. Git remains the detailed history; `ChangeLog` should summarize coherent changes rather than mechanically duplicating every commit.

Release publishing uses the installed `release-tools` CLI and GoReleaser from
the dev container.  See [Release Checklist](release-checklist.md) for the
containerized release flow, version checks, tag command, and publish command.

## Testing

See [Testing Guide](testing.md) for unit, integration, PTY, parsed-screen snapshot, and optional VHS visual-review workflows.

In short, `make verify` is the canonical quality gate. PTY tests assert parsed VT100 screen state from the real `rile` binary, parsed-screen snapshots live under `tests/snapshots/`, and optional GIF/PNG visual artifacts under ignored `artifacts/` are review evidence rather than the pass/fail oracle.

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

The dev container in `Containerfile.dev` provides:

| Tool | Purpose | Required |
| --- | --- | --- |
| `rustup` | Toolchain and component management. | Yes |
| `cargo` | Build, test, run, and package commands. | Yes |
| `rustfmt` | Rust formatting checks. | Yes |
| `clippy` | Rust lint checks. | Yes |
| `rust-analyzer` | Editor/LSP support. | Useful, not part of `verify` |
| `cargo-nextest` | Preferred test runner. | Yes, with `cargo test` fallback in `scripts/test` |
| `cargo-insta` | Parsed-screen snapshot checks. | Yes, used by `make verify` |
| `cargo-deny` | License, advisory, source, and dependency policy checks. | Yes |
| `cargo-audit` | Security advisory checks. | Yes |
| `cargo-machete` | Unused dependency detection. | Yes |

Current host status in this workspace: `cargo`, `podman`, and `make` are available; `rustup`, `rustfmt`, clippy, `rust-analyzer`, `cargo-nextest`, `cargo-insta`, `cargo-deny`, `cargo-audit`, and `cargo-machete` are not. That is why the dev container is the canonical tooling environment.

The visual tooling container in `Containerfile.visual` provides `vhs`, `ttyd`, `ffmpeg`, Chromium, and Rust for optional visual artifact generation. It is separate from the normal dev container so `make verify` stays smaller, faster, and independent of browser/video tooling.

## Dev Container Workflow

The dev image is defined in `Containerfile.dev`. It intentionally uses that name instead of plain `Containerfile` so tooling images are not confused with future runtime images.

Interactive development:

```sh
make shell
```

`make shell` sets Podman's interactive detach key sequence to `Ctrl-]` so
Emacs-style `C-p` movement reaches terminal editors instead of being held as
the first byte of Podman's default `Ctrl-p Ctrl-q` detach sequence. Override it
with `PODMAN_DETACH_KEYS`, for example `PODMAN_DETACH_KEYS=ctrl-^ make shell`.

One-shot tasks:

```sh
make build
make fmt
make fmt-check
make test
make snapshot-test
make lint
make audit
make unused-deps
make verify
make visual-demos
make visual-frames
```

The Makefile delegates to scripts:

- `scripts/devshell` opens an interactive shell in the dev container.
- `scripts/in-container` runs one command in a fresh dev container.
- `scripts/build` runs `cargo build --locked`.
- `scripts/fmt` runs `cargo fmt` and updates Rust source formatting.
- `scripts/fmt-check` runs `cargo fmt --check` without modifying files.
- `scripts/test` runs `cargo nextest run --locked` when available, otherwise `cargo test --locked`.
- `scripts/test-cargo` always runs `cargo test --locked`.
- `scripts/snapshot-test` runs check-only parsed-screen snapshot tests through `cargo insta test`.
- `scripts/lint` runs `scripts/fmt-check` and `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- `scripts/audit` runs `cargo deny check` and `cargo audit`.
- `scripts/unused-deps` runs `cargo machete`.
- `scripts/verify` runs build, tests, snapshot checks, lint, audit, and unused dependency checks.
- `scripts/visual-demos` validates VHS tapes, builds Rile once, and records optional GIFs.
- `scripts/visual-frames` regenerates visual demos and verifies named PNG screenshots.
- `scripts/tools` prints the versions of expected tools.

`cargo-deny` reads policy from `deny.toml`. The current policy denies yanked crates, denies unknown registries and git sources, denies wildcard dependencies, warns on multiple dependency versions, and allows Rile's GPL license plus the permissive licenses used by current dependencies.

## Direct Host Workflow

Direct host development is supported if the same tools are installed locally. Use scripts directly:

```sh
./scripts/build
./scripts/fmt
./scripts/fmt-check
./scripts/test-cargo
./scripts/snapshot-test
./scripts/lint
./scripts/audit
./scripts/unused-deps
```

On this host, only `./scripts/build` and `./scripts/test-cargo` are expected to work until the missing Rust components and cargo subcommands are installed.

## CI Status

CI is deferred until Forgejo CI is configured for the official repository.
Future CI should call the same non-interactive scripts used by `make verify`;
it should not call `scripts/devshell`.

Optional hosted CI visual artifact generation should be a separate, non-blocking job from `make verify`. That job may run `make visual-frames` in the visual tooling container and upload ignored files from `artifacts/` for review. GIFs and PNGs should remain review evidence only; PTY assertions and parsed-screen snapshots remain the correctness gates.
