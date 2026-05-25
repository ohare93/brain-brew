---
title: Stable IDs and adapter IDs
---

# Stable IDs and adapter IDs

Brain Brew separates deck identity from external-tool identity.

## Stable IDs

Stable IDs are maintainer-owned names for deck entities:

```yaml
notes:
  note.finland:
    note_type_id: note-type.ultimate-geography
```

They are used by overlays, diffs, manifests, and tests. They should be readable and stable across releases.

Good stable IDs:

- `note.finland`
- `field.capital`
- `template.country-map`
- `media.flag.finland`

## Adapter IDs

Adapter IDs preserve identity in external tools such as Anki/CrowdAnki:

```yaml
notes:
  note.finland:
    adapter_ids:
      crowdanki:guid: abc123
```

A translation overlay may map external IDs when a legacy translated target already has different Anki GUIDs:

```yaml
id: overlay.translation.de
kind: translation
translations:
  adapter_ids:
    crowdanki:guid:
      en-guid: de-guid
```

## Why keep both?

Stable IDs make source review pleasant. Adapter IDs keep round trips compatible with existing exported decks.

A semantic diff therefore reports stable paths:

```text
~ notes.note.finland.fields.field.capital
  - Helsinki
  + Helsingfors
```

The path remains meaningful even when external GUIDs differ by language.
