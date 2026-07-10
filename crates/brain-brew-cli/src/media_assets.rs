use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use brain_brew_core::CanonicalDeck;
use brain_brew_formats::media;

use crate::media_ownership::MediaRootSelections;
use crate::media_verification::MediaVerificationMode;
use crate::path_authorization::PathAuthorizer;
use crate::planner::{MediaDeclarationProvenance, TargetPlan};

#[derive(Debug)]
pub(crate) struct MediaVerificationResult {
    pub(crate) warnings: Vec<String>,
    pub(crate) assets: Vec<(PathBuf, Vec<u8>)>,
}

pub(crate) fn validate_media_semantics(
    deck: &CanonicalDeck,
    mode: MediaVerificationMode,
) -> Result<Vec<String>, String> {
    let report = media::reference_report(deck);
    if !report.errors.is_empty() {
        return Err(media::MediaValidationReport {
            errors: report.errors,
        }
        .to_string());
    }
    let hash_policy = match mode {
        MediaVerificationMode::Strict => media::MediaHashPolicy::Required,
        MediaVerificationMode::ReferenceOnly => media::MediaHashPolicy::Optional,
    };
    media::validate_declarations(deck, hash_policy).map_err(|error| error.to_string())?;
    Ok(report
        .warnings
        .into_iter()
        .map(|warning| warning.message)
        .collect())
}

pub(crate) fn collect_media_assets(
    deck: &CanonicalDeck,
    media_root: &Path,
) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    validate_media_semantics(deck, MediaVerificationMode::Strict)?;
    let assets = read_media_assets(deck, media_root)?;
    media::validate_hashes(deck, &assets).map_err(|error| error.to_string())?;
    let mut output = BTreeMap::new();
    for declaration in deck.media.values() {
        let bytes = assets.get(&declaration.path).cloned().ok_or_else(|| {
            format!(
                "{}: declared media file {:?} was not read",
                media_root.display(),
                declaration.path
            )
        })?;
        output.insert(PathBuf::from("media").join(&declaration.path), bytes);
    }
    Ok(output.into_iter().collect())
}

/// Validate a composed target by resolving every declaration through its final
/// package owner. A relative path is never allowed to select another package's root.
pub(crate) fn verify_owned_media(
    plan: &TargetPlan,
    deck: &CanonicalDeck,
    roots: &MediaRootSelections,
    mode: MediaVerificationMode,
    collect_assets: bool,
) -> Result<MediaVerificationResult, String> {
    let mut warnings = validate_media_semantics(deck, mode)?;
    ensure_plan_matches_deck(plan, deck)?;
    plan.media_reference_bindings(deck)?;
    let assets = match mode {
        MediaVerificationMode::Strict => {
            collect_owned_media_assets(plan, deck, roots, collect_assets)?
        }
        MediaVerificationMode::ReferenceOnly => Vec::new(),
    };
    if let Some(warning) = mode.development_warning(deck.media.len()) {
        warnings.push(warning);
    }
    Ok(MediaVerificationResult {
        warnings,
        assets: if collect_assets { assets } else { Vec::new() },
    })
}

fn collect_owned_media_assets(
    plan: &TargetPlan,
    deck: &CanonicalDeck,
    roots: &MediaRootSelections,
    retain_assets: bool,
) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    validate_media_semantics(deck, MediaVerificationMode::Strict)?;
    roots.require_for_plan(plan)?;
    ensure_plan_matches_deck(plan, deck)?;
    let mut assets = BTreeMap::new();
    for declaration in plan.media_declarations.values() {
        let root = roots.require_for_declaration(&plan.qualified_name, declaration)?;
        let (bytes, actual) =
            read_owned_asset(&plan.qualified_name, declaration, root, retain_assets)?;
        if declaration.sha256.is_empty() {
            return Err(owned_error(
                &plan.qualified_name,
                declaration,
                root,
                "media entry has empty sha256",
            ));
        }
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
        if let Some(bytes) = bytes {
            let output_path = PathBuf::from("media").join(&declaration.path);
            if let Some(previous) = assets.insert(output_path.clone(), bytes.clone())
                && previous != bytes
            {
                return Err(format!(
                    "media output collision for target {} at {} selected different bytes",
                    plan.qualified_name,
                    output_path.display()
                ));
            }
        }
    }
    Ok(assets.into_iter().collect())
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
    retain_bytes: bool,
) -> Result<(Option<Vec<u8>>, String), String> {
    let path = owned_asset_path(target, declaration, root)?;
    let mut file = File::open(&path)
        .map_err(|error| owned_error(target, declaration, root, &error.to_string()))?;
    let mut bytes = Vec::new();
    let mut hasher = Sha256::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| owned_error(target, declaration, root, &error.to_string()))?;
        if count == 0 {
            break;
        }
        hasher.update(&chunk[..count]);
        if retain_bytes {
            bytes.extend_from_slice(&chunk[..count]);
        }
    }
    Ok((
        retain_bytes.then_some(bytes),
        format!("{:x}", hasher.finalize()),
    ))
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
