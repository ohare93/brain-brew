---
title: Implement structural field-name safety and Anki reference validation
priority: critical
---

## Goal

Apply the chosen field-name policy and ensure no localized export can retain stale `{{Field}}` references.

## Acceptance Criteria

- Field names are excluded from translation or all references are atomically rewritten according to decision
- Template validation resolves Mustache field references against the final note-type schema
- Coverage no longer encourages unsafe identifier translation
- Existing dictionaries receive migration diagnostics
- The Capital→Hauptstadt reproduction fails safely or exports fully rewritten cards

## Implementation Notes

Depends on field-name decision and canonical source migration support.


## Completion Summary

- Made Anki field identifiers structural and emitted typed migration diagnostics for forbidden field-name translations
- Added final-schema Mustache reference validation in core and revalidation after variable rendering at CrowdAnki export
- Covered direct/triple/section/filter reference grammar plus malformed and unknown references
- Prevented translation coverage/stub generation from re-soliciting structural identifiers
- Added Capital→Hauptstadt regression proving validation fails before output publication
- Passed focused core/formats/CLI tests, fmt, clippy, full CI, and independent Claude judgment

### Files Changed

- crates/brain-brew-core/src/template_validation.rs
- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/src/validate.rs
- crates/brain-brew-core/tests/template_field_validation.rs
- crates/brain-brew-core/tests/translation_coverage.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/tests/cli_contract.rs
