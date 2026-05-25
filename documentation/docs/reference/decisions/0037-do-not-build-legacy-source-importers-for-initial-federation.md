# ADR-037: Do Not Build Legacy Source Importers for Initial Federation

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Many existing shared decks are maintained in project-specific source layouts such as CSV files, scripts, templates, or other build systems. Ultimate Geography uses such a legacy source layout through Brain Brew and is valuable as a large parity fixture.

A tempting path is to build importers for those legacy source layouts. That would make one-time migration easier for some projects, but it risks turning Brain Brew into a collection of source-layout converters rather than a general deck federation and round-trip system.

The desired maintainer workflow is to refactor existing sources into Canonical Deck and overlay files, potentially with AI assistance, then prove success by composing/exporting and comparing against existing release artifacts.

## Related Decisions

- [ADR-018: Use Single Canonical Deck File as Source of Truth](0018-use-single-canonical-deck-file-as-source-of-truth.md) - legacy CSV/source layouts are not the primary source-of-truth format.
- [ADR-028: Defer Recipe System Until CLI Semantics Stabilize](0028-defer-recipe-system-until-cli-semantics-stabilize.md) - source-layout recipe compatibility should not lead the design.
- [ADR-034: Use Minimal Federated Deck Manifest](0034-use-minimal-federated-deck-manifest.md) - reproducible workflows are declared as canonical deck/overlay targets, not legacy source imports.

## Decision

Do not build public legacy source importers as part of the initial federation system.

Deck maintainers migrate by authoring or refactoring into Canonical Deck files, overlays, and a Federated Deck manifest. Correctness is proven by deterministic compose/export results and semantic comparison against known-good adapter outputs such as existing CrowdAnki releases.

Ultimate Geography remains a parity fixture and case study. It should exercise the generic Canonical Deck, overlay, manifest, validation, compose, and CrowdAnki export behavior rather than become a special source-import feature.

## Rationale

This keeps Brain Brew focused on being a better general deck federation/build system rather than a one-time migration converter. It also aligns with the idea that source migration can be assisted externally, including by AI, while Brain Brew provides the precise canonical target format and verification tools.

## Alternatives Considered

- **Build project-specific importers**: useful for one-time migration, but product scope expands quickly and public APIs become tied to legacy layouts.
- **Build a generic source-adapter framework now**: extensible, but effectively starts a recipe/source ETL system before the canonical federation workflow is mature.

## Implications

- Public CLI commands should be generic and operate on Canonical Deck files, overlays, manifests, and adapter exports.
- Ultimate Geography-specific source import code should not become a public product surface.
- The Ultimate Geography parity fixture should ultimately be represented as canonical source files in the repository, with tests proving exported parity against legacy CrowdAnki artifacts.
