---
title: Choose public Rust crate support commitment
priority: medium
---

## Goal

Decide whether core/formats are supported semver interfaces with examples and compatibility commitments or implementation crates published only to support the CLI.

## Acceptance Criteria

- Choose supported-public or implementation-package status
- Define semver/documentation expectations
- Align crates.io descriptions and repository documentation
- Identify required consumer tests/examples if public

## Implementation Notes

Does not block correctness work.

## Questions

### Q1: Recommended: treat core/formats as preview public interfaces only after extracted-package examples and semver policy exist; until then label them implementation packages. Approve or choose immediate public support?
