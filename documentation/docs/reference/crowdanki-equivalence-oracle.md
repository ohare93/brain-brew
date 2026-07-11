---
title: CrowdAnki equivalence oracle
---

# CrowdAnki equivalence oracle

`canonical_crowdanki_equivalence` is the fail-closed adapter-equivalence comparison between a
`CanonicalDeck` and a CrowdAnki `deck.json`. It parses and imports the JSON with
the normal strict schema, projects both sides through
`crowdanki-export-import-v1`, then uses the complete core semantic diff. It is
not a JSON subset comparison or a workflow for merging into existing source.

An error is one of:

- `Unsupported`: an unknown, nested, non-default, unsafe, duplicate, or otherwise
  unmodelled CrowdAnki property was encountered;
- `Canonical`: source cannot be exported under the profile;
- `MediaBytesRequired`: canonical hashes exist but the caller supplied only
  reference-only JSON; or
- `Differences`: a typed report with canonical semantic path, exact actual JSON
  path where available, category, expected value, and actual value.

## Inventory and mapping

| Canonical property | CrowdAnki observation | Oracle treatment |
| --- | --- | --- |
| deck name, description | `name`, `desc` | compared |
| deck UUID/config UUID/config name | `crowdanki_uuid`, `deck_config_uuid`, default config name | compared; all other config values must be the one documented default |
| deck stable ID | absent | regenerated from visible deck name |
| note model UUID, name, CSS | `note_models[].crowdanki_uuid`, `name`, `css` | compared |
| fields, names, order | `flds[].name`, `ord`, array order | names/order compared; non-default display options reject |
| templates, names, question/answer HTML, order/ord | `tmpls[]` | compared; ord must exactly equal its zero-based array position |
| LaTeX pre/post/SVG, requirements, sort field, model tags/version | model options | only documented defaults are supported; any other value rejects |
| canonical variables and typed structured values | rendered/lowered adapter strings | deliberately projected: variables render and images/messages lower before exact comparison |
| note GUID and model UUID | `notes[].guid`, `note_model_uuid` | compared as opaque exact UTF-8 strings; empty/duplicate GUID rejects |
| note fields and field order | `notes[].fields[]` and model field order | compared |
| note tags | `notes[].tags` | compared as a set; CrowdAnki tag-array order is not semantic |
| note stable ID | absent | regenerated from NFC first-field plus exact GUID suggestion algorithm |
| media paths/declarations | `media_files` | compared as a set of declared paths; duplicate/case-colliding/unsafe paths reject |
| media bytes, length, SHA-256 | absent from `deck.json` | only compared when byte handoff is supplied; reference-only success says `NotProven` |
| tombstones | absent | physical omissions only; typed tombstones are projected away |
| non-CrowdAnki adapter IDs | absent | discarded only by the named profile |
| child decks, dynamic scheduling, cloze models, note data/flags, field/browser options, unknown keys | JSON state with no canonical model | reject, never ignore or normalize |

Map and set ordering is nonsemantic only where the canonical type is a map/set:
declarations and tags. Field arrays, template arrays, ordinal values, message
components, image sequences, empty versus absent JSON fields, Unicode text,
GUIDs, IDs, configuration values, and hashes are never normalized.

## Laws and mutation matrix

The format test matrix starts from a valid exported fixture and makes one change
per case. It covers deck metadata/configuration, model UUID/name/CSS,
field/template values and order/ord, LaTeX/requirements/sort/default options,
GUID/model identity, fields/tags, media declarations, unknown nested state, and
unsafe/duplicate state. Each supported mutation must yield `Differences`; each
unmodelled mutation must yield `Unsupported` before projection.

The import/export law is exercised for standard state, NFC/NFD note suggestion
identity, CJK and RTL text, exact opaque GUIDs, strict structured image lowering,
and structured-message rendering. `export -> import` compares the two projected
canonical values. `import -> export` preserves every supported JSON property and
rejects any source property that cannot be represented.

## Media proof boundary

A `deck.json` declares filenames, not asset bytes. Therefore a caller that has
canonical SHA-256 values must provide `CrowdAnkiImportMediaBytes`; the oracle
binds them through the reviewed import plan and compares each resulting hash. A
reference-only call can establish declaration/reference equivalence only and
returns `CrowdAnkiMediaByteProof::NotProven`. It must not be used as a release
media-integrity claim.
