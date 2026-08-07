---
title: Validate YAML unions and scalar types before conversion
priority: critical
---

## Goal

Reject conflicting, orphaned, or partial field/message/image/media alternatives and unintended YAML scalar coercions instead of accepting then discarding data.

## Acceptance Criteria

- Each union shape has exactly one valid representation
- Conflicting or incomplete structured values fail with schema-location diagnostics
- Boolean, null, and numeric scalars are rejected where canonical strings are required
- Public emitters return errors rather than panic on constructible invalid map keys
- A malformed-union matrix and hostile scalar corpus cover all decoder entry points

## Implementation Notes

Depends on duplicate-aware decoding so one strict decoder path owns all failures.


## Completion Summary

- Added fail-closed schema validation for malformed field, message, image, media, and overlay union representations
- Rejected unintended boolean, null, and numeric coercion at canonical string positions while preserving intentional typed schema values
- Added schema-path diagnostics and unchanged-byte CLI failure coverage
- Made overlay, manifest, and lock emitters fallible and validated constructible invalid states instead of panicking
- Added malformed-union/scalar matrices and preserved UG canonical formatting, idempotence, and all-target composition
- Passed delegated fmt, full non-browser tests, clippy, focused fixture tests, and independent Claude judgment

### Files Changed

- crates/brain-brew-formats/src/strict_yaml.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/src/manifest.rs
- crates/brain-brew-formats/src/lockfile.rs
- crates/brain-brew-formats/src/media_map.rs
- crates/brain-brew-formats/tests/yaml_schema_strictness.rs
- crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs
- crates/brain-brew-formats/tests/emitter_roundtrip.rs
- crates/brain-brew-formats/tests/overlay_yaml.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/commands/diff.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/media_includes.rs
- crates/brain-brew-cli/tests/translations_cli.rs
