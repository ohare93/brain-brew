---
title: Canonicalize the real UG source under the pinned tool
priority: critical
---

## Goal

Apply and review canonical formatting to production deck, Hardcore, language, extension, and variant sources—not only the fixture.

## Acceptance Criteria

- `brainbrew fmt --check` passes over every production source
- The current 20-file formatter delta is reviewed for semantic neutrality
- No include or structured content is lost
- All 74 main and 26 Hardcore targets compose after canonicalization
- Canonical source changes are committed independently from behavioral migration

## Implementation Notes

After pinned baseline; use strict duplicate/union decoder before accepting mass formatting.
