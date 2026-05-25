# ADR-028: Defer Recipe System Until CLI Semantics Stabilize

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

ADR-003 proposed a Nix-inspired declarative recipe system. The fresh start still values repeatable workflows, but the first milestone must prove CanonicalDeck, overlays, adapters, and semantic diffing before freezing a recipe language around them.

## Decision

Defer the user-facing recipe/manifest system beyond the first milestone. Initial workflows use explicit CLI arguments. Recipes can be introduced once command semantics and domain concepts have stabilized.

## Rationale

A recipe language too early would encode guesses about compose/export/import behavior. CLI arguments are less ergonomic but make the first implementation smaller and easier to revise.

## Implications

- ADR-003 is deferred for the fresh start, not abandoned forever.
- The first milestone should avoid designing a full workflow DSL.
- Tests may still use helper code or fixture conventions, but these are not a public recipe language.
