//! Shared plan-first adapter for recoverable source-file replacements.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::workspace_transaction::{
    ExpectedTarget, FailureInjector, FailurePoint, FileFingerprint, InjectedFailure, NoFailures,
    RealWorkspaceFilesystem, ValidatedReplacement, WorkspaceTransactionManager,
    WorkspaceTransactionPlan, WorkspaceWrite,
};

/// One replacement whose original bytes were observed while the complete mutation was planned.
pub(crate) struct PlannedWorkspaceFile {
    path: PathBuf,
    expected: ExpectedTarget,
    replacement: ValidatedReplacement,
}

impl PlannedWorkspaceFile {
    pub(crate) fn validated(
        path: impl Into<PathBuf>,
        original: Vec<u8>,
        replacement: Vec<u8>,
        validator: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<Self, String> {
        Self::validated_with_expected(
            path,
            ExpectedTarget::Present(FileFingerprint::for_bytes(&original)),
            replacement,
            validator,
        )
    }

    pub(crate) fn validated_new(
        path: impl Into<PathBuf>,
        replacement: Vec<u8>,
        validator: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<Self, String> {
        Self::validated_with_expected(path, ExpectedTarget::Absent, replacement, validator)
    }

    fn validated_with_expected(
        path: impl Into<PathBuf>,
        expected: ExpectedTarget,
        replacement: Vec<u8>,
        validator: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<Self, String> {
        let replacement = ValidatedReplacement::validate(replacement, validator)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            path: path.into(),
            expected,
            replacement,
        })
    }
}

/// Recover interrupted writes before a mutator reads or plans against the workspace.
pub(crate) fn recover_workspace(workspace_root: &Path) -> Result<(), String> {
    WorkspaceTransactionManager::new(&RealWorkspaceFilesystem, &NoFailures)
        .recover(workspace_root)
        .map(|_| ())
        .map_err(|error| format!("workspace transaction recovery failed: {error}"))
}

/// Validate the complete replacement set, then commit it through the durable journal.
pub(crate) fn commit_workspace_files(
    workspace_root: &Path,
    files: Vec<PlannedWorkspaceFile>,
) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }

    let root = std::fs::canonicalize(workspace_root)
        .map_err(|error| format!("{}: {error}", workspace_root.display()))?;
    let created_directories = create_missing_target_parents(&root, &files)?;
    let result = (|| {
        let writes = files
            .into_iter()
            .map(|file| {
                let absolute = match &file.expected {
                    ExpectedTarget::Present(_) => std::fs::canonicalize(&file.path)
                        .map_err(|error| format!("{}: {error}", file.path.display()))?,
                    ExpectedTarget::Absent => {
                        let parent = file.path.parent().ok_or_else(|| {
                            format!("{} has no parent directory", file.path.display())
                        })?;
                        let parent = std::fs::canonicalize(parent)
                            .map_err(|error| format!("{}: {error}", parent.display()))?;
                        let name = file
                            .path
                            .file_name()
                            .ok_or_else(|| format!("{} has no file name", file.path.display()))?;
                        parent.join(name)
                    }
                };
                let target = absolute.strip_prefix(&root).map_err(|_| {
                    format!(
                        "workspace transaction target {} is outside {}",
                        absolute.display(),
                        root.display()
                    )
                })?;
                Ok(WorkspaceWrite::new(target, file.expected, file.replacement))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let transaction = WorkspaceTransactionPlan::new(&root, writes)
            .validate(&RealWorkspaceFilesystem)
            .map_err(|error| format!("workspace transaction plan is invalid: {error}"))?;
        transaction_trace("transaction_begin");
        let result =
            WorkspaceTransactionManager::new(&RealWorkspaceFilesystem, &EnvironmentFailures)
                .commit(transaction)
                .map_err(|error| format!("workspace transaction commit failed: {error}"));
        transaction_trace("transaction_end");
        result
    })();
    if result.is_err() {
        remove_empty_directories(&created_directories);
    }
    result
}

fn create_missing_target_parents(
    root: &Path,
    files: &[PlannedWorkspaceFile],
) -> Result<Vec<PathBuf>, String> {
    let mut required = Vec::new();
    for file in files {
        if !matches!(file.expected, ExpectedTarget::Absent) {
            continue;
        }
        let parent = file
            .path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", file.path.display()))?;
        let absolute = if parent.is_absolute() {
            parent.to_path_buf()
        } else {
            root.join(parent)
        };
        if !absolute.starts_with(root) {
            return Err(format!(
                "workspace transaction target parent {} is outside {}",
                absolute.display(),
                root.display()
            ));
        }
        let mut missing = Vec::new();
        let mut cursor = absolute.as_path();
        while !cursor.exists() {
            missing.push(cursor.to_path_buf());
            cursor = cursor
                .parent()
                .ok_or_else(|| format!("{} has no existing ancestor", absolute.display()))?;
        }
        let ancestor = std::fs::canonicalize(cursor)
            .map_err(|error| format!("{}: {error}", cursor.display()))?;
        if !ancestor.starts_with(root) {
            return Err(format!(
                "workspace transaction target parent {} escapes {} through {}",
                absolute.display(),
                root.display(),
                ancestor.display()
            ));
        }
        missing.reverse();
        required.extend(missing);
    }
    required.sort();
    required.dedup();
    let mut created = Vec::new();
    for directory in required {
        if directory.exists() {
            continue;
        }
        if let Err(error) = std::fs::create_dir(&directory) {
            remove_empty_directories(&created);
            return Err(format!("{}: {error}", directory.display()));
        }
        created.push(directory);
    }
    Ok(created)
}

#[derive(Clone, Copy, Debug)]
struct EnvironmentFailures;

impl FailureInjector for EnvironmentFailures {
    fn check(&self, point: &FailurePoint) -> Option<InjectedFailure> {
        if let FailurePoint::Replace(_) = point
            && let Ok(value) = std::env::var("BRAINBREW_TRANSACTION_SLEEP_BEFORE_REPLACE_MS")
            && let Ok(milliseconds) = value.parse::<u64>()
            && milliseconds > 0
        {
            std::thread::sleep(std::time::Duration::from_millis(milliseconds));
        }
        let requested = std::env::var("BRAINBREW_TRANSACTION_FAIL_POINT").ok()?;
        let actual = failure_point_name(point);
        if requested != actual {
            return None;
        }
        Some(
            if std::env::var("BRAINBREW_TRANSACTION_FAIL_MODE").as_deref() == Ok("crash") {
                InjectedFailure::Crash
            } else {
                InjectedFailure::Error
            },
        )
    }
}

fn failure_point_name(point: &FailurePoint) -> String {
    match point {
        FailurePoint::CreateJournal => "create-journal".to_owned(),
        FailurePoint::StageReplacement(index) => format!("stage:{index}"),
        FailurePoint::Backup(index) => format!("backup:{index}"),
        FailurePoint::MarkPrepared => "mark-prepared".to_owned(),
        FailurePoint::Replace(index) => format!("replace:{index}"),
        FailurePoint::RecordCommit(index) => format!("record-commit:{index}"),
        FailurePoint::MarkCommitted => "mark-committed".to_owned(),
        FailurePoint::FinalizeCleanup => "finalize".to_owned(),
        FailurePoint::Rollback(index) => format!("rollback:{index}"),
        FailurePoint::PublishRollbackRestore(index) => format!("rollback-restore:{index}"),
        FailurePoint::RecordRollback(index) => format!("record-rollback:{index}"),
        FailurePoint::MarkRolledBack => "mark-rolled-back".to_owned(),
    }
}

fn transaction_trace(event: &str) {
    let Ok(path) = std::env::var("BRAINBREW_TRANSACTION_TRACE") else {
        return;
    };
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| writeln!(file, "{event}"));
}

fn remove_empty_directories(directories: &[PathBuf]) {
    for directory in directories.iter().rev() {
        let _ = std::fs::remove_dir(directory);
    }
}
