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


## Completion Summary

- Added fail-closed Cargo and production npm advisory/license policy with exact, owned, expiry-checked exceptions and explicit npm audit triage
- Generated artifact-derived CycloneDX SBOMs, checksums, provenance metadata, and keyless Sigstore signatures for shipped artifacts
- Bound verification to source SHA, release workflow, artifact bytes, and checksums; host fails closed on missing/tampered metadata
- Added exact tag-scoped release-workflow OIDC identity validation and restricted id-token permission to signing jobs
- Changed GitHub build provenance attestations to actual shipped artifact subjects, excluding metadata sidecars
- Added static policy/workflow/tamper regression tests and release-security documentation
- Regenerated canonical embedded Workbench assets; freshness, extracted crate verification, aggregate CI, and full task validation passed
- Accepted after independent judge remediation re-review

### Files Changed

- .github/workflows/release.yml
- .github/workflows/reusable-quality.yml
- supply-chain-policy.toml
- scripts/check_dependency_policy.py
- scripts/generate_release_metadata.py
- scripts/check_release_security.py
- scripts/package_artifact_smoke.py
- scripts/smoke_release_archive.py
- scripts/tests/test_supply_chain_gates.py
- scripts/tests/test_release_gate_workflows.py
- scripts/tests/test_release_security_policy.py
- devenv.nix
- documentation/docs/reference/release-security.md
- crates/brain-brew-cli/assets/workbench/index.html
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-24224569c4e4b16d_bg.wasm
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-f0aef0fc4b8956b4.js
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-f0aef0fc4b8956b4_bg.wasm
