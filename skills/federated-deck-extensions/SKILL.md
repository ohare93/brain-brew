---
name: federated-deck-extensions
description: Use when creating, reviewing, or refactoring Brain Brew Federated Deck workspaces, especially translation overlays, extension overlays, variant targets, or Ultimate Geography-style decks. Enforces variable-first, shared-extension design so agents do not duplicate per-language templates or encode rendered values where source variables should be used.
---

# Federated Deck Extensions

Use this skill whenever you touch a Brain Brew `deck.yaml`, `brainbrew.yaml`, `overlays/languages/*.yaml`, `overlays/variants/**/*.yaml`, or any UG-style Federated Deck workspace.

The goal is to keep source declarative, maintainable, and parity-safe:

- base deck owns shared structure;
- translation overlays translate dictionaries and variables;
- shared extension overlays add new structure once;
- language-specific extension overlays contain only the genuinely language/adapter-specific residue.

## Required context pass

Before changing design, read or inspect:

1. `CONTEXT.md` for project language.
2. `documentation/docs/authoring/workspace.md`, `documentation/docs/authoring/manifests-targets.md`, and `documentation/docs/concepts/overlays.md` for manifest, variable, and overlay syntax.
3. The active ADR index in `documentation/docs/reference/decisions/README.md` when making architectural changes.
4. The current workspace files:
   - `brainbrew.yaml`
   - `deck.yaml`
   - `overlays/languages/*.yaml`
   - `overlays/variants/**/*.yaml`

Useful inspection commands:

```bash
rg -n 'variables:|\$\{|translations:|card_templates:|note_types:|depends_on:' deck.yaml overlays brainbrew.yaml
brainbrew targets --manifest brainbrew.yaml
brainbrew explain --manifest brainbrew.yaml --target <target>
```

## Design rules

### 1. Variables before duplicated template text

If text appears in card templates, note type names, field labels, repeated descriptions, or extension templates, first ask: **should this be a source variable?**

Prefer:

```yaml
name: '${note-type.name}${variant.name-suffix}'
variables:
  note-type.name: Ultimate Geography
  variant.name-suffix: ''
  label.flag: Flag
  label.location: Location
```

and templates like:

```html
<div class="type">${label.flag}</div>
{{#Flag similarity}}<div class="info">${sentence.flag-similar}</div>{{/Flag similarity}}
<div class="type">${label.location}</div>
```

Avoid per-language copies of the same HTML just to replace `Flag`, `Location`, or a repeated phrase.

### 2. Translation overlays translate variables, not paths, for shared wording

For repeated labels/model names, use `translations.variables`:

```yaml
translations:
  variables:
    note-type.name:
      Ultimate Geography: 'Ultimate Geography [DA]'
    label.flag:
      Flag: Flag
    label.location:
      Location: Placering
    sentence.flag-similar:
      'Flag similar to {{Flag similarity}}.': 'Flaget ligner {{Flag similarity}}.'
```

Use `translations.changes` for content text and path-scoped exceptions. Do **not** path-scope metadata like `note_types.note-type.ultimate-geography.name` if a variable can express it.

### 3. Use field fills for non-translation blank content

If an extension fills fields that already exist but are blank on base notes, use `field_fills` in an extension or patch overlay. Do not put extension content under `translations.additions` just because it is path-indexed.

Prefer:

```yaml
id: overlay.extension.hardcore.field-fills.en
kind: extension
field_fills:
  note.anguilla:
    field.capital: The Valley
    field.flag: '<img src="ug-flag-anguilla.svg" />'
```

Reserve `translations.additions` for blank localized text that is genuinely part of a translation overlay.

### 4. Shared extension overlay first, language-specific residue later

For Standard/Extended-style variants, create one shared extension overlay for structural additions:

```text
overlays/variants/extended.yaml
```

It should add card templates/fields once and use variables for localized labels.

Per-language files should be small:

```text
overlays/variants/extended/da.yaml
```

These files should usually contain only adapter identity preservation, deck metadata exceptions, or genuinely language-specific residue. They should **not** copy card-template HTML.

### 5. Manifest dependency order matters

For a localized extended target, wire dependencies so composition is deterministic and adapter IDs see the expected base:

```yaml
overlays:
  overlay.variant.extended:
    file: overlays/variants/extended.yaml
    kind: extension

  overlay.variant.extended.da:
    file: overlays/variants/extended/da.yaml
    kind: extension
    depends_on:
      - overlay.translation.da
      - overlay.variant.extended
```

That expands to translation first, shared extension second, language-specific extension last.

### 6. Remember: variables render at export, not during compose

Overlay `expected_base` checks run against source values before CrowdAnki export renders variables. If a source name is:

```yaml
name: '${note-type.name}${variant.name-suffix}'
```

then a later overlay must not expect the rendered value `Ultimate Geography [DA]` at `note_types...name`.

Use a variable change instead:

```yaml
variables:
  variant.name-suffix:
    intent: replace
    value: ' [Extended]'
    expected_base:
      value: ''
```

This avoids the mistake of comparing rendered output against unrendered source.

## Red flags to stop and refactor

Stop if you see any of these:

- `overlays/variants/extended/<lang>.yaml` contains full `card_templates:` blocks for every language.
- Template HTML differs only by labels such as `Flag`, `Location`, or `Flag similar...`.
- Translation overlays include `notes:` blocks for ordinary field translations.
- Translation overlays use `translations.additions` for extension-owned field content instead of `field_fills`.
- Translation overlays path-scope note-type names instead of translating a variable.
- `expected_base` refers to the rendered value of a variable-backed source property.
- The same extension template exists in more than one language file.
- An overlay changes adapter IDs and structure in the same large block when those can be separated into shared structure plus per-language identity residue.

## Safe refactoring workflow

Use TDD/RGR for behavior changes. For fixture/source refactors, make the guard explicit first.

1. Add or strengthen a test that catches the mistake:
   - translation overlays do not use per-note field replacement blocks;
   - translation overlays translate note-type/model names through variables;
   - per-language extension overlays do not contain copied card-template HTML;
   - manifest targets still compose/export.
2. Refactor source:
   - add variables to `deck.yaml`;
   - move shared extension templates to a shared overlay;
   - replace hardcoded template text with `${...}` references;
   - shrink per-language extension overlays;
   - update `brainbrew.yaml` dependencies.
3. Format all changed source:

```bash
brainbrew fmt deck.yaml
brainbrew fmt brainbrew.yaml
find overlays -name '*.yaml' -print0 | xargs -0 -n1 brainbrew fmt
```

4. Verify composition/export:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
```

5. For UG parity, compare against the configured Brain Brew goldens. A passing verify should end with:

```text
✓ verified <target-count> targets
  manifest: brainbrew.yaml
```

## Review checklist

Before finishing, answer yes to all:

- Are repeated template labels represented as variables?
- Are repeated model/note-type names represented as variables plus suffixes where needed?
- Are translation overlays mostly dictionaries and variable translations?
- Is there one shared extension overlay for shared card-template/field additions?
- Are language-specific extension overlays small and free of copied template HTML?
- Does manifest dependency expansion produce the intended order?
- Did `brainbrew verify --manifest brainbrew.yaml --all-targets` pass?
