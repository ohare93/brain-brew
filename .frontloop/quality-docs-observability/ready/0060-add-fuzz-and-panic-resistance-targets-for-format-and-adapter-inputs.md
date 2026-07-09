---
title: Add fuzz and panic-resistance targets for format and adapter inputs
priority: medium
---

## Goal

Continuously exercise YAML, include, manifest, lock, media, and CrowdAnki decoders/emitters against malformed data without panic, hang, or silent acceptance.

## Acceptance Criteria

- Fuzz targets cover canonical YAML, overlays, manifests, locks, includes, media maps, and CrowdAnki JSON
- Direct emitters are tested against constructible invalid domain values
- Crash/hang/silent-loss findings become minimized regression fixtures
- Scheduled CI runs bounded fuzz campaigns and stores useful artifacts
- Resource limits keep adversarial inputs bounded

## Implementation Notes

After strict decoder work; do not substitute fuzzing for targeted TDD.
