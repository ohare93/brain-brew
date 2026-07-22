---
title: Ultimate Geography regression fixture
---

# Ultimate Geography regression fixture

Brain Brew vendors a complete, immutable Ultimate Geography regression input so
normal tests do not depend on a checkout, a network fetch, a submodule, or a
release cache.

## Contract

The reviewed snapshot is Ultimate Geography `brainbrew-migration` revision
`795853d49832ab550b5cb872da47413377ebec5e`, descending from the migration
history rebased on upstream revision
`e1fd85184e70f32650b67b750c44c4b0588c79dd`. Expected output was accepted with
Brain Brew `1.0.0-alpha.3` revision
`77b092ddb82fb0dfdaf64713ed081a4ac9f2eb97`. This snapshot includes the reviewed
flag-similarity translation and separator corrections recorded by the UG source
revision. Hardcore image attribution is
separately pinned to Hardcore Geography revision
`09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46`.

- `fixtures/ultimate-geography/` is an exact whitelist of the two manifests,
  canonical deck/overlay source, scalar includes, `media.yaml`, all 609 real
  media files, all eight UG-owned goldens referenced by the manifests, and
  upstream `LICENSE.md` plus the 548-row UG `sources.csv` attribution.
- `fixtures/ultimate-geography-attribution/hardcore-geography/` preserves the
  exact upstream `README.md` and `sources.csv` attribution bytes without
  changing the byte-exact UG snapshot.
- `fixtures/ultimate-geography-expected/crowdanki/<target>/deck.json` contains
  complete parsed output for exactly 74 main and 26 companion targets.
- Expected targets never contain media copies. Every strict verification reads
  the one source media tree.
- `fixtures/ultimate-geography.lock.json` binds provenance, target-to-manifest
  mapping, counts, byte inventories, source/media/golden/attribution digests,
  the Hardcore revision and per-file supplement hashes, exact normalized
  attribution coverage, the accepted source digest, a parsed/canonical
  expected-JSON digest, and the reviewed generator executable SHA-256 plus
  deterministic source/build identity.

The snapshot intentionally excludes UG repository/VCS, CI, planning,
documentation, spreadsheet maintainer sources, and generated build paths because
the pinned manifests do not read them. Licensing and attribution are retained
even though manifests do not read them. Those exclusions and reasons are
machine-readable in the lock. There is no hidden fixture-only language/profile
patch: the manifests are the exact pinned UG bytes.

The vendored media is not covered wholesale by Brain Brew's root license. The
548 UG image rows and 56 Hardcore image rows retain their separate upstream
terms; the pinned Hardcore revision has no repository `LICENSE`/`NOTICE` and its
README supplies no broader grant. See `THIRD_PARTY_ASSETS.md`, both pinned
`sources.csv` files, and UG's `LICENSE.md` before redistribution. The
`file:///home/adam/...` string inside
`ug-map-galapagos_islands.png` is inert, byte-preserved upstream PNG metadata;
Brain Brew never treats it as a filesystem or network input.

## Mandatory gate

The normal `brain-brew-formats` integration suite composes and exports every one
of the 100 targets, parses each committed expected `deck.json`, and requires
exact JSON value equality. It rejects unknown, missing, or extra expected target
directories. The same test validates canonical non-empty hashes and real bytes
for every target and proves that the all-target declaration union is exactly the
single 609-file media tree. An attribution inventory test proves exact,
unambiguous coverage of all 604 images by 548 UG plus 56 Hardcore source rows
and of the five non-image runtime files by UG's license notice. It uses
case-preserving Unicode NFC POSIX-basename normalization and fails on any
normalization collision, missing/extra row, or unknown file.

Serialization whitespace and object-key order are not regressions: the lock and
tests digest parsed JSON in canonical key order. Any count-preserving value
substitution is a regression and fails.

The `795853d…` refresh deliberately corrects flag-similarity content in 28
of the 100 targets. Against the preceding accepted snapshot, the complete parsed
JSON delta is 74 occurrences of 17 reviewed `(GUID, field 6, old, new)` tuples;
there are no changes at any other JSON path. The source-owned audit is excluded
from the runtime fixture whitelist, while this repository retains the accepted
all-target outputs and their changed semantic digest as the regression oracle.

## Maintenance boundaries

`scripts/sync-ug-fixture.sh` has three mutually exclusive operations:

1. Source sync copies only the reviewed whitelist from an explicitly revised UG
   checkout. It updates source provenance but never expected output or the
   separate Hardcore supplement/lock section.
2. `--accept-expected` is the only operation that regenerates and blesses all
   100 expected JSON files. It requires the exact reviewed executable, explicit
   revision, and a source root matching the hard-coded alpha.3 source identity.
   The current reviewed binary has SHA-256
   `58782c88efedc3691be904bcf730f4314c4ce475c7ccb607ee4556ddb767c259`,
   and its 68-file generator source identity is
   `754018e336b8f5877460c4430be7809d703f34762439dc477441eebb10a3be61`.
3. `--check` is read-only. It authenticates the same executable/source/build
   identity, checks the lock/source/target/expected state, validates the pinned
   Hardcore supplement and attribution coverage, and regenerates all outputs in
   a temporary workspace for parsed comparison. With
   an explicitly revised UG checkout it also proves vendored source byte parity.

All operations are standard-library/local-binary only. They do not invoke Git,
Jujutsu, or the network, and never write to the UG checkout. Exact commands and
digest framing are documented in `scripts/ug-fixture-sync/README.md`.

For an additional strict CLI probe:

```bash
target/debug/brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew.yaml \
  --all-targets --media-root media
target/debug/brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml \
  --all-targets --media-root media
```
