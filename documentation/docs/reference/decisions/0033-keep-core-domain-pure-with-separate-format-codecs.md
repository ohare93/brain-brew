# ADR-033: Keep Core Domain Pure with Separate Format Codecs

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The Rust workspace needs a reusable core for future CLI, UI, and possible Anki integration. Putting YAML, CrowdAnki, filesystem, or terminal concerns directly into the core would make the domain model less portable, but placing codecs only in the CLI would make them hard to reuse.

## Decision

Start with three crates: `brain-brew-core` for the pure domain model, validation, composition, and semantic diffing; `brain-brew-formats` for YAML and CrowdAnki codecs/adapters over in-memory values; and `brain-brew-cli` as a thin command-line interface.

## Rationale

This keeps the core deterministic and dependency-light while still making format support reusable outside the CLI. It is slightly more workspace setup than two crates, but avoids extracting adapters from the CLI later.

## Implications

- `brain-brew-core` must not depend on format-specific crates.
- `brain-brew-formats` depends on `brain-brew-core`.
- `brain-brew-cli` depends on both core and formats, and owns filesystem and terminal interaction.
