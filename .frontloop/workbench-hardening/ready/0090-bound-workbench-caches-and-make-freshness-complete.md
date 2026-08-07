---
title: Bound Workbench caches and make freshness complete
priority: high
---

## Goal

Replace unbounded three-deck-per-selection caching and all-target cold media composition with bounded, shared, dependency-complete data structures.

## Acceptance Criteria

- Selection caches use a measured LRU/single-active policy and Arc sharing
- Transitive includes and locked dependencies participate in freshness
- Media catalogs do not compose every target on first request
- A 93-selection UG traversal stays within an approved RSS budget
- Cold/warm latency budgets and invalidation tests are enforced

## Implementation Notes

After provenance-aware planner; preserve generation guards.
