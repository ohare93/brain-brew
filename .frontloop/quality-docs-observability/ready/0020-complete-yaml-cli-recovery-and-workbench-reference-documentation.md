---
title: Complete YAML CLI recovery and Workbench reference documentation
priority: high
---

## Goal

Document the actual schema and supported workflows, including variables, sparse overlays, manifests, locks, stable IDs, include preservation, stale resolution, recovery, and API maturity.

## Acceptance Criteria

- YAML reference covers all canonical/overlay/manifest/lock fields and stable-ID grammar
- Include materialization wording matches tested behavior
- CLI reference reflects required options and JSON contracts
- Recovery docs cover lock update, source transactions, Workbench drafts/conflicts, and release gates
- Legacy Iced and broad-pivot descriptions are removed

## Implementation Notes

Generate schema tables where practical to prevent drift.
