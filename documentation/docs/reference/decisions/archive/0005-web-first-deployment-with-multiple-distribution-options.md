# ADR-005: Web First Deployment with Multiple Distribution Options

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Need to balance ease of use for casual users with power-user requirements for automation and self-hosting.

## Decision

Prioritize a self-contained web application that runs entirely in the browser, with additional deployment options for different use cases.

## Rationale

**Pros:**
- **Zero installation**: Users can try immediately without setup
- **Universal access**: Works on any device with a browser
- **Self-contained**: No server dependencies for basic usage
- **Flexible deployment**: Same codebase works for all scenarios
- **Business model**: SaaS offering possible with hosted sync service

**Deployment Options:**
1. **Standalone web app**: Runs entirely in browser, no server needed
2. **CLI tool**: For automation and power users
3. **Self-hosted server**: For teams and persistent sync
4. **SaaS offering**: Hosted sync service for convenience

**Cons:**
- **Browser limitations**: File system access requires user interaction
- **Offline sync**: More complex without persistent background process

## Alternatives Considered

- **Desktop application**: Platform-specific builds and maintenance
- **Server-only**: Barriers to entry for casual users
- **CLI-only**: Limited appeal to non-technical users

## Implications

- Web interface becomes primary user experience
- File I/O must work through browser APIs and user interaction
- Need clear upgrade path from web app to more advanced deployments
- Server sync becomes premium/convenience feature
