# ADR-003: Recipe System Based on Nix Philosophy

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Users need a way to define complex, repeatable workflows for converting and processing notes. Brain Brew's recipe system was powerful but complex.

## Decision

Implement a declarative recipe system inspired by Nix's functional, reproducible approach, using YAML syntax for accessibility.

## Rationale

**Pros:**
- **Reproducible builds**: Same inputs always produce same outputs
- **Composable**: Transformation functions can be reused and combined
- **Declarative**: Users describe what they want, not how to achieve it
- **Reversible**: Recipes can be automatically inverted (inputs ↔ outputs)
- **Version controllable**: YAML files work well with git
- **Familiar syntax**: YAML more accessible than learning Nix language

**Cons:**
- **Learning curve**: Functional thinking may be unfamiliar to some users
- **Limited expressiveness**: May not handle all edge cases without custom functions

## Alternatives Considered

- **Imperative scripts**: More flexible but not reproducible
- **Native Nix files**: Too complex for average users
- **JSON configuration**: Less human-readable than YAML
- **GUI-only**: Not suitable for automation and version control

## Implications

- All transformations must be pure functions
- Recipe validation becomes critical user experience
- Need visual recipe builder for less technical users
- Caching/memoization opportunities for unchanged steps
