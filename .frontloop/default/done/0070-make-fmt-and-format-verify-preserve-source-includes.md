---
title: Make fmt and format verification preserve source !include structure
priority: high
---

## Goal

`brainbrew fmt` never changes document structure: `!include` markers survive formatting verbatim. Format verification actually verifies include-bearing files instead of silently passing them.

## Problem

Compose/export resolve `!include` correctly on read, and the workbench apply path preserves include structure when writing (`workbench.rs` ~:3681-3752, raw parse keeping tagged scalars, edits routed into the included file). But two paths break the contract:

1. **`fmt` destructively materializes includes.** `fmt.rs` ~:17-20 reads the file, calls `format_source_at`, writes the result back. `format_source_at` (`crates/brain-brew-cli/src/io.rs` ~:37-41) resolves includes BEFORE formatting — so `brainbrew fmt` on an include-bearing deck (like real UG's, whose templates/descriptions are all externalized via `!include`) inlines every included file's content into the source and severs every include link. A formatter that changes structure is a data-loss bug in the most routine command.
2. **Format verification fails open on includes.** `verify_format_with` (`io.rs` ~:517-537) formats the include-resolved text, then — if the raw file contained `!include` — returns `Ok(())` unconditionally (the comment admits the bytes can't be compared). Canonical-format enforcement therefore silently does not apply to exactly the files UG cares most about; a malformed include-bearing file passes verify forever. Opposite of ADR-0010's fail-closed posture.

## Process

TDD / red-green-refactor. Failing tests first:

1. `fmt` on an include-bearing deck fixture leaves every `!include` marker byte-intact and does not inline included content (currently fails: includes are materialized).
2. `fmt` on an include-bearing file is idempotent: second run produces zero diff.
3. `fmt` still normalizes the non-include parts of an include-bearing file (mis-formatted sibling fields get fixed) — preserving includes must not mean skipping formatting.
4. `verify` format check REJECTS a non-canonical include-bearing file (currently fails: unconditional Ok) and accepts a canonical one.
5. Include resolution behavior in compose/export/read paths is unchanged (existing tests stay green).

Then implement (green), then refactor `format_source_at`/`verify_format_with` and the emitter onto the shared include-preserving path.

## Acceptance Criteria

- The canonical emitters can emit an `!include <path>` tagged scalar verbatim where one appeared in the source (serde_yaml `Value::Tagged` on the parse side; the workbench apply path shows the raw-parse-preserving approach).
- `format_source_at` formats the RAW document (includes preserved); include resolution remains a read-path concern only. No fmt code path writes materialized include content into a source file.
- `verify_format_with` compares canonical bytes for include-bearing files like any other file; the fail-open early-return and its comment are gone.
- Included files themselves (e.g. `.html`) are opaque content: fmt does not read, format, or touch them.
- Run the real check: fmt on the refreshed UG fixture (or a fixture file using the current UG include layout) round-trips with includes intact.
- `cargo test --workspace` passes.

## Design Decisions

- The include-preserving format capability should be ONE implementation shared by fmt, verify, and (if it can adopt it cheaply) the workbench apply serializer — not a fourth include-handling variant.
- Related but out of scope: UG-side task `10-low-review-include-materialization-workaround-in-pr-736-evidence-script.md` can only fully remove its workaround once this lands; note completion there.
