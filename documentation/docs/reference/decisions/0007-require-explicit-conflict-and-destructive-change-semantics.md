# ADR-007: Require Explicit Conflict and Destructive-Change Semantics

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Overlays are powerful enough to replace, remove, merge, or override deck content. Silent last-write-wins behavior would make it easy for a translation, extension, or patch to accidentally erase upstream work. Maintainers need upgrade-safe overlays that fail when the base deck has changed in a way the overlay did not expect.

## Decision

Overlay changes must declare their intent, and destructive or conflict-resolving changes must declare the expected base.

Brain Brew supports explicit change intents such as add, merge, replace, remove, and override. Replace, remove, and override require an expected base value or fingerprint. Removals are represented as tombstones. Conflicting overlay changes fail unless a later overlay explicitly resolves the conflict. Field fills may only fill values that are still blank.

## Rationale

**Pros:**

- Prevents stale overlays from silently applying to changed upstream content.
- Makes destructive changes reviewable.
- Supports safe upgrades from a base deck to a newer release.
- Gives diagnostics enough information to explain conflicts precisely.

**Cons:**

- Overlay files are more verbose for destructive changes.
- Authors need to update expected bases when intentionally rebasing.
- Composition can fail where a permissive merge tool might produce an output.

## Alternatives Considered

- **Last-write-wins**: rejected because it hides data loss.
- **Always require full expected base for every change**: rejected because additions and non-destructive merges would become too noisy.
- **Text-patch semantics**: rejected because deck entities need semantic identity and adapter-independent conflict checks.
- **Delete by omission**: rejected because missing data is ambiguous; tombstones record intentional removal.

## Implications

- Composition must carry enough provenance to detect incompatible overlay changes.
- Validation should fail closed for stale expected bases and non-blank field fills.
- Diffs and CLI reports should show before/after values for destructive changes.
- Overlay authoring docs should teach change intent as part of the source format.
