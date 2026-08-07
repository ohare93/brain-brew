//! Clean, recoverable publication of generated output directory trees.
//!
//! A complete tree is staged beside its destination. Existing destinations are
//! moved to a backup only after staging succeeds; ordinary publication failures
//! restore that backup. A small sibling journal makes interruption state explicit
//! and recoverable on the next attempt.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use brain_brew_formats::safe_relative_path::SafeRelativePath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::workspace_mutation::nearest_existing_ancestor;

#[derive(Debug)]
pub(crate) struct OutputArtifact {
    pub(crate) path: PathBuf,
    pub(crate) bytes: Vec<u8>,
}

impl OutputArtifact {
    pub(crate) fn new(path: impl Into<PathBuf>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum PublishState {
    Prepared,
    Published,
}

#[derive(Debug, Deserialize, Serialize)]
struct PublishJournal {
    version: u32,
    target_name: String,
    stage_name: String,
    backup_name: String,
    original_fingerprint: Option<String>,
    replacement_fingerprint: String,
    state: PublishState,
}

pub(crate) fn publish_output_tree(
    output: &Path,
    artifacts: Vec<OutputArtifact>,
    force: bool,
) -> Result<(), String> {
    if artifacts.is_empty() {
        return Err("refusing to publish an empty output tree".to_owned());
    }
    let requested = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("cannot resolve current directory: {error}"))?
            .join(output)
    };
    let requested_parent = requested
        .parent()
        .ok_or_else(|| format!("output {} has no parent directory", output.display()))?;
    let root = nearest_existing_ancestor(requested_parent)?;
    let created_parents = create_missing_parents(&root, requested_parent)?;
    let result = publish_in_existing_parent(&requested, artifacts, force);
    if result.is_err() {
        remove_empty_parents(&created_parents);
    }
    result
}

fn publish_in_existing_parent(
    requested: &Path,
    artifacts: Vec<OutputArtifact>,
    force: bool,
) -> Result<(), String> {
    let parent = fs::canonicalize(
        requested
            .parent()
            .ok_or_else(|| format!("{} has no parent", requested.display()))?,
    )
    .map_err(|error| format!("{}: {error}", requested.display()))?;
    let target_name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("output {} must have a UTF-8 file name", requested.display()))?;
    let target = parent.join(target_name);
    let journal_path = parent.join(format!(".brainbrew-{target_name}.publish.json"));
    recover_output_journal(&parent, &journal_path)?;

    let original_fingerprint = match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            if !force {
                return Err(format!(
                    "refusing to overwrite existing generated output {}; pass --force to cleanly replace it",
                    target.display()
                ));
            }
            Some(tree_fingerprint(&target)?)
        }
        Ok(metadata) => {
            return Err(format!(
                "refusing to replace generated output {} with unsupported type {:?}",
                target.display(),
                metadata.file_type()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("{}: {error}", target.display())),
    };

    let nonce = nonce();
    let stage_name = format!(".brainbrew-{target_name}-{nonce}.stage");
    let backup_name = format!(".brainbrew-{target_name}-{nonce}.backup");
    let stage = parent.join(&stage_name);
    let backup = parent.join(&backup_name);
    fs::create_dir(&stage).map_err(|error| format!("{}: {error}", stage.display()))?;
    let stage_result = stage_artifacts(&stage, artifacts);
    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let replacement_fingerprint = match tree_fingerprint(&stage) {
        Ok(fingerprint) => fingerprint,
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    let mut journal = PublishJournal {
        version: 1,
        target_name: target_name.to_owned(),
        stage_name,
        backup_name,
        original_fingerprint,
        replacement_fingerprint,
        state: PublishState::Prepared,
    };
    if let Err(error) = write_journal(&journal_path, &journal) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }

    let publish_result: Result<(), String> = (|| {
        if journal.original_fingerprint.is_some() {
            fs::rename(&target, &backup).map_err(|error| {
                format!(
                    "could not move existing generated output {} to recovery backup {}: {error}",
                    target.display(),
                    backup.display()
                )
            })?;
            sync_directory(&parent)?;
        }
        injected_failure("before-publish")?;
        fs::rename(&stage, &target).map_err(|error| {
            format!(
                "could not publish staged output {} as {}: {error}",
                stage.display(),
                target.display()
            )
        })?;
        sync_directory(&parent)?;
        journal.state = PublishState::Published;
        write_journal(&journal_path, &journal)?;
        injected_failure("after-publish")?;
        Ok(())
    })();

    if let Err(error) = publish_result {
        if std::env::var("BRAINBREW_OUTPUT_FAIL_MODE").as_deref() == Ok("crash") {
            return Err(format!(
                "output publication interrupted; recoverable state is recorded at {}: {error}",
                journal_path.display()
            ));
        }
        rollback_publication(&parent, &journal)?;
        remove_journal(&journal_path)?;
        return Err(format!(
            "output publication failed and original output was restored: {error}"
        ));
    }

    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(|error| format!("{}: {error}", backup.display()))?;
        sync_directory(&parent)?;
    }
    remove_journal(&journal_path)
}

fn stage_artifacts(stage: &Path, artifacts: Vec<OutputArtifact>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for artifact in artifacts {
        let raw = artifact.path.to_str().ok_or_else(|| {
            format!(
                "generated artifact path {:?} is not valid UTF-8",
                artifact.path
            )
        })?;
        let safe = SafeRelativePath::new(raw)
            .map_err(|error| format!("invalid generated artifact path {raw:?}: {error}"))?;
        if !seen.insert(safe.as_str().to_owned()) {
            return Err(format!("duplicate generated artifact path {raw:?}"));
        }
        let destination = stage.join(safe.as_path());
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|error| format!("{}: {error}", destination.display()))?;
        file.write_all(&artifact.bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("{}: {error}", destination.display()))?;
    }
    sync_tree(stage)
}

fn recover_output_journal(parent: &Path, journal_path: &Path) -> Result<(), String> {
    let bytes = match fs::read(journal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("{}: {error}", journal_path.display())),
    };
    let journal: PublishJournal = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{}: invalid output recovery journal: {error}",
            journal_path.display()
        )
    })?;
    if journal.version != 1
        || !is_single_name(&journal.target_name)
        || !is_single_name(&journal.stage_name)
        || !is_single_name(&journal.backup_name)
    {
        return Err(format!(
            "{}: unsafe output recovery journal",
            journal_path.display()
        ));
    }
    match journal.state {
        PublishState::Prepared => rollback_publication(parent, &journal)?,
        PublishState::Published => {
            let target = parent.join(&journal.target_name);
            let actual = optional_tree_fingerprint(&target)?;
            if actual.as_deref() != Some(&journal.replacement_fingerprint) {
                return Err(recovery_conflict(
                    &target,
                    &journal.replacement_fingerprint,
                    actual,
                ));
            }
            let stage = parent.join(&journal.stage_name);
            let backup = parent.join(&journal.backup_name);
            if stage.exists() {
                fs::remove_dir_all(&stage)
                    .map_err(|error| format!("{}: {error}", stage.display()))?;
            }
            if backup.exists() {
                fs::remove_dir_all(&backup)
                    .map_err(|error| format!("{}: {error}", backup.display()))?;
            }
        }
    }
    remove_journal(journal_path)
}

fn rollback_publication(parent: &Path, journal: &PublishJournal) -> Result<(), String> {
    let target = parent.join(&journal.target_name);
    let stage = parent.join(&journal.stage_name);
    let backup = parent.join(&journal.backup_name);
    let current = optional_tree_fingerprint(&target)?;
    if backup.exists() {
        if current.is_some()
            && current.as_deref() != Some(&journal.replacement_fingerprint)
            && current.as_deref() != journal.original_fingerprint.as_deref()
        {
            return Err(recovery_conflict(
                &target,
                &journal.replacement_fingerprint,
                current,
            ));
        }
        if target.exists() {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("{}: {error}", target.display()))?;
        }
        fs::rename(&backup, &target)
            .map_err(|error| format!("{} -> {}: {error}", backup.display(), target.display()))?;
    } else if journal.original_fingerprint.is_none() {
        if let Some(actual) = current {
            if actual != journal.replacement_fingerprint {
                return Err(recovery_conflict(
                    &target,
                    &journal.replacement_fingerprint,
                    Some(actual),
                ));
            }
            fs::remove_dir_all(&target)
                .map_err(|error| format!("{}: {error}", target.display()))?;
        }
    } else if current.as_deref() != journal.original_fingerprint.as_deref() {
        return Err(recovery_conflict(
            &target,
            journal
                .original_fingerprint
                .as_deref()
                .expect("original fingerprint exists"),
            current,
        ));
    }
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|error| format!("{}: {error}", stage.display()))?;
    }
    sync_directory(parent)
}

fn write_journal(path: &Path, journal: &PublishJournal) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.next");
    if temp.exists() {
        fs::remove_file(&temp).map_err(|error| format!("{}: {error}", temp.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| format!("{}: {error}", temp.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("{}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| format!("{}: {error}", path.display()))?;
    sync_directory(
        path.parent()
            .ok_or_else(|| format!("{} has no parent", path.display()))?,
    )
}

fn remove_journal(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => sync_directory(
            path.parent()
                .ok_or_else(|| format!("{} has no parent", path.display()))?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn create_missing_parents(root: &Path, requested: &Path) -> Result<Vec<PathBuf>, String> {
    let mut missing = Vec::new();
    let mut cursor = requested;
    while !cursor.exists() {
        missing.push(cursor.to_path_buf());
        cursor = cursor
            .parent()
            .ok_or_else(|| format!("{} has no existing ancestor", requested.display()))?;
    }
    let ancestor =
        fs::canonicalize(cursor).map_err(|error| format!("{}: {error}", cursor.display()))?;
    if !ancestor.starts_with(root) {
        return Err(format!(
            "output parent {} escapes selected output root {} through {}",
            requested.display(),
            root.display(),
            ancestor.display()
        ));
    }
    missing.reverse();
    let mut created = Vec::new();
    for path in missing {
        if let Err(error) = fs::create_dir(&path) {
            remove_empty_parents(&created);
            return Err(format!("{}: {error}", path.display()));
        }
        created.push(path);
    }
    Ok(created)
}

fn remove_empty_parents(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = fs::remove_dir(path);
    }
}

fn sync_tree(path: &Path) -> Result<(), String> {
    let mut directories = vec![path.to_path_buf()];
    for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("{}: {error}", path.display()))?;
        if entry
            .file_type()
            .map_err(|error| format!("{}: {error}", entry.path().display()))?
            .is_dir()
        {
            sync_tree(&entry.path())?;
            directories.push(entry.path());
        }
    }
    for directory in directories.into_iter().rev() {
        sync_directory(&directory)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn optional_tree_fingerprint(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => tree_fingerprint(path).map(Some),
        Ok(metadata) => Err(format!(
            "output recovery path {} has unsupported type {:?}",
            path.display(),
            metadata.file_type()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn tree_fingerprint(root: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hash_directory(root, Path::new(""), &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_directory(root: &Path, relative: &Path, hasher: &mut Sha256) -> Result<(), String> {
    let directory = root.join(relative);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().into_string().map_err(|_| {
            format!(
                "generated output contains a non-UTF-8 entry under {}",
                directory.display()
            )
        })?;
        let child = relative.join(name);
        let child_text = child
            .to_str()
            .ok_or_else(|| format!("generated output path {:?} is not UTF-8", child))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("{}: {error}", entry.path().display()))?;
        if metadata.file_type().is_dir() {
            hasher.update(b"directory\0");
            hasher.update(child_text.as_bytes());
            hasher.update(b"\0");
            hash_directory(root, &child, hasher)?;
        } else if metadata.file_type().is_file() {
            let bytes = fs::read(entry.path())
                .map_err(|error| format!("{}: {error}", entry.path().display()))?;
            hasher.update(b"file\0");
            hasher.update(child_text.as_bytes());
            hasher.update(b"\0");
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(&bytes);
        } else {
            return Err(format!(
                "refusing generated output tree {}: entry {} has unsupported type {:?}",
                root.display(),
                entry.path().display(),
                metadata.file_type()
            ));
        }
    }
    Ok(())
}

fn recovery_conflict(path: &Path, expected: &str, actual: Option<String>) -> String {
    format!(
        "output recovery conflict at {}: expected tree fingerprint {}, found {} (backup and journal were retained)",
        path.display(),
        expected,
        actual.as_deref().unwrap_or("absent")
    )
}

fn injected_failure(point: &str) -> Result<(), String> {
    if std::env::var("BRAINBREW_OUTPUT_FAIL_POINT").as_deref() == Ok(point) {
        Err(format!("injected output failure at {point}"))
    } else {
        Ok(())
    }
}

fn is_single_name(value: &str) -> bool {
    Path::new(value).file_name().and_then(|name| name.to_str()) == Some(value)
}

fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}
