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

Without a media root, Brain Brew can check that references are internally consistent.

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets
```

Referenced-but-undeclared media fails verification. Declared-but-unreferenced media is reported as a warning.

With `--media-root`, it also checks that files exist and declared hashes are non-empty and current:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

## Refresh hashes

After intentionally editing a media file, update source state with:

```bash
brainbrew media hash --manifest brainbrew.yaml --all-targets --media-root media/
```

The command writes missing or stale SHA-256 values into deck or overlay source YAML through the include-preserving formatter, so `!include`-bearing sources keep their include structure.

## CrowdAnki import

CrowdAnki import suggests stable media IDs from media file paths. If two different media paths would derive the same suggested stable ID, import fails closed and names both paths so the ambiguity can be resolved before accepting suggested IDs. Exact duplicate paths are deduplicated instead of treated as a collision. Use `import_deck_accept_suggested_ids`/`brainbrew import crowdanki --accept-suggested-ids` only after reviewing those suggested IDs.

## Export media

CrowdAnki export copies declared media into the export folder's `media/` subdirectory when a media root is supplied:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target en-standard \
  --media-root media/ \
  --out build/crowdanki/en-standard
```

Because export is authoritative for declared media, downstream scripts can drop blanket `cp media/*` steps. Undeclared files in the media root are not copied.

## Why references instead of embedded files?

Keeping assets external keeps source readable, makes hashing explicit, and avoids storing binary data in YAML.
