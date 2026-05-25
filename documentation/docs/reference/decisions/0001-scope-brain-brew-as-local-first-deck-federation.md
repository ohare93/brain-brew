# ADR-001: Scope Brain Brew as Local-First Deck Federation

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew has had several possible directions: legacy Python recipe builds, a broader Note Nexus-style note sync system, a web-first application, and Dropbox-style bidirectional sync. The clearest proven need is narrower: shared Anki-compatible deck maintainers need to compose base decks with translations, extensions, patches, and personal changes without copying the whole deck or losing round-trip fidelity.

Ultimate Geography is the motivating case study. It needs repeatable local builds, version-controlled source, language variants, extension variants, media handling, and CI-friendly verification more than it needs SaaS, live sync, or a web app as the source of truth.

## Decision

Brain Brew is a Rust-based, local-first deck federation and round-trip engine for shared Anki-compatible decks.

The current product surface is a CLI/library workflow for deck maintainers. The current milestone excludes SaaS sync, live Anki sync, review-state storage, a web app as source of truth, and public compatibility with legacy Python Brain Brew recipes.

## Rationale

**Pros:**

- Focuses the project on a concrete maintainer workflow.
- Keeps deck source local, inspectable, and version-controllable.
- Makes deterministic CI verification a first-class use case.
- Preserves the useful part of the older federation idea while avoiding the hardest live-sync problems.

**Cons:**

- Learners do not get live personal sync in the current milestone.
- Existing legacy Python recipe users need migration guidance rather than drop-in compatibility.
- A future GUI or SaaS product must build on this smaller foundation later.

## Alternatives Considered

- **Web-first product**: rejected for now because browser file access and hosted sync distract from source fidelity and deck composition.
- **Live bidirectional sync**: rejected for now because it introduces sync metadata, conflict UI, and scheduling concerns outside deck source.
- **Legacy recipe compatibility first**: rejected because it would freeze the new model around old recipe semantics before federation semantics are stable.

## Implications

- Architectural work is judged against local deck federation and round-trip behavior.
- Documentation should describe Brain Brew as a maintainer tool, not a universal note sync service.
- CLI verification and reproducible local builds are part of the core product, not secondary tooling.
- Deferred product ideas can return only after the federation model is stable.
