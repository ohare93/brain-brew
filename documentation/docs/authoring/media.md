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

Media declaration paths use portable safe-relative syntax. Absolute, drive, UNC, backslash, `.`/`..`, empty components, controls/NUL, URL-scheme colons, bidi/zero-width format controls, trailing dots/spaces, and Windows device names fail before asset I/O. Spaces, Unicode, quotes, ampersands, `#`, `?`, and literal `%` are valid filename characters and are encoded safely when rendered. Asset reads and export destinations also require canonical containment, including when an existing parent is a symlink.

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

A CrowdAnki bootstrap import emits an ordinary new deck file and re-inlines the `media:` block; it does not preserve a previously hoisted media include or modify the source that was exported.

## CSV image fields

CSV note descriptors can map an image-only field explicitly:

```yaml
field.flag:
  column: main.flag
  type: image
```

Without a `delimiter`, each non-empty cell is exactly one stable media ID. Add an exact delimiter when a cell may contain several ordered images:

```yaml
field.flag:
  column: main.flag
  type: image
  delimiter: '|'
```

```csv
media.flag.bolivia.blur|media.flag.bolivia
```

Every segment must be a non-empty valid media ID. Brain Brew does not trim or escape segments. The delimiter only separates CSV source values; export renders the images as adjacent tags with no separator. An empty whole cell remains an ordinary empty scalar, and `media.yaml` remains the sole authority for paths and hashes. Scalar mappings preserve `<img ...>` text as raw HTML and never infer image references.

Normalize legacy `<img>` CSV cells to stable media IDs once before switching their mappings from `type: scalar` to `type: image`. Mixed text and images remain scalar HTML.

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

Brain Brew renders these to Anki-compatible HTML during export, for example `<img src="flags/france.svg" />`. Multi-image fields render as adjacent image tags with no separator. Rendering UTF-8 percent-encodes every byte except RFC 3986 unreserved characters and `/`, then HTML-attribute escapes the result. For example the declared filesystem/CrowdAnki filename `flags/旗 & #1?.svg` renders as `flags/%E6%97%97%20%26%20%231%3F.svg`; the declaration and `media_files` entry remain unchanged. Raw HTML/CSS scanners decode HTML entities and percent escapes before comparing references to declarations and reject malformed or unsafe encodings.

## When to keep raw HTML

Structured images are additive and optional. Raw HTML remains valid for mixed text, custom attributes, card templates, styling, links, sounds, and legacy fields:

```yaml
field.description: 'See <img src="flags/france.svg" /> for the flag.'
```

A field should be either raw text/HTML, a structured message, or structured image reference(s). Mixed text plus `!image` in the same field is not supported.

## Verification and migration

### Migrating from optional media roots

Older commands treated an omitted `--media-root` as reference-only checking. That implicit downgrade is removed. For every media-bearing release target:

1. run `brainbrew media hash` against each owning package's real root;
2. commit canonical 64-character lowercase hashes (never placeholders);
3. pass the root package root plus each dependency root as `<package-id>=<directory>` to verify/export;
4. remove blanket release-script `cp media/*` steps and let strict export stage only declarations;
5. use `--media-mode reference-only` only for fixtures/development checks that are explicitly not byte-integrity evidence.

Hashless fixtures do not need fake hashes: empty hashes are permitted only in reference-only mode. Any non-empty hash must still be canonical. Existing output is not touched when any media check fails.

`brainbrew verify` fails if an `!image` references an unknown media ID. Strict release verification is the default whenever the composed target has media: every final owning package needs an explicit root, every hash must be exactly 64 lowercase hexadecimal characters, and every byte must exist and match. A missing root never downgrades validation.

For a fixture or development workspace that deliberately has no media bytes, select the explicit non-release mode:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-mode reference-only
```

Reference-only mode still checks structured IDs, raw rendered references, declaration collisions, safe paths, and canonical syntax for every non-empty hash. It skips roots/bytes and prints `NOT RELEASE-READY` in human output; `--json` reports `media.release_ready: false`. It cannot be combined with `--media-root`.

To migrate existing strict image-only fields, run:

```bash
brainbrew media images-to-refs --manifest brainbrew.yaml --all-targets
```

The converter rewrites only fields whose entire trimmed content is one or more strict `<img src="PATH" />` tags and where every path maps to exactly one declared media ID. Non-strict HTML, undeclared paths, and duplicate path declarations are left raw and counted in the report.
