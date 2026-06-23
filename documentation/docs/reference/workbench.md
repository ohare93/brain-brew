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

## Note pivot translation slice

The first workbench slice exposes target-translation editing only:

- `GET /api/workbench/note-pivot` returns the selected target language, target, translation overlay, main note-field progress, note rows, field statuses, occurrence counts, and near-Anki source/target previews.
- `POST /api/workbench/apply-preview` accepts browser-local staged edits and returns changed entries, affected overlay files, and validation results without writing YAML.
- `POST /api/workbench/apply` repeats validation and rewrites the selected Translation Overlay as canonical YAML.
- Browser-local staged edits are stored in localStorage until Apply, so refresh keeps unsaved target translations while canonical YAML remains unchanged.

Source edits, stale-record creation, new language scaffolding, and richer pivots are later slices.
