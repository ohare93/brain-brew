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

Media declaration paths use the same portable safe-relative syntax as package files. Absolute, drive, UNC, backslash, `.`/`..`, and empty-component forms fail before asset I/O. Asset reads and export destinations also require canonical containment, including when an existing parent is a symlink.

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

## Federated ownership and media roots

Every final declaration is owned by the package source that introduced it. An ordered `replace` or `override` transfers ownership to the replacing package; a cross-package `merge`, ambiguous final path/output, or duplicate stable-ID add is rejected. Raw path references and stable-ID `!image` references resolve through that final owner—Brain Brew never guesses from the selected target's root. The narrow compatibility exception is same-package alias IDs with exactly the same owner root, path, and hash; they select identical bytes, while path-to-ID source migration still reports the alias as ambiguous instead of guessing an ID.

An unqualified root remains backward compatible and applies **only to the root package**:

```bash
brainbrew verify --manifest brainbrew.yaml --target combined --media-root media/
```

Map dependency roots explicitly with repeatable `<package-id>=<directory>` values:

```bash
brainbrew verify --manifest brainbrew.yaml --target combined \
  --media-root media/ \
  --media-root anki-geo.ultimate-geography=/srv/ug-media
```

Relative directories are resolved from the root manifest workspace. A qualified mapping takes its package identity from the registry; unknown packages, duplicate mappings (including both unqualified and qualified mappings for the root package), and missing mappings for a declaration owner fail before asset reads. Verify, export, and Workbench authorize each declaration path beneath only that selected owner root.

Media mutation is intentionally narrower: `media hash` and `media images-to-refs` may write only root-workspace Canonical Deck/Overlay sources. Explicit includes, package-root dependencies, and locked/cache packages are read-only. If a requested operation would change one, the entire operation fails before the transaction writes anything; locked package tree hashes are checked around mutation and caches are never repaired silently.

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
