# ADR-034: Use Minimal Federated Deck Manifest

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew has now proven the first CLI semantics for formatting, validation, composition, CrowdAnki import/export, semantic diffing, and Ultimate Geography migration. Full Ultimate Geography reproduction needs a way to declare every reproducible build target, including the base deck, ordered overlays, and verification/export commands usable in CI.

ADR-028 deferred a recipe/manifest system until those CLI semantics stabilized. Continuing with only ad hoc CLI invocations would make large deck federations harder to reproduce and harder to verify consistently. A full recipe DSL would risk over-scoping the project before the federation package model is proven.

## Related Decisions

- [ADR-015: Use Ordered Overlay Stack with Explicit Conflicts](0015-use-ordered-overlay-stack-with-explicit-conflicts.md) - manifests declare ordered overlay stacks for build targets.
- [ADR-017: Require Byte-Stable Canonicalized Source Round Trips](0017-require-byte-stable-canonicalized-source-round-trips.md) - manifests must support deterministic source and build verification.
- [ADR-018: Use Single Canonical Deck File as Source of Truth](0018-use-single-canonical-deck-file-as-source-of-truth.md) - a manifest references Canonical Deck files; it does not replace the Canonical Deck format.
- [ADR-028: Defer Recipe System Until CLI Semantics Stabilize](0028-defer-recipe-system-until-cli-semantics-stabilize.md) - this decision introduces a minimal manifest now that the initial CLI semantics have stabilized.

## Decision

Introduce a small public Federated Deck manifest for reproducible deck builds. The default manifest filename is `brainbrew.yaml`.

The manifest declares:

- the base Canonical Deck file;
- an overlay catalog with named overlays, paths, kinds, and dependencies;
- named build targets that select overlays from the catalog;
- enough metadata for CLI verification and export workflows.

The first `verify --all-targets` scope is manifest parsing, canonical source formatting checks, dependency expansion, target composition, and target validation. Adapter export determinism and golden export comparison can be layered on later without changing the core manifest concept.

Overlay dependencies are inclusion dependencies: selecting an overlay also selects its dependencies. Dependencies are expanded deterministically into an ordered overlay stack before composition. Cycles, missing dependencies, or ambiguous ordering fail validation. After expansion, the normal ordered compose and explicit conflict rules still apply.

The manifest is intentionally not a general-purpose recipe language. It describes reproducible deck composition targets and delegates behavior to existing CLI commands and library semantics.

A manifest target may hide multiple implementation overlay files behind a simple user-facing choice, such as a language/variant target. Users and CI select named targets; maintainers can still factor overlays internally to avoid duplication through overlay dependencies.

## Rationale

A minimal manifest gives deck maintainers and CI a stable, reviewable source for all intended resolved decks. It also gives future GUI and Anki add-on work a single backend contract for listing targets and composing selected overlays.

This keeps the project aligned with the local-first deck federation model while avoiding a premature workflow DSL.

## Alternatives Considered

- **CLI arguments only**: smallest implementation, but weak for CI, hard to audit, and unfriendly for large federations such as Ultimate Geography.
- **Full recipe DSL now**: powerful, but would reintroduce the scope ADR-028 deliberately deferred and could freeze unproven workflow concepts.

## Implications

- The CLI should learn to discover `brainbrew.yaml` by default, read a manifest, expand overlay dependencies deterministically, and operate on named targets.
- The Ultimate Geography parity fixture should use the same Federated Deck manifest shape as any other large deck, with a base deck, overlays, and media references.
- CI can verify all manifest targets deterministically.
- Future UI or Anki add-on work can use the manifest to discover build targets and overlay choices without exposing every internal overlay file directly.
