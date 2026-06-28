<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Emacs Reference Tooling

This directory contains optional tooling for capturing user-visible terminal
GNU Emacs behavior as reference evidence for Rile feature work.

The tooling uses Debian-packaged Emacs and ELPA packages inside a container. It
does not vendor Emacs, Vertico, or Marginalia source into Rile and must not be
used to copy, translate, or mechanically port their implementation code.

## Profiles

- `core`: stages `early-init.el` and an empty `init.el` in the scenario home,
  then runs `emacs -nw`; use this for base GNU Emacs behavior with the same
  minimal startup UI as other Emacs captures.
- `modern`: stages `early-init.el` and `init.el` in the scenario home, then runs
  `emacs -nw` with Vertico and Marginalia; use this for modern completion UX
  evidence only, not as canonical base Emacs behavior.

## Commands

Build the reference environment wrappers and provenance files:

```sh
tools/reference/emacs/build
```

Capture a scenario:

```sh
tools/reference/emacs/capture m-x-completion-modern
```

Outputs are written under `artifacts/reference/emacs/`, which is ignored by Git.

Initial inspection scenarios:

- `baseline-ui-core`: core Emacs screen layout with reference early init.
- `baseline-ui-modern`: modern Emacs screen layout with Vertico/Marginalia.
- `modeline-state-core`: core Emacs mode-line clean, modified, saved,
  read-only, and writable states.
- `back-to-indentation-core`: base Emacs `M-m` back-to-indentation behavior.
- `consecutive-kills-core`: base Emacs consecutive kill and yank behavior.
- `dirty-buffer-quit-clean-core`: base Emacs `C-x C-c` behavior with a clean
  buffer.
- `dirty-buffer-quit-modified-core`: base Emacs `C-x C-c` save, cancel, and
  exit-anyway prompts with a modified buffer.
- `join-line-core`: base Emacs `M-^` join-line behavior.
- `keyboard-macro-core`: base Emacs keyboard macro and repeat behavior.
- `list-buffers-core`: base Emacs `C-x C-b` list-buffers behavior.
- `long-line-core`: base Emacs long-line markers, region faces, and movement
  behavior in narrow windows.
- `long-line-hscroll-core`: base Emacs truncated long-line horizontal
  auto-scrolling with `C-f`/`C-b` and `M-f`/`M-b`.
- `mark-whole-buffer-core`: base Emacs `C-x h` mark-whole-buffer behavior.
- `newline-and-indent-core`: base Emacs `C-j` newline-and-indent behavior.
- `quoted-insert-core`: base Emacs `C-q` quoted-insert behavior.
- `query-replace-core`: base Emacs `M-%` query-replace prompts, choices, and
  completion status.
- `recenter-core`: base Emacs repeated `C-l` recenter behavior in short and
  long buffers.
- `rectangle-commands-core`: base Emacs explicit `C-x r` rectangle copy, yank,
  kill, delete, clear, and open behavior.
- `rectangle-mark-core`: base Emacs `C-x SPC` rectangle mark mode with regular
  `M-w` copy and `C-y` yank behavior.
- `rectangle-string-number-core`: base Emacs `string-rectangle` and
  `rectangle-number-lines` behavior.
- `registers-core`: base Emacs point, text, rectangle, and number register
  behavior.
- `shell-command-core`: base Emacs `M-!` and `M-|` shell-command behavior.
- `shell-command-region-replace-core`: focused base Emacs `C-u M-|` region
  replacement behavior.
- `yank-pop-core`: base Emacs `M-y` yank-pop behavior.
- `m-x-completion-core`: base Emacs `M-x` completion behavior.
- `m-x-completion-modern`: Vertico/Marginalia `M-x` completion behavior.
- `describe-function-completion-modern`: Vertico/Marginalia `C-h f` command
  completion behavior.
- `describe-variable-completion-modern`: Vertico/Marginalia `C-h v` variable
  completion behavior.
- `find-file-completion-modern`: Vertico/Marginalia file completion behavior.
- `find-file-long-path-core`: base Emacs `C-x C-f` prompt display with a long
  current directory.
- `incremental-search-wrap-core`: base Emacs incremental search wrapping behavior.
- `insert-file-core`: base Emacs `C-x i` insert-file behavior.
- `buffer-completion-modern`: Vertico/Marginalia buffer completion behavior.
- `kill-buffer-completion-modern`: Vertico/Marginalia `C-x k` completion,
  default-buffer, and y-or-n-p dirty-buffer confirmation behavior.
- `prompt-history-modern`: Vertico/Marginalia minibuffer history behavior.
- `minibuffer-navigation-modern`: Vertico/Marginalia minibuffer history,
  line movement, and page movement behavior.
- `help-describe-core`: base Emacs `C-h k` and `C-h f` help behavior.
- `read-only-open-core`: base Emacs `C-x C-r` read-only file open behavior.
- `toggle-read-only-core`: base Emacs `C-x C-q` toggle-read-only behavior.
- `universal-argument-core`: base Emacs `C-u` universal-argument behavior.

Scenario files may define `setup_reference_files CAPTURE_DIR CAPTURE_REL` when a
capture needs extra files inside its ignored artifact directory.

See `docs/reference-testing.md` for the workflow and licensing/provenance rules.
