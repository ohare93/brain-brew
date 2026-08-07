---
title: What is deck federation?
---

# What is deck federation?

Deck federation is composition without copying.

A maintainer publishes a base deck. Other maintainers can publish overlays that translate it, extend it, patch it, or personalize it. Brain Brew composes those pieces into a resolved deck for export.

```text
base deck
  + German translation overlay
  + Extended card-template overlay
  + Hardcore Geography extension overlay
  = resolved German Extended Hardcore deck
```

## Why not copy the deck?

Copies are hard to update. When upstream fixes a note, every copied deck must incorporate the change manually.

Federation keeps the relationship explicit:

- the base deck owns shared content and structure;
- overlays describe bounded changes;
- expected bases catch stale assumptions;
- lock files pin upstream package inputs.

## Package composition

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

A downstream package can extend an upstream target:

```yaml
targets:
  en-america:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - overlay.extension.america
```

The upstream target composes first, then the downstream overlay stack applies.

## Update flow

```bash
brainbrew lock update --package anki-geo.ultimate-geography --git https://github.com/anki-geo/ultimate-geography.git --ref main
brainbrew lock verify
brainbrew verify --manifest brainbrew.yaml --all-targets
```

Upstream updates become deliberate review events, not accidental floating dependencies.
