# Architecture Decision Records

This directory contains active Architecture Decision Records (ADRs) for the fresh-start Brain Brew architecture.

Earlier pre-fresh-start ADRs are preserved in [`archive/`](archive/) for historical context. New implementation work should follow the active ADRs below unless deliberately revisiting a recorded decision.

## Decision Log

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [0011](0011-narrow-scope-to-local-first-deck-federation.md) | Narrow Scope to Local-First Deck Federation | Accepted | 2026-05-22 |
| [0012](0012-use-canonical-deck-as-federation-format.md) | Use Canonical Deck as Federation Format | Accepted | 2026-05-22 |
| [0013](0013-use-stable-ids-as-primary-identity.md) | Use Stable IDs as Primary Identity | Accepted | 2026-05-22 |
| [0014](0014-allow-overlays-to-target-any-deck-entity.md) | Allow Overlays to Target Any Deck Entity | Accepted | 2026-05-22 |
| [0015](0015-use-ordered-overlay-stack-with-explicit-conflicts.md) | Use Ordered Overlay Stack with Explicit Conflicts | Accepted | 2026-05-22 |
| [0016](0016-represent-overlays-as-sparse-canonical-deck-fragments.md) | Represent Overlays as Sparse Canonical Deck Fragments | Accepted | 2026-05-22 |
| [0017](0017-require-byte-stable-canonicalized-source-round-trips.md) | Require Byte-Stable Canonicalized Source Round Trips | Accepted | 2026-05-22 |
| [0018](0018-use-single-canonical-deck-file-as-source-of-truth.md) | Use Single Canonical Deck File as Source of Truth | Accepted | 2026-05-22 |
| [0019](0019-use-strict-canonical-yaml-for-canonical-deck-files.md) | Use Strict Canonical YAML for Canonical Deck Files | Accepted | 2026-05-22 |
| [0020](0020-use-rust-for-core-and-cli.md) | Use Rust for Core and CLI | Accepted | 2026-05-22 |
| [0021](0021-structure-rust-workspace-around-reusable-core.md) | Structure Rust Workspace Around Reusable Core | Accepted | 2026-05-22 |
| [0022](0022-preserve-anki-compatible-deck-semantics.md) | Preserve Anki-Compatible Deck Semantics | Accepted | 2026-05-22 |
| [0023](0023-exclude-review-state-from-canonical-deck.md) | Exclude Review State from Canonical Deck | Accepted | 2026-05-22 |
| [0024](0024-store-media-as-external-assets-with-references.md) | Store Media as External Assets with References | Accepted | 2026-05-22 |
| [0025](0025-use-strict-validation-by-default.md) | Use Strict Validation by Default | Accepted | 2026-05-22 |
| [0026](0026-fail-on-unsupported-adapter-data.md) | Fail on Unsupported Adapter Data | Accepted | 2026-05-22 |
| [0027](0027-require-expected-base-for-destructive-overlay-changes.md) | Require Expected Base for Destructive Overlay Changes | Accepted | 2026-05-22 |
| [0028](0028-defer-recipe-system-until-cli-semantics-stabilize.md) | Defer Recipe System Until CLI Semantics Stabilize | Accepted | 2026-05-22 |
| [0029](0029-use-human-readable-stable-ids-with-separate-adapter-ids.md) | Use Human-Readable Stable IDs with Separate Adapter IDs | Accepted | 2026-05-22 |
| [0030](0030-review-suggested-stable-ids-during-import.md) | Review Suggested Stable IDs During Import | Accepted | 2026-05-22 |
| [0031](0031-key-canonical-deck-entities-by-stable-id.md) | Key Canonical Deck Entities by Stable ID | Accepted | 2026-05-22 |
| [0032](0032-represent-removals-as-tombstones.md) | Represent Removals as Tombstones | Accepted | 2026-05-22 |
| [0033](0033-keep-core-domain-pure-with-separate-format-codecs.md) | Keep Core Domain Pure with Separate Format Codecs | Accepted | 2026-05-22 |
| [0034](0034-use-minimal-federated-deck-manifest.md) | Use Minimal Federated Deck Manifest | Accepted | 2026-05-23 |
| [0035](0035-use-language-neutral-stable-ids-for-translated-targets.md) | Use Language-Neutral Stable IDs for Translated Targets | Accepted | 2026-05-23 |
| [0036](0036-model-deck-variants-as-extension-overlays.md) | Model Deck Variants as Extension Overlays | Accepted | 2026-05-23 |
| [0037](0037-do-not-build-legacy-source-importers-for-initial-federation.md) | Do Not Build Legacy Source Importers for Initial Federation | Accepted | 2026-05-23 |
| [0038](0038-use-manifest-package-metadata-and-target-checks.md) | Use Manifest Package Metadata and Target Checks | Accepted | 2026-05-23 |
| [0039](0039-use-source-variables-and-translation-dictionaries.md) | Use Source Variables and Translation Dictionaries for Translation Overlays | Accepted | 2026-05-23 |
| [0040](0040-use-locked-federated-package-inputs.md) | Use Locked Federated Package Inputs | Accepted | 2026-05-24 |
| [0041](0041-continue-as-rust-based-brain-brew.md) | Continue as Rust-Based Brain Brew | Accepted | 2026-05-25 |

## Process

### Creating a New ADR

1. Copy `template.md` to `NNNN-title.md` where NNNN is the next sequential number
2. Fill in all sections completely
3. Set status to "Proposed"
4. Create pull request for review
5. Update status to "Accepted" when merged

### ADR Lifecycle

- **Proposed**: Under discussion
- **Accepted**: Decision made and implemented
- **Deprecated**: No longer relevant
- **Superseded**: Replaced by newer ADR

### Integration with Development

- Reference ADRs in code comments for architectural decisions
- Include ADR links in PR descriptions for architectural changes
- Review ADRs during architectural discussions
