---
title: CrowdAnki bootstrap boundary
---

# CrowdAnki bootstrap boundary

## Current decision

`default/7500` is **deferred**: Brain Brew does not currently support pulling Anki or CrowdAnki edits into an existing Federated Deck source. There is no existing-source reconciliation command, flag, or automatic merge.

`import crowdanki` is supported only to bootstrap a **new, full-deck** Canonical Deck workspace from a CrowdAnki folder. It writes a reviewed plan and, after approval, a separately named output workspace. It never mutates or merges into a base deck, include tree, overlay stack, or manifest.

`--force` has one narrow meaning: recoverably replace the requested bootstrap output directory. It does not make an existing source a valid import destination and does not authorize a merge.

## Why import cannot choose ownership

CrowdAnki contains adapter-visible deck content, not the maintainer's source structure. A full-deck import cannot determine whether a changed value belongs in the base, an extension or patch overlay, a translation dictionary or target adaptation, an included scalar or media map, or a media asset. It also cannot recover source variables, include boundaries, typed structured fields, tombstone intent, or overlay ordering from the exported folder.

Treat the imported workspace as bootstrap output, not as a replacement for an established Federated Deck. Even the documented adapter-equivalence projection is intentionally lossy: source variables are rendered, structured fields are lowered, media hashes and typed tombstones are absent, unsupported adapter IDs are absent, and canonical stable IDs are suggested again from adapter-visible data.

## Safe workflow when edits were made in Anki

1. Preserve the existing source workspace unchanged and versioned.
2. Import the CrowdAnki folder into a new, empty bootstrap destination using the [plan/review/apply flow](importing-crowdanki.md). Do not point `--out` at the source workspace.
3. Compose the intended source target and inspect a semantic diff against the bootstrap output.
4. Where a sparse patch is appropriate, use `brainbrew diff --as-overlay` only to produce a **review artifact**. It is not a reverse mapper and does not write source files.
5. Manually route each accepted change to its maintainer-owned location: base source, include, structured field, translation or target adaptation, overlay, media declaration, or media asset. Review and validate the result before committing.

No command silently writes those changes back. In particular, do not replace a source tree with imported output: doing so can flatten source structure and discard the ownership and intent that make federation reviewable.

A future feature may offer a human-approved, reviewable overlay-draft workflow if maintainers demonstrate demand and its loss boundaries can be designed safely. That is not current support and no command should be presented as that workflow.

## Ultimate Geography

Ultimate Geography workflow policy is owned by its maintainers outside this repository. Brain Brew's UG fixture is a compatibility case study, not authority to change upstream contributor policy. This tool does not restore Ultimate Geography's legacy Anki-to-source workflow; upstream maintainers must choose and document any replacement process independently.
