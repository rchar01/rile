<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Release Checklist

Rile releases use the installed `release-tools` CLI and GoReleaser from the dev
container. The repository owns `.release-tools.env` and `.goreleaser.yaml`; the
shared release behavior stays in `release-tools`. See
[External Projects](external-projects.md) for release-tooling links.

## Pre-Release Checks

Before tagging a release:

- Confirm `Cargo.toml`, `Cargo.lock`, and `NEWS` name the target version.
- Confirm `NEWS` uses a `release-tools` compatible GNU NEWS heading such as
  `* Noteworthy changes in release 0.9.0 (2026-07-05)`.
- Confirm `README.md`, `docs/`, and `NEWS` describe current behavior.
- Run the full quality gate:

```sh
make verify
```

- Run release-tool validation inside the dev container:

```sh
make release-tools-check
make release-doctor
make release-check
make release-snapshot
make release-notes RELEASE_VERSION=v0.9.0
```

## Release Commit And Tag

After checks pass, commit the release-prep changes and create an annotated tag:

```sh
git commit
git tag -a v0.9.0 -m "v0.9.0"
```

Push both the release branch and tag before publishing.  For the official Rile
repository, the remote is `cb` and the release branch is `main`:

```sh
git push cb main v0.9.0
```

Use the configured remote and release branch for other clones.

## Publish

Publishing requires a Codeberg-compatible token. Prefer an environment-only
`RELEASE_TOKEN_FILE` for local maintainer releases; the container wrapper mounts
that file read-only and passes only the container-side token path to
`release-tools`:

```sh
RELEASE_TOKEN_FILE=~/.config/forge/token make release-publish-tag RELEASE_VERSION=v0.9.0
```

`RELEASE_TOKEN`, `GITEA_TOKEN`, `GITHUB_TOKEN`, and `GITLAB_TOKEN` remain
supported fallbacks, but the wrapper passes inherited token variables by name so
token values are not serialized into the host `podman run` arguments. Do not
commit token values or token paths to `.release-tools.env`.

`release-tools publish-tag` publishes from a clean temporary clone of the exact
local tag.  This prevents uncommitted or post-tag worktree changes from leaking
into release assets.

## Artifact Contract

The 0.9 release flow publishes:

- `rile_<version>_linux_amd64`
- `checksums.txt`

The current release is Linux amd64 only.  Add more targets only after the dev
container installs and verifies the required cross-compilation tooling.
