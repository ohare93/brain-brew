---
title: Core composition correctness
slug: core-compose-correctness
status: active
created_at: 2026-07-09
completed_at:
---

## Goal

Restore fail-closed semantics in the pure domain: representation-aware field values, real destructive-change preconditions, typed tombstones, correct structured-message resolution, complete semantic differences, and machine-readable errors.

## Sequence

Decide precondition and tombstone representations first. Implement semantic field values before destructive checks, then messages and semantic diff, and finish by stabilizing error interfaces and exhaustive regression laws.
