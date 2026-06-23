# ADR-014: Require Workbench API and Browser E2E Tests

**Date**: 2026-06-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The Deck Workbench is a local file-editing application with a Rust server, Iced/WASM browser UI, browser-local staged edits, and Apply operations that mutate Canonical Deck files and Translation Overlays. Unit tests and service-level tests are not enough to prove that the intended workflow works end to end.

## Decision

Every user-visible Deck Workbench slice must have both API integration tests and browser E2E tests before it is considered done.

API integration tests start the real workbench server on loopback with a temp workspace, call JSON HTTP endpoints, and inspect resulting source files. Browser E2E tests use Rust WebDriver tooling with `thirtyfour`, a devenv-provided Chromium/chromedriver, and the real Iced/WASM UI. Browser tests must exercise visible UI state, staged browser-local edits, Apply preview/validation, and resulting file changes. They should save screenshots/artifacts on failure, but visual snapshot diffing is not required initially.

Workbench E2E tests live in a dedicated workspace test crate, `crates/brain-brew-workbench-e2e`, to keep browser automation dependencies out of production crates. The standard browser/API E2E gate is `devenv shell e2e`, and CI must run it. Purpose-built small fixtures should cover most scenarios, with at least one UG-like smoke path for real-world package shape.

## Rationale

**Pros:**

- Proves the actual local-server/browser/file-editing workflow.
- Catches regressions in WASM UI wiring, browser local storage, HTTP APIs, and YAML mutation.
- Keeps slower browser automation out of the default Rust test crate dependencies.
- Makes workbench slices demonstrably usable before they are marked complete.

**Cons:**

- Adds Chromium/chromedriver and WebDriver test infrastructure to devenv/CI.
- Browser tests are slower and can be more fragile than pure Rust tests.
- Test fixtures and failure artifacts need ongoing maintenance.

## Alternatives Considered

- **Playwright**: rejected in favor of Rust WebDriver tests to keep the harness in Rust.
- **Rust integration tests only**: rejected because they do not exercise the browser UI, local storage, or real user workflow.
- **Browser E2E only**: rejected because API integration tests provide faster, more precise coverage of server behavior and file mutations.
- **Visual snapshot testing from the start**: deferred because semantic assertions plus failure screenshots provide useful coverage with less flakiness.

## Implications

- New workbench features need an E2E acceptance plan before implementation.
- CI must provision browser automation dependencies reproducibly.
- `devenv shell test` can remain the normal Rust test gate, while `devenv shell e2e` becomes the workbench E2E gate.
