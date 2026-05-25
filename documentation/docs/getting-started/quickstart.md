---
title: Quick start
---

# Quick start

This page builds the smallest useful Federated Deck: one base deck, one translation overlay, and one target.

## 1. Create `deck.yaml`

```yaml
deck:
  id: deck.capitals
  name: Capital Cities
  description: A tiny example deck.
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

## 2. Add a translation overlay

Create `overlays/languages/de.yaml`:

```yaml
id: overlay.translation.de
kind: translation
translations:
  changes:
    France: Frankreich
    Paris: Paris
```

## 3. Add `brainbrew.yaml`

```yaml
package:
  id: example.capitals
  version: 0.1.0
base: deck.yaml
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  en-standard:
    overlays: []
  de-standard:
    overlays:
      - overlay.translation.de
```

## 4. Compose and verify

```bash
brainbrew targets --manifest brainbrew.yaml
brainbrew compose --manifest brainbrew.yaml --target de-standard --out build/de-standard.yaml
brainbrew verify --manifest brainbrew.yaml --all-targets
```

You now have a reproducible source package with named targets.

## What next?

- Add [source variables](../authoring/translations.md#translate-source-variables) for repeated template labels.
- Add an [extension overlay](../authoring/extensions.md) for new content.
- Add [field fills](../authoring/field-fills.md) when an extension fills blank fields on existing notes.
