---
title: Deepen historical parity across all target families
priority: high
---

## Goal

Replace the 12-target name/count signature with immutable comparisons of templates, CSS, field schema/order, config, notes, tags, identity, and media content.

## Acceptance Criteria

- Parity sampling covers all materially distinct target families or all 100 targets where feasible
- Baseline revisions and artifacts are immutable
- Model/template/config/media signatures observe every release-relevant property
- Differences are classified and reviewed, not silently normalized
- The evidence script fails on missing baseline data

## Implementation Notes

Build on committed goldens; keep historical and current-tool oracles independent.
