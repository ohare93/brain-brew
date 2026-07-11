---
title: Bound and prune recursive package discovery
priority: medium
---

## Goal

Prevent irrelevant trees and symlink cycles from making package-root discovery slow, nondeterministic, or unsafe.

## Acceptance Criteria

- Discovery prunes `.git`, `.jj`, target, generated output, cache, and configured ignore directories
- Symlink cycles cannot recurse indefinitely
- Maximum depth/file budgets are configurable with actionable errors
- Traversal order is deterministic
- Large-tree benchmarks and fixtures cover pruning and duplicate manifests

## Implementation Notes

Last federation scalability task after path semantics are centralized.


## Completion Summary

- Added one registry-owned DiscoveryPolicy/Result/Stats/Error path for every package-root route and translation suggestions
- Added deterministic sorted no-follow traversal with root/component symlink, special entry, permission, replacement, and directory identity checks
- Pruned exact VCS/devenv/target/build/output/dist/site/node_modules/docs/cache/transaction/Nix-result names before descent
- Added repeatable SafeRelativePath-authorized component glob ignores with documented *, ?, and ** semantics
- Added validated configurable depth/entry/manifest defaults and hard maxima with actionable budget diagnostics
- Exposed discovery stats in targets JSON and added pruning/symlink/budget/duplicate/override/route/moderate-count regressions
- Passed CPU-bounded full CI, focused discovery/planner/path tests, E2E rerun, docs, release smoke, and Claude judgment

### Files Changed

- CHANGELOG.md
- crates/brain-brew-cli/src/package_resolver.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/src/args.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/commands/compose.rs
- crates/brain-brew-cli/src/commands/validate.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/targets.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/package_discovery.rs
- documentation/docs/authoring/packages-locking.md
- documentation/docs/reference/cli.md
