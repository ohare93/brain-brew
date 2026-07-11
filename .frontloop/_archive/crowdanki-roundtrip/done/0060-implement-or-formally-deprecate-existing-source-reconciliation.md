---
title: Implement or formally deprecate existing-source reconciliation
priority: medium
---

## Goal

Resolve `default/clarify/7500` and either build a reviewed Anki-to-existing-federated-source merge workflow or document why import remains bootstrap-only and provide replacement guidance.

## Acceptance Criteria

- The default/7500 decision is explicitly resolved
- If implemented, changes are mapped to canonical base versus overlay ownership with conflict review and no silent rewrites
- If deprecated, CLI/docs remove round-trip implications and describe supported export/bootstrap boundaries
- UG migration documentation reflects the decision
- End-to-end tests cover the chosen workflow

## Implementation Notes

Final CrowdAnki product task; do not start before default/7500 clarification and safe mutation/equivalence work.


## Completion Summary

- Formally deferred existing-source Anki/CrowdAnki reconciliation pending demonstrated maintainer demand
- Defined CrowdAnki import as plan/review/apply full-deck bootstrap output only, never a merge into an existing federated source or overlay stack
- Removed user-facing round-trip/pull/reconciliation promises while retaining adapter-equivalence implementation terminology
- Documented safe boundaries for export, bootstrap import, plan/apply, and diff --as-overlay review artifacts including include/structured/translation/media lossiness
- Documented that UG workflow policy is externally owned and Brain Brew does not restore legacy Anki-to-source support
- Added end-to-end CLI contracts proving bootstrap import refuses existing output without explicit force, preserves it on refusal, rejects pull/reconcile flags, and does not advertise a reconciliation workflow
- Passed full tests, focused CLI contracts, fmt, clippy, docs, release smoke, and Claude judgment

### Files Changed

- AGENTS.md
- CONTEXT.md
- README.md
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/output_transaction.rs
- crates/brain-brew-cli/tests/crowdanki_import_plan_cli.rs
- documentation/docs/authoring/crowdanki-bootstrap-boundary.md
- documentation/docs/authoring/diff-explain.md
- documentation/docs/authoring/importing-crowdanki.md
- documentation/docs/authoring/media.md
- documentation/docs/concepts/canonical-deck.md
- documentation/docs/concepts/identity.md
- documentation/docs/concepts/media.md
- documentation/docs/concepts/what-is-federation.md
- documentation/docs/examples/ultimate-geography.md
- documentation/docs/intro.md
- documentation/docs/reference/cli.md
- documentation/docs/reference/crowdanki-equivalence-oracle.md
- documentation/docs/reference/decisions/0001-scope-brain-brew-as-local-first-deck-federation.md
- documentation/docs/reference/decisions/0003-use-canonical-deck-as-federation-format.md
- documentation/docs/reference/decisions/0005-store-maintainer-source-as-strict-canonical-yaml.md
- documentation/docs/reference/glossary.md
- documentation/docs/reference/project-scope.md
- documentation/docs/reference/workspace-transactions.md
- documentation/docs/reference/yaml.md
- documentation/sidebars.js
