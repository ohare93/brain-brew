---
title: Choose compositional strict translation ownership policy
priority: critical
---

## Goal

Define how split base, extension, fill, and companion dictionaries jointly satisfy release completeness without judging every overlay against the entire intermediate deck.

## Acceptance Criteria

- Select combined target-stack completeness, source-unit ownership by introducing overlay, or another explicit model
- Define handling for shared content, structural units, ignored/no-change, and target adaptations
- Specify diagnostics attributable to the responsible overlay
- Provide examples for main and companion Hardcore stacks

## Implementation Notes

Blocks strict coverage implementation and UG translation certification.

## Questions

### Q1: Recommended: assign source-unit ownership to the overlay introducing the unit, then evaluate completeness jointly across the final target stack. Approve or choose combined-stack-only policy?
