# ADR-019: Use Canonical Entity Fingerprints for Complete Destructive Changes

**Date**: 2026-07-10  
**Status**: Accepted  
**Deciders**: Project Lead

> **Amendment:** Fingerprint v2 adds field RTL direction to field-definition and note-type fingerprints. Unchanged card-template, note, and media-reference encodings continue to emit v1 so their vectors remain stable; v1 and v2 text forms are both validated.

## Context

ADR-007 requires an expected value or fingerprint before destructive overlay changes. Complete note type, field definition, card template, note, and media operations previously accepted `entity_present`, and several paths checked only that some expected base had been supplied. A stale overlay could therefore erase a different current entity. YAML summaries would be formatting-dependent, while requiring maintainers to repeat complete prior entities would be noisy and error-prone.

Sparse note-field values are now semantic `FieldValue` values under ADR-018 and already have an exact typed equality model.

## Decision

Sparse/property changes continue to use exact typed expected values. Complete entity replacement, override, and removal use `EntityFingerprint`, with canonical text `sha256:v<version>:<lowercase digest>`.

Each version hashes a hand-defined tagged, length-prefixed canonical domain encoding. It domain-separates `brainbrew:<version>:<entity-kind>`, preserves semantic sequence order, sorts maps/sets, distinguishes every `FieldValue` and message-component variant, and includes stable IDs, adapter IDs, and all entity configuration. It does not use YAML, Debug, Serde, or platform output.

The covered complete families are note type, field definition, card template, note, and media reference. The actual current entity is fingerprinted immediately before every operation. Presence-only expected bases never authorize a destructive operation, including `override`. A stale override fails its precondition before conflict resolution.

Canonical YAML rejects legacy `expected_base: entity_present` with migration guidance. `brainbrew diff --as-overlay` generates fingerprints from the exact prior deck. JSON composition diagnostics expose stable code/category, DeckPath, entity kind, intent, overlay ID, expected state, and actual state.

## Rationale

- A compact digest checks every semantic property without duplicating prior entities in overlays.
- Domain/version separation prevents cross-kind and future-schema confusion.
- Hand-defined bytes remain stable across YAML formatting, map insertion, process, and platform.
- Tool generation prevents hand-derived or summary-string hashes.
- Comparing immediately before mutation protects ordered/concurrent overlay stacks and keeps `override` explicit rather than unsafe.

## Alternatives Considered

- **Presence only:** rejected because it cannot detect wrong-but-present or stale entities.
- **YAML/Serde/Debug bytes:** rejected because presentation and implementation details are not a stable domain contract.
- **Complete expected entity bodies:** rejected as verbose, difficult to migrate, and easy to copy incorrectly.
- **Sparse changes only:** retained as the preferred authoring style, but rejected as the only API because complete import/diff replacement and removal remain useful.

## Implications

- This is a breaking overlay-schema migration for presence-only destructive operations.
- Changing canonical bytes requires a new schema version; existing v1 vectors remain fixed.
- Maintainers regenerate affected overlays from the intended exact base.
- Complete note and note-type bodies are valid for `replace`/`override` only with the matching fingerprint; `merge` remains sparse authoring except complete field/template/media compatibility paths, which are fingerprint-protected.
- ADR-020 now defines typed/path-addressed tombstone storage; this decision remains responsible for checking the exact entity state before the removal record is created.
