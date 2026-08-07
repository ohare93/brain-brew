# ADR-011: Use a Local Deck Workbench Server with an Iced/WASM UI

**Date**: 2026-06-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Deck maintainers and translators need an ergonomic interface for reviewing and editing deck source in context. The interface must show notes, produced cards, source-language text, target-language text, translation status, and pending changes without making a web app or generated artifact the source of truth.

Brain Brew remains local-first: Canonical Deck files, manifests, and overlays are the canonical storage. A previous static HTML export was rejected because it was not an interactive workbench and did not match the intended workflow.

## Decision

Brain Brew will provide a **Deck Workbench** launched by `brainbrew workbench serve --manifest brainbrew.yaml`.

The command runs a local Rust server bound to `127.0.0.1` on an available port by default, opens the browser unless `--no-open` is supplied, serves an Iced/WASM frontend, and exposes JSON HTTP APIs for workspace data and apply operations. The browser UI is not allowed to write source files directly. All file access, validation, and YAML mutation happen server-side through Brain Brew semantics.

The first durable workbench capability centers on translation and source note-field editing in context. It supports language-first selection, note/card/source-string/card pivots, near-Anki previews, staged browser-local edits, apply previews, explicit Apply to canonical Translation Overlay YAML, and direct Canonical Deck File updates for existing source-language note field text.

## Rationale

**Pros:**

- Preserves Brain Brew source files as canonical storage.
- Gives translators a real interactive application instead of static generated HTML.
- Keeps local file write permissions in the Rust process, not browser code.
- Allows the UI to use Rust/Iced state architecture while still running in a browser.
- Keeps room for future Deck Workbench modes beyond translation and note-field text editing.

**Cons:**

- Adds a WASM frontend build and serving pipeline.
- Iced web/WASM support is a more experimental frontend choice than a conventional TypeScript framework.
- The server/frontend API must be versioned and tested.
- Browser local storage drafts require careful stale-file detection before Apply.

## Alternatives Considered

- **Static self-contained HTML export**: rejected because it is not an interactive file-oriented workbench and creates a clumsy draft/apply flow.
- **Native Iced desktop app first**: rejected for now because the desired workflow is a browser/webview-style local UI and because the browser is better suited to card-like previews and local storage.
- **Conventional Svelte/TypeScript frontend**: rejected by preference in favor of Iced/WASM, while retaining a Rust server API boundary.
- **Web app as source of truth**: rejected because Brain Brew source remains YAML/overlay-based canonical storage.

## Implications

- `brainbrew workbench serve` must be treated as a local file-editing process and bind locally by default.
- The Iced/WASM UI should live in a separate workspace crate from the CLI server.
- Workbench APIs should be JSON HTTP first, with file fingerprints/polling for stale-session detection.
- Apply operations must show a validation/preview step before writing source files.
- Direct Canonical Deck File editing should start with existing note field text only; structural deck authoring remains out of scope until separately designed.
