---
title: Extension overlays
---

# Extension overlays

An extension overlay adds optional content or structure without copying the full deck.

## Add fields and values

Use `field_additions` when an extension adds fields to an existing note type and optionally fills them on existing notes. Existing notes that do not provide a value for a new field receive a blank value automatically.

```yaml
id: overlay.extension.population
kind: extension
field_additions:
  note-type.country:
    fields:
      field.population: Population
      field.area: Area
    values:
      note.france:
        field.population: 68 million
        field.area: 643,801 km²
      note.germany:
        field.population: 84 million
        field.area: 357,592 km²
```

## Add notes

```yaml
id: overlay.extension.regions
kind: extension
notes:
  note.brittany:
    intent: add
    note:
      note_type_id: note-type.country
      fields:
        field.country: Brittany
        field.capital: Rennes
      tags:
        - Europe
      adapter_ids: {}
```

## Add card templates

```yaml
id: overlay.variant.extended
kind: extension
note_types:
  note-type.country:
    intent: merge
    card_templates:
      template.capital-to-country:
        intent: add
        insert_after: template.country-to-capital
        template:
          name: Capital → Country
          question_format: '{{Capital}}'
          answer_format: '{{Country}}'
          adapter_ids: {}
```

## Shared extension, small language residues

For variants such as “Extended”, put shared structure in one overlay:

```text
overlays/variants/extended.yaml
```

Put only real language-specific residue in per-language files:

```text
overlays/variants/extended/de.yaml
```

Avoid copying full template HTML just to translate labels. Use source variables and translation dictionaries instead.
