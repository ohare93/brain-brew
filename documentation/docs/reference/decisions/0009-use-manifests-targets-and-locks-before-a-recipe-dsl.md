# ADR-009: Use Manifests, Targets, and Locks Before a Recipe DSL

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Legacy Brain Brew recipes and earlier Note Nexus planning both pointed toward declarative, reproducible deck builds. That goal is still useful, but a general transformation DSL would be too much surface area before Canonical Deck, overlays, targets, adapter export, and package composition are stable.

Maintainers still need a way to name package metadata, declare available overlays, define build targets, depend on other federated packages, and lock inputs for CI.

## Decision

Use minimal federated deck manifests, named targets, and lockfiles for the current milestone.

A `brainbrew.yaml` manifest declares package metadata, source files, overlay catalogs, and build targets. A build target resolves one base deck plus selected overlays into a resolved deck. `brainbrew.lock` records federated package inputs and revisions for reproducible builds. Brain Brew does not expose a general recipe DSL or promise legacy Python recipe compatibility in the current milestone.

## Rationale

**Pros:**

- Keeps the reproducible-build spirit of recipes without committing to a full language.
- Gives CI and downstream packages stable target names.
- Makes dependencies explicit and lockable.
- Leaves room for a future recipe DSL to build on proven semantics.

**Cons:**

- Some custom transformations still require external preprocessing.
- Users with complex legacy recipes need migration work.
- Manifests are less expressive than a full pipeline language.

## Alternatives Considered

- **Full recipe DSL now**: rejected because it would obscure the core federation model too early.
- **Legacy Python recipe compatibility as public API**: rejected because old semantics do not match the new Canonical Deck overlay model cleanly.
- **Ad hoc CLI arguments only**: rejected because targets, packages, and CI need named source-controlled declarations.
- **Native Nix expressions as recipes**: rejected because they would be powerful but too high-friction for deck maintainers.

## Implications

- Manifest and lockfile codecs belong in `brain-brew-formats`.
- CLI commands should operate on named targets where possible.
- Package locks are part of reproducible federation, not an optional cache detail.
- Future recipe work should reuse manifest targets and composition semantics instead of replacing them.
