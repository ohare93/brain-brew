---
title: Workspace layout
---

# Workspace layout

A Federated Deck workspace contains a manifest, a base deck, and overlays.

```text
my-deck/
  brainbrew.yaml
  deck.yaml
  content/
    description.md
  templates/
    country-front.html
    country-back.html
  styles/
    cards.css
  schema/
    note-types.yaml
  overlays/
    languages/de.yaml
    variants/extended.yaml
    variants/extended/de.yaml
    extensions/rivers.yaml
    patches/capitals.yaml
  media/
    flags/fi.svg
```

## `deck.yaml`

The base Canonical Deck. It owns shared structure and content. Large scalar content can live in normal package files and be referenced from the deck source:

```yaml
deck:
  description: !include content/description.md
note_types:
  note-type.country:
    card_templates:
      template.country-capital:
        question_format: !include templates/country-front.html
        answer_format: !include templates/country-back.html
    styling: !include styles/cards.css
```

`!include` paths are resolved relative to the package root (the directory containing `brainbrew.yaml`) and the composed/resolved deck contains the final inlined text.

A base deck may also move its complete note-type ID mapping into one structural include:

```yaml
note_types: !include schema/note-types.yaml
```

The included file starts with note-type IDs at its root; it does not repeat the `note_types:` key. `brainbrew fmt deck.yaml` preserves the marker, while `brainbrew fmt schema/note-types.yaml` canonicalizes the standalone map. This form is base-deck-only: overlay `note_types` changes remain inline and sparse.

## `overlays/`

Sparse changes to the base deck. Keep overlays small and purpose-shaped:

- language overlays in `overlays/languages/`;
- shared variant overlays in `overlays/variants/`;
- optional content extensions in `overlays/extensions/`;
- corrections in `overlays/patches/`.

## `brainbrew.yaml`

The manifest declares package metadata, named overlays, dependencies, and build targets. It also defines the package root used for file includes.

```yaml
package:
  id: example.capitals
  version: 0.1.0
base: deck.yaml
include_roots:
  - shared-source-text
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
targets:
  de-standard:
    overlays:
      - overlay.translation.de
```

Most packages do not need `include_roots`; use it only to search a dedicated source-text directory inside the package. Manifest-owned paths never select files outside the package: absolute paths, `.`/`..`, Windows drive/UNC forms, backslashes, and symlink escapes are rejected. Move older sibling-directory includes beneath the package root or supply an external source through an explicit caller-owned workflow.

## Formatting

Use canonical formatting as a review gate:

```bash
brainbrew fmt deck.yaml
brainbrew fmt brainbrew.yaml
find overlays -name '*.yaml' -print0 | xargs -0 -n1 brainbrew fmt
```

`brainbrew verify --all-targets` also checks formatting.
