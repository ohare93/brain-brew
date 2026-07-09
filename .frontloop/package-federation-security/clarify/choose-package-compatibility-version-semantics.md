---
title: Choose package compatibility version semantics
priority: high
---

## Goal

Decide whether package compatibility uses exact versions, real semantic ranges, or removes the currently inert field until implemented.

## Acceptance Criteria

- Select exact, semver-range, or temporary removal semantics
- Define validation for base and extension package relationships
- Specify manifest migration and diagnostics
- Add examples covering compatible and incompatible package sets

## Implementation Notes

Blocks compatibility enforcement but not path/hash hardening.

## Questions

### Q1: Recommended: implement real semver ranges if federation is promoted from experimental; otherwise remove `compatible_base_versions` until promotion. Which posture should this preview take?
