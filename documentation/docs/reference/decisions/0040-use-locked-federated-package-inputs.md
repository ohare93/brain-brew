# ADR-040: Use Locked Federated Package Inputs

**Date**: 2026-05-24  
**Status**: Accepted  
**Deciders**: Brain Brew contributors

## Context

Federated Decks only become true federation when deck packages can live in separate repositories and compose reproducibly. An America package should be able to extend a specific Ultimate Geography target, and a learner or maintainer should be able to compose Ultimate Geography + America + Mountains + Rivers without copying any upstream source.

That requires two separate concerns:

1. **Composition references**: a target in one manifest must be able to extend a target from another package and apply local or remote overlays on top.
2. **Source locking**: remote package inputs must be pinned to a deliberate source revision and content hash, so updates are explicit and reviewable.

Nix flakes are a useful model: a flake input has an original reference and a locked reference. The lock records a Git revision and a `narHash`, the SRI hash of the Nix Archive serialization of the fetched source tree. Nix commands update the lock only when asked, and normal builds use the lock.

## Related Decisions

- [ADR-034: Use Minimal Federated Deck Manifest](0034-use-minimal-federated-deck-manifest.md) - Manifests stay declarative.
- [ADR-038: Use Manifest Package Metadata and Target Checks](0038-use-manifest-package-metadata-and-target-checks.md) - Package identity and dependency metadata are the foundation for resolution.
- [ADR-039: Use Source Variables and Translation Dictionaries](0039-use-source-variables-and-translation-dictionaries.md) - Extension packages should avoid copying translated template structure.

## Decision

Extend target semantics so a target may declare an upstream target it extends:

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

Composition with `--include` or `--package-root` builds the upstream target first, then applies the dependent package's overlay stack.

For source locking, adopt a `brainbrew.lock` design modeled after `flake.lock`:

```yaml
version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    locked:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      rev: ccf150a1b21e...
      nar_hash: sha256-...
```

The manifest names dependencies and composition references; the lock records exactly which source tree satisfied them. Updating a dependency is a deliberate lock update followed by `brainbrew diff` / `brainbrew explain` review.

## Rationale

- Target-level `extends` matches how deck maintainers think: "America extends Ultimate Geography English Standard".
- Package-qualified references avoid path coupling between repositories.
- Locking by revision plus content hash makes builds reproducible and makes silent upstream drift impossible.
- A Nix-like lock file is familiar, auditable, and CI-friendly.
- Keeping fetching/locking outside the core crate preserves the pure-domain boundary.

## Alternatives Considered

- **Copy upstream `deck.yaml` into every extension repo**: rejected because it destroys federation and makes updates manual and error-prone.
- **Floating Git branches in manifests**: rejected because composition would change without a maintainer choosing to update.
- **Only semantic versions**: rejected because package versions are useful compatibility labels but do not identify immutable source trees.
- **Use Nix flakes directly as the Brain Brew package model**: rejected for now. Brain Brew should be usable without making every deck package a Nix flake, while still supporting Nix-style locking semantics.

## Implications

- CLI commands that compose manifest targets should accept `--include` and `--package-root` so separate checked-out repos can be composed locally.
- Lock commands should fetch, hash, verify, and update package inputs explicitly; normal compose/export/verify commands should use the committed lock without mutating it.
- Compatibility failures after an upstream update should be surfaced through normal overlay composition errors, stale translation entries, missing paths, and semantic diffs.
- Package registries can remain deferred; Git URLs plus lock entries are enough for early federation.
