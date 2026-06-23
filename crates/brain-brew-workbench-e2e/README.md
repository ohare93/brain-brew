# Deck Workbench E2E harness

This crate contains browser E2E tests for `brainbrew workbench serve`. It is a workspace member so it stays formatted and lockfile-managed, but the normal `devenv shell test` gate excludes it; run the browser suite explicitly:

```bash
devenv shell e2e
```

The `e2e` script builds the Iced/WASM UI into `target/workbench-ui`, builds the `brainbrew` binary, starts the devenv-provided `chromedriver`, and runs the Rust `thirtyfour` tests against a real local workbench server.

## Fixtures

- Small purpose-built fixtures should be created in temp directories by each test. They keep failures focused and allow tests to inspect mutated YAML files directly.
- UG-like smoke coverage should use a reduced fixture with the same shape as Ultimate Geography: a source language, a target language, a translation overlay, a target variant, and file-backed assets/includes when that behavior is under test. Full `fixtures/ultimate-geography/brainbrew.yaml` coverage can be added as a slower smoke once the workbench exposes enough UI to navigate real-world decks.

Failure artifacts are written under `target/workbench-e2e-artifacts` by default, or `BRAINBREW_E2E_ARTIFACT_DIR` when set.
