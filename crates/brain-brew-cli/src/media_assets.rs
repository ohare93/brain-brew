use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::media;

pub(crate) fn validate_media_references(deck: &CanonicalDeck) -> Result<(), String> {
    let report = media::reference_report(deck);
    for warning in &report.warnings {
        eprintln!("warning: {}", warning.message);
    }
    if report.errors.is_empty() {
        Ok(())
    } else {
        Err(media::MediaValidationReport {
            errors: report.errors,
        }
        .to_string())
    }
}

pub(crate) fn validate_media_assets(deck: &CanonicalDeck, media_root: &Path) -> Result<(), String> {
    validate_media_references(deck)?;
    let assets = read_media_assets(deck, media_root)?;
    media::validate_hashes(deck, &assets).map_err(|error| error.to_string())
}

fn read_media_assets(
    deck: &CanonicalDeck,
    media_root: &Path,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut assets = BTreeMap::new();
    for media in deck.media.values() {
        let relative_path = safe_media_relative_path(&media.path)?;
        let full_path = media_root.join(&relative_path);
        match fs::read(&full_path) {
            Ok(bytes) => {
                assets.insert(media.path.clone(), bytes);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", full_path.display())),
        }
    }
    Ok(assets)
}

pub(crate) fn copy_media_assets(
    deck: &CanonicalDeck,
    media_root: &Path,
    out_dir: &Path,
) -> Result<(), String> {
    for media in deck.media.values() {
        let relative_path = safe_media_relative_path(&media.path)?;
        let source = media_root.join(&relative_path);
        let destination = out_dir.join("media").join(&relative_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::copy(&source, &destination).map_err(|error| {
            format!("{} -> {}: {error}", source.display(), destination.display())
        })?;
    }
    Ok(())
}

fn safe_media_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!("unsafe media path {}", path.display()));
    }
    Ok(path.to_path_buf())
}
