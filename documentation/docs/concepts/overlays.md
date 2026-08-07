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
| `add` | create a new entity, or fill a blank field value; field-level `add` fails with `AlreadyExists` when the field is already non-blank |
| `merge` | update selected properties or sub-entities of an existing entity; field-level `merge` only fills a blank field value |
| `replace` | replace a value, requiring an expected base |
| `remove` | remove an entity/value, requiring an expected base |
| `override` | intentionally override another overlay, requiring an expected base |

`replace`, `remove`, and `override` require `expected_base` so stale overlays fail loudly. Sparse properties use the exact prior typed value. Complete note, note-type, field-definition, card-template, and media operations use a [canonical entity fingerprint](../reference/entity-fingerprints.md). Presence-only `entity_present` baselines are rejected. To change an existing non-blank field value, use `replace` with `expected_base`; field-level `merge`, `add`, and `fill` operations are for blank-field fills and fail closed when the field already has content.

A full `note:` or `note_type:` body is valid with `intent: add`, or with fingerprint-protected `replace`/`override`. Prefer sparse field or sub-entity changes for ordinary edits. A `merge` carrying a full note or note-type body fails closed.

A non-`add` field-definition change must target an existing field definition. If the field definition is absent, composition fails with `MissingOverlayTarget` instead of creating it implicitly.
