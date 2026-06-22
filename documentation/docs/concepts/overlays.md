---
title: Overlay kinds
---

# Overlay kinds

An overlay is a sparse set of changes applied to a base deck.

Overlay `kind` is maintainer-facing metadata. It tells readers what kind of contribution they are reviewing.

## The four kinds

| Kind | Use it for | Avoid using it for |
| --- | --- | --- |
| `translation` | localized text, localized variables, translated adapter IDs | new deck content or structure |
| `extension` | new notes, fields, templates, media, tags, or blank-field content | corrections to existing upstream content |
| `patch` | reviewed corrections or adjustments to existing content | large new optional content sets |
| `personal` | learner/local content that should survive updates | shared upstream package changes |

## Translation overlay

```yaml
id: overlay.translation.de
kind: translation
translations:
  direct:
    Germany: Deutschland
  variables:
    label.capital:
      Capital: Hauptstadt
```

Translation dictionaries use the source text as an implicit expected base.

## Extension overlay

```yaml
id: overlay.extension.population
kind: extension
field_additions:
  note-type.country:
    fields:
      field.population: Population
    values:
      note.france:
        field.population: 68 million
```

Extensions add optional content or structure.

## Patch overlay

```yaml
id: overlay.patch.capitals
kind: patch
notes:
  note.south-africa:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: Pretoria, Cape Town, Bloemfontein
        expected_base:
          value: Pretoria
```

Patches should make reviewed corrections with explicit expected bases.

## Personal overlay

```yaml
id: overlay.personal.my-notes
kind: personal
notes:
  note.my-example:
    intent: add
    note:
      note_type_id: note-type.country
      fields:
        field.country: Exampleland
        field.capital: Example City
      tags:
        - MyNotes
      adapter_ids: {}
```

Personal overlays are source content only. Brain Brew does not store review history.

## Change intents

Inside overlays, individual changes declare an intent:

| Intent | Meaning |
| --- | --- |
| `add` | create a new entity or value |
| `merge` | update selected properties of an existing entity |
| `replace` | replace a value, requiring an expected base |
| `remove` | remove an entity/value, requiring an expected base |
| `override` | intentionally override another overlay, requiring an expected base |

`replace`, `remove`, and `override` require `expected_base` so stale overlays fail loudly.
