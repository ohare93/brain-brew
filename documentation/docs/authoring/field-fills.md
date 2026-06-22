---
title: Field fills
---

# Field fills

`field_fills` is an overlay shorthand for filling existing blank note fields.

Use it when content belongs to an extension or patch, not to a translation dictionary.

## Example

```yaml
id: overlay.extension.hardcore.field-fills.en
kind: extension
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
  note.canary-islands:
    field.capital: Santa Cruz de Tenerife, Las Palmas
    field.capital-info: The capital is shared between the two cities of Santa Cruz de Tenerife and Las Palmas.
```

This lowers to explicit checked changes:

```yaml
notes:
  note.anguilla:
    intent: merge
    fields:
      field.capital:
        intent: replace
        value: The Valley
        expected_base:
          value: ''
```

If upstream later fills `field.capital`, composition fails instead of overwriting it.

## When to use it

Use `field_fills` for:

- adding extension-owned content to blank fields on existing notes;
- preserving a non-destructive “only if blank” policy;
- language-specific extension content such as Hardcore Geography's filled capitals/flags.

Do not use it for:

- adding new field definitions — use [`field_additions`](extensions.md#add-fields-and-values);
- translating non-blank source text — use [`translations.direct`](translations.md#direct-translations) or [`translations.contextual`](translations.md#contextual-translations);
- adding new notes — use `notes` with `intent: add`.

## Why not `translations.target_additions`?

A path-indexed value is not automatically a translation.

`translations.target_additions` says “this blank localized text belongs to a translation overlay.”

`field_fills` says “this extension or patch fills a blank field with new content.”

Keeping them separate makes English extension content possible without inventing an English translation overlay.
