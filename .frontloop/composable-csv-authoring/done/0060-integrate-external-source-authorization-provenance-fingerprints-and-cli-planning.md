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


## Completion Summary

- Added one authorized filesystem loader for CSV descriptors and tables, resolving relative to the referring source while enforcing existing safe-relative and selected package-root/symlink confinement rules.
- Registered every loaded descriptor and primary/joined table as deterministic authoritative sources and propagated their exact materialized-byte hashes through plans, explain output, locks, verification, and Workbench freshness.
- Routed direct and package canonical deck reads—including validate, fmt, compose, diff, verify, and CrowdAnki export—through the shared CSV materialization path with literal/default parameters and no ambient target inference.
- Made canonical CSV formatting fail closed when authoritative sources are missing, unsafe, or malformed instead of falling back to syntax-only formatting.
- Added security, package/include/join, command-parity, deterministic explain, byte-invalidation, lock, and freshness regressions; passed fresh Grok security review, focused suites, full workspace tests, fmt, and clippy.

### Files Changed

- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/src/commands/fmt.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/path_authorization.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/tests/csv_authoring_sources.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/tests/csv_note_sources.rs
- .frontloop/composable-csv-authoring/done/0060-integrate-external-source-authorization-provenance-fingerprints-and-cli-planning.md
