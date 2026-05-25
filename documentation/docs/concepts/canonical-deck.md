---
title: Canonical Deck
---

# Canonical Deck

A Canonical Deck is Brain Brew' format-independent representation of an Anki-compatible deck.

It includes deck metadata, note types, card templates, notes, tags, media references, tombstones, stable IDs, and adapter IDs.

It excludes review history and scheduling state.

## Shape

```yaml
deck:
  id: deck.capitals
  name: Capital Cities
  description: A small geography deck.
  adapter_ids: {}
note_types:
  note-type.capital:
    name: Capital Card
    field_order:
      - field.country
      - field.capital
    fields:
      field.country:
        name: Country
      field.capital:
        name: Capital
    card_template_order:
      - template.country-to-capital
    card_templates:
      template.country-to-capital:
        name: Country → Capital
        question_format: '{{Country}}'
        answer_format: '{{Capital}}'
        adapter_ids: {}
    styling: ''
    adapter_ids: {}
notes:
  note.france:
    note_type_id: note-type.capital
    fields:
      field.country: France
      field.capital: Paris
    tags: []
    adapter_ids: {}
media: {}
tombstones: []
```

## Strict source

Canonical YAML is deliberately strict:

- unknown fields fail;
- stable IDs key entities;
- note type field/template order is explicit;
- formatting is deterministic;
- comments are not part of the durable model.

Run the formatter before review:

```bash
brainbrew fmt deck.yaml
```

## Round trips

CrowdAnki import/export is an adapter around this model. The adapter preserves Anki-compatible deck semantics and external IDs, but the Canonical Deck stays the source of truth.
