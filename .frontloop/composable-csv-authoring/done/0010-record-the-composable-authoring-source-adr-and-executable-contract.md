---
title: Record the composable authoring-source ADR and executable contract
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-1
---

## Goal

Document the accepted boundary between authoring sources and the resolved canonical model before implementation. Fix the source-expression, ownership-transfer, translation, media, capability, and verification rules so later tasks can proceed without rediscovering product decisions.

## Acceptance Criteria

- Add a new ADR defining direct YAML notes and tagged CSV-backed notes as alternative source representations of the same resolved note map
- Define a composable, disjoint source expression for mixed CSV and inline note ownership, including explicit exclusions and fatal stable-ID collisions rather than ordered implicit overrides
- Define `translations.from_csv` as a source-preserving authoring declaration that materializes the existing TranslationDictionary categories rather than replacing localized note values
- Define import parameters for localized column suffixes separately from deck/template text variables, with empty language meaning no suffix and non-empty language adding the configured separator
- Define explicit table aliases, explicit join keys, exact UTF-8 headers, duplicate/missing-row behavior, blank-cell policy, stable note IDs, adapter IDs, tags, and scalar/image field semantics
- Define CSV-owned paths as read-only capabilities, record the loss of historical stale detection for live CSV-owned translation pairs, and exclude CSV write-back and legacy recipe parity
- Amend or supersede ADR-0005 and ADR-0008 as needed, update the decision index/project scope, and record that production UG changes remain gated until this epic's certification task passes
- Describe the required red-green-refactor workflow and fixture progression for every implementation slice

## Design Decisions

- CanonicalDeck, Overlay, and TranslationDictionary remain filesystem- and CSV-independent resolved domain types
- CSV is a first-class read-only authoring source, not a one-time importer and not a CrowdAnki adapter
- Pure inline `notes:` mappings remain source-compatible; CSV and mixed-source syntax must not require an empty placeholder note map
- Mixed ownership transfers are explicit and disjoint; silent last-writer-wins behavior is forbidden
- No Ultimate Geography repository changes belong in this epic

## Implementation Notes

Start from documentation/docs/reference/decisions/0005-store-maintainer-source-as-strict-canonical-yaml.md, 0008-use-source-variables-and-translation-dictionaries.md, project-scope.md, CONTEXT.md, canonical_source_document.rs, overlay_source_document.rs, and the existing UG fixture-contract Frontloop decision. This task precedes all implementation tasks.


## Completion Summary

- Accepted ADR-022 defining composable direct/CSV authoring sources, strict descriptors and joins, source-preserving translation materialization, explicit ownership transfer, read-only capabilities, and the UG certification gate.
- Added Authoring Source domain language, amended ADR-0005 and ADR-0008, and updated the ADR index and project scope.
- Incorporated Grok 4.5 and Fable review findings covering fixed schema wording, exclusions, locale validation, tracked inputs, stale detection, crate boundaries, BOM diagnostics, and adapter-ID blanks.

### Files Changed

- CONTEXT.md
- documentation/docs/reference/decisions/0005-store-maintainer-source-as-strict-canonical-yaml.md
- documentation/docs/reference/decisions/0008-use-source-variables-and-translation-dictionaries.md
- documentation/docs/reference/decisions/0022-use-composable-read-only-authoring-sources.md
- documentation/docs/reference/decisions/README.md
- documentation/docs/reference/project-scope.md
