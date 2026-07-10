use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::media;

use crate::media_ownership::MediaRootSelections;
use crate::path_authorization::PathAuthorizer;
use crate::planner::{MediaDeclarationProvenance, TargetPlan};

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

/// Validate a composed target by resolving every declaration through its final
/// package owner. A relative path is never allowed to select another package's root.
pub(crate) fn validate_owned_media_assets(
    plan: &TargetPlan,
    deck: &CanonicalDeck,
    roots: &MediaRootSelections,
) -> Result<(), String> {
    validate_media_references(deck)?;
    roots.require_for_plan(plan)?;
    ensure_plan_matches_deck(plan, deck)?;
    for declaration in plan.media_declarations.values() {
        let root = roots.require_for_declaration(&plan.qualified_name, declaration)?;
        let bytes = read_owned_asset(&plan.qualified_name, declaration, root)?;
        if declaration.sha256.is_empty() {
            return Err(owned_error(
                &plan.qualified_name,
                declaration,
                root,
                "media entry has empty sha256",
            ));
        }
        let actual = media::sha256_hex(&bytes);
        if actual != declaration.sha256 {
            return Err(owned_error(
                &plan.qualified_name,
                declaration,
                root,
                &format!(
                    "sha256 mismatch: expected {}, actual {actual}",
                    declaration.sha256
                ),
            ));
        }
    }
    Ok(())
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

pub(crate) fn copy_owned_media_assets(
    plan: &TargetPlan,
    deck: &CanonicalDeck,
    roots: &MediaRootSelections,
    out_dir: &Path,
) -> Result<(), String> {
    roots.require_for_plan(plan)?;
    ensure_plan_matches_deck(plan, deck)?;
    let destination_authorizer = PathAuthorizer::new("export", out_dir)?;
    for declaration in plan.media_declarations.values() {
        let root = roots.require_for_declaration(&plan.qualified_name, declaration)?;
        let source = owned_asset_path(&plan.qualified_name, declaration, root)?;
        let destination = destination_authorizer
            .authorize_create(
                Path::new("<export>"),
                format!("media.{}.path", declaration.id),
                &format!("media/{}", declaration.path),
            )
            .map_err(|error| error.to_string())?
            .into_path_buf();
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        fs::copy(&source, &destination).map_err(|error| {
            owned_error(
                &plan.qualified_name,
                declaration,
                root,
                &format!("copy to {} failed: {error}", destination.display()),
            )
        })?;
    }
    Ok(())
}

fn ensure_plan_matches_deck(plan: &TargetPlan, deck: &CanonicalDeck) -> Result<(), String> {
    if plan.media_declarations.len() != deck.media.len() {
        return Err(format!(
            "media ownership plan for target {} has {} declarations, composed deck has {}",
            plan.qualified_name,
            plan.media_declarations.len(),
            deck.media.len()
        ));
    }
    for binding in plan.media_reference_bindings(deck)? {
        let Some(owner) = plan.media_declarations.get(&binding.declaration.id) else {
            return Err(format!(
                "media ownership plan for target {} lost declaration {} bound from reference {}",
                plan.qualified_name, binding.declaration.id, binding.reference
            ));
        };
        if owner != &binding.declaration {
            return Err(format!(
                "media ownership plan for target {} changed owner after binding reference {}",
                plan.qualified_name, binding.reference
            ));
        }
    }
    for media in deck.media.values() {
        let Some(owner) = plan.media_declarations.get(media.id.as_str()) else {
            return Err(format!(
                "media ownership plan for target {} has no owner for declaration {}",
                plan.qualified_name, media.id
            ));
        };
        if owner.path != media.path || owner.sha256 != media.sha256 {
            return Err(format!(
                "media ownership plan for target {} disagrees with final declaration {}: planned path/hash {:?}/{:?}, composed {:?}/{:?}",
                plan.qualified_name, media.id, owner.path, owner.sha256, media.path, media.sha256
            ));
        }
    }
    Ok(())
}

fn read_owned_asset(
    target: &str,
    declaration: &MediaDeclarationProvenance,
    root: &Path,
) -> Result<Vec<u8>, String> {
    let path = owned_asset_path(target, declaration, root)?;
    fs::read(&path).map_err(|error| owned_error(target, declaration, root, &error.to_string()))
}

fn owned_asset_path(
    target: &str,
    declaration: &MediaDeclarationProvenance,
    root: &Path,
) -> Result<std::path::PathBuf, String> {
    PathAuthorizer::new(
        format!("media for package {}", declaration.package_label()),
        root,
    )?
    .authorize_read(
        &declaration.source,
        format!("media.{}.path", declaration.id),
        &declaration.path,
    )
    .map(|path| path.into_path_buf())
    .map_err(|error| owned_error(target, declaration, root, &error.to_string()))
}

fn owned_error(
    target: &str,
    declaration: &MediaDeclarationProvenance,
    root: &Path,
    reason: &str,
) -> String {
    format!(
        "media ownership error for target {target}, package {}, declaration {}, source {}, path {:?}, root {} ({}): {reason}",
        declaration.package_label(),
        declaration.id,
        declaration.source.display(),
        declaration.path,
        root.display(),
        declaration.source_kind.ownership_name()
    )
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
