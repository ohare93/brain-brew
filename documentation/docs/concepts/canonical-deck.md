---
title: Canonical Deck
---

# Canonical Deck

A Canonical Deck is Brain Brew' format-independent representation of an Anki-compatible deck.

It includes deck metadata, note types, card templates, notes, tags, media references, typed path tombstones, stable IDs, and adapter IDs.

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

## Removal history

A non-empty tombstone identifies an exact entity/value kind and its full containing path, rather than only a Stable ID. It also retains the removing overlay and operation when composition produced it. This prevents a removed identity from being silently reintroduced while allowing identical ID text in another kind or parent scope. See [Typed tombstones](../reference/yaml.md#typed-tombstones) and [ADR-020](../reference/decisions/0020-address-removals-with-typed-path-tombstones.md).

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

## CrowdAnki adapter boundary

CrowdAnki import/export is an adapter around this model. Export preserves supported Anki-compatible deck semantics and external IDs; import creates a separate full-deck bootstrap output. The Canonical Deck source remains the source of truth, and import does not merge edits into an existing source or overlay stack. See [CrowdAnki bootstrap boundary](../authoring/crowdanki-bootstrap-boundary.md).
