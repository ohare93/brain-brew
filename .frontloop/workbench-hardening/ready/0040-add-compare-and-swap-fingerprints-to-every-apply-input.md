---
title: Add compare-and-swap fingerprints to every Apply input
priority: critical
---

## Goal

Reject drafts when any affected canonical, overlay, include, or locked dependency input changed since preview/context load.

## Acceptance Criteria

- Apply carries expected fingerprints/generations for the complete transitive source set
- Server recomputes and compares all preconditions immediately before commit
- Stale external edits return a typed conflict without modifying files
- Fingerprints include scalar/media includes and package dependencies
- Concurrency tests cover same-file and related-file changes

## Implementation Notes

Depends on typed contracts and provenance-aware source planning.
