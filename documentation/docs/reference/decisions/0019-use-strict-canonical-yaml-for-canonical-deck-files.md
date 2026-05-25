# ADR-019: Use Strict Canonical YAML for Canonical Deck Files

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CanonicalDeck files are maintainer-owned source files. They need to be pleasant for humans to edit, especially for card templates, styling, and structured deck metadata, while still supporting byte-stable canonicalized round trips.

## Decision

Use a strict, schema-validated YAML subset for CanonicalDeck files, paired with a canonical formatter. The YAML accepted by Brain Brew should avoid ambiguous YAML features and should round-trip through the formatter deterministically. The canonical formatter may remove YAML comments; durable maintainer-facing explanations should be represented as modeled data rather than comments.

## Rationale

YAML is friendlier than JSON for large human-authored deck files with multiline content, but unrestricted YAML is too loose for reliable tooling. A constrained YAML subset gives maintainers a readable source format while preserving stable diffs and validation.

## Implications

- ADR-008 still applies to API/data exchange where JSON is useful, but CanonicalDeck source files use YAML.
- The formatter is part of the source contract, not an optional convenience.
- Tests should include byte-stable canonical YAML fixtures.
