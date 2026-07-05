# SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
# SPDX-License-Identifier: GPL-3.0-or-later

SHELL := /bin/sh
.DEFAULT_GOAL := help

IMAGE ?= rile-dev
VISUAL_IMAGE ?= rile-visual
PERF_IMAGE ?= rile-perf
REFERENCE_EDITORS ?= emacs zile kg rile
REF_EDITOR ?=
REF_SCENARIO ?=
RELEASE_VERSION ?= v0.9.0
IN_CONTAINER := IMAGE=$(IMAGE) ./scripts/in-container
RELEASE_IN_CONTAINER := IMAGE=$(IMAGE) ./scripts/release-in-container
VISUAL_IN_CONTAINER := IMAGE=$(VISUAL_IMAGE) CONTAINERFILE=Containerfile.visual ./scripts/in-container

.PHONY: help shell tools build test test-cargo snapshot-test fmt fmt-check lint audit unused-deps verify run release-doctor release-check release-snapshot release-notes release-publish-tag perf-smoke reference-capture reference-capture-all visual-demos visual-frames clean

## Show available commands
help:
	@printf '%s\n' 'Available targets:'
	@awk '\
		/^## / { help = substr($$0, 4); next } \
		/^[a-zA-Z0-9_.-]+:/ { \
			if (help != "") { \
				target = $$1; \
				sub(/:.*/, "", target); \
				printf "  %-24s %s\n", target, help; \
				help = ""; \
			} \
		} \
	' $(MAKEFILE_LIST) | sort

## Open an interactive shell in the dev container
shell:
	IMAGE=$(IMAGE) ./scripts/devshell

## Check required development tools inside the dev container
tools:
	$(IN_CONTAINER) ./scripts/tools

## Build the Rust crate in the dev container
build:
	$(IN_CONTAINER) ./scripts/build

## Run the preferred test runner in the dev container
test:
	$(IN_CONTAINER) ./scripts/test

## Run Cargo's built-in test runner in the dev container
test-cargo:
	$(IN_CONTAINER) ./scripts/test-cargo

## Run opt-in parsed-screen snapshot tests
snapshot-test:
	$(IN_CONTAINER) ./scripts/snapshot-test $(ARGS)

## Format Rust code in the dev container
fmt:
	$(IN_CONTAINER) ./scripts/fmt

## Check Rust formatting in the dev container
fmt-check:
	$(IN_CONTAINER) ./scripts/fmt-check

## Run formatting and lint checks in the dev container
lint:
	$(IN_CONTAINER) ./scripts/lint

## Run advisory, license, and dependency policy checks
audit:
	$(IN_CONTAINER) ./scripts/audit

## Check for unused dependencies
unused-deps:
	$(IN_CONTAINER) ./scripts/unused-deps

## Run the full development verification suite
verify:
	$(IN_CONTAINER) ./scripts/verify

## Run the Rile binary in the dev container; pass ARGS='--help'
run:
	$(IN_CONTAINER) ./scripts/run $(ARGS)

## Check release-tools and GoReleaser configuration
release-doctor:
	$(RELEASE_IN_CONTAINER) release-tools doctor

## Validate GoReleaser release configuration
release-check:
	$(RELEASE_IN_CONTAINER) release-tools check

## Build a local release snapshot without publishing
release-snapshot:
	$(RELEASE_IN_CONTAINER) release-tools snapshot

## Generate release notes for RELEASE_VERSION
release-notes:
	$(RELEASE_IN_CONTAINER) release-tools notes $(RELEASE_VERSION)

## Publish an existing tag; set RELEASE_TOKEN and RELEASE_VERSION
release-publish-tag:
	$(RELEASE_IN_CONTAINER) release-tools publish-tag $(RELEASE_VERSION)

## Run optional large-file and long-line performance smoke tests
perf-smoke:
	PERF_IMAGE=$(PERF_IMAGE) tools/perf/run $(ARGS)

## Capture one reference scenario; set REF_EDITOR=zile REF_SCENARIO=smoke-open
reference-capture:
	@test -n "$(strip $(REF_EDITOR))" || { printf '%s\n' 'REF_EDITOR is required, e.g. REF_EDITOR=zile' >&2; exit 2; }
	@test -n "$(strip $(REF_SCENARIO))" || { printf '%s\n' 'REF_SCENARIO is required, e.g. REF_SCENARIO=registers' >&2; exit 2; }
	@test -x "tools/reference/$(REF_EDITOR)/capture" || { printf 'unknown reference editor: %s\n' "$(REF_EDITOR)" >&2; exit 2; }
	tools/reference/$(REF_EDITOR)/capture "$(REF_SCENARIO)"

## Capture all reference scenarios; optionally set REF_EDITOR=zile
reference-capture-all:
	@editors="$(strip $(REF_EDITOR))"; \
	if [ -z "$$editors" ]; then editors="$(REFERENCE_EDITORS)"; fi; \
	for editor in $$editors; do \
		if [ ! -x "tools/reference/$$editor/capture" ]; then \
			printf 'unknown reference editor: %s\n' "$$editor" >&2; \
			exit 2; \
		fi; \
		for scenario in tools/reference/$$editor/scenarios/*.scenario; do \
			name=$$(basename "$$scenario" .scenario); \
			printf 'Capturing %s/%s\n' "$$editor" "$$name"; \
			"tools/reference/$$editor/capture" "$$name"; \
		done; \
	done

## Generate optional VHS visual demo GIFs; pass ARGS='demos/name.tape'
visual-demos:
	$(VISUAL_IN_CONTAINER) ./scripts/visual-demos $(ARGS)

## Generate named PNG frames for visual review; pass ARGS='demos/name.tape'
visual-frames:
	$(VISUAL_IN_CONTAINER) ./scripts/visual-frames $(ARGS)

## Remove Cargo build artifacts
clean:
	rm -rf target
