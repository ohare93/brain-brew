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

The URL identifies where the archive was obtained; the mandatory tree hash authenticates the extracted source tree. Remote Git and tarball URLs must use HTTPS. `file://` and plain filesystem paths are explicit local archive inputs; HTTP cannot be selected by lock YAML.

## Required tree hash

`locked.nar_hash` is exactly one canonical SRI SHA-256 value:

```text
sha256-<standard padded base64 encoding of exactly 32 digest bytes>
```

Missing, empty, malformed, noncanonical, duplicate, or unknown-algorithm hashes fail while parsing the lock, before any package manifest is loaded. Brain Brew computes this hash over deterministic Nix Archive (NAR) serialization in Rust and does not require the `nix` command at runtime.

A cached tree is revalidated and rehashed before every use. A mismatching or policy-invalid cache entry is rejected with instructions to remove it; Brain Brew does not silently consume or repair tampered cached content.

## Package tree entry policy

Fetched and locked package trees contain **regular files and directories only**. Brain Brew rejects every symlink, including links whose text appears to stay inside the package. It also rejects filesystem hard links and special files, and rejects archive hard links, devices, FIFOs, sparse/continuous/unknown entries, unsafe raw paths, and duplicate or colliding normalized targets. This policy is identical for local path snapshots, GitHub source archives, tarballs, staging trees, and warm cache trees.

Archives are inspected entry-by-entry and are never passed to a general-purpose unpack operation. Download bytes stream to a bounded private temporary file; gzip output streams to a second bounded tar file; raw metadata preflight and extraction then reopen that file without retaining the archive in memory. Files are created in a private temporary tree with create-new semantics, ownership and set-ID metadata are discarded, and permissions are normalized to `0644`/`0755`. PAX/GNU metadata, duplicate targets, and metadata entries are included in the entry/decompressed accounting. The complete tree is validated before hashing. Cache publication copies and revalidates that exact tree in the cache filesystem, checks its hash again, and atomically renames it to the content-addressed destination while holding a publication lock. A failed download, decompression, extraction, or publication removes its private temporary state and does not replace an existing valid cache entry.

## Fetch policy defaults

All GitHub API, GitHub codeload, and remote tarball requests use one non-environment-configurable `FetchPolicy`:

| Budget | Default |
|---|---:|
| connect timeout | 10 seconds |
| individual socket read timeout | 30 seconds |
| monotonic total download/decompression deadline | 120 seconds |
| redirects | 5 |
| compressed/downloaded response | 64 MiB |
| GitHub JSON response | 1 MiB |
| decompressed tar stream | 512 MiB |
| one regular file | 64 MiB |
| physical archive entries, including metadata | 20,000 |
| total expanded regular files | 256 MiB |
| path bytes / components | 1,024 / 32 |
| one PAX/GNU metadata entry | 64 KiB |
| decompressed/compressed expansion ratio | 200:1 |

These limits leave substantial headroom over the maintained Ultimate Geography source fixture (about 1 MiB total, 119 files, and a largest source file below 200 KiB) while bounding archives that are far outside the current source-package use case. There are no hidden environment overrides or CLI limit switches. Code-level policy injection is reserved for deterministic adapter tests.

Redirects are followed manually. Every hop must remain HTTPS, URL credentials are forbidden, and all authorization headers are stripped. HTTPS cross-host redirects are allowed for content-delivery networks, subject to the same redirect and total-deadline budgets. `Content-Length` is rejected early when oversized but is never trusted: missing, false, and chunked lengths are bounded by bytes actually streamed.

### Symlink migration

Locks/caches produced from package sources containing symlinks are no longer usable, even if an older release accepted them or the links were lexically contained. `lock update` and cached use report the rejected relative entry and the reject-all-symlinks policy. Replace each link with a real regular file or directory in the package source, remove the rejected cache entry if instructed, and regenerate the package lock with `brainbrew lock update`. There is no compatibility switch that preserves links.

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

Remote packages can be used from a valid warm cache without a network request. Network transport budgets and archive download/expanded-size/entry-count limits remain a separate hardening concern; entry types and paths are already fail-closed here.
