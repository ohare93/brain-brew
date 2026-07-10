# Deck Workbench E2E harness

This crate contains browser E2E tests for `brainbrew workbench serve`. It is a workspace member so it stays formatted and lockfile-managed, but the normal `devenv shell test` gate excludes it; run the browser suite explicitly:

```bash
devenv shell e2e
```

The `e2e` script builds the Leptos/WASM UI into `target/workbench-ui`, builds `brainbrew` with the development-only `workbench-write-dev` feature, starts write scenarios with explicit `--enable-write`, keeps the app-shell containment scenario read-only, starts the devenv-provided `chromedriver`, and runs the Rust `thirtyfour` tests against a real local Workbench server. The app-shell scenario verifies the read-only banner and disabled Apply control; the edit/apply scenario verifies the visible/API `development_write` marker before writing. Normal release binaries do not contain this capability.

## Fixtures

- Small purpose-built fixtures should be created in temp directories by each test. They keep failures focused and allow tests to inspect mutated YAML files directly.
- UG-like smoke coverage uses both reduced fixtures and the real `fixtures/ultimate-geography/brainbrew.yaml` manifest for bounded navigation/media checks.

## Failure artifacts

Failure artifacts are written under `target/workbench-e2e-artifacts` by default, or `BRAINBREW_E2E_ARTIFACT_DIR` when set. Each failing test gets a timestamped subdirectory containing:

- `failure.txt` — the anyhow error chain.
- `failure.png` — browser screenshot when ChromeDriver can capture one.
- `page.html` — full DOM snapshot.
- `debug.json` — structured Workbench diagnostics: active view/button state, visible row/card counts, active element and contenteditable caret state, plus visible media `src`, dimensions, HTTP status/content type, byte length, and missing-placeholder detection.

Use these files first when investigating GUI regressions such as cursor jumps, blank panes, excessive rendering, or missing media.
