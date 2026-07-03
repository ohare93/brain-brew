---
title: Fix stale target count in the ultimate-geography example docs
priority: low
---

## Goal

The ultimate-geography worked example in the docs reflects the real UG repo, and no longer hardcodes a target count that rots as UG grows.

## Problem

`documentation/docs/examples/ultimate-geography.md` still claims "56 verified targets" (line 27) and shows `✓ verified 56 targets` sample output (line 44). The real UG repo now builds roughly 80 targets across its two manifests (`brainbrew.yaml` + `brainbrew-hardcore.yaml`, 16 languages including the later-added Hebrew). This page is the only real-world example in the docs — its whole purpose is to be a trustworthy worked example, and stale numbers on it signal "the docs aren't maintained".

## Acceptance Criteria

- Re-verify the current numbers first against the live repo (`~/Development/external/ultimate-geography`, or its upstream): run `verify --all-targets` / `targets` per manifest and use the actual counts. Do not trust the "~80" in this task file.
- Update the prose and the sample output. De-specify where possible so the page can't rot the same way again — e.g. "all language targets across both manifests" in prose, and mark the sample-output count as illustrative (or regenerate it as part of the edit and date it).
- While on the page, sweep it for other stale facts against the real repo (language count, manifest names, layout paths — the repo reorganized to `templates/<name>/{question,answer}.html`); fix what's cheap, note anything bigger as a follow-up rather than expanding scope.
- Docs build passes (whatever `documentation/` uses to build/lint).

## Design Decisions

- Docs-only change; no code, no fixtures. Fixture drift vs the real repo is a separate concern.
