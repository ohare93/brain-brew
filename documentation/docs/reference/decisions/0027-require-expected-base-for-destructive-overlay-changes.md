# ADR-027: Require Expected Base for Destructive Overlay Changes

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Overlays may outlive the base deck version they were written against. If a patch or override silently applies after the base changes, it can reintroduce old content or erase upstream fixes.

## Decision

Overlay changes with replace, remove, or override intent must declare the base value or hash they expect. Add and merge changes may declare expectations but are not required to.

## Rationale

Destructive and conflict-resolving changes need drift protection. Requiring an expected base makes stale overlays fail validation instead of silently overwriting newer deck content.

## Implications

- Overlay authoring is slightly more verbose for destructive changes.
- Validation must compare expected bases against the deck state at the point where the overlay applies.
- Patch overlays become safer across upstream deck upgrades.
