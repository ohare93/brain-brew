---
title: Establish accessibility and performance regression budgets
priority: medium
---

## Goal

Turn Workbench accessibility, target verification, media, package discovery, and cache measurements into repeatable quality gates.

## Acceptance Criteria

- Keyboard/screen-reader semantics are tested for all Workbench workflows
- Budgets cover 74-target verify, cold/warm media, 93 selections, package discovery, and large media sets
- Benchmarks report CPU, wall time, RSS, reads, and payload sizes where relevant
- Thresholds are based on reproducible fixtures and allow intentional reviewed updates
- Scheduled reports distinguish correctness failures from performance regressions

## Implementation Notes

After architecture/performance fixes define expected baselines.
