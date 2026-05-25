---
title: Continue as Rust-Based Brain Brew
---

# ADR-0041: Continue as Rust-Based Brain Brew

## Status

Accepted

## Date

2026-05-25

## Context

The deck federation and round-trip engine began under a temporary working name, while the existing Brain Brew repository still held the legacy Python recipe-based implementation. The project should continue under the established Brain Brew name instead of introducing a separate product and repository.

The current architecture is a Rust implementation centered on Canonical Deck source, overlays, manifests, package locks, CrowdAnki import/export, and CI-friendly verification. The legacy Python Brain Brew recipe pipeline remains historically important, but recipe compatibility would constrain the new federation model before its CLI semantics are stable.

The Brain Brew repository is licensed under the Unlicense, and the Rust continuation should keep that repository license.

## Decision

Continue the Rust deck federation implementation as **Brain Brew** in the `brain-brew` repository.

Use these public names by default:

- human name: Brain Brew;
- CLI binary: `brainbrew`;
- manifest file: `brainbrew.yaml`;
- lock file: `brainbrew.lock`;
- Rust crates: `brain-brew-core`, `brain-brew-formats`, and `brain-brew-cli`;
- Nix package and app: `brainbrew`.

Keep the repository licensed under the Unlicense.

Treat the old Python recipe-based implementation as **legacy Python Brain Brew**. It may remain available through historical branches, bookmarks, releases, or package artifacts, but compatibility with the legacy recipe format is not a public API for the current Rust milestone.

## Consequences

- User-facing documentation and examples describe the Rust federation workflow as Brain Brew.
- Existing Brain Brew users see this as a continuation rather than a separate replacement product.
- Legacy recipe compatibility remains a possible future migration aid, not an architectural requirement.
- Downstream decks such as Ultimate Geography should describe their migration as staying on Brain Brew, now using Rust-based Brain Brew instead of legacy Python Brain Brew.
- Packaging, cache directories, manifests, lock files, and CLI help use `brainbrew` names consistently.
