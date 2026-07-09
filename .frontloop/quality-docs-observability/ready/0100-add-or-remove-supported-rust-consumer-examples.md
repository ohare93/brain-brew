---
title: Add or remove supported Rust consumer examples
priority: low
---

## Goal

Implement the approved crate-support decision with compile-tested examples and stability notes, or reclassify package metadata to avoid unsupported reusable-interface claims.

## Acceptance Criteria

- Crate metadata and docs match the support decision
- If public, examples consume packaged crates outside the workspace and run in CI
- If internal, reusable-interface marketing and unsupported promises are removed
- Versioning and deprecation expectations are explicit

## Implementation Notes

Depends on public Rust crate support clarification and release package verification.
