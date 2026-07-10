---
title: Diagnostic and error contracts
---

# Diagnostic and error contracts

Brain Brew domain diagnostics are independent of their human-readable `Display` text. Core callers should use `ValidationError::diagnostic()` or `ComposeError::diagnostic()` and branch on the stable code and typed metadata.

## JSON schema version 1

CLI domain failures and Workbench HTTP failures use an error object with `schema_version: 1`. The stable fields are:

- `code` and `category` at the envelope and diagnostic levels;
- `command` and `context` for CLI errors;
- typed `path`, `overlay`, `source`, `intent`, and `entity_kind` attribution;
- `expected`, `actual`, and `conflict.first`/`conflict.current` composition metadata;
- `field_graph` reference/cycle details;
- ordered `children` for final validation issues;
- `message`, which is supplemental display text and must not be parsed.

Workbench returns the same `error.schema_version`, `code`, `category`, `message`, and ordered `diagnostics` fields. Domain-backed composition and development-write validation failures use HTTP 422, invalid requests use 400, read-only mutation attempts use 403, and unexpected adapter failures use 500. Apply-preview success embeds the same version-1 diagnostic objects under `validation.diagnostics`; clients must not expect or reconstruct newline-joined `validation.errors` strings.

Ordering follows deterministic core validation/composition order. Object consumers must ignore unknown fields. A future incompatible envelope changes `schema_version`; adding optional fields does not.

## Stable codes

Composition codes: `missing_expected_base`, `invalid_expected_base`, `expected_base_mismatch`, `overlay_conflict`, `missing_overlay_target`, `entity_already_exists`, `missing_overlay_payload`, `missing_translation`, `stale_translation_entry`, `validation_failed`, and `tombstoned_address_reuse`.

Validation codes: `missing_note_type`, `unknown_note_field`, `missing_note_field`, `mismatched_entity_id`, `duplicate_field_definition`, `duplicate_card_template`, `invalid_message_reference`, `invalid_message_target_representation`, `message_dependency_cycle`, `invalid_stable_id`, `conflicting_field_representation`, and `unknown_media_reference`.

## Core compatibility and migration

This change is source-breaking for callers that construct `ValidationError` or `ComposeError` with struct literals: both types gained typed diagnostic fields. Their existing `kind`, `path`, and `message` fields remain available. `ComposeErrorKind::ValidationFailed` remains for compatibility, but final composition now emits one parent error whose `validation_errors` owns every original `ValidationError`; callers must iterate those children rather than parse the parent message.

Before:

```rust
if error.kind == ComposeErrorKind::ValidationFailed {
    // Do not inspect error.message.
}
```

After:

```rust
if error.kind == ComposeErrorKind::ValidationFailed {
    for issue in &error.validation_errors {
        eprintln!("{} at {}", issue.kind.code(), issue.path);
    }
}

let diagnostic = error.diagnostic();
// Serialize or render this projection in an adapter crate.
```

Core remains format-independent and has no serde, HTTP, terminal, filesystem, or CLI dependency.
