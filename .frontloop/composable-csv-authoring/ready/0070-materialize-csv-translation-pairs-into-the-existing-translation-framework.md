---
title: Materialize CSV translation pairs into the existing translation framework
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-7
---

## Goal

Allow translation overlays to declare `translations.from_csv`, pairing English/source and localized target columns and producing ordinary validated direct, contextual, no-change, adaptation, deletion, and adapter-ID translation semantics in memory.

## Acceptance Criteria

- Parse and preserve a `translations.from_csv` declaration alongside disjoint inline YAML translation entries
- Pair each localized mapped field's unsuffixed source column with its parameterized target column using the note source descriptor and stable note paths
- Generate one reusable direct translation when every occurrence of a source has one distinct non-equal target
- Generate contextual entries for every affected occurrence when one source maps to multiple targets, without choosing a majority value
- Generate no-change only when its global semantics are valid; use contextual source-equal decisions when a reusable direct translation exists elsewhere
- Handle empty-source/non-empty-target as an imported target adaptation and non-empty-source/empty-target according to the ADR's explicit legacy-parity deletion policy; ignore both-empty pairs
- Generate adapter-ID translation maps, especially source GUID to localized GUID, through existing TranslationDictionary semantics
- Merge imported and inline entries transactionally: allow disjoint or identical ownership and reject semantic conflicts
- Run existing translation validation, coverage, composition, and export unchanged over the materialized dictionary
- Add RGR fixtures covering deduplication, contextual conflicts, no-change, source/target-only values, blank deletion, GUIDs, strict coverage, and source-preserving formatting

## Design Decisions

- CSV does not apply localized note replacements; it authors an ordinary TranslationDictionary
- Existing core translation precedence and validation remain authoritative
- Imported adaptations retain CSV file/row/column provenance and an explicit legacy-import reason/review status
- Historical stale detection is unavailable while both source and target keys regenerate from live CSV and must be documented

## Implementation Notes

Depends on joins/parameters and authorized loading. Reuse brain-brew-core translation APIs and invariants rather than duplicating translation application in formats or CLI. Coordinate with translation-integrity and architecture-performance translation tasks.
