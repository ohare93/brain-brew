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

## Decision

Implement real SemVer compatibility for the hardened federation preview. Dependencies identify an exact package version; an extension declares `base_package` plus an OR-list of canonical SemVer requirements, with comma-separated comparators interpreted as AND and prereleases following the `semver` crate. Compatibility validates the exact selected/locked dependency; Brain Brew does not solve versions. Invalid/noncanonical versions or requirements fail closed with manifest context. ADR-0017 and the schema references record the migration.

Agent Tick was unavailable during execution, so the orchestrator applied the real-SemVer option while carrying out the user's instruction to complete the full federation hardening program.
