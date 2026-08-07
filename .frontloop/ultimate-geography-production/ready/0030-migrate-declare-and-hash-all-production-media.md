---
title: Migrate declare and hash all production media
priority: critical
---

## Goal

Replace empty/inline media declarations with complete structured references and verified hashes rooted in production media bytes.

## Acceptance Criteria

- All 546 base and five Experimental empty hashes are populated from verified bytes
- Raw/inline references are migrated or explicitly allowlisted
- Every referenced asset is declared and every declared asset is present according to policy
- Main and Hardcore verify/export succeed with media validation
- Generated outputs copy only declared assets and no blanket `cp media/*` masks gaps

## Implementation Notes

After canonical source and safe media mutation tooling.
