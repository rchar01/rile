# SPDX-FileCopyrightText: 2026 Rile contributors
# SPDX-License-Identifier: GPL-3.0-or-later

SHELL := /bin/sh
.DEFAULT_GOAL := help

IMAGE ?= rile-dev
IN_CONTAINER := IMAGE=$(IMAGE) ./scripts/in-container

.PHONY: help shell tools build test test-cargo lint audit unused-deps verify run clean

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

## Remove Cargo build artifacts
clean:
	rm -rf target
