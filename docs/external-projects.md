<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# External Projects

This page collects links for external projects Rile uses in development,
testing, reference capture, performance smoke testing, and release publishing.
It is a documentation aid, not a vendoring or licensing inventory. See
[NOTICE.md](../NOTICE.md) and [Reference Testing](reference-testing.md) for
Rile's reference-code and provenance policy.

The dev-container workflow remains the source of truth for installed tool
versions. Check `Containerfile.dev`, `Containerfile.visual`,
`tools/perf/Containerfile`, `tools/reference/*/Containerfile`, `Cargo.toml`, and
`.goreleaser.yaml` when version details matter.

## Development Workflow

| Project | Use In Rile |
| --- | --- |
| [Podman](https://podman.io/) | Builds and runs the development, visual, reference, and performance containers. |
| [GNU Bash](https://www.gnu.org/software/bash/) | Runs the repository helper scripts. |
| [GNU Make](https://www.gnu.org/software/make/) | Provides the repository command facade in `Makefile`. |
| [Git](https://git-scm.com/) | Tracks source history and drives release branch/tag publishing. |
| [curl](https://curl.se/) | Downloads pinned tools and reference sources in container builds. |
| [Rust](https://www.rust-lang.org/) | Language and toolchain for the crate and release builds. |
| [rustup](https://rust-lang.github.io/rustup/) | Manages Rust toolchain components inside the dev container. |
| [Cargo](https://doc.rust-lang.org/cargo/) | Builds, tests, and packages the Rust crate. |
| [rustfmt](https://github.com/rust-lang/rustfmt) | Formats Rust source through `make fmt` and `make lint`. |
| [Clippy](https://github.com/rust-lang/rust-clippy) | Runs Rust lints with `-D warnings`. |
| [rust-analyzer](https://rust-analyzer.github.io/) | Installed in the dev container for editor integration. |
| [Debian](https://www.debian.org/) | Base distribution for the project containers. |
| [Rust Docker Image](https://hub.docker.com/_/rust) | Base image for the dev, visual, and performance containers. |

## Cargo Quality Tools

| Project | Use In Rile |
| --- | --- |
| [cargo-nextest](https://nexte.st/) | Preferred test runner for `make test` when available. |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | License, advisory, source, and dependency policy checks. |
| [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) | RustSec advisory checks. |
| [cargo-machete](https://github.com/bnjbvr/cargo-machete) | Unused dependency checks. |
| [cargo-insta](https://insta.rs/) | Parsed-screen snapshot verification for PTY snapshots. |

## Release Tooling

| Project | Use In Rile |
| --- | --- |
| [release-tools](https://codeberg.org/rch/release-tools) | Shared release validation, notes, snapshot, and publish workflow. |
| [GoReleaser](https://goreleaser.com/) | Builds release artifacts and checksums from `.goreleaser.yaml`. |
| [Codeberg](https://codeberg.org/) | Official Git hosting and release publishing target. |
| [Gitea](https://about.gitea.com/) | Forge API family used by Codeberg-compatible release publishing. |

## Visual And Reference Tooling

| Project | Use In Rile |
| --- | --- |
| [VHS](https://github.com/charmbracelet/vhs) | Records optional terminal GIFs and PNG frames for visual review and reference captures. |
| [ttyd](https://github.com/tsl0922/ttyd) | Terminal server used by VHS-based visual capture. |
| [Chromium](https://www.chromium.org/chromium-projects/) | Browser backend for visual capture tooling. |
| [FFmpeg](https://ffmpeg.org/) | Media tool used by visual capture workflows. |
| [GNU Emacs](https://www.gnu.org/software/emacs/) | Reference editor for behavior evidence and performance smoke comparisons. |
| [GNU Zile](https://www.gnu.org/software/zile/) | Reference editor for behavior evidence and performance smoke comparisons. |
| [GNU time](https://www.gnu.org/software/time/) | Measures optional performance smoke-test memory and timing data. |
| [kg](https://github.com/troglobit/kg) | Reference editor for behavior evidence and performance smoke comparisons. |
| [vim-tiny](https://packages.debian.org/bookworm/vim-tiny) | Debian `vi` baseline used by optional performance smoke tests. |
| [Vertico](https://github.com/minad/vertico) | Debian ELPA package used in the modern Emacs reference profile. |
| [Marginalia](https://github.com/minad/marginalia) | Debian ELPA package used in the modern Emacs reference profile. |
| [Orderless](https://github.com/oantolin/orderless) | Debian ELPA package available to the modern Emacs reference profile. |

## Direct Rust Crates

These are Rile's direct runtime and development crate dependencies. Use
`Cargo.toml` and `Cargo.lock` for exact versions.

| Crate | Use In Rile |
| --- | --- |
| [libc](https://crates.io/crates/libc) | Unix terminal and system calls. |
| [unicode-segmentation](https://crates.io/crates/unicode-segmentation) | Grapheme-aware text movement and editing. |
| [unicode-width](https://crates.io/crates/unicode-width) | Terminal display-width calculation. |
| [anyhow](https://crates.io/crates/anyhow) | Test and support-code error handling. |
| [assert_cmd](https://crates.io/crates/assert_cmd) | CLI and integration test command assertions. |
| [expectrl](https://crates.io/crates/expectrl) | PTY-backed terminal integration tests. |
| [insta](https://crates.io/crates/insta) | Snapshot test assertions. |
| [predicates](https://crates.io/crates/predicates) | Test predicates for command assertions. |
| [tempfile](https://crates.io/crates/tempfile) | Temporary files and directories in tests. |
| [vt100](https://crates.io/crates/vt100) | Parsed terminal screen state for PTY tests and snapshots. |

## Package And Registry Services

| Project | Use In Rile |
| --- | --- |
| [crates.io](https://crates.io/) | Rust crate registry used by Cargo dependencies and cargo-installed tools. |
| [crates.io index](https://github.com/rust-lang/crates.io-index) | Allowed Cargo registry source in `deny.toml`. |
| [GNU FTP](https://ftp.gnu.org/) | Source for the pinned GNU Zile reference release tarball. |
