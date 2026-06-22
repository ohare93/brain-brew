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

## File include roots

`!include` paths in `deck.yaml` and overlay YAML are resolved relative to the package root, which is the directory containing the manifest. They cannot escape that root by default. If a workspace intentionally shares source text from a sibling directory, configure a safe include root:

```yaml
base: deck.yaml
include_roots:
  - ../shared-source-text
overlays: {}
targets:
  en-standard:
    overlays: []
```

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

The manifest `kind` should match the overlay file's `kind`.

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

A downstream package can extend an upstream target:

```yaml
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
