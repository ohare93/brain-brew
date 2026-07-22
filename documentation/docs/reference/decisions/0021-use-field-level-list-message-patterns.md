# ADR-021: Use Field-Level List Message Patterns

**Date**: 2026-07-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Structured field messages can reuse translated note fields and independently translate small fragments, but every invocation currently repeats its complete `format` and full note-field reference paths. Ultimate Geography flag-similarity values repeat two formats dozens of times and encode one- and two-item lists with numbered variables such as `country_1` and `description_2`.

The repeated glue belongs to the field definition, while each note should own only the ordered comparison data. References should remain typed and stable rather than relying on interpolated path strings or YAML anchors.

## Decision

A field definition may declare a reusable `message_pattern` of `kind: list`. It defines one `item_format`, one `separator`, and named parameter types. The initial parameter types are:

- `note_field_ref`, which declares the target field once and accepts a referenced note stable ID at each invocation;
- `text`, which accepts independently translatable scalar text at each invocation.

A `note_field_ref` argument may instead use the explicit `{text: ...}` object when the label is genuinely text rather than a dependency, such as an extension-companion build where the referenced note is absent. The explicit argument kind remains part of semantic equality and fingerprints. Explicit `text` is rejected for a `text` parameter, whose concise scalar already has those semantics.

A note invokes its field's pattern with a non-empty ordered YAML sequence of parameter mappings directly at the field value. The earlier `{items: [...]}` mapping remains reader-compatible for migration, but canonical formatting always emits the direct sequence. Tagged `!image` scalar sequences remain structurally distinct from list-message mapping sequences. Empty scalar fields remain valid and are the only blank/fillable representation. Existing inline structured messages remain supported.

List patterns and invocations are semantic core values. Validation checks pattern placeholders, declared and supplied parameters, typed references, missing values, and dependency cycles. Rendering uses the same field dependency graph as inline structured messages and lowers the result to plain adapter text.

Pattern glue is translated once at field-definition paths. Invocation text and references retain stable item/parameter paths, and a contextual translation may override the separator at one consuming note path. A consuming-path contextual reference decision is materialized even when its target equals the source, so it can intentionally override a conflicting reusable direct translation on the referenced field. Nested contextual YAML remains an ergonomic grouping of the same flattened stable paths.

## Rationale

- Shared glue is authored and translated once.
- Ordered items replace numbered variables and naturally support more than two entries.
- `country: note.moldova` is concise while retaining a typed reference to the declared `field.country`; `country: {text: Sierra Leone}` is an explicit translatable escape hatch when no dependency exists.
- A direct item sequence removes a redundant `items` wrapper while remaining unambiguous from tagged structured-image sequences.
- Reference validation, cycle detection, semantic diff, and fingerprints remain format-independent core behavior.
- Existing adapter output remains a normal scalar string.

## Alternatives Considered

- **YAML anchors**: rejected because they provide textual reuse without typed validation and do not compose cleanly across overlays.
- **Named one-item and two-item templates**: rejected because arity-specific templates preserve numbered-variable duplication and do not scale.
- **Implicit current-note references**: rejected because a comparison normally references a different note.
- **Formats-only desugaring to inline messages**: rejected because canonical formatting would lose the concise source representation and core equality/diff would not retain author intent.

## Implications

- `FieldDefinition` and `FieldValue` gain semantic list-pattern structures.
- Complete field-definition and note fingerprints include the new structures without changing fingerprints for definitions that have no pattern.
- Overlay field values can carry list invocations where the composed field definition supplies the pattern.
- Translators review shared `item_format`/`separator` glue once and per-item text/reference units at their consuming note paths.
- Ultimate Geography source migration happens in its own repository and is then synced into Brain Brew's provenance-locked fixture; the vendored fixture is not edited directly.
