---
title: Integrate external-source authorization, provenance, fingerprints, and CLI planning
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-6
---

## Goal

Make CSV declarations safe and complete across direct-file commands, manifests, packages, locks, planners, freshness checks, explain/verify, and rebuild invalidation before treating the feature as usable.

## Acceptance Criteria

- Authorize every descriptor and CSV path with existing package-root and safe-relative-path rules, including symlink and traversal rejection
- Add descriptors and all joined CSV files to source provenance, registered source enumeration, target plan sources, fingerprints/locks, and Workbench freshness signatures
- Ensure validate, fmt, compose, semantic diff, verify, and CrowdAnki export use one materialization path and report actionable source locations
- Define and test direct-file behavior when source parameters require a manifest/target context
- Make changes to any authoritative CSV invalidate the correct plan and verification result
- Cover root workspaces, package-owned sources, includes, missing files, unsafe paths, and deterministic explain output in CLI integration tests
- Apply red-green-refactor with security/path tests written before the corresponding loader behavior

## Design Decisions

- The formats crate remains filesystem-free; CLI/package loaders own I/O and authorization
- No authoritative CSV may be omitted from fingerprints or freshness
- The feature remains fail-closed until this integration is complete

## Implementation Notes

Depends on note source and join support. Primary seams: crates/brain-brew-cli/src/io.rs, planner.rs, lock/verify commands, source provenance kinds, and safe path tests. Coordinate with architecture-performance tasks that plan planner/translation module splits.
