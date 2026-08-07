---
title: Automate reproducible declared-media release packaging
priority: high
---

## Goal

Produce correctly named and validated UG release archives from composed targets without recopying the entire media tree.

## Acceptance Criteria

- Archive names/layout derive from one version/source-of-truth model
- Only declared verified media is included
- Every archive contains a valid CrowdAnki deck and passes import/smoke validation
- Main and companion outputs cannot overwrite or contaminate one another
- Release descriptions read version text from one canonical source

## Implementation Notes

Depends on media completion and legacy workflow disposition.
