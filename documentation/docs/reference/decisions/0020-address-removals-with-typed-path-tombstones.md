# ADR-020: Address Removals with Typed Path Tombstones

**Date**: 2026-07-10  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Canonical decks previously stored tombstones as a flat set of `StableId` values. A note, note type, and media reference could legally share the same text identity, while nested field definitions and card templates are scoped by their parent note type. Flat IDs therefore aliased unrelated entities. Several remove operations also deleted data without recording removal history, and adding a note could erase an unrelated same-ID tombstone.

Removal history must survive ordered overlay composition, remain adapter-independent, and explain which overlay removed an address. Existing canonical files require a bounded migration path.

## Decision

Tombstones use an exhaustive typed `TombstoneAddress` with the complete containing path. Current variants cover every path that supports `intent: remove`: deck scalar values, variables and adapter IDs; note types and their scalar/map values; field definitions; card templates and their scalar/map values; notes and their variables, field values, tags, and adapter IDs; and media references. Nested field/template IDs include their parent note-type ID, and note field values include note and field IDs. Adding a future removable kind requires adding an explicit address variant and codec mapping.

A `TombstoneRecord` contains the address and optional migration provenance. Records created by composition always contain the removing overlay ID and `remove` operation. Canonical YAML writes records as explicit `kind` plus canonical `path`, and writes provenance when known.

All explicit add, merge, replace, remove, and override targets consult existing tombstones before mutation. Reusing an exact removed address fails with `tombstoned_address_reuse`; override does not clear the record. A container tombstone blocks mutations at every structurally contained address. Removing a container records only the container—not a generated record for every descendant—so output stays bounded while ancestor checks prevent ordering bypasses. Same StableId text in another kind or under another parent remains independent.

Composition retains the existing physical behavior of each removal: notes may remain as inactive retained records, while other entities are generally removed from their collection. Validation, translation, rendering, media analysis, and CrowdAnki export use active-address semantics when a typed source retains removed entities.

Flat canonical YAML such as `tombstones: [note.finland]` is compatibility input only. The reader infers `note`, `note_type`, or `media_reference` only when exactly one retained top-level identity in the loaded document matches. Zero matches and cross-kind/multiple matches fail with typed migration guidance. Bare IDs are never interpreted as nested field or template addresses. The canonical writer always emits typed records, so `brainbrew fmt deck.yaml` migrates unambiguous input. Empty output remains `tombstones: []`.

## Rationale

- Exhaustive variants make incomplete nested addresses unrepresentable.
- Full paths preserve parent scope without imposing global StableId uniqueness.
- Address-keyed records prevent duplicate exact tombstones and provide deterministic ordering.
- Container-only records avoid potentially unbounded descendant expansion while ancestor checks retain fail-closed behavior.
- Provenance makes ordered-stack failures actionable and prevents override from rewriting history.
- A narrow compatibility reader allows safe normalization without guessing nested ownership.

## Alternatives Considered

- **Keep flat IDs and require global uniqueness:** rejected because nested identities are intentionally parent-scoped and future kinds would remain fragile.
- **Store arbitrary strings/DeckPath values:** rejected because invalid collection or partial paths would remain representable and future removals could bypass explicit review.
- **Generate tombstones for every descendant:** rejected because large container removals would expand source and descendant sets can change across versions; ancestor blocking gives the required invariant with one record.
- **Allow add/override to clear a tombstone:** rejected because it erases removal provenance and permits stack-order reintroduction.
- **Infer nested legacy IDs:** rejected because a bare field/template ID does not identify its owner.

## Implications

- `CanonicalDeck::tombstones` is now `Tombstones`, keyed by `TombstoneAddress`, rather than `BTreeSet<StableId>`.
- CrowdAnki reports omitted typed note addresses, and JSON output includes tombstone kind/path records.
- Semantic diff paths use `tombstones.<canonical-path>` and include provenance changes.
- Format consumers must migrate non-empty flat tombstone YAML. Unambiguous files can run `brainbrew fmt`; ambiguous, unknown, and nested records require manual typed records.
- New removable operations are incomplete until they define an address variant, removal collection, mutation guard, YAML kind/path mapping, and active projection tests.
