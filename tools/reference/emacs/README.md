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
- `join-line-core`: base Emacs `M-^` join-line behavior.
- `keyboard-macro-core`: base Emacs keyboard macro and repeat behavior.
- `list-buffers-core`: base Emacs `C-x C-b` list-buffers behavior.
- `mark-whole-buffer-core`: base Emacs `C-x h` mark-whole-buffer behavior.
- `newline-and-indent-core`: base Emacs `C-j` newline-and-indent behavior.
- `quoted-insert-core`: base Emacs `C-q` quoted-insert behavior.
- `rectangle-mark-core`: base Emacs `C-x SPC` rectangle mark mode with regular
  `M-w` copy and `C-y` yank behavior.
- `yank-pop-core`: base Emacs `M-y` yank-pop behavior.
- `m-x-completion-core`: base Emacs `M-x` completion behavior.
- `m-x-completion-modern`: Vertico/Marginalia `M-x` completion behavior.
- `find-file-completion-modern`: Vertico/Marginalia file completion behavior.
- `incremental-search-wrap-core`: base Emacs incremental search wrapping behavior.
- `insert-file-core`: base Emacs `C-x i` insert-file behavior.
- `buffer-completion-modern`: Vertico/Marginalia buffer completion behavior.
- `prompt-history-modern`: Vertico/Marginalia minibuffer history behavior.
- `help-describe-core`: base Emacs `C-h k` and `C-h f` help behavior.
- `read-only-open-core`: base Emacs `C-x C-r` read-only file open behavior.
- `toggle-read-only-core`: base Emacs `C-x C-q` toggle-read-only behavior.
- `universal-argument-core`: base Emacs `C-u` universal-argument behavior.

Scenario files may define `setup_reference_files CAPTURE_DIR CAPTURE_REL` when a
capture needs extra files inside its ignored artifact directory.

See `docs/reference-testing.md` for the workflow and licensing/provenance rules.
