---
title: Make quickstart import and installation examples executable in CI
priority: high
---

## Goal

Turn user documentation into tested workflows that create directories, use canonical YAML, pass required flags, and install the documented CLI.

## Acceptance Criteria

- Quickstart runs from an empty temporary directory to verified/exported output
- Import examples include the reviewed ID workflow and safe output behavior
- Every advertised install command is smoke-tested or removed
- Command snippets are extracted/executed to prevent drift
- Failures report the documentation page and snippet

## Implementation Notes

Update behavior only after relevant CLI/release tasks land; scaffold the harness first.
