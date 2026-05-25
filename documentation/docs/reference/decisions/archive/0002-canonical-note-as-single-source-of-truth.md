# ADR-002: Canonical Note as Single Source of Truth

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Need to design data flow between multiple format sources (markdown, Anki, CSV, etc.) while maintaining correctness and avoiding complex N×N translations.

## Decision

Use a single CanonicalNote format as the hub, with all other formats as spokes that translate to/from this canonical representation.

## Rationale

**Pros:**
- **Simplified architecture**: Only need N adapters instead of N×N converters
- **Single source of truth**: Server/sync engine only understands one format
- **Extensibility**: Adding new formats requires only one new adapter
- **Type safety**: All transformations go through typed canonical format
- **Testing**: Can test each adapter independently

**Cons:**
- **Potential information loss**: Some format-specific features may not translate
- **Abstraction overhead**: Extra translation step for direct format-to-format conversion

## Alternatives Considered

- **Direct format translation**: Complex N×N problem
- **Multiple canonical formats**: Defeats the purpose of unification
- **Format-aware server**: Server would need to understand all formats

## Implications

- All adapters must implement to/from CanonicalNote conversion
- Server logic simplified to single data model
- Format-specific features must be represented in canonical format or metadata
- Adapter quality becomes critical for user experience
