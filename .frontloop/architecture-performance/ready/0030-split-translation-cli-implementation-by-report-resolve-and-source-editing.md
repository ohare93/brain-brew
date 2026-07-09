---
title: Split translation CLI implementation by report resolve and source editing
priority: medium
---

## Goal

Separate the 3k-line translation command implementation into private modules that consume centralized core policy and canonical source-document interfaces.

## Acceptance Criteria

- Report generation, resolution orchestration, source editing, and rendering are distinct private modules
- No translation policy remains duplicated with core or Workbench
- JSON/text renderers share typed report data
- Source editing uses the common mutation interface
- Behavior and output snapshots remain deterministic

## Implementation Notes

After translation-integrity and canonical-source-integrity migration tasks.
