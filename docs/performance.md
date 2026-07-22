<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Performance Smoke Testing

Rile includes optional performance smoke tooling for comparing terminal editor
open and navigation latency on large files and long lines. These tests are not a
correctness gate and are not part of `make verify`.

Run the default smoke suite:

```sh
make perf-smoke
```

The default editor set is Rile, GNU Emacs, GNU Zile, kg, and `vi` from Debian's
`vim-tiny` package. Override it with a comma-separated list:

```sh
make perf-smoke PERF_EDITORS=rile,emacs,zile
```

The default suite runs three repetitions of these cases:

- open a roughly 52 MB file with 500,000 normal-width lines;
- open a single-line file with a 100,000-column line;
- redraw at column zero on that 100,000-column line;
- move to the end of that 100,000-column line;
- open and redraw a 100,000-column Rust line with repeated keywords;
- send a burst of page-down commands in a 100,000-line file.

Run larger cases with:

```sh
make perf-smoke PERF_LEVEL=full
```

`PERF_LEVEL=full` adds a roughly 208 MB normal-line file plus redraw and
end-of-line cases on a 1,000,000-column line. Use it only when the machine has
enough memory and the longer runtime is acceptable.

After a successful run has built the reference editor binaries, reuse them by
skipping the reference build step:

```sh
make perf-smoke PERF_SKIP_REFERENCE_BUILD=1
```

Do not use `PERF_SKIP_REFERENCE_BUILD=1` on a fresh checkout or after deleting
`artifacts/reference/`; the harness expects the Emacs, Zile, and kg binaries to
already exist.

Generated fixtures, raw timing output, JSONL records, and Markdown summaries are
written under ignored `artifacts/perf/` paths. The summary file reports median
open latency, median operation latency where applicable, and median maximum RSS
from GNU `time -v`.

The harness measures terminal behavior through a pseudo-terminal with a fixed
`100x24` size. It starts each editor in a temporary home directory, waits for
expected screen text, sends editor-specific quit and movement keys, and records
timeouts as failures. The Emacs command disables its interactive large-file
warning so open latency measures rendering rather than a confirmation prompt.
`vi` is included as a practical terminal-editor baseline; it is not treated as
an Emacs-like behavior reference.

Interpret the results as local baseline evidence, not a stable benchmark suite.
Use them to identify obvious regressions or decide whether deeper profiling is
needed before changing the buffer storage model. The plain-text long-line cases
exercise viewport projection, while the Rust cases separately exercise syntax
span generation and priority merging.
