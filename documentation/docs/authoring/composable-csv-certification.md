---
title: Composable CSV certification fixture
---

# Composable CSV certification fixture

`fixtures/composable-csv-authoring/` is Brain Brew's small, repository-owned
Ultimate Geography-shaped contract fixture. It is synthetic and has no runtime,
source, or update dependency on the Ultimate Geography repository.

The fixture has two manifests over the same authoritative tables:

- `brainbrew-all-csv.yaml` keeps notes, translations, and Experimental region
  codes in CSV;
- `brainbrew-migrated.yaml` moves `note.france`, the reusable `Europe`
  translation, two contextual `Shared` occurrences, and France's region code to
  native YAML with explicit exclusions.

Both expose English, German, and Spanish Standard and Experimental targets. The
maintained test proves that every storage-only migration has an empty semantic
diff and that the German Experimental CrowdAnki directory is byte-identical.
It separately uses Workbench to prove a remaining CSV unit is read-only while a
transferred unit can be edited and gains native stale-translation tracking.

## Run the certification

From the repository root:

```bash
devenv shell -- cargo test -p brainbrew \
  --test composable_csv_certification

devenv shell -- cargo test -p brainbrew \
  --features workbench-write-dev --test cli \
  workbench_certified_composable_csv_fixture_enforces_and_transfers_capabilities
```

The first command is the executable workflow. It runs the real `brainbrew`
binary to format source declarations, validate and compose both states, report
translation coverage and CSV provenance, compare semantic output, verify strict
media hashes, export CrowdAnki, compare export bytes, inspect `explain` source
fingerprints, and prove package-lock invalidation after a CSV change. The second
command is the server-authoritative Workbench write-boundary
check. Both run in normal CI partitions through `devenv shell ci`.

Useful manual inspection commands are:

```bash
brainbrew verify \
  --manifest fixtures/composable-csv-authoring/brainbrew-all-csv.yaml \
  --all-targets --media-root media

brainbrew translations \
  --manifest fixtures/composable-csv-authoring/brainbrew-all-csv.yaml \
  --target de-experimental --overlay overlay.translation.de --json

brainbrew explain \
  --manifest fixtures/composable-csv-authoring/brainbrew-all-csv.yaml \
  --target de-experimental --json
```

`--media-root media` is relative to the manifest workspace, not the shell's
current directory. The integration test owns temporary compose/export paths so
running it never creates fixture build artifacts.

## What the fixture certifies

### Notes, descriptors, joins, and literal parameters

The unchanged direct note map remains the smallest syntax for native notes:

```yaml
notes:
  note.france:
    note_type_id: note-type.country
    fields: {field.country: France}
    tags: []
    adapter_ids: {}
```

A single table-backed set names its descriptor and literal parameters:

```yaml
notes: !csv
  descriptor: sources/countries.yaml
  parameters:
    language: ''
```

A gradual transfer uses an explicitly tagged sequence; order never grants an
override:

```yaml
notes:
  - !csv
    descriptor: sources/countries.yaml
    parameters: {language: ''}
    exclude:
      note_ids: [note.france]
  - !inline
    note.france:
      note_type_id: note-type.country
      fields: {field.country: France}
      tags: []
      adapter_ids: {}
```

Every inline transferred note must still define its complete note-type field
set; the abbreviated documentation body above only illustrates source tagging.
See [Workspace layout](workspace.md) and
[ADR-022](../reference/decisions/0022-use-composable-read-only-authoring-sources.md).

`sources/countries.yaml` is strict: it names one primary table, required country
and GUID joins, an optional hint join, explicit stable note IDs, exact headers,
tags, and a fixed note type. Ordinary `notes: !csv` uses it to materialize the
complete Standard note shape. Standard and Experimental translation overlays
reuse the same descriptor: `translations.from_csv` validates its mapped fields
as a subset of the resolved note type, so the Experimental region field remains
owned by its structural overlay. Unknown or misspelled translation field
mappings still fail. The `language` parameter is a `localized_column`;
declarations pass literal `de` or `es`, selecting exact `:de` or `:es` headers.
There is no manifest-language inference, header normalization, fallback lookup,
transform expression, or ID generation.

The all-CSV base demonstrates typed `image` mappings for flag and map cells.
Cells contain stable media IDs, while `media.yaml` remains the only path/hash
authority. The strict workflow reads six repository-owned SVGs and rejects a
byte whose SHA-256 no longer matches. See [Authoring media fields](media.md).

### Translation inference and transfer

The German localized column exercises every supported pair category:

- reusable `Europe` becomes a direct translation at all three occurrences;
- repeated `Shared` has conflicting targets (`Gemeinsam` and `Geteilt`) and is
  contextual at every occurrence;
- unchanged `Paris`, `Berlin`, and `Madrid` are reviewed `no_change` values;
- France's source-only hint is a path-scoped deletion;
- Germany's target-only hint is a path-scoped adaptation;
- localized CrowdAnki GUIDs use adapter-ID translation.

Spanish is the independently composed second target language. Its values are
non-conflicting: country, capital, `Europe`, `Shared`, and France's hint infer
reusable direct translations; Madrid is `no_change`; Germany's blank-source
hint becomes the target-only `Añadido` adaptation; and GUIDs are localized
through the adapter-ID map.

Both-blank text pairs are ignored. Exactly one blank adapter-ID cell remains a
fatal error because the adapter map cannot represent an adaptation or deletion.
CSV inference is global-occurrence-aware against the complete translation-free
source shape, including the Experimental field addition even though the shared
translation descriptor does not own that field.

Migration selectors are exact unions: non-empty `source_texts`, stable
`note_ids`, and canonical `paths`. They are not globs, regular expressions, or
predicates. Every selector must match, and every excluded occurrence must have
an equivalent native dictionary/adaptation entry in the same state. Partial
transfer contextualizes remaining CSV occurrences so an imported global
`direct` or `no_change` decision cannot cross the ownership boundary.

### Sparse Experimental values

`experimental-*.yaml` uses
`field_additions.note-type.country.values.from_csv` only for the field that the
extension adds. Non-empty region-code cells materialize; empty or absent
optional values claim no ownership. The migrated state excludes France and
provides the exact typed inline value. Unknown notes, unmatched exclusions,
duplicate ownership, conflicting inline values, or fields not owned by the
addition fail transactionally. The empty
`translation-experimental-de.yaml` overlay deliberately follows the shared
extension in the German target. It models the extension-specific translation
seam and lets Workbench inspect sparse units without putting extension content
in the base-language dictionary.

### Provenance, freshness, locks, and Workbench

`brainbrew explain --json` lists each descriptor and table with a SHA-256 input
fingerprint. The translation JSON report lists each remaining CSV-owned unit's
declaration, descriptor, file, logical row, header, column, canonical path,
category, source, and target. The maintained test changes a CSV in a temporary
copy and proves both the explain hash and a path package lock become stale.
Descriptors and tables also participate in verification and Workbench workspace
freshness/compare-and-swap fingerprints.

Workbench read paths consume the resolved notes and translations normally, but
capability is per ownership unit. The server rejects preview and apply for a
CSV-owned source/translation/sparse cell with a typed actionable error. An
excluded inline unit in the same target stays writable. UI disabling is only a
convenience; server validation is the security boundary, and a mixed apply that
crosses into CSV ownership fails as one transaction.

## Troubleshooting and limits

- **Missing/duplicate header, row-width, join, ID, or type errors:** use the
  descriptor/file/row/header/column diagnostic literally; Brain Brew does not
  trim or guess CSV structure.
- **Missing required join:** add the keyed row or declare that join optional
  only when absence is part of the data contract.
- **Unmatched exclusion or ownership collision:** exclude an otherwise
  materializable unit and add a complete semantically equal new owner in the
  same state.
- **Source mismatch in `translations.from_csv`:** the unsuffixed CSV cell must
  equal the complete resolved source occurrence before translation.
- **Stale workspace/lock:** rerun explain/verify and deliberately refresh the
  reviewed lock after checking changed descriptors and tables.
- **Live CSV translation changed but is not reported stale:** this is expected.
  CSV pairs regenerate from current bytes and retain no historical source key.
  Transfer ownership to native YAML before relying on stale-source review.

CSV write-back, byte-preserving CSV rewrites, automatic language-column
creation, arbitrary transforms/derivatives, and legacy Python recipe
compatibility are explicit non-goals.

## Ultimate Geography gate

Passing this maintained repository fixture is the prerequisite for resuming the
`ultimate-geography-production` epic. It does not itself adopt or modify live
Ultimate Geography sources. After this certification commit, production work
may resume only through the separately pinned live-consumer/update phase in the
approved UG fixture contract; that mandatory live gate is not replaced by this
fixture.
