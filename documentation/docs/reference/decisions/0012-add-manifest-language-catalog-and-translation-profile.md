# ADR-012: Add Manifest Language Catalog and Translation Profile

**Date**: 2026-06-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Translation work should be language-first: translators choose a source or target language, see progress, select a primary target for previews, switch among target labels such as standard or extended, and create new languages without hand-editing every manifest target. Existing manifests have overlays and build targets, but no explicit language catalog or translation profile.

Naming conventions such as `da-standard` are useful defaults, but relying on them alone would make the Deck Workbench infer too much from package-specific target names.

## Decision

Brain Brew manifests will gain top-level `languages:` metadata and a top-level `translation_profile:`.

`languages:` is a map keyed by language code. Each entry includes `display_name`, `source`, `primary_target`, and `targets`, where `targets` maps friendly target labels to Build Target IDs. Source language entries set `source: true` and omit `translation_overlays`. Target language entries name their `translation_overlays` as a labeled map because one language may translate the base deck plus extension-specific content overlays.

Example:

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

`translation_profile:` classifies translation work for UI progress and review. The shape includes structural note fields, configured metadata categories, metadata path globs, metadata exclusions for broad globs, and metadata category ordering:

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

Main language completion counts non-structural note field text. Metadata such as deck description, note type names, field labels, and card templates appears in a separate metadata checklist and does not block main completion.

## Rationale

**Pros:**

- Makes language-first workflows explicit and reviewable.
- Avoids hard-coding target naming conventions into the workbench.
- Supports source-language entries without pretending the source language is a Translation Overlay.
- Lets new-language scaffolding mirror an existing/template language while previewing IDs and paths before writing.
- Handles packages where multiple extension/content overlays have separate translations for the same target language.
- Keeps translator-facing progress focused on deck content instead of structural/media/template noise.

**Cons:**

- Extends the manifest schema.
- Existing packages need metadata before the workbench can provide first-class language navigation.
- Progress semantics become distinct from strict raw translation coverage unless documented carefully.

## Alternatives Considered

- **Infer languages and primary targets from target names**: rejected because package naming conventions are not reliable enough for a durable UI.
- **Put all metadata under `workbench:`**: rejected because languages are broader than one UI and should be usable by CLI/reporting workflows too.
- **Use `variants:` inside language entries**: rejected because “Variant” is package/UG terminology; Brain Brew’s formal concrete composition concept is **Build Target**.
- **Classify structural fields in every Translation Overlay**: rejected because field roles are source/workspace facts, not per-language translation decisions.

## Implications

- Manifest parsing/formatting must preserve the new metadata deterministically.
- Workbench language dashboards should use `languages:` instead of target-name inference.
- New-language creation should default to a labeled base translation overlay such as `overlay.translation.<code>`, `overlays/languages/<code>.yaml`, extension-specific translation overlays that mirror the selected template language, and target IDs based on the selected template, with an editable preview before writing.
- Verification/reporting may expose both main deck-content completion and metadata status.
