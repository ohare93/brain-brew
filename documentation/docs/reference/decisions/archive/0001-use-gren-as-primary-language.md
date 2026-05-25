# ADR-001: Use Gren as Primary Language

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Need to choose a primary language for Brain Brew that can run in multiple environments (CLI, browser, server) while providing strong guarantees for data transformation correctness.

## Decision

Use Gren as the primary language for core logic, adapters, and user interfaces.

## Rationale

**Pros:**
- **Cross-platform deployment**: Same codebase runs in browser (web app), command line (CLI), and potentially server
- **Type safety**: Compile-time guarantees prevent data corruption during format transformations
- **Functional programming**: Perfect fit for transformation pipelines and sync logic
- **Elm heritage**: Proven architecture pattern for complex state management
- **No runtime dependencies**: Users don't need Node.js, Python, etc.

**Cons:**
- **Smaller ecosystem** compared to mainstream languages
- **Learning curve** for contributors unfamiliar with functional programming
- **Limited libraries** for some integrations

## Alternatives Considered

- **TypeScript/JavaScript**: Better ecosystem but no compile-time guarantees
- **Rust**: Great performance but complex for newcomers, doesn't run in browser
- **Python**: Excellent for data processing but runtime errors and deployment complexity
- **Elixir**: Great for distributed systems but doesn't compile to browser

## Implications

- All core transformation logic must be implemented in Gren
- External integrations handled via HTTP APIs and JSON
- Team needs Gren expertise or training
- Protobuf abandoned in favor of JSON for simplicity
