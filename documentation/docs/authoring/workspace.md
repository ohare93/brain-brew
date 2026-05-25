---
title: Workspace layout
---

# Workspace layout

A Federated Deck workspace contains a manifest, a base deck, and overlays.

```text
my-deck/
  brainbrew.yaml
  deck.yaml
  overlays/
    languages/de.yaml
    variants/extended.yaml
    variants/extended/de.yaml
    extensions/rivers.yaml
    patches/capitals.yaml
  media/
    flags/fi.svg
```

## `deck.yaml`

The base Canonical Deck. It owns shared structure and content.

## `overlays/`

Sparse changes to the base deck. Keep overlays small and purpose-shaped:

- language overlays in `overlays/languages/`;
- shared variant overlays in `overlays/variants/`;
- optional content extensions in `overlays/extensions/`;
- corrections in `overlays/patches/`.

## `brainbrew.yaml`

The manifest declares package metadata, named overlays, dependencies, and build targets.

```yaml
package:
  id: example.capitals
  version: 0.1.0
base: deck.yaml
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  de-standard:
    overlays:
      - overlay.translation.de
```

## Formatting

Use canonical formatting as a review gate:

```bash
brainbrew fmt deck.yaml
brainbrew fmt brainbrew.yaml
find overlays -name '*.yaml' -print0 | xargs -0 -n1 brainbrew fmt
```

`brainbrew verify --all-targets` also checks formatting.
