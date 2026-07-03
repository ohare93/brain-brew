---
title: Refresh the UG fixture from upstream via a repeatable sync script
priority: medium
---

## Goal

`fixtures/ultimate-geography/` matches the real ultimate-geography repo (current layout, all 16 languages including Hebrew, both manifests), plus one deliberate, documented delta: the ADR-0012 `languages:`/`translation_profile` catalog. A checked-in script makes the refresh repeatable whenever UG moves again.

## Problem

The fixture is a mid-migration snapshot that has drifted from the real repo in both directions:

- Behind: pre-refactor `source/` include layout (real repo uses `templates/<name>/{question,answer}.html`); missing Hebrew (`he.yaml` — the only RTL language, so script direction is currently untested here); missing the hardcore second manifest (the real repo builds two manifests; the hardcore shape earned two high-priority tasks in UG's pr736-review epic and is unrepresented in this repo's tests).
- Ahead: carries the ADR-0012 `languages:`/`translation_profile` catalog (~240 lines of `brainbrew.yaml`) that the real UG doesn't use yet.

Every "composes the UG fixture byte-identically" acceptance criterion in the queued refactor tasks is only as strong as the fixture's resemblance to production; right now it certifies a deck shape that no longer exists.

## Acceptance Criteria

- A script (e.g. `scripts/sync-ug-fixture.sh` or a small Rust/xtask — implementer's choice) that:
  - Copies the fixture-relevant files from a UG checkout (path passed as an argument; default may assume `../external/ultimate-geography` but must not hardcode-require it) into `fixtures/ultimate-geography/`: both manifests, all overlays, templates, deck sources, and whatever media/metadata files the fixture tests need.
  - Reapplies the translation_profile delta on top. Keep the delta maintainable: either a patch file or a separate YAML fragment the script splices in — NOT hand-editing after each sync. The delta and its rationale (ADR-0012 coverage until UG adopts it upstream — see the UG-side frontloop task) are documented in a README next to the script or fixture.
  - Is idempotent: running it twice produces no diff.
- The fixture after refresh: current UG layout, 16 languages including `he.yaml`, both `brainbrew.yaml` and `brainbrew-hardcore.yaml`, plus the translation_profile delta.
- All fixture-dependent tests updated and passing: `cargo test --workspace`. Expect churn in expected outputs (`ultimate_geography_fixture.rs`, overlay/canonical-yaml tests, docs examples that cite fixture facts) — update expectations to the new fixture, but investigate any change that looks semantic rather than layout-driven before accepting it.
- Hardcore manifest is actually exercised: at least one test composes/verifies a hardcore target (new coverage — the shape was previously untested here).
- The sync script's usage is documented (one paragraph: when to run it, what the delta is).

## Design Decisions

- The fixture intentionally stays "real UG + translation_profile" rather than splitting into two fixtures; the delta is temporary — a UG-side task exists to adopt translation_profile upstream, after which the delta (and splice step) can be deleted.
- Sync direction is one-way (UG → fixture). The script must never write into the UG checkout.
- If some UG files are irrelevant to tests (e.g. full media binaries), the script may exclude them, but the exclusion list lives in the script, not in tribal knowledge.
