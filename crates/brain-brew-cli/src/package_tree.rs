//! One fail-closed policy for fetched package trees and archive extraction.
//!
//! Package snapshots and caches contain directories and regular files only.
//! Symlinks (including apparently contained links), hard links, and every
//! special filesystem/archive entry are rejected.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};

use brain_brew_formats::safe_relative_path::SafeRelativePath;
use tar::{Archive, EntryType};

pub(crate) fn copy_filtered(source: &Path, destination: &Path) -> Result<(), String> {
    let (canonical_source, source_metadata) = establish_root(source, "source package tree")?;
    create_private_directory(destination)?;
    if let Err(error) = copy_directory(
        source,
        &canonical_source,
        &source_metadata,
        destination,
        true,
    ) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    validate(destination, "staged package tree")
}

pub(crate) fn copy_complete(source: &Path, destination: &Path) -> Result<(), String> {
    validate(source, "staged package tree")?;
    let (canonical_source, source_metadata) = establish_root(source, "staged package tree")?;
    create_private_directory(destination)?;
    if let Err(error) = copy_directory(
        source,
        &canonical_source,
        &source_metadata,
        destination,
        false,
    ) {
        let _ = fs::remove_dir_all(destination);
        return Err(error);
    }
    validate(destination, "publication package tree")
}

pub(crate) fn validate(root: &Path, tree_name: &str) -> Result<(), String> {
    let (canonical_root, root_metadata) = establish_root(root, tree_name)?;
    validate_directory(root, &canonical_root, root, &root_metadata, tree_name)
}

pub(crate) fn extract_tar<R: Read>(mut reader: R, destination: &Path) -> Result<(), String> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read tar archive: {error}"))?;
    preflight_raw_archive(&bytes)?;

    create_private_directory(destination)?;
    let result = extract_preflighted_archive(&bytes, destination)
        .and_then(|()| validate(destination, "extracted package tree"));
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}

fn preflight_raw_archive(bytes: &[u8]) -> Result<(), String> {
    let mut archive = Archive::new(Cursor::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| format!("failed to inspect tar archive: {error}"))?
        .raw(true);
    for (index, entry) in entries.enumerate() {
        let mut entry = entry
            .map_err(|error| format!("failed to inspect raw tar entry {}: {error}", index + 1))?;
        let entry_type = entry.header().entry_type();
        match entry_type {
            EntryType::Regular | EntryType::Directory => {
                validate_archive_path(
                    entry.header().path_bytes().as_ref(),
                    entry_type == EntryType::Directory,
                    index + 1,
                )?;
            }
            EntryType::GNULongName => {
                let mut path = Vec::new();
                entry.read_to_end(&mut path).map_err(|error| {
                    format!(
                        "failed to inspect GNU long-name tar entry {}: {error}",
                        index + 1
                    )
                })?;
                while matches!(path.last(), Some(0 | b'\n')) {
                    path.pop();
                }
                validate_archive_path(&path, true, index + 1)?;
            }
            EntryType::XHeader => validate_pax_path_metadata(&mut entry, index + 1)?,
            EntryType::Link => return Err(rejected_archive_type(index + 1, "hard link")),
            EntryType::Symlink => return Err(rejected_archive_type(index + 1, "symlink")),
            EntryType::Char | EntryType::Block => {
                return Err(rejected_archive_type(index + 1, "device"));
            }
            EntryType::Fifo => return Err(rejected_archive_type(index + 1, "fifo")),
            EntryType::GNUSparse => return Err(rejected_archive_type(index + 1, "sparse")),
            EntryType::GNULongLink => {
                return Err(rejected_archive_type(index + 1, "long link"));
            }
            EntryType::Continuous | EntryType::XGlobalHeader | EntryType::__Nonexhaustive(_) => {
                return Err(rejected_archive_type(index + 1, "unknown"));
            }
        }
    }
    Ok(())
}

fn validate_pax_path_metadata<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    index: usize,
) -> Result<(), String> {
    let extensions = entry
        .pax_extensions()
        .map_err(|error| format!("failed to inspect PAX tar entry {index}: {error}"))?
        .ok_or_else(|| {
            format!("archive package tree policy rejected malformed PAX entry {index}")
        })?;
    for extension in extensions {
        let extension = extension
            .map_err(|error| format!("failed to inspect PAX tar entry {index}: {error}"))?;
        match extension.key_bytes() {
            b"path" => validate_archive_path(extension.value_bytes(), true, index)?,
            key if key.starts_with(b"GNU.sparse.") => {
                return Err(rejected_archive_type(index, "sparse metadata"));
            }
            b"linkpath" => {
                return Err(format!(
                    "archive package tree policy rejected link metadata in PAX entry {index}"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn extract_preflighted_archive(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let mut archive = Archive::new(Cursor::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| format!("failed to inspect tar archive: {error}"))?;
    let mut targets = BTreeMap::<PathBuf, EntryType>::new();

    for (index, entry) in entries.enumerate() {
        let mut entry =
            entry.map_err(|error| format!("failed to inspect tar entry {}: {error}", index + 1))?;
        let entry_type = entry.header().entry_type();
        if !matches!(entry_type, EntryType::Regular | EntryType::Directory) {
            return Err(rejected_archive_type(
                index + 1,
                archive_type_name(entry_type),
            ));
        }
        let relative = validated_archive_relative(
            entry.path_bytes().as_ref(),
            entry_type == EntryType::Directory,
            index + 1,
        )?;
        reject_target_collision(&targets, relative.as_path(), entry_type, index + 1)?;
        targets.insert(relative.clone(), entry_type);

        let target = destination.join(&relative);
        ensure_archive_parents(destination, relative.as_path(), index + 1)?;
        if entry_type == EntryType::Directory {
            match fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
                Ok(_) => {
                    return Err(format!(
                        "archive package tree policy rejected colliding directory target {:?} in entry {}",
                        relative.display(),
                        index + 1
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&target).map_err(|error| {
                        format!("failed to create {}: {error}", target.display())
                    })?;
                    normalize_directory_permissions(&target)?;
                }
                Err(error) => return Err(format!("{}: {error}", target.display())),
            }
        } else {
            let mut output = create_new_file(&target).map_err(|error| {
                format!(
                    "archive package tree policy could not create-new/no-follow target {}: {error}",
                    target.display()
                )
            })?;
            io::copy(&mut entry, &mut output)
                .map_err(|error| format!("failed to extract {}: {error}", target.display()))?;
            output
                .sync_all()
                .map_err(|error| format!("failed to sync {}: {error}", target.display()))?;
            normalize_file_permissions(&target, entry.header().mode().unwrap_or(0) & 0o111 != 0)?;
        }
    }
    Ok(())
}

fn reject_target_collision(
    targets: &BTreeMap<PathBuf, EntryType>,
    relative: &Path,
    entry_type: EntryType,
    index: usize,
) -> Result<(), String> {
    if targets.contains_key(relative) {
        return Err(format!(
            "archive package tree policy rejected duplicate normalized target {:?} in entry {index}",
            relative.display()
        ));
    }
    for ancestor in relative.ancestors().skip(1) {
        if targets
            .get(ancestor)
            .is_some_and(|kind| *kind == EntryType::Regular)
        {
            return Err(format!(
                "archive package tree policy rejected target {:?} below file {:?} in entry {index}",
                relative.display(),
                ancestor.display()
            ));
        }
    }
    if entry_type == EntryType::Regular && targets.keys().any(|target| target.starts_with(relative))
    {
        return Err(format!(
            "archive package tree policy rejected file target {:?} colliding with a child target in entry {index}",
            relative.display()
        ));
    }
    Ok(())
}

fn validate_archive_path(raw: &[u8], directory: bool, index: usize) -> Result<(), String> {
    validated_archive_relative(raw, directory, index).map(|_| ())
}

fn validated_archive_relative(
    raw: &[u8],
    directory: bool,
    index: usize,
) -> Result<PathBuf, String> {
    let raw = std::str::from_utf8(raw).map_err(|_| {
        format!("archive package tree policy rejected non-UTF-8 archive path in entry {index}")
    })?;
    let normalized = if directory {
        raw.strip_suffix('/').unwrap_or(raw)
    } else {
        raw
    };
    let safe = SafeRelativePath::new(normalized).map_err(|error| {
        format!(
            "archive package tree policy rejected archive path {raw:?} in entry {index}: {error}"
        )
    })?;
    Ok(safe.as_path().to_path_buf())
}

fn rejected_archive_type(index: usize, kind: &str) -> String {
    format!("archive package tree policy rejected {kind} entry {index}")
}

fn archive_type_name(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::Link => "hard link",
        EntryType::Symlink => "symlink",
        EntryType::Char | EntryType::Block => "device",
        EntryType::Fifo => "fifo",
        EntryType::GNUSparse => "sparse",
        _ => "unknown",
    }
}

fn ensure_archive_parents(root: &Path, relative: &Path, index: usize) -> Result<(), String> {
    let parent = relative
        .parent()
        .ok_or_else(|| format!("archive entry {index} has no authorized parent"))?;
    let mut current = root.to_path_buf();
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(format!(
                    "archive package tree policy rejected replaced/non-directory parent {} in entry {index}",
                    current.display()
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|error| format!("failed to create {}: {error}", current.display()))?;
                normalize_directory_permissions(&current)?;
            }
            Err(error) => return Err(format!("{}: {error}", current.display())),
        }
    }
    Ok(())
}

fn establish_root(root: &Path, tree_name: &str) -> Result<(PathBuf, fs::Metadata), String> {
    let before = fs::symlink_metadata(root)
        .map_err(|error| format!("{tree_name} {}: {error}", root.display()))?;
    let kind = classify(&before);
    if kind != "directory" {
        return Err(format!(
            "{tree_name} policy rejected root {}: expected directory, found {kind}",
            root.display()
        ));
    }
    let canonical = fs::canonicalize(root)
        .map_err(|error| format!("{tree_name} root {}: {error}", root.display()))?;
    let after = fs::symlink_metadata(root)
        .map_err(|error| format!("{tree_name} root {} was replaced: {error}", root.display()))?;
    if !after.is_dir() || !same_file(&before, &after) {
        return Err(format!(
            "{tree_name} policy rejected root {}: directory was replaced while authorizing it",
            root.display()
        ));
    }
    Ok((canonical, after))
}

fn validate_directory(
    root: &Path,
    canonical_root: &Path,
    directory: &Path,
    expected: &fs::Metadata,
    tree_name: &str,
) -> Result<(), String> {
    authorize_existing_directory(directory, canonical_root, expected, tree_name)?;
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let kind = classify(&metadata);
        if kind == "directory" {
            validate_directory(root, canonical_root, &path, &metadata, tree_name)?;
        } else if kind == "regular file" {
            reject_hard_link(&path, &metadata, tree_name)?;
            authorize_existing_file(&path, canonical_root, &metadata, tree_name)?;
        } else {
            return Err(tree_rejection(root, &path, tree_name, kind));
        }
    }
    Ok(())
}

fn authorize_existing_directory(
    directory: &Path,
    canonical_root: &Path,
    expected: &fs::Metadata,
    tree_name: &str,
) -> Result<(), String> {
    let before = fs::symlink_metadata(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    if !before.is_dir() || !same_file(expected, &before) {
        return Err(format!(
            "{tree_name} policy rejected {}: directory was replaced during traversal",
            directory.display()
        ));
    }
    let canonical =
        fs::canonicalize(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    if !canonical.starts_with(canonical_root) {
        return Err(format!(
            "{tree_name} policy rejected {}: replaced directory resolves outside authorized root {}",
            directory.display(),
            canonical_root.display()
        ));
    }
    let after = fs::symlink_metadata(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    if !after.is_dir() || !same_file(expected, &after) {
        return Err(format!(
            "{tree_name} policy rejected {}: directory was replaced while authorizing traversal",
            directory.display()
        ));
    }
    Ok(())
}

fn authorize_existing_file(
    path: &Path,
    canonical_root: &Path,
    expected: &fs::Metadata,
    tree_name: &str,
) -> Result<(), String> {
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !canonical.starts_with(canonical_root) {
        return Err(format!(
            "{tree_name} policy rejected {}: replaced file resolves outside authorized root {}",
            path.display(),
            canonical_root.display()
        ));
    }
    let after =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !after.is_file() || !same_file(expected, &after) {
        return Err(format!(
            "{tree_name} policy rejected {}: file was replaced while authorizing it",
            path.display()
        ));
    }
    Ok(())
}

fn copy_directory(
    source: &Path,
    canonical_root: &Path,
    expected: &fs::Metadata,
    destination: &Path,
    filtered: bool,
) -> Result<(), String> {
    authorize_existing_directory(source, canonical_root, expected, "source package tree")?;
    let mut entries = fs::read_dir(source)
        .map_err(|error| format!("{}: {error}", source.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if filtered && should_skip(&name.to_string_lossy()) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(&name);
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?;
        let kind = classify(&metadata);
        if kind == "directory" {
            fs::create_dir(&destination_path)
                .map_err(|error| format!("{}: {error}", destination_path.display()))?;
            normalize_directory_permissions(&destination_path)?;
            copy_directory(
                &source_path,
                canonical_root,
                &metadata,
                &destination_path,
                filtered,
            )?;
        } else if kind == "regular file" {
            reject_hard_link(&source_path, &metadata, "source package tree")?;
            authorize_existing_file(
                &source_path,
                canonical_root,
                &metadata,
                "source package tree",
            )?;
            copy_regular_file(&source_path, &destination_path, &metadata)?;
        } else {
            return Err(format!(
                "source package tree policy rejected {} ({kind})",
                source_path.display()
            ));
        }
    }
    Ok(())
}

fn copy_regular_file(
    source: &Path,
    destination: &Path,
    before: &fs::Metadata,
) -> Result<(), String> {
    let mut input =
        open_file_no_follow(source).map_err(|error| format!("{}: {error}", source.display()))?;
    let opened = input
        .metadata()
        .map_err(|error| format!("{}: {error}", source.display()))?;
    if !opened.is_file() || !same_file(before, &opened) {
        return Err(format!(
            "source package tree policy rejected {}: file was replaced while snapshotting",
            source.display()
        ));
    }
    let mut output = create_new_file(destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    io::copy(&mut input, &mut output).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    output
        .sync_all()
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    normalize_file_permissions(destination, executable(before))
}

fn open_file_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

fn create_new_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino() && left.file_type() == right.file_type()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn reject_hard_link(path: &Path, metadata: &fs::Metadata, tree_name: &str) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        return Err(format!(
            "{tree_name} policy rejected {} (hard link count {})",
            path.display(),
            metadata.nlink()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_hard_link(
    _path: &Path,
    _metadata: &fs::Metadata,
    _tree_name: &str,
) -> Result<(), String> {
    Ok(())
}

fn tree_rejection(root: &Path, path: &Path, tree_name: &str, kind: &str) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    format!(
        "{tree_name} policy rejected {} ({kind}); packages may contain only regular files and directories, and symlinks require lock regeneration under the reject-all-symlinks policy",
        relative.display()
    )
}

fn classify(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        "symlink"
    } else if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else {
        classify_special(&file_type)
    }
}

#[cfg(unix)]
fn classify_special(file_type: &fs::FileType) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    if file_type.is_socket() {
        "socket"
    } else if file_type.is_fifo() {
        "fifo"
    } else if file_type.is_char_device() || file_type.is_block_device() {
        "device"
    } else {
        "special file"
    }
}

#[cfg(not(unix))]
fn classify_special(_file_type: &fs::FileType) -> &'static str {
    "special file"
}

fn create_private_directory(path: &Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    normalize_directory_permissions(path)
}

#[cfg(unix)]
fn executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn normalize_directory_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(not(unix))]
fn normalize_directory_permissions(path: &Path) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(unix)]
fn normalize_file_permissions(path: &Path, executable: bool) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(not(unix))]
fn normalize_file_permissions(path: &Path, _executable: bool) -> Result<(), String> {
    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("{}: {error}", path.display()))?
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn should_skip(name: &str) -> bool {
    matches!(name, ".git" | ".jj" | ".hg" | ".svn" | "target" | "result")
        || name.starts_with("result-")
}
