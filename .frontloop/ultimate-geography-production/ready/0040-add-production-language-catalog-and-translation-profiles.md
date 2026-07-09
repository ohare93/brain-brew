---
title: Add production language catalog and translation profiles
priority: high
---

## Goal

Make the documented Workbench and translation commands operate against real UG manifests instead of fixture-only catalog/profile state.

## Acceptance Criteria

- Production manifests declare all supported languages and profiles canonically
- Workbench accepts representative language selections including German
- Catalog/profile data has one source of truth across main and companion manifests
- Consistency tests detect duplicated companion drift
- Fixture generation follows the approved fixture contract

## Implementation Notes

After fixture policy; coordinate with translation ownership model.
