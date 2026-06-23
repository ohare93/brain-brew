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
```

Refresh release-embedded assets after frontend changes:

```bash
devenv shell workbench-ui-embed
```

The Iced/WASM source lives in `crates/brain-brew-workbench-ui`. It builds with Trunk for `wasm32-unknown-unknown`, renders the initial sidebar/canvas/inspector app shell, and fetches workspace metadata from `/api/workspace`.

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

New language scaffolding and richer card pivots are later slices.
