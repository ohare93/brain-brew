# ADR-010: Fail Closed on Unsupported Adapter Data

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Brain Brew imports and exports Anki-compatible decks, currently through CrowdAnki. External formats can contain data that Brain Brew does not model, including review scheduling state, format-specific defaults, add-on metadata, and other unsupported fields. Silently preserving opaque payloads would make composition hard to reason about; silently dropping them would be worse.

Media also needs explicit handling: deck source should reference media assets without embedding arbitrary file bytes into canonical YAML.

## Decision

Brain Brew preserves Anki-compatible deck semantics that are represented in Canonical Deck and fails closed on unsupported adapter data.

Review history and scheduling state are not part of Canonical Deck. Media assets stay external and are represented by media references. Unknown or unsupported adapter data must either be modeled deliberately, rejected with a clear diagnostic, or handled by an explicit compatibility rule. Strict validation is the default.

## Rationale

**Pros:**

- Avoids accidental data loss from ignored adapter fields.
- Keeps the Canonical Deck model honest and reviewable.
- Separates deck content from learner-specific review state.
- Makes media reproducible through references, checks, and external assets.

**Cons:**

- Some real-world decks may fail import until support is added.
- Compatibility work requires explicit modeling and tests.
- Brain Brew cannot be used as an arbitrary adapter-data carrier.

## Alternatives Considered

- **Opaque passthrough for all unknown data**: rejected because overlays and semantic diffs could not reason about it.
- **Best-effort import that drops unknown data**: rejected because it risks silent corruption.
- **Store review history in Canonical Deck**: rejected because review state belongs to learners and schedulers, not shared deck source.
- **Embed media bytes in YAML**: rejected because media assets are better stored and verified as external files.

## Implications

- Import/export parity tests must name the adapter semantics Brain Brew supports.
- Unsupported data diagnostics should explain what was rejected and why.
- New adapter support requires domain modeling, validation, and fixtures.
- Canonical Deck remains deck source, not a full backup of every external application database.
