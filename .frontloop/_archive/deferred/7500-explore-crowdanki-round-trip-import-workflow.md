---
title: Explore a CrowdAnki round-trip import workflow (pull in-Anki edits back to source)
priority: low
---

## Goal

A maintainer who edits notes directly in Anki can bring those edits home: export via CrowdAnki, run one supported Brain Brew workflow (ideally a single command), and get a reviewable change against the canonical source — restoring, in overlay-native form, what the old Python world's `anki_to_source.yaml` recipe provided.

## Status: clarify — deliberately deferred

The loss of the old Anki→source recipe was a **purposeful simplification** during the UG migration, not an oversight. This is a later thing. Do not promote to ready until the core review backlog (media integrity, include preservation, stale resolution, resolver unification) has landed and there is actual maintainer demand for the round trip.

## Problem

The primitives exist but are dead-ended:

- `brainbrew import crowdanki <folder> --accept-suggested-ids --out deck.yaml` (`crates/brain-brew-cli/src/commands/import.rs` ~:9) does a full re-import into a fresh deck file — right for initial migration, destructive for round-tripping (would flatten includes, structured messages, media declarations, overlay federation).
- `brainbrew diff <left> <right> --as-overlay` (`crates/brain-brew-cli/src/commands/diff.rs` ~:11) drafts an overlay from a semantic diff — the correct primitive for expressing "what changed in Anki" against a composed target.
- Nothing connects them; UG's CONTRIBUTING documents CrowdAnki in one direction only (smoke-test import into Anki, ~:494).

The plausible workflow: import the Anki export to a temp canonical deck → diff against the composed English target → emit a reviewable overlay/patch draft → human reviews and applies to source.

## Questions to settle before ready

1. **ID alignment.** Import derives suggested StableIds from names/first field. When do they line up with the existing deck's IDs, and what does the workflow do when they don't? (Interacts with the derived-ID-collision fail-closed fix.)
2. **Reverse-mapping lossiness.** The diff runs against the composed target, where includes are materialized and structured messages are rendered. Edits landing in those fields can be detected but not automatically written back to source form. Is "reviewable draft + human step" the contract, or does any subset (plain scalar fields) get an automatic write-back path? (`!image` adoption would shrink this gap for image fields.)
3. **Command shape.** A documented three-step recipe using existing commands vs a convenience command (`brainbrew pull`-style) wrapping import→compose→diff. What does the output artifact look like — overlay draft on stdout, a file, a workbench surface?
4. **Scope of targets.** Old world was English-only. Is the round trip defined only against the base/EN composed target, or per-target (which would imply reverse-mapping through translation dictionaries — much harder, likely out of scope)?
5. **Demand check.** Confirm a real UG maintainer scenario before building; if nobody edits in Anki anymore, close this instead.

## Acceptance Criteria (draft, to firm up on promotion)

- A supported, documented, tested workflow (command or recipe) producing a reviewable source-level change from a CrowdAnki export of an existing deck.
- Non-destructive by construction: never overwrites `deck.yaml`/overlays directly; output is a draft a human applies.
- Honest about lossiness: structured-message and include-backed fields are flagged as needing manual handling, not silently mangled.
- UG-side follow-up task (CONTRIBUTING maintainer section) filed once the tool side ships.
