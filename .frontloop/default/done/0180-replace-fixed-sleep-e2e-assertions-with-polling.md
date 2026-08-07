---
title: Replace fixed-sleep E2E assertions with condition polling
priority: low
---

## Goal

No E2E assertion depends on wall-clock timing: the loading/stale-state scenario polls for observable conditions using the suite's existing `wait_for_*` helpers, per ADR-0014.

## Problem

`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs` ~:406 and ~:441 inject an artificial 800ms delay into a server response (to exercise the workbench loading/stale-state UI), then do a fixed 950ms sleep before asserting the UI has settled:

- It races with a 150ms margin: the assertion holds only if the delayed response + render round-trip completes within 150ms of the injected delay expiring. CI load, WebDriver latency, or WASM startup jitter eats that margin → unreproducible flake.
- It is slow even when passing: every run pays the full 950ms unconditionally, twice.
- It violates ADR-0014 in a file whose own conventions (`wait_for_*` polling helpers, used everywhere else) show the rule is known — a standing bad example for the next test author to copy.

## Acceptance Criteria

- Both fixed sleeps are gone; the scenario polls for observable conditions with the suite's standard `wait_for_*` helpers and timeout:
  - If the scenario must assert the intermediate state (loading/stale indicator visible during the delay), poll for that state FIRST, then poll for the settled state — both are conditions, neither needs a clock.
- No new fixed sleeps anywhere in the E2E crate; a quick sweep confirms these two were the only wall-clock assertions (fix any others found the same way).
- Consider shrinking the 800ms injected delay once nothing sleeps against it — keep it just large enough that the intermediate-state poll can reliably observe the loading state; note the chosen value.
- The scenario still meaningfully exercises the loading/stale-state UI (do not delete the intermediate-state assertion to make polling trivial).
- E2E suite passes (BRAINBREW_E2E_* env per existing docs); `cargo test --workspace` unaffected.

## Design Decisions

- Test-only change; no production code, no server changes beyond (optionally) the injected-delay parameter the test already controls.
