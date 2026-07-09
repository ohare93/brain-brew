---
title: Gate every release on reusable quality and artifact workflows
priority: critical
---

## Goal

Prevent cargo-dist hosting or channel publication unless CI, extracted-package verification, package smoke, Nix checks, embedded assets, docs, and representative consumer gates are green for the same commit.

## Acceptance Criteria

- Release jobs depend on one reusable quality workflow for the same SHA
- Package and artifact smoke install from produced artifacts, not source paths
- No tag can reach upload/host after a required gate fails or is skipped
- The workflow is exercised safely on pull requests without publication credentials

## Implementation Notes

Depends on extracted-crate and Nix gate tasks; UG consumer gate is added by its epic.
