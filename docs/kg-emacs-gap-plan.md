<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# kg And Emacs Gap Plan

This plan tracks kg command gaps that are useful for Rile, but Emacs remains the
behavior reference. kg is evidence for small-editor coverage and terminal UX;
it is not a source-porting guide or a behavior oracle.

## Rules

- Prefer base terminal GNU Emacs behavior for command names, keys, prompts,
  prefix arguments, read-only handling, messages, and undo behavior.
- Use modern Emacs evidence only for modern completion or optional-mode UX.
- Preserve Rile's stronger existing behavior when kg is narrower.
- Treat kg-only conveniences as optional unless they match default Emacs behavior
  or have a concrete user need.
- Cite behavior evidence in docs or tests before implementing unclear commands.

## Gap Matrix

| kg feature | kg entry point | Emacs reference | Rile target |
| --- | --- | --- | --- |
| Cursor position report | `M-x what-cursor-position` | `what-cursor-position`, usually `C-x =` | Implement with Emacs-style command name and Rile-specific UTF-8 position details. |
| Window-line movement | `M-r` | `move-to-window-line-top-bottom`, `M-r` | Implement `M-r`, preserving Rile window/view state. |
| Revert current buffer | `M-x revert-buffer` | `revert-buffer` | Implemented for file-backed normal buffers with dirty confirmation. |
| Clear dirty flag | `M-x not-modified` | `not-modified` | Implemented without changing file contents or undo history. |
| Save modified buffers | `C-x s` | `save-some-buffers` | Implemented as per-buffer prompting; skips special, read-only, and unnamed buffers. |
| Per-buffer auto revert | `M-x auto-revert-mode` | `auto-revert-mode` | Implemented for clean file-backed buffers; never discards dirty edits. |
| Global auto revert | `M-x global-auto-revert-mode` | `global-auto-revert-mode` | Implemented as an editor-wide toggle reusing the clean-buffer safety rule. |
| Current-line whitespace cleanup | `M-x delete-trailing-space` | no default priority over `delete-trailing-whitespace` | Defer unless a clear Emacs-compatible command target is chosen. |
| kg whole-buffer whitespace cleanup | `M-x whitespace-cleanup` | `delete-trailing-whitespace` | Already covered by Rile's `delete-trailing-whitespace`; keep Emacs name. |
| Version display | `M-x version` | `emacs-version`, `about-emacs` | Already covered differently by `about-rile`; keep Rile command. |
| Suspend editor | `C-z` | `suspend-frame`, `C-z` on capable terminals | Implemented with raw-mode/alternate-screen restore around `SIGTSTP`. |
| Keyboard macro aliases | `F3`, `F4` | `kmacro-start-macro-or-insert-counter`, `kmacro-end-or-call-macro` | Implemented for common terminal F3/F4 sequences. |
| Ctrl-arrow aliases | Ctrl arrows/Home/End | terminal and Emacs dependent | Implemented for standard Ctrl-modified CSI sequences. |
| Shift selection | Shift arrows/Home/End | `shift-select-mode` | Defer as optional-mode work; do not make a kg-only default. |
| CUA aliases | `S-Del`, `C-Ins`, `S-Ins` | `cua-mode` | Defer; not base Emacs default. |
| Auto-pair insertion | normal typing | `electric-pair-mode` | Defer as optional modern mode; do not enable by default. |

## Implementation Order

- [x] Add `what-cursor-position` and `move-to-window-line-top-bottom`.
- [x] Add `revert-buffer` and `not-modified`.
- [x] Add `save-some-buffers` on `C-x s`.
- [x] Add safe `auto-revert-mode` and `global-auto-revert-mode`.
- [x] Add safe default-compatible key aliases after input parser tests.
- [ ] Revisit optional modes: shift selection, CUA aliases, and electric pair.

## Preserved Differences

- Keep Rile's `C-h` prefix and self-documentation commands instead of kg's static
  help-only behavior.
- Keep Rile's richer completion UI and matching instead of kg's inline picker.
- Keep `C-x k` buffer completion instead of kg's direct current-buffer kill.
- Keep Rile's register and rectangle superset.
- Keep Rile's safer shell-command default that shows output in a special buffer;
  `C-u` keeps insert/replace behavior available.
