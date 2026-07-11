---
title: Import CrowdAnki
---

# Import CrowdAnki

Import makes a complete Canonical Deck file from a CrowdAnki `deck.json`:

```bash
brainbrew import crowdanki build/crowdanki/en-standard \
  --accept-suggested-ids --out deck.yaml
```

`--accept-suggested-ids` accepts the deterministic automatic suggestions for every imported entity. There is currently no suggested-ID override file or selective override argument.

## Imported note IDs

CrowdAnki does not store Brain Brew stable IDs. For every imported note, Brain Brew preserves the original `guid` unchanged under `adapter_ids.crowdanki:guid` and independently suggests the canonical note ID from the first field and that GUID.

1. First-field text is normalized to Unicode NFC with the Rust `unicode-normalization` crate. Composed and decomposed equivalents therefore receive the same normalized input.
2. An ASCII-readable slug uses only ASCII letters and digits, lowercased with ASCII-only rules. No locale-sensitive case conversion or Unicode case folding is used.
3. A unique readable slug stays `note.<slug>`, for example `Finland` becomes `note.finland`.
4. A repeated readable slug becomes `note.<slug>-<digest>`. A blank first field, or a first field without ASCII letters/digits (for example Cyrillic, CJK, or RTL text), becomes `note.imported-<digest>`.

`<digest>` starts as the first 12 lowercase hexadecimal characters (48 bits) of SHA-256 over a versioned domain string and length-delimited UTF-8 NFC first field plus the unchanged source GUID. It is not a replacement for the GUID. If two suggestions in one collision group share that prefix, Brain Brew extends every digest in that group by four hexadecimal characters at a time through all 64 characters (256 bits). A full-digest collision receives a final `-2`, `-3`, … suffix ordered by normalized first field then source GUID. Thus collision resolution is independent of CrowdAnki note-array order, platform, locale, hash-map iteration, and import timing.

## CrowdAnki identity validation

A source GUID must be non-empty and unique within the imported deck. GUIDs are opaque UTF-8 adapter IDs: Brain Brew does **not** trim whitespace, normalize Unicode, case-fold, or otherwise rewrite them. Thus `guid`, ` guid `, and Unicode lookalikes are distinct; only byte-for-byte equal non-empty text collides. Duplicate-GUID diagnostics list every affected `$.notes[index].guid` location. An import without a `guid` is rejected by the strict JSON schema; an empty `guid` is rejected by identity validation.

For canonical decks exported to CrowdAnki, a missing `crowdanki:guid` adapter ID has the explicit effective-GUID fallback of that note's canonical stable ID. An explicitly present empty value and any duplicate effective GUID fail closed before export or round-trip projection. The source order's active note indices and canonical note paths are included in that diagnostic.

CrowdAnki standard-model template ordinal identity is zero-based array position: for each note model, `tmpls[index].ord` must equal `index`. This simultaneously requires valid non-negative representable ordinals, uniqueness, contiguity, and input array ordering. Brain Brew rejects duplicate, gapped, negative, overflowed, or reordered ordinals before conversion; it never sorts, renumbers, or repairs templates. Valid template order and ordinals are preserved through import and export.

Adding a note that collides with a previously unique readable slug changes that group from `note.<slug>` to digest-suffixed IDs on a fresh import. This is intentional and deterministic; keep the generated canonical source as the reviewed identity record. Non-Latin and blank values never use the shared `note.unnamed` ID.

Note models, fields, templates, and media use their own suggestions. Their collisions still fail closed; this import command has no override input for them.
