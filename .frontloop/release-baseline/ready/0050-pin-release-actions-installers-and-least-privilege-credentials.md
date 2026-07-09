---
title: Pin release actions installers and least-privilege credentials
priority: high
---

## Goal

Remove mutable third-party execution and broad publication credentials from the release path.

## Acceptance Criteria

- GitHub Actions are pinned to reviewed commit SHAs
- Downloaded installers are pinned and checksum-verified or built from a locked dependency
- Job permissions are minimized and publication tokens are isolated to host jobs
- Homebrew writes cannot run on untrusted pull-request code
- Renovation instructions exist for intentional pin updates

## Implementation Notes

Apply after the release workflow dependency graph is explicit.
