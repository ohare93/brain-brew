# ADR-007: Content Based Identification for Sync

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Need to link notes between different formats for bidirectional sync while avoiding polluting markdown files with Anki IDs.

## Decision

Use content-based identification (hashing) as primary method, with optional hidden ID injection for preserving Anki review history.

## Rationale

**Primary Strategy - Content Hashing:**
- **Clean notes**: No ID pollution in markdown files
- **Automatic linking**: Notes automatically linked by content similarity
- **Version controllable**: Markdown files remain readable and git-friendly

**Fallback Strategy - Hidden IDs:**
```markdown
# What is ATP? <!-- anki:1234567890 -->
Adenosine triphosphate
```

**Pros:**
- **User choice**: Users can choose clean vs. review-history-preserving approach
- **Graceful degradation**: Content changes break link but don't corrupt data
- **Flexibility**: Different strategies for different use cases

**Cons:**
- **Review history loss**: Content changes break links to Anki review data
- **Complexity**: Need to handle both identification strategies

## Alternatives Considered

- **Visible IDs**: Too intrusive for note-taking experience
- **Separate mapping files**: Complex to maintain, can drift out of sync
- **UUID generation**: No natural linking between formats

## Implications

- Sync engine must handle both identification strategies
- Users need clear understanding of trade-offs
- Content normalization important for stable hashing
- Migration tools needed for switching between strategies
