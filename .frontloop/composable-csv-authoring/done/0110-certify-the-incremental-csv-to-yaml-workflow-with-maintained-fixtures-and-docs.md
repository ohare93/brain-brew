---
title: Certify the incremental CSV-to-YAML workflow with maintained fixtures and docs
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-11
---

## Goal

Finish the epic with a small repository-owned UG-shaped fixture and executable documentation proving that all-CSV, mixed, and progressively native YAML authoring produce the same validated outputs.

## Acceptance Criteria

- Create a compact synthetic fixture owned by Brain Brew, not copied wholesale from the Ultimate Geography repository, with a primary table, explicit joins, stable note IDs, tags, localized GUIDs, scalar translations, shared typed flag/map media, hashes, and sparse region codes
- Include at least two target languages, a repeated source with conflicting targets requiring contextual translation, reusable direct translations, no-change, source-only deletion, target-only adaptation, and adapter-ID translation
- Represent an initial all-CSV state and one or more migrated states where a country/note, a reusable translation, a contextual translation, and a sparse overlay value move to YAML
- Assert equal CanonicalDeck semantic diff and equal representative CrowdAnki output across storage-only migrations, while separately asserting the added native stale/edit capability
- Exercise fmt, validate, compose, translation coverage, verify, semantic diff, explain/source provenance, lock/fingerprint behavior, media integrity, and CrowdAnki export through documented commands
- Document direct notes versus `!csv` and mixed source syntax, descriptors, joins, parameters, images/media IDs, translations.from_csv inference, exclusions/ownership transfer, read-only Workbench behavior, limitations, and troubleshooting
- Run repository formatting, linting, unit/integration tests, docs checks, and the fixture's executable workflow successfully
- Confirm the epic changes no files in the Ultimate Geography repository and record that the existing ultimate-geography-production epic may resume only after this certification passes

## Design Decisions

- Every implementation task grows the fixture through red-green-refactor; this final task consolidates rather than postpones testing
- The fixture demonstrates behavior and migration equivalence without making live UG a build-time dependency
- Full CSV write-back and old Brain Brew recipe compatibility remain explicit non-goals

## Implementation Notes

Depends on every prior task. Align with the existing approved UG fixture contract: repository-owned deterministic fixture coverage first, then a separately pinned live-consumer/update phase after this epic. Avoid documentation promises beyond the tested narrow contract.


## Completion Summary

- Added a compact repository-owned UG-shaped fixture with strict joined/localized CSV descriptors, two target languages, stable IDs/tags/GUIDs, typed flag/map media with hashes, and sparse region codes.
- Certified exact direct/contextual/no-change/deletion/adaptation/adapter-ID translation cases and all-CSV versus progressively migrated note, translation, and sparse ownership states.
- Added executable CLI certification for format preservation, validation, composition, coverage/provenance, semantic diff, verification, explain fingerprints, lock invalidation, media integrity, and byte-identical CrowdAnki export.
- Added fixture-backed Workbench certification for CSV note/translation/sparse read-only capabilities, migrated inline writability, typed rejection, and native stale tracking.
- Documented all supported syntax, inference/transfer rules, limitations, troubleshooting, non-goals, and the post-certification Ultimate Geography live-consumer gate.
- Passed fresh Grok final review, focused/full tests, fmt, clippy, docs build, and UG boundary checks; full CI's unchanged supply-chain stage remains externally red on 16 pre-existing untriaged npm advisories with no suppression added.

### Files Changed

- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/composable_csv_certification.rs
- documentation/docs/authoring/composable-csv-certification.md
- documentation/docs/authoring/workspace.md
- documentation/docs/reference/project-scope.md
- documentation/sidebars.js
- fixtures/composable-csv-authoring/brainbrew-all-csv.yaml
- fixtures/composable-csv-authoring/brainbrew-migrated.yaml
- fixtures/composable-csv-authoring/deck-all-csv.yaml
- fixtures/composable-csv-authoring/deck-migrated.yaml
- fixtures/composable-csv-authoring/experimental-all-csv.yaml
- fixtures/composable-csv-authoring/experimental-migrated.yaml
- fixtures/composable-csv-authoring/media.yaml
- fixtures/composable-csv-authoring/note-types.yaml
- fixtures/composable-csv-authoring/sources/countries.yaml
- fixtures/composable-csv-authoring/sources/countries-experimental.yaml
- fixtures/composable-csv-authoring/sources/regions.yaml
- fixtures/composable-csv-authoring/sources/data/main.csv
- fixtures/composable-csv-authoring/sources/data/countries.csv
- fixtures/composable-csv-authoring/sources/data/hints.csv
- fixtures/composable-csv-authoring/sources/data/guids.csv
- fixtures/composable-csv-authoring/translation-de-all-csv.yaml
- fixtures/composable-csv-authoring/translation-de-migrated.yaml
- fixtures/composable-csv-authoring/translation-es-all-csv.yaml
- fixtures/composable-csv-authoring/translation-es-migrated.yaml
- fixtures/composable-csv-authoring/translation-experimental-de.yaml
- fixtures/composable-csv-authoring/media/flag-france.svg
- fixtures/composable-csv-authoring/media/flag-germany.svg
- fixtures/composable-csv-authoring/media/flag-spain.svg
- fixtures/composable-csv-authoring/media/map-france.svg
- fixtures/composable-csv-authoring/media/map-germany.svg
- fixtures/composable-csv-authoring/media/map-spain.svg
- .frontloop/composable-csv-authoring/done/0110-certify-the-incremental-csv-to-yaml-workflow-with-maintained-fixtures-and-docs.md
