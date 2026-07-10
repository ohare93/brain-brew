---
title: Packages and lock files
---

# Packages and lock files

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

Federated packages let one repository compose with another without copying upstream source.

## Downstream package

```yaml
package:
  id: anki-geo.america
  version: 0.1.0
  base_package: anki-geo.ultimate-geography
  compatible_base_versions:
    - '>=0.1.0, <0.2.0'
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
base: deck.yaml
overlays:
  overlay.extension.america:
    file: overlays/america.yaml
    kind: extension
targets:
  en-america:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - overlay.extension.america
```

## Version and compatibility semantics

Package metadata uses the [`semver`](https://docs.rs/semver/) requirement grammar and is enforced before any target or package source is planned.

- `version` is a full Semantic Version such as `1.2.3` or `2.0.0-beta.1`; opaque labels and shortened versions are invalid.
- Every `depends_on` entry is an exact `<package-id>@<SemVer>` pin. Omitting `@version` or putting a range there is invalid. Exact pins keep the selected registry/lock identity reproducible, including any build metadata.
- An extension identifies its upstream with `base_package`. That ID must also have an exact `depends_on` pin.
- `compatible_base_versions` must be non-empty when `base_package` is present and must be absent otherwise. Base packages therefore omit both fields.
- Commas inside one requirement are **AND**: `>=1.2.0, <2.0.0`. Separate list entries are **OR** alternatives.
- Prereleases follow the `semver` crate rule. A prerelease is matched only by a comparator containing a prerelease with the same major/minor/patch tuple; for example `>=2.0.0-alpha.1, <2.0.0` can match `2.0.0-alpha.2`, while `>=1.0.0` does not match `2.0.0-alpha.1`. Build metadata does not affect range matching.

Formatting canonicalizes requirement spacing. Empty or malformed versions/requirements fail at manifest decode with the manifest path and declaring field. Brain Brew validates the complete package registry from the root manifest, explicit `--include` files, `--package-root` discovery, and sibling locks before target planning. Missing packages, duplicate/conflicting package IDs, exact-version mismatches, incompatible base versions, self-cycles, and multi-package cycles all fail closed.

This is validation, not dependency solving: maintainers choose and lock one exact dependency version, then declare whether that selected base version is in the extension's supported range.

## Local includes

During local development, include another manifest explicitly:

```bash
brainbrew compose \
  --manifest america/brainbrew.yaml \
  --include ultimate-geography/brainbrew.yaml \
  --target en-america
```

Or discover sibling packages:

```bash
brainbrew targets --package-root ../anki-geo-packages
```

## Bounded package-root discovery

Every command that accepts `--package-root` uses the registry planner's single discovery implementation. Discovery reads directory entries in sorted order, never follows symlinks, and returns manifests in package-root/normalized-relative-path order. A symlink, special entry, unreadable candidate directory, replaced directory identity, or filesystem read error outside a pruned tree fails closed with the package root and current path.

The defaults are:

| Budget | Default | Override | Maximum override |
|---|---:|---|---:|
| directory depth (package root is depth 0) | 32 | `--discovery-max-depth <n>` | 256 |
| inspected entries, including roots and entries later pruned | 100,000 | `--discovery-max-entries <n>` | 10,000,000 |
| discovered `brainbrew.yaml` manifests | 1,000 | `--discovery-max-manifests <n>` | 100,000 |

Zero, non-decimal, overflow, and effectively unbounded overrides are rejected. Budget failures report `package_root`, `current_path`, `consumed`, `limit`, and the exact override flag. These defaults leave substantial room above the measured Brain Brew repository (600 inspected entries, depth 6, 2 manifests after pruning) and Ultimate Geography fixture (146 inspected entries, depth 4, 1 manifest), while bounding accidental scans of a repository root.

Before metadata or descent, discovery prunes directory entries with these exact names: `.git`, `.jj`, `.hg`, `.svn`, `.devenv`, `.direnv`, `target`, `build`, `output`, `outputs`, `dist`, `site`, `_site`, `node_modules`, `.docusaurus`, `.cache`, `.brainbrew-cache`, `.brainbrew-transactions`, and `result`. Nix `result-*` entries and Brain Brew `.brainbrew-*.stage`/`.brainbrew-*.backup` publication directories are also pruned by their complete structural names. Matching is not based on substrings: names such as `builder`, `my-target`, and `legitimate-generated-name` remain discoverable.

Use repeatable `--package-ignore <pattern>` for repository-specific generated or declared export directories that do not use those conventional names:

```bash
brainbrew targets \
  --package-root ../anki-geo-packages \
  --package-ignore 'releases/generated' \
  --package-ignore 'vendor/**/fixtures-*'
```

Ignore values use the same portable, package-root-relative authorization as `SafeRelativePath`: absolute paths, `.`/`..`, backslashes, drive/UNC forms, repeated separators, controls, and other ambiguous forms are rejected before traversal. `*` and `?` match within one complete path component; a component equal to `**` matches zero or more complete components. A literal pattern matches only that exact relative path. Built-ins have precedence over configured patterns, and both only prune (a configured pattern cannot re-enable a built-in tree). `targets --json` reports roots/entries/directories/files/manifests visited and built-in/configured prune counts under `discovery`.

## Lock an upstream package

```bash
brainbrew lock update \
  --package anki-geo.ultimate-geography \
  --git https://github.com/anki-geo/ultimate-geography.git \
  --ref main

brainbrew lock verify
```

After locking, normal commands resolve packages from `brainbrew.lock` automatically:

```bash
brainbrew compose --manifest america/brainbrew.yaml --target en-america
brainbrew verify --manifest america/brainbrew.yaml --all-targets
```

## Supported source inputs

```bash
brainbrew lock update --package pkg.id --path ../pkg
brainbrew lock update --package pkg.id --tarball https://example.org/pkg.tar.gz
brainbrew lock update --package pkg.id --git https://github.com/owner/repo.git --ref main
```

The CLI emits lock schema version 2 and computes its mandatory canonical SRI SHA-256 `nar_hash` in Rust; Nix is not required at runtime. Path locks store portable paths relative to `brainbrew.lock` (for example, `path: ../pkg`). GitHub URLs are normalized to `https://github.com/owner/repo.git`, and the locked source records a full immutable commit ID.

Version 1 locks are insecure and rejected rather than silently interpreted. Move or remove the old lock, then regenerate every package with its corresponding `brainbrew lock update` command.

Package snapshots use a reject-all-symlinks policy for path, GitHub, and tarball sources. Only regular files and directories may enter staging or the content-addressed cache; hard links and special entries are rejected too. If an older package or cache contains a symlink, replace it with real package-owned content, remove the rejected cache entry if the diagnostic requests that, and rerun `brainbrew lock update`. Even an apparently contained symlink is intentionally rejected.

Remote sources are HTTPS-only. Brain Brew bounds redirects, connect/read/total time, downloaded and decompressed bytes, expansion ratio, entry count, individual and total regular-file bytes, metadata, and path size/depth. Downloads and decompressed tar streams use private temporary files rather than unbounded memory, and failures occur before cache publication. See [Lock file reference](../reference/lockfile.md#fetch-policy-defaults) for exact defaults and rationale.

## Review after updates

When upstream changes, rerun:

```bash
brainbrew lock verify
brainbrew verify --manifest brainbrew.yaml --all-targets
brainbrew explain --manifest brainbrew.yaml --target en-america
```

Expected failures are the review surface: stale translation entries, expected-base mismatches, missing targets, media mismatches, or changed golden exports.

Federated media keeps the same trust boundary as package source. Final declarations retain their declaring package/source kind and must use that package's media-root mapping (for example `--media-root anki-geo.ultimate-geography=/srv/ug-media`). Locked/cache source files are read-only to root-workspace media mutators, and their package tree hashes are checked before and after mutation; Brain Brew never edits or silently repairs a dependency cache.
