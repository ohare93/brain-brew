---
title: Manifests and targets
---

# Manifests and targets

`brainbrew.yaml` names the reproducible build targets in a workspace.

## Minimal manifest

```yaml
package:
  id: example.capitals
  version: 0.1.0
base: deck.yaml
overlays: {}
targets:
  en-standard:
    overlays: []
```

Manifest `base`, overlay `file`, configured export `out`/`golden`, package-lock `manifest`, include, and media paths all use the canonical safe-relative syntax described below. Brain Brew checks canonical root containment before reads and checks the deepest existing ancestor before creating outputs.

## File include roots

`!include` paths in `deck.yaml` and overlay YAML use portable safe-relative syntax. They are resolved first beneath the package root (the directory containing the manifest), then beneath any configured in-package include roots:

```yaml
base: deck.yaml
include_roots:
  - shared-source-text
overlays: {}
targets:
  en-standard:
    overlays: []
```

Paths are non-empty, use `/`, and contain only normal components. Absolute paths, Windows drive/UNC forms, backslashes, repeated separators, and every `.` or `..` component are rejected before I/O. Symlinks may be followed only when their canonical target stays inside the selected root. Older manifests that used `../shared-source-text` must move that content beneath the package root or select it explicitly outside the manifest workflow.

The composed deck always contains the resolved scalar text; exported decks do not depend on these source files.

## Overlay catalog

The catalog gives every overlay a stable reference:

```yaml
overlays:
  overlay.translation.de:
    file: overlays/languages/de.yaml
    kind: translation
  overlay.variant.extended:
    file: overlays/variants/extended.yaml
    kind: extension
```

The catalog key must equal the decoded overlay file's `id`, and a declared manifest `kind` must equal the file's `kind`. Registry loading decodes every authorized catalog source before planning any target. Unknown kinds, aliases that give one source conflicting identities, and package-qualified references whose owning catalog/source disagrees fail closed.

## Overlay dependencies

Dependencies are inclusion dependencies. Selecting the dependent overlay selects its dependencies first.

```yaml
overlays:
  overlay.variant.extended.de:
    file: overlays/variants/extended/de.yaml
    kind: extension
    depends_on:
      - overlay.translation.de
      - overlay.variant.extended
```

The expanded stack is deterministic:

```bash
brainbrew explain --manifest brainbrew.yaml --target de-extended
```

## Targets

A target is a named composition goal.

```yaml
targets:
  de-extended:
    overlays:
      - overlay.variant.extended.de
    exports:
      crowdanki:
        out: build/crowdanki/de-extended
```

Users and CI select targets instead of memorizing overlay paths.

## Language catalog

A manifest can declare source and target languages explicitly so language-first tools such as the Deck Workbench do not infer meaning from target names.

```yaml
languages:
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
      extended: en-extended
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
      hardcore: overlay.translation.hardcore.da
    primary_target: standard
    targets:
      standard: da-standard
      extended: da-extended
```

`languages` is keyed by language code. A source language sets `source: true` and omits `translation_overlays`. A target language uses labeled `translation_overlays` so one language can translate base content plus extension-specific content, and its `targets` map connects friendly labels such as `standard` or `extended` to concrete build target IDs.

## Translation profile

`translation_profile` classifies progress for language-first review. Main completion focuses on non-structural note field text; metadata belongs in a separate checklist.

```yaml
translation_profile:
  structural_fields:
    - field.flag
    - field.map
  metadata_categories:
    - key: deck-metadata
      label: Deck metadata
      paths:
        - deck.name
        - deck.description
    - key: note-type-name
      label: Note type names
      paths:
        - note_types.*.name
    - key: field-label
      label: Field labels
      paths:
        - note_types.*.fields.*.name
    - key: card-template-name
      label: Card template names
      paths:
        - note_types.*.card_templates.*.name
    - key: tag
      label: Tags
      paths:
        - notes.*.tags.*
  metadata_paths:
    - deck.*
    - note_types.*
    - notes.*.tags.*
  metadata_exclude_paths:
    - deck.adapter_ids.*
    - note_types.*.adapter_ids.*
    - note_types.*.card_templates.*.adapter_ids.*
    - notes.*.adapter_ids.*
  metadata_category_order:
    - deck-metadata
    - note-type-name
    - field-label
    - card-template-name
    - tag
```

`structural_fields` excludes source fields such as flags or maps from main text completion. `metadata_categories` assigns metadata paths to maintainer-defined checklist groups with stable kebab-case keys and human labels. `metadata_paths` marks broad metadata review work instead of main translation completion, `metadata_exclude_paths` removes paths from that checklist even when a broad include such as `deck.*` or `note_types.*` would match, and `metadata_category_order` can override the checklist grouping order (otherwise category declaration order is used). Path `*` wildcards match dotted stable IDs, so `notes.*.tags.*` matches a path such as `notes.note.finland.tags.Europe`.

## Translation coverage policy

Translated targets can choose how strictly `brainbrew verify` treats untranslated fallbacks:

```yaml
targets:
  de-dev:
    overlays:
      - overlay.translation.de
    translation_coverage: lenient
  de-release:
    overlays:
      - overlay.translation.de
    translation_coverage: strict
```

`lenient` is the default and allows missing translations while translators are in progress. `strict` fails verification when a translation overlay leaves a non-empty source string untranslated. Use `brainbrew translations --target <target>` to inspect the missing/stale entries, and `--apply` to seed `source: source` stubs before handing the file to translators.

## Package-qualified targets

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

A downstream package declares an exact dependency pin plus an explicit compatible base range, then extends an upstream target. See [Packages and lock files](packages-locking.md#version-and-compatibility-semantics) for canonical SemVer, OR/AND, and prerelease behavior.

```yaml
package:
  id: anki-geo.america
  version: 0.1.0
  base_package: anki-geo.ultimate-geography
  compatible_base_versions:
    - '>=0.1.0, <0.2.0'
  depends_on:
    - anki-geo.ultimate-geography@0.1.0
targets:
  en-america:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - overlay.extension.america
```

It may also mix package-qualified overlays:

```yaml
targets:
  en-mixed:
    extends: anki-geo.ultimate-geography:en-standard
    overlays:
      - anki-geo.america:overlay.extension.america
      - anki-geo.mountains:overlay.extension.rockies
```
