# ADR-038: Use Manifest Package Metadata and Target Checks

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Brain Brew contributors

## Context

The minimal Federated Deck manifest made base decks, overlay catalogs, and build targets reproducible. As soon as manifests become useful across repositories, they also need stable package identity, dependency metadata, and target-level reproducibility checks that CI can run without project-specific test harnesses.

Ultimate Geography previously proved CrowdAnki parity through repository-specific tests. That pattern should become generic manifest behavior so future deck packages can declare expected export artifacts directly in `brainbrew.yaml`.

## Related Decisions

- [ADR-017: Require Byte-Stable Canonicalized Source Round Trips](0017-require-byte-stable-canonicalized-source-round-trips.md) - Source formatting remains a verification gate.
- [ADR-024: Store Media as External Assets with References](0024-store-media-as-external-assets-with-references.md) - Manifest verification can now validate and copy referenced media assets from an explicit media root.
- [ADR-034: Use Minimal Federated Deck Manifest](0034-use-minimal-federated-deck-manifest.md) - This extends the minimal manifest without turning it into a broad recipe DSL.

## Decision

Extend `brainbrew.yaml` with optional package metadata and optional per-target export checks:

```yaml
package:
  id: anki-geo.ultimate-geography
  version: 0.1.0
  compatible_base_versions:
    - '>=0.1,<0.2'
  depends_on:
    - anki-geo.shared-geography@0.1.0

base: deck.yaml

targets:
  en-standard:
    overlays: []
    exports:
      crowdanki:
        out: build/en-standard
        golden: goldens/en-standard/deck.json
```

`verify` remains non-mutating, but it reads `exports.crowdanki.golden` and compares the generated CrowdAnki JSON semantically as JSON. `export crowdanki --manifest ... --target ...` uses `exports.crowdanki.out` as the default output directory when configured; otherwise, omitting `--out` writes to `build/crowdanki/<target>`.

Media asset validation stays filesystem-bound CLI behavior. `--media-root` instructs `verify` and `export crowdanki` to validate declared/used media references, asset existence, and hashes. `export crowdanki --media-root ...` also copies referenced assets into the export folder's CrowdAnki `media/` subdirectory.

## Rationale

- Package metadata gives future federation a stable handle without introducing package fetching, registries, or lockfiles yet.
- Target export checks make reproducibility observable in normal CI instead of hidden in fixture-specific tests.
- Keeping export checks target-scoped matches how maintainers think about release artifacts.
- Requiring an explicit `--media-root` keeps the core and format crates filesystem-free while making CI media validation practical.

## Alternatives Considered

- **Full recipe DSL now**: rejected because ADR-028 still applies; manifests should remain small and declarative.
- **Always export during verify**: rejected because verification should not mutate the workspace unless a later command explicitly requests build artifacts.
- **Byte-compare golden JSON**: rejected in favor of JSON semantic equality so whitespace-only changes do not break checks.
- **Implicit media root discovery**: rejected for now because local workspaces may organize assets differently.

## Implications

- Federated Deck manifests can identify packages and expose target/export metadata to CLI and future UI tools.
- Generic CI can run `brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media` and get source formatting, composition, validation, media, and configured golden export checks.
- Local package discovery can start with `--package-root`, exact `package-id@version` dependency checks, and qualified target names before introducing registries or lockfiles.
- Future package resolution can build on `package.id`, `package.version`, `package.depends_on`, and `package.compatible_base_versions` without changing target semantics.
