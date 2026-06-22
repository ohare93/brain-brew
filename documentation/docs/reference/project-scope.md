---
title: Project scope and architecture
---

# Project scope and architecture

Brain Brew is a Rust-based, local-first deck federation and round-trip engine for shared Anki-compatible decks.

## Current focus

The maintained product surface is:

- Canonical Deck source;
- overlays and deterministic composition;
- manifest targets;
- CrowdAnki import/export;
- media references;
- federated package locks;
- CLI verification suitable for CI.

## Non-goals for the current milestone

- SaaS or server sync;
- live Anki sync;
- storing review/scheduling state;
- a web app as the source of truth;
- legacy Python Brain Brew recipe compatibility as a public API;
- arbitrary unsupported adapter-data passthrough.

## Crate boundaries

```text
brain-brew-core     pure domain model, validation, compose, semantic diff
brain-brew-formats  YAML, CrowdAnki, manifest, lockfile, media codecs
brainbrew           filesystem, terminal output, command wiring (in crates/brain-brew-cli)
```

`brain-brew-core` must not depend on YAML, CrowdAnki, filesystem, terminal, or CLI concerns.

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
