---
title: Choose the UG fixture contract
priority: critical
---

## Goal

Decide whether Brain Brew's UG fixture is an exact production snapshot, production plus an explicit generated migration patch, or an intentionally future-facing fixture paired with a mandatory live-consumer gate.

## Acceptance Criteria

- Select one fixture contract and owner
- Define how sync detects drift and what generated changes require review
- Specify whether fixture-only language/profile/media migrations are permitted
- Define the mandatory production-consumer CI signal

## Implementation Notes

Blocks fixture synchronization redesign but not production canonicalization.

## Questions

### Q1: Recommended: track an exact pinned production snapshot plus an explicit reviewed migration transform, and require both fixture and live-consumer gates. Approve or choose another contract?
