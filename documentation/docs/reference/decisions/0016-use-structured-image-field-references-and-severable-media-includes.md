# ADR-016: Use Structured Image Field References and Severable Media Includes

**Date**: 2026-07-04  
**Status**: Proposed  
**Deciders**: Project Lead

## Context

Brain Brew currently represents media assets as declared media references keyed by stable ID, with each declaration carrying a `path` and `sha256` (`MediaReference` in `crates/brain-brew-core/src/model.rs:1170-1176`; `MediaYaml` in `crates/brain-brew-formats/src/canonical_yaml.rs:2272-2287`). Verification, however, compares used paths to declared paths: `referenced_paths` scans note field strings, styling, and card templates, and `reference_report` compares the used path set with `deck.media.values().map(|media| media.path)` (`crates/brain-brew-formats/src/media.rs:8-42,99-116`). `brainbrew media hash` also walks declaration paths and updates `sha256` entries by path (`crates/brain-brew-cli/src/commands/media.rs:163-192`).

Ultimate Geography fixtures use raw Anki-compatible image HTML today. Re-verification with `rg` found:

- 602 total `<img ...>` tags under `fixtures/ultimate-geography/**/*.yaml`, and all 602 exactly match `<img src="X" />` with no other attributes.
- `fixtures/ultimate-geography/deck.yaml` contains 546 strict image tags in 540 image-only field values: 221 `field.flag` values and 319 `field.map` values. The extra six tags are six `field.flag` blur+normal pairs.
- Hardcore extension overlays add 56 strict image tags in 55 image-only field values: `overlays/extensions/hardcore.yaml` has 33 single-image values, and `overlays/extensions/hardcore/field-fills.yaml` has 23 tags in 22 `field.flag` values.
- `fixtures/ultimate-geography/deck-hardcore.yaml` currently has no notes and no `<img>` tags; the audited Bali hardcore blur+normal pair is in `fixtures/ultimate-geography/overlays/extensions/hardcore/field-fills.yaml:15` as `<img src="ug-flag-bali-blur.png" /><img src="ug-flag-bali.png" />`.
- Every field containing image HTML contains only one or more exact image tags, optionally YAML-quoted. There is no audited UG field with mixed surrounding text plus image HTML.

The current canonical YAML model already has a precedent for structured field values: note fields store raw strings in `fields` and structured messages in a parallel `field_messages` map (`crates/brain-brew-core/src/model.rs:1118-1148`). Canonical YAML decodes field values as either scalar strings or message structures (`FieldValueYaml` in `crates/brain-brew-formats/src/canonical_yaml.rs:2138-2192`), overlays can set either a scalar `value` or a structured `message` (`FieldChangeYaml` in `canonical_yaml.rs:1878-1916`), compose preserves/removes the parallel structured map when field changes apply (`crates/brain-brew-core/src/compose.rs:1128-1151`), and `render_variables` lowers structured messages to plain field strings before adapter export (`compose.rs:1588-1702`). CrowdAnki export already calls `render_variables()` (`crates/brain-brew-formats/src/crowdanki.rs:17-24`).

File includes are scalar-content conveniences today, not structural splices. `resolve_file_includes` replaces `!include path` with a YAML string and rejects non-scalar-content paths; `is_scalar_content_path` explicitly returns false for any path containing `media` (`crates/brain-brew-formats/src/source_includes.rs:276-285,436-445`). The include-preserving format path also uses string sentinels (`source_includes.rs:46-117`), which cannot deserialize where `CanonicalDeckYaml` expects the top-level `media` mapping. In addition, `brainbrew media hash` mutates the raw source value with `media.as_mapping_mut()`; if `media:` is a tagged `!include`, `as_mapping_mut()` returns `None` and the command currently returns `Ok(0)` without updating any hashes (`crates/brain-brew-cli/src/commands/media.rs:163-192`). Finally, when a file contains any `!include`, the include resolver rejects any other tag as `UnsupportedTag`, so adding `!image` requires the resolver to pass `!image` through instead of failing (`source_includes.rs:287-294`).

## Decision

Canonical Deck YAML will support structured image field references with the tag form `!image <path>`, and media declarations may later be split into an includable structural `media:` mapping.

The image reference is the media path, not the media stable ID. The path in `!image ug-flag-france.svg` must exactly match both the `path` inside one declared media entry and the asset path used on disk. Stable IDs still identify media declarations, but field references do not use them.

Structured image references are additive. Raw HTML remains valid everywhere it is valid today, including note fields, overlay field changes, field additions, field fills, card template HTML, and styling. Card template HTML and styling remain raw HTML and continue to use regex extraction for media verification; structured `!image` is only for note field-value positions.

The accepted YAML shapes are:

```yaml
field.flag: !image ug-flag-france.svg

field.flag:
  - !image ug-flag-bali-blur.png
  - !image ug-flag-bali.png
```

A scalar `!image` is the canonical single-image form. A sequence is accepted only when every item is a tagged scalar `!image <path>` and the sequence is non-empty. The canonical emitter writes a single-element sequence back as the scalar form and writes two or more images as a block sequence of tagged scalars.

The exact YAML positions that accept `!image` are:

- base note field values under `notes.<note>.fields.<field>`;
- overlay note field change `value` under `notes.<note>.fields.<field>.value`;
- `field_additions.<note_type>.values.<note>.<field>` values;
- `field_fills.<note>.<field>` values.

Mixed text plus structured image in one field is out of scope. A field is either raw text/HTML, a structured message, or a structured image field. If a field needs text around an image, it remains a raw HTML string for now.

The core model will follow the ADR-008 structured-message precedent by adding a parallel structured map on `Note`, not by replacing all field storage with a field-value enum. The intended shape is a map such as `field_images: BTreeMap<StableId, Vec<FieldImageReference>>`, where each `FieldImageReference` stores the media `path`. `Note.fields` continues to contain raw field strings; validation rejects a field that is simultaneously represented by raw text, `field_messages`, and `field_images` in conflicting ways. Overlay field changes likewise gain a structured-image payload parallel to the existing scalar `value` and structured `message` payloads.

Rendering happens during `CanonicalDeck::render_variables()`, in the same lowering phase that already resolves structured messages before adapter export. Rendering `!image p` produces the exact byte string `<img src="p" />` with one space before `/>`. Rendering a multi-image field concatenates the rendered image tags with no separator bytes. The Bali hardcore field therefore renders exactly as `<img src="ug-flag-bali-blur.png" /><img src="ug-flag-bali.png" />`.

CrowdAnki import reverse-maps raw HTML to `!image` only when a field's entire trimmed content is one or more strict image tags matching this pattern:

```text
<img src="PATH" />(ASCII-whitespace* <img src="PATH" />)*
```

`PATH` must be non-empty and contain no double quote, `<`, `>`, carriage return, or newline. The only accepted attribute is `src`, it must be double-quoted, and the tag must use the exact self-closing form `<img src="PATH" />`. Leading and trailing whitespace is ignored by the trimmed-content check. ASCII whitespace between consecutive image tags is tolerated on import but canonicalizes to no separator when rendered. Any other HTML, attribute, non-self-closing form, surrounding text, comment, or mixed content stays as raw HTML.

Structured image references participate in media verification as exact path strings against declared media paths. `referenced_paths` should collect `field_images` paths directly, while raw string fields, card templates, and styling continue through the existing rendered-field regex scanner. Verification currently runs on the composed, pre-render deck (`crates/brain-brew-cli/src/commands/verify.rs:60-66`), so structured image references must survive compose and overlay merge just as `field_messages` survive today.

For structural media includes, `media: !include media.yaml` is the only initially approved mapping-position include. The included file is a media-map source file whose root YAML value is exactly the mapping normally found under top-level `media:`: stable media IDs mapped to `{path, sha256}` objects. The include resolver must parse that file as YAML and splice a mapping, not read it as a scalar string. This is a whitelist, not a general arbitrary-mapping include facility.

Included media files get their own format and verify treatment as a new source file kind: formatting canonicalizes the root media mapping with the same ordering and scalar rules as an inline `media:` block, and verification validates that the referenced include file parses as a media mapping and contributes declarations to the composed deck. `brainbrew media hash` must follow `media: !include ...` and write changed `sha256` values into the included media file rather than silently no-oping on the tagged top-level value.

The structural `media:` include work is severable from structured `!image` fields. If the include-preserving formatter, media hash writeback, or new media-map file kind expands the implementation, it should ship in a later focused run instead of delaying `!image` field references.

## Rationale

**Pros:**

- Path references match how media is declared, verified, hashed, and found on disk today.
- `!image <path>` is greppable and reviewable in field values without opening the media declaration block.
- Exported CrowdAnki output remains byte-identical after migration because the render contract fixes `<img src="p" />` exactly and joins multi-image fields with no separator.
- The parallel-map model is a small, precedent-backed extension of `field_messages` and avoids a broad field-value enum refactor across validation, compose, semantic diff, translation, YAML, and adapters.
- Strict import reverse-mapping avoids false positives: unsupported image HTML remains raw HTML and continues to work.
- Keeping raw HTML valid everywhere preserves current card template and styling behavior.
- Whitelisting only `media:` as a structural include keeps include semantics understandable and avoids opening arbitrary YAML AST splicing.

**Cons:**

- Path references are not rename-robust; renaming a media file requires updating declarations and `!image` field references together.
- The parallel-map model adds another mutually exclusive field representation that validation and compose must keep consistent.
- Structured image fields cannot express mixed text+image content; those fields remain raw HTML.
- Importing fields with whitespace between consecutive image tags canonicalizes that whitespace away if the field is reverse-mapped to structured images.
- Structural media includes need non-trivial formatter and writeback support because the current include-preserving path is scalar-sentinel based.

## Alternatives Considered

- **Reference media stable IDs in field values, such as `!image media.flag-france`**: rejected. Stable IDs are rename-robust, but all existing media validation and hashing behavior compares exact paths. Stable-ID field references would require a new resolution step during render and verify, and errors would have to explain both missing IDs and ID-to-path mismatches. The current codebase points toward path semantics: declarations are keyed by ID but checked by path, reference verification compares path sets, and `media hash` walks paths.
- **Replace note fields with a core field-value enum**: rejected for this phase. It is semantically tidy, but it is a broad model migration. A parallel `field_images` map matches the existing `field_messages` approach and localizes the change.
- **Allow structured image components inside `StructuredMessage`**: rejected for now because no audited UG field needs mixed text+image, and image references are media-verification concerns rather than translation components.
- **Convert all raw `<img>` HTML everywhere to `!image`**: rejected. Card templates and styling remain raw HTML surfaces; only note field-value positions get structured refs.
- **Allow arbitrary mapping-position `!include`**: rejected. It would make include resolution context-sensitive across the whole schema and would complicate formatting, diagnostics, and writeback. Only top-level `media:` is approved initially.
- **Block `media:` includes until all include-preserving formatting is redesigned**: rejected as a design outcome but accepted as an implementation sequencing option. The ADR designs the feature, while recommending that implementation be severed if it grows beyond the `!image` work.

## Implications

- Canonical YAML parsing and emission need tag-aware field value handling for `!image` scalars and non-empty sequences of `!image` scalars. Other tags in those positions remain invalid unless separately specified.
- The include resolver must no longer reject `!image` tags merely because a file also contains `!include`; it should pass `!image` through to canonical YAML parsing.
- Compose must preserve, replace, and remove structured image field data analogously to `field_messages`, including overlay field changes, `field_additions`, and `field_fills`.
- `render_variables()` becomes the single lowering point for both structured messages and structured image fields before adapter export.
- CrowdAnki import should prefer structured images only for strict whole-field image HTML and keep all other field HTML unchanged.
- Media verification must combine structured image paths with regex-extracted raw HTML paths and compare the result to declared media paths exactly.
- `media: !include media.yaml` should be implemented as a separate structural-include slice if it conflicts with the scalar include-preservation machinery or media hash writeback.
