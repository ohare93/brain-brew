# ADR-021: Structure Rust Workspace Around Reusable Core

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The project starts as a Rust CLI, but future frontends may include an Iced-based UI, web surface, or Anki integration. If deck federation logic is embedded in CLI handlers, those future surfaces will duplicate or bypass the core behavior.

## Decision

Structure the Rust workspace around a reusable core library. The core owns CanonicalDeck, overlays, validation, federation, and deterministic behavior. Adapter code handles formats such as YAML, CSV, and CrowdAnki. The CLI remains a thin wrapper around library calls.

## Rationale

This preserves one deterministic implementation of the domain rules while allowing multiple interfaces later. It is slightly more setup than a single binary crate, but avoids locking the project into a CLI-only shape.

## Implications

- Tests for core federation behavior should live with the core library.
- CLI tests should focus on command wiring and file behavior.
- Adapter boundaries can begin as modules and become separate crates when reuse or dependency isolation justifies it.
