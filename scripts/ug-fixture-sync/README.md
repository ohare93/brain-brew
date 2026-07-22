# Pinned Ultimate Geography fixture contract

The Ultimate Geography fixture is a vendored, hermetic regression input. It is
not a submodule, a generated migration fork, or a network cache.

## Pinned inputs

- Ultimate Geography: `brainbrew-migration` at
  `adda7ad925c62fa6542679dfb5bc1c6401466480`, descending from the migration
  history rebased on upstream `e1fd85184e70f32650b67b750c44c4b0588c79dd`
- Brain Brew generator: `rust-brainbrew` / `1.0.0-alpha.3` at
  `d745834534139b965732e007a58b489dad44449d`
- Hardcore Geography attribution source: `main` at
  `09ce7c3ba665eac6b0794d089a4e0bbafbfc0f46`

`fixtures/ultimate-geography.lock.json` records those pins, the exact 74 main +
26 companion target mapping, source/media/UG-golden metadata, separate UG and
Hardcore attribution digests, exact 609-file attribution coverage, the source
digest from which expected output was accepted, the semantic digest of all 100
expected JSON files, and the reviewed generator's executable SHA-256 plus
deterministic source/build identity.

The source snapshot lives at `fixtures/ultimate-geography/`. Its top level is an
exact whitelist: both manifests and deck files, `media.yaml`, `overlays/`, all
scalar include trees (`descriptions/`, `templates/`, and `styles/`), the single
real `media/` tree, UG's manifest-referenced `goldens/`, and upstream
`LICENSE.md` plus `sources.csv`. Repository/VCS, planning, documentation,
spreadsheet maintainer sources, and generated build paths are intentionally
excluded and recorded with reasons in the lock.

The byte-exact UG snapshot is unchanged by the separate
`fixtures/ultimate-geography-attribution/hardcore-geography/` supplement. It
contains the exact Hardcore `README.md` and `sources.csv` bytes; its pinned
revision has no `LICENSE` or `NOTICE`, and the README supplies no license grant.
Fixture media retains its upstream per-file licensing and attribution and is not
covered wholesale by Brain Brew's root `LICENSE`; see `THIRD_PARTY_ASSETS.md`.
The `file:///home/adam/...` string embedded in
`media/ug-map-galapagos_islands.png` is inert, byte-preserved PNG metadata, not
an active local path or dependency.

Full parsed outputs live separately at
`fixtures/ultimate-geography-expected/crowdanki/<target>/deck.json`. There are
exactly 100. No expected target contains media; all targets share the one real
vendored media tree.

## Three separate boundaries

Acceptance and generated-output checks require both the exact reviewed binary
and a source root at the pinned Brain Brew revision. The reproducible release
executable SHA-256 is
`063a2106ad5c0000eb6afbb5896e950d2616b98aac372498cc623be0d358c411`;
the 68-file generator source identity is
`19b6910358db8c0dd1cd35f4ae936deff1d3090ea88ec8a1c7f9ab9686c96081`.
A different executable is rejected even if it prints `1.0.0-alpha.3`, and a
changed lock cannot bless a different hash because both Python and Rust tests
hold reviewed constants. The lock records the exact Rust/Cargo versions and the
reproducible build contract: release profile, incremental compilation disabled,
no debug info, and `<source-root>` remapped to `/brainbrew`. Two independent
source-root builds produced byte-identical executables during acceptance.

```bash
source_root=/path/to/brain-brew-at-d745834
CARGO_INCREMENTAL=0 \
RUSTFLAGS="-C debuginfo=0 --remap-path-prefix=$source_root=/brainbrew" \
  cargo build --locked --offline --release -p brainbrew --bin brainbrew \
  --manifest-path "$source_root/Cargo.toml"
```

### 1. Refresh source only

```bash
scripts/sync-ug-fixture.sh --sync-source \
  --ug-checkout /home/jmo/Development/external/ultimate-geography \
  --ug-revision adda7ad925c62fa6542679dfb5bc1c6401466480
```

This copies the whitelist byte-for-byte in one direction and updates only source
provenance/inventory in the lock. It never formats UG source, appends fixture-only
manifest fields, invokes Git/Jujutsu, writes to the UG checkout, changes expected
output, or writes/removes the separately pinned Hardcore attribution directory
or its lock section. A changed source digest deliberately leaves expected output
stale.

### 2. Explicitly accept expected output

After reviewing the source delta and generator pin:

```bash
scripts/sync-ug-fixture.sh --accept-expected \
  --brainbrew-bin /path/to/reviewed/brainbrew \
  --brainbrew-revision d745834534139b965732e007a58b489dad44449d \
  --brainbrew-source-root /path/to/brain-brew-at-d745834
```

Only this command regenerates and publishes the 100 expected `deck.json` files.
Generation uses explicit reference-only exports in a temporary copy so media is
not duplicated. Acceptance parses every JSON file, requires the exact 74 + 26
target set, and atomically updates the expected tree and its lock record.
Reference-only generation is not media-integrity evidence; strict media bytes
are a separate mandatory test/gate below.

### 3. Read-only drift check

```bash
scripts/sync-ug-fixture.sh --check \
  --ug-checkout /home/jmo/Development/external/ultimate-geography \
  --ug-revision adda7ad925c62fa6542679dfb5bc1c6401466480 \
  --brainbrew-bin /path/to/reviewed/brainbrew \
  --brainbrew-revision d745834534139b965732e007a58b489dad44449d \
  --brainbrew-source-root /path/to/brain-brew-at-d745834
```

`--check` never updates source, lock, or expected files. It rejects source/lock
or checkout drift, stale acceptance, malformed/missing/extra targets, expected
value drift, Hardcore supplement/provenance/file-digest drift, incomplete or
ambiguous normalized-filename attribution, a wrong or modified generator
executable/source/build identity, and generated output that differs from
accepted parsed JSON. Omit the UG checkout/revision pair for a fully offline
committed-tree check; the pinned Brain Brew source root and executable are still
required to authenticate the regeneration tool.

## Mandatory offline gates

The normal Rust test gate runs
`ultimate_geography_fixture_matches_all_pinned_outputs_and_strict_real_media`.
It independently enforces source/media/expected digests and exact inventory,
composes all 100 targets, compares every generated export with its committed
parsed JSON value, verifies every target's declarations against real media
bytes/hashes, and proves the union uses exactly the single 609-file media tree.
The inventory gate separately proves that all 604 images map unambiguously to
548 UG plus 56 Hardcore source rows, while the five non-image runtime assets map
to the UG license notice. Matching uses case-preserving Unicode NFC POSIX
basenames and rejects normalization, collision, missing, extra, and unknown-file
drift. The boundary unit tests also include count-preserving value substitution,
source drift, and missing/extra target failures.

The release-strict CLI verification can also be run directly, without network:

```bash
target/debug/brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew.yaml \
  --all-targets --media-root media
target/debug/brainbrew verify \
  --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml \
  --all-targets --media-root media
```

## Digest definition

Source/media/UG-golden tree hashes are SHA-256 over the domain
`brainbrew-tree-sha256-v1\0`, followed by each sorted UTF-8 relative path,
NUL, decimal content length, NUL, and file bytes. Expected output uses the
analogous `brainbrew-json-tree-sha256-v1\0` domain after parsing each
`<target>/deck.json` and serializing objects with sorted keys and compact JSON.
Attribution coverage uses `brainbrew-attribution-coverage-sha256-v1\0`, then
each sorted normalized filename framed in the same way with its exact UG/HG
owner label as content. Thus expected comparisons are value-exact while key
order and whitespace alone do not create drift, and attribution ownership cannot
be reassigned without changing its reviewed digest.
