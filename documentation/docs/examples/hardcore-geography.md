---
title: Hardcore Geography overlay
---

# Hardcore Geography overlay

Hardcore Geography is represented in two fixture shapes: as an Ultimate Geography extension in `brainbrew.yaml`, and as the standalone upstream companion manifest in `brainbrew-hardcore.yaml`.

## Layout

```text
fixtures/ultimate-geography/brainbrew-hardcore.yaml
fixtures/ultimate-geography/deck-hardcore.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/field-fills/*.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/companion-note-type-translations/*.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/companion-translations/*.yaml
fixtures/ultimate-geography/overlays/extensions/hardcore/translations/*.yaml
```

## Responsibilities

`hardcore.yaml` adds shared extension structure:

- new Hardcore companion notes;
- `UG::Overlapping` tags where companion rows overlap main UG rows;
- media references;
- distinct `note.hardcore-*` stable IDs for overlapping companion notes so main UG notes are preserved.

`field-fills/<lang>.yaml` fills blank fields on Hardcore companion notes:

```yaml
id: overlay.extension.hardcore.field-fills
kind: extension
field_fills:
  note.hardcore-anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
```

`companion-note-type-translations/<lang>.yaml` and `companion-translations/<lang>.yaml` support the standalone Hardcore companion manifest. `translations/<lang>.yaml` translates new Hardcore notes and maps legacy translated GUIDs. There is no English Hardcore translation file because English only needs extension field fills.

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

- add separate `note.hardcore-*` companion notes;
- fill blank capital/flag fields on those companion notes;
- add `UG::Overlapping` to the companion notes;
- preserve existing Ultimate Geography maps and adapter IDs on the main UG notes.

The one known non-blank disagreement is left out of the extension:

- `note.bali.fields.field.country-info`: UG has `Island of Indonesia.`, Hardcore has `Province of Indonesia.`.

If maintainers want that wording change, add a patch overlay with reviewed expected bases.
