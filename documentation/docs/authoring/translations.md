---
title: Translation overlays
---

# Translation overlays

A translation overlay changes deck language or localized text. It should not add unrelated extension content.

Translation dictionaries separate source-keyed translations from target-only text:

| Section | Use it for | Key shape |
| --- | --- | --- |
| `direct` | reusable translations of exact non-empty source strings | `source text: target text` |
| `contextual` | path-scoped translations for a source string inside a deck context | `context path -> source text: target text` |
| `target_additions` | target-language text for fields intentionally blank in the source deck | `stable deck path: target text` |

Contextual translations win over direct translations. When multiple contextual scopes match, the longest matching context path wins.

The source key in `direct` and `contextual` is the expected base. If the English/source text changes, composition fails with a stale source-key error instead of silently applying the wrong translation.

## Direct translations

Use `translations.direct` for ordinary country/capital names and other reusable source strings that should receive the same translation everywhere they appear.

```yaml
id: overlay.translation.de
kind: translation
translations:
  direct:
    Germany: Deutschland
    Austria: Österreich
    Vienna: Wien
```

If `Germany` no longer appears in extracted translatable text, composition reports a stale direct translation source.

## Contextual translations

Use `translations.contextual` when a source string needs a translation only inside a stable deck context, or when the same English string needs different target text in different places.

```yaml
translations:
  direct:
    Georgia: Georgien
  contextual:
    notes.note:
      georgia:
        Georgia: Georgien
      us-georgia:
        Georgia: Georgia
      us-georgia.fields.field.region:
        Georgia: US-Bundesstaat Georgia
    deck.description:
      Georgia: Georgien-Hinweis im Beschreibungstext
```

This means:

- under `notes.note.georgia`, translate `Georgia` as `Georgien`;
- under `notes.note.us-georgia`, translate `Georgia` as `Georgia`;
- at the more specific `notes.note.us-georgia.fields.field.region`, translate `Georgia` as `US-Bundesstaat Georgia`;
- under `deck.description`, translate `Georgia` with description-specific wording.

The nested shape is an ergonomic way to avoid repeating full stable paths. It flattens to context paths such as `notes.note.georgia` and `notes.note.us-georgia.fields.field.region`.

A context applies to any extracted string at that path or below it. A note-level context such as `notes.note.georgia` applies to every translated field/tag under that note unless a more specific context also matches.

Contextual entries may exist with or without a `direct` fallback. If both exist, contextual wins for matching paths and `direct` remains the fallback elsewhere.

## Target-language additions for blank source fields

Use `translations.target_additions` only when blank localized text genuinely belongs to the translation overlay. This is valid when the source deck intentionally has no English text for the field but a target language should supply text.

```yaml
translations:
  target_additions:
    notes.note.united-kingdom.fields.field.country-info: Offiziell das Vereinigte Königreich Großbritannien und Nordirland.
```

The current source value must be blank. If it is non-empty, composition rejects the entry and points to `direct` or `contextual` instead.

If an extension fills blank fields with new content, use [`field_fills`](field-fills.md) instead.

## Translate source variables

Variables keep card templates shared across languages.

Base source:

```yaml
note_types:
  note-type.country:
    variables:
      label.capital: Capital
      label.location: Location
    card_templates:
      template.map:
        question_format: '<div>${label.location}</div>{{Map}}'
```

Translation overlay:

```yaml
translations:
  variables:
    label.capital:
      Capital: Hauptstadt
    label.location:
      Location: Lage
```

Prefer variable translations over copying whole card templates per language.

## Translate adapter IDs

Legacy translated decks may already have different CrowdAnki GUIDs.

```yaml
translations:
  adapter_ids:
    crowdanki:guid:
      english-guid: german-guid
```

## Complete coverage, translator context, and sync/apply

Use `brainbrew translate` (aliases: `translation`, `translations`) to inspect coverage before editing a translated overlay. In an interactive terminal, running it with missing choices opens a manifest-aware selector:

```bash
brainbrew translate
brainbrew translate --manifest fixtures/ultimate-geography/brainbrew.yaml
```

The interactive workflow lets you choose from the known manifests, targets, languages, translation overlays, notes, fields, path prefixes, and report/apply mode. It prints the equivalent non-interactive command before running so the same review can be repeated in CI or shared with another maintainer.

For scripts and CI, pass the scope explicitly:

```bash
brainbrew translations --manifest brainbrew.yaml --target de-standard
brainbrew translations --manifest brainbrew.yaml --all-targets --language de
brainbrew translations --manifest brainbrew.yaml --target de-standard --note note.berlin
brainbrew translations --manifest brainbrew.yaml --target de-standard --path-prefix notes.note.berlin.fields.field.country
brainbrew translations --manifest brainbrew.yaml --target de-standard --json
```

Report mode is the default and never modifies files. It groups extracted text into direct translations, contextual overrides, target-language additions, ignored entries, stale/invalid keys, and missing/untranslated fallbacks. Human output is colored when terminal color is enabled; JSON output is stable and uncolored. Missing fallbacks are source strings that would currently pass through unchanged in a translated target.

To seed translator work after adding English notes or fields, run apply explicitly:

```bash
brainbrew translations --manifest brainbrew.yaml --target de-standard --apply
```

Non-interactive `--apply` inserts deterministic `source: source` stubs into `translations.direct` for the missing fallbacks in scope. Interactive apply is selective: choose rows, then choose direct `source: source`, contextual `source: source` at the suggested safe context, an `ignore_paths` entry, or skip. It keeps direct reusable translations distinct from `contextual` overrides and does not invent target-language additions for blank source fields. Existing comments and layout are preserved where practical; run `brainbrew fmt overlays/languages/de.yaml` when you want fully canonical formatting.

When `require_complete: true`, composition fails if any extracted non-empty translatable string is not translated by `direct`, translated by a matching `contextual` entry, or matched by `ignore_paths`. For release workflows, prefer target-level verification policy in `brainbrew.yaml`:

```yaml
targets:
  de-standard:
    overlays:
      - overlay.translation.de
    translation_coverage: strict
```

`translation_coverage: lenient` is the default and allows untranslated fallbacks during development. `translation_coverage: strict` makes `brainbrew verify` fail when a translation overlay leaves missing fallbacks. The CLI flag `brainbrew verify --translation-coverage strict` can override target configuration for one run.

Translator context views should present extracted strings in the same categories:

- source strings that occur once or can be safely reused are candidates for `direct`;
- repeated source strings should show their stable deck contexts so translators can choose a reusable `direct` translation, `contextual` entries, or both;
- blank source fields should be shown as target-language addition opportunities and written to `target_additions` only when the blank text belongs to the translation overlay.

During sync/apply, stale source-key errors mean the translator should refresh against the current source deck before editing the target text. Missing direct/contextual translation errors indicate an untranslated extracted string. Invalid contextual errors indicate either a stale source key or an invalid context path. Invalid target-addition errors indicate the source is no longer blank.

## Deterministic section order

The formatter emits translation dictionary sections in this order:

1. `require_complete`
2. `ignore_paths`
3. `direct`
4. `contextual`
5. `target_additions`
6. `variables`
7. `adapter_ids`

A file with no `direct` section starts at the next non-empty section. That is still deterministic.
