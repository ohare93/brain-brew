---
title: Lock file reference
---

# Lock file reference

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

`brainbrew.lock` pins and authenticates package source trees for reproducible federation.

## Version 2 shape

```yaml
version: 2
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      ref: main
    locked:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      rev: ccf150a1b21e0000000000000000000000000000
      nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
```

Every `original` and `locked` source is a tagged source-specific type. Unknown fields and fields belonging to another source type are rejected.

## Locked source variants

### Local path snapshot

```yaml
original:
  type: path
  path: ../pkg
locked:
  type: path
  path: ../pkg
  nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
```

Path selections are stored relative to the directory containing `brainbrew.lock`, so a lock and sibling package can be relocated together. `lock verify` always snapshots and hashes the live authorized path, even when its content-addressed cache is warm.

### GitHub Git revision

```yaml
original:
  type: git
  url: https://github.com/owner/repo.git
  ref: main
locked:
  type: git
  url: https://github.com/owner/repo.git
  rev: 0123456789abcdef0123456789abcdef01234567
  nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
```

`locked.rev` is mandatory and must be a full lowercase 40-character Git commit ID. Lock update resolves branch names and abbreviated revisions to a full GitHub commit and normalizes GitHub repository URLs to `https://github.com/owner/repo.git`.

### Tarball URL

```yaml
original:
  type: tarball
  url: https://example.org/pkg.tar.gz
locked:
  type: tarball
  url: https://example.org/pkg.tar.gz
  nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
```

The URL identifies where the archive was obtained; the mandatory tree hash authenticates the extracted source tree.

## Required tree hash

`locked.nar_hash` is exactly one canonical SRI SHA-256 value:

```text
sha256-<standard padded base64 encoding of exactly 32 digest bytes>
```

Missing, empty, malformed, noncanonical, duplicate, or unknown-algorithm hashes fail while parsing the lock, before any package manifest is loaded. Brain Brew computes this hash over deterministic Nix Archive (NAR) serialization in Rust and does not require the `nix` command at runtime.

A cached tree is rehashed before every use. A mismatching cache entry is rejected with instructions to remove it; Brain Brew does not silently consume or repair tampered cached content.

## Package paths and containment

`manifest` is a portable safe-relative path to the package manifest inside the authenticated source tree. Absolute, drive, UNC, backslash, `.`/`..`, and symlink-escape forms are rejected before the manifest is read. Once the source root is selected, package-owned manifest, base, overlay, include, and media paths may not escape it.

## Updating and verifying

```bash
brainbrew lock update --package pkg.id --path ../pkg
brainbrew lock update --package pkg.id --tarball https://example.org/pkg.tar.gz
brainbrew lock update --package pkg.id --git https://github.com/owner/repo.git --ref main
brainbrew lock update --package pkg.id --git https://github.com/owner/repo.git --rev 0123456789abcdef0123456789abcdef01234567
brainbrew lock verify
```

Only `brainbrew lock update` rewrites `brainbrew.lock`. Normal compose, targets, and verify operations never weaken or mutate it.

## Version 1 migration

Version 1 used one ambiguous optional-field source mapping and allowed hashless entries. It is insecure and is not interpreted as version 2.

Move or remove the old lock, then regenerate **every package** with its corresponding `brainbrew lock update` command. Verification and formatting report this migration action explicitly instead of accepting or silently upgrading the old entries.

## Cache location

Locked sources are cached in the platform cache directory. Override it for CI or tests:

```bash
BRAINBREW_CACHE_DIR=/tmp/brainbrew-cache brainbrew lock verify
```

Remote packages can be used from a valid warm cache without a network request. Network transport and archive extraction budgets remain a separate hardening concern.
