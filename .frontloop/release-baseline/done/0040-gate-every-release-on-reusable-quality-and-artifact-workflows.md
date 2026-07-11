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


## Completion Summary

- Added SHA-bound reusable quality workflow shared by CI and tagged release workflows
- Required immutable SHA validation, artifact evidence, Rust quality, extracted crates, Nix package, prepared E2E, docs/embed, package/archive smoke, and consumer-evidence contract
- Made artifact smoke install/package only real Cargo and cargo-dist artifacts with checksum/provenance validation
- Made every release plan/build/host path hard-depend on same-SHA quality success with no always/skip bypass
- Kept pull requests credential-free and side-effect-free while exercising the reusable workflow
- Added fail-closed representative live-consumer interface; fixture evidence cannot satisfy release hosting
- Added workflow/static gate regression tests and documentation
- Passed workflow tests, full suite, fmt/clippy, artifact/archive smoke, Nix check, 13 E2E, docs, and Claude judgment

### Files Changed

- .github/workflows/reusable-quality.yml
- .github/workflows/ci.yml
- .github/workflows/release.yml
- .github/workflows/package-smoke.yml
- scripts/package_artifact_smoke.py
- scripts/smoke_release_archive.py
- scripts/verify_representative_consumer.py
- scripts/tests/test_release_gate_workflows.py
- scripts/tests/test_nix_e2e_partition.py
- devenv.nix
- documentation/docs/reference/releasing.md
