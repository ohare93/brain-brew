---
title: Enforce media integrity in verify and export
priority: high
---

## Goal

The media declaration model becomes load-bearing: verify fails closed on media references with no declaration and on declarations whose sha256 is empty or doesn't match the file; export ships declared media itself so consumers stop hand-copying; a tooling command populates/refreshes hashes. An image update is thereby recorded in source state (hash bump, git-visible), not just in the binary file.

## Problem

Verified against the real UG repo (2026-07-03):

- All 546 declared media entries in UG's `deck.yaml` have `sha256: ''` — the integrity field is populated nowhere and enforced nowhere. A corrupted or silently-swapped media file passes `verify --media-root`.
- 61 files on disk are referenced from field content (hardcore extension overlays: `overlays/extensions/hardcore.yaml`, `hardcore/field-fills.yaml` — e.g. `ug-flag-bali-blur.png`) but declared nowhere. Nothing cross-checks referenced-in-content vs declared, so verify can't see it (ADR-0010 says this should fail closed).
- UG's CI (`integrity-check.yml:44`) and CONTRIBUTING hand-`cp media/*` into export output — the blanket copy is what masks the 61 missing declarations, and it means export is not authoritative for media in the tool's flagship deployment.

## Acceptance Criteria

- **Referenced-vs-declared check in verify**: scan composed field content of every target for media references (initially: `<img src>`/`[sound:]`-style extraction over rendered fields; exact-match upgrade comes later via the separate `!image` structured-reference task) and fail with a listing of referenced-but-undeclared paths. Also report declared-but-unreferenced as a warning (unused declarations are noise, not corruption).
- **Hash enforcement in verify**: with `--media-root`, a declared entry whose file is missing, whose sha256 is empty, or whose file hash differs from the declared sha256 is an error naming the entry, the path, and both hashes. (Provide a migration flag or staged severity only if UG adoption needs it; default is fail closed.)
- **Hash tooling**: a command (e.g. `brainbrew media hash --manifest ... --media-root ...`) that computes and writes sha256s into the deck source for missing/stale entries, emitting canonical YAML (respects the include-preserving formatter once that task lands). Refreshing after an intentional image edit is this one command.
- **Authoritative export**: `export crowdanki` copies the declared media set for the target into the output media directory itself; document that consumers can drop manual `cp media/*` steps.
- Tests: fixture with an undeclared-but-referenced file (verify fails), a hash mismatch (verify fails), empty hash (verify fails), and a full green path where export output contains exactly the declared media. Run against the UG fixture once refreshed.
- `cargo test --workspace` passes.

## Design Decisions

- Overlays can already add/change media (`media_changes`, full ChangeIntent semantics) — no model change needed; the UG-side fix declares the hardcore media in the hardcore extension overlay. This task is enforcement + tooling only.
- The content-scan reference extractor should be one clearly-named function so the future `!image` structured reference can replace its innards without touching verify's logic.
- CrowdAnki `media_files`/export layout must keep matching what Anki imports expect — the authoritative copy replicates what the manual `cp` achieved for declared files, it does not invent a new layout.
