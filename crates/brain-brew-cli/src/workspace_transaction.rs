//! Journaled, recoverable transactions for writes under one workspace root.
//!
//! This module deliberately does not claim that a sequence of renames is atomic. A transaction
//! makes each replacement individually atomic, records enough durable state to undo every
//! replacement, and requires restart recovery before another cooperating writer may proceed.

// This module intentionally lands before mutator migration (tasks 0050/0060).
#![allow(dead_code)]

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CONTROL_DIRECTORY: &str = ".brainbrew-transactions";
const JOURNAL_FILE: &str = "journal.json";
const JOURNAL_TEMP_FILE: &str = "journal.next";
const LOCK_FILE: &str = "workspace.lock";
const JOURNAL_VERSION: u32 = 1;
static TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// SHA-256 and byte length used for optimistic concurrency checks.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct FileFingerprint {
    pub(crate) sha256: String,
    pub(crate) length: u64,
}

impl FileFingerprint {
    pub(crate) fn for_bytes(bytes: &[u8]) -> Self {
        Self {
            sha256: format!("{:x}", Sha256::digest(bytes)),
            length: bytes.len() as u64,
        }
    }
}

/// The state a caller observed while constructing a transaction plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedTarget {
    Absent,
    Present(FileFingerprint),
}

/// Replacement bytes that have passed the caller's format/domain validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedReplacement {
    bytes: Vec<u8>,
    new_file_mode: u32,
}

impl ValidatedReplacement {
    pub(crate) fn validate(
        bytes: Vec<u8>,
        validator: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<Self, TransactionError> {
        validator(&bytes).map_err(|message| TransactionError::InvalidReplacement { message })?;
        Ok(Self {
            bytes,
            new_file_mode: 0o644,
        })
    }

    pub(crate) fn with_new_file_mode(mut self, mode: u32) -> Result<Self, TransactionError> {
        if mode & !0o7777 != 0 {
            return Err(TransactionError::InvalidReplacement {
                message: format!("invalid Unix file mode {mode:#o}"),
            });
        }
        self.new_file_mode = mode;
        Ok(self)
    }
}

/// One file replacement, addressed relative to the canonical workspace root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceWrite {
    pub(crate) target: PathBuf,
    pub(crate) expected: ExpectedTarget,
    pub(crate) replacement: ValidatedReplacement,
}

impl WorkspaceWrite {
    pub(crate) fn new(
        target: impl Into<PathBuf>,
        expected: ExpectedTarget,
        replacement: ValidatedReplacement,
    ) -> Self {
        Self {
            target: target.into(),
            expected,
            replacement,
        }
    }
}

/// An unvalidated transaction plan. Validation performs no filesystem mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceTransactionPlan {
    pub(crate) workspace_root: PathBuf,
    pub(crate) writes: Vec<WorkspaceWrite>,
}

impl WorkspaceTransactionPlan {
    pub(crate) fn new(workspace_root: impl Into<PathBuf>, writes: Vec<WorkspaceWrite>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            writes,
        }
    }

    pub(crate) fn validate(
        self,
        filesystem: &dyn WorkspaceFilesystem,
    ) -> Result<ValidatedWorkspaceTransaction, TransactionError> {
        validate_plan(self, filesystem)
    }
}

/// A complete, immutable plan whose paths, expected state, and filesystem have been checked.
#[derive(Clone, Debug)]
pub(crate) struct ValidatedWorkspaceTransaction {
    workspace_root: PathBuf,
    root_filesystem: u64,
    writes: Vec<ValidatedWrite>,
}

#[derive(Clone, Debug)]
struct ValidatedWrite {
    relative_target: String,
    canonical_parent: PathBuf,
    absolute_target: PathBuf,
    expected: ExpectedTarget,
    replacement: ValidatedReplacement,
    original_mode: Option<u32>,
}

/// File type and platform metadata needed by transaction validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceMetadata {
    pub(crate) kind: WorkspaceFileKind,
    pub(crate) filesystem: u64,
    pub(crate) mode: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceFileKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// An acquired cooperative workspace lock. Dropping it releases the lock.
pub(crate) trait WorkspaceLock: Send {}

/// Injectable filesystem boundary used by validation, commit, rollback, and recovery.
pub(crate) trait WorkspaceFilesystem: Send + Sync {
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    fn metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata>;
    fn symlink_metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn read_directories(&self, path: &Path) -> io::Result<Vec<PathBuf>>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write_new_durable(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()>;
    fn replace_durable(&self, source: &Path, target: &Path) -> io::Result<()>;
    fn remove_file_durable(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all_durable(&self, path: &Path) -> io::Result<()>;
    fn sync_directory(&self, path: &Path) -> io::Result<()>;
    fn try_lock(&self, path: &Path) -> io::Result<Box<dyn WorkspaceLock>>;
}

/// Production filesystem adapter. Unix device IDs and modes enforce the one-filesystem contract.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RealWorkspaceFilesystem;

struct RealWorkspaceLock(File);
impl WorkspaceLock for RealWorkspaceLock {}

impl Drop for RealWorkspaceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

impl WorkspaceFilesystem for RealWorkspaceFilesystem {
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata> {
        real_metadata(fs::metadata(path)?)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata> {
        real_metadata(fs::symlink_metadata(path)?)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn read_directories(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut directories = fs::read_dir(path)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_dir())
                    .map(|_| entry.path())
            })
            .collect::<Vec<_>>();
        directories.sort();
        Ok(directories)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn write_new_durable(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        set_file_mode(&file, mode)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        self.sync_directory(parent(path)?)
    }

    fn replace_durable(&self, source: &Path, target: &Path) -> io::Result<()> {
        fs::rename(source, target)?;
        self.sync_directory(parent(target)?)
    }

    fn remove_file_durable(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)?;
        self.sync_directory(parent(path)?)
    }

    fn remove_dir_all_durable(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)?;
        self.sync_directory(parent(path)?)
    }

    fn sync_directory(&self, path: &Path) -> io::Result<()> {
        File::open(path)?.sync_all()
    }

    fn try_lock(&self, path: &Path) -> io::Result<Box<dyn WorkspaceLock>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive()?;
        Ok(Box::new(RealWorkspaceLock(file)))
    }
}

#[cfg(unix)]
fn real_metadata(metadata: fs::Metadata) -> io::Result<WorkspaceMetadata> {
    use std::os::unix::fs::MetadataExt;
    let file_type = metadata.file_type();
    let kind = if file_type.is_file() {
        WorkspaceFileKind::File
    } else if file_type.is_dir() {
        WorkspaceFileKind::Directory
    } else if file_type.is_symlink() {
        WorkspaceFileKind::Symlink
    } else {
        WorkspaceFileKind::Other
    };
    Ok(WorkspaceMetadata {
        kind,
        filesystem: metadata.dev(),
        mode: metadata.mode() & 0o7777,
    })
}

#[cfg(not(unix))]
fn real_metadata(_metadata: fs::Metadata) -> io::Result<WorkspaceMetadata> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "workspace transactions require Unix filesystem identity and permission metadata",
    ))
}

#[cfg(unix)]
fn set_file_mode(file: &File, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_file_mode(_file: &File, _mode: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "workspace transactions require Unix permission metadata",
    ))
}

fn parent(path: &Path) -> io::Result<&Path> {
    path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no parent directory", path.display()),
        )
    })
}

/// A deterministic semantic failure point. `Crash` simulates process loss and skips auto-rollback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FailurePoint {
    CreateJournal,
    StageReplacement(usize),
    Backup(usize),
    MarkPrepared,
    Replace(usize),
    RecordCommit(usize),
    MarkCommitted,
    FinalizeCleanup,
    Rollback(usize),
    RecordRollback(usize),
    MarkRolledBack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InjectedFailure {
    Error,
    Crash,
}

pub(crate) trait FailureInjector: Send + Sync {
    fn check(&self, point: &FailurePoint) -> Option<InjectedFailure>;
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NoFailures;

impl FailureInjector for NoFailures {
    fn check(&self, _point: &FailurePoint) -> Option<InjectedFailure> {
        None
    }
}

/// Result of recovery for one durable transaction journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryAction {
    RolledBack { transaction_id: String },
    FinalizedCommit { transaction_id: String },
}

/// Transaction coordinator. All cooperating writers use one advisory lock per workspace root.
pub(crate) struct WorkspaceTransactionManager<'a> {
    filesystem: &'a dyn WorkspaceFilesystem,
    failures: &'a dyn FailureInjector,
}

impl<'a> WorkspaceTransactionManager<'a> {
    pub(crate) fn new(
        filesystem: &'a dyn WorkspaceFilesystem,
        failures: &'a dyn FailureInjector,
    ) -> Self {
        Self {
            filesystem,
            failures,
        }
    }

    pub(crate) fn commit(
        &self,
        transaction: ValidatedWorkspaceTransaction,
    ) -> Result<(), TransactionError> {
        let control = transaction.workspace_root.join(CONTROL_DIRECTORY);
        self.ensure_control_directory(
            &transaction.workspace_root,
            transaction.root_filesystem,
            &control,
        )?;
        let _lock = self.acquire_lock(&control)?;
        self.ensure_no_pending_transactions(&control)?;
        self.revalidate(&transaction)?;

        let transaction_id = transaction_id();
        let transaction_directory = control.join(&transaction_id);
        let replacements = transaction_directory.join("replacements");
        let backups = transaction_directory.join("backups");
        self.filesystem
            .create_dir_all(&replacements)
            .and_then(|()| self.filesystem.create_dir_all(&backups))
            .map_err(|source| {
                io_error(
                    "create transaction directories",
                    &transaction_directory,
                    source,
                )
            })?;
        self.filesystem
            .sync_directory(&control)
            .map_err(|source| io_error("sync transaction control directory", &control, source))?;

        let mut journal = Journal::new(&transaction_id, &transaction);
        let journal_path = transaction_directory.join(JOURNAL_FILE);
        if let Err(failure) = self.inject(&FailurePoint::CreateJournal) {
            return Err(self.interrupted(transaction_id, journal.state, failure));
        }
        self.write_initial_journal(&journal_path, &journal)?;

        self.prepare_complete_plan(
            &transaction,
            &transaction_directory,
            &mut journal,
            &journal_path,
        )?;

        // Catch non-cooperating edits made while replacements and backups were prepared.
        self.revalidate(&transaction)?;
        if let Err(failure) = self.inject(&FailurePoint::MarkPrepared) {
            return Err(self.interrupted(transaction_id, journal.state, failure));
        }
        journal.state = JournalState::Prepared;
        self.persist_journal(&journal_path, &journal)?;

        journal.state = JournalState::Committing { completed: 0 };
        self.persist_journal(&journal_path, &journal)?;
        for index in 0..transaction.writes.len() {
            if let Err(failure) = self.inject(&FailurePoint::Replace(index)) {
                return self.handle_commit_failure(
                    &journal_path,
                    &transaction_directory,
                    &mut journal,
                    failure,
                );
            }
            let staged = replacements.join(index.to_string());
            let write = &transaction.writes[index];
            let target = &write.absolute_target;
            if let Err(error) = validate_current_target(self.filesystem, write) {
                return self.handle_commit_error(
                    &journal_path,
                    &transaction_directory,
                    &mut journal,
                    error,
                );
            }
            if let Err(source) = self.filesystem.replace_durable(&staged, target) {
                return self.handle_commit_error(
                    &journal_path,
                    &transaction_directory,
                    &mut journal,
                    io_error("replace target", target, source),
                );
            }
            if let Err(failure) = self.inject(&FailurePoint::RecordCommit(index)) {
                return self.handle_commit_failure(
                    &journal_path,
                    &transaction_directory,
                    &mut journal,
                    failure,
                );
            }
            journal.state = JournalState::Committing {
                completed: index + 1,
            };
            if let Err(error) = self.persist_journal(&journal_path, &journal) {
                return self.handle_commit_error(
                    &journal_path,
                    &transaction_directory,
                    &mut journal,
                    error,
                );
            }
        }

        if let Err(failure) = self.inject(&FailurePoint::MarkCommitted) {
            return self.handle_commit_failure(
                &journal_path,
                &transaction_directory,
                &mut journal,
                failure,
            );
        }
        journal.state = JournalState::Committed;
        self.persist_journal(&journal_path, &journal)?;
        if let Err(failure) = self.inject(&FailurePoint::FinalizeCleanup) {
            return Err(self.interrupted(transaction_id, journal.state, failure));
        }
        self.cleanup_transaction(&transaction_directory)?;
        Ok(())
    }

    /// Recover every transaction under a workspace root while holding the cooperative lock.
    pub(crate) fn recover(
        &self,
        workspace_root: &Path,
    ) -> Result<Vec<RecoveryAction>, TransactionError> {
        let root = self
            .filesystem
            .canonicalize(workspace_root)
            .map_err(|source| io_error("canonicalize workspace root", workspace_root, source))?;
        let control = root.join(CONTROL_DIRECTORY);
        let root_filesystem = self
            .filesystem
            .metadata(&root)
            .map_err(|source| io_error("inspect recovery root", &root, source))?
            .filesystem;
        match self.filesystem.symlink_metadata(&control) {
            Ok(metadata) if metadata.kind == WorkspaceFileKind::Directory => {
                self.validate_control_directory(&root, root_filesystem, &control, metadata)?;
            }
            Ok(metadata) => {
                return Err(TransactionError::UnsafeControlPath {
                    path: control,
                    reason: format!("control path has unsupported type {:?}", metadata.kind),
                });
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(io_error(
                    "inspect transaction control directory",
                    &control,
                    source,
                ));
            }
        }
        let _lock = self.acquire_lock(&control)?;
        let mut actions = Vec::new();
        for directory in self
            .filesystem
            .read_directories(&control)
            .map_err(|source| io_error("list transaction journals", &control, source))?
        {
            let journal_path = directory.join(JOURNAL_FILE);
            if !path_exists(self.filesystem, &journal_path)? {
                // A crash before the initial journal cannot have replaced a target.
                let id = directory
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_owned();
                self.cleanup_transaction(&directory)?;
                actions.push(RecoveryAction::RolledBack { transaction_id: id });
                continue;
            }
            let mut journal = self.read_journal(&journal_path)?;
            self.validate_recovery_journal(&root, &directory, &journal)?;
            match journal.state {
                JournalState::Committed => {
                    self.verify_journal_tree(&journal, RecoveryTree::Replacement)?;
                    let id = journal.transaction_id.clone();
                    self.cleanup_transaction(&directory)?;
                    actions.push(RecoveryAction::FinalizedCommit { transaction_id: id });
                }
                JournalState::RolledBack => {
                    self.verify_journal_tree(&journal, RecoveryTree::Original)?;
                    let id = journal.transaction_id.clone();
                    self.cleanup_transaction(&directory)?;
                    actions.push(RecoveryAction::RolledBack { transaction_id: id });
                }
                JournalState::Preparing | JournalState::Prepared => {
                    // No target replacement is legal before Committing is durable. Verify that
                    // invariant instead of requiring backups that may still be in preparation.
                    for entry in &journal.entries {
                        let target = root.join(&entry.target);
                        let current = self.current_fingerprint(&target)?;
                        if current != entry.original {
                            return Err(TransactionError::RecoveryConflict {
                                path: target,
                                expected_original: entry.original.clone(),
                                expected_replacement: entry.replacement.clone(),
                                actual: current,
                            });
                        }
                    }
                    let id = journal.transaction_id.clone();
                    journal.state = JournalState::RolledBack;
                    self.persist_journal(&journal_path, &journal)?;
                    self.cleanup_transaction(&directory)?;
                    actions.push(RecoveryAction::RolledBack { transaction_id: id });
                }
                JournalState::Committing { .. } | JournalState::RollingBack { .. } => {
                    self.rollback(&journal_path, &directory, &mut journal)?;
                    let id = journal.transaction_id.clone();
                    self.cleanup_transaction(&directory)?;
                    actions.push(RecoveryAction::RolledBack { transaction_id: id });
                }
            }
        }
        Ok(actions)
    }

    fn prepare_complete_plan(
        &self,
        transaction: &ValidatedWorkspaceTransaction,
        transaction_directory: &Path,
        journal: &mut Journal,
        journal_path: &Path,
    ) -> Result<(), TransactionError> {
        for (index, write) in transaction.writes.iter().enumerate() {
            if let Err(failure) = self.inject(&FailurePoint::StageReplacement(index)) {
                return Err(self.interrupted(
                    journal.transaction_id.clone(),
                    journal.state.clone(),
                    failure,
                ));
            }
            let staged = transaction_directory
                .join("replacements")
                .join(index.to_string());
            let mode = write
                .original_mode
                .unwrap_or(write.replacement.new_file_mode);
            self.filesystem
                .write_new_durable(&staged, &write.replacement.bytes, mode)
                .map_err(|source| io_error("stage replacement", &staged, source))?;
            journal.prepared_replacements = index + 1;
            self.persist_journal(journal_path, journal)?;
        }
        for (index, write) in transaction.writes.iter().enumerate() {
            if write.original_mode.is_none() {
                journal.prepared_backups = index + 1;
                self.persist_journal(journal_path, journal)?;
                continue;
            }
            if let Err(failure) = self.inject(&FailurePoint::Backup(index)) {
                return Err(self.interrupted(
                    journal.transaction_id.clone(),
                    journal.state.clone(),
                    failure,
                ));
            }
            let backup = transaction_directory
                .join("backups")
                .join(index.to_string());
            let bytes = self
                .filesystem
                .read(&write.absolute_target)
                .map_err(|source| {
                    io_error("read target for backup", &write.absolute_target, source)
                })?;
            let actual = FileFingerprint::for_bytes(&bytes);
            match &write.expected {
                ExpectedTarget::Present(expected) if *expected == actual => {}
                _ => {
                    return Err(TransactionError::ExpectedStateChanged {
                        path: write.absolute_target.clone(),
                    });
                }
            }
            self.filesystem
                .write_new_durable(&backup, &bytes, write.original_mode.unwrap_or(0o600))
                .map_err(|source| io_error("write transaction backup", &backup, source))?;
            journal.prepared_backups = index + 1;
            self.persist_journal(journal_path, journal)?;
        }
        Ok(())
    }

    fn handle_commit_failure(
        &self,
        journal_path: &Path,
        transaction_directory: &Path,
        journal: &mut Journal,
        failure: InjectedFailure,
    ) -> Result<(), TransactionError> {
        if failure == InjectedFailure::Crash {
            return Err(self.interrupted(
                journal.transaction_id.clone(),
                journal.state.clone(),
                failure,
            ));
        }
        let original = TransactionError::Injected {
            point: "commit".to_owned(),
            recoverable_transaction: Some(journal.transaction_id.clone()),
        };
        self.rollback(journal_path, transaction_directory, journal)
            .and_then(|()| self.cleanup_transaction(transaction_directory))
            .map_or_else(Err, |()| {
                Err(TransactionError::RolledBack {
                    transaction_id: journal.transaction_id.clone(),
                    cause: Box::new(original),
                })
            })
    }

    fn handle_commit_error(
        &self,
        journal_path: &Path,
        transaction_directory: &Path,
        journal: &mut Journal,
        original: TransactionError,
    ) -> Result<(), TransactionError> {
        self.rollback(journal_path, transaction_directory, journal)
            .and_then(|()| self.cleanup_transaction(transaction_directory))
            .map_or_else(Err, |()| {
                Err(TransactionError::RolledBack {
                    transaction_id: journal.transaction_id.clone(),
                    cause: Box::new(original),
                })
            })
    }

    fn rollback(
        &self,
        journal_path: &Path,
        transaction_directory: &Path,
        journal: &mut Journal,
    ) -> Result<(), TransactionError> {
        journal.state = JournalState::RollingBack {
            remaining: journal.entries.len(),
        };
        self.persist_journal(journal_path, journal)?;
        for index in (0..journal.entries.len()).rev() {
            if let Err(failure) = self.inject(&FailurePoint::Rollback(index)) {
                return Err(self.interrupted(
                    journal.transaction_id.clone(),
                    journal.state.clone(),
                    failure,
                ));
            }
            let entry = &journal.entries[index];
            let target = journal.workspace_root.join(&entry.target);
            self.assert_recoverable_target(&target, entry)?;
            if entry.original.is_some() {
                let backup = transaction_directory
                    .join("backups")
                    .join(index.to_string());
                let rollback_stage = transaction_directory
                    .join("backups")
                    .join(format!("{index}.restore"));
                let bytes = self
                    .filesystem
                    .read(&backup)
                    .map_err(|source| io_error("read rollback backup", &backup, source))?;
                let fingerprint = FileFingerprint::for_bytes(&bytes);
                if entry.original.as_ref() != Some(&fingerprint) {
                    return Err(TransactionError::CorruptJournal {
                        path: backup,
                        reason: "backup fingerprint does not match journal".to_owned(),
                    });
                }
                self.filesystem
                    .write_new_durable(
                        &rollback_stage,
                        &bytes,
                        entry.original_mode.unwrap_or(0o600),
                    )
                    .map_err(|source| io_error("stage rollback", &rollback_stage, source))?;
                self.filesystem
                    .replace_durable(&rollback_stage, &target)
                    .map_err(|source| io_error("restore target", &target, source))?;
            } else if path_exists(self.filesystem, &target)? {
                self.filesystem
                    .remove_file_durable(&target)
                    .map_err(|source| io_error("remove newly-created target", &target, source))?;
            }
            if let Err(failure) = self.inject(&FailurePoint::RecordRollback(index)) {
                return Err(self.interrupted(
                    journal.transaction_id.clone(),
                    journal.state.clone(),
                    failure,
                ));
            }
            journal.state = JournalState::RollingBack { remaining: index };
            self.persist_journal(journal_path, journal)?;
        }
        if let Err(failure) = self.inject(&FailurePoint::MarkRolledBack) {
            return Err(self.interrupted(
                journal.transaction_id.clone(),
                journal.state.clone(),
                failure,
            ));
        }
        journal.state = JournalState::RolledBack;
        self.persist_journal(journal_path, journal)
    }

    fn assert_recoverable_target(
        &self,
        target: &Path,
        entry: &JournalEntry,
    ) -> Result<(), TransactionError> {
        let current = self.current_fingerprint(target)?;
        if current.as_ref() == entry.original.as_ref()
            || current.as_ref() == Some(&entry.replacement)
            || (current.is_none() && entry.original.is_none())
        {
            Ok(())
        } else {
            Err(TransactionError::RecoveryConflict {
                path: target.to_path_buf(),
                expected_original: entry.original.clone(),
                expected_replacement: entry.replacement.clone(),
                actual: current,
            })
        }
    }

    fn verify_journal_tree(
        &self,
        journal: &Journal,
        tree: RecoveryTree,
    ) -> Result<(), TransactionError> {
        for entry in &journal.entries {
            let target = journal.workspace_root.join(&entry.target);
            let current = self.current_fingerprint(&target)?;
            let expected = match tree {
                RecoveryTree::Original => entry.original.as_ref(),
                RecoveryTree::Replacement => Some(&entry.replacement),
            };
            if current.as_ref() != expected {
                return Err(TransactionError::RecoveryConflict {
                    path: target,
                    expected_original: entry.original.clone(),
                    expected_replacement: entry.replacement.clone(),
                    actual: current,
                });
            }
        }
        Ok(())
    }

    fn current_fingerprint(
        &self,
        target: &Path,
    ) -> Result<Option<FileFingerprint>, TransactionError> {
        match self.filesystem.read(target) {
            Ok(bytes) => Ok(Some(FileFingerprint::for_bytes(&bytes))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(io_error("read target during recovery", target, source)),
        }
    }

    fn revalidate(
        &self,
        transaction: &ValidatedWorkspaceTransaction,
    ) -> Result<(), TransactionError> {
        let root_metadata = self
            .filesystem
            .metadata(&transaction.workspace_root)
            .map_err(|source| {
                io_error(
                    "inspect workspace root",
                    &transaction.workspace_root,
                    source,
                )
            })?;
        if root_metadata.filesystem != transaction.root_filesystem {
            return Err(TransactionError::ExpectedStateChanged {
                path: transaction.workspace_root.clone(),
            });
        }
        for write in &transaction.writes {
            let current_parent = self
                .filesystem
                .canonicalize(&write.canonical_parent)
                .map_err(|source| {
                    io_error(
                        "canonicalize target parent",
                        &write.canonical_parent,
                        source,
                    )
                })?;
            if current_parent != write.canonical_parent
                || !current_parent.starts_with(&transaction.workspace_root)
            {
                return Err(TransactionError::OutsideWorkspace {
                    path: PathBuf::from(&write.relative_target),
                    resolved: current_parent,
                });
            }
            match self.filesystem.symlink_metadata(&write.absolute_target) {
                Ok(metadata) if metadata.kind != WorkspaceFileKind::File => {
                    return Err(TransactionError::UnsupportedFileType {
                        path: write.absolute_target.clone(),
                        kind: metadata.kind,
                    });
                }
                Ok(metadata) if metadata.filesystem != transaction.root_filesystem => {
                    return Err(TransactionError::CrossFilesystem {
                        path: PathBuf::from(&write.relative_target),
                        root_filesystem: transaction.root_filesystem,
                        target_filesystem: metadata.filesystem,
                    });
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(io_error(
                        "inspect transaction target",
                        &write.absolute_target,
                        source,
                    ));
                }
            }
            validate_current_target(self.filesystem, write)?;
        }
        Ok(())
    }

    fn ensure_control_directory(
        &self,
        root: &Path,
        root_filesystem: u64,
        control: &Path,
    ) -> Result<(), TransactionError> {
        match self.filesystem.symlink_metadata(control) {
            Ok(metadata) => {
                self.validate_control_directory(root, root_filesystem, control, metadata)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.filesystem.create_dir_all(control).map_err(|source| {
                    io_error("create transaction control directory", control, source)
                })?;
                self.filesystem
                    .sync_directory(root)
                    .map_err(|source| io_error("sync workspace root", root, source))?;
                let metadata = self
                    .filesystem
                    .symlink_metadata(control)
                    .map_err(|source| {
                        io_error("inspect transaction control directory", control, source)
                    })?;
                self.validate_control_directory(root, root_filesystem, control, metadata)
            }
            Err(source) => Err(io_error(
                "inspect transaction control directory",
                control,
                source,
            )),
        }
    }

    fn validate_control_directory(
        &self,
        root: &Path,
        root_filesystem: u64,
        control: &Path,
        metadata: WorkspaceMetadata,
    ) -> Result<(), TransactionError> {
        if metadata.kind != WorkspaceFileKind::Directory {
            return Err(TransactionError::UnsafeControlPath {
                path: control.to_path_buf(),
                reason: format!("control path has unsupported type {:?}", metadata.kind),
            });
        }
        let canonical = self.filesystem.canonicalize(control).map_err(|source| {
            io_error(
                "canonicalize transaction control directory",
                control,
                source,
            )
        })?;
        if canonical != control
            || !canonical.starts_with(root)
            || metadata.filesystem != root_filesystem
        {
            return Err(TransactionError::UnsafeControlPath {
                path: control.to_path_buf(),
                reason: "control directory escapes the root or crosses filesystems".to_owned(),
            });
        }
        Ok(())
    }

    fn acquire_lock(&self, control: &Path) -> Result<Box<dyn WorkspaceLock>, TransactionError> {
        let lock_path = control.join(LOCK_FILE);
        self.filesystem
            .try_lock(&lock_path)
            .map_err(|source| TransactionError::WorkspaceBusy {
                path: lock_path,
                source,
            })
    }

    fn ensure_no_pending_transactions(&self, control: &Path) -> Result<(), TransactionError> {
        let pending = self
            .filesystem
            .read_directories(control)
            .map_err(|source| io_error("list pending transactions", control, source))?;
        if let Some(path) = pending.first() {
            return Err(TransactionError::RecoveryRequired { path: path.clone() });
        }
        Ok(())
    }

    fn write_initial_journal(
        &self,
        path: &Path,
        journal: &Journal,
    ) -> Result<(), TransactionError> {
        let bytes = serde_json::to_vec_pretty(journal).map_err(|error| {
            TransactionError::CorruptJournal {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        self.filesystem
            .write_new_durable(path, &bytes, 0o600)
            .map_err(|source| io_error("write initial transaction journal", path, source))
    }

    fn persist_journal(&self, path: &Path, journal: &Journal) -> Result<(), TransactionError> {
        let bytes = serde_json::to_vec_pretty(journal).map_err(|error| {
            TransactionError::CorruptJournal {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        let temp = path.with_file_name(JOURNAL_TEMP_FILE);
        if path_exists(self.filesystem, &temp)? {
            self.filesystem
                .remove_file_durable(&temp)
                .map_err(|source| io_error("remove stale journal update", &temp, source))?;
        }
        self.filesystem
            .write_new_durable(&temp, &bytes, 0o600)
            .map_err(|source| io_error("write transaction journal update", &temp, source))?;
        self.filesystem
            .replace_durable(&temp, path)
            .map_err(|source| io_error("publish transaction journal update", path, source))
    }

    fn read_journal(&self, path: &Path) -> Result<Journal, TransactionError> {
        let bytes = self
            .filesystem
            .read(path)
            .map_err(|source| io_error("read transaction journal", path, source))?;
        let journal: Journal =
            serde_json::from_slice(&bytes).map_err(|error| TransactionError::CorruptJournal {
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
        if journal.version != JOURNAL_VERSION {
            return Err(TransactionError::CorruptJournal {
                path: path.to_path_buf(),
                reason: format!("unsupported journal version {}", journal.version),
            });
        }
        Ok(journal)
    }

    fn validate_recovery_journal(
        &self,
        root: &Path,
        transaction_directory: &Path,
        journal: &Journal,
    ) -> Result<(), TransactionError> {
        if journal.workspace_root != root {
            return Err(TransactionError::CorruptJournal {
                path: transaction_directory.join(JOURNAL_FILE),
                reason: "journal workspace root does not match recovery root".to_owned(),
            });
        }
        if transaction_directory
            .file_name()
            .and_then(|name| name.to_str())
            != Some(journal.transaction_id.as_str())
        {
            return Err(TransactionError::CorruptJournal {
                path: transaction_directory.join(JOURNAL_FILE),
                reason: "journal transaction ID does not match its directory".to_owned(),
            });
        }
        let root_filesystem = self
            .filesystem
            .metadata(root)
            .map_err(|source| io_error("inspect recovery root", root, source))?
            .filesystem;
        for entry in &journal.entries {
            let target = Path::new(&entry.target);
            validate_relative_target(target)?;
            let requested_parent = root.join(target.parent().unwrap_or_else(|| Path::new("")));
            let canonical_parent =
                self.filesystem
                    .canonicalize(&requested_parent)
                    .map_err(|source| {
                        io_error(
                            "canonicalize recovery target parent",
                            &requested_parent,
                            source,
                        )
                    })?;
            if !canonical_parent.starts_with(root) {
                return Err(TransactionError::OutsideWorkspace {
                    path: target.to_path_buf(),
                    resolved: canonical_parent,
                });
            }
            let filesystem = self
                .filesystem
                .metadata(&canonical_parent)
                .map_err(|source| {
                    io_error("inspect recovery target parent", &canonical_parent, source)
                })?
                .filesystem;
            if filesystem != root_filesystem {
                return Err(TransactionError::CrossFilesystem {
                    path: target.to_path_buf(),
                    root_filesystem,
                    target_filesystem: filesystem,
                });
            }
        }
        Ok(())
    }

    fn cleanup_transaction(&self, directory: &Path) -> Result<(), TransactionError> {
        self.filesystem
            .remove_dir_all_durable(directory)
            .map_err(|source| io_error("remove completed transaction", directory, source))
    }

    fn inject(&self, point: &FailurePoint) -> Result<(), InjectedFailure> {
        self.failures.check(point).map_or(Ok(()), Err)
    }

    fn interrupted(
        &self,
        transaction_id: String,
        state: JournalState,
        failure: InjectedFailure,
    ) -> TransactionError {
        TransactionError::Interrupted {
            transaction_id,
            state,
            simulated_crash: failure == InjectedFailure::Crash,
        }
    }
}

fn validate_plan(
    plan: WorkspaceTransactionPlan,
    filesystem: &dyn WorkspaceFilesystem,
) -> Result<ValidatedWorkspaceTransaction, TransactionError> {
    if plan.writes.is_empty() {
        return Err(TransactionError::EmptyPlan);
    }
    let workspace_root = filesystem
        .canonicalize(&plan.workspace_root)
        .map_err(|source| io_error("canonicalize workspace root", &plan.workspace_root, source))?;
    let root_metadata = filesystem
        .metadata(&workspace_root)
        .map_err(|source| io_error("inspect workspace root", &workspace_root, source))?;
    if root_metadata.kind != WorkspaceFileKind::Directory {
        return Err(TransactionError::UnsupportedFileType {
            path: workspace_root,
            kind: root_metadata.kind,
        });
    }

    let mut seen = BTreeSet::new();
    let mut writes = Vec::with_capacity(plan.writes.len());
    for write in plan.writes {
        let relative_target = validate_relative_target(&write.target)?;
        let parent_relative = write.target.parent().unwrap_or_else(|| Path::new(""));
        let requested_parent = workspace_root.join(parent_relative);
        let canonical_parent = filesystem
            .canonicalize(&requested_parent)
            .map_err(|source| {
                if source.kind() == io::ErrorKind::NotFound {
                    TransactionError::MissingParent {
                        path: requested_parent.clone(),
                    }
                } else {
                    io_error("canonicalize target parent", &requested_parent, source)
                }
            })?;
        if !canonical_parent.starts_with(&workspace_root) {
            return Err(TransactionError::OutsideWorkspace {
                path: write.target,
                resolved: canonical_parent,
            });
        }
        let parent_metadata = filesystem
            .metadata(&canonical_parent)
            .map_err(|source| io_error("inspect target parent", &canonical_parent, source))?;
        if parent_metadata.kind != WorkspaceFileKind::Directory {
            return Err(TransactionError::UnsupportedFileType {
                path: canonical_parent,
                kind: parent_metadata.kind,
            });
        }
        if parent_metadata.filesystem != root_metadata.filesystem {
            return Err(TransactionError::CrossFilesystem {
                path: write.target,
                root_filesystem: root_metadata.filesystem,
                target_filesystem: parent_metadata.filesystem,
            });
        }
        let file_name =
            write
                .target
                .file_name()
                .ok_or_else(|| TransactionError::InvalidTarget {
                    path: write.target.clone(),
                    reason: "target has no file name".to_owned(),
                })?;
        let absolute_target = canonical_parent.join(file_name);
        if !seen.insert(absolute_target.clone()) {
            return Err(TransactionError::DuplicateTarget { path: write.target });
        }

        let metadata = match filesystem.symlink_metadata(&absolute_target) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(io_error(
                    "inspect transaction target",
                    &absolute_target,
                    source,
                ));
            }
        };
        let original_mode = match metadata {
            Some(metadata) => {
                if metadata.kind != WorkspaceFileKind::File {
                    return Err(TransactionError::UnsupportedFileType {
                        path: absolute_target,
                        kind: metadata.kind,
                    });
                }
                if metadata.filesystem != root_metadata.filesystem {
                    return Err(TransactionError::CrossFilesystem {
                        path: write.target,
                        root_filesystem: root_metadata.filesystem,
                        target_filesystem: metadata.filesystem,
                    });
                }
                Some(metadata.mode)
            }
            None => None,
        };

        let validated = ValidatedWrite {
            relative_target,
            canonical_parent,
            absolute_target,
            expected: write.expected,
            replacement: write.replacement,
            original_mode,
        };
        validate_current_target(filesystem, &validated)?;
        writes.push(validated);
    }

    Ok(ValidatedWorkspaceTransaction {
        workspace_root,
        root_filesystem: root_metadata.filesystem,
        writes,
    })
}

fn validate_relative_target(path: &Path) -> Result<String, TransactionError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(TransactionError::InvalidTarget {
            path: path.to_path_buf(),
            reason: "target must be a non-empty relative path".to_owned(),
        });
    }
    if path
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == std::ffi::OsStr::new(CONTROL_DIRECTORY))
    {
        return Err(TransactionError::InvalidTarget {
            path: path.to_path_buf(),
            reason: "the transaction control directory is reserved".to_owned(),
        });
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(TransactionError::InvalidTarget {
                path: path.to_path_buf(),
                reason: "target may contain only normal relative path components".to_owned(),
            });
        }
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| TransactionError::InvalidTarget {
            path: path.to_path_buf(),
            reason: "target path must be valid UTF-8 for the durable journal".to_owned(),
        })
}

fn validate_current_target(
    filesystem: &dyn WorkspaceFilesystem,
    write: &ValidatedWrite,
) -> Result<(), TransactionError> {
    let current = match filesystem.read(&write.absolute_target) {
        Ok(bytes) => ExpectedTarget::Present(FileFingerprint::for_bytes(&bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => ExpectedTarget::Absent,
        Err(source) => {
            return Err(io_error(
                "read transaction target",
                &write.absolute_target,
                source,
            ));
        }
    };
    if current == write.expected {
        Ok(())
    } else {
        Err(TransactionError::ExpectedStateMismatch {
            path: write.absolute_target.clone(),
            expected: write.expected.clone(),
            actual: current,
        })
    }
}

fn path_exists(
    filesystem: &dyn WorkspaceFilesystem,
    path: &Path,
) -> Result<bool, TransactionError> {
    match filesystem.symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(io_error("inspect path", path, source)),
    }
}

fn transaction_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("txn-{}-{nanos}-{sequence}", std::process::id())
}

#[derive(Clone, Copy, Debug)]
enum RecoveryTree {
    Original,
    Replacement,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Journal {
    version: u32,
    transaction_id: String,
    workspace_root: PathBuf,
    state: JournalState,
    prepared_replacements: usize,
    prepared_backups: usize,
    entries: Vec<JournalEntry>,
}

impl Journal {
    fn new(transaction_id: &str, transaction: &ValidatedWorkspaceTransaction) -> Self {
        Self {
            version: JOURNAL_VERSION,
            transaction_id: transaction_id.to_owned(),
            workspace_root: transaction.workspace_root.clone(),
            state: JournalState::Preparing,
            prepared_replacements: 0,
            prepared_backups: 0,
            entries: transaction
                .writes
                .iter()
                .map(|write| JournalEntry {
                    target: write.relative_target.clone(),
                    original: match &write.expected {
                        ExpectedTarget::Absent => None,
                        ExpectedTarget::Present(fingerprint) => Some(fingerprint.clone()),
                    },
                    replacement: FileFingerprint::for_bytes(&write.replacement.bytes),
                    original_mode: write.original_mode,
                    replacement_mode: write
                        .original_mode
                        .unwrap_or(write.replacement.new_file_mode),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub(crate) enum JournalState {
    Preparing,
    Prepared,
    Committing { completed: usize },
    Committed,
    RollingBack { remaining: usize },
    RolledBack,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JournalEntry {
    target: String,
    original: Option<FileFingerprint>,
    replacement: FileFingerprint,
    original_mode: Option<u32>,
    replacement_mode: u32,
}

#[derive(Debug)]
pub(crate) enum TransactionError {
    EmptyPlan,
    InvalidReplacement {
        message: String,
    },
    InvalidTarget {
        path: PathBuf,
        reason: String,
    },
    MissingParent {
        path: PathBuf,
    },
    OutsideWorkspace {
        path: PathBuf,
        resolved: PathBuf,
    },
    DuplicateTarget {
        path: PathBuf,
    },
    UnsupportedFileType {
        path: PathBuf,
        kind: WorkspaceFileKind,
    },
    CrossFilesystem {
        path: PathBuf,
        root_filesystem: u64,
        target_filesystem: u64,
    },
    ExpectedStateMismatch {
        path: PathBuf,
        expected: ExpectedTarget,
        actual: ExpectedTarget,
    },
    ExpectedStateChanged {
        path: PathBuf,
    },
    WorkspaceBusy {
        path: PathBuf,
        source: io::Error,
    },
    RecoveryRequired {
        path: PathBuf,
    },
    UnsafeControlPath {
        path: PathBuf,
        reason: String,
    },
    CorruptJournal {
        path: PathBuf,
        reason: String,
    },
    RecoveryConflict {
        path: PathBuf,
        expected_original: Option<FileFingerprint>,
        expected_replacement: FileFingerprint,
        actual: Option<FileFingerprint>,
    },
    Injected {
        point: String,
        recoverable_transaction: Option<String>,
    },
    Interrupted {
        transaction_id: String,
        state: JournalState,
        simulated_crash: bool,
    },
    RolledBack {
        transaction_id: String,
        cause: Box<TransactionError>,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlan => write!(formatter, "workspace transaction plan is empty"),
            Self::InvalidReplacement { message } => {
                write!(formatter, "replacement bytes failed validation: {message}")
            }
            Self::InvalidTarget { path, reason } => {
                write!(
                    formatter,
                    "invalid transaction target {}: {reason}",
                    path.display()
                )
            }
            Self::MissingParent { path } => write!(
                formatter,
                "transaction target parent {} does not exist; parent creation is outside this contract",
                path.display()
            ),
            Self::OutsideWorkspace { path, resolved } => write!(
                formatter,
                "transaction target {} escapes the workspace root through {}",
                path.display(),
                resolved.display()
            ),
            Self::DuplicateTarget { path } => {
                write!(formatter, "duplicate transaction target {}", path.display())
            }
            Self::UnsupportedFileType { path, kind } => write!(
                formatter,
                "unsupported transaction target type {kind:?} at {}",
                path.display()
            ),
            Self::CrossFilesystem {
                path,
                root_filesystem,
                target_filesystem,
            } => write!(
                formatter,
                "transaction target {} is on filesystem {target_filesystem}, not workspace filesystem {root_filesystem}",
                path.display()
            ),
            Self::ExpectedStateMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "transaction target {} does not match expected state {expected:?}; found {actual:?}",
                path.display()
            ),
            Self::ExpectedStateChanged { path } => write!(
                formatter,
                "transaction target {} changed after validation",
                path.display()
            ),
            Self::WorkspaceBusy { path, source } => write!(
                formatter,
                "workspace transaction lock {} is busy: {source}",
                path.display()
            ),
            Self::RecoveryRequired { path } => write!(
                formatter,
                "workspace has a pending transaction at {}; recover it before committing",
                path.display()
            ),
            Self::UnsafeControlPath { path, reason } => write!(
                formatter,
                "unsafe workspace transaction control path {}: {reason}",
                path.display()
            ),
            Self::CorruptJournal { path, reason } => write!(
                formatter,
                "transaction journal or backup {} is corrupt: {reason}",
                path.display()
            ),
            Self::RecoveryConflict {
                path,
                expected_original,
                expected_replacement,
                actual,
            } => write!(
                formatter,
                "cannot recover {}: current fingerprint {actual:?} is neither original {expected_original:?} nor replacement {expected_replacement:?}",
                path.display()
            ),
            Self::Injected {
                point,
                recoverable_transaction,
            } => write!(
                formatter,
                "injected transaction failure at {point}; recoverable transaction: {recoverable_transaction:?}"
            ),
            Self::Interrupted {
                transaction_id,
                state,
                simulated_crash,
            } => write!(
                formatter,
                "transaction {transaction_id} interrupted in state {state:?} (simulated_crash={simulated_crash}); durable recovery is required"
            ),
            Self::RolledBack {
                transaction_id,
                cause,
            } => write!(
                formatter,
                "transaction {transaction_id} failed and restored the original workspace: {cause}"
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WorkspaceBusy { source, .. } | Self::Io { source, .. } => Some(source),
            Self::RolledBack { cause, .. } => Some(cause),
            _ => None,
        }
    }
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> TransactionError {
    TransactionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn replacement(text: &str) -> ValidatedReplacement {
        ValidatedReplacement::validate(text.as_bytes().to_vec(), |bytes| {
            if bytes.is_empty() {
                Err("empty source".to_owned())
            } else {
                Ok(())
            }
        })
        .unwrap()
    }

    fn expected(path: &Path) -> ExpectedTarget {
        ExpectedTarget::Present(FileFingerprint::for_bytes(&fs::read(path).unwrap()))
    }

    fn fixture() -> (TempDir, WorkspaceTransactionPlan) {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("deck.yaml"), b"old deck\n").unwrap();
        fs::write(directory.path().join("overlay.yaml"), b"old overlay\n").unwrap();
        let plan = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![
                WorkspaceWrite::new(
                    "deck.yaml",
                    expected(&directory.path().join("deck.yaml")),
                    replacement("new deck\n"),
                ),
                WorkspaceWrite::new(
                    "overlay.yaml",
                    expected(&directory.path().join("overlay.yaml")),
                    replacement("new overlay\n"),
                ),
                WorkspaceWrite::new("created.yaml", ExpectedTarget::Absent, replacement("new\n")),
            ],
        );
        (directory, plan)
    }

    fn assert_original(root: &Path) {
        assert_eq!(fs::read(root.join("deck.yaml")).unwrap(), b"old deck\n");
        assert_eq!(
            fs::read(root.join("overlay.yaml")).unwrap(),
            b"old overlay\n"
        );
        assert!(!root.join("created.yaml").exists());
    }

    fn assert_replaced(root: &Path) {
        assert_eq!(fs::read(root.join("deck.yaml")).unwrap(), b"new deck\n");
        assert_eq!(
            fs::read(root.join("overlay.yaml")).unwrap(),
            b"new overlay\n"
        );
        assert_eq!(fs::read(root.join("created.yaml")).unwrap(), b"new\n");
    }

    #[derive(Debug)]
    struct FailOnce {
        point: FailurePoint,
        kind: InjectedFailure,
        fired: Mutex<bool>,
    }

    impl FailOnce {
        fn new(point: FailurePoint, kind: InjectedFailure) -> Self {
            Self {
                point,
                kind,
                fired: Mutex::new(false),
            }
        }
    }

    impl FailureInjector for FailOnce {
        fn check(&self, point: &FailurePoint) -> Option<InjectedFailure> {
            let mut fired = self.fired.lock().unwrap();
            if !*fired && point == &self.point {
                *fired = true;
                Some(self.kind)
            } else {
                None
            }
        }
    }

    #[test]
    fn validates_replacements_expected_fingerprints_and_commits_complete_plan() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .commit(transaction)
            .unwrap();
        assert_replaced(directory.path());
        assert_eq!(
            fs::read_dir(directory.path().join(CONTROL_DIRECTORY))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().unwrap().is_dir())
                .count(),
            0
        );
    }

    #[test]
    fn replacement_validation_is_required_before_a_plan_can_be_built() {
        let error = ValidatedReplacement::validate(Vec::new(), |_| Err("not canonical".to_owned()))
            .unwrap_err();
        assert!(matches!(error, TransactionError::InvalidReplacement { .. }));
    }

    #[test]
    fn validation_rejects_duplicate_external_parent_and_symlink_escape_without_mutation() {
        let filesystem = RealWorkspaceFilesystem;
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("deck.yaml"), b"old\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), directory.path().join("escape")).unwrap();

        let duplicate = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![
                WorkspaceWrite::new(
                    "deck.yaml",
                    expected(&directory.path().join("deck.yaml")),
                    replacement("a"),
                ),
                WorkspaceWrite::new(
                    "deck.yaml",
                    expected(&directory.path().join("deck.yaml")),
                    replacement("b"),
                ),
            ],
        );
        assert!(matches!(
            duplicate.validate(&filesystem).unwrap_err(),
            TransactionError::DuplicateTarget { .. }
        ));

        let absolute = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![WorkspaceWrite::new(
                outside.path().join("outside.yaml"),
                ExpectedTarget::Absent,
                replacement("outside"),
            )],
        );
        assert!(matches!(
            absolute.validate(&filesystem).unwrap_err(),
            TransactionError::InvalidTarget { .. }
        ));

        #[cfg(unix)]
        {
            let escape = WorkspaceTransactionPlan::new(
                directory.path(),
                vec![WorkspaceWrite::new(
                    "escape/outside.yaml",
                    ExpectedTarget::Absent,
                    replacement("outside"),
                )],
            );
            assert!(matches!(
                escape.validate(&filesystem).unwrap_err(),
                TransactionError::OutsideWorkspace { .. }
            ));
        }
        assert_eq!(
            fs::read(directory.path().join("deck.yaml")).unwrap(),
            b"old\n"
        );
        assert!(!outside.path().join("outside.yaml").exists());
        assert!(!directory.path().join(CONTROL_DIRECTORY).exists());
    }

    #[test]
    fn validation_rejects_stale_expected_state_and_unsupported_target_types() {
        let filesystem = RealWorkspaceFilesystem;
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("deck.yaml"), b"current\n").unwrap();
        fs::create_dir(directory.path().join("target-directory")).unwrap();
        let stale = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![WorkspaceWrite::new(
                "deck.yaml",
                ExpectedTarget::Present(FileFingerprint::for_bytes(b"stale\n")),
                replacement("next"),
            )],
        );
        assert!(matches!(
            stale.validate(&filesystem).unwrap_err(),
            TransactionError::ExpectedStateMismatch { .. }
        ));
        let directory_target = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![WorkspaceWrite::new(
                "target-directory",
                ExpectedTarget::Absent,
                replacement("next"),
            )],
        );
        assert!(matches!(
            directory_target.validate(&filesystem).unwrap_err(),
            TransactionError::UnsupportedFileType { .. }
        ));
    }

    struct DeviceOverrideFilesystem {
        real: RealWorkspaceFilesystem,
        overrides: BTreeMap<PathBuf, u64>,
    }

    impl WorkspaceFilesystem for DeviceOverrideFilesystem {
        fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
            self.real.canonicalize(path)
        }
        fn metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata> {
            let mut metadata = self.real.metadata(path)?;
            if let Some(device) = self.overrides.get(path) {
                metadata.filesystem = *device;
            }
            Ok(metadata)
        }
        fn symlink_metadata(&self, path: &Path) -> io::Result<WorkspaceMetadata> {
            self.real.symlink_metadata(path)
        }
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.real.read(path)
        }
        fn read_directories(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
            self.real.read_directories(path)
        }
        fn create_dir_all(&self, path: &Path) -> io::Result<()> {
            self.real.create_dir_all(path)
        }
        fn write_new_durable(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
            self.real.write_new_durable(path, bytes, mode)
        }
        fn replace_durable(&self, source: &Path, target: &Path) -> io::Result<()> {
            self.real.replace_durable(source, target)
        }
        fn remove_file_durable(&self, path: &Path) -> io::Result<()> {
            self.real.remove_file_durable(path)
        }
        fn remove_dir_all_durable(&self, path: &Path) -> io::Result<()> {
            self.real.remove_dir_all_durable(path)
        }
        fn sync_directory(&self, path: &Path) -> io::Result<()> {
            self.real.sync_directory(path)
        }
        fn try_lock(&self, path: &Path) -> io::Result<Box<dyn WorkspaceLock>> {
            self.real.try_lock(path)
        }
    }

    #[test]
    fn injected_filesystem_identity_rejects_cross_filesystem_plan() {
        let (directory, plan) = fixture();
        let canonical_root = fs::canonicalize(directory.path()).unwrap();
        let root_device = RealWorkspaceFilesystem
            .metadata(&canonical_root)
            .unwrap()
            .filesystem;
        let filesystem = DeviceOverrideFilesystem {
            real: RealWorkspaceFilesystem,
            overrides: BTreeMap::from([(canonical_root.clone(), root_device + 1)]),
        };
        assert!(matches!(
            plan.validate(&filesystem).unwrap_err(),
            TransactionError::CrossFilesystem { .. }
        ));
    }

    #[test]
    fn all_prepare_and_backup_crashes_leave_original_tree_and_recoverable_journal() {
        let points = [
            FailurePoint::CreateJournal,
            FailurePoint::StageReplacement(0),
            FailurePoint::StageReplacement(1),
            FailurePoint::StageReplacement(2),
            FailurePoint::Backup(0),
            FailurePoint::Backup(1),
            FailurePoint::MarkPrepared,
        ];
        for point in points {
            let (directory, plan) = fixture();
            let filesystem = RealWorkspaceFilesystem;
            let transaction = plan.validate(&filesystem).unwrap();
            let failure = FailOnce::new(point.clone(), InjectedFailure::Crash);
            let error = WorkspaceTransactionManager::new(&filesystem, &failure)
                .commit(transaction)
                .unwrap_err();
            assert!(
                matches!(error, TransactionError::Interrupted { .. }),
                "{point:?}: {error}"
            );
            assert_original(directory.path());
            let actions = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
                .recover(directory.path())
                .unwrap();
            assert_eq!(actions.len(), 1, "{point:?}");
            assert_original(directory.path());
        }
    }

    #[test]
    fn crashes_at_every_replace_and_commit_record_are_rolled_back_on_restart() {
        let points = [
            FailurePoint::Replace(0),
            FailurePoint::RecordCommit(0),
            FailurePoint::Replace(1),
            FailurePoint::RecordCommit(1),
            FailurePoint::Replace(2),
            FailurePoint::RecordCommit(2),
            FailurePoint::MarkCommitted,
        ];
        for point in points {
            let (directory, plan) = fixture();
            let filesystem = RealWorkspaceFilesystem;
            let transaction = plan.validate(&filesystem).unwrap();
            let failure = FailOnce::new(point.clone(), InjectedFailure::Crash);
            WorkspaceTransactionManager::new(&filesystem, &failure)
                .commit(transaction)
                .unwrap_err();
            let actions = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
                .recover(directory.path())
                .unwrap();
            assert_eq!(actions.len(), 1, "{point:?}");
            assert_original(directory.path());
        }
    }

    #[test]
    fn commit_rechecks_expected_state_after_validation() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        fs::write(directory.path().join("deck.yaml"), b"external edit\n").unwrap();
        let error = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .commit(transaction)
            .unwrap_err();
        assert!(matches!(
            error,
            TransactionError::ExpectedStateMismatch { .. }
        ));
        assert_eq!(
            fs::read(directory.path().join("deck.yaml")).unwrap(),
            b"external edit\n"
        );
        assert_eq!(
            fs::read(directory.path().join("overlay.yaml")).unwrap(),
            b"old overlay\n"
        );
    }

    #[test]
    fn cooperative_workspace_lock_rejects_a_concurrent_writer() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        let control = directory.path().join(CONTROL_DIRECTORY);
        fs::create_dir(&control).unwrap();
        let _held_lock = filesystem.try_lock(&control.join(LOCK_FILE)).unwrap();
        let error = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .commit(transaction)
            .unwrap_err();
        assert!(matches!(error, TransactionError::WorkspaceBusy { .. }));
        assert_original(directory.path());
    }

    #[test]
    fn ordinary_replace_failure_rolls_back_before_returning() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        let failure = FailOnce::new(FailurePoint::Replace(1), InjectedFailure::Error);
        let error = WorkspaceTransactionManager::new(&filesystem, &failure)
            .commit(transaction)
            .unwrap_err();
        assert!(
            matches!(error, TransactionError::RolledBack { .. }),
            "{error}"
        );
        assert_original(directory.path());
    }

    #[test]
    fn rollback_failures_leave_an_explicit_journal_and_recovery_resumes_idempotently() {
        let points = [
            FailurePoint::Rollback(2),
            FailurePoint::RecordRollback(2),
            FailurePoint::Rollback(1),
            FailurePoint::RecordRollback(1),
            FailurePoint::Rollback(0),
            FailurePoint::RecordRollback(0),
            FailurePoint::MarkRolledBack,
        ];
        for rollback_point in points {
            let (directory, plan) = fixture();
            let filesystem = RealWorkspaceFilesystem;
            let transaction = plan.validate(&filesystem).unwrap();
            let failures = TwoFailures::new(
                (FailurePoint::Replace(2), InjectedFailure::Error),
                (rollback_point.clone(), InjectedFailure::Crash),
            );
            WorkspaceTransactionManager::new(&filesystem, &failures)
                .commit(transaction)
                .unwrap_err();
            WorkspaceTransactionManager::new(&filesystem, &NoFailures)
                .recover(directory.path())
                .unwrap();
            assert_original(directory.path());
        }
    }

    struct TwoFailures {
        failures: Mutex<Vec<(FailurePoint, InjectedFailure)>>,
    }

    impl TwoFailures {
        fn new(
            first: (FailurePoint, InjectedFailure),
            second: (FailurePoint, InjectedFailure),
        ) -> Self {
            Self {
                failures: Mutex::new(vec![first, second]),
            }
        }
    }

    impl FailureInjector for TwoFailures {
        fn check(&self, point: &FailurePoint) -> Option<InjectedFailure> {
            let mut failures = self.failures.lock().unwrap();
            if failures
                .first()
                .is_some_and(|(expected, _)| expected == point)
            {
                Some(failures.remove(0).1)
            } else {
                None
            }
        }
    }

    #[test]
    fn committed_journal_is_deterministically_finalized_after_cleanup_crash() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        let failure = FailOnce::new(FailurePoint::FinalizeCleanup, InjectedFailure::Crash);
        WorkspaceTransactionManager::new(&filesystem, &failure)
            .commit(transaction)
            .unwrap_err();
        assert_replaced(directory.path());
        let actions = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .recover(directory.path())
            .unwrap();
        assert!(matches!(
            actions.as_slice(),
            [RecoveryAction::FinalizedCommit { .. }]
        ));
        assert_replaced(directory.path());
    }

    #[test]
    fn committed_recovery_refuses_to_accept_unrecognized_mixed_content() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        let failure = FailOnce::new(FailurePoint::FinalizeCleanup, InjectedFailure::Crash);
        WorkspaceTransactionManager::new(&filesystem, &failure)
            .commit(transaction)
            .unwrap_err();
        fs::write(directory.path().join("overlay.yaml"), b"third party\n").unwrap();
        let error = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .recover(directory.path())
            .unwrap_err();
        assert!(matches!(error, TransactionError::RecoveryConflict { .. }));
        assert_eq!(
            fs::read(directory.path().join("overlay.yaml")).unwrap(),
            b"third party\n"
        );
    }

    #[test]
    fn recovery_refuses_to_overwrite_unrecognized_concurrent_content() {
        let (directory, plan) = fixture();
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        let failure = FailOnce::new(FailurePoint::RecordCommit(0), InjectedFailure::Crash);
        WorkspaceTransactionManager::new(&filesystem, &failure)
            .commit(transaction)
            .unwrap_err();
        fs::write(directory.path().join("deck.yaml"), b"third party\n").unwrap();
        let error = WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .recover(directory.path())
            .unwrap_err();
        assert!(matches!(error, TransactionError::RecoveryConflict { .. }));
        assert_eq!(
            fs::read(directory.path().join("deck.yaml")).unwrap(),
            b"third party\n"
        );
        assert!(directory.path().join(CONTROL_DIRECTORY).exists());
    }

    #[test]
    fn existing_permissions_are_preserved_and_new_mode_is_explicit() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("script.sh");
        fs::write(&existing, b"old\n").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o751)).unwrap();
        let new_replacement = replacement("new file\n").with_new_file_mode(0o640).unwrap();
        let plan = WorkspaceTransactionPlan::new(
            directory.path(),
            vec![
                WorkspaceWrite::new("script.sh", expected(&existing), replacement("new\n")),
                WorkspaceWrite::new("new.yaml", ExpectedTarget::Absent, new_replacement),
            ],
        );
        let filesystem = RealWorkspaceFilesystem;
        let transaction = plan.validate(&filesystem).unwrap();
        WorkspaceTransactionManager::new(&filesystem, &NoFailures)
            .commit(transaction)
            .unwrap();
        assert_eq!(
            fs::metadata(existing).unwrap().permissions().mode() & 0o7777,
            0o751
        );
        assert_eq!(
            fs::metadata(directory.path().join("new.yaml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o640
        );
    }
}
