use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::crowdanki;
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};

use crate::workspace_mutation::{
    PlannedWorkspaceFile, commit_workspace_files, nearest_existing_ancestor, recover_workspace,
};

const USAGE: &str = "usage: brainbrew import crowdanki <deck-folder> --accept-suggested-ids [--force] --out deck.yaml";

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let output_path = absolute_output_path(&parsed.out_path)?;
    let output_root = output_root(&output_path)?;
    recover_workspace(&output_root)?;

    let deck_json_path = parsed.deck_dir.join("deck.json");
    let deck_json = fs::read_to_string(&deck_json_path)
        .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;
    let deck = crowdanki::import_deck_accept_suggested_ids(&deck_json)
        .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;

    let provenance = SourceProvenance::new(output_path.display().to_string())
        .with_source_root(output_root.display().to_string());
    let emission = CanonicalSourceDocument::from_deck(provenance, deck)
        .and_then(|document| document.emit())
        .map_err(|error| error.to_string())?;
    if !emission.included().is_empty() {
        return Err("CrowdAnki import unexpectedly planned included source outputs".to_owned());
    }
    let replacement = emission.root().text().as_bytes().to_vec();

    let planned = match fs::symlink_metadata(&output_path) {
        Ok(_) if !parsed.force => {
            return Err(format!(
                "refusing to overwrite existing import output {}; pass --force to replace an existing regular file",
                output_path.display()
            ));
        }
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!(
                "refusing to replace non-file import output {}",
                output_path.display()
            ));
        }
        Ok(_) => {
            let original = fs::read(&output_path)
                .map_err(|error| format!("{}: {error}", output_path.display()))?;
            PlannedWorkspaceFile::validated(
                output_path.clone(),
                original,
                replacement,
                canonical_import_validator(&output_path),
            )?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            PlannedWorkspaceFile::validated_new(
                output_path.clone(),
                replacement,
                canonical_import_validator(&output_path),
            )?
        }
        Err(error) => return Err(format!("{}: {error}", output_path.display())),
    };

    commit_workspace_files(&output_root, vec![planned])?;
    println!("imported crowdanki deck");
    Ok(())
}

struct ImportArgs {
    deck_dir: PathBuf,
    out_path: PathBuf,
    force: bool,
}

fn parse_args(args: &[String]) -> Result<ImportArgs, String> {
    if args.first().map(String::as_str) != Some("crowdanki") {
        return Err(USAGE.to_owned());
    }
    let Some(deck_dir) = args.get(1).filter(|value| !value.starts_with('-')) else {
        return Err(USAGE.to_owned());
    };
    let mut accepted_ids = false;
    let mut force = false;
    let mut out_path = None;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--accept-suggested-ids" if !accepted_ids => {
                accepted_ids = true;
                index += 1;
            }
            "--force" if !force => {
                force = true;
                index += 1;
            }
            "--out" if out_path.is_none() => {
                let Some(path) = args.get(index + 1).filter(|value| !value.starts_with('-')) else {
                    return Err("--out requires a path".to_owned());
                };
                out_path = Some(PathBuf::from(path));
                index += 2;
            }
            duplicate if matches!(duplicate, "--accept-suggested-ids" | "--force" | "--out") => {
                return Err(format!("duplicate import argument {duplicate:?}"));
            }
            other => return Err(format!("unexpected import argument {other:?}")),
        }
    }
    if !accepted_ids {
        return Err(
            "non-interactive CrowdAnki import requires --accept-suggested-ids for now".to_owned(),
        );
    }
    let out_path = out_path.ok_or_else(|| "missing --out".to_owned())?;
    Ok(ImportArgs {
        deck_dir: PathBuf::from(deck_dir),
        out_path,
        force,
    })
}

fn output_root(out_path: &Path) -> Result<PathBuf, String> {
    let parent = out_path
        .parent()
        .ok_or_else(|| format!("import output {} has no parent", out_path.display()))?;
    nearest_existing_ancestor(parent)
}

fn absolute_output_path(out_path: &Path) -> Result<PathBuf, String> {
    if out_path.file_name().is_none() {
        return Err(format!(
            "import output {} has no file name",
            out_path.display()
        ));
    }
    if out_path.is_absolute() {
        Ok(out_path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(out_path))
            .map_err(|error| format!("cannot resolve current directory: {error}"))
    }
}

fn canonical_import_validator(path: &Path) -> impl FnOnce(&[u8]) -> Result<(), String> + '_ {
    move |bytes| {
        let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
        let source = SourceFile::new(SourceProvenance::new(path.display().to_string()), text);
        let document = CanonicalSourceDocument::parse(source).map_err(|error| error.to_string())?;
        let emitted = document.emit().map_err(|error| error.to_string())?;
        if emitted.root().text() == text {
            Ok(())
        } else {
            Err("generated import output is not canonical Canonical Deck source".to_owned())
        }
    }
}
