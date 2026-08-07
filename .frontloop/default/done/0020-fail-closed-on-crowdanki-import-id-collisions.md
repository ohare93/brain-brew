---
title: Fail closed on CrowdAnki import stable-ID collisions
priority: high
---

## Goal

CrowdAnki import must return an error when two source entities derive the same stable ID, instead of silently keeping only the last one.

## Problem

In `crates/brain-brew-formats/src/crowdanki.rs` (`into_deck`, ~lines 790-802), stable IDs are slug-derived (note types from model name, notes from first field value; derivation ~lines 1085-1111). Note types are inserted with `BTreeMap::insert` and notes collected into a `BTreeMap` — both last-wins on duplicate keys. Distinct entities whose names/first fields slugify identically (e.g. "São Tomé" vs "Sao Tome", two notes both starting "Congo") silently vanish from the imported deck, which then validates cleanly. Violates ADR-0010 (fail closed on unsupported adapter data) and ADR-0004 (reviewable stable IDs). Field-level duplicates are already caught by validation (`DuplicateFieldDefinition`); note and note-type level are not.

## Acceptance Criteria

- Importing a CrowdAnki export where two notes derive the same stable ID returns a `CrowdAnkiError` naming the colliding ID and identifying both source notes (e.g. their GUIDs and/or first-field values).
- Same for two note models deriving the same stable ID (naming both model names).
- Error message points the user at the suggested-IDs override path (`import_deck_accept_suggested_ids`) as the resolution.
- The `note_type_by_uuid` map is also collision-checked (two models with the same UUID should already be impossible upstream, but the derived-ID map must not mask a collision).
- Happy-path round-trip tests still pass; `cargo test --workspace` passes.

## Process

Use TDD / red-green-refactor: write the two failing collision tests in `crates/brain-brew-formats/tests/crowdanki.rs` first (duplicate note slug, duplicate note-model slug), confirm they fail with the current silent last-wins behavior, then implement the collision checks, then refactor.

## Implementation Notes

- Collision check is a few lines at each insert site (`BTreeMap::insert` returning `Some` / entry API); most of the work is the tests.
- Keep error style consistent with existing `CrowdAnkiError::Unsupported` messages in the same file.
