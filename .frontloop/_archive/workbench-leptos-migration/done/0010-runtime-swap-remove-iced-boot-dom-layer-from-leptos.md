---
title: Runtime swap — remove Iced, boot the existing DOM layer from Leptos
priority: high
---

## Goal

Remove Iced entirely from `crates/brain-brew-workbench-ui` and replace it with a minimal Leptos (CSR) bootstrap, keeping the existing imperative DOM layer (`publish_*` functions, event `Closure`s, localStorage helpers, generation counters) byte-for-byte functional. Runtime swap, not a rewrite: after this stage the app behaves identically and all 13 E2E scenarios stay green. **This is the de-risk milestone — delegate first, judge, and integrate before any later stage.**

## Background (verified)

`run()` (lib.rs:29) starts `iced::application(WorkbenchApp::new, update, view)`. The Iced layer does only this: on start, `Task::perform(fetch_workspace())` + `Task::perform(fetch_note_pivot())`; on completion, `publish_workspace_probe("loaded", …)` / `publish_note_pivot_panel(&pivot)` (or error variants; `NotePivotLoaded(Err)` with `is_stale_workbench_request` is swallowed). `Message::RefreshWorkspace` is never dispatched from the DOM — all interactivity is self-contained `web_sys`. The Iced `view()` renders a decorative canvas shell no E2E test observes; it may disappear.

## Work items

1. `Cargo.toml`: delete both `iced` deps (wasm32 + non-wasm32 sections). Add `leptos` 0.8.x `features = ["csr"]` (pin the current 0.8 patch at impl time) under the wasm32 target (or unconditional if it builds natively cleanly — native `devenv shell test` build must stay green). Keep `gloo-net`, `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys`, `serde_json`. Update crate `description` (currently "Iced/WASM frontend") to Leptos.
2. `src/main.rs`: replace `fn main() -> iced::Result {…}` with a Leptos CSR entry: on wasm32, `leptos::mount::mount_to_body(App)` (+ optional console_error_panic_hook); on non-wasm32, `fn main() {}` so `--all-targets` builds.
3. `src/lib.rs`: delete `run()`, `theme()`, `WorkbenchApp`, `Message`, the Iced `view()`, all `iced::` imports. Add a minimal wasm32-gated `App` component that renders nothing visible of its own and, on mount, does exactly what `WorkbenchApp::new` + `update` did (set probe loading; fetch `/api/workspace` → `publish_workspace_probe`; fetch note pivot → `publish_note_pivot_panel` / swallow stale / `publish_note_pivot_error`). Preserve exact status strings — probe text is asserted. Keep `WorkspaceSummary`/`from_workspace_json` public and unchanged (`tests/workspace_summary.rs` must stay green unmodified).
4. Do NOT restructure/rename/"improve" any `publish_*` fn, event wiring, generation counter, or localStorage helper this stage. Mechanical deletion of dead Iced-only code is fine; behavior changes are not.
5. Do not modify `index.html`. `mount_to_body` appends after the static `#brainbrew-workbench-e2e` div; probe updates continue via the existing `publish_workspace_probe` path.
6. Regenerate embedded release assets: `devenv shell workbench-ui-embed`, include the refreshed `crates/brain-brew-cli/assets/workbench` in the payload (the one sanctioned write outside the UI crate — purges Iced/wgpu from shipped assets).

## Acceptance Criteria

- `rg -i "iced|wgpu|tiny-skia" crates/brain-brew-workbench-ui/ Cargo.lock` shows no iced/wgpu/tiny-skia entries attributable to this crate (Cargo.lock loses the iced tree entirely).
- All validation gates green, including full **13/13** `devenv shell e2e`. Zero E2E test-file edits.
- `tests/workspace_summary.rs` unmodified and green.
- `crates/brain-brew-cli/assets/workbench` regenerated via workbench-ui-embed.
- Report: exact Leptos version pinned; whether wasm-bindgen moved off the locked 0.2.125; any Trunk/devenv friction (this stage exists to surface it).

## What green demonstrates

Leptos 0.8 CSR builds under nix-pinned Trunk 0.21.14 / wasm-bindgen-cli with the existing `index.html` `data-bin` entry and no Trunk.toml; `mount_to_body` coexists with the static boot anchor and the `set_inner_html` layer; the Iced/wgpu tree is fully excised; and all 13 scenarios survive the swap. If any fails, we spent one Run, not six.

## Delegation

- Agentleman Run `agm-run-20260706142040-dilnlr` (workspace handle `wb-leptos-0010`), delegated 2026-07-06, `workspace: true`, maxFixAttempts 3.
- Automated checks wired: `devenv shell test`, `devenv shell workbench-ui-build`, `devenv shell e2e`.
- On done: apply the Fable judge gate against these acceptance criteria before integrating; do not accept an unjudged payload.

## Outcome (2026-07-06): DONE, judged ACCEPT, integrated

Fable judge verdict **ACCEPT** — all 9 acceptance criteria verified against the actual diff; imperative DOM layer untouched (only dead native no-op stubs deleted); 13/13 e2e confirmed in the run transcript; wasm 3.69MB→386KB; zero iced/wgpu strings; exactly 9 changed files; server/e2e-oracle/devenv.nix untouched. Non-blocking notes: added `console_error_panic_hook` (standard, wasm32-only); removed a now-dead Iced-internal status string (never DOM-asserted). Leptos 0.8.20 pinned; wasm-bindgen held at 0.2.125 (no devenv.nix bump needed).

Integrated into the `default` workspace as jj change `pwtunoykqqky` (commit `0bb…`). Undo: `jj op restore b4d04e758cd5`.

## Risk

Toolchain compat is existential. If the trunk build fails on wasm-bindgen version skew (Cargo.lock 0.2.125 vs pinned CLI), the fix is a `devenv.nix` pin bump — **parent-owned; surface it, do not hack around it in the delegate.**
