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
