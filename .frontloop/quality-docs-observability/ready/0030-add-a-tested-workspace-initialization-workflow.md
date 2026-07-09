---
title: Add a tested workspace initialization workflow
priority: high
---

## Goal

Provide an `init` command or an explicitly supported generated scaffold so a new maintainer can create a valid manifest, deck, directories, and first target without hand-assembling undocumented state.

## Acceptance Criteria

- The workflow creates all required directories and canonical starter files
- Generated workspace passes format and verify immediately
- Options cover a minimal local deck and clearly defer advanced federation
- Existing destinations require explicit safe overwrite behavior
- CLI and documentation tests exercise initialization from an empty directory

## Implementation Notes

Land before executable quickstart so the quickstart uses the supported scaffold.
