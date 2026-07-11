---
title: Import CrowdAnki
---

# Import CrowdAnki

Import is a review-first three-step workflow. It never turns generated IDs into source just because an import command was run:

```bash
# 1. This reads deck.json and writes only the review artifact.
brainbrew import crowdanki plan build/crowdanki/en-standard --out import-plan.json

# 2. Inspect source locations, GUID/model/template evidence, suggestions, and decisions.
brainbrew import crowdanki review --plan import-plan.json

# 3. Apply the exact reviewed source and plan.
brainbrew import crowdanki apply build/crowdanki/en-standard \
  --plan import-plan.json --approve-plan --out deck.yaml
```

The version-1 `brain-brew.crowdanki-import-plan` is canonical pretty JSON (or deterministic YAML when the plan output ends in `.yaml`/`.yml`). It contains the raw `deck.json` SHA-256 and byte length, import-options fingerprint, source JSON path, source GUID, model UUID/name, template name where relevant, suggested stable ID, status, and decision for every deck, note type, field, template, note, and media identity. `plan` has no Canonical Deck source side effect and writes through the recoverable output transaction.

Automatic suggestions have `status: automatic` and `decision: { kind: automatic }`. Do not edit them merely to accept them: `apply --approve-plan` is the explicit review acknowledgement. A collision has `status: requires_override`; edit only its decision, for example:

```yaml
decision:
  kind: override
  stable_id: note-type.country-imported
```

`apply` validates the complete generated inventory and evidence, legal Stable ID syntax, unique selected IDs globally and in each identity domain, selected-ID conflicts, the source fingerprint, and source locations before it opens a source-output transaction. Missing approval, unresolved/rejected decisions, duplicate or invalid overrides, edited evidence, stale plans, and changed `deck.json` all fail closed before `deck.yaml` is written. The old `--accept-suggested-ids` bypass is removed.

If a process interruption leaves a workspace transaction journal, rerun the same `plan` or `apply` command; Brain Brew performs journal recovery before reading or writing. It reports a recovery conflict rather than overwriting a changed output. For a deliberate destination replacement, add `--force`; only an existing regular file can be replaced.

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

Note models, fields, templates, and media use the same reviewed plan mechanism. Their evidence is included so a maintainer can select a precise override rather than patching untyped YAML or changing CrowdAnki input bytes.
