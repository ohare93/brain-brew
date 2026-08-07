# Research: Iced Composer/Compositor and Anki Add-on Direction

**Date:** 2026-05-23

## Question

Should Brain Brew pursue a standalone Iced composer/compositor-style UI, an Anki add-on, or both after the CLI/manifest model stabilizes?

## Sources consulted

- Iced README: https://github.com/iced-rs/iced/blob/master/README.md
- Iced API docs: https://docs.iced.rs/iced/
- Iced changelog: https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- Anki add-on basics: https://addon-docs.ankiweb.net/a-basic-addon.html
- Anki hooks and webviews: https://addon-docs.ankiweb.net/hooks-and-filters.html
- Anki Qt/PyQt guidance: https://addon-docs.ankiweb.net/qt.html
- Anki Python module packaging: https://addon-docs.ankiweb.net/python-modules.html

## Findings

### Iced standalone composer

Iced is a Rust GUI framework inspired by Elm. Its model maps well to a deck composition UI because Brain Brew already has a deterministic state transition shape:

```text
workspace manifest + selected target/overlays + media root
  -> compose
  -> validate
  -> diff/explain
  -> export
```

Relevant Iced strengths:

- Rust-native, type-safe state/update/view architecture.
- Cross-platform desktop support.
- Built-in widgets for lists, text input, scrolling, tables/grids, markdown, etc.
- Async actions fit long-running compose/export/verify work.
- 0.14-era changelog shows meaningful GUI/testing improvements: headless mode, first-class end-to-end testing, table/grid widgets, devtools foundations, and hot reloading.

Risks:

- The Iced README still labels Iced as experimental software.
- Native packaging brings GPU/windowing/backend issues, especially through `wgpu` and platform graphics stacks.
- A desktop UI is a separate product surface; it should not become the source of truth for build semantics.

Best fit:

- A **standalone local composer** after CLI semantics are stable.
- Primary UX: open `brainbrew.yaml`, list targets, show expanded overlay stack, preview semantic diff/conflicts, run verify, export CrowdAnki.
- The GUI should shell out to `brainbrew` or call a stable Rust library API; it should not invent separate composition rules.

### Anki add-on

Anki add-ons are Python packages loaded by Anki. Official docs show add-ons attaching menu actions through `aqt`, using new-style hooks, and building UI with Qt/PyQt via `aqt.qt`.

Relevant strengths:

- Direct access to Anki collection and UI.
- Natural place for “Import/Update from Federated Deck workspace” workflows.
- Webview hooks allow HTML/JS UI injection when needed.
- Qt is already Anki’s UI toolkit, so a small native-feeling add-on can be built without adding a separate GUI runtime.

Risks:

- Non-standard Python packages must be bundled; C-extension/native dependencies are much harder because they require per-platform compatible builds.
- Embedding Rust/Iced directly inside an Anki add-on is likely high-friction: Anki expects Python/PyQt UI, while Iced wants its own native window/event/rendering stack.
- Direct collection mutation is sensitive and should wait until import/update semantics are proven by CLI fixtures.

Best fit:

- A **thin Python/Qt add-on** later, not an Iced add-on.
- Initial add-on should select a Federated Deck workspace/target, call `brainbrew compose` or `brainbrew export crowdanki`, and hand the output to Anki-compatible import/update code.
- Avoid bundling complex Rust/Python native integrations until there is strong demand.

## Recommendation

1. **Do not build GUI/add-on yet as product work.** The manifest, verifier, package metadata, media workflow, explain output, and authoring helpers should remain the foundation.
2. **Prototype Iced first for maintainer workflows**, not for end users in Anki:
   - target picker;
   - overlay stack visualizer;
   - conflict/diff pane;
   - verify/export buttons;
   - reads exactly the same manifests used by CLI/CI.
3. **Prototype Anki add-on second and keep it thin:** Python/PyQt menu action/dialog that shells out to the CLI or a packaged binary.
4. **Avoid embedding Iced inside Anki.** It combines the risks of both worlds and does not align with Anki’s add-on architecture.

## Suggested prototype gates

Before any UI prototype becomes durable product code:

- `brainbrew verify --manifest ... --all-targets` must cover formatting, composition, validation, media, and configured golden exports.
- `brainbrew explain --manifest ... --target ...` must expose enough conflict/diff detail for a UI to render without reimplementing core logic.
- target discovery must be machine-readable via `brainbrew targets --json`.
- exports must be deterministic and reproducible in CI.

## Worth assessment

- **Iced composer worth:** medium-high for maintainers once federation packages grow beyond a few overlays.
- **Anki add-on worth:** high later if import/update from Federated Decks becomes a common user workflow.
- **Iced inside Anki worth:** low; avoid.
