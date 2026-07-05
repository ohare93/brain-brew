# ADR-016: Use Structured Image Field References and Severable Media Includes

**Date**: 2026-07-04  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew currently represents media assets as declared media references keyed by stable ID, with each declaration carrying a `path` and `sha256` (`MediaReference` in `crates/brain-brew-core/src/model.rs`; `MediaYaml` in `crates/brain-brew-formats/src/canonical_yaml.rs`). Verification, however, compares used paths to declared paths: `referenced_paths` scans note field strings, styling, and card templates, and `reference_report` compares the used path set with `deck.media.values().map(|media| media.path)` (`crates/brain-brew-formats/src/media.rs`). `brainbrew media hash` also walks declaration paths and updates `sha256` entries by path (`crates/brain-brew-cli/src/commands/media.rs`).

Ultimate Geography fixtures use raw Anki-compatible image HTML today. Re-verification with `rg` found:

- 602 total `<img ...>` tags under `fixtures/ultimate-geography/**/*.yaml`, and all 602 exactly match `<img src="X" />` with no other attributes.
- `fixtures/ultimate-geography/deck.yaml` contains 546 strict image tags in 540 image-only field values: 221 `field.flag` values and 319 `field.map` values. The extra six tags are six `field.flag` blur+normal pairs.
- Hardcore extension overlays add 56 strict image tags in 55 image-only field values: `overlays/extensions/hardcore.yaml` has 33 single-image values, and `overlays/extensions/hardcore/field-fills.yaml` has 23 tags in 22 `field.flag` values.
- `fixtures/ultimate-geography/deck-hardcore.yaml` currently has no notes and no `<img>` tags; the audited Bali hardcore blur+normal pair is in `fixtures/ultimate-geography/overlays/extensions/hardcore/field-fills.yaml:15` as `<img src="ug-flag-bali-blur.png" /><img src="ug-flag-bali.png" />`.
- Every field containing image HTML contains only one or more exact image tags, optionally YAML-quoted. There is no audited UG field with mixed surrounding text plus image HTML.

The current canonical YAML model already has a precedent for structured field values: note fields store raw strings in `fields` and structured messages in a parallel `field_messages` map (`crates/brain-brew-core/src/model.rs`). Canonical YAML decodes field values as either scalar strings or message structures (`FieldValueYaml` in `crates/brain-brew-formats/src/canonical_yaml.rs`), overlays can set either a scalar `value` or a structured `message` (`FieldChangeYaml` in `canonical_yaml.rs`), compose preserves/removes the parallel structured map when field changes apply (`crates/brain-brew-core/src/compose.rs`), and `render_variables` lowers structured messages to plain field strings before adapter export (`compose.rs`). CrowdAnki export already calls `render_variables()` (`crates/brain-brew-formats/src/crowdanki.rs`).

File includes are scalar-content conveniences today, not structural splices. `resolve_file_includes` replaces `!include path` with a YAML string and rejects non-scalar-content paths; `is_scalar_content_path` explicitly returns false for any path containing `media` (`crates/brain-brew-formats/src/source_includes.rs`). The include-preserving format path also uses string sentinels (`source_includes.rs`), which cannot deserialize where `CanonicalDeckYaml` expects the top-level `media` mapping. In addition, `brainbrew media hash` mutates the raw source value with `media.as_mapping_mut()`; if `media:` is a tagged `!include`, `as_mapping_mut()` returns `None` and the command currently returns `Ok(0)` without updating any hashes (`crates/brain-brew-cli/src/commands/media.rs`). Finally, when a file contains any `!include`, the include resolver rejects any other tag as `UnsupportedTag`, so adding `!image` requires the resolver to pass `!image` through instead of failing (`source_includes.rs`).

## Decision

Canonical Deck YAML will support structured image field references with the tag form `!image <media-stable-id>`, and media declarations may later be split into an includable structural `media:` mapping.

1. **Reference form: `!image <media-stable-id>`.**

   The image reference is the stable ID key of a declared media reference, not the media file path. For example, `field.flag: !image flag-france` refers to the `flag-france` entry in the deck's `media:` declaration map. The referenced declaration then supplies the media asset path and hash:

   ```yaml
   media:
     flag-france:
       path: ug-flag-france.svg
       sha256: ...

   notes:
     note.france:
       fields:
         field.flag: !image flag-france
   ```

   This matches the existing model shape: media declarations are keyed by stable ID and each declaration stores `{path, sha256}` (`MediaReference` has `id`, `path`, and `sha256`; `MediaYaml` deserializes `path` and `sha256` under the map key). Renaming a media file changes only the one declaration's `path`; every structured field reference to that media stable ID keeps working untouched. The trade-off is indirection: render, import reverse-mapping, and verification need an explicit ID-to-declaration resolution step, and unknown IDs become hard errors.

2. **Multi-image sequence shape.**

   The accepted YAML shapes are:

   ```yaml
   field.flag: !image flag-france

   field.flag:
     - !image flag-bali-blur
     - !image flag-bali
   ```

   A scalar `!image` is the canonical single-image form. A sequence is accepted only when every item is a tagged scalar `!image <media-stable-id>` and the sequence is non-empty. The canonical emitter writes a single-element sequence back as the scalar form and writes two or more images as a block sequence of tagged scalars.

3. **Accepted positions, raw-HTML fallback, and additive scope.**

   Structured image references are additive. Raw HTML remains valid everywhere it is valid today, including note fields, overlay field changes, field additions, field fills, card template HTML, and styling. Card template HTML and styling remain raw HTML and continue to use regex extraction for media verification; structured `!image` is only for note field-value positions.

   The exact YAML positions that accept `!image` are:

   - base note field values under `notes.<note>.fields.<field>`;
   - overlay note field change `value` under `notes.<note>.fields.<field>.value`;
   - `field_additions.<note_type>.values.<note>.<field>` values;
   - `field_fills.<note>.<field>` values.

   Mixed text plus structured image in one field is out of scope. A field is either raw text/HTML, a structured message, or a structured image field. If a field needs text around an image, it remains a raw HTML string for now.

4. **Core representation and render contract.**

   The core model will follow the ADR-008 structured-message precedent by adding a parallel structured map on `Note`, not by replacing all field storage with a field-value enum. The intended shape is a map such as `field_images: BTreeMap<StableId, Vec<FieldImageReference>>`, where each `FieldImageReference` stores the referenced media stable ID. `Note.fields` continues to contain raw field strings; validation rejects a field that is simultaneously represented by raw text, `field_messages`, and `field_images` in conflicting ways. Overlay field changes likewise gain a structured-image payload parallel to the existing scalar `value` and structured `message` payloads.

   Rendering happens during `CanonicalDeck::render_variables()`, in the same lowering phase that already resolves structured messages before adapter export. Rendering `!image id` first resolves `id` against the composed deck's media declarations. If a matching media declaration exists, rendering uses that declaration's `path` and produces the exact byte string `<img src="<path>" />` with one space before `/>`. Rendering a multi-image field concatenates the rendered image tags with no separator bytes.

   Export stays byte-identical to today's raw HTML when each structured ID resolves to the same path the raw HTML previously used. That is the migration-equivalence contract. For example, if `flag-bali-blur` resolves to `ug-flag-bali-blur.png` and `flag-bali` resolves to `ug-flag-bali.png`, the Bali hardcore field renders exactly as `<img src="ug-flag-bali-blur.png" /><img src="ug-flag-bali.png" />`.

   If an `!image` ID has no matching media declaration in the composed deck, rendering fails closed with a hard render/compose error naming the missing media ID and the field path, such as `unknown media id "flag-france" referenced in field notes.note.france.fields.field.flag`. This follows ADR-0010's rule that unsupported or unmodeled adapter data must be rejected with a clear diagnostic rather than silently dropped or guessed.

5. **CrowdAnki import reverse-mapping.**

   CrowdAnki import reverse-maps raw HTML to `!image` only when a field's entire trimmed content is one or more strict image tags matching this pattern:

   ```text
   <img src="PATH" />(ASCII-whitespace* <img src="PATH" />)*
   ```

   `PATH` must be non-empty and contain no double quote, `<`, `>`, carriage return, or newline. The only accepted attribute is `src`, it must be double-quoted, and the tag must use the exact self-closing form `<img src="PATH" />`. Leading and trailing whitespace is ignored by the trimmed-content check. ASCII whitespace between consecutive image tags is tolerated on import but canonicalizes to no separator when rendered. Any other HTML, attribute, non-self-closing form, surrounding text, comment, or mixed content stays as raw HTML.

   For each strict tag, import maps `PATH` to a media stable ID by looking up the composed/imported deck's media declaration whose `path == PATH`, then emits `!image <that declaration's stable-id>`. The UG survey evidence remains applicable: the audited strict fields qualify for reverse-mapping, and their paths map to the corresponding declaration IDs.

   Ambiguous or incomplete mappings are safe fallbacks, not import errors:

   - If no media declaration has `path == PATH`, the field stays raw HTML.
   - If more than one media declaration has `path == PATH`, the field stays raw HTML because the path-to-ID mapping is ambiguous and import must not choose an arbitrary stable ID.
   - If a multi-image field contains any tag whose path has no unique declaration ID, the whole field stays raw HTML so import does not create a partially structured field.

6. **Verification semantics.**

   Structured image references participate in media verification by resolving media stable IDs against the composed, pre-render deck. An `!image` whose ID is not a declared media ID is an error: `unknown media id "<id>" referenced in field "<path>"`. After successful resolution, the declaration's `path`, `sha256`, and on-disk asset checks are covered by the existing media-integrity behavior from task 0080.

   Raw string fields, card templates, and styling continue through the existing rendered-field regex scanner and compare referenced paths with declared paths. The referenced-versus-declared check therefore has two branches:

   - structured `!image` references are checked by media ID existence, then resolved to declarations for path/hash/on-disk validation;
   - raw HTML references are checked by path-set comparison, as today.

   Verification currently runs on the composed, pre-render deck (`crates/brain-brew-cli/src/commands/verify.rs`), so structured image references must survive compose and overlay merge just as `field_messages` survive today. This changes the future role of the task-0080 extractor: structured refs are no longer matched by path in the regex innards; they are resolved by ID, while regex path extraction remains for raw HTML surfaces.

7. **Structural `media: !include` remains severable.**

   For structural media includes, `media: !include media.yaml` is the only initially approved mapping-position include. The included file is a media-map source file whose root YAML value is exactly the mapping normally found under top-level `media:`: stable media IDs mapped to `{path, sha256}` objects. The include resolver must parse that file as YAML and splice a mapping, not read it as a scalar string. This is a whitelist, not a general arbitrary-mapping include facility.

   Included media files get their own format and verify treatment as a new source file kind: formatting canonicalizes the root media mapping with the same ordering and scalar rules as an inline `media:` block, and verification validates that the referenced include file parses as a media mapping and contributes declarations to the composed deck. `brainbrew media hash` must follow `media: !include ...` and write changed `sha256` values into the included media file rather than silently no-oping on the tagged top-level value.

   The structural `media:` include work is severable from structured `!image` fields. If the include-preserving formatter, media hash writeback, or new media-map file kind expands the implementation, it should ship in a later focused run instead of delaying `!image` field references. Stable-ID field references make this severable media block more valuable: field references do not care whether media declarations live inline or in an included media map, because they resolve through the composed declaration ID set either way.

## Rationale

**Pros:**

- `!image <media-stable-id>` is rename-robust: renaming a media file updates one declaration `path`, while all field references to that media ID remain unchanged.
- Referencing the declaration ID matches Brain Brew's stable-ID model for deck entities and the existing media declaration shape keyed by ID with `{path, sha256}` values.
- Exported CrowdAnki output remains byte-identical after migration when each ID resolves to the same path previously used in raw HTML, because the render contract fixes `<img src="<path>" />` exactly and joins multi-image fields with no separator.
- The parallel-map model is a small, precedent-backed extension of `field_messages` and avoids a broad field-value enum refactor across validation, compose, semantic diff, translation, YAML, and adapters.
- Strict import reverse-mapping avoids false positives: unsupported image HTML remains raw HTML and continues to work.
- Keeping raw HTML valid everywhere preserves current card template and styling behavior.
- Whitelisting only `media:` as a structural include keeps include semantics understandable and avoids opening arbitrary YAML AST splicing.

**Cons:**

- Stable-ID references add indirection: reviewers may need to open the media declaration block to see the eventual file path.
- Render, import, and verify need explicit resolution steps, and unknown media IDs are hard errors.
- The parallel-map model adds another mutually exclusive field representation that validation and compose must keep consistent.
- Structured image fields cannot express mixed text+image content; those fields remain raw HTML.
- Importing fields with whitespace between consecutive image tags canonicalizes that whitespace away if the field is reverse-mapped to structured images.
- Structural media includes need non-trivial formatter and writeback support because the current include-preserving path is scalar-sentinel based.

## Alternatives Considered

- **Reference media paths in field values, such as `!image ug-flag-france.svg`**: rejected. This was the original proposed choice because it matched today's verification, hashing, disk lookup, and raw HTML export path strings directly. It is also greppable without opening the media declaration block. The Project Lead reversed this decision because path references are not rename-robust: renaming a media asset requires updating both the declaration's `path` and every structured field reference. The accepted stable-ID form keeps field references stable and localizes file renames to the declaration, at the cost of indirection and explicit resolution.
- **Replace note fields with a core field-value enum**: rejected for this phase. It is semantically tidy, but it is a broad model migration. A parallel `field_images` map matches the existing `field_messages` approach and localizes the change.
- **Allow structured image components inside `StructuredMessage`**: rejected for now because no audited UG field needs mixed text+image, and image references are media-verification concerns rather than translation components.
- **Convert all raw `<img>` HTML everywhere to `!image`**: rejected. Card templates and styling remain raw HTML surfaces; only note field-value positions get structured refs.
- **Allow arbitrary mapping-position `!include`**: rejected. It would make include resolution context-sensitive across the whole schema and would complicate formatting, diagnostics, and writeback. Only top-level `media:` is approved initially.
- **Block `media:` includes until all include-preserving formatting is redesigned**: rejected as a design outcome but accepted as an implementation sequencing option. The ADR designs the feature, while recommending that implementation be severed if it grows beyond the `!image` work.

## Implications

- Canonical YAML parsing and emission need tag-aware field value handling for `!image` scalars and non-empty sequences of `!image` scalars. Other tags in those positions remain invalid unless separately specified.
- The include resolver must no longer reject `!image` tags merely because a file also contains `!include`; it should pass `!image` through to canonical YAML parsing.
- Compose must preserve, replace, and remove structured image field data analogously to `field_messages`, including overlay field changes, `field_additions`, and `field_fills`.
- `render_variables()` becomes the single lowering point for both structured messages and structured image fields before adapter export; for images, it resolves media stable IDs to declaration paths before rendering raw HTML.
- CrowdAnki import should prefer structured images only for strict whole-field image HTML whose paths uniquely map to declared media IDs, and keep all other field HTML unchanged.
- Media verification must check structured image references by media ID existence and continue checking raw HTML by regex-extracted path references.
- `media: !include media.yaml` should be implemented as a separate structural-include slice if it conflicts with the scalar include-preservation machinery or media hash writeback.
