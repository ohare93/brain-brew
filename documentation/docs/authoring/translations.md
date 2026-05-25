---
title: Translation overlays
---

# Translation overlays

A translation overlay changes deck language or localized text. It should not add unrelated extension content.

## Basic dictionary

```yaml
id: overlay.translation.de
kind: translation
translations:
  changes:
    Germany: Deutschland
    Austria: Österreich
```

The source key is the expected base. If `Germany` no longer exists where the overlay expects it, composition fails with a stale translation entry.

## Path-scoped translations

Use a path when the same source text needs different translations in different places.

```yaml
translations:
  changes:
    Overseas territory of the United Kingdom.:
      notes.note.bermuda.fields.field.country-info: Britisches Überseegebiet.
      notes.note.falkland-islands.fields.field.country-info: Britisches Überseegebiet des Vereinigten Königreichs.
```

## Blank localized text

Use `translations.additions` only when blank localized text genuinely belongs to the translation overlay.

```yaml
translations:
  additions:
    notes.note.united-kingdom.fields.field.country-info: Offiziell das Vereinigte Königreich Großbritannien und Nordirland.
```

If an extension fills blank fields with new content, use [`field_fills`](field-fills.md) instead.

## Translate source variables

Variables keep card templates shared across languages.

Base source:

```yaml
note_types:
  note-type.country:
    variables:
      label.capital: Capital
      label.location: Location
    card_templates:
      template.map:
        question_format: '<div>${label.location}</div>{{Map}}'
```

Translation overlay:

```yaml
translations:
  variables:
    label.capital:
      Capital: Hauptstadt
    label.location:
      Location: Lage
```

Prefer variable translations over copying whole card templates per language.

## Translate adapter IDs

Legacy translated decks may already have different CrowdAnki GUIDs.

```yaml
translations:
  adapter_ids:
    crowdanki:guid:
      english-guid: german-guid
```

## Deterministic section order

The formatter emits translation dictionary sections in this order:

1. `require_complete`
2. `ignore_paths`
3. `changes`
4. `additions`
5. `variables`
6. `adapter_ids`

A file with no `changes` starts at the next non-empty section. That is still deterministic.
