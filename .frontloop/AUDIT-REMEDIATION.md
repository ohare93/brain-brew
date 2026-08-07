# Brain Brew audit remediation program

Source of truth: `audit/16-synthesis.md`, supported by `audit/01-flow-map.md` through `audit/15-release-security.md`.

This file defines cross-epic ordering. Within each epic, ready-task numeric prefixes are strict sequence, not estimates of parallel safety. A task whose Implementation Notes names a clarification or another epic must not start until that prerequisite is resolved.

## Epics

1. `release-baseline` — immutable versions, extracted crate verification, Nix, release gating, trusted channels.
2. `canonical-source-integrity` — fail-closed YAML, one source-document interface, recoverable transactions, safe CLI output.
3. `core-compose-correctness` — semantic field values, expected bases, tombstones, messages, semantic diff, typed errors.
4. `package-federation-security` — contained/authenticated packages, bounded fetches, one planner, media provenance.
5. `workbench-hardening` — containment, typed contract, security, CAS/preview/transactions, complete UI, bounded caches.
6. `translation-integrity` — structural content policy, compositional ownership, explicit deletion/adaptation, shared mutation policy.
7. `crowdanki-roundtrip` — Unicode identity, GUID/ordinal validation, reviewable import, media handoff, equivalence/reconciliation.
8. `ultimate-geography-production` — real-consumer canonical/media/translation/parity/package/release baseline.
9. `quality-docs-observability` — truthful ADRs/docs, initialization, executable examples, generated/fuzz/mutation/platform/budget gates.
10. `architecture-performance` — later structural deepening and measured optimization after correctness stabilizes.

## Phase 0 — decisions and immediate containment

Resolve these clarification gates before starting their dependent tasks:

- Release version and supported channels.
- Replacement precondition representation and typed tombstone model.
- Package symlink, compatibility-version, and release-media policies.
- Workbench trust/transaction scope.
- Structural field-name, strict translation ownership, and blank translation policies.
- UG fixture contract and legacy workflow disposition.
- Public Rust crate support commitment.
- Existing `default/clarify/7500` for Anki-to-existing-source reconciliation.

Immediate code containment that does not wait for every decision:

1. `workbench-hardening/0010` — mark Apply experimental or make release Workbench read-only.
2. `canonical-source-integrity/0010` — reject duplicate dynamic keys.
3. `package-federation-security/0010` — central safe-relative-path authorization.
4. `core-compose-correctness/0010` — semantic field values.
5. `ultimate-geography-production/0010` — pinned/reconciled consumer baseline.
6. `quality-docs-observability/0010` — make ADRs and architecture records truthful.

## Phase 1 — integrity foundations

These streams may proceed in parallel, preserving each epic's numeric order:

- `canonical-source-integrity/0010–0040`: strict decoding, source documents, transactions.
- `core-compose-correctness/0010–0050`: value semantics, preconditions, tombstones, messages, complete diff.
- `package-federation-security/0010–0030`: path authorization, immutable lock schema, safe extraction.
- `release-baseline/0010–0030`: version synchronization, extracted crates, Nix/E2E separation.
- `crowdanki-roundtrip/0010–0020`: imported stable IDs and identity validation.

Exit: no silent source loss, no unauthenticated package-tree reads, destructive composition has complete regression coverage, and package artifacts can be verified independently.

## Phase 2 — shared mutation, planning, and Workbench safety

1. Complete `canonical-source-integrity/0050–0070`.
2. Complete `package-federation-security/0040–0090`.
3. Complete `translation-integrity/0010–0050` once its decisions are resolved.
4. Complete `workbench-hardening/0020–0060`; remove read-only/experimental containment only after 0030–0060 pass.
5. Complete `crowdanki-roundtrip/0030–0050`.
6. Complete `core-compose-correctness/0060` with the typed Workbench contract.

Exit: every mutator plans and validates before writing, uses canonical document operations, has recoverable commit semantics, and carries package/media provenance.

## Phase 3 — production Ultimate Geography acceptance

Follow `ultimate-geography-production/0020–0100` in order. Translation tasks 0050/0060 require the translation-integrity epic; golden tasks 0070/0080 require the complete CrowdAnki oracle; packaging and final consumer gate require media ownership and release verification policy.

In parallel after their prerequisites:

- `release-baseline/0040–0070`.
- `quality-docs-observability/0020–0040`.

Exit: actual UG—not only the fixture—formats, verifies all 74+26 targets with media, exports representative goldens, packages declared media reproducibly, and gates Brain Brew releases.

## Phase 4 — product completeness

- `workbench-hardening/0070–0110`: complete list/detail, drafts, bounded caches, accessibility/lifecycle, pivot removal.
- `crowdanki-roundtrip/0060`: implement or deprecate existing-source reconciliation after `default/7500`.
- Remaining executable docs and supported installation/Rust interface work.
- Resolve/complete existing `default/ready/0150` cross-language matrix in coordination with Workbench pagination and translation ownership; do not duplicate it.

## Phase 5 — architecture and long-horizon quality

Only after affected behavior is stable:

1. `architecture-performance/0010–0040` — typed paths and implementation splits.
2. `architecture-performance/0050–0070` — measured composition, media, and list-index optimization.
3. `quality-docs-observability/0050–0100` — property, fuzz, mutation, cross-platform, accessibility/performance, and public-crate gates.

## Global delivery rules

- Use red-green-refactor for every behavior change.
- Each task must preserve deterministic canonical output and run the relevant Devenv gates.
- Security/correctness tasks require the reproduced audit probe as a regression test.
- Structural cleanup follows behavior fixes; do not mix mass refactors with semantic corrections.
- Real UG acceptance is mandatory before release claims are restored.
- Do not mark an epic done while any clarification that affects its shipped behavior remains unresolved.
