---
title: Ultimate Geography fixture
---

# Ultimate Geography fixture

Ultimate Geography is the large case study in this repository.

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
- the current UG include layout under `templates/<name>/{question,answer}.html`;
- a shared Extended variant overlay;
- small per-language Extended metadata overlays;
- Hardcore Geography as both an extension overlay and its standalone companion manifest;
- 74 verified Ultimate Geography targets plus 26 Hardcore companion targets.

## Inspect targets

```bash
brainbrew targets --manifest fixtures/ultimate-geography/brainbrew.yaml
```

## Verify all targets

```bash
brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets
```

Expected output:

```text
✓ verified 74 targets
  manifest: fixtures/ultimate-geography/brainbrew.yaml
```

## Export one target

```bash
brainbrew export crowdanki \
  --manifest fixtures/ultimate-geography/brainbrew.yaml \
  --target de-extended \
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
