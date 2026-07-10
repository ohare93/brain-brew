---
title: Workspace transaction contract
---

# Workspace transaction contract

`brainbrew` owns a journaled filesystem transaction module for future multi-file source mutations. The module is intentionally not yet wired into Workbench, media, import, format, lock, compose, or export commands; those migrations are tracked separately. Existing sequential-write behavior must not be described as batch-atomic.

## Interface

A caller:

1. validates each replacement's bytes with its format/domain validator, producing `ValidatedReplacement`;
2. supplies a workspace-relative `WorkspaceWrite` with `ExpectedTarget::Absent` or a SHA-256/length `FileFingerprint`;
3. validates the complete `WorkspaceTransactionPlan` without filesystem mutation;
4. commits the resulting `ValidatedWorkspaceTransaction`; and
5. invokes workspace recovery before retrying after an interrupted transaction.

Filesystem operations are behind `WorkspaceFilesystem`; semantic failures are behind `FailureInjector`. The production adapter is `RealWorkspaceFilesystem`.

## Fail-closed plan boundary

One plan is authorized under one canonical existing workspace root. Validation rejects:

- an empty plan or empty, absolute, parent-relative, current-relative, or non-UTF-8 target paths;
- targets under the reserved `.brainbrew-transactions` control directory;
- duplicate targets after canonical parent resolution;
- a missing target parent (the transaction does not create user-owned directory structure);
- a parent symlink that resolves outside the canonical workspace root;
- target symlinks, directories, devices, sockets, FIFOs, and other non-regular file types;
- a target parent or existing target with a filesystem/device ID different from the root;
- expected absence when a target exists, expected presence when it does not, or a stale expected fingerprint; and
- replacement bytes that have not passed a caller-provided validator.

The commit path acquires the cooperative workspace lock and repeats containment, file-type, filesystem, and expected-content checks before preparation. Existing target permissions are preserved. A new file uses the explicit mode carried by its validated replacement (default `0644`).

This contract deliberately rejects external include roots and cross-filesystem batches. It does not decide whether a later product version may support them through a different transaction model.

## Durable layout and state machine

Transactions use the same-root directory:

```text
<workspace>/.brainbrew-transactions/
  workspace.lock
  txn-.../
    journal.json
    replacements/<entry-index>
    backups/<entry-index>
```

The JSON journal contains a version, transaction ID, canonical root, complete entry list, original/replacement fingerprints, modes, preparation progress, and state.

```text
Preparing -> Prepared -> Committing{completed} -> Committed -> cleanup
                    \-> RollingBack{remaining} -> RolledBack -> cleanup
```

Before the first target replacement:

- the complete plan is present in a durable journal;
- every replacement is written and synced;
- every existing target has a synced backup with a verified fingerprint; and
- `Prepared`, then `Committing{completed: 0}`, is durably published.

Each target replacement is an individual same-filesystem rename followed by a parent-directory sync. After each replacement, commit progress is durably advanced. This is **recoverable multi-file commit**, not atomic multi-file rename.

An ordinary commit error attempts rollback before returning. If rollback succeeds, the error explicitly reports that the original workspace was restored. If rollback or cleanup fails, the transaction directory and journal remain for recovery.

## Restart recovery

Recovery takes the same cooperative lock and processes journals in deterministic directory order:

- `Preparing` or `Prepared`: verify every target is still original, then clean up;
- `Committing` or `RollingBack`: accept only an original or planned replacement fingerprint, then restore every original target and remove every planned-new target;
- `Committed`: retain the new tree and finish cleanup;
- `RolledBack`: retain the old tree and finish cleanup.

A directory created before the initial journal is safe to remove because target replacement is forbidden before journal publication. A rollback restore-stage file left by a crash is discarded and regenerated from the fingerprint-verified backup; its existing bytes are never trusted. A malformed journal, missing/corrupt backup, path escape, cross-filesystem parent, or target containing bytes that match neither the old nor new fingerprint stops recovery with an explicit error. Recovery never treats an unrecognized mixed tree as successful.

## Concurrency

`workspace.lock` is an advisory exclusive file lock. It serializes cooperating Brain Brew processes, and the validated plan is rechecked after lock acquisition. A pending journal blocks a new commit until recovery runs. Non-cooperating editors are detected by fingerprint checks before preparation and by old/new fingerprint checks during recovery, but advisory locking cannot prevent them from racing a commit.

## Durability and platform assumptions

The production implementation assumes:

- Unix device IDs and permission modes are available;
- staged files, backups, journals, and targets are on one filesystem;
- file `sync_all`, directory `sync_all`, and same-filesystem rename provide their documented local-filesystem persistence semantics;
- rename replaces a regular file atomically for one path; and
- the underlying filesystem and storage stack honor flushes after power loss.

The production adapter fails as unsupported on non-Unix platforms rather than weakening filesystem or permission validation. Network filesystems, unusual FUSE implementations, hardware that ignores flushes, and non-cooperating writers remain platform risks. Windows support requires a separately tested adapter because replacement rename, locking, device identity, and permission behavior differ.
