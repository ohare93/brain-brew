---
title: Unify YAML scalar emission and test the byte-stability promise
priority: high
---

## Goal

One shared scalar-emission implementation for all hand-rolled YAML emitters, with an adversarial test suite that actually pins the ADR-0005 promises: idempotent formatting and lossless round-trip.

## Problem

Emission of canonical YAML is hand-rolled (parsing is serde_yaml) per ADR-0005's determinism goal, but:

- The "can this string be emitted as a plain scalar" predicate is copied three times with different rules: `crates/brain-brew-formats/src/canonical_yaml.rs` ~:1044 (disallows `:`), `manifest.rs` ~:438 (allows `:`), `lockfile.rs` ~:102 (allows `:`). These predicates are correctness-load-bearing (a bare string that YAML re-parses differently — `yes`, `1.0`, trailing `:` — is silent data corruption); a fix to one copy won't reach the others.
- Block-scalar emission (`write_multiline_or_scalar`, canonical_yaml.rs ~:1031) emits `|`/`|-` with no explicit indentation indicator; content with a leading-space first line or trailing whitespace on lines is the classic hand-rolled-emitter corruption case. UG's multiline HTML template fields go through this path.
- `can_emit_plain_scalar` (~:1052) force-single-quotes all non-ASCII, so most content in 15 of UG's 16 language overlays is quoted — deterministic but noisy for the files translators edit (nice-to-have to relax, severable).
- The ADR-0005 promises are barely tested: round-trip tests assert semantic equality; only one byte-level assertion exists (`tests/canonical_yaml.rs` ~:71). No idempotence (`format(format(x)) == format(x)`) or adversarial-content tests.

## Process

TDD / red-green-refactor. Write the adversarial test suite FIRST against the current emitters:

1. Table-driven round-trip tests (`parse(emit(x)) == x`) over hostile strings: leading/trailing whitespace, lines with trailing spaces, first line starting with a space, YAML keyword lookalikes (`yes`, `no`, `null`, `~`, `1.0`, `0x1F`), embedded single/double quotes, `: ` inside strings, `#`, non-ASCII (accented, CJK, Hebrew/RTL), empty string, lone newline.
2. Idempotence tests: format-twice-compare bytes, for unit cases and the full `fixtures/ultimate-geography` deck + overlays + manifest + a lockfile.
3. Any red test is a real pre-existing bug: fix it in the shared implementation (green), then refactor the three emitters onto it.

## Acceptance Criteria

- A single shared scalar-emission module in the formats crate (plain-scalar predicate, quoting routine, block-scalar writer) used by canonical_yaml, manifest, and lockfile emitters; the `:` discrepancy is reconciled deliberately (document which rule won and why).
- The adversarial round-trip + idempotence suite above exists and passes.
- Formatting the full ultimate-geography fixture is byte-identical before and after the refactor, EXCEPT where a red test proved the old bytes were corrupt or where the reconciled `:` rule changes quoting — capture and report any such diffs explicitly.
- Optional/severable: relax the plain-scalar rule to allow common non-ASCII word characters only if the round-trip suite proves it safe; skip if in doubt.
- `cargo test --workspace` passes.

## Design Decisions

- Do NOT replace the hand-rolled emitter with serde_yaml emission — determinism/byte-stability is the point (ADR-0005). This task consolidates and hardens, not rewrites.
- Emitting an explicit block-scalar indentation indicator (e.g. `|2`) or falling back to quoted style for pathological content are both acceptable; correctness of round-trip beats prettiness.
