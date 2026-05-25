# ADR-030: Review Suggested Stable IDs During Import

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CrowdAnki imports contain adapter IDs and names but not Brain Brew stable IDs. The importer can generate readable ID suggestions, but accepting those suggestions silently would turn guesses into long-lived canonical identity.

## Decision

CrowdAnki import uses an interactive prompt to let maintainers accept or fix suggested stable IDs. In non-interactive mode, import fails with the suggestions unless the user explicitly passes `--accept-suggested-ids` or provides a reviewed mapping file.

## Rationale

Stable IDs are part of the maintainer-owned source contract. Interactive review makes first-time import humane, while non-interactive failure prevents CI or scripts from accidentally canonizing bad IDs. The explicit flag remains available for fixtures and trusted imports.

## Implications

- Import must support both interactive and non-interactive execution paths.
- Suggested IDs are not canonical until accepted or mapped.
- Golden tests can use explicit mappings or `--accept-suggested-ids` where appropriate.
