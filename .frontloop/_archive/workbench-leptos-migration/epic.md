---
title: workbench leptos migration
slug: workbench-leptos-migration
status: active
created_at: 2026-07-06
completed_at:
---

## Goal

Replace the Iced/canvas frontend in `crates/brain-brew-workbench-ui` with **Leptos** (CSR, Rust→WASM DOM), preserving behavioral parity as judged by the 13 thirtyfour E2E scenarios in `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs`, with **zero changes to the server, its `/api/*` endpoints, or the Trunk build/serve/embed pipeline**.

This epic is the promotion of `default/ready/0130-migrate-workbench-ui-to-declarative-state-driven-architecture` per that task's own guidance ("if it stalls, split into a dedicated epic with the spike as its first task"). The Phase-0 spike is **done**: Iced-on-web renders its widget tree to a `<canvas>` (wgpu/tiny-skia), emits no DOM, and cannot host Anki HTML/CSS card previews, `contenteditable` editing, or real `<img>` media — which is why the current code bypasses Iced entirely with ~28 raw `set_inner_html`/`web_sys` operations. ADR-0011's actual value ("Rust-native type-safe state/update/view in the browser") is preserved by Leptos while gaining the DOM. **Framework decision: Leptos (final).**

## Why the staging works (key finding)

The Iced runtime layer is only ~100 lines: `update()` fires two initial fetches and forwards results to `publish_*`; every interactive DOM closure is self-contained `web_sys` that never dispatches back into Iced. So Stage 1 is a **runtime swap** (Iced out, Leptos bootstrap in, legacy DOM layer retained) that keeps all 13 E2E green immediately, and each later stage is a **strangler-pattern** port of one DOM region — the parity oracle gates every integration. Main is never left with a red suite.

## Fixed points / hard no-go areas

- Server + all `/api/*` endpoints (`crates/brain-brew-cli/src/commands/workbench.rs`). Frontend-only. The server already exposes every list/detail/pivot/apply endpoint the UI consumes — no server work.
- `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs` — the **13** scenarios are the acceptance oracle. No test edits without explicit, reported justification.
- `index.html` boot anchor (`#brainbrew-workbench-e2e`), `data-trunk` links, dev-assets (`target/workbench-ui`), release embed (`crates/brain-brew-cli/assets/workbench` via `include_dir!`), and the nix-pinned Trunk pipeline.
- localStorage staged-edit key schema; the DOM contract E2E relies on (row budgets, no nested view switcher, no fetch from inactive pivots, media `src`/dimensions/HTTP status).

## Validation floor (every task)

`devenv shell fmt` (then fmt:check clean) · `devenv shell test` · `devenv shell clippy` (`-D warnings`, excl. e2e crate) · `devenv shell workbench-ui-build` · `devenv shell e2e` (all **13/13** green). CLI package/binary is `brainbrew`.

## Stages (strictly sequential: 0010 → 0020 → 0030 → 0040 → 0050 → 0060)

- **0010** Runtime swap — remove Iced, boot the existing DOM layer from Leptos (de-risk milestone; delegate first)
- **0020** Leptos-owned panel skeleton — view switcher, view sections, probe, error surface
- **0030** Componentize notes view — nav, detail, contenteditable fields, card previews, media
- **0040** Componentize notes workflows — apply box/preview, new-language panel, multi-pane comparison
- **0050** Componentize card, source-string, metadata views
- **0060** Purge legacy remnants, trim deps, embed refresh, full CI

Delivery: each stage is a self-contained agentleman Run (`workspace: true`), judged against its acceptance criteria (judge inspects actual E2E evidence), integrated into Main before the next is delegated.

## Open risks for the lead

1. **Leptos/Trunk/wasm-bindgen nix-pin compat** is the existential risk — Stage 0010 surfaces it first. If Leptos 0.8 drags `wasm-bindgen` past the pinned CLI, the fix is a `devenv.nix` pin bump (parent-owned, not delegated).
2. **contenteditable cursor-jump** threatens the typing-flow tests — Stage 0030 mandates the uncontrolled-element pattern.
3. **Stale-response guards** ported as explicit generation tokens (not Leptos `Resource` auto-cancel) — a test asserts absence of stale DOM.
4. Task statement originally said 14 scenarios; the file has **13**.
5. Untracked `.frontloop/default/` and repo-root `default/` dirs are unrelated; keep out of Workspace payloads.
