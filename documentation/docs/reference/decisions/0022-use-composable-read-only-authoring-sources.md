# ADR-022: Use Composable Read-Only Authoring Sources

**Date**: 2026-08-05  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew currently treats strict Canonical YAML as the maintainer source and materializes it directly into `CanonicalDeck`, `Overlay`, and `TranslationDictionary`. Ultimate Geography and similar decks already maintain large, joined, localized tables. Expanding those tables into generated YAML before maintainers can gradually take native YAML ownership would create a large duplicate source tree and make an all-at-once migration necessary.

CSV must therefore be usable as source-controlled authoring input without becoming a second domain model, an ordered override mechanism, a CrowdAnki adapter, or a revival of the legacy recipe DSL. Maintainers also need to move ownership between old CSV layouts, new CSV layouts, and inline YAML without changing resolved deck output.

## Decision

### Root YAML and resolved domain model

The strict YAML Canonical Deck File remains the root declaration. It embeds or names Authoring Sources that materialize disjoint canonical paths. Inline YAML and explicitly declared CSV are alternative source representations; after materialization they produce the existing `CanonicalDeck`, `Overlay`, and `TranslationDictionary` domain types.

The core crate remains filesystem-, YAML-, and CSV-independent. CSV declarations, descriptors, parsing, materialization, and source provenance belong in `brain-brew-formats`. Callers inject authorized bytes. CLI and package loaders own filesystem access and path authorization.

CSV is a first-class, read-only Authoring Source. It is not a one-time importer, an adapter format in this role, or writable canonical storage.

### Composable note sources

The existing direct note map remains valid and unchanged:

```yaml
notes:
  note.france:
    note_type_id: note-type.country
    fields: { ... }
    tags: []
    adapter_ids: {}
```

A single CSV-backed note source may be declared directly:

```yaml
notes: !csv
  descriptor: sources/countries.csv.yaml
  parameters:
    language: ''
```

A deck may instead declare a tagged sequence of note sources:

```yaml
notes:
  - !csv
    descriptor: sources/legacy.yaml
    parameters:
      language: ''
    exclude:
      note_ids:
        - note.france
  - !csv
    descriptor: sources/v2.yaml
    parameters:
      language: ''
  - !inline
    note.france:
      note_type_id: note-type.country
      fields: { ... }
      tags: []
      adapter_ids: {}
```

Every list item is explicitly tagged. Initially the supported item tags are `!csv` and `!inline`; future source representations require new explicit tags. Source file extensions and `!include` never infer a note source type.

Source order is preserved by formatting but has no ownership or override meaning. Materialized note maps are unioned only when their stable note IDs are disjoint. A duplicate stable ID across any pair of sources is fatal, even when the resulting notes would be equal.

A CSV source may exclude explicit stable note IDs. Every exclusion must name a note that source would otherwise materialize. Unknown or duplicate exclusions are fatal. Moving ownership requires excluding the ID from its old CSV source and defining it in another CSV or inline source in the same resolved source graph. Removing an entire old source after its last transfer is complete needs no retained exclusion.

Pure inline decks do not need a wrapper, and pure CSV or mixed decks do not need an empty placeholder note map.

### CSV source descriptors

A `!csv` declaration references one reusable strict YAML descriptor. The descriptor owns reusable table, join, parameter, and canonical mapping rules. The declaration owns literal parameter values and ownership exclusions.

A note descriptor has this closed conceptual shape:

```yaml
version: 1
primary_table: main
tables:
  main:
    path: data/main.csv
  country:
    path: data/country.csv
  guid:
    path: data/guid.csv
parameters:
  language:
    type: localized_column
    default: ''
    separator: ':'
joins:
  - left: main.country
    right: country.country
    required: true
  - left: main.country
    right: guid.country
    required: true
note:
  id: main.stable_id
  note_type_id: note-type.country
  fields:
    field.country:
      column: country.country
      localized_by: language
      type: scalar
    field.flag:
      column: main.flag
      type: scalar
  tags:
    column: main.tags
    delimiter: ', '
  adapter_ids:
    crowdanki:
      column: guid.guid
      localized_by: language
```

The serialized descriptor shape above and the following contract are fixed:

- `version`, one explicit primary-table alias, table aliases and paths, ordered joins, parameter declarations, and one fixed note-type mapping are required and strict; unknown keys fail.
- Every canonical mapping names a table alias and exact UTF-8 header. Aliases are unambiguous and headers are not lowercased, trimmed, Unicode-normalized, or otherwise guessed.
- Empty or duplicate headers, invalid UTF-8, malformed records, and records whose width differs from the header fail with descriptor, CSV file, logical row, and column/header diagnostics. A UTF-8 BOM is not silently stripped; diagnostics identify it explicitly rather than reporting only that the declared first header is absent.
- Extra unmapped columns are allowed. This permits one table to carry several language columns while a declaration selects one language.
- Every primary row materializes one note unless explicitly excluded. Stable note IDs come from an explicit mapped cell; they are never derived from row number, display text, a slug heuristic, or an adapter ID.
- An ordinary `notes: !csv` descriptor maps exactly one fixed note type and materializes complete notes. Every field declared by that note type is mapped exactly once; missing and unknown field mappings fail through canonical validation.
- Table and source iteration is deterministic. Resolved maps use canonical stable-ID ordering rather than filesystem or hash-map order. CSV file row order is not rewritten.

All descriptor and CSV paths use the existing portable safe-relative path and package-root authorization rules. Absolute paths, traversal, backslashes, drive/UNC forms, repeated separators, and symlink escapes fail before reads. Descriptors cannot recursively include or discover other descriptors.

### Joins

Joins are explicit flat many-to-one lookups. Each join names both qualified key columns. The left side belongs to the primary table and the right side belongs to one joined table; joined tables cannot feed chained, recursive, inferred, many-to-many, or formula-based joins.

Primary note-ID cells are unique and non-empty. Right-side lookup keys are also unique and non-empty. Left-side foreign keys are non-empty but may repeat, which gives the join its many-to-one cardinality. Duplicate primary note IDs or right-side lookup keys are fatal.

Joins are required by default. An unmatched row fails closed unless that join is explicitly optional:

- a required join must match every primary row exactly once;
- an optional join may be absent for a primary row and then contributes empty cells for that alias;
- every present match remains unique;
- unreferenced rows in a joined table are allowed so a shared lookup may be a superset.

Optional joins are necessary for intentionally sparse derivative tables such as country information or capital hints. Their missing cells use the same type-specific empty rules as explicit empty CSV cells.

### Localized columns and values

`localized_column` is the only source parameter type introduced by this decision. It is separate from deck/template `${...}` Source Variables and from manifest language inference.

For a mapping that opts into parameter `language`, the selected header is:

```text
base header                          when language == ""
base header + separator + language   otherwise
```

The parameter default is the empty string. Non-empty values and separators are used literally; there is no case conversion, locale normalization, expression evaluation, or fallback header search. Field and adapter-ID mappings may opt in. Shared mappings such as tags and media normally remain unsuffixed.

Declaration arguments are always explicit literal values or descriptor defaults. They never bind implicitly to ambient manifest/target context. Consequently direct-file commands materialize the same source as manifest commands and do not need a hidden target-language environment.

Cells are decoded literally with no trimming and no configurable null tokens:

- a scalar field receives the exact cell text; empty means the intentional `FieldValue::Scalar("")`;
- a note ID must be a non-empty valid Stable ID;
- a non-empty adapter-ID cell adds the declared adapter namespace, while an empty cell omits it;
- a tags cell is split by the descriptor's exact delimiter; empty means no tags, while empty or duplicate segments fail;
- an optional missing join cell behaves like an empty cell of the mapped type.

Initial CSV note materialization may preserve legacy image HTML as an ordinary scalar field. A later explicit `image` field mapping accepts stable media IDs and materializes the existing structured image value. Without `delimiter`, a non-empty cell contains exactly one media ID. With a non-empty exact `delimiter`, the cell is split into a non-empty ordered image sequence; every segment must be a non-empty valid stable media ID. No trimming, escaping, or delimiter fallback is applied. The delimiter is source syntax only: multi-image fields retain the existing adjacent-tag rendering with no output separator. An empty whole cell remains an ordinary empty scalar.

That migration includes a one-time source normalization from HTML cells to media IDs. It does not add an HTML parser, infer media paths, alter `media.yaml` path/hash authority, or add mixed text/image CSV values. Scalar HTML and structured image values remain semantically distinct even when they lower to equal adapter bytes.

### CSV-backed translation dictionaries

A translation overlay may preserve one or more CSV declarations alongside normal inline dictionary entries:

```yaml
translations:
  from_csv:
    - descriptor: sources/countries.csv.yaml
      parameters:
        language: de
      exclude:
        source_texts: []
        note_ids: []
        paths: []
  direct: {}
  contextual: {}
  no_change: []
  variables: {}
  adapter_ids: {}
```

`from_csv` pairs each opted-in unsuffixed source column with the localized column selected by the declaration's non-empty `language` parameter. An omitted or empty `language` is a fatal declaration error rather than a source-to-itself pairing. It uses the descriptor's stable note IDs and canonical field paths. It materializes the existing `TranslationDictionary` and path-scoped Target Adaptation semantics; it never applies localized note replacements.

Unlike complete CSV note materialization, a CSV translation descriptor owns a validated subset of the resolved note type's fields. Every mapped field must exist, so unknown or misspelled mappings remain fatal, but fields omitted from the descriptor may be owned by another overlay. An omitted structural field receives no CSV translation ownership and remains owned by its structural overlay. No declaration option selects this behavior; the `translations.from_csv` context determines it.

Inference is global-occurrence-aware across the complete resolved source deck:

- both source and target empty: ignore the pair;
- source empty and target non-empty: create a path-scoped `Adapt` Target Adaptation with empty `expected_source`;
- source non-empty and target empty: create a path-scoped `Delete` Target Adaptation with that exact `expected_source`;
- every occurrence of one non-empty source has the same one non-equal target: create one reusable direct translation;
- every occurrence is source-equal: create global `no_change`;
- one source has multiple targets: create contextual decisions for every affected occurrence and never select a majority value;
- a source-equal occurrence that coexists with a differing target elsewhere becomes a contextual source-equal decision, not global `no_change`;
- if any occurrence lies outside the CSV declaration or outside CSV ownership, a generated direct/no-change entry is allowed only when its global effect is still explicitly covered and semantically valid; otherwise CSV-owned occurrences become contextual.

Adapter-ID mappings use the same parameterized source/target pairing and materialize the existing per-adapter translation maps. Their blank policy is narrower because the current domain map has no adapter-ID adaptation or deletion: both cells empty are ignored; equal non-empty IDs need no mapping; differing non-empty IDs create a mapping; and exactly one empty cell is a fatal row/column error. Pair validation happens before ownership exclusions, so an exclusion cannot bypass an unrepresentable one-sided adapter-ID pair. Imported text adaptations and deletions carry stable CSV file/row/column provenance, translation ownership, and a fixed legacy-import review reason.

Imported and inline translation decisions are merged transactionally through existing core validation. Disjoint decisions and semantically identical duplicates are accepted; incompatible direct, contextual, no-change, adaptation, deletion, or adapter-ID decisions fail without partial materialization. An identical inline duplicate does not make a CSV-owned unit writable: it remains CSV-owned until explicitly excluded.

### Translation ownership transfer

Each `translations.from_csv` declaration supports exactly three literal exclusion selectors:

- `source_texts`: exact non-empty source strings across all their importable occurrences;
- `note_ids`: stable note IDs covering all importable translation occurrences on those notes;
- `paths`: exact canonical occurrence paths.

Globs, predicates, regular expressions, and arbitrary filters are not supported. Empty selector lists are valid and mean no exclusions of that kind; every provided selector entry must match at least one otherwise-importable occurrence. Native inline dictionary/adaptation entries must explicitly cover every excluded occurrence; incomplete transfers and semantic conflicts fail.

An imported global direct or no-change decision must never leak back into an excluded occurrence. When one occurrence is excluded, remaining CSV-owned occurrences for that source are materialized contextually as needed to preserve the ownership boundary. When every occurrence remains CSV-owned, ordinary direct/no-change deduplication remains available.

A storage-only ownership migration is expected to preserve composed deck semantics, translation coverage classification, and adapter output. Once an entry is solely inline-owned it gains normal native editing and stale-source detection and may be changed in a separate behavior change.

Live CSV-owned translation pairs do not retain historical stale detection because both the source key and target decision are regenerated from current CSV bytes. Fingerprints detect that inputs changed, but the regenerated pair appears current. This limitation is reported until ownership moves to native YAML.

### Sparse overlay values

The same descriptor, join, parameter, source-list, exclusion, collision, and provenance rules may be reused only at the existing `field_additions.<note-type>.values` boundary. Direct inline value maps remain valid. CSV-backed values map explicit stable note IDs to explicitly declared added fields and then enter normal overlay composition and expected-base validation.

This sparse boundary has a different completeness rule from base notes: only non-empty mapped cells materialize. An empty cell or missing optional-join cell contributes no field-value entry and claims no CSV ownership; ordinary field-addition composition supplies the blank field. A row whose selected sparse fields are all empty is ignored.

This does not introduce generic CSV-backed overlay patches, recursive derivatives, arbitrary transforms, note-model selection, or a general YAML merge language.

### Provenance, capabilities, and freshness

Materialization returns resolved domain values plus source ownership/provenance metadata outside the core model. Provenance identifies the root declaration, descriptor, CSV table/file, logical data row, exact header/column, and canonical path where available.

CSV-owned paths are read-only capabilities:

- normal browse, compose, compare, coverage, semantic diff, verify, and export paths consume materialized values without special output semantics;
- edits to CSV-owned note fields, tags, adapter IDs, translation decisions, or sparse values are rejected server-side with a typed actionable error;
- inline-owned units in the same workspace remain writable under existing transaction rules;
- a preview/apply transaction that crosses any CSV-owned path fails as a whole;
- UI disabling is advisory; server-side capability enforcement is authoritative.

Descriptors and every referenced CSV file are registered source inputs for their owning Authoring Source declaration. They appear in deterministic explain/plan output and participate in package locks, target fingerprints, verification inputs, Workbench freshness signatures, and compare-and-swap checks. Changing any authoritative descriptor or CSV invalidates the affected plan. No command may use a second materialization path that omits these dependencies.

Formatting preserves `!csv`, tagged source sequences, `translations.from_csv`, and sparse CSV declarations instead of expanding rows. Descriptor YAML is canonically formattable. CSV bytes are read but never rewritten.

### Delivery and Ultimate Geography gate

Every implementation slice uses Red-Green-Refactor:

1. add the smallest failing unit or maintained fixture assertion for the next behavior;
2. implement the smallest passing change;
3. refactor only with the focused and existing suites green.

The repository-owned synthetic UG-shaped fixture grows with each slice: scalar single-source notes; joins and literal language parameters; later normalized typed images; multiple disjoint note sources and ownership transfer; authorization/provenance/fingerprints; translation inference and exceptional blank pairs; translation ownership transfer and stale-behavior contrast; Workbench capabilities; sparse overlay values; and finally all-CSV, mixed, and progressively native YAML states.

Storage-only migration states must prove equal `CanonicalDeck` semantic diff where their semantic representation is unchanged and equal representative CrowdAnki output. The deliberate scalar-HTML-to-structured-image normalization instead proves equal lowered adapter bytes while retaining ADR-018's semantic distinction.

The final certification exercises formatting, validation, composition, translation coverage, verification, semantic diff, explain/provenance, locks/fingerprints, media integrity, Workbench read/write boundaries, and CrowdAnki export through maintained commands.

No Ultimate Geography repository changes belong in this epic. Production UG work remains gated until the certification task passes. After certification, the separately pinned live-consumer/update phase remains mandatory under the existing UG fixture contract.

## Rationale

**Pros:**

- preserves the existing pure resolved domain model and translation engine;
- supports gradual ownership movement between several CSV layouts and native YAML;
- makes collisions and transfer boundaries explicit instead of order-dependent;
- keeps existing inline YAML source compatible;
- reuses source-controlled tabular data without generating duplicate YAML;
- provides deterministic security, provenance, freshness, and read-only editing behavior;
- stays narrowly below a recipe/query language.

**Cons:**

- source loading now has a materialization phase and a provenance sidecar;
- strict descriptors require explicit aliases, keys, mappings, and stable IDs;
- CSV-owned units cannot be edited in Workbench;
- live CSV-owned translations cannot detect historical source drift;
- production sources without explicit stable IDs must add or join an explicit ID table;
- sparse lookup and exceptional translation policies add rules that must remain fixture-certified.

## Alternatives Considered

- **One-time CSV import to generated YAML**: rejected because it duplicates authoritative data and prevents gradual source ownership transfer.
- **Keep YAML as the only Authoring Source**: rejected because it forces an all-at-once UG migration and contradicts the required source-preserving workflow.
- **One CSV source plus one inline map only**: rejected because maintainers may need old and new CSV layouts to coexist during migration.
- **Ordered source overrides or last-writer-wins**: rejected because source order would silently change ownership and output.
- **Generic `!include` or file-extension inference**: rejected because every source representation must be explicit and strictly validated.
- **Ambient manifest language parameters**: rejected because one source file would materialize differently depending on hidden caller context.
- **Automatic header normalization, inferred joins, stable-ID slugging, or transform expressions**: rejected because guesses hide drift and recreate the legacy recipe DSL.
- **CSV-local direct translation inference**: rejected because a global dictionary entry could affect unrelated inline or differently sourced occurrences.
- **Always-contextual CSV translations**: rejected because globally valid direct/no-change decisions should retain the existing compact dictionary semantics.
- **CSV write-back**: rejected because safe row-preserving edits, new language columns, formatting preservation, and multi-file transactions are a separate product problem.
- **Legacy Python recipe compatibility**: rejected; only the narrow certified tabular seams in this ADR are supported.

## Amendments and implications

This ADR amends ADR-005. Strict canonical YAML remains the deterministic root declaration and canonical format for native source, but it is no longer the only maintainer-owned Authoring Source. Declared read-only CSV may own explicit canonical paths.

This ADR amends ADR-008. Source variables and `TranslationDictionary` remain authoritative, and arbitrary external spreadsheets still do not replace translation semantics. A source-controlled, explicitly declared CSV may now materialize the existing dictionary categories under the rules above.

Project scope and authoring documentation must distinguish the root Canonical Deck File, its Authoring Sources, and the resolved Canonical Deck. CSV write-back, general legacy recipe parity, live Anki sync, and adapter-to-source ownership recovery remain non-goals.
