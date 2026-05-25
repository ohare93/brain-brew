# ADR-020: Use Rust for Core and CLI

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

ADR-001 chose Gren for the original broad vision because it promised deterministic functional logic across CLI, web, and UI. The narrowed fresh start needs a local-first CLI/library with strong file I/O, YAML parsing and formatting, schema validation, CrowdAnki/CSV adapters, deterministic testing, and possible future integration with Anki or a Rust UI stack.

## Decision

Use Rust for the core library and CLI. The design should favor a reusable core crate so future frontends—such as a GUI/web surface through the Rust ecosystem—and possible Anki integration can share the same deterministic deck federation logic.

## Rationale

Rust keeps strong types and deterministic behavior while fitting the practical adapter and packaging needs better than Gren. It also aligns better with Anki's own Rust direction and leaves room for native, web, or extension-style frontends later.

## Implications

- ADR-001 is superseded for the fresh start.
- Core federation logic should live in a library crate, not be trapped inside CLI command handlers.
- Frontend/UI work remains deferred; Rust is chosen now for the core and CLI foundation.
