---
title: YAML format reference
---

# YAML format reference

This is a compact reference. See the authoring pages for examples and guidance.

## Canonical Deck file

Top-level keys:

```yaml
deck: {}
note_types: {}
notes: {}
media: {}
tombstones: []
```

Important rules:

- unknown keys fail;
- entity maps are keyed by stable ID;
- note type field/template order is explicit;
- `adapter_ids` preserve external identities.

## Overlay file

Top-level keys:

```yaml
id: overlay.example
kind: extension
translations: {}
field_additions: {}
field_fills: {}
deck: {}
note_types: {}
notes: {}
media: {}
```

Only `id` and `kind` are required. Other sections are sparse.

## Overlay kinds

```yaml
kind: translation # localized text
kind: extension   # new content or structure
kind: patch       # corrections or adjustments
kind: personal    # learner/local source content
```

## Translation dictionary

```yaml
translations:
  require_complete: true
  ignore_paths:
    - notes.note.example.fields.field.private-note
  changes:
    Germany: Deutschland
  additions:
    notes.note.example.fields.field.country-info: Localized blank text.
  variables:
    label.capital:
      Capital: Hauptstadt
  adapter_ids:
    crowdanki:guid:
      old-guid: new-guid
```

Formatter order is deterministic: `require_complete`, `ignore_paths`, `changes`, `additions`, `variables`, `adapter_ids`.

## Field additions

Adds field definitions and optionally fills those new fields. Notes that omit a newly added field receive a blank value automatically.

```yaml
field_additions:
  note-type.country:
    fields:
      field.population: Population
    values:
      note.france:
        field.population: 68 million
```

## Field fills

Fills existing blank fields with an expected blank base.

```yaml
field_fills:
  note.anguilla:
    field.capital: The Valley
```

## Manifest file

```yaml
package:
  id: example.capitals
  version: 0.1.0
  depends_on:
    - upstream.package@0.1.0
base: deck.yaml
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  de-standard:
    overlays:
      - overlay.translation.de
```

## Lock file

```yaml
version: 1
packages:
  upstream.package:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: git
      url: https://github.com/owner/repo.git
      ref: main
    locked:
      type: git
      url: https://github.com/owner/repo.git
      rev: abc123
      nar_hash: sha256-...
```
