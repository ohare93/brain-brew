# ADR-015: Use Lazy Single-Work-Item Workbench Editing

**Date**: 2026-06-24  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The Deck Workbench must remain usable on Ultimate Geography-sized decks. The first Workbench slices rendered note, card, source-string, and optional-metadata pivots as large editable surfaces. That made simple actions such as changing the active language expensive because the browser had to fetch large JSON payloads, build many DOM nodes, render many card previews/media references, and attach many event handlers before the user could continue.

Maintainers also expect the Workbench to behave as a focused editor: use a pivot to find the next work item, then edit one Note, Card, Source String, or optional metadata item at a time while seeing enough source and multilingual context to make a correct decision. Rendering all editable rows for an entire pivot does not match that mental model and does not scale.

## Decision

The Deck Workbench will use a **lazy single-work-item editing model**.

- Pivot views are compact, paginated navigation lists. They are used to find and select work items, not to render every editable field as one large page.
- Editing happens in a selected-item detail pane. At most one Note, Card, Source String, or optional metadata item is the primary editable item at a time.
- Multilingual/source context is lazy and selected-item scoped. The UI may show source language text, the active target language, pinned comparison languages, or languages with missing/stale values for the selected item, but it must not render all languages for all items.
- Secondary pivots and context panes load only when explicitly requested or when needed for the selected item. Language/target/overlay switches should provide fast visible feedback and must not hydrate unrelated pivot data in the background.
- Browser-local staged edits remain independent of what is currently mounted in the DOM. Unmounting a row or selecting another item must not lose staged edits.
- Apply remains explicit. Apply Preview and Confirm Apply collect all staged edits, show affected files, validation results, and grouped file/content-group changes, then write canonical YAML only after confirmation.

The Workbench explicitly rejects rendering full Note/Card/Source String/optional metadata pivots as one large editable webpage for UG-sized decks.

## API Boundaries

Initial APIs should split **navigation list** data from **selected detail** data.

Navigation endpoints return compact rows only:

- `GET /api/workbench/note-list`
- `GET /api/workbench/card-list`
- `GET /api/workbench/source-string-list`
- `GET /api/workbench/optional-metadata-list`

List responses include the active language/target/overlay/filter selection, deck-wide progress totals, `total`, `limit`, `offset`, `has_more`, and compact row summaries. They must not include every editable field, card preview, or every occurrence body unless that data is necessary for navigation.

Detail endpoints return one selected editable work item plus lazy context:

- `GET /api/workbench/note-detail?note=<stable-note-id>`
- `GET /api/workbench/card-detail?card=<stable-card-id>`
- `GET /api/workbench/source-string-detail?source=<encoded-source>`
- `GET /api/workbench/optional-metadata-detail?path=<encoded-path>`

Detail responses include editable rows for the active item, source previews/card previews where relevant, selected-item context, and enough stable identity to stage edits. Comparison language context may be a nested detail response or a separate selected-item context endpoint, but it remains scoped to the selected item.

Existing full pivot endpoints may remain temporarily for migration, but new user-visible Workbench work should target the list/detail API shape.

## Pagination

List endpoints use bounded pagination:

- `limit` defaults to a small page size suitable for UG-sized decks.
- `offset` selects the first row in the current page.
- The server enforces a maximum `limit` and returns clear `400` errors for invalid pagination parameters.
- Deck-wide progress and coverage totals remain totals for the selected language/target/overlay, not totals for only the current page.

The browser may implement `Load more`, page controls, or virtual scrolling over these bounded list pages.

## Staged Edit Keys

Staged edits are browser-local until Apply and must survive row unmounting, pagination, filter changes, and item navigation. They should be keyed by stable scope and source identity, for example:

```text
language + target + overlay + edit-kind + stable-path + source-key
```

Source edits also include the source scope and impact action. Target translation edits include mode (`direct`, `contextual`, `no_change`) and the effective source key used for Apply. Secondary comparison panes stage edits with their own language/target/overlay scope.

When a row or detail pane mounts, it reads the staged-edit map and overlays any existing staged value on top of server data. Apply Preview and Confirm Apply collect from the central staged-edit map, not from only currently visible DOM nodes.

## Stale Response Handling

Workbench fetches carry a freshness token/generation derived from the active selection. When a user changes language, target, overlay, filter, or selected item, outstanding responses from previous generations must be ignored if they arrive late. Actual network request cancellation is optional; stale-response protection is required.

This prevents a slow German response from overwriting the UI after the user has already switched to French or another selected item.

## Testing Implications

Per ADR-014, user-visible Workbench slices require both API integration tests and browser E2E tests. For this architecture, tests should cover:

- list endpoint pagination, invalid parameters, and deck-wide totals;
- detail endpoint loading for one selected item;
- staged edits surviving navigation away from and back to an item;
- Apply Preview/Confirm Apply collecting staged edits that are not currently visible;
- browser E2E proving language switches do not eagerly request unrelated pivots;
- browser E2E enforcing bounded DOM row counts for the Ultimate Geography fixture.

## Rationale

**Pros:**

- Keeps language/target/overlay switches responsive on large decks.
- Matches the expected workflow: navigate by pivot, edit one work item, inspect selected context.
- Bounds JSON payloads, DOM nodes, media previews, and event-handler wiring.
- Preserves canonical YAML and explicit Apply semantics.
- Creates smaller API seams that are easier to test and evolve.

**Cons:**

- Requires API migration from full pivot payloads to list/detail endpoints.
- Requires careful staged-edit key design because the edited item may not remain mounted.
- Adds stale-response bookkeeping to browser fetch paths.
- Users may need explicit controls to load broader context when desired.

## Alternatives Considered

- **Keep full editable pivot pages and only optimize rendering:** rejected because it still sends and represents more data than the user intends to edit.
- **Virtual-scroll the existing full pivot payloads:** useful as an implementation technique, but insufficient because full payload transfer and all-item edit semantics remain.
- **Auto-load every secondary pivot in the background:** rejected because it makes simple selection changes slow and unpredictable on UG-sized decks.
- **Make a web database the source of truth:** rejected by ADR-011; canonical YAML/manifests/overlays remain the source of truth.

## Implications

- Workbench implementation should migrate in stages: lazy secondary pivot loading, paginated navigation lists, selected-item detail panes, central staged-edit collection, and request freshness guards.
- Existing full pivot endpoints are compatibility scaffolding, not the target architecture.
- Browser E2E should prefer bounded DOM/payload assertions over brittle wall-clock timing assertions.
- Documentation and UI copy should describe pivots as navigation aids and detail panes as the editing surface.
