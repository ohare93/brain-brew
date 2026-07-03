---
title: Make Workbench apply writes atomic and all-or-nothing
priority: medium
---

## Goal

A workbench Apply either fully lands on disk or changes nothing. No torn files, no partially-applied multi-file edits, no interleaved concurrent applies.

## Problem

In `crates/brain-brew-cli/src/commands/workbench.rs`, the apply paths — `apply_request_json` (~:744, writes ~:874-887), `apply_staged_source_edits` (~:3666), `apply_new_language_request` (~:1624) — write multiple source files (base deck + overlays) sequentially and in place:

- A failure mid-sequence (validation error while serializing a later file, disk full, process death) leaves the git-tracked source of truth in a state no single user action produces: base and overlay disagree, and verify may not catch it.
- An in-place write that dies partway leaves truncated YAML; the strict parser then rejects the whole workspace.
- No locking: two concurrent apply requests (or apply racing another handler's read) can interleave.

Recovery today is "git checkout" — acceptable for disaster, but the workbench should be trustworthy over source files by construction.

## Process

TDD / red-green-refactor. Write failing tests first:

1. An apply whose second output file fails validation/serialization must leave ALL files byte-identical to their pre-apply state (currently fails: first file is already written).
2. Simulated write failure during the write phase (e.g. injectable writer or read-only target file) must leave previously-written targets untouched (temp files may remain; targets must not change).
3. Torn-file protection: no code path writes a target file in place (assert temp+rename via the shared write helper).
4. Two concurrent apply requests are serialized: final state equals one apply followed by the other, never an interleaving.

Then implement (green), then refactor the three apply paths onto one shared write-transaction helper.

## Acceptance Criteria

- All apply outputs are serialized and validated in memory BEFORE any file IO begins.
- Each file is written temp-file-in-same-directory + fsync + rename; a shared helper is the only way apply code touches disk.
- Apply handlers are mutually exclusive (mutex on the workspace state), and an apply bumps the freshness generation exactly once, after the write phase completes.
- On a rename-phase failure, the error response lists exactly which files were updated and which were not.
- Existing workbench integration tests and E2E pass; `cargo test --workspace` passes.

## Design Decisions

- Scope is the workbench apply/new-language write paths. If other CLI writers (e.g. `translations --apply` in translations.rs) can share the helper trivially, do so; otherwise leave them for a follow-up.
- No cross-file journal/WAL — temp+rename per file after full up-front serialization is sufficient for a single-user local tool; the failure window between renames is acceptable when reported precisely.
