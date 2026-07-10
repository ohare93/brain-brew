---
title: Limit default Rust test and build parallelism
priority: critical
---

## Goal

Make Brain Brew's default Devenv test/build commands CPU-friendly so the heavy Ultimate Geography regression tests do not saturate the machine, accepting longer wall-clock time.

## Acceptance Criteria

- `devenv shell test` runs Rust test cases with a default maximum of two concurrent libtest threads
- Cargo compilation invoked through the project environment defaults to at most two jobs
- The limits apply consistently to CI-style, development-write, focused, and E2E Rust test entrypoints unless an explicit documented override is supplied
- A regression check demonstrates the effective defaults inside `devenv shell`
- Developer documentation explains the thermal-friendly defaults and how to intentionally override them
- Full test, clippy, and representative E2E gates remain green under the limited defaults

## Design Decisions

- Use a conservative default of 2 for both Rust test threads and Cargo build jobs
- Keep explicit per-invocation overrides available for CI or developers who want more throughput

## Implementation Notes

Urgent user-requested thermal-safety task. Integrate before resuming the larger remediation queue.


## Completion Summary

- Added runtime Devenv defaults of RUST_TEST_THREADS=2 and CARGO_BUILD_JOBS=2
- Applied the same fallback in enterShell and enterTest so named tasks, focused commands, dev-write tests, clippy/builds, Trunk/E2E, release smoke, and arbitrary Devenv Cargo commands inherit it
- Preserved explicit caller overrides through shell fallback semantics
- Added check:rust-parallelism using real nested Devenv shells to prove defaults and overrides and wired it into ci/devenv test
- Documented cooler but longer runs, overrides, and scope in AGENTS, README, and install docs
- Passed full CI under bounded defaults, including heavy UG regressions and 13 browser E2E, plus independent Claude judgment

### Files Changed

- devenv.nix
- AGENTS.md
- README.md
- documentation/docs/getting-started/install.md
