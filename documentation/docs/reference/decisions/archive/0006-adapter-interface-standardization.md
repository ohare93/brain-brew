# ADR-006: Adapter Interface Standardization

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Multiple format adapters need consistent interface while allowing format-specific optimizations and features.

## Decision

Define a standard adapter interface that all formats must implement, with HTTP API endpoints for external adapters.

## Rationale

**Pros:**
- **Consistency**: All adapters work the same way from user perspective
- **Testability**: Standard interface enables comprehensive testing
- **Extensibility**: Third parties can implement adapters in any language
- **Language agnostic**: External adapters communicate via HTTP/JSON
- **Modularity**: Adapters can be developed and released independently

**Standard Interface:**
```gren
type alias Adapter =
    { name : String
    , pull : () -> Task Error (List CanonicalNote)
    , push : List CanonicalNote -> Task Error ()
    , detectChanges : Time.Posix -> Task Error (List NoteId)
    }
```

**Cons:**
- **Abstraction overhead**: May limit format-specific optimizations
- **HTTP overhead**: External adapters have network latency

## Alternatives Considered

- **Format-specific interfaces**: Would create inconsistent user experience
- **Plugin architecture**: More complex than HTTP APIs
- **Library-only approach**: Limits language choices for adapters

## Implications

- All built-in adapters must implement standard interface
- External adapter documentation and examples needed
- HTTP API design becomes critical for third-party adoption
- Adapter marketplace/registry possible in the future
