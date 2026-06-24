# Architecture Decision Records

This directory contains the current Architecture Decision Records (ADRs) for Brain Brew.

The ADRs are written in hindsight around the decisions that still shape the Rust-based, local-first deck federation architecture. Superseded historical ADRs are not kept in this directory; use repository history for older experiments and abandoned directions.

## Decision Log

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [0001](0001-scope-brain-brew-as-local-first-deck-federation.md) | Scope Brain Brew as Local-First Deck Federation | Accepted | 2026-05-25 |
| [0002](0002-use-rust-workspace-with-pure-core-and-format-codecs.md) | Use Rust Workspace with Pure Core and Format Codecs | Accepted | 2026-05-25 |
| [0003](0003-use-canonical-deck-as-federation-format.md) | Use Canonical Deck as Federation Format | Accepted | 2026-05-25 |
| [0004](0004-identify-deck-entities-with-human-readable-stable-ids.md) | Identify Deck Entities with Human-Readable Stable IDs | Accepted | 2026-05-25 |
| [0005](0005-store-maintainer-source-as-strict-canonical-yaml.md) | Store Maintainer Source as Strict Canonical YAML | Accepted | 2026-05-25 |
| [0006](0006-compose-federated-decks-with-ordered-sparse-overlays.md) | Compose Federated Decks with Ordered Sparse Overlays | Accepted | 2026-05-25 |
| [0007](0007-require-explicit-conflict-and-destructive-change-semantics.md) | Require Explicit Conflict and Destructive-Change Semantics | Accepted | 2026-05-25 |
| [0008](0008-use-source-variables-and-translation-dictionaries.md) | Use Source Variables and Translation Dictionaries | Accepted | 2026-05-25 |
| [0009](0009-use-manifests-targets-and-locks-before-a-recipe-dsl.md) | Use Manifests, Targets, and Locks Before a Recipe DSL | Accepted | 2026-05-25 |
| [0010](0010-fail-closed-on-unsupported-adapter-data.md) | Fail Closed on Unsupported Adapter Data | Accepted | 2026-05-25 |
| [0011](0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md) | Use a Local Deck Workbench Server with an Iced/WASM UI | Accepted | 2026-06-23 |
| [0012](0012-add-manifest-language-catalog-and-translation-profile.md) | Add Manifest Language Catalog and Translation Profile | Accepted | 2026-06-23 |
| [0013](0013-use-stale-translation-records-for-source-text-changes.md) | Use Stale Translation Records for Source Text Changes | Accepted | 2026-06-23 |
| [0014](0014-require-workbench-api-and-browser-e2e-tests.md) | Require Workbench API and Browser E2E Tests | Accepted | 2026-06-23 |
| [0015](0015-use-lazy-single-work-item-workbench-editing.md) | Use Lazy Single-Work-Item Workbench Editing | Accepted | 2026-06-24 |

## Process

### Creating a New ADR

1. Copy the structure from an existing ADR to `NNNN-title.md`, where `NNNN` is the next sequential number.
2. Fill in Context, Decision, Rationale, Alternatives Considered, and Implications.
3. Set status to `Proposed` while under discussion.
4. Update this decision log when the ADR is accepted, superseded, or deprecated.

### ADR Lifecycle

- **Proposed**: under discussion.
- **Accepted**: decision made and currently governing implementation.
- **Deprecated**: no longer relevant.
- **Superseded**: replaced by a newer ADR.

### Integration with Development

- Reference ADRs in code comments for architectural decisions when useful.
- Include ADR links in PR or change descriptions for architectural changes.
- Review ADRs during architectural discussions and update them when the architecture changes.
