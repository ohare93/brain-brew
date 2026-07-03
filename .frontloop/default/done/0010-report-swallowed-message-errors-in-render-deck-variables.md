---
title: Report swallowed structured-message errors in render_deck_variables
priority: high
---

## Goal

`render_deck_variables` must fail when structured-message resolution fails after variable substitution, instead of silently returning a deck with broken/empty messages.

## Problem

In `crates/brain-brew-core/src/lib.rs` (~line 3623), the success branch does:

```rust
if errors.is_empty() {
    let mut message_errors = Vec::new();
    resolve_structured_messages_with_validation_errors(&mut rendered, &mut message_errors);
    Ok(rendered)
}
```

`message_errors` is populated by the resolver but never inspected, so any message that fails to resolve post-substitution (e.g. a `ref` to a nonexistent field produced via a variable) is silently dropped. Everywhere else in the pipeline this resolver's errors are surfaced (compose errors, validation errors); this is the only call site that discards them.

## Acceptance Criteria

- `render_deck_variables` returns an error when `resolve_structured_messages_with_validation_errors` reports any error.
- The error output identifies the offending path/message, consistent with how compose/validate report the same failures.
- Regression test: a deck whose variable expansion yields an unresolvable structured message causes `render_deck_variables` to fail; the same deck with the variable corrected succeeds.
- `cargo test --workspace` passes.

## Design Decisions

- Whether to extend `VariableRenderReport` with the existing validation-error type or add a distinct variant is left to the implementor; prefer whichever keeps error display consistent.

## Implementation Notes

- Callers of `render_deck_variables` (e.g. CrowdAnki export path in `crates/brain-brew-formats/src/crowdanki.rs`) already handle the `Err` case, so no caller changes expected beyond error display.
