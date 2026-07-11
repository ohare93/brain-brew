---
title: Media references
---

# Media references

Media assets are external files. Canonical Deck YAML stores references to those files and their hashes.

## Declare media

```yaml
media:
  media.flag.finland:
    path: flags/fi.svg
    sha256: 7b2b...
```

Field text, template text, and note-type styling can then use normal Anki-compatible references (`<img src="...">`, `<script src="...">`, `<link href="...">`, CSS `url(...)`, and `[sound:...]`):

```yaml
notes:
  note.finland:
    fields:
      field.flag: '<img src="flags/fi.svg" />'
note_types:
  note-type.country:
    styling: |
      @import url("css/maps.css");
```

## Verify media

Targets without media verify normally. For any target with declared or referenced media, `verify` is release-strict by default: referenced-but-undeclared media fails, unused declarations warn, every owning package needs an explicit `--media-root`, hashes must be canonical 64-character lowercase SHA-256, and every file must exist and match.

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

Hashless fixtures that intentionally test only source structure must opt into clearly non-release development behavior:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets \
  --media-mode reference-only
```

Reference-only mode still validates declarations, references, collisions, safe paths, and any non-empty hash syntax. Human and JSON output prominently report that it is not release-ready. A missing root never selects this mode implicitly.

## Refresh hashes

After intentionally editing a media file, update source state with:

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
```

The command writes missing or stale SHA-256 values into deck or overlay source YAML through the include-preserving formatter, so `!include`-bearing sources keep their include structure.

## CrowdAnki import

CrowdAnki import plans suggest stable media IDs from media file paths. Exact duplicate paths are deduplicated. Different paths that suggest the same ID are recorded as `requires_override` entries with their exact JSON locations; maintainers select distinct legal IDs in the reviewed import plan before apply. The same versioned plan records note, note-type, field, and template evidence, and apply verifies it against the unchanged source. See [Import CrowdAnki](../authoring/importing-crowdanki.md).

## Export media

Strict CrowdAnki export copies validated declared media into the export folder's `media/` subdirectory and requires the owner roots for every media target:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target en-standard \
  --media-root media/ \
  --out build/crowdanki/en-standard
```

Because export is authoritative for declared media, downstream scripts can drop blanket `cp media/*` steps. Undeclared files in the media root are not copied. `--media-mode reference-only` may produce a development `deck.json` without copying bytes, but still rejects undeclared references and path/output collisions and reports `NOT RELEASE-READY`.

## Why references instead of embedded files?

Keeping assets external keeps source readable, makes hashing explicit, and avoids storing binary data in YAML.
