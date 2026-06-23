# ADR-013: Use Stale Translation Records for Source Text Changes

**Date**: 2026-06-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Translation Dictionary keys are source text expected bases. When source-language note field text changes, existing target-language decisions keyed by the old source text no longer match, while the new source text appears missing. Some source edits are typo or wording fixes where the existing target text remains acceptable; others require a translator to review or retranslate.

A source-language maintainer may not be able to update every target language immediately. Brain Brew needs a way to keep translated decks usable while making review debt explicit in source.

## Decision

Translation Dictionaries will support persisted **Stale Translation Records** under `translations.stale_records`.

A stale record stores the old source, new source, prior target text, and optional context path when needed:

```yaml
translations:
  stale_records:
    - old_source: Autonomous community of Spain.
      new_source: Autonomous region of Spain.
      target: Selvstyrende region af Spanien.
      context: notes.note.canary-islands.fields.field.country-info
```

During compose/export, a stale record applies `target` to `new_source` but emits a warning that the translation needs review. Stale records do not block the main language completion percentage by default. `brainbrew verify` should warn by default and support an optional strict policy that fails when stale records remain.

When the Deck Workbench applies source-language edits, affected translations default to creating stale records. Maintainers may explicitly choose to migrate an old source key to the new source key while preserving target text when they know the existing translation is still correct. Resolving a stale record creates or updates the normal direct/contextual translation for `new_source` and removes the stale record.

## Rationale

**Pros:**

- Keeps translated decks usable after source edits.
- Preserves review debt in the repo for the appropriate translation maintainer.
- Avoids silent key migration when source meaning may have changed.
- Lets source-language maintainers safely update source text without being responsible for every target language.

**Cons:**

- Extends the Translation Dictionary schema.
- Compose/export can produce output from a known-stale translation unless strict policy is enabled.
- Tooling must surface warnings clearly so stale records are not forgotten.

## Alternatives Considered

- **Transient stale reports only**: rejected because review debt would not be preserved clearly in source after a source edit.
- **Always migrate keys automatically**: rejected because source changes can invalidate target text semantically.
- **Always fail until all translations are updated**: rejected because source maintainers may not know every target language and translated decks should remain usable with warnings.
- **Do not apply stale target text**: rejected because it would regress translated decks to source-language fallback immediately after source edits.

## Implications

- Translation coverage and context reports must include stale records as warning/review items.
- Workbench Apply preview for source edits must show affected target translations and default them to stale records.
- Release workflows can opt into strict stale-record failure.
- Documentation must explain the difference between stale records, missing translations, direct translations, contextual overrides, and global no-change decisions.
