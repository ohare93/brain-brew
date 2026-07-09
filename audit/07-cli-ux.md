# CLI maintainer UX audit

## Review

### Correct

- The core read/report flows work against the repository's Ultimate Geography case study. `targets` returned 74 main targets and 100 targets when both manifests were supplied; `validate --target de-extended` reported 319 notes, one note type, six templates, and 546 media references; `explain --json` returned three overlays and 753 semantic changes. This agrees with the documented 74/26 split in `documentation/docs/examples/ultimate-geography.md:35-63`.
- Both documented UG verification gates passed exactly:
  - `target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets` -> exit 0, `✓ verified 74 targets`.
  - `target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml --all-targets` -> exit 0, `✓ verified 26 targets`.
- Human/JSON stream separation is good for the commands covered by the documented contract. `explain ... --target does-not-exist --json` returned exit 1, 1,422 bytes of JSON on stdout, and empty stderr, matching `documentation/docs/reference/cli.md:9-33` and the dispatcher in `crates/brain-brew-cli/src/main.rs:15-27,82-87`.
- Workbench starts on loopback with an ephemeral port and is script-discoverable: `brainbrew workbench serve ... --port 0 --no-open` printed `Workbench listening at http://127.0.0.1:37275`; `/api/health` returned status `ok`, and `/api/workspace` exposed 74 targets, 16 languages, and 69 fingerprints. The loopback bind is explicit at `crates/brain-brew-cli/src/commands/workbench.rs:162-177`.
- Package locking failed closed in the exercised path flow. `lock update --package anki-geo.ultimate-geography --path fixtures/ultimate-geography` and `lock verify` both exited 0; adding a temporary file to the live package made verify exit 1 with the expected/found NAR hashes. The check is wired at `crates/brain-brew-cli/src/commands/lock.rs:94-105`.
- A CrowdAnki export/import round trip produced a Canonical Deck that `validate` accepted, and `fmt` followed by `validate` remained green. Existing fixture coverage also checks semantic round-trip equivalence at `crates/brain-brew-cli/tests/ug_style_fixture.rs:99-120`.

### Severity-ranked findings

#### S1 — High: Workbench “atomic” multi-file apply can commit only part of an edit

`write_files_atomically` prepares and fsyncs all temporary files, but then renames them sequentially (`crates/brain-brew-cli/src/commands/workbench.rs:3878-3963`). If rename 2 fails after rename 1, there is no rollback. The API returns HTTP 500 while canonical source is already changed.

This is not hypothetical: the checked-in test deliberately injects a failure at rename index 1 and asserts the response says `updated files: deck.yaml` and `not updated files: da.yaml, nb.yaml` (`crates/brain-brew-cli/tests/cli.rs:1153-1184`). Command evidence:

```text
$ devenv shell cargo test -p brainbrew --test cli \
    workbench_apply_rename_failure_reports_updated_and_not_updated_files -- --exact
running 1 test
... ok
test result: ok. 1 passed
```

For an authoring UI that may update base plus several translation overlays in one apply (`workbench.rs:1007-1035`), a reported failure can therefore leave source semantically inconsistent. The helper should either implement backup/rollback/recovery metadata or stop presenting this as an atomic transaction and make recovery a first-class flow.

#### S1 — High: `media hash` writes source files before knowing the whole operation can succeed

The command processes sources in a loop and immediately calls `fs::write` for each changed file (`crates/brain-brew-cli/src/commands/media.rs:46-74`). A later missing media asset leaves earlier canonical sources modified even though the command exits 1.

Adversarial evidence used a valid target whose base declared existing `good.png` and whose extension declared missing `missing.png`:

```text
$ brainbrew media hash --manifest /tmp/.../brainbrew.yaml \
    --all-targets --media-root /tmp/.../assets
exit=1 base-hash-now-empty=no overlay-hash-empty=yes stdout-bytes=0
/tmp/.../assets/missing.png: No such file or directory (os error 2)
```

The base's hash was changed to `770e6076...`, while the overlay remained untouched. `media images-to-refs` has the same write-as-you-go shape at `media.rs:89-116`. All reads, transformations, and validation should complete before a transaction writes any source.

#### S1 — High: Import overwrites canonical source without `--force`, atomics, or even strict argument parsing

Import writes directly to the requested path (`crates/brain-brew-cli/src/commands/import.rs:27-36`). There is no existence check, force flag, staging file, backup, or parent transaction. In the exercised round trip:

```text
$ printf 'DO NOT OVERWRITE\n' > imported.yaml
$ brainbrew import crowdanki export/ --accept-suggested-ids \
    --out imported.yaml --bogus
exit=0 sentinel-preserved=no
stdout="imported crowdanki deck"
```

This can destroy maintainer-owned source, and the same invocation silently accepted `--bogus`. The parser merely searches for the first `--out` and ignores everything else (`crates/brain-brew-cli/src/args.rs:295-302`), directly contradicting the promise that unknown options are rejected in `documentation/docs/reference/cli.md:7`. Import needs a complete parser plus create-new-by-default/`--force` behavior and atomic replacement.

#### S2 — Medium: Media-complete verification/export is opt-in, and successful output does not say assets were skipped

Both full UG verifies passed with 546 declarations but no media root. In code, `verify` checks actual bytes/hashes only when `--media-root` is supplied; otherwise it checks references alone (`crates/brain-brew-cli/src/commands/verify.rs:47-79`). Likewise, export only validates/copies media under `Some(media_root)` (`crates/brain-brew-cli/src/commands/export.rs:86-106`).

Concrete output from an export without `--media-root` was a success report saying `media references: 546`, while the fresh output contained only `deck.json`. The reference docs disclose the distinction (`documentation/docs/reference/cli.md:141-149`), but the success message does not. A CI gate or release export can therefore be green and incomplete due to one omitted flag. At minimum, report `media assets: not checked/not copied` prominently; preferably permit a manifest-configured media root and make release verification fail closed when media is declared but assets are not checked.

#### S2 — Medium: Export reuses dirty output directories and is not transactional

`write_crowdanki_export` creates/reuses the output directory, overwrites `deck.json`, then copies media (`crates/brain-brew-cli/src/commands/export.rs:94-101`). It neither cleans stale files nor stages a complete replacement. Evidence:

```text
$ brainbrew export crowdanki ... --out /tmp/.../export
$ echo sentinel > /tmp/.../export/stale-from-old-export.bin
$ brainbrew export crowdanki ... --out /tmp/.../export
first-exit=0 second-exit=0 stale-file-after-rerun=yes files=2
```

Old media or unrelated sensitive files can be unintentionally included when the directory is archived. A copy failure after line 96 can also leave new `deck.json` with partial/old media. Export should stage a clean sibling directory and rename it into place, or require/offer an explicit clean mode and enumerate retained files.

#### S2 — Medium: The documented first compose fails when `build/` does not already exist

The reference, tour, quickstart, and built-in help all show `--out build/...` (`documentation/docs/reference/cli.md:68-74`, `documentation/docs/getting-started/cli-tour.md:27-33`, `documentation/docs/getting-started/quickstart.md:82-90`, `crates/brain-brew-cli/src/help.rs:48-49`), but compose calls `fs::write` without creating the parent (`crates/brain-brew-cli/src/commands/compose.rs:31-34,74-76`). Evidence from a fresh temp tree:

```text
$ brainbrew compose ... --out /tmp/.../missing/build/en.yaml
exit=1 output-exists=no stdout-bytes=0
/tmp/.../missing/build/en.yaml: No such file or directory (os error 2)
```

Export already creates its output directory. Compose should likewise create the parent or docs must include `mkdir -p build` everywhere.

#### S2 — Medium: The import onboarding/review flow is internally inconsistent and incomplete

The CLI refuses import without `--accept-suggested-ids` (`crates/brain-brew-cli/src/commands/import.rs:15-18`), yet the reference's only import command omits that required flag (`documentation/docs/reference/cli.md:93-99`). Running it exactly returned exit 1: `non-interactive CrowdAnki import requires --accept-suggested-ids for now`.

The built-in help exposes only all-at-once acceptance (`crates/brain-brew-cli/src/help.rs:54-55`); there is no dry-run/suggestions report or selective correction flow before canonical output. There is also no `init` command (`brainbrew init` -> exit 1, `unknown command "init"`), and top-level help lists no workspace scaffold (`help.rs:8-21`). Thus the current starting path is “accept every generated ID into a full deck, then manually build a manifest/workspace.” Add an inspectable import plan or temp-output review workflow and either an init/scaffold command or an explicit documented manual equivalent.

#### S2 — Medium: Parse failures lose the source filename

Read failures include paths, but successful reads followed by deck/overlay/manifest parse errors discard them (`crates/brain-brew-cli/src/io.rs:68-96`). Evidence:

```text
$ brainbrew targets --manifest /tmp/.../bad-manifest.yaml
exit=1
failed to parse manifest YAML: base: invalid type: sequence, expected a string at line 1 column 7

$ brainbrew validate fixtures/ultimate-geography/deck.yaml \
    --overlay /tmp/.../bad-overlay.yaml
exit=1
invalid overlay kind "wat"
```

Neither diagnostic names the malformed file. That becomes especially confusing for `verify --all-targets` and package/include stacks. Wrap every parser error with the display path and, for target operations, target/overlay context.

#### S2 — Medium: `translations --json` has JSON success but plain-text failure

Help advertises `translations ... --json` (`crates/brain-brew-cli/src/help.rs:66-67`), and a UG summary returned valid JSON with empty stderr. But JSON error dispatch only recognizes validate/explain/diff/targets (`crates/brain-brew-cli/src/main.rs:82-87`). Evidence:

```text
$ brainbrew translations --manifest /tmp/definitely-missing-brainbrew.yaml \
    --target de-standard --json
exit=1 stdout-bytes=0
stderr="No Brain Brew manifest found ..."
```

A script cannot consistently parse one selected output mode. Either extend the JSON error envelope to translations or remove/qualify its machine-readable claim.

#### S3 — Low: `diff` cannot directly serve as a CI “fail if changed” gate

A one-change comparison printed `1 semantic change` but exited 0. The implementation always returns `Ok(())` after reporting (`crates/brain-brew-cli/src/commands/diff.rs:20-35`), and help/reference expose no `--exit-code`/`--quiet` mode (`documentation/docs/reference/cli.md:101-109`). JSON makes custom scripting possible, but a conventional optional `--exit-code` would make this discoverable and robust.

#### S3 — Low: Global help/version bypass argument validation despite the documented strictness

The dispatcher returns command help whenever *any* argument is `--help` before parsing the rest (`crates/brain-brew-cli/src/main.rs:38-47`), and version ignores trailing arguments (`main.rs:67-70`). Evidence:

```text
$ brainbrew compose --bogus --help   # prints help, exit 0
$ brainbrew --version --bogus        # prints version, exit 0
```

This conflicts with the blanket rejection statement at `documentation/docs/reference/cli.md:7`. It is low impact, but strict parsing claims should either exclude help/version or behavior should be tightened.

### Notes and residual risks

- `plan.md` and `progress.md` named in the task were absent at the supplied paths, so there was no plan/progress context to inspect.
- No external checkout of upstream Ultimate Geography existed under `/home/jmo/Development/projects`; only `fixtures/ultimate-geography` and its duplicate under `default/` were available. The docs say this fixture merely “mirrors” upstream (`documentation/docs/examples/ultimate-geography.md:5-7`). The 74/26 verification is therefore strong repository-fixture evidence, not attestation against a live upstream checkout or real media tree.
- Direct, non-atomic `fs::write` remains in other mutators: `fmt` (`crates/brain-brew-cli/src/commands/fmt.rs:18-23`), lock update (`commands/lock.rs:76-81`), compose (`commands/compose.rs:31-34,74-76`), and both media migrations (`commands/media.rs:58-69,111-116`). Disk-full/process-kill behavior was not fault-injected for all of them.
- The full browser interaction surface was not exercised with a real browser; only server startup and JSON health/workspace APIs were tested. Existing repository browser/API tests reduce but do not eliminate that residual risk.

## Command evidence summary

| Flow | Command/result |
|---|---|
| Build | `devenv shell cargo build -p brainbrew` -> exit 0 |
| Help/init | `brainbrew --help` -> 0; `brainbrew init` -> 1 unknown command |
| Targets | main manifest -> 74 lines; both manifests -> 100 lines; JSON parsed |
| Validate | UG `de-extended` -> 0, 319 notes/6 templates/546 media refs |
| Compose | valid composition logic; missing output parent -> 1 |
| Explain | UG JSON -> 3 overlays/753 changes; missing target JSON -> 1 with stderr empty |
| Verify | main 74 and Hardcore 26 targets -> both 0 |
| Export/import/fmt | export -> 0; import -> 0 and validated; fmt then validate -> 0 |
| Diff | one semantic change -> exit 0 |
| Lock | update/verify -> 0; live package mutation -> verify 1 |
| Workbench | loopback server, health/workspace JSON -> 0; targeted rename-failure test -> pass |
| Media mutation | later missing asset -> exit 1 after earlier source changed |
| Translation JSON | success JSON -> 0; missing manifest with `--json` -> plain stderr/1 |

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Severity-ranked review findings include exact command results and file:line references across help, import, fmt, validate, compose, diff, export, targets, explain, verify, lock, media, translations, and workbench."
    }
  ],
  "changedFiles": [],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "devenv shell cargo build -p brainbrew",
      "result": "passed",
      "summary": "Fresh CLI build completed."
    },
    {
      "command": "target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew.yaml --all-targets",
      "result": "passed",
      "summary": "Verified all 74 main UG fixture targets."
    },
    {
      "command": "target/debug/brainbrew verify --manifest fixtures/ultimate-geography/brainbrew-hardcore.yaml --all-targets",
      "result": "passed",
      "summary": "Verified all 26 Hardcore companion fixture targets."
    },
    {
      "command": "devenv shell cargo test -p brainbrew --test cli workbench_apply_rename_failure_reports_updated_and_not_updated_files -- --exact",
      "result": "passed",
      "summary": "Confirmed the existing test codifies a partial multi-file rename commit."
    },
    {
      "command": "adversarial CLI matrix against fixtures/ultimate-geography and temporary workspaces",
      "result": "passed",
      "summary": "Captured exit codes, stdout/stderr, overwrite behavior, stale export retention, media partial writes, JSON behavior, lock integrity, and workbench health."
    }
  ],
  "validationOutput": [
    "UG main: verified 74 targets",
    "UG Hardcore: verified 26 targets",
    "UG de-extended: valid target with 319 notes and 546 media references",
    "Imported/fmt-formatted round-trip deck validated successfully"
  ],
  "residualRisks": [
    "No external live Ultimate Geography checkout or real media tree was available; testing used the repository fixture that mirrors upstream.",
    "Browser UI interaction and fault injection for every direct fs::write mutator were out of scope.",
    "Workbench and media commands have demonstrated partial-write behavior that remains unresolved."
  ],
  "noStagedFiles": true,
  "notes": "Review only; no product source or tests were edited. plan.md and progress.md were absent."
}
```
