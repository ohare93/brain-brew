---
title: Workspace layout
---

# Workspace layout

A Federated Deck workspace contains a manifest, a base deck, and overlays.

```text
my-deck/
  brainbrew.yaml
  deck.yaml
  content/
    description.md
  templates/
    country-front.html
    country-back.html
  styles/
    cards.css
  schema/
    note-types.yaml
  overlays/
    languages/de.yaml
    variants/extended.yaml
    variants/extended/de.yaml
    extensions/rivers.yaml
    patches/capitals.yaml
  media/
    flags/fi.svg
```

## `deck.yaml`

The base Canonical Deck. It owns shared structure and content. Large scalar content can live in normal package files and be referenced from the deck source:

```yaml
deck:
  description: !include content/description.md
note_types:
  note-type.country:
    card_templates:
      template.country-capital:
        question_format: !include templates/country-front.html
        answer_format: !include templates/country-back.html
    styling: !include styles/cards.css
```

`!include` paths are resolved relative to the package root (the directory containing `brainbrew.yaml`) and the composed/resolved deck contains the final inlined text.

A base deck may also move its complete note-type ID mapping into one structural include:

```yaml
note_types: !include schema/note-types.yaml
```

The included file starts with note-type IDs at its root; it does not repeat the `note_types:` key. `brainbrew fmt deck.yaml` preserves the marker, while `brainbrew fmt schema/note-types.yaml` canonicalizes the standalone map. This form is base-deck-only: overlay `note_types` changes remain inline and sparse.

## `overlays/`

Sparse changes to the base deck. Keep overlays small and purpose-shaped:

- language overlays in `overlays/languages/`;
- shared variant overlays in `overlays/variants/`;
- optional content extensions in `overlays/extensions/`;
- corrections in `overlays/patches/`.

## CSV-backed translation ownership

A translation overlay may keep `translations.from_csv` beside ordinary inline dictionary entries. The declaration reads the note descriptor's exact unsuffixed and localized columns and materializes the existing translation dictionary; it does not replace notes or infer a language from the target name. Its mapped fields must exist but may be a subset of the resolved note type, so fields owned by structural overlays need not be repeated in a shared translation descriptor.

CSV-owned pairs are regenerated from the current CSV bytes. Source fingerprints detect input changes, but historical stale-translation review is unavailable because the old source key is not retained. To regain native stale detection, transfer the affected source text, note, or path to inline YAML by excluding it from `from_csv` and adding the equivalent inline decision in the same change:

```yaml
translations:
  from_csv:
    - descriptor: sources/countries.csv.yaml
      parameters:
        language: de
      exclude:
        source_texts:
          - Reusable source text
        note_ids:
          - note.france
        paths:
          - notes.note.germany.fields.field.capital
  direct:
    Reusable source text: Wiederverwendbarer Ausgangstext
  contextual:
    notes.note.germany.fields.field.capital:
      Berlin: Berlin
```

The three selectors are literal: exact non-empty source text, stable note ID, and exact canonical occurrence path. They do not support globs, regular expressions, or predicates. Every selector must match a CSV unit, and every excluded field or adapter-ID occurrence must have an equivalent inline decision. When only some occurrences of reusable text move, remaining CSV-owned occurrences become contextual so an imported global decision cannot cross the ownership boundary. `brainbrew translations` reports the remaining CSV-owned units and `--json` includes their declaration, CSV cell, canonical path, category, source, and target provenance.

## CSV-backed sparse field additions

An extension may source values only for fields that it adds at `field_additions.<note-type>.values.from_csv`:

```yaml
field_additions:
  note-type.country:
    fields:
      field.region-code: Region code
    values:
      from_csv:
        - descriptor: sources/regions.yaml
          parameters: {}
          exclude:
            note_ids:
              - note.france
      note.france:
        field.region-code: WE
```

The descriptor must map the same note type and only fields added by this `field_additions` block. Non-empty mapped scalar or image cells become ordinary field-addition values; empty cells and missing optional-join cells claim no ownership, so sparse rows are allowed. Unknown non-empty note rows, duplicate ownership, inline collisions, and unmatched exclusions fail the whole materialization. An excluded note must provide identical inline values in the same block, as shown above.

Formatting preserves the declaration and never rewrites CSV. Descriptor and table files participate in plans, locks, verification, and Workbench freshness. Workbench reports CSV-backed sparse cells as read-only with descriptor/file/row/column provenance; excluded inline values use normal YAML capabilities.

## `brainbrew.yaml`

The manifest declares package metadata, named overlays, dependencies, and build targets. It also defines the package root used for file includes.

```yaml
package:
  id: example.capitals
  version: 0.1.0
base: deck.yaml
include_roots:
  - shared-source-text
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  de-standard:
    overlays:
      - overlay.translation.de
```

Most packages do not need `include_roots`; use it only to search a dedicated source-text directory inside the package. Manifest-owned paths never select files outside the package: absolute paths, `.`/`..`, Windows drive/UNC forms, backslashes, and symlink escapes are rejected. Move older sibling-directory includes beneath the package root or supply an external source through an explicit caller-owned workflow.

## Formatting

Use canonical formatting as a review gate:

```bash
brainbrew fmt deck.yaml
brainbrew fmt brainbrew.yaml
find overlays -name '*.yaml' -print0 | xargs -0 -n1 brainbrew fmt
```

`brainbrew verify --all-targets` also checks formatting. The maintained
[Composable CSV certification fixture](composable-csv-certification.md) shows
all-CSV and mixed/native states together and provides the executable end-to-end
workflow.
