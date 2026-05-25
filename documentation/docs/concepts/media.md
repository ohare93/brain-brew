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

With `--media-root`, it also checks files and hashes:

```bash
brainbrew verify --manifest brainbrew.yaml --all-targets --media-root media/
```

## Export media

CrowdAnki export copies declared media into the export folder's `media/` subdirectory when a media root is supplied:

```bash
brainbrew export crowdanki \
  --manifest brainbrew.yaml \
  --target en-standard \
  --media-root media/ \
  --out build/crowdanki/en-standard
```

## Why references instead of embedded files?

Keeping assets external keeps source readable, makes hashing explicit, and avoids storing binary data in YAML.
