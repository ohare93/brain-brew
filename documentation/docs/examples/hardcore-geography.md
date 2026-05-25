---
title: Hardcore Geography overlay
---

# Hardcore Geography overlay

Hardcore Geography is represented as an Ultimate Geography extension, not as a copied deck.

Source used for the migration:

- repository: [anki-geo/hardcore-geography](https://github.com/anki-geo/hardcore-geography)
- inspected commit: `09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46`

## Layout

```text
fixtures/ultimate-geography/overlays/extensions/hardcore.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/field-fills/*.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/translations/*.yaml
```

## Responsibilities

`hardcore.yaml` adds shared extension structure:

- new Hardcore notes;
- `UG::Overlapping` tags;
- media references;
- preserved base map fields for overlapping notes.

`field-fills/<lang>.yaml` fills blank fields on existing UG notes:

```yaml
id: overlay.extension.hardcore.field-fills.en
kind: extension
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
```

`translations/<lang>.yaml` translates new Hardcore notes and maps legacy translated GUIDs. There is no English Hardcore translation file because English only needs extension field fills.

## Composition order

```text
translation.<lang>
  -> extension.hardcore
  -> translation.hardcore.<lang>
  -> extension.hardcore.field-fills.<lang>
```

Extended Hardcore targets include the shared Extended variant too.

## Non-destructive policy

For overlapping rows, Hardcore is additive:

- fill blank capital/flag fields;
- add `UG::Overlapping`;
- preserve existing Ultimate Geography maps;
- preserve existing Ultimate Geography adapter IDs for existing notes.

The one known non-blank disagreement is left out of the extension:

- `note.bali.fields.field.country-info`: UG has `Island of Indonesia.`, Hardcore has `Province of Indonesia.`.

If maintainers want that wording change, add a patch overlay with reviewed expected bases.
