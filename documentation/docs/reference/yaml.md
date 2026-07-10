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
- map keys such as stable IDs, adapter keys, tags, variable names, and target-adaptation paths are validated and quoted when emitted; a key containing a newline or carriage return is rejected as `UnemittableYamlKey`;
- note type field/template order is explicit;
- `adapter_ids` preserve external identities;
- scalar content fields may use `!include package/relative/path` as an authoring convenience;
- deck files may use the single structural include `media: !include <media-map.yaml>` for the top-level media map.

### Typed tombstones

Non-empty tombstones are explicit typed records. `path` is the complete canonical DeckPath, including every nested owner:

```yaml
tombstones:
  - kind: note
    path: notes.note.finland
    removed_by: overlay.patch.remove-finland
    operation: remove
  - kind: field_definition
    path: note_types.note-type.country.fields.field.flag
  - kind: note_field
    path: notes.note.finland.fields.field.flag
  - kind: card_template
    path: note_types.note-type.country.card_templates.template.flag-country
  - kind: media_reference
    path: media.media.flag.finland
```

`removed_by` and `operation` are emitted together when composition provenance is available; compatibility records can omit both. `operation` is currently always `remove`. Records are ordered by typed address, duplicate exact addresses fail, and `kind` must agree with `path`.

A container removal writes one container record. It does not synthesize descendant records, but every descendant mutation checks its ancestors, so a later overlay cannot bypass a removed note, note type, or card template. The same StableId text in another kind or under another parent is independent. Reintroduction fails with code `tombstoned_address_reuse`, including the attempted intent/overlay and original removal provenance. `override` cannot clear provenance.

The flat form is read only for migration:

```yaml
tombstones: [note.finland]
```

Brain Brew infers only a retained top-level note, note type, or media reference, and only when exactly one kind matches. Unknown IDs, IDs shared across kinds, and bare nested field/template IDs fail with guidance. Migrate an unambiguous file with:

```bash
brainbrew fmt deck.yaml
```

For a rejected record, replace the bare ID manually with the intended `kind` and full `path`, then rerun `fmt`. Canonical output never writes flat IDs; empty output remains `tombstones: []`.

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

## Structured image fields

Image-only note fields may reference declared media by stable ID instead of embedding raw `<img>` HTML:

```yaml
media:
  media.flag.finland:
    path: flags/fi.svg
    sha256: 7b2b...
notes:
  note.finland:
    fields:
      field.flag: !image media.flag.finland
      field.comparison:
        - !image media.flag.finland.blur
        - !image media.flag.finland
```

Accepted positions are base note field values, overlay field change `value`, `field_additions` values, and `field_fills` values. A single image emits as scalar `!image <media-stable-id>`; multiple images emit as a non-empty sequence of `!image` tagged scalars. Unknown media IDs fail verification/rendering. Raw HTML remains valid and is required for mixed text plus images, custom attributes, card templates, and styling.

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

### Message reference scope and graph resolution

A `ref` is always a complete canonical `notes.<note-id>.fields.<field-id>` path. References may cross notes in the same composed deck; aliases and display names are not reference keys. Brain Brew builds one deck-wide graph from the final semantic `FieldValue` map after the ordered overlay/translation stack. Scalar fields are terminal nodes, messages depend on every referenced field, and structured images are terminal semantic nodes lowered by the rendering adapter. A message may therefore reference a scalar, another message, or an image field. Image lowering keeps the exact deterministic `<img src="..." />` adapter form documented above.

Validation and rendering use the same graph plan. Missing notes, missing field definitions, absent values, tombstoned dependencies, malformed paths/messages, and unrepresentable image targets are typed failures carrying the consuming note/field/component path and dependency. Cycles fail validation before export. Their diagnostic includes a canonical closed path such as `A -> B -> C -> A`; a tail leading into the cycle is not included in the closed trace.

Nodes and edges are traversed in canonical field-path order, dependencies are emitted before consumers, and each node is resolved at most once. Successful planning/resolution is `O(V + E)` after the canonical deck maps have been built. Diamonds share the same memoized dependency value. Output and diagnostics do not depend on source map insertion order or unrelated fields. A later overlay replacement remains visible through every live reference; only an explicit full-message translation or consuming-path translation that intentionally differs from the referenced field materializes a literal and severs that edge.

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
  variables:
    label.capital:
      Capital: Hauptstadt
  adapter_ids:
    crowdanki:guid:
      old-guid: new-guid
```

Formatter order is deterministic inside `translations`: `require_complete`, `ignore_paths`, `direct`, `contextual`, `no_change`, `variables`, `adapter_ids`.

Top-level target adaptations and stale translations are emitted after `translations`:

```yaml
target_adaptations:
  notes.note.example.fields.field.country-info:
    expected_source: ''
    target: Localized blank text.
stale_translations:
  - old_source: Old source text.
    new_source: New source text.
    target: Existing target text needing review.
    context: notes.note.example.fields.field.country-info
```

A target-adaptation path may be present in either the top-level `target_adaptations` map or the legacy `translations.target_adaptations` map, but not both. Duplicating the same path in both places is rejected as an invalid translation dictionary.

## Expected bases and complete entity fingerprints

Sparse destructive changes use the exact prior typed value under `expected_base.value`. Complete note, note-type, field-definition, card-template, and media replacement/override/removal uses:

```yaml
expected_base:
  fingerprint: sha256:v1:<64 lowercase hexadecimal digits>
```

`expected_base: entity_present` is no longer valid. Generate fingerprints from the intended exact prior deck with `brainbrew diff --as-overlay`; do not derive hashes by hand. Algorithm/version/digest shape is validated while decoding, and composition compares the actual current entity immediately before mutation. See [Canonical entity fingerprints](entity-fingerprints.md) for the byte-level specification and migration.

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
      note.finland:
        field.population-map: !image media.map.population.finland
```

## Field fills

Fills existing blank fields with an expected blank base.

```yaml
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: !image media.flag.anguilla
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

Deck files also support one structural include form for large media declarations:

```yaml
media: !include media.yaml
```

The included file is a standalone media map whose root is the normal `media:` mapping contents, with stable IDs at column 0:

```yaml
media.flag.france:
  path: flags/france.svg
  sha256: 7b2b...
```

This whitelist applies only to top-level `media:` in deck files. It is not supported in overlay files or any other mapping position. Formatting a deck that uses the structural include preserves `media: !include media.yaml`; format the included file itself with `brainbrew fmt media.yaml`. `brainbrew media hash` follows the include and writes hashes into the media-map file. A CrowdAnki import that rewrites the deck re-inlines `media:` instead of preserving the include.

Include paths use one portable safe-relative syntax and are authorized beneath a selected canonical root. Under a manifest workflow, the package root is selected first; optional `include_roots` may name additional existing directories inside that package. Empty, absolute/rooted, Windows drive/UNC, backslash-separated, repeated-separator, `.` component, and `..` component forms are rejected before target I/O. Existing targets and the deepest existing ancestor of new targets must resolve canonically beneath the selected root, so escaping symlinks fail.

Formatting preserves scalar `!include` directives; composition materializes their scalar content in memory. This is an intentional compatibility change: older `../shared` includes are no longer accepted, even with `include_roots`. Move shared files under the package or select an external root explicitly at a caller-owned boundary.

## Manifest file

```yaml
package:
  id: example.capitals-extension
  version: 0.1.0
  base_package: upstream.package
  compatible_base_versions:
    - '>=0.1.0, <0.2.0'
  depends_on:
    - upstream.package@0.1.0
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
    translation_coverage: strict # optional; lenient by default
```

`package.version` and every exact dependency version are full Semantic Versions. `depends_on` requires `<package-id>@<SemVer>`; ranges are not accepted there. `base_package` and a non-empty `compatible_base_versions` list are declared together for extension/base compatibility, while base packages omit both. Each compatibility list item is an OR branch and comma-separated comparators inside an item are AND. Requirements are canonicalized and use the `semver` crate's prerelease matching behavior. See [Packages and lock files](../authoring/packages-locking.md#version-and-compatibility-semantics).

Overlay catalog keys must equal decoded `overlay.id`; declared catalog kinds must equal decoded `overlay.kind` and be one of `translation`, `extension`, `patch`, or `personal`.

## Lock file

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

```yaml
version: 2
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
      rev: 0123456789abcdef0123456789abcdef01234567
      nar_hash: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
```

Version 2 uses source-tagged path, Git, and tarball mappings. Every locked source requires one canonical SRI SHA-256 NAR hash; Git locks additionally require a full immutable commit ID. Unknown and source-inapplicable fields are rejected. See [Lock file reference](lockfile.md), including version 1 migration guidance.
