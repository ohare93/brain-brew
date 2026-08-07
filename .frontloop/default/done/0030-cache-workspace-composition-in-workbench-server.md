---
title: Cache workspace composition in the Workbench server
priority: high
---

## Goal

Workbench request handlers must stop re-planning and re-composing the workspace from disk on every request; per-request work should be bounded (ADR-0015), keyed on the existing freshness generation.

## Problem

In `crates/brain-brew-cli/src/commands/workbench.rs`:

- `media_path_declared` (~lines 978-1009) runs on every `GET /api/media/<path>` and, per request, re-reads the manifest and fully re-plans + re-composes **every overlay of every target** just to check whether the path is declared media. For UG (~80 targets across two manifests, 319 notes, 546 media files) a single note view (flag + map) triggers hundreds of full-deck compositions.
- `current_manifest` (~line 471) re-reads the manifest file per request; `selected_translation_context` (~lines 1011-1072) re-plans and re-composes the selected target per request.
- All of this blocking file IO and CPU-heavy composition runs inline in `async fn` handlers on Tokio worker threads — zero uses of `spawn_blocking` in the crate.
- Minor: `media_file_candidates` (~lines 945-975) probes `external/<deckname>/...` under every ancestor directory of the manifest root — many stat calls per request; looks like a leftover dev-layout hack.

The server already tracks a freshness generation that bumps when workspace files change (~lines 3525-3579) — the invalidation hook exists but nothing is cached on it.

## Acceptance Criteria

- `media_path_declared` is a lookup into a cached set of declared media paths (union across all targets), built at most once per freshness generation.
- Composed deck / translation-context state for the selected target is cached keyed on the freshness generation (and invalidated by apply/new-language writes, which already bump the generation).
- Manifest is not re-read from disk on requests that hit a valid cache.
- Blocking plan/compose/file work in handlers runs via `tokio::task::spawn_blocking` (or an equivalent dedicated worker), not inline on async workers.
- Behavior is unchanged when files change on disk: after an edit, the next request reflects the new state (existing freshness E2E semantics still pass).
- Existing workbench integration tests in `crates/brain-brew-cli/tests/cli.rs` and the E2E suite pass. Add an integration test asserting media requests succeed after a workspace edit (cache invalidation) and that an undeclared path is still rejected.

## Design Decisions

- Single task on purpose: the media-path set and composed-state cache share the same generation-keyed invalidation; implement one caching layer, two consumers.
- Consider tightening `media_file_candidates` to manifest root + media root only, but do not break the documented `--media-root` behavior; if the ancestor probing is load-bearing for some workflow, keep it and note why.

## Implementation Notes

- Cache lives on the shared server state struct behind a `tokio::sync::RwLock` (or `arc-swap`); key = generation number.
- `compose_lenient_translation_overlay` (duplicated at `workbench.rs:~4414` and `translations.rs:~1314`) is in the hot path; hoisting it to a shared location can be done here or left to the separate dedup task.
