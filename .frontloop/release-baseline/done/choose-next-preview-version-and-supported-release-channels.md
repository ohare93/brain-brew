---
title: Choose next preview version and supported release channels
priority: critical
---

## Goal

Establish the immutable version and channel policy all publication work will implement; 1.0.0-alpha.1 is already published with incompatible interfaces.

## Acceptance Criteria

- Select the next workspace version and exact internal dependency version
- Choose supported channels for the next preview
- Define whether supported channels must publish together or may phase in
- Record the decision in release documentation

## Implementation Notes

Blocks version publication and the pinned Ultimate Geography baseline.

## Questions

### Q1: Recommended: use the next prerelease version, support crates.io plus pinned GitHub artifacts first, and advertise Homebrew/Nix only after their gates are green. Which version and channels should be authoritative?
