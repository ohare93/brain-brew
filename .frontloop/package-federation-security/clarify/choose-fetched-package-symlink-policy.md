---
title: Choose fetched-package symlink policy
priority: critical
---

## Goal

Decide whether fetched package snapshots reject all symlinks or permit only links canonically contained within the authenticated root.

## Acceptance Criteria

- Select the policy for path, git, and tarball sources
- Define behavior for links to files, directories, broken links, and cycles
- Specify canonicalization timing and diagnostics
- Document compatibility impact for existing packages

## Implementation Notes

Blocks snapshot/extraction hardening.

## Questions

### Q1: Recommended: reject symlinks in fetched/locked packages for the next preview; reconsider contained links only with demonstrated consumer need. Approve or choose contained-link support.
