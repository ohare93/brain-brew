---
title: Add a cross-language matrix view to the Workbench
priority: medium
---

## Goal

For a selected note (or source string), the Workbench shows all languages side by side in one table — resolved value plus translation status per language — restoring the cross-language visibility the old CSV world had for free (one row per fact, one column per language) and the overlay architecture deliberately traded away without a replacement.

## Problem

All three workbench pivots (`note-pivot`, `source-string-pivot`, `card-pivot` — `workbench.rs` ~:1302-1318) take a single `language`/`target`/`overlay` selection; nothing in the tool shows one note across languages. Comparing 16 languages of `field.capital` for one note today means opening 16 overlay files or re-selecting the workbench context 16 times. Daily UG moments this blocks:

- Reviewing a new-language PR (e.g. Hebrew) next to sibling languages.
- After an English source edit: which of the 16 languages are resolved vs still stale vs missing — an inherently cross-language question the tool can only answer one language at a time.
- Terminology-consistency checks across related languages (da/no/sv, es/pt).

## Acceptance Criteria

- A workbench API endpoint returning, for a selected note (and optionally a single field or source string), a matrix: one row per language target, with the composed/resolved value and a status per cell (translated / stale / missing / no_change — reuse the existing coverage categories, don't invent a parallel taxonomy).
- The matrix is built from the SAME resolution logic compose/verify use (via the unified resolver once `0050-unify-translation-resolution-into-single-resolver.md` lands) — what the matrix says must be what ships.
- Composing all language targets per request rides the generation-keyed composition cache (`0030-cache-workspace-composition-in-workbench-server.md` is a hard dependency — without it this view is ~16 full compositions per click).
- A workbench UI panel renders the matrix with staleness/missing visually flagged; built on the migrated declarative UI (`0130-migrate-workbench-ui-to-declarative-state-driven-architecture.md`), not as new `format!`-HTML on the legacy path.
- Optional/severable: a CLI twin (e.g. `translations matrix --note X [--field Y]`) emitting the same data as aligned text or TSV for terminal-based PR review.
- Integration test on the UG fixture: matrix for one note returns a row per language with correct statuses (include a stale and a missing case); E2E covers rendering one matrix.
- `cargo test --workspace` passes.

## Design Decisions

- Read-only view in this task. Editing cells in place, or resolving stale records from the matrix, are natural follow-ups that ride the stale-resolution task (`0140-wire-stale-translation-resolution-into-cli-and-workbench.md`) — link them, don't build them here.
- Sequencing: after the composition cache (hard), after/with the UI migration (strong preference), after the resolver unification (correctness of the status column).
- Row set = language targets from the manifest; the view must not hardcode UG's language list or assume every language covers every note (missing IS a status, not an error).
