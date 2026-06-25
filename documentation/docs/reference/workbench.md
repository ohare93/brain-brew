---
title: Deck Workbench
---

# Deck Workbench

The Deck Workbench is launched locally:

```bash
brainbrew workbench serve --manifest brainbrew.yaml
```

The server binds `127.0.0.1` on an available port by default and serves JSON APIs plus browser UI assets. Release builds embed the checked-in Trunk output from `crates/brain-brew-cli/assets/workbench` in the `brainbrew` binary. For frontend development, build or watch the Iced/WASM UI into a server-readable directory:

```bash
devenv shell workbench-ui-build
devenv shell workbench-ui-watch
brainbrew workbench serve --manifest brainbrew.yaml --dev-assets target/workbench-ui --no-open
# Optional when media files live outside the manifest root:
brainbrew workbench serve --manifest brainbrew.yaml --media-root media/
```

Refresh release-embedded assets after frontend changes:

```bash
devenv shell workbench-ui-embed
```

The Iced/WASM source lives in `crates/brain-brew-workbench-ui`. It builds with Trunk for `wasm32-unknown-unknown`, renders the initial sidebar/canvas/inspector app shell, and fetches workspace metadata from `/api/workspace`.

The target Workbench interaction model is documented in [ADR-015: Use Lazy Single-Work-Item Workbench Editing](decisions/0015-use-lazy-single-work-item-workbench-editing.md). In short: pivots should be compact paginated navigation lists, editing should happen in one selected item detail pane at a time, multilingual context should be lazy and selected-item scoped, and browser-local staged edits remain unapplied until explicit Apply.

Validation commands for this scaffold:

```bash
devenv shell cargo test -p brain-brew-workbench-ui
devenv shell workbench-ui-build
devenv shell test
devenv shell clippy
```

Browser E2E tests are required for user-visible workbench slices:

```bash
devenv shell e2e
```

The E2E harness lives in `crates/brain-brew-workbench-e2e`, uses Rust `thirtyfour` with devenv-provided Chromium/chromedriver, and writes failure screenshots/logs under `target/workbench-e2e-artifacts` by default. `devenv shell ci` includes this E2E gate; `devenv shell test` remains the faster non-browser Rust suite.

## Note pivot editing slice

The current note pivot supports target-translation edits and constrained source note-field edits:

- `GET /api/workbench/note-pivot` returns the selected target language, target, translation overlay, main note-field progress, note rows, field statuses, occurrence counts, source edit controls, and near-Anki source/target previews.
- `POST /api/workbench/apply-preview` accepts browser-local staged edits and returns changed entries, affected source/overlay files, and validation results without writing YAML.
- `POST /api/workbench/apply` repeats validation, applies source edits first, then applies target translation edits against the updated source state.
- Target translation edits rewrite the selected Translation Overlay as canonical YAML.
- Source note-field edits rewrite the Canonical Deck File, except `!include`-backed scalar fields rewrite the included file and keep the include reference intact.
- Repeated source text edits default to the current field only. The browser UI shows the occurrence count and offers an all-occurrences scope.
- Source edits default affected translations to Stale Translation Records. Maintainers can instead migrate the existing translation key to the new source while preserving target text.
- Browser-local staged edits are stored in localStorage until Apply, so refresh keeps unsaved source and target edits while canonical YAML remains unchanged.

## Source String pivot

The Source String pivot complements the note-first view for reusable translation work:

- `GET /api/workbench/source-string-pivot` groups translatable note-field and structured-message component text by source string.
- Each source string reports status, occurrence count, completion counts, content-group badges, and how many occurrences a direct reusable translation will affect.
- Selecting a source string shows every note/field occurrence with friendly context, field-level paths, content-group badges, and source/target previews.
- Direct reusable translations are the default. Global no-change is staged as `translations.no_change`; unchanged exceptions are staged as normal contextual overrides whose target equals the source text.
- Contextual override controls use the selected occurrence's field-level path by default.
- Structured-message source strings are exposed at component/format paths first; whole-field contextual overrides remain an advanced/manual action.
- Source String staged edits share the same browser-local localStorage and Apply preview/confirmation workflow as the Note pivot.

## Card pivot

The Card pivot reviews produced cards one at a time, where a card is a Note rendered through one Card Template:

- `GET /api/workbench/card-pivot` iterates produced cards for the selected language/target/overlay.
- Card rows can be filtered by all/missing/stale/needs-work and content-group badges derived from note type and tags.
- The selected card returns source and target front/back near-Anki previews using the existing Workbench renderer for fields, source variables, basic conditionals, styling, and declared media paths.
- Field inspectors expose the underlying note-field source and translation rows used by that card. Browser edits can stage target translation edits or source note-field edits and use the shared Apply preview/confirmation APIs.
- The Card pivot does not introduce card-specific storage; writes still update canonical translation/source YAML through Workbench Apply.

## Metadata checklist

Metadata review is separate from the main note-field completion metric:

- `GET /api/workbench/metadata` returns main note-field progress plus metadata progress for the selected language/target/overlay.
- Metadata rows use the configured `translation_profile.metadata_categories`; category keys are stable API values and labels are display text. `metadata_exclude_paths` can hide identity data such as adapter IDs even when broad category globs would otherwise match.
- The browser renders a Metadata checklist with status/warning text. Stale metadata is shown as a warning and does not increase the main note-field stale count.
- Metadata edits stage browser-local translation edits and use the same Apply preview/confirmation workflow as Note, Source String, Card, and comparison panes.

## Comparison language panes

Comparison panes let maintainers review another target language beside the active pivot without changing canonical storage rules:

- `GET /api/workbench/comparison-pane` returns Note, Source String, and Card pivot summaries for one comparison language/target/overlay selection.
- The browser pane shows target text, translation status, content-group context, Source String rows, and Card preview context for that comparison language.
- Comparison panes can stay read-only or be toggled writable. Writable comparison edits stage translation entries with that pane's own language/target/overlay scope, so editing one language does not affect another unless those edits are explicitly staged and applied.

## Flexible pane layouts

Pane layout controls turn read/write status into a workflow preset instead of a product limit:

- The default translator preset keeps the source pane read-only and the selected target pane writable.
- Maintainers can toggle the source pane writable for existing note-field source edits.
- Maintainers can load an additional target-language pane and toggle that target pane writable independently.
- Target panes stage translation-dictionary edits with their own language/target/overlay scope, while source panes stage Canonical Deck File edits.
- Apply preview collects all visible pane scopes, groups changes by affected file and content group, and can include the Canonical Deck File plus multiple Translation Overlay files in one confirmation.
- Apply still writes only after explicit confirmation; browser-local staged edits remain in localStorage until applied.

## New language scaffolding

The Workbench can scaffold a new target language from an existing target-language template:

- `GET /api/workbench/new-language-preview` derives an editable draft without writing files.
- `POST /api/workbench/new-language` writes the confirmed manifest changes and new translation overlay files.
- Defaults follow workspace conventions: `overlay.translation.<code>`, `overlays/languages/<code>.yaml`, target IDs `<code>-<target-label>`, and copied language target labels from the template.
- Every template `translation_overlays` group is selected in the preview by default; maintainers can deselect groups or edit generated overlay IDs, file paths, and target IDs before creation.
- New translation overlay files start with an empty `translations: {}` dictionary so progress reports all source strings as missing until reviewed.
- The server re-reads `brainbrew.yaml` after writes, so `/api/workspace`, language selectors, and Note/Source String/Card pivots can load the new language immediately.
