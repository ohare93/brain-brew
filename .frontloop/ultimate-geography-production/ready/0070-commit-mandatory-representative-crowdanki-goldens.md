---
title: Commit mandatory representative CrowdAnki goldens
priority: critical
---

## Goal

Add independent non-optional golden exports covering representative production target families and languages.

## Acceptance Criteria

- Goldens cover source language, non-Latin, RTL/CJK, Extended, Experimental, main Hardcore, and companion Hardcore
- Manifest targets configure the golden paths explicitly
- Missing goldens fail rather than early-return success
- Comparison uses the complete normalized equivalence oracle
- Intentional deviations use narrow reviewed allowlists with rationale

## Implementation Notes

Depends on canonical/media/translation fixes and complete CrowdAnki oracle.
