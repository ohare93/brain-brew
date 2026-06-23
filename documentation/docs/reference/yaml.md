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
- `adapter_ids` preserve external identities;
- scalar content fields may use `!include package/relative/path` as an authoring convenience.

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

Only `id` and `kind` are required. Other sections are sparse. Overlay scalar content values may also use `!include package/relative/path`.

## Overlay kinds

```yaml
kind: translation # localized text
kind: extension   # new content or structure
kind: patch       # corrections or adjustments
kind: personal    # learner/local source content
```

## Structured field messages

Note fields are usually scalar strings. For genuinely composite text that should reuse existing translations, a field may use an inline `format` with named message variables:

```yaml
notes:
  note.finland:
    fields:
      field.flag-similarity:
        format: '{country} ({description})'
        variables:
          country:
            ref: notes.note.iceland.fields.field.country
          description:
            text: blue background with a white cross
```

`format` renders `{variable}` placeholders and can itself be translated when a language needs different glue or ordering. `ref` resolves another note field before export, `text` is extracted for translation coverage at the named variable path, and `literal` is available for non-translatable named variables. Adapter exports receive a plain resolved string.

The older positional component form remains accepted for simple cases:

```yaml
field.flag-similarity:
  message:
    - ref: notes.note.iceland.fields.field.country
    - literal: ' ('
    - text: blue background with a white cross
    - literal: ')'
```

## Translation dictionary

```yaml
translations:
  require_complete: true
  ignore_paths:
    - notes.note.example.fields.field.private-note
  direct:
    Germany: Deutschland
  contextual:
    notes.note:
      country-georgia:
        Georgia: Georgien
      us-georgia.fields.field.region:
        Georgia: Georgia
  no_change:
    - Andorra
    - Djibouti
  target_additions:
    notes.note.example.fields.field.country-info: Localized blank text.
  stale_records:
    - old_source: Old source text.
      new_source: New source text.
      target: Existing target text needing review.
      context: notes.note.example.fields.field.country-info
  variables:
    label.capital:
      Capital: Hauptstadt
  adapter_ids:
    crowdanki:guid:
      old-guid: new-guid
```

Formatter order is deterministic: `require_complete`, `ignore_paths`, `direct`, `contextual`, `no_change`, `target_additions`, `stale_records`, `variables`, `adapter_ids`.

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

## File includes

Use `!include` when a scalar content field is easier to maintain as a normal file:

```yaml
deck:
  description: !include content/description.md
note_types:
  note-type.country:
    card_templates:
      template.country-capital:
        question_format: !include templates/country-capital-front.html
        answer_format: !include templates/country-capital-back.html
    styling: !include styles/cards.css
```

In overlays, include the value at the scalar property you are changing:

```yaml
deck:
  description:
    intent: replace
    value: !include content/translated-description.md
    expected_base:
      value: !include content/base-description.md
```

Include paths are deterministic and package-root-relative: under a manifest workflow they are resolved relative to the directory containing `brainbrew.yaml`. A path may not escape that package root unless the manifest declares an explicit safe include root. Formatting a file that uses `!include` materializes the included scalar content into canonical YAML.

## Manifest file

```yaml
package:
  id: example.capitals
  version: 0.1.0
  depends_on:
    - upstream.package@0.1.0
base: deck.yaml
include_roots:
  - ../shared-source-text
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  de-standard:
    overlays:
      - overlay.translation.de
    translation_coverage: strict # optional; lenient by default
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
