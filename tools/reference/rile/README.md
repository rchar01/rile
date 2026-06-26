<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Rile Reference Captures

This directory contains first-party Rile behavior-capture scenarios. They run the
current Rile binary with deterministic visual-test flags and write ignored VHS
artifacts under `artifacts/reference/rile/`.

Rile captures are comparison evidence for hardening existing behavior. They are
not an external behavior source and are not a correctness oracle; automated PTY
tests and parsed-screen snapshots remain the correctness gates.

Capture one scenario:

```sh
make reference-capture REF_EDITOR=rile REF_SCENARIO=open-line
```

Capture all Rile scenarios:

```sh
make reference-capture-all REF_EDITOR=rile
```

The wrapper uses the repository `Containerfile.visual` so VHS, ttyd, Chromium,
and Rust are available in one container. Each run builds an ignored capture
binary under `artifacts/reference/rile/build/target/` and launches it as:

```sh
artifacts/reference/rile/build/target/debug/rile --visual-test --test-size WIDTHxHEIGHT <fixture>
```

Navigation comparison scenarios:

- `long-line`: Rile long-line markers, region faces, help continuation, and
  horizontal movement frames aligned with the Emacs core and Zile scenarios.
- `recenter`: Rile repeated `C-l` recenter behavior in short and long buffers,
  aligned with the Emacs core `recenter-core` scenario.

Completion comparison scenarios:

- `m-x-completion-modern`: Rile command completion frames aligned with the
  Emacs modern `M-x` scenario, plus Rile Orderless component and regexp frames.
- `find-file-completion-modern`: Rile file completion frames aligned with the
  Emacs modern file scenario, plus a Rile arbitrary-substring rejection frame.
- `minibuffer-navigation-modern`: Rile minibuffer history, line movement, and
  page movement frames aligned with the Emacs modern scenario.
- `buffer-completion`: Rile switch-buffer completion behavior.
- `kill-buffer-completion`: Rile kill-buffer completion and dirty-buffer
  confirmation behavior.
