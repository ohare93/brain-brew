---
title: Add structured !image field references and an includable media block
priority: medium
---

## Goal

Fields reference images structurally (`field.flag: !image ug-flag-zimbabwe.svg`) instead of embedding raw `<img>` HTML, and the media declaration block can live in its own included file (`media: !include media.yaml`). Media references become exactly verifiable, the renderer owns image markup, and the deck source stops carrying ~1,600 lines of media boilerplate inline.

## Problem

- Image references are opaque HTML strings in field values. The media-integrity task's referenced-vs-declared verify check must regex-scrape `<img src>` out of content — workable but fragile, and the tool can never transform image markup (alt text, path rewriting) or diff image changes structurally. This mirrors the raw-string problem structured messages (ADR-0008 lineage) already solved for flag-similarity: same disease, same cure.
- Survey of the real UG repo (2026-07-03): 602 `<img>` tags across deck + overlays, every single one exactly `<img src="X" />` — no attributes, no variation. 221 flag fields + 319 map fields in `deck.yaml`; multi-image only in the hardcore extension (e.g. Bali: blur + normal pair). So a minimal `!image <path>` covers 100% of production usage, with a sequence form for the multi-image cases.
- `!include` is restricted to scalar content fields (`source_includes.rs` ~:101), so the 546-entry media block cannot be externalized from `deck.yaml` today.

## Phase 1 — ADR (do this first, get it reviewed before implementing)

Write ADR-00NN settling, with UG's surveyed usage as the evidence base:

1. **Reference form**: `!image <path>` (path matches media declaration `path` and what's on disk — recommended, matches current HTML and stays greppable) vs `!image <media-stable-id>` (rename-robust but indirect). Pick one, record why.
2. **Multi-image fields**: YAML sequence of tagged scalars (`field.flag: [!image a.png, !image b.png]` or block-sequence form) — confirm the canonical emitter/parser shape.
3. **Mixed text+image content**: out of scope unless the audit finds real usage (UG appears to have none — verify during the audit); raw HTML remains valid field content, `!image` is additive, not mandatory.
4. **Render contract**: at compose/export, `!image p` renders to exactly `<img src="p" />` — byte-identical to today's UG output, so adoption is output-neutral and provable by export diff.
5. **Import reverse-mapping**: CrowdAnki import recognizes fields that are purely one-or-more `<img src="..."/>` tags and emits `!image` form; anything else stays raw HTML (safe fallback, not an error).
6. **Verify semantics**: `!image` references join the referenced-vs-declared check as exact matches (replacing the regex innards of the extraction function the media-integrity task isolates for this purpose).
7. **Includable media block**: lift the scalar-only `!include` restriction for whitelisted structural positions (at minimum `media:` in a deck) — NOT arbitrary mapping includes. Define interaction with the include-preserving fmt/verify work (tagged-scalar emission is shared machinery — coordinate with task `0070-make-fmt-and-format-verify-preserve-source-includes.md`) and with the `media hash` command (it must write hashes into the included file, through the include).

## Phase 2 — Implementation

- Parser + canonical emitter support for `!image` (scalar and sequence) in decks and overlays; compose/export rendering per the ADR; import reverse-mapping; verify exact-match integration.
- `media: !include` per the ADR, working through fmt (structure-preserving), verify, and `media hash`.
- Migration tooling: a one-shot command or script converting pure-`<img>` fields to `!image` (UG has ~600 conversions; must be mechanical and output-neutral).
- Tests: round-trip (parse/emit/idempotence) for both new forms; export byte-equivalence on a fixture before/after migration; import reverse-map; verify exact matching; multi-image sequence case.
- `cargo test --workspace` passes; docs updated (authoring docs + yaml reference).

## Design Decisions

- Sequence AFTER the media-integrity task (which works with regex extraction and is not blocked on this) and coordinate with the include-preserving fmt task (shared tagged-scalar emission).
- Adoption in UG is a separate UG-side task to file once this ships (run the migration, externalize the media block, verify output-neutrality by export diff).
- The includable-media-block half (#7) is severable: if it turns out to fight the include-preservation work, split it out rather than stalling `!image`.
