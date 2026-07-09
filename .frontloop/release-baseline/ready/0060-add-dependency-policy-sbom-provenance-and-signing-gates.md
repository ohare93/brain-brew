---
title: Add dependency policy SBOM provenance and signing gates
priority: medium
---

## Goal

Add advisory/license policy and verifiable supply-chain metadata for released crates and binaries.

## Acceptance Criteria

- Cargo and npm production dependencies have automated advisory and license checks with documented exceptions
- Each release produces an SBOM for shipped artifacts
- Provenance/attestations tie artifacts to the source SHA and workflow
- Checksums and signatures are published and verified by smoke tests
- Current npm audit findings are triaged rather than silently ignored

## Implementation Notes

Do after the trusted release workflow is established; avoid blocking integrity fixes on long-term signing choices.
