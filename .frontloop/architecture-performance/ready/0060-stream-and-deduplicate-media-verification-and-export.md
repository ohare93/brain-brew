---
title: Stream and deduplicate media verification and export
priority: medium
---

## Goal

Avoid rereading/retaining the same common media bytes for every target while preserving per-package ownership and exact hash/reference failures.

## Acceptance Criteria

- Identical owned assets are hashed once per command execution
- Verification streams bytes within bounded memory
- Per-target reference diagnostics remain complete
- Export uses a validated declared-media plan and clean transaction
- Benchmarks cover UG common assets, duplicate references, and large individual files

## Implementation Notes

Depends on federated media ownership and transaction modules.
