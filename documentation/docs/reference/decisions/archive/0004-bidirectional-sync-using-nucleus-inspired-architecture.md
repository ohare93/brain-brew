# ADR-004: Bidirectional Sync Using Nucleus Inspired Architecture

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Users want to edit notes in either their note-taking system (markdown) or Anki, with changes syncing bidirectionally while preserving Anki's review history.

## Decision

Implement bidirectional sync using an architecture inspired by Dropbox's Nucleus model, with explicit conflict detection and resolution.

https://dropbox.tech/infrastructure/-testing-our-new-sync-engine

## Rationale

**Pros:**

- **Explicit conflict handling**: No silent data loss from "last write wins"
- **Atomic operations**: Both sides sync successfully or neither does
- **Audit trail**: Clear history of what changed and why
- **Proven approach**: Dropbox's model handles similar multi-source sync challenges
- **Preserves review history**: Anki note IDs maintained across syncs

**Cons:**

- **Complexity**: More complex than one-way conversion
- **Storage overhead**: Need to track sync metadata
- **User intervention**: Some conflicts require manual resolution

## Alternatives Considered

- **One-way sync only**: Simpler but limits user workflow flexibility
- **Last-write-wins**: Simple but causes data loss
- **Direct bidirectional sync**: Complex without intermediate state tracking

## Implications

- Need persistent storage for sync metadata
- Conflict resolution UI becomes important user experience
- Change detection mechanisms required for all adapters
- Checksums/hashing needed for content comparison
