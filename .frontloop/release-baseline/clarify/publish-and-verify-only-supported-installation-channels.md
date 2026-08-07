---
title: Publish and verify only supported installation channels
priority: high
---

## Goal

Make README and installation documentation point exclusively to live, tested artifacts matching the documented CLI interface.

## Acceptance Criteria

- Every advertised tag URL, installer, crate, formula, or Nix command resolves successfully
- Each supported channel installs the same version and passes a CLI feature smoke test
- Unavailable Homebrew/GitHub/Nix claims are removed or clearly marked planned
- Release-channel checks run after publication and report actionable failures

## Implementation Notes

Last release-baseline task; depends on selected channel policy and green artifact gates.


## Blocked

Final acceptance requires real alpha.2 publication and post-publication verification of advertised crates.io, GitHub artifact/tag, and pinned Nix channels. Publishing is an external irreversible action that has not been explicitly requested; current release policy retains manual crates.io publication and live Ultimate Geography consumer evidence is deliberately still absent, which blocks tagged hosting fail-closed. The task can resume after an explicit publish/verification request and required live-consumer evidence configuration.
