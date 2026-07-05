---
title: Lock file reference
---

# Lock file reference

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

`brainbrew.lock` pins package inputs for reproducible federation.

## Shape

```yaml
version: 1
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
      rev: ccf150a1b21e...
      nar_hash: sha256-...
  local.example:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: path
      path: ../pkg
    locked:
      type: path
      path: ../pkg
      nar_hash: sha256-...
```

## Fields

- `manifest`: path to the package manifest inside the fetched source tree.
- `package.version`: package version read from the locked manifest.
- `original`: the maintainer-requested source.
- `locked`: the immutable source actually used.
- `locked.nar_hash`: SRI SHA-256 of the source tree's Nix Archive serialization.

Path-based locks store portable paths relative to the directory containing `brainbrew.lock`, not absolute machine-local paths. For example, a package locked from a sibling directory is stored as `path: ../pkg`.

Brain Brew computes `nar_hash` in Rust. The CLI does not require the `nix` command at runtime.

## Supported inputs

### Path

```bash
brainbrew lock update --package pkg.id --path ../pkg
```

### Tarball

```bash
brainbrew lock update --package pkg.id --tarball https://example.org/pkg.tar.gz
```

### GitHub Git URL

```bash
brainbrew lock update --package pkg.id --git https://github.com/owner/repo.git --ref main
brainbrew lock update --package pkg.id --git https://github.com/owner/repo.git --rev abc123
```

Use `https://github.com/...` for new locks. Brain Brew also accepts `http://github.com/...` inputs for existing or scripted source declarations. GitHub inputs resolve through the GitHub API and download the commit tarball.

## Cache

Locked sources are cached in the platform cache directory. Override for CI/tests:

```bash
BRAINBREW_CACHE_DIR=/tmp/brainbrew-cache brainbrew lock verify
```

## Mutation policy

Normal commands never mutate the lock file:

```bash
brainbrew compose --manifest brainbrew.yaml --target en-standard
brainbrew verify --manifest brainbrew.yaml --all-targets
```

Only `brainbrew lock update` rewrites `brainbrew.lock`.
