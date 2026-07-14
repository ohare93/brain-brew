---
title: Make blank deletion and target adaptations explicit and general
priority: high
---

## Goal

Implement the approved blank policy and replace UG-specific magic-reason `target_additions` with a documented typed adaptation model.

## Acceptance Criteria

- Blank direct entries cannot silently erase global content
- Explicit deletions/adaptations are path-scoped, reviewed, and represented in coverage
- The magic UG reason string is removed from format semantics
- Schema and migration docs explain the general adaptation model
- Round-trip tests cover legacy UG data and canonical new emission

## Implementation Notes

Depends on blank-policy decision and strict YAML union work.


## Completion Summary

- Replaced UG magic target-addition semantics with typed path-scoped adaptation and deletion decisions
- Rejected blank direct/contextual/stale translations; only explicit scoped deletion can remove content
- Added expected-source, intent, ownership, required reason, validation, coverage states, and transactional mutation behavior
- Added legacy YAML compatibility reader, canonical typed writer, warnings, migration diagnostics, and round-trip tests
- Documented general adaptation/deletion schema and migration behavior
- Independently judged ACCEPT

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/src/translation_mutation.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/src/fmt.rs
- crates/brain-brew-cli/src/cli.rs
- documentation/docs/authoring/translations.md
- documentation/docs/reference/yaml.md
