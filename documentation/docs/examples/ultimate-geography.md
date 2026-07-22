---
title: Ultimate Geography fixture
---

# Ultimate Geography fixture

Ultimate Geography is the large, pinned case study in this repository. The
fixture preserves the upstream repository's two-manifest shape: 74 targets in
the main Ultimate Geography manifest and 26 targets in the standalone Hardcore
Geography companion manifest.

```text
fixtures/
  ultimate-geography.lock.json
  ultimate-geography/
    deck.yaml
    deck-hardcore.yaml
    brainbrew.yaml
    brainbrew-hardcore.yaml
    media.yaml
    media/                         # one real 609-file tree
    goldens/                       # UG-owned referenced goldens
    descriptions/
    templates/
    styles/
    overlays/
  ultimate-geography-attribution/
    hardcore-geography/{README.md,sources.csv}
  ultimate-geography-expected/
    crowdanki/<100 targets>/deck.json
```

The source is an exact whitelist from UG `brainbrew-migration` revision
`a934c935...`, rebased on upstream `e1fd8518...`, not a fixture-only migrated
derivative. It demonstrates:

- English Standard as a base Canonical Deck;
- language overlays for all 16 UG languages, including Hebrew RTL coverage;
- source variables for shared card-template labels;
- scalar includes for descriptions, templates, and styling;
- a shared Extended variant overlay and small language-specific residue;
- Hardcore Geography as both an extension overlay and standalone companion;
- real declared media bytes and canonical hashes;
- exact attribution coverage from 548 UG and 56 separately pinned Hardcore
  image records, plus five UG runtime files;
- complete parsed CrowdAnki output for every target.

## Inspect the exact target inventory

```bash
brainbrew targets --manifest fixtures/ultimate-geography/brainbrew.yaml
brainbrew targets --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml
```

The machine-readable lock and mandatory tests require exactly 74 + 26. Missing,
extra, or cross-manifest duplicate targets fail.

## Verify all targets strictly

Both manifests can be verified offline against the single vendored media root:

```bash
brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew.yaml \
  --all-targets --media-root media
brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml \
  --all-targets --media-root media
```

Expected output ends with:

```text
✓ verified 74 targets
  media verification: strict
✓ verified 26 targets
  media verification: strict
```

The normal formats integration gate additionally exports all 100 targets and
compares each parsed JSON value with its committed expected `deck.json`. Expected
target directories contain no media copies.

## Export one target from a disposable copy

The checked-in source tree is digest-locked, so experiment in a temporary copy:

```bash
rm -rf /tmp/brainbrew-ug-example
cp -a fixtures/ultimate-geography /tmp/brainbrew-ug-example
brainbrew export crowdanki \
  --manifest /tmp/brainbrew-ug-example/brainbrew.yaml \
  --target de-extended \
  --media-root media \
  --out /tmp/brainbrew-ug-example/build/crowdanki/de-extended
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

That keeps the Extended card-template HTML shared instead of copied for every
language.

## Fixture maintenance and workflow policy

Source refresh, expected-output acceptance, and read-only checking are separate
hermetic boundaries. See
[Ultimate Geography regression fixture](../reference/ultimate-geography-fixture.md)
for the contract and exact commands.

This fixture documents Brain Brew behavior only; Ultimate Geography maintainer
workflow policy is owned externally. CrowdAnki import can bootstrap a separate
full-deck workspace for inspection, but it does not merge into UG base, include,
translation, extension, or media sources. Preserve upstream source and manually
route reviewed changes; see
[CrowdAnki bootstrap boundary](../authoring/crowdanki-bootstrap-boundary.md).
