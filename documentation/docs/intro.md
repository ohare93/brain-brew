---
title: Welcome
slug: /
---

# Brain Brew

Brain Brew is a Rust-based, local-first deck federation and round-trip engine for shared Anki-compatible decks.

It continues the established Brain Brew project name while replacing the legacy Python recipe pipeline with canonical deck source, overlays, manifests, and reproducible verification.

It helps a deck maintainer keep one canonical source, then compose it with translations, extensions, patches, and personal overlays without copying the whole deck.

## What you can do today

- validate Canonical Deck YAML;
- compose overlay stacks into resolved decks;
- import and export CrowdAnki folders;
- compare decks by stable IDs instead of raw lines;
- publish Federated Deck packages with named targets;
- lock upstream package inputs for reproducible downstream composition.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

## The core loop

```bash
brainbrew targets --manifest brainbrew.yaml
brainbrew verify --manifest brainbrew.yaml --all-targets
brainbrew compose --manifest brainbrew.yaml --target en-standard --out build/en-standard.yaml
brainbrew export crowdanki --manifest brainbrew.yaml --target en-standard
```

## Start here

1. [Install the CLI](getting-started/install.md).
2. [Compose a tiny Federated Deck](getting-started/quickstart.md).
3. Learn the [overlay kinds](concepts/overlays.md).
4. Author [translations](authoring/translations.md), [extensions](authoring/extensions.md), and [field fills](authoring/field-fills.md).

Ultimate Geography appears throughout these docs as a large, real-world fixture. It is not a special CLI feature.
