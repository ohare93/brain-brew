---
title: Authoring media fields
---

# Authoring media fields

Declare media once with a stable ID, then reference that ID from image-only note fields:

```yaml
media:
  media.flag.france:
    path: flags/france.svg
    sha256: 7b2b...

notes:
  note.france:
    fields:
      field.flag: !image media.flag.france
```

`!image` uses the media stable ID, not the file path. If `flags/france.svg` is later renamed, update only the `media:` declaration's `path`; field references remain stable.

## Hoisting large media maps

Deck files may keep the top-level media declaration in a separate media-map file:

```yaml
media: !include media.yaml
```

The included file is not a full deck or overlay. Its root value is exactly the mapping that would normally live under `media:`:

```yaml
media.flag.france:
  path: flags/france.svg
  sha256: 7b2b...
media.map.france:
  path: maps/france.png
  sha256: a91c...
```

This structural include is deliberately narrow: `media: !include <file>` is supported only for the top-level `media:` key in deck files. It is not supported in overlay files, and `!include` is not a general mapping splice elsewhere. `brainbrew fmt media.yaml` canonicalizes the standalone media map, while formatting the deck preserves `media: !include media.yaml`. `brainbrew media hash` follows the include and writes refreshed `sha256` values back to the included media file.

A CrowdAnki import that rewrites a deck emits an ordinary deck file and re-inlines the `media:` block; it does not preserve a previously hoisted media include.

## Single and multi-image fields

Use a scalar tag for one image:

```yaml
field.flag: !image media.flag.france
```

Use a non-empty sequence when the field is exactly multiple images, such as a blur plus the normal flag:

```yaml
field.flag:
  - !image media.flag.bali.blur
  - !image media.flag.bali
```

Brain Brew renders these to Anki-compatible HTML during export, for example `<img src="flags/france.svg" />`. Multi-image fields render as adjacent image tags with no separator.

## When to keep raw HTML

Structured images are additive and optional. Raw HTML remains valid for mixed text, custom attributes, card templates, styling, links, sounds, and legacy fields:

```yaml
field.description: 'See <img src="flags/france.svg" /> for the flag.'
```

A field should be either raw text/HTML, a structured message, or structured image reference(s). Mixed text plus `!image` in the same field is not supported.

## Verification and migration

`brainbrew verify` fails if an `!image` references an unknown media ID. It still checks that the resolved media path is declared, hashed, and present when `--media-root` is supplied.

To migrate existing strict image-only fields, run:

```bash
brainbrew media images-to-refs --manifest brainbrew.yaml --all-targets
```

The converter rewrites only fields whose entire trimmed content is one or more strict `<img src="PATH" />` tags and where every path maps to exactly one declared media ID. Non-strict HTML, undeclared paths, and duplicate path declarations are left raw and counted in the report.
