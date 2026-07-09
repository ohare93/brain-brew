---
title: Split core composition along behavior seams
priority: medium
---

## Goal

Decompose the oversized composition implementation into private compose, render, conflict, tombstone, and semantic-diff modules behind the existing deep core interface.

## Acceptance Criteria

- Public core interface does not grow merely to mirror implementation pieces
- Conflict/destructive semantics have one implementation location
- Rendering/message and semantic-diff code have focused private modules
- All correctness/property tests remain green
- Module split is behavior-neutral and follows completed core fixes

## Implementation Notes

Tidy-first structural work only after core-compose-correctness completes.
