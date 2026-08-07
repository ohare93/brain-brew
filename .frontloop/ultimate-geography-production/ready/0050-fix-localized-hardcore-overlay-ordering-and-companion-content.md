---
title: Fix localized Hardcore overlay ordering and companion content
priority: critical
---

## Goal

Ensure extension/fill content is translated by the correct companion dictionaries before localized Hardcore targets are certified.

## Acceptance Criteria

- Main localized Hardcore targets select required companion content translations
- Overlay order is explicit and matches the standalone companion semantics
- Fresh cs/de/non-Latin/RTL exports contain no unintended English extension prose
- Strict compositional coverage passes under the chosen policy
- Manifest tests prevent future order regressions

## Implementation Notes

Depends on compositional translation ownership implementation.
