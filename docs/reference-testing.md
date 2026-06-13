<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Reference Testing

Rile includes optional reference-testing tooling for studying user-visible behavior of other terminal editors. Current reference targets are GNU Zile and kg.

Reference testing is not part of Rile's normal quality gate. It is a way to produce behavior evidence before writing original Rile requirements and tests.

## Licensing And Provenance

The reference-testing tooling in this repository is original Rile project material. It does not vendor, incorporate, copy, translate, or mechanically port GNU Zile or kg source code.

The Zile reference tooling may download a pinned upstream GNU Zile release into ignored local artifacts. Zile is GPL-3.0-or-later software from the GNU Project. Any local downloaded source tree keeps its upstream license files and notices in `artifacts/reference/zile/`, which is ignored by Git.

The kg reference tooling may clone a pinned upstream kg commit into ignored local artifacts. kg is BSD-2-Clause software by its upstream authors. Any local cloned source tree keeps its upstream license files and notices in `artifacts/reference/kg/`, which is ignored by Git.

Generated screenshots, GIFs, temporary files, downloaded tarballs, extracted sources, and installed reference binaries are review evidence only. They are ignored local artifacts unless explicitly distributed with their required upstream notices.

Use reference-editor behavior as evidence for original Rile requirements. Do not copy reference implementation code into Rile.

## Directory Layout

Committed tooling lives under:

```text
tools/reference/zile/
tools/reference/kg/
```

Ignored generated outputs live under:

```text
artifacts/reference/zile/
artifacts/reference/kg/
```

Scenario files under `tools/reference/<editor>/scenarios/` are original Rile project scenario definitions. They describe fixtures, terminal sizes, keystrokes, and frame names for visual behavior capture. The capture scripts apply each scenario's `WIDTH` and `HEIGHT` with `stty cols` and `stty rows` before launching the reference editor.

## Build The Zile Reference

Build the reference container and download/build the pinned Zile release:

```sh
tools/reference/zile/build
```

The script verifies the pinned release tarball checksum before extracting and building it. The installed reference binary is written under:

```text
artifacts/reference/zile/install/bin/zile
```

## Build The kg Reference

Build the reference container and clone/build the pinned kg commit:

```sh
tools/reference/kg/build
```

The installed reference binary is written under:

```text
artifacts/reference/kg/install/bin/kg
```

## Capture A Scenario

Capture the smoke scenario:

```sh
tools/reference/zile/capture smoke-open
```

Capture another scenario:

```sh
tools/reference/zile/capture open-line
```

Capture a kg scenario:

```sh
tools/reference/kg/capture baseline-ui
```

Capture outputs are written under:

```text
artifacts/reference/zile/captures/<scenario>/
artifacts/reference/kg/captures/<scenario>/
```

Each capture directory may include:

- generated fixture files;
- generated VHS tape files;
- named PNG frames;
- optional GIF output;
- a temporary `HOME` used by the reference editor run.

## Scenario Format

Scenarios are Bash fragments sourced by the capture script. Keep them simple and deterministic:

```sh
SCENARIO_NAME=smoke-open
SCENARIO_DESCRIPTION='Open a fixture and quit.'
WIDTH=100
HEIGHT=24
FIXTURE_NAME=smoke-open.txt
FIXTURE_CONTENT=$'alpha\nbeta\n'

vhs_steps() {
  cat <<'EOF'
Wait+Screen /alpha/
Screenshot {{FRAME_DIR}}/00-open.png
Ctrl+X
Ctrl+C
EOF
}
```

Supported placeholders in `vhs_steps` output:

- `{{FRAME_DIR}}`: capture frame directory.
- `{{FIXTURE}}`: generated fixture path.
- `{{ZILE}}`: installed reference Zile binary.
- `{{KG}}`: installed reference kg binary.
- `{{HOME}}`: temporary home directory for the scenario.

## How To Use Evidence

For each feature scenario:

- Capture reference-editor frames with fixed fixture text and terminal size.
- Inspect the screenshots and write a short behavior summary.
- Turn that summary into original Rile requirements and acceptable differences.
- Add Rile unit tests, PTY tests, parsed-screen snapshots, or optional Rile VHS demos as appropriate.
- Verify Rile with `make verify` before committing feature work.

Screenshots are evidence, not the pass/fail oracle. Rile's automated PTY tests and parsed-screen snapshots remain the correctness gates.
