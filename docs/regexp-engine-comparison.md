<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Regexp Engine Comparison

This note records the trade-off between replacing Rile's recursive regexp
matcher with a safe built-in engine and using the Rust
[`regex`](https://docs.rs/regex/1.12.4/regex/) crate. It exists so the built-in
approach can be reconsidered if its implementation or binary cost stops being
smaller than the dependency-backed alternative.

The current recursive backtracking matcher is not an acceptable final option.
It can take exponential time, exhaust memory, or overflow the stack on crafted
patterns and lines. The built-in option below therefore means an iterative,
priority-aware Thompson/Pike VM or an equivalent bounded design, not adding a
work limit to the existing recursion.

## Working Decision

Prefer a safe built-in VM while it remains materially smaller and preserves
Rile's existing regexp behavior without excessive complexity. Reconsider the
`regex` crate when the built-in implementation crosses any of the review
thresholds below.

This is an engineering direction, not a permanent architecture constraint.
Security, predictable matching, and maintainability take precedence over
avoiding one dependency.

## Comparison

| Area | Safe Built-In VM | `regex` Crate |
| --- | --- | --- |
| Worst-case matching | Must be designed and proved bounded | Worst-case `O(m * n)` matching is an upstream guarantee |
| Stack safety | Requires an iterative matcher and bounded compilation | Matching is not based on unbounded recursive backtracking |
| Pattern limits | Must be designed, surfaced, and tested by Rile | Compiled-size and nesting limits are provided by `RegexBuilder` |
| Current syntax | Parser and AST can remain unchanged | Requires a compatibility layer that includes Emacs-to-Rust regexp translation |
| Word semantics | Preserves Rile's alphanumeric-plus-underscore definition | Unicode `\w` and boundaries are broader than Rile's definition |
| Captures | Requires priority-aware tagged threads | Capture ranges are provided by the crate |
| Backward search | Existing Rile wrapper remains | Rile still needs a backward-search wrapper |
| Line-local search | Existing behavior remains | Rile must continue invoking the engine per line |
| Replacement syntax | Existing `\&`, `\1` through `\9`, and `\\` handling remains | Rile should retain its replacement expander |
| Incremental compilation | Small custom parser and compiler | More expensive compilation on each prompt update |
| Runtime optimization | Rile owns all optimization work | Mature literal, DFA, one-pass, bounded-backtracker, and Pike VM paths are available through features |
| Fuzzing and review | Rile owns the complete security burden | Upstream has extensive tests, fuzzing, and OSS-Fuzz coverage |
| Runtime dependencies | No new dependency | Adds `regex`, `regex-automata`, and `regex-syntax`; performance features also add `aho-corasick` and `memchr` |
| Maintenance | Rile maintains parser, compiler, VM, captures, and Unicode behavior | Rile maintains only its compatibility layer and editor-specific policies |

## Compatibility Work for `regex`

The crate is not a drop-in replacement. A compatibility layer would need to:

- translate Emacs-style `\(...\)`, `\|`, and `\{m,n\}` operators;
- escape bare `(`, `)`, `{`, `}`, and `|`, which Rile treats literally;
- preserve unsupported escapes as literals where Rile currently does so;
- select Unicode-aware case-insensitive compilation according to Rile's smart
  case rule;
- preserve `\<`, `\>`, `\b`, `\B`, `\w`, and `\W`, or deliberately adopt the
  crate's broader Unicode word definition and document the behavior change;
- preserve line-local, backward-search, zero-width, and non-overlapping match
  behavior; and
- keep Rile's existing regexp replacement expansion.

Most existing conformance, capture, replacement, editor, and PTY tests should
be reusable against either backend. A crate migration would additionally need
translator-focused tests and explicit coverage for every accepted escape.

## Size Baseline

The [published v0.9.0 Linux amd64
release](https://codeberg.org/rch/rile/releases/tag/v0.9.0) reports the raw
`rile_0.9.0_linux_amd64` artifact as 1,458,144 bytes, or 1.39 MiB. That release
uses Cargo's normal release profile. The current normal dependency graph
contains only `libc`, `unicode-segmentation`, and `unicode-width`.

The lockfile already contains `regex` through development dependencies, but the
crate is not linked into the release executable. Its presence in `Cargo.lock`
therefore does not reduce the runtime binary cost of adopting it.

No exact Rile comparison build has been made yet. Published upstream
measurements from the [`regex` size-reduction
work](https://github.com/rust-lang/regex/pull/613) provide only planning ranges:

| Candidate | Planning Change From Current Binary | Planning Final Size |
| --- | ---: | ---: |
| Safe built-in VM replacing the current matcher | Decision target: no more than 0.1 MiB | Target: no more than about 1.5 MiB |
| `regex` with only `std` | Historical upstream range: about 0.3-0.6 MiB | Estimate: about 1.7-2.0 MiB |
| `regex` with selected Unicode and performance features | Derived estimate: about 0.5-0.9 MiB | Estimate: about 1.9-2.3 MiB |
| `regex` with all default features | Historical upper estimate: up to about 1.3 MiB | Estimate: about 2.7 MiB |

Only the v0.9.0 baseline is a Rile measurement. The built-in number is a decision
target, and every `regex` number is a planning estimate derived from older
upstream measurements. Final size depends on the Rust version, target, linker,
feature set, release profile, and dead-code elimination. A tailored Rile build
likely needs Unicode case folding and word support, so the `std`-only figure is
a lower bound rather than the expected configuration.

## Review Thresholds

Stop and compare against a real `regex` prototype when any of these conditions
is met:

- the built-in matcher remains recursive or cannot demonstrate bounded work;
- capture priority, greedy matching, zero-width loops, or counted repetition
  require special cases that are difficult to reason about or fuzz;
- the custom implementation takes more than roughly two focused engineering
  weeks to reach complete conformance and pathological-input coverage;
- production matcher/compiler code grows beyond the size of a straightforward
  `regex` compatibility layer;
- the built-in release executable is less than 0.2 MiB smaller than the
  feature-tailored `regex` executable;
- normal patterns or visible-line highlighting are materially slower than the
  crate-backed implementation; or
- future regexp features would require another substantial VM change.

The 0.2 MiB threshold is deliberately stricter than "custom is one byte
smaller." Below that saving, maintaining a security-sensitive engine is not a
good trade for the small absolute binary reduction.

## Measurement Procedure

Before making the final backend decision, build these variants from the same
commit with the same pinned toolchain and target:

1. The current matcher as a baseline.
2. The safe built-in VM.
3. `regex` with the minimum features needed for current Rile semantics.
4. `regex` with default features as an upper bound.

For every variant, record:

- raw and stripped executable bytes;
- compressed release-asset bytes;
- text, data, and BSS section sizes;
- per-crate contribution from `cargo bloat --release --crates`;
- clean release build time;
- incremental compile latency for short and long prompt input;
- forward, backward, replacement, and highlight latency on ordinary files; and
- completion time and peak memory for known pathological patterns.

Useful pathological cases include optional-atom chains followed by a failing
suffix, ambiguous repeated alternatives such as `\(a\|aa\)*b`, repeated greedy
atoms followed by a failing suffix, deeply nested groups, and large counted
repetitions of nullable groups.

## Decision Rule

Choose the safe built-in VM only when measurement shows a meaningful size
advantage and its implementation remains bounded, testable, and maintainable.
Otherwise use `regex`, accept the measured binary growth, remove the
no-external-engine architecture guard, and keep Rile's user-visible behavior in
a focused compatibility layer.
