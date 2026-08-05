---
title: Project scope and architecture
---

# Project scope and architecture

Brain Brew is a Rust-based, local-first deck federation tool for shared Anki-compatible decks.

## Current focus

The maintained product surface is:

- strict root Canonical Deck YAML with composable, disjoint Authoring Sources;
- read-only source-preserving CSV authoring for certified canonical seams;
- overlays and deterministic composition;
- manifest targets;
- CrowdAnki export and plan/review/apply full-deck bootstrap import;
- media references;
- federated package locks;
- CLI verification suitable for CI.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

The Canonical Deck File remains the strict root YAML declaration. Inline YAML and explicitly declared read-only sources such as CSV may own disjoint canonical paths and materialize into the same filesystem-independent `CanonicalDeck`, `Overlay`, and `TranslationDictionary` models. Source order never grants override precedence, and CSV-owned paths remain read-only until ownership moves explicitly to native YAML.

Production Ultimate Geography source changes remain gated until the composable CSV authoring epic's repository-owned fixture certification passes. The separately pinned live-consumer gate then remains mandatory.

## Non-goals for the current milestone

- SaaS or server sync;
- live Anki sync;
- storing review/scheduling state;
- a web app as the source of truth;
- legacy Python Brain Brew recipe compatibility as a public API;
- CSV write-back, byte-preserving CSV edits, or automatic new-language columns;
- arbitrary unsupported adapter-data passthrough;
- reconciling Anki or CrowdAnki edits into an existing Canonical Deck, include tree, or overlay stack.

CrowdAnki import creates only a separate full-deck bootstrap output. The product has no automatic base-versus-overlay ownership inference and no current Anki-to-source workflow. `diff --as-overlay` can draft a review artifact from two canonical decks, but maintainers must manually route accepted include, structured-field, translation, overlay, media declaration, and media-asset changes. See [CrowdAnki bootstrap boundary](../authoring/crowdanki-bootstrap-boundary.md).

## Crate boundaries

```text
brain-brew-core     pure domain model, validation, compose, semantic diff
brain-brew-formats  YAML, CSV authoring-source, CrowdAnki, manifest, lockfile, media codecs
brainbrew           filesystem, terminal output, command wiring (in crates/brain-brew-cli)
```

`brain-brew-core` must not depend on YAML, CSV, CrowdAnki, filesystem, terminal, or CLI concerns.

## Development method

Use Red-Green-Refactor for behavior changes:

1. add a failing test;
2. implement the smallest change that passes;
3. refactor with tests green.

Run before meaningful commits:

```bash
devenv test
```

## Decision log

Architectural decisions live in [Architecture decisions](decisions/README.md).
