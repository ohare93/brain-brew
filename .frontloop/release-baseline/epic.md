---
title: Release baseline and trusted distribution
slug: release-baseline
status: active
created_at: 2026-07-09
completed_at:
---

## Goal

Restore a publishable, independently verified Brain Brew release baseline across crates.io, Nix, GitHub artifacts, and whichever installation channels are explicitly selected. Eliminate immutable-version reuse, path-only smoke tests, ungated publication, mutable workflow inputs, and unsupported install claims.

## Sequence

Resolve version/channel policy first, then repair extracted-package and Nix gates, wire those gates into release, harden supply-chain inputs, and only then advertise or extend release channels.
