# Manifest, Lock, Source, and Media Audit

## Review

### Correct

- **Target/overlay dependency expansion is deterministic and detects local and cross-package cycles.** Local expansion uses a visited set plus active stack (`crates/brain-brew-formats/src/manifest.rs:24-42,124-153`); package-aware expansion keys both package index and overlay ID (`crates/brain-brew-cli/src/io.rs:337-423`), and target inheritance has its own active stack (`crates/brain-brew-cli/src/io.rs:269-302`). Existing cycle coverage is in `crates/brain-brew-formats/tests/manifest_yaml.rs:825-850`.
- **File includes have materially better containment than ordinary package paths.** Include roots are canonicalized, absolute paths are rejected, lexical and canonical containment are checked, nested scalar cycles report a chain, and only top-level `media` is structurally spliced (`crates/brain-brew-formats/src/source_includes.rs:321-466,468-510`). `!image` is intentionally passed through (`source_includes.rs:402`).
- **ADR-0016 structured image behavior is substantially implemented.** Unknown stable IDs fail during rendering with a field path, and rendering emits the specified exact tag (`crates/brain-brew-core/src/compose.rs:1776-1805`). Verification resolves structured IDs while retaining raw HTML/CSS scanning (`crates/brain-brew-formats/src/media.rs:45-70,136-175`). Structural media maps are parsed and formatted separately, and hash writeback follows a base `media: !include` (`crates/brain-brew-cli/src/commands/media.rs:307-340`).
- **Real Ultimate Geography usage exercises the intended severable shape.** `fixtures/ultimate-geography/deck.yaml:5527` uses `media: !include media.yaml`; repository inspection found 602 `!image` uses, zero remaining strict raw `<img src="…" />` fields, 546 declarations, and no duplicate declaration paths. All 67 manifest overlay catalog IDs/kinds match their files. `brainbrew targets --json` reported 74 targets and `brainbrew verify --all-targets` passed all 74.
- **Warm-cache lock behavior is reproducible when a valid hash is present.** `fetch_locked_source_with_mode` prefers a cache entry only after re-hashing it (`crates/brain-brew-cli/src/commands/lock.rs:430-455,584-601`), while `lock verify` deliberately re-reads live path sources. Existing tests prove path drift detection and relocatable relative locks (`crates/brain-brew-cli/tests/lock_cli.rs:6-120`). A warm git/tarball cache therefore supports offline reads; a cold cache necessarily fetches.

### Blocker — locked package manifests can escape the hashed source tree

**Refs:** `crates/brain-brew-cli/src/commands/lock.rs:48,68,107-108,474-475`; `crates/brain-brew-cli/src/io.rs:94-96,298-310,415-420`; ADR-0009 requires locks to be reproducible package inputs (`documentation/docs/reference/decisions/0009-use-manifests-targets-and-locks-before-a-recipe-dsl.md:15-17,41-45`).

`Path::join` discards the fetched source prefix when `package.manifest` is absolute, and neither `lock update` nor lock parsing requires the manifest to be a contained relative path. The same issue applies to manifest `base` and overlay `file` values: they are joined and read without the canonical containment checks used for `!include`. Preserved source-tree symlinks (`commands/lock.rs:796-824,834-845`) provide another escape route because later `read_to_string` follows them.

**Verified scenario:** an audit fixture locked an unrelated source directory while passing an absolute `--package-manifest /tmp/.../host/brainbrew.yaml`. After lock creation, the host deck outside the NAR-hashed source was changed from `Original Host Deck` to `Mutated Outside Lock`. `brainbrew lock verify` still printed `verified 1 locked package`, and a downstream locked compose emitted `name: Mutated Outside Lock`. The lock contained the absolute manifest path and a hash covering only the unrelated source bytes.

**Impact:** a lock can claim one hash while composition reads mutable/unhashed files; a package can also make Brain Brew read host paths outside its snapshot. This defeats the central ADR-0009 integrity guarantee and is a package trust-boundary failure.

**Required fix/test:** require `manifest`, `base`, and overlay `file` to be non-empty package-relative paths; canonicalize the final existing target and reject absolute, `..`, and symlink escapes from the fetched package root. Add CLI tests for absolute and parent manifest paths, absolute/parent base and overlay paths, and in-tree symlinks targeting outside the snapshot.

### High — hand-edited locks may be completely unpinned and still verify

**Refs:** `crates/brain-brew-formats/src/lockfile.rs:32-40,176-198,215-229`; `crates/brain-brew-cli/src/commands/lock.rs:94-108,430-454,584-587`.

`locked.nar_hash` is optional, and lock parsing validates neither required fields per source type nor hash/revision syntax. With no hash, `cached_source` returns `None`, the live source is fetched/snapshotted, and hash comparison is skipped. A `locked` path, tarball, or git source can therefore remain mutable despite being accepted as a lock.

**Verified scenario:** a manual path lock with no `nar_hash` was verified successfully; after mutating its deck, the same unchanged lock verified successfully again. Existing lockfile tests only require the `locked` mapping itself (`crates/brain-brew-formats/tests/lockfile_yaml.rs:85-106`), and CLI lock tests cover only locks emitted by `lock update`.

**Required fix/test:** make a valid SRI SHA-256 mandatory in every `locked` source; require exactly the fields appropriate to `path`, `git`, or `tarball`, and require a resolved git revision. Add malformed/unpinned lock tests and a cold-cache offline diagnostic test. An explicit `--offline` mode would also give predictable fail-fast UX instead of attempting network access.

### High — ordinary `verify` accepts empty or malformed media checksums

**Refs:** `crates/brain-brew-cli/src/commands/verify.rs:47-75`; `crates/brain-brew-cli/src/media_assets.rs:8-26`; `crates/brain-brew-formats/src/media.rs:89-127`; ADR-0016 verification contract at `documentation/docs/reference/decisions/0016-use-structured-image-field-references-and-severable-media-includes.md:100-109`.

Without `--media-root`, verification calls only `validate_media_references`; checksum validation, including the explicit `EmptyHash` check, runs only when asset bytes are supplied. Thus source can be reported verified while its media declarations are not content-addressed at all.

**Real UG evidence:** `fixtures/ultimate-geography/media.yaml:1-9` begins with empty `sha256` values (and the declaration file uses this pattern throughout), yet `devenv shell brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets` reported `✓ verified 74 targets`. This is weaker than ADR-0016's statement that structured references proceed through path, SHA-256, and on-disk integrity checks.

**Required fix/test:** always validate checksum presence and canonical 64-character lowercase hex syntax; with `--media-root`, additionally compare bytes and require assets. Add a CLI test proving no-media-root verification rejects empty/malformed checksums, while a separate diagnostic clearly says when byte validation was not requested.

### High — media migration/hash commands can rewrite locked dependency cache entries using the wrong media root

**Refs:** `crates/brain-brew-cli/src/commands/media.rs:40-74,89-116,285-299,358-379`; package-qualified overlays resolve to the dependency package path in `crates/brain-brew-cli/src/io.rs:395-420`.

`collect_manifest_source_files` inserts every planned overlay file, including package-qualified overlays fetched from `brainbrew.lock`. Both `media hash` and `media images-to-refs` then write those paths in place. For a locked dependency, that path is under the content-addressed cache. `media hash` also resolves every asset against the root package's single `--media-root`, not the owning overlay package's root.

**Scenario:** a downstream target selects `upstream:overlay.extension.media`; running either media mutation command can alter `~/.cache/brainbrew/sources/<hash>/...`. The next cache read notices a NAR mismatch and deletes the cache (`commands/lock.rs:592-601`); a remote dependency that was previously usable offline now needs a refetch. Hashing can additionally write hashes for downstream files into upstream declarations when relative media paths overlap.

**Required fix/test:** source collection must retain package ownership and mutability. Mutating commands should modify only the root workspace by default, reject locked/cache sources, and require an explicit per-package operation for dependencies. Add locked package-qualified overlay tests asserting cache bytes/NAR hash remain unchanged. The export/hash model also needs per-package media-root provenance before federated packages with independent asset trees are reliable.

### High — dependency validation is bypassed for explicit `--include`; package cycles and compatibility metadata are not enforced

**Refs:** `crates/brain-brew-cli/src/io.rs:202-241`; identical gate in `crates/brain-brew-cli/src/commands/targets.rs:27-38`; `crates/brain-brew-cli/src/package_resolver.rs:16-61`; `crates/brain-brew-formats/src/manifest.rs:159-165,670-688`.

Dependency validation runs only when `--package-root` is non-empty or lock paths were found, not when manifests are supplied via `--include`. The validator checks only presence and optional exact `@version`; it does not detect package dependency cycles. `compatible_base_versions` is parsed, emitted, and displayed but has no enforcement consumer (the real UG manifest declares `>=0.1,<0.2` at `fixtures/ultimate-geography/brainbrew.yaml:1-5`).

**Verified scenario:** an explicitly included upstream package declared `depends_on: example.missing@1.0.0`; a root target extended it and composition succeeded. The same metadata would fail if discovered through `--package-root` or a lock. Cyclic A→B→A metadata likewise passes the current validator as long as both IDs exist.

**Required fix/test:** validate the complete registry whenever more than the root manifest is loaded, irrespective of discovery mechanism; reject malformed dependency strings and dependency cycles with a package chain. Define and test the semantics of `compatible_base_versions` or remove it until enforceable. Add parity tests showing `--include`, `--package-root`, and lock discovery produce the same dependency result.

### Medium — package-qualified targets compose but `targets --json` cannot expand them

**Refs:** package-aware resolver `crates/brain-brew-cli/src/io.rs:337-457`; JSON path `crates/brain-brew-cli/src/commands/targets.rs:40-59`; local-only expansion `crates/brain-brew-cli/src/io.rs:460-508` and `crates/brain-brew-formats/src/manifest.rs:24-42`.

The JSON listing expands each manifest without a registry, so a documented package-qualified overlay is looked up literally in the local catalog. Text listing works because it does not expand.

**Verified scenario:** a target extending `example.up:base` and selecting `example.up:overlay.patch` composed through the package-aware planner, but `brainbrew targets --manifest root --include up --json` failed with `manifest overlay "example.up:overlay.patch" does not exist`.

**Required fix/test:** use the same registry/planner for compose, explain, verify, and target JSON; add JSON coverage for qualified `extends`, qualified overlays, transitive overlay dependencies, and target/overlay cycles with full package-qualified chains.

### Medium — manifest overlay identity/kind is advisory and can diverge from the loaded overlay

**Refs:** catalog fields are accepted as arbitrary optional strings (`crates/brain-brew-formats/src/manifest.rs:168-174,692-708`); loading reads the file but does not compare catalog ID/kind with `Overlay.id`/`Overlay.kind` (`crates/brain-brew-cli/src/io.rs:314-325,382-421`). Documentation says the manifest kind should match (`documentation/docs/authoring/manifests-targets.md:38-52`).

A catalog key can point to a file with a different `id`, and `kind: translation` can point to an extension. Expansion/explain reports the catalog identity, while compose conflict provenance and translation tooling use the file identity/kind. Duplicate file IDs under distinct catalog keys can also yield misleading self-conflict diagnostics.

**Real UG evidence:** an audit script checked all 67 catalog entries and found zero ID/kind mismatches, so the fixture is disciplined; the loader does not guarantee this for other packages.

**Required fix/test:** validate catalog key == overlay file ID and catalog kind == parsed overlay kind during planning/verify; reject unknown manifest kinds. Add mismatch tests, including two catalog entries whose files share one ID.

### Medium — media path safety is checked too late and is not symlink-safe; export may skip reference validation

**Refs:** media paths are only lexically checked for absolute/parent components (`crates/brain-brew-cli/src/media_assets.rs:29-45,48-76`; duplicate logic at `commands/media.rs:618-627`); structured rendering interpolates the raw path into HTML (`crates/brain-brew-core/src/compose.rs:1782-1804`); export validates media only when `--media-root` is supplied (`crates/brain-brew-cli/src/commands/export.rs:86-100`).

A media root containing an in-tree symlink to an external directory passes the lexical check, and `fs::read`/`fs::copy` follows it. This matters especially because package snapshots preserve symlinks. Paths containing `"`, `<`, `>`, CR, or LF are also accepted and are interpolated unescaped into `<img src="…" />`, even though ADR-0016's strict reverse mapper explicitly excludes those characters. Finally, an export without `--media-root` does not call `validate_media_references`, so raw HTML can name an undeclared asset and still produce `deck.json`.

**Required fix/test:** validate media path syntax in the domain/format layer, reject URI-like and HTML-breaking paths, canonicalize each existing source under canonical media root before reading/copying, and always validate declaration/reference consistency before export. Add Unix symlink-escape, quote/newline path, and no-media-root raw-reference export tests.

### Note — source verification is not transitive across target inheritance

`verify` formats the root manifest/base and only `plan.overlays` retained after inherited targets have already been composed (`crates/brain-brew-cli/src/commands/verify.rs:38-60`; `crates/brain-brew-cli/src/io.rs:298-334`). Upstream inherited overlays and included media-map formatting are therefore not independently checked by a downstream verify. This is lower risk for a correctly hashed dependency but weakens source-quality UX. A registry plan should retain transitive source provenance so verify can report what was checked without rewriting dependency sources.

## Missing test matrix

1. Lock containment: absolute/`..`/symlink manifest, base, and overlay paths.
2. Lock semantics: absent/malformed NAR hash, illegal source-field combinations, absent git revision, warm/cold offline behavior.
3. Discovery parity: `--include` vs `--package-root` vs lock, package dependency cycles, malformed dependency specs, compatibility ranges.
4. Registry UX: package-qualified `targets --json`, full cycle chains, manifest/file ID and kind mismatches.
5. Federated media ownership: package-qualified overlays under lock; mutation commands must not touch cache/dependency files or use another package's media root.
6. Media integrity: empty/malformed checksum without media root, raw undeclared reference during export, symlink escapes, HTML-breaking paths.
7. Transitive verify: inherited base/overlay/media-map formatting and clear reporting of dependency source verification.

## Audit constraints

- Requested `plan.md` and `progress.md` did not exist at the supplied paths; review proceeded from ADRs, docs, implementation, tests, fixtures, and executable reproductions.
- Review only: no project/source files or tests were modified. This audit report is the only requested output file.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Completed the requested manifest/target/overlay/package-lock/source/include/media review without modifying project source; wrote only audit/06-manifest-lock-media.md."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Findings include exact file/line references, executable scenarios, real Ultimate Geography counts and 74-target verification, ranked impact, and explicit missing tests."
    }
  ],
  "changedFiles": [
    "audit/06-manifest-lock-media.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell cargo test -p brain-brew-formats --test manifest_yaml --test lockfile_yaml --test media_map --test media_references",
      "result": "passed",
      "summary": "44 focused format/manifest/lock/media tests passed."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test lock_cli --test media_includes",
      "result": "passed",
      "summary": "5 focused CLI lock/include tests passed."
    },
    {
      "command": "devenv shell brainbrew targets --manifest fixtures/ultimate-geography/brainbrew.yaml --json",
      "result": "passed",
      "summary": "Reported 74 Ultimate Geography targets."
    },
    {
      "command": "devenv shell brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets",
      "result": "passed",
      "summary": "Verified all 74 targets; also demonstrated that empty media hashes are not checked without --media-root."
    },
    {
      "command": "temporary-fixture lock escape, unpinned-lock, explicit-include dependency, and qualified-target JSON reproductions under /tmp",
      "result": "passed",
      "summary": "Reproduced mutable out-of-hash manifest composition, acceptance of hashless locks, dependency-validation bypass, and targets --json failure."
    },
    {
      "command": "jj status && jj diff --stat (before and after review)",
      "result": "passed",
      "summary": "Pre-review working copy was clean; post-review status showed this report plus four concurrent audit reports, with no project/source edits."
    },
    {
      "command": "python3 acceptance-report JSON validation",
      "result": "passed",
      "summary": "Confirmed the final fenced acceptance-report is valid JSON and names this audit file."
    }
  ],
  "validationOutput": [
    "Ultimate Geography: 74 targets; 602 !image uses; 546 media declarations; 0 strict raw image fields; 0 duplicate media paths; 0 overlay catalog ID/kind mismatches.",
    "Lock escape reproduction: lock verify succeeded after an out-of-hash host deck changed, and compose emitted the changed name.",
    "Unpinned lock reproduction: verification succeeded before and after mutation with zero nar_hash fields.",
    "Explicit --include reproduction: composition succeeded despite a missing declared package dependency.",
    "Qualified target reproduction: targets --json failed with manifest overlay package-qualified ID does not exist."
  ],
  "residualRisks": [
    "No full workspace test/CI run was needed for this review; focused suites and real UG all-target verification passed.",
    "Cold-cache network behavior was inspected in code but not exercised against live GitHub/tarball services.",
    "Federated media cache mutation and media-root symlink escape were established from direct filesystem call paths but not executed against a real remote package."
  ],
  "noStagedFiles": true,
  "notes": "plan.md and progress.md were absent. Jujutsu is used, so there is no Git staging area; no source edits were made. Post-review status also contained audit/02-core-correctness.md, audit/04-yaml-formats.md, audit/05-crowdanki.md, and audit/08-workbench-backend.md from concurrent reviewers; they were not created or modified by this review."
}
```
