# ADR-018: Use Single Canonical Deck File as Source of Truth

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Ultimate Geography and Brain Brew use split CSVs and recipes as maintainer source, but supporting that layout first would make source-layout mechanics compete with the core federation model. A single CanonicalDeck file gives the fresh start a simpler source-of-truth boundary while still allowing CSV and CrowdAnki adapters.

## Decision

For the first milestone, the maintainer source of truth is a single CanonicalDeck file. CSV and CrowdAnki are adapter formats used to import, export, and prove round-trip behavior; they are not the primary source layout initially.

## Rationale

This keeps the first implementation focused on the deck model, overlay semantics, stable identity, and adapter fidelity. Split CSV source layouts can be added later once the canonical model is proven.

## Implications

- Byte-stable canonicalized source applies first to the CanonicalDeck file.
- CSV adapter support should not force the core source layout to mimic Ultimate Geography's derivative CSV structure.
- Brain Brew migration remains a later concern.
