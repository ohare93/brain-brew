use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::media;

use crate::path_authorization::PathAuthorizer;

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
    let authorizer = PathAuthorizer::new("media", media_root)?;
    for media in deck.media.values() {
        let full_path = authorizer
            .authorize_read(
                Path::new("<composed deck>"),
                format!("media.{}.path", media.id),
                &media.path,
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
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
    let source_authorizer = PathAuthorizer::new("media", media_root)?;
    let destination_authorizer = PathAuthorizer::new("export", out_dir)?;
    for media in deck.media.values() {
        let source = source_authorizer
            .authorize_read(
                Path::new("<composed deck>"),
                format!("media.{}.path", media.id),
                &media.path,
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
        let destination = destination_authorizer
            .authorize_create(
                Path::new("<export>"),
                format!("media.{}.path", media.id),
                &format!("media/{}", media.path),
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::copy(&source, &destination).map_err(|error| {
            format!("{} -> {}: {error}", source.display(), destination.display())
        })?;
    }
    Ok(())
}
