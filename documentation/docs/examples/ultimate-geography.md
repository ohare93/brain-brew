---
title: Ultimate Geography fixture
---

# Ultimate Geography fixture

Ultimate Geography is the large case study in this repository. The fixture mirrors the upstream repository's two-manifest shape: the main Ultimate Geography manifest plus the standalone Hardcore Geography companion manifest.

```text
fixtures/ultimate-geography/
  deck.yaml
  deck-hardcore.yaml
  brainbrew.yaml
  brainbrew-hardcore.yaml
  descriptions/ultimate-geography/*.html
  templates/ultimate-geography/**
  styles/ultimate-geography/card.css
  overlays/languages/*.yaml
  overlays/variants/extended.yaml
  overlays/variants/extended/*.yaml
  overlays/extensions/hardcore.yaml
```

It demonstrates:

- English Standard as a base Canonical Deck;
- language overlays for all 16 UG languages, including Hebrew RTL coverage;
- source variables for shared card-template labels;
- the current UG include layout for core and Extended cards under `templates/ultimate-geography/<card>/{question,answer}.html`;
- a shared Extended variant overlay;
- small per-language Extended metadata overlays;
- Hardcore Geography as both an extension overlay and its standalone companion manifest;
- all language targets declared across both manifests.

## Inspect targets

List targets from both manifests instead of relying on a committed count:

```bash
brainbrew targets --manifest fixtures/ultimate-geography/brainbrew.yaml
brainbrew targets --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml
```

## Verify all targets

Verify both manifests so the main UG targets and the standalone Hardcore companion targets stay covered:

```bash
brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets --media-mode reference-only
brainbrew verify --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml --all-targets --media-mode reference-only
```

Illustrative output, regenerated against the fixture on 2026-07-04 after measuring the upstream target listings:

```text
warning: target ...: MEDIA REFERENCE-ONLY DEVELOPMENT MODE: ... NOT RELEASE-READY
✓ verified 74 targets
  manifest: fixtures/ultimate-geography/brainbrew.yaml
  media verification: reference_only (NOT RELEASE-READY)

✓ verified 26 targets
  manifest: fixtures/ultimate-geography/brainbrew-hardcore.yaml
  media verification: reference_only (NOT RELEASE-READY)
```

The repository fixture intentionally excludes the external Ultimate Geography media tree and has hashless declarations, so these commands prove formatting, composition, and reference structure only. They are not byte-integrity or release-readiness evidence. A real UG release must use default strict mode with package owner roots and real hashed bytes. If upstream adds languages or target families, rerun the target listing for each manifest and refresh this example instead of copying the old numbers.

## Export one target

```bash
brainbrew export crowdanki \
  --manifest fixtures/ultimate-geography/brainbrew.yaml \
  --target de-extended \
  --media-mode reference-only \
  --out /tmp/de-extended-crowdanki
```

## Important design pattern

Repeated template wording is represented with source variables:

```yaml
variables:
  label.flag: Flag
  label.location: Location
```

Language overlays translate the variables:

```yaml
translations:
  variables:
    label.location:
      Location: Lage
```

That keeps the Extended card-template HTML shared instead of copied for every language.

## Workflow policy boundary

This fixture documents Brain Brew behavior only; Ultimate Geography maintainer workflow policy is owned externally. CrowdAnki import can bootstrap a separate full-deck workspace for inspection, but it does not restore the legacy Anki-to-source workflow or merge into UG base, include, translation, extension, or media sources. Preserve the upstream source and manually route reviewed changes; see [CrowdAnki bootstrap boundary](../authoring/crowdanki-bootstrap-boundary.md).
