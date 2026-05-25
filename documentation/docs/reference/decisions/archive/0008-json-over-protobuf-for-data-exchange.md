# ADR-008: JSON Over Protobuf for Data Exchange

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Need efficient, type-safe data exchange format for APIs and configuration files.

## Decision

Use JSON for all data exchange, with Gren's type-safe JSON decoders for validation.

## Rationale

**Pros:**
- **Universal support**: Every language and tool supports JSON
- **Human readable**: Easy to debug and manually edit
- **Gren native**: Built-in JSON support with excellent error messages
- **Single language**: No need to learn protobuf schema language
- **Tooling**: Extensive JSON ecosystem for validation, formatting, etc.

**Cons:**
- **Larger payload**: Less compact than protobuf
- **No schema evolution**: Breaking changes require version coordination

## Alternatives Considered

- **Protobuf**: Better performance but adds complexity and tooling requirements
- **MessagePack**: More compact but less ecosystem support
- **YAML**: Good for configuration but not APIs

## Implications

- All adapters communicate via JSON APIs
- Recipe files use YAML (human-friendly) but convert to JSON internally
- Comprehensive JSON schema validation needed
- API versioning strategy required for future changes
