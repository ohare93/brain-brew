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
