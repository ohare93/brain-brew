---
title: Downstream package example
---

# Downstream package example

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

This example creates a package that extends Ultimate Geography with an America overlay.

## Manifest

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

## Local development with includes

```bash
brainbrew compose \
  --manifest america/brainbrew.yaml \
  --include ultimate-geography/brainbrew.yaml \
  --target en-america \
  --out build/en-america.yaml
```

## Lock the upstream package

```bash
cd america
brainbrew lock update \
  --package anki-geo.ultimate-geography \
  --path ../ultimate-geography
brainbrew lock verify
```

Now the include is implicit:

```bash
brainbrew compose --manifest brainbrew.yaml --target en-america
```

## Mixing overlays from multiple packages

```yaml
targets:
  en-mixed:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - anki-geo.america:overlay.extension.america
      - anki-geo.mountains:overlay.extension.rockies
```

This makes a learner or downstream maintainer's stack explicit and reproducible.
