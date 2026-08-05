# ADR-005: Store Maintainer Source as Strict Canonical YAML

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

> **Amendment:** ADR-022 keeps strict canonical YAML as the root Canonical Deck File and native source format, but permits explicitly declared read-only Authoring Sources such as CSV to own disjoint canonical paths.

## Context

Deck maintainers need source files that are easy to review, version, canonicalize, and export through adapter formats. Brain Brew should preserve intentional deck information, but it should not promise to preserve arbitrary input bytes, hand formatting, unsupported adapter payloads, or source ownership recovery from an adapter export.

## Decision

Store the root Canonical Deck File and native maintainer-owned source as strict canonical YAML. ADR-022 permits that root to declare read-only Authoring Sources for disjoint canonical paths.

Canonical YAML is schema-driven and deterministic. Formatting produces byte-stable canonicalized source, not arbitrary original source bytes. A federated package may contain multiple source files such as a base deck, overlays, a manifest, a lockfile, and declared external source inputs, but each native Canonical Deck YAML file has one canonical representation.

## Rationale

**Pros:**

- YAML is readable and reviewable for deck maintainers.
- Deterministic serialization keeps diffs meaningful.
- Strict decoding catches mistakes before export.
- Canonicalization gives deterministic source stability without preserving irrelevant formatting.

**Cons:**

- Users cannot rely on custom YAML formatting surviving a format pass.
- Strict schemas require migrations when the source format evolves.
- YAML still needs careful validation to avoid ambiguous values.

## Alternatives Considered

- **JSON source**: rejected for maintainer source because it is noisier to edit by hand, though JSON may still be useful at integration boundaries.
- **Protobuf or binary source**: rejected because it is not reviewable as deck source.
- **Preserve original input formatting**: rejected because it makes adapters and canonicalization much harder while providing little semantic value.
- **Multiple loosely-defined source files for one deck**: rejected because it obscures the canonical source of truth.

## Implications

- YAML codecs live in `brain-brew-formats`, not `brain-brew-core`.
- Format commands should normalize source deterministically.
- Tests should assert canonicalized byte stability for formatter output and explicit adapter-equivalence boundaries.
- Documentation should distinguish Canonical Deck source from generated adapter artifacts.
