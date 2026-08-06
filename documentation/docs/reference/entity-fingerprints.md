---
title: Canonical entity fingerprints
---

# Canonical entity fingerprints

Brain Brew uses a stable fingerprint as the expected base for every complete entity replacement, override, or removal. Sparse property changes continue to carry their exact prior typed value.

## Text form

Supported fingerprints use:

```text
sha256:v<version>:<64 lowercase hexadecimal digits>
```

`sha256` is the digest algorithm; `v1` and `v2` are supported Brain Brew entity-encoding schema versions. Unknown algorithms or versions, uppercase/non-hex digests, and the wrong digest length are rejected before composition.

The entity kind is domain-separated inside the hashed bytes rather than repeated in the text. A fingerprint generated for a note can therefore never match a media reference with otherwise similar text.

## Canonical encoding

The implementation is hand-defined in `brain-brew-core`; it never hashes Debug, YAML, Serde, filesystem, or platform-dependent output.

- The first value is the UTF-8 domain string `brainbrew:<version>:<entity-kind>`, where the kind is `note-type`, `field-definition`, `card-template`, `note`, or `media-reference`.
- Every scalar has a one-byte field/variant tag, an unsigned 64-bit big-endian byte length, and the exact UTF-8 bytes.
- Every sequence has a one-byte field tag and an unsigned 64-bit big-endian element count. Entity-defined sequence order is retained.
- Every option has a one-byte field tag and a one-byte `0`/`1` presence marker before its value.
- Maps and sets are encoded in canonical sorted key/value order. Adapter IDs and variables include both key and value.
- Nested structures use explicit field and variant tags. No concatenation is ambiguous.
- SHA-256 is computed over those bytes.

The complete semantic inputs are:

| Entity kind | Included properties |
| --- | --- |
| note type | stable ID, name, sorted variables, field-definition sequence, card-template sequence, styling, sorted adapter IDs |
| field definition | stable ID, name, list-message pattern, RTL direction |
| card template | stable ID, name, sorted variables, question format, answer format, sorted adapter IDs |
| note | stable ID, note-type ID, sorted variables, sorted field map, sorted tags, sorted adapter IDs |
| media reference | stable ID, path, SHA-256 declaration |

A note field includes its semantic `FieldValue` variant. Scalar, ordered image references, and structured message are distinct. Structured messages include positional component order, optional format, sorted named variables, and each literal/text/field-reference variant.

Version 2 adds field RTL direction. Field-definition and note-type fingerprints therefore use v2; unchanged card-template, note, and media-reference encodings continue to emit v1 so their established vectors remain stable. A v1 field-definition or note-type fingerprint cannot authorize a current complete change and must be regenerated.

The public pure-core functions are `fingerprint_note_type`, `fingerprint_field_definition`, `fingerprint_card_template`, `fingerprint_note`, and `fingerprint_media_reference`. Golden vectors and per-property mutation tests guard the schemas.

## Overlay schema

Complete changes use:

```yaml
notes:
  note.finland:
    intent: remove
    expected_base:
      fingerprint: sha256:v1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
```

Complete `note`, `note_type`, card-template, field-definition, and media bodies use the same form for `replace` and `override`. The fingerprint is always compared with the actual current entity immediately before the change. This includes changes made by an earlier overlay in the ordered stack. `override` can resolve provenance conflicts only after that current-state check succeeds.

Sparse values still use exact values:

```yaml
fields:
  field.capital:
    intent: replace
    value: Helsingfors
    expected_base:
      value: Helsinki
```

## Migration and generation

The old marker is intentionally rejected:

```yaml
expected_base: entity_present # invalid
```

The diagnostic directs maintainers to exact values or generated fingerprints. Do not calculate hashes by hand. Generate a reviewed overlay from the exact old and desired decks:

```bash
brainbrew diff old-deck.yaml desired-deck.yaml \
  --as-overlay --id overlay.patch.reviewed > overlay.yaml
```

`diff --as-overlay` emits exact typed values for sparse changes and fingerprints for complete note, note-type, and media changes/removals. Reapplying that overlay to a wrong, missing, or newer base fails before mutation.

This is a breaking overlay-schema change. Existing presence-only overlays must be regenerated from their intended base. Changing the algorithm or canonical domain schema requires a new text version and migration; existing version bytes must not be reinterpreted.

## Diagnostics

Composition precondition errors expose a stable code/category plus DeckPath, entity kind, intent, overlay ID, expected state, and actual state. `brainbrew explain --json` emits these fields directly, so tools do not need to parse the English message.
