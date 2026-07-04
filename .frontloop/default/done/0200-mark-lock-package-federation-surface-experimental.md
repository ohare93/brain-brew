---
title: Mark the lock/package federation surface experimental before the next publish
priority: medium
---

## Goal

The lock/package subsystem (`brainbrew lock`, `FederationLock`, downstream deck packages) is honestly labeled **experimental** across docs and CLI, and explicitly excluded from the stability promise of the upcoming crates.io release — without deleting or destabilizing any of the working, tested code.

## Problem

The lock/package federation machinery is fully built and tested (`crates/brain-brew-cli/src/commands/lock.rs` ~885 lines, `crates/brain-brew-formats/src/lockfile.rs` ~238 lines; remote package fetch, gzipped NAR unpack, NAR-style hash pinning; ~41 references in the CLI test suite) and prominently documented (`documentation/docs/reference/lockfile.md`, `examples/downstream-package.md`, featured in `intro.md` and `concepts/what-is-federation.md`). It has **zero real consumers**: ultimate-geography — the only production deck — has no lock file; no downstream package exists anywhere.

Publishing the next version with this surface presented as a stable pillar freezes a lockfile format and CLI contract that no real usage has pressure-tested. The decision (2026-07-03 review, issue 20) was to **relabel**, not trim: keep the code and the vision, drop the implicit stability commitment.

## Acceptance Criteria

- Every doc page presenting the lock/package feature (`reference/lockfile.md`, `examples/downstream-package.md`, `intro.md`, `concepts/what-is-federation.md`, plus mentions in `getting-started/cli-tour.md`, `reference/project-scope.md`, `reference/releasing.md` — re-grep for the full list, don't trust this one) carries a clear, consistent experimental notice: the feature works, but the lockfile format and `lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.
- `brainbrew lock --help` (and any subcommand help text) states the experimental status in one line.
- The stability/versioning story is stated once, centrally (`releasing.md` or wherever the release policy lives): what IS covered by the next release's compatibility promise (canonical YAML format, deck/overlay semantics, core CLI verbs) and that lock/package is explicitly outside it.
- No code behavior changes; `cargo test --workspace` still passes untouched.
- Zero deletions: all lock/package code, tests, and docs remain — this is labeling, not trimming.

## Design Decisions

- Sequencing: MUST land before the next crates.io publish (the whole point is what that release promises). Pairs with the UG-side version-pinning task only in timing, not content.
- Tombstone/patch/personal overlay kinds are NOT part of this task — they were reviewed and judged cheap, coherent parts of the overlay algebra needing no relabel (review issue 20, tiers 2–3).
- If a real downstream consumer appears later, removing the experimental notices is the closing act of whatever task onboards it.
