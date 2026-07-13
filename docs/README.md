<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Rile Documentation

This directory contains maintainer and contributor documentation for Rile.

## Guides

- [Architecture](architecture.md): current module boundaries, runtime flow,
  editor state, rendering, terminal integration, and known hotspots.
- [Backups and Auto-Save](backups-and-auto-save.md): current implementation of
  save backups, auto-save files, cleanup, recovery warnings, and configuration.
- [Development Notes](development.md): repository scope, milestone history, tooling, containers, release files, and CI expectations.
- [Emacs Function Reference](emacs-function-reference.md): durable curated Emacs
  command behavior notes for completed and future Rile compatibility work.
- [External Projects](external-projects.md): links for development, release,
  visual, reference, performance, and direct crate dependencies.
- [Performance Smoke Testing](performance.md): optional large-file and long-line
  timing comparisons against reference editors.
- [Reference Testing](reference-testing.md): optional behavior-capture workflow for reference editors such as GNU Zile and kg.
- [Release Checklist](release-checklist.md): containerized `release-tools` and
  GoReleaser workflow for cutting Rile releases.
- [Self-Documentation Architecture](self-documentation.md): implemented command,
  keymap, option, mode, help, and metadata-testing architecture.
- [Testing Guide](testing.md): unit, integration, PTY, snapshot, and optional visual-review workflows.

## Planning Documents

- [Architecture Improvement Ideas](architecture-improvement-ideas.md): future
  refactor candidates and validation expectations, separate from current
  architecture documentation.
Completed implementation plans should not remain here as permanent docs. When a plan finishes, move durable guidance into a guide and rely on Git history for the original checklist.
