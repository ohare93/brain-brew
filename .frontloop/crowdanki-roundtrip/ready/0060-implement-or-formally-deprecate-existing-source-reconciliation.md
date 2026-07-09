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
