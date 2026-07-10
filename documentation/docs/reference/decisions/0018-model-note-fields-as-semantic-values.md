# ADR-018: Model Note Fields as Semantic Values

**Date**: 2026-07-10  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

ADR-016 introduced structured image references by following the existing structured-message precedent: raw scalar text, messages, and images were stored in synchronized maps, with empty scalar placeholders for structured values. Composition and expected-base logic subsequently treated those placeholders as blank field content. An add, merge, or field fill could therefore erase a structured value without declaring a destructive intent, while semantic diff and translation had to reconstruct one value from several independently mutable maps.

## Decision

Core represents every note field with one `FieldValue` in one field map. `FieldValue` distinguishes:

- scalar text, including the intentional empty scalar;
- a non-empty ordered sequence of stable media-ID image references;
- a structured message.

The variant is part of canonical equality, ordering, hashing, and debug output. A scalar containing rendered HTML is not equal to the structured value that lowers to the same adapter bytes.

Only an empty scalar is blank and fillable. Images and messages are always non-blank. Empty image sequences and malformed message shapes fail construction or canonical validation. Field changes carry one optional semantic value: absence is reserved for removal, so dual raw/message/image payloads are not representable in core.

Field expected bases compare complete semantic values. Existing scalar expected-base syntax remains compatible; canonical YAML also accepts structured values beneath `expected_base.value`.

Canonical validation resolves field definitions and validates representations together. It rejects unknown structured-image media IDs at the note/field path before rendering. Translation extracts scalar prose and message text/format components, does not extract image IDs, and preserves structured messages until adapter lowering. Rendering resolves messages and images to deterministic scalar adapter text. Semantic diff compares complete `FieldValue` values.

## Rationale

One semantic value removes synchronization and placeholder states from the domain. Composition can apply blank, fill, replace, override, and expected-base rules atomically, and adapters remain the only layer that lowers structured values to HTML or CrowdAnki strings.

## Compatibility

Canonical YAML syntax and CrowdAnki bytes are unchanged for existing scalar, message, and `!image` values. The public Rust model changes incompatibly: `Note.field_messages`, `Note.field_images`, and the three independent `FieldChange` payload members are removed. Callers use `Note.fields: FieldMap` (keyed by `StableId`) and `FieldChange.value: Option<FieldValue>` instead.

This ADR supersedes only ADR-016's parallel-map core representation. ADR-016's YAML syntax, stable media-ID references, import mapping, rendering, verification, and include decisions remain active.

## Implications

- Core APIs cannot express split or dual field representations.
- Source-document edits mutate scalar variants only and preserve image/message variants.
- Media include-preserving formatting must retain enough synthetic declaration context to run canonical unknown-media validation before restoring the include directive.
- Complete entity-level expected-base fingerprints and typed tombstones remain separate follow-up work.
