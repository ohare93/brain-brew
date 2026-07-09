---
title: Introduce a typed versioned Workbench server UI contract
priority: critical
---

## Goal

Replace ad hoc JSON values and silent client defaults with shared DTOs, enums, version negotiation, and one error envelope.

## Acceptance Criteria

- Server and independently compiled UI share or generate request/response contract types
- All routes include an explicit contract version
- Unknown/missing fields fail visibly rather than defaulting silently
- API 404 and failures use one typed error envelope
- Request/body/list limits are part of the interface and contract-tested

## Implementation Notes

Coordinate typed core errors; establish before changing Apply or detail routes.
