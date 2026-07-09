---
title: Add media-byte handoff and safe import output transactions
priority: high
---

## Goal

Import declared media bytes alongside canonical declarations and prevent accidental overwrite or partial workspace creation.

## Acceptance Criteria

- Import inventories CrowdAnki media and verifies every referenced byte
- Generated declarations receive real hashes when bytes are available
- Output defaults to a new clean destination and requires explicit overwrite intent
- Source plus media commit through the workspace transaction module
- Missing/duplicate/unsafe media paths fail before any output changes

## Implementation Notes

Depends on source transactions and package-safe media paths.
