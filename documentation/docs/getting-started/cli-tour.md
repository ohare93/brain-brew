---
title: CLI tour
---

# CLI tour

The CLI has human-readable output by default and machine-readable JSON where it matters.

## List targets

```bash
brainbrew targets --manifest brainbrew.yaml
brainbrew targets --manifest brainbrew.yaml --json
```

Use this before composing so you know the available build targets.

## Validate source

```bash
brainbrew validate deck.yaml
brainbrew validate --manifest brainbrew.yaml --target en-standard
```

Validation checks deck structure, references, media declarations, and overlay composition invariants.

## Compose a target

```bash
brainbrew compose --manifest brainbrew.yaml --target de-standard --out build/de-standard.yaml
```

Composition applies the target's overlay stack and writes a resolved Canonical Deck.

## Export CrowdAnki

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --out build/crowdanki/de-standard
```

Add media verification/copying with:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target de-standard \
  --media-root media/
```

## Explain a target

```bash
brainbrew explain --manifest brainbrew.yaml --target de-standard
brainbrew explain --manifest brainbrew.yaml --target de-standard --json
```

`explain` shows the expanded overlay stack and the semantic changes introduced by that stack.

## Draft an overlay from edits

```bash
brainbrew diff deck.yaml edited.yaml \
  --as-overlay \
  --id overlay.patch.capitals \
  --kind patch > overlays/patches/capitals.yaml
```

This is useful after making a manual experiment and turning it into a reviewed overlay.

## Verify everything

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
```

`verify` is the CI-friendly gate. It checks formatting, parsing, composition, validation, media references, lock files, and configured CrowdAnki goldens.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.
