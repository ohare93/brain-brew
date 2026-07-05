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

The CLI computes `nar_hash` in Rust and does not require Nix at runtime. Path-based locks store portable paths relative to `brainbrew.lock` (for example, `path: ../pkg`) instead of absolute local paths. GitHub inputs should use `https://github.com/...`; `http://github.com/...` is also accepted for compatibility.

## Review after updates

When upstream changes, rerun:

```bash
brainbrew lock verify
brainbrew verify --manifest brainbrew.yaml --all-targets
brainbrew explain --manifest brainbrew.yaml --target en-america
```

Expected failures are the review surface: stale translation entries, expected-base mismatches, missing targets, media mismatches, or changed golden exports.
