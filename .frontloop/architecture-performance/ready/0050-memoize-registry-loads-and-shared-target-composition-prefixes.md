---
title: Memoize registry loads and shared target composition prefixes
priority: medium
---

## Goal

Reduce 74-target verification from repeated full registry/source/overlay work while preserving exact deterministic diagnostics and output.

## Acceptance Criteria

- Manifest registry and immutable source documents load once per command
- Targets sharing base/overlay prefixes reuse validated immutable composition states
- Errors retain target/overlay attribution despite reuse
- 74-target UG benchmark improves against the recorded ~18.9-second baseline
- Cached and uncached results are byte/diagnostic equivalent

## Implementation Notes

After consolidated planner and core correctness; optimize measured prefixes before adding parallelism.
