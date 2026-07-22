---
title: Translation overlays
---

# Translation overlays

A translation overlay changes deck language or localized text. It should not add unrelated extension content.

Translation overlays separate faithful translations from intentional target-language adaptations:

| Section | Use it for | Key shape |
| --- | --- | --- |
| `translations.direct` | reusable faithful translations of exact non-empty source strings | `source text: target text` |
| `translations.contextual` | faithful translations scoped to a stable deck context | `context path -> source text: target text` |
| `translations.no_change` | translator-reviewed text whose faithful target is identical to source | `[source text]` |
| `target_adaptations` | explicit path-scoped target-language adaptation or deletion | `stable deck path -> intent, ownership, expected_source, reason, target (adapt only)` |
| `stale_translations` | review debt for source text that changed while reusing the prior target text temporarily | list of `old_source`, `new_source`, `target`, optional `context` |

Contextual translations win over direct translations. When multiple contextual scopes match, the longest matching context path wins. No-change entries cover missing translation checks but never modify composed output. Target adaptations apply only at their exact path and only when `expected_source` still matches. Stale translations apply their `target` text to `new_source` while reporting a stale-review warning until resolved.

The source key in `direct`, `contextual`, and `no_change` is the expected base. If the English/source text changes, composition fails with a stale source-key error instead of silently applying the wrong translation or no-change decision. Use `stale_translations` when a source maintainer intentionally carries the old target text forward as explicit review debt.

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

Use `translations.contextual` when a source string needs a faithful translation only inside a stable deck context, or when the same English string needs different faithful target text in different places.

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

## Explicit no-change entries

Use `translations.no_change` when a translator has reviewed source text and intentionally left it identical in the target language. This is different from a missing translation: future source additions will still be reported until they are either translated or explicitly marked no-change.

```yaml
translations:
  direct:
    Germany: Deutschland
  no_change:
    - Andorra
    - Canada
    - Djibouti
```

Use `no_change` for names or phrases that should stay identical everywhere they appear. If a string usually stays unchanged but needs translation in one context, add it to `no_change` and add a `contextual` translation for the exception. If a string usually translates but stays unchanged in one context, use a contextual translation whose target equals the source.

Keep `ignore_paths` for structural paths that translators should not review at all, such as tags, flag/map HTML, or adapter metadata. Use `no_change` for translator-facing text that was reviewed and intentionally left as-is.

## Structured messages for composite fields

Use a structured field message when a field value is genuinely assembled from reusable translated pieces, such as a list of country names plus small qualifier fragments. Do not use it for every ordinary sentence; plain scalar strings are easier to read and translate when the whole sentence is the natural translation unit.

Base source:

```yaml
notes:
  note.finland:
    fields:
      field.flag-similarity:
        format: '{country_1} ({description_1}), {country_2} ({description_2})'
        variables:
          country_1:
            ref: notes.note.iceland.fields.field.country
          description_1:
            text: blue background with a white cross
          country_2:
            ref: notes.note.norway.fields.field.country
          description_2:
            text: red background with a blue cross
```

Message variables mean:

- `format` is inline at the usage site and renders `{variable}` placeholders. If it lacks a translation, coverage records it as an `UntranslatedFallback`; under the default lenient policy that is reported without failing the build, while `translation_coverage: strict` fails the check. Translate `format` directly or contextually when a target language needs different glue, ordering, spacing, or separators;
- `ref` points at another note field and can reuse that field's existing translation, such as a country-name entry in `translations.direct`;
- `text` is an editable translatable fragment and appears in translation coverage at a named component path such as `notes.note.finland.fields.field.flag-similarity.message.variables.description_1`;
- `literal` can still be used for a named non-translatable variable, but punctuation and separators usually belong in `format`.

Translation overlay:

```yaml
translations:
  direct:
    Iceland: Island
    Norway: Norge
    red background with a blue cross: rød bakgrunn med blått kors
  contextual:
    notes.note.finland.fields.field.flag-similarity.message.variables.description_1:
      blue background with a white cross: blå bakgrunn med hvitt kors
```

If the target language needs different glue for this field shape, translate the format string instead of overriding the whole rendered field:

```yaml
translations:
  direct:
    '{country_1} ({description_1}), {country_2} ({description_2})': '{country_1}({description_1})、{country_2}({description_2})'
```

The resolved field exported to Anki is still a plain string:

```text
Island (blå bakgrunn med hvitt kors), Norge (rød bakgrunn med blått kors)
```

Strict coverage reports missing and stale entries for the `format` glue and for each `text` or `ref` component instead of requiring one long key for the whole composite field. The translator context view shows the resolved message plus its components so translators can edit the glue, reusable country names, and qualifier fragments separately. If a target language needs a special whole-field wording, add a contextual translation for the full resolved source string at the field or note context; that full override replaces the component-composed output for that target.

Translation coverage and application resolve source fields through the same validated dependency graph used by export. Reusable direct/no-change translations stay attached to the referenced upstream field, so a later overlay replacement propagates through downstream messages. A consuming-path contextual adaptation intentionally materializes that component as a literal when its target differs from the dependency's translated target, including a source-equal contextual decision that overrides a conflicting reusable direct translation. Missing references and cycles are errors rather than empty fallback text.

Coordinate with deck maintainers before migrating existing large fields: structured messages are best for repeated, composite source text where component reuse clearly reduces duplication.

### Translate reusable list message patterns

A field-level list message pattern keeps repeated glue out of each note. Translate its shared `item_format` and `separator` at the field-definition context. Nested contextual mappings group the common stable path without changing the flattened matching semantics:

```yaml
translations:
  contextual:
    note_types.note-type.ultimate-geography.fields.field.flag-similarity.message_pattern:
      item_format:
        '{country} ({description})': '{country}（{description}）'
      separator:
        ', ': '、'
```

Each `text` item parameter remains a translatable unit at a stable path such as:

```text
notes.note.yemen.fields.field.flag-similarity.message.items.0.description
```

A `note_field_ref` parameter reuses the referenced field's normal translation. A consuming-path contextual translation may intentionally specialize that one occurrence. It is materialized even when its target equals the source, so an explicit decision such as `Puerto Rico: Puerto Rico` at `notes.note.cuba.fields.field.flag-similarity.message.items.0.country` wins over a reusable direct `Puerto Rico: Portoriko` translation.

When a target composition does not contain the referenced note, a `note_field_ref` argument can instead carry explicit translatable text without creating a graph dependency:

```yaml
field.flag-similarity:
  - country:
      text: Sierra Leone
    description: slightly lighter blue
```

The explicit country text and the scalar description both appear at their consuming item paths in translation coverage. A genuine note-specific list-format exception can override the source separator without copying the item format or whole rendered field:

```yaml
translations:
  contextual:
    notes.note.yemen.fields.field.flag-similarity.message:
      separator:
        ', ': ', e '
```

Shared glue appears once in translation coverage; item text and references retain their consuming note/item/parameter context. Existing inline structured messages remain compatible and should be used for irregular composites rather than forcing them into a list pattern.

## Stale translations

Use `stale_translations` when source text changed and the previous target text should keep translated decks usable until a translator reviews it.

```yaml
stale_translations:
  - old_source: Autonomous community of Spain.
    new_source: Autonomous region of Spain.
    target: Selvstyrende region af Spanien.
    context: notes.note.canary-islands.fields.field.country-info
```

A stale translation with no `context` acts like a direct translation for `new_source`. A contextual stale translation applies only at or below its context path, using the same context matching rules as `translations.contextual`. `brainbrew compose`, `brainbrew export crowdanki`, and lenient/default `brainbrew verify` emit stale warnings but still apply the target text. A strict translation coverage policy fails while stale translations remain.

Resolving a stale translation usually moves it into a normal translation entry for `new_source` (`direct` when contextless, `contextual` when context is present) and removes the stale translation. If an existing direct, no-change, or matching contextual entry already shadows that stale record, resolving only deletes the superseded stale record; it does not create or update another translation entry.

## Path-scoped adaptations and deletions

Use top-level `target_adaptations` only for a target-language decision that intentionally diverges from, supplements, or removes source wording at one exact stable deck path. Every record is typed and reviewable: `intent`, `ownership`, `expected_source`, and a non-blank `reason` are required. Translation overlays must declare `ownership: translation`; extension-owned blank content belongs in [`field_fills`](field-fills.md), not in this section.

```yaml
target_adaptations:
  notes.note.taiwan.fields.field.country-info:
    intent: adapt
    ownership: translation
    expected_source: Partially recognised state claimed by China.
    target: 中国宣称对台湾拥有主权，但仅被部分国家承认
    reason: target-language geopolitical wording
  notes.note.united-kingdom.fields.field.country-info:
    intent: adapt
    ownership: translation
    expected_source: ''
    target: Offiziell das Vereinigte Königreich Großbritannien und Nordirland.
    reason: target-language explanatory addition
  notes.note.example.fields.field.private-note:
    intent: delete
    ownership: translation
    expected_source: Source-only learner instruction.
    reason: not appropriate in this target-language edition
```

`adapt` requires non-blank `target` text and may deliberately add localized text when `expected_source` is blank. `delete` requires a non-blank `expected_source` and has no `target`; it cannot act as a global source-key fallback. Direct, contextual, and variable translations always reject blank targets, so a blank value can never silently erase every occurrence of a source string.

The current source value must equal `expected_source`. If it does not, composition rejects the record so maintainers can review whether the decision still applies. Coverage reports adaptations as `target_adaptation` and deletions as `target_deletion`; final stack coverage retains a deleted entry with its source owner and resolving overlay.

Legacy `translations.target_additions` and untyped adaptation maps remain reader-compatible only. `brainbrew fmt <overlay.yaml>` writes the canonical typed form and the codec exposes a migration diagnostic telling maintainers to review the generated reason.

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

The interactive workflow is a real arrow-key terminal UI: use ↑/↓ to move, Enter to select, Space to toggle rows during selective apply, and `q` to cancel. It lets you choose from the known manifests, targets, notes, fields, path prefixes, and report/apply mode. Language and overlay selectors appear only when they disambiguate the selected targets; for example, after choosing `da-standard`, Brain Brew already knows the Danish translation overlay. It prints the equivalent non-interactive command before running so the same review can be repeated in CI or shared with another maintainer.

For scripts and CI, pass the scope explicitly:

```bash
brainbrew translations --manifest brainbrew.yaml --target de-standard
brainbrew translations --manifest brainbrew.yaml --all-targets --language de
brainbrew translations --manifest brainbrew.yaml --target de-standard --note note.berlin
brainbrew translations --manifest brainbrew.yaml --target de-standard --path-prefix notes.note.berlin.fields.field.country
brainbrew translations --manifest brainbrew.yaml --target de-standard --context --source Georgia
brainbrew translations --manifest brainbrew.yaml --target de-standard --context --duplicates
brainbrew translations --manifest brainbrew.yaml --target de-standard --context --status missing
brainbrew translations --manifest brainbrew.yaml --target de-standard --json
brainbrew translations --manifest brainbrew.yaml --all-targets --summary --json
brainbrew translations --manifest brainbrew.yaml --target de-standard --full
```

Report mode is the default and never modifies files. The human report is translator-focused by default: it shows missing note-field text translations and summarizes structural/media/tag values separately so flag HTML, map HTML, tags, deck metadata, and template names do not drown out translator work. For language-first tools, `brainbrew.yaml` can declare `languages` and `translation_profile` metadata as described in [Manifests and targets](manifests-targets.md). Use `--full` when you intentionally want every scalar fallback. JSON output remains stable and includes all coverage entries. Every successful JSON mode has this versioned, regression-tested envelope; only the `kind` payload changes:

```json
{
  "schema_version": 1,
  "kind": "translation_report",
  "reports": [],
  "applied": {}
}
```

`kind` is `translation_report`, `translation_context`, `translation_summary`, or `stale_resolution`. The report payload contains `reports` and `applied`; each report has deterministic category totals and ordered entries. Errors use the versioned `{ "error": ... }` envelope described in the [CLI reference](../reference/cli.md#exit-and-error-contract), so `translations --json` never mixes prose with JSON. Use `--context` for the first translator-in-context terminal view: it shows source English and target text together with translation status, note id, field id/name, note type, and card templates where the field appears. Filter or navigate the context view with `--language`, `--note`, `--field`, `--source`, `--duplicates`, `--status missing|stale|translated|direct|contextual|no-change`, and `--path-prefix`. Repeated source strings are shown as duplicate source groups so translators can see whether a reusable `direct`/`no_change` entry is enough or a path-specific `contextual` override is needed. YAML remains the canonical storage format; the terminal context view is the first ergonomic translator interface over that canonical data, and `--context --json` exposes the same note/field/card context model intended for the Deck Workbench. Use `--summary` for compact per-language/per-overlay counts; summary mode de-duplicates identical reports across target variants and includes direct translations, contextual translations, no-change entries, target adaptations, variables, adapter IDs, raw untranslated fallbacks, actionable missing text translations, ignored entries, and stale/invalid keys. Human summary output uses narrow aligned columns by default; add `--summary --full` to include overlay/file columns, or `--summary --json` for complete machine-readable metadata. Missing fallbacks are source strings that would currently pass through unchanged in a translated target.

To seed translator work after adding English notes or fields, run apply explicitly:

```bash
brainbrew translations --manifest brainbrew.yaml --target de-standard --apply
```

Non-interactive `--apply` preserves the existing scriptable behavior: it inserts deterministic `source: source` translation stubs into `translations.direct` for the missing text fallbacks in scope. Interactive apply is selective: toggle rows with Space, confirm with Enter, then choose one action for all selected rows — mark `no_change`, add direct `source: source`, add contextual `source: source`, add `ignore_paths`, skip — or choose to decide per row. Context viewing does not introduce a second mutation path: combine context filters with `--apply --interactive` when you want to apply changes, and Brain Brew routes them through the same typed source-document operation. It keeps reviewed no-change decisions distinct from real translations and does not invent target adaptations for blank source fields.

Translation apply and stale-resolution writes canonically rewrite each affected overlay, so YAML comments and hand layout are not retained; comments are not part of the durable Canonicalized Source model. Scalar `!include` directives are retained. Brain Brew builds and validates every affected overlay and final target composition before writing, then commits all changed files through one fingerprint-checked, journaled workspace transaction. An interrupted transaction is recovered before the next mutating translation command.

When `require_complete: true`, composition fails if any extracted non-empty translatable string is not translated by `direct`, translated by a matching `contextual` entry, marked by `no_change`, or matched by `ignore_paths`. For release workflows, prefer target-level verification policy in `brainbrew.yaml`:

```yaml
targets:
  de-standard:
    overlays:
      - overlay.translation.de
    translation_coverage: strict
```

`translation_coverage: lenient` is the default and allows untranslated fallbacks during development. `translation_coverage: strict` makes `brainbrew verify` fail when a translation overlay leaves missing fallbacks. The CLI flag `brainbrew verify --translation-coverage strict` can override target configuration for one run.

Translator context views should present extracted strings in the same categories:

- source strings that occur once or can be safely reused are candidates for `direct` or `no_change`;
- repeated source strings should show their stable deck contexts so translators can choose a reusable `direct` translation, reusable `no_change`, contextual entries, or both;
- blank source fields should be shown as target adaptation opportunities and written to `target_adaptations` only when the blank text belongs to the translation overlay.

During sync/apply, stale source-key errors mean the translator should refresh against the current source deck before editing the target text. Missing direct/contextual translation errors indicate an untranslated extracted string. Invalid contextual errors indicate either a stale source key or an invalid context path. Invalid target-adaptation errors indicate the source no longer matches the adaptation's `expected_source`.

## Deterministic section order

The formatter emits translation dictionary sections in this order:

1. `require_complete`
2. `ignore_paths`
3. `direct`
4. `contextual`
5. `no_change`
6. `variables`
7. `adapter_ids`

Top-level `target_adaptations` and `stale_translations` are emitted after the `translations` dictionary. A file with no `direct` section starts at the next non-empty section. That is still deterministic.
