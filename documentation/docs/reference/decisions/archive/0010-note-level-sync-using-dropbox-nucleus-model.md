# ADR-010: Note Level Sync Using Dropbox Nucleus Model

**Date**: 2025-08-06  
**Status**: Accepted  
**Deciders**: Project Lead  

## Context

Building upon [ADR-004: Bidirectional Sync Using Nucleus Inspired Architecture](0004-bidirectional-sync-using-nucleus-inspired-architecture.md), this ADR specifies the detailed implementation of note-level sync.

The sync server needs to handle bidirectional synchronization between different source formats (markdown files, Anki decks, CSV files) where:

1. **Many-to-many relationships exist** - One markdown file contains multiple notes, one Anki deck contains notes from multiple markdown files
2. **Different organizational structures** - Markdown organizes by files, Anki by decks, CSV by rows
3. **Granular conflict resolution needed** - Individual notes may conflict, not entire collections
4. **Cross-source updates required** - Changing one note should update it everywhere it appears

Traditional file-level sync models don't work because the "sync unit" (individual notes) exists within larger containers (files, decks) that don't align across formats.

## Related Decisions

- [ADR-004: Bidirectional Sync Using Nucleus Inspired Architecture](0004-bidirectional-sync-using-nucleus-inspired-architecture.md) - Establishes the high-level architectural decision for bidirectional sync that this ADR implements in detail

## Decision

Implement **note-level sync using Dropbox's Nucleus model**, where each CanonicalNote is treated as an individual sync unit with its own three-way merge state.

**Architecture:**
- **Server storage**: Each note stored as individual entity (file or DB record)
- **Sync metadata**: Per-note sync trees tracking local/remote/server states
- **Conflict resolution**: Dropbox three-way merge applied per note
- **Cross-source propagation**: Resolved notes pushed to all sources containing them

## Rationale

### Server Storage Structure

```
server/
├── notes/
│   ├── biology_001.json        # Individual CanonicalNote storage
│   ├── biology_002.json
│   └── chemistry_001.json
└── sync_metadata/
    ├── biology_001_sync.json   # Per-note sync state
    ├── biology_002_sync.json
    └── chemistry_001_sync.json
```

### Sync State Per Note

```gren
type alias NoteSyncTree =
    { noteId : NoteId
    , localState : Maybe CanonicalNote      -- Last known from source A (e.g., markdown)
    , remoteState : Maybe CanonicalNote     -- Last known from source B (e.g., Anki)  
    , serverState : CanonicalNote           -- Server canonical version
    , sources : Dict SourceId SourcePresence -- Where this note appears
    , lastSync : Dict SourceId Time.Posix   -- Last sync per source
    }

type alias SourcePresence =
    { location : NoteLocation
    , lastSeen : Time.Posix
    , checksum : String
    }

type NoteLocation
    = MarkdownLocation { file : String, lineStart : Int, lineEnd : Int }
    | AnkiLocation { deck : String, noteId : String }
    | CsvLocation { file : String, row : Int }
```

### Sync Process

```gren
-- 1. Sources report their current note states
scanSource : SourceId -> Task Error (Dict NoteId CanonicalNote)

-- 2. For each note, apply Dropbox three-way merge
syncNote : NoteSyncTree -> CanonicalNote -> CanonicalNote -> SyncResult
syncNote syncTree newLocalNote newRemoteNote =
    case (hasChanged syncTree.localState newLocalNote, 
          hasChanged syncTree.remoteState newRemoteNote) of
        (False, False) -> NoChange
        (True, False) -> LocalToServer newLocalNote
        (False, True) -> RemoteToServer newRemoteNote
        (True, True) -> Conflict newLocalNote newRemoteNote syncTree.serverState

-- 3. Push resolved notes back to all sources containing them
propagateNote : NoteId -> CanonicalNote -> List SourceId -> Task Error ()
```

### API Design

```json
// Source scans and reports all its notes
POST /sync/scan
{
  "source_id": "biology.md",
  "source_type": "markdown",
  "notes": [
    {
      "id": "biology_001", 
      "canonical_note": {...},
      "location": { "file": "biology.md", "line_start": 10, "line_end": 15 },
      "checksum": "abc123"
    }
  ]
}

// Server responds with sync operations per note
{
  "operations": [
    {
      "note_id": "biology_001",
      "action": "pull", 
      "canonical_note": {...}
    },
    {
      "note_id": "biology_002", 
      "action": "conflict",
      "local_version": {...},
      "remote_version": {...},
      "server_version": {...}
    }
  ]
}
```

**Pros:**
- **Proven sync model** - Dropbox Nucleus has solved the three-way merge problem at scale
- **Granular resolution** - Only conflicted notes require user intervention
- **Format agnostic** - Works regardless of source organization (files vs decks)
- **Many-to-many support** - Notes can exist in multiple sources naturally
- **Atomic operations** - Each note sync is independent and atomic
- **Clear conflict semantics** - Well-understood three-way merge rules per note

**Cons:**
- **Storage overhead** - Each note requires its own sync metadata
- **Complexity** - More complex than simple file-level sync
- **Cross-source updates** - One change may trigger updates in multiple sources
- **Identity management** - Requires robust note ID generation and matching

## Alternatives Considered

**Collection-Level Sync:**
```gren
-- Sync entire files/decks as units
sync markdown_file ↔ anki_deck
```
*Rejected because collections don't align across formats (one deck may contain notes from multiple files).*

**Simple Last-Write-Wins:**
```gren
-- No conflict resolution, newest change wins
if newer(note_from_anki) then use_anki_version else use_markdown_version
```
*Rejected because it causes data loss and provides no conflict resolution.*

**Manual Collection Mapping:**
```yaml
# Require users to explicitly map sources
sync_mappings:
  - markdown: "biology.md" ↔ anki: "Biology Deck"
  - markdown: "chemistry.md" ↔ anki: "Chemistry Deck"  
```
*Rejected because it's too rigid and doesn't handle many-to-many relationships.*

**Note-Per-File Storage:**
```
-- Store each note as separate file on disk
notes/biology_001.md
notes/biology_002.md
```
*Rejected because it breaks natural workflow and explodes file count.*

## Implications

**Server Implementation:**
- Must store individual CanonicalNotes with unique IDs
- Per-note sync metadata tracking local/remote/server states
- Three-way merge conflict resolution engine
- Cross-source note propagation system

**Source Adapters:**
- Must extract individual notes from collections (files, decks)
- Report note locations within source containers
- Accept individual note updates and merge back into collections
- Handle note creation/deletion within existing containers

**Conflict Resolution:**
- User interface for resolving note-level conflicts
- Merge strategies for different field types (text, media, etc.)
- Audit trail of conflict resolutions

**Identity Management:**
- Consistent note ID generation across sources
- Content-based hashing for automatic note matching
- Fallback to explicit ID mapping when needed

**Performance Considerations:**
- Batch operations for scanning large sources
- Incremental sync based on timestamps/checksums  
- Efficient storage and retrieval of individual notes
- Caching of sync metadata for frequently accessed notes

**Migration Path:**
- Existing Brain Brew workflows can be converted to this model
- Initial sync may require matching existing notes across sources
- Gradual adoption possible (start with single-source, add sync later)