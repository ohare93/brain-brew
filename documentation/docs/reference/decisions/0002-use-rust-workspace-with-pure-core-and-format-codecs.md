# ADR-002: Use Rust Workspace with Pure Core and Format Codecs

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew needs reliable deck transformations, deterministic validation, reusable import/export codecs, and a distributable CLI. Earlier exploration favored Gren for cross-platform type safety, while legacy Brain Brew used Python recipes. The current implementation is a Rust rewrite with clear crate boundaries.

## Decision

Use Rust for the core domain model, format codecs, and CLI.

Structure the workspace around these crates:

- `brain-brew-core`: pure domain model, validation, composition, semantic diff;
- `brain-brew-formats`: YAML, CrowdAnki, manifest, lockfile, and media codecs;
- `brainbrew`: thin CLI package in `crates/brain-brew-cli` for filesystem access, prompts, command wiring, and report rendering.

`brain-brew-core` must not depend on YAML, CrowdAnki, filesystem, terminal, or CLI concerns.

## Rationale

**Pros:**

- Rust gives strong static guarantees for source-to-deck transformations.
- The ecosystem supports robust CLI distribution, serde codecs, testing, and Nix packaging.
- Pure domain code is easier to test than code coupled to files or terminal output.
- Separate format crates keep adapter decisions from leaking into federation semantics.

**Cons:**

- Rust has a higher contributor learning curve than Python.
- Browser-first reuse is less direct than with a compile-to-JavaScript language.
- Some UI experiments may need bindings or a separate frontend later.

## Alternatives Considered

- **Gren as primary language**: attractive for typed functional transformations, but less practical for the current Rust/Nix/CLI distribution path.
- **Continue legacy Python Brain Brew**: rejected because the new federation model needs stronger domain boundaries and deterministic validation.
- **TypeScript CLI and web app**: rejected for now because the current milestone is local CLI/library behavior, not web-first UX.

## Implications

- Domain behavior should enter through tests in `brain-brew-core` first.
- Format behavior belongs in `brain-brew-formats`, even when exposed through the CLI.
- The CLI should stay thin and avoid owning deck semantics.
- New dependencies should be evaluated against these crate boundaries.
