//! Shared plan-first adapter for recoverable source-file replacements.

use std::path::{Path, PathBuf};

use crate::workspace_transaction::{
    ExpectedTarget, FileFingerprint, NoFailures, RealWorkspaceFilesystem, ValidatedReplacement,
    WorkspaceTransactionManager, WorkspaceTransactionPlan, WorkspaceWrite,
};

/// One replacement whose original bytes were observed while the complete mutation was planned.
pub(crate) struct PlannedWorkspaceFile {
    path: PathBuf,
    original: Vec<u8>,
    replacement: ValidatedReplacement,
}

impl PlannedWorkspaceFile {
    pub(crate) fn validated(
        path: impl Into<PathBuf>,
        original: Vec<u8>,
        replacement: Vec<u8>,
        validator: impl FnOnce(&[u8]) -> Result<(), String>,
    ) -> Result<Self, String> {
        let replacement = ValidatedReplacement::validate(replacement, validator)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            path: path.into(),
            original,
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
    let writes = files
        .into_iter()
        .map(|file| {
            let absolute = std::fs::canonicalize(&file.path)
                .map_err(|error| format!("{}: {error}", file.path.display()))?;
            let target = absolute.strip_prefix(&root).map_err(|_| {
                format!(
                    "workspace transaction target {} is outside {}",
                    absolute.display(),
                    root.display()
                )
            })?;
            Ok(WorkspaceWrite::new(
                target,
                ExpectedTarget::Present(FileFingerprint::for_bytes(&file.original)),
                file.replacement,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let transaction = WorkspaceTransactionPlan::new(&root, writes)
        .validate(&RealWorkspaceFilesystem)
        .map_err(|error| format!("workspace transaction plan is invalid: {error}"))?;
    WorkspaceTransactionManager::new(&RealWorkspaceFilesystem, &NoFailures)
        .commit(transaction)
        .map_err(|error| format!("workspace transaction commit failed: {error}"))
}
