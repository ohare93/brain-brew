---
title: Add typed CSV media-reference fields
priority: high
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-4
---

## Goal

Let CSV-backed notes create the same structured image values as `!image` YAML fields while retaining media.yaml as the authority for paths and hashes.

## Acceptance Criteria

- Add an explicit image field mapping type whose non-empty CSV cells contain stable media IDs and materialize FieldValue::Images
- Treat an empty image cell as the ordinary empty scalar field required by the note type
- Reject unknown, malformed, tombstoned, or otherwise invalid media references through existing canonical validation
- Preserve existing media declarations, path ownership, hash verification, export lowering, and semantic distinction between raw HTML scalars and structured images
- Add fixtures proving successful flag/map references, missing media errors, hash verification, empty images, and unchanged CrowdAnki lowering
- Document the recommended one-time source normalization from `<img>` HTML cells to media IDs rather than adding an HTML parser
- Develop with red-green-refactor and leave focused regression tests

## Design Decisions

- CSV stores stable media IDs, not YAML tags and not rendered `<img>` HTML
- media.yaml remains the sole path/hash declaration
- Single image references are sufficient initially; do not invent a multi-image cell encoding without a concrete consumer

## Implementation Notes

Depends on note materialization; may land before or after explicit joins. Reuse FieldValue::Images, FieldImageReference, existing media binding, and release-media integrity paths.
