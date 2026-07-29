use std::fs;
use std::path::Path;

use brain_brew_formats::{
    canonical_yaml, lockfile, manifest, media_map, note_type_map, source_includes,
};

use crate::help;
use crate::io::{
    canonical_source_document, overlay_source_document, workspace_root_for_source_path,
};
use crate::output;
use crate::workspace_mutation::{PlannedWorkspaceFile, commit_workspace_files, recover_workspace};

#[derive(Clone, Copy)]
enum SourceKind {
    CanonicalDeck,
    Overlay,
    Manifest,
    Lockfile,
    MediaMap,
    NoteTypeMap,
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!("{}", help::command("fmt").expect("fmt help exists"));
        return Ok(());
    }
    if args.len() != 1 {
        return Err(help::usage_error("fmt", "usage: brainbrew fmt <deck.yaml>"));
    }
    let path = Path::new(&args[0]);
    let workspace_root = workspace_root_for_source_path(path);
    recover_workspace(&workspace_root)?;

    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let migration_diagnostics =
        canonical_yaml::overlay_migration_diagnostics(&input).unwrap_or_default();
    let (kind, formatted) = format_typed_source(path, &input)?;
    if formatted != input {
        let replacement = formatted.into_bytes();
        let original = input.into_bytes();
        let planned = PlannedWorkspaceFile::validated(path, original, replacement, |bytes| {
            validate_typed_source(path, kind, bytes)
        })?;
        commit_workspace_files(&workspace_root, vec![planned])?;
    }
    output::print_success("formatted source", &[("path", path.display().to_string())]);
    for diagnostic in migration_diagnostics {
        eprintln!("warning: {}: {}", diagnostic.path, diagnostic.message);
    }
    Ok(())
}

fn format_typed_source(path: &Path, input: &str) -> Result<(SourceKind, String), String> {
    let mut errors = Vec::new();
    match canonical_source_document(path, input).and_then(|document| {
        document
            .emit()
            .map(|emission| emission.root().text().to_owned())
            .map_err(|error| error.to_string())
    }) {
        Ok(formatted) => return Ok((SourceKind::CanonicalDeck, formatted)),
        Err(error) => errors.push(format!("deck: {error}")),
    }
    match overlay_source_document(path, input).and_then(|document| {
        document
            .emit()
            .map(|emission| emission.root().text().to_owned())
            .map_err(|error| error.to_string())
    }) {
        Ok(formatted) => return Ok((SourceKind::Overlay, formatted)),
        Err(error) => errors.push(format!("overlay: {error}")),
    }
    match manifest::format_str(input) {
        Ok(formatted) => return Ok((SourceKind::Manifest, formatted)),
        Err(error) => errors.push(format!("manifest: {error}")),
    }
    match lockfile::format_str(input) {
        Ok(formatted) => return Ok((SourceKind::Lockfile, formatted)),
        Err(error) => errors.push(format!("lockfile: {error}")),
    }
    match media_map::format_str(input) {
        Ok(formatted) => return Ok((SourceKind::MediaMap, formatted)),
        Err(error) => errors.push(format!("media map: {error}")),
    }
    match source_includes::format_preserving_file_includes(input, note_type_map::format_str) {
        Ok(formatted) => return Ok((SourceKind::NoteTypeMap, formatted)),
        Err(error) => errors.push(format!("note-type map: {error}")),
    }
    Err(format!(
        "{}: unrecognized Brain Brew source file ({})",
        path.display(),
        errors.join("; ")
    ))
}

fn validate_typed_source(path: &Path, kind: SourceKind, bytes: &[u8]) -> Result<(), String> {
    let input = std::str::from_utf8(bytes)
        .map_err(|error| format!("{}: replacement is not UTF-8: {error}", path.display()))?;
    match kind {
        SourceKind::CanonicalDeck => canonical_source_document(path, input).and_then(|document| {
            document
                .emit()
                .map(|_| ())
                .map_err(|error| error.to_string())
        }),
        SourceKind::Overlay => overlay_source_document(path, input).and_then(|document| {
            document
                .emit()
                .map(|_| ())
                .map_err(|error| error.to_string())
        }),
        SourceKind::Manifest => manifest::from_str(input)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        SourceKind::Lockfile => lockfile::from_str(input)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        SourceKind::MediaMap => media_map::from_str(input)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        SourceKind::NoteTypeMap => {
            source_includes::format_preserving_file_includes(input, note_type_map::format_str)
                .map(|_| ())
        }
    }
}
