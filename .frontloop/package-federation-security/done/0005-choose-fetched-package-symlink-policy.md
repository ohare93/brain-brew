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

## Decision

Adopt **reject all symlinks** for path, Git, tarball, staging, and cached package trees in the next preview. This is the safest auditable policy and is reversible only through a future explicit format/policy decision backed by a demonstrated consumer need. Links to files or directories, broken links, and link cycles are all rejected from entry metadata before traversal or extraction. Diagnostics identify the source tree and offending entry. The incompatible behavior and migration guidance are documented in the package-locking and lockfile references.

Agent Tick was unavailable during execution, so the orchestrator applied the task's conservative recommended option under the user's instruction to execute the full remediation program without stopping.
