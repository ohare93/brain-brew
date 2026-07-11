use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::crowdanki::{self, CrowdAnkiImportPlan};
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use serde_json::json;

use crate::output_transaction::{OutputArtifact, publish_output_tree};
use crate::path_authorization::PathAuthorizer;
use crate::workspace_mutation::write_output_file;

const USAGE: &str = "usage:\n  brainbrew import crowdanki plan <deck-folder> --out import-plan.json [--media-root media/ | --media-mode reference-only] [--force] [--json]\n  brainbrew import crowdanki review --plan import-plan.json [--json]\n  brainbrew import crowdanki apply <deck-folder> --plan import-plan.json --approve-plan --out workspace-dir [--media-root media/ | --media-mode reference-only] [--force] [--json]";

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) != Some("crowdanki") {
        return Err(USAGE.to_owned());
    }
    match args.get(1).map(String::as_str) {
        Some("plan") => run_plan(&args[2..]),
        Some("review") => run_review(&args[2..]),
        Some("apply") => run_apply(&args[2..]),
        Some(_) if args.iter().any(|arg| arg == "--accept-suggested-ids") => Err(
            "--accept-suggested-ids is removed: generate and review a plan with `brainbrew import crowdanki plan`, then apply it with `--approve-plan`"
                .to_owned(),
        ),
        _ => Err(USAGE.to_owned()),
    }
}

fn run_plan(args: &[String]) -> Result<(), String> {
    let parsed = parse_plan_args(args)?;
    let source_path = parsed.deck_dir.join("deck.json");
    let source =
        fs::read(&source_path).map_err(|error| format!("{}: {error}", source_path.display()))?;
    let supplied = read_import_media_bytes(
        &source,
        &source_path,
        parsed.media_root.as_deref(),
        parsed.reference_only,
    )?;
    let plan = if parsed.reference_only {
        crowdanki::plan_import(&source)
    } else {
        crowdanki::plan_import_with_media(&source, &supplied)
    }
    .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let plan_bytes = if parsed
        .out_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("yaml")
        || parsed
            .out_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("yml")
    {
        plan.to_canonical_yaml()
            .map_err(|error| error.to_string())?
            .into_bytes()
    } else {
        plan.to_canonical_json()
            .map_err(|error| error.to_string())?
            .into_bytes()
    };
    write_output_file(&parsed.out_path, plan_bytes, parsed.force, |bytes| {
        let reread = CrowdAnkiImportPlan::from_bytes(bytes).map_err(|error| error.to_string())?;
        if reread == plan {
            Ok(())
        } else {
            Err("generated import plan is not canonical".to_owned())
        }
    })?;
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": 1,
                "action": "plan",
                "plan": parsed.out_path,
                "entries": plan.entries.len(),
                "source_sha256": plan.provenance.source_sha256,
            }))
            .expect("JSON success serializes")
        );
    } else {
        println!(
            "generated CrowdAnki import plan: {}",
            parsed.out_path.display()
        );
        print_review_summary(&plan);
    }
    Ok(())
}

fn run_review(args: &[String]) -> Result<(), String> {
    let parsed = parse_review_args(args)?;
    let bytes = fs::read(&parsed.plan_path)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    let plan = CrowdAnkiImportPlan::from_bytes(&bytes)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": 1,
                "action": "review",
                "plan": parsed.plan_path,
                "format": plan.format,
                "version": plan.version,
                "entries": plan.entries,
            }))
            .expect("JSON success serializes")
        );
    } else {
        println!("CrowdAnki import plan: {} v{}", plan.format, plan.version);
        println!(
            "source sha256: {} ({} bytes)",
            plan.provenance.source_sha256, plan.provenance.source_bytes
        );
        print_review_summary(&plan);
    }
    Ok(())
}

fn run_apply(args: &[String]) -> Result<(), String> {
    let parsed = parse_apply_args(args)?;
    let deck_json_path = parsed.deck_dir.join("deck.json");
    let deck_json = fs::read(&deck_json_path)
        .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;
    let plan_bytes = fs::read(&parsed.plan_path)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    let plan = CrowdAnkiImportPlan::from_bytes(&plan_bytes)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    let supplied = read_import_media_bytes(
        &deck_json,
        &deck_json_path,
        parsed.media_root.as_deref(),
        parsed.reference_only,
    )?;
    let deck = if parsed.reference_only {
        crowdanki::apply_import_plan(&deck_json, &plan, parsed.approve_plan)
    } else {
        crowdanki::apply_import_plan_with_media(&deck_json, &plan, parsed.approve_plan, &supplied)
    }
    .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;
    let provenance = SourceProvenance::new(parsed.out_path.join("deck.yaml").display().to_string())
        .with_source_root(parsed.out_path.display().to_string());
    let emission = CanonicalSourceDocument::from_deck(provenance, deck)
        .and_then(|document| document.emit())
        .map_err(|error| error.to_string())?;
    if !emission.included().is_empty() {
        return Err("CrowdAnki import unexpectedly planned included source outputs".to_owned());
    }
    let source_bytes = emission.root().text().as_bytes().to_vec();
    // Keep the pre-v2 source-file destination only for explicit reference-only imports.
    // Strict imports always use the clean source-plus-media workspace transaction below.
    if parsed.reference_only && parsed.out_path.extension().is_some() {
        write_output_file(
            &parsed.out_path,
            source_bytes,
            parsed.force,
            canonical_import_validator(&parsed.out_path),
        )?;
    } else {
        let mut artifacts = vec![OutputArtifact::new("deck.yaml", source_bytes)];
        if !parsed.reference_only {
            artifacts.extend(supplied.into_iter().map(|asset| {
                OutputArtifact::new(PathBuf::from("media").join(asset.path), asset.bytes)
            }));
        }
        publish_output_tree(&parsed.out_path, artifacts, parsed.force)?;
    }
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": 1,
                "action": "apply",
                "plan": parsed.plan_path,
                "out": parsed.out_path,
                "media_mode": if parsed.reference_only { "reference_only" } else { "strict" },
                "status": "imported",
            }))
            .expect("JSON success serializes")
        );
    } else {
        println!("applied reviewed CrowdAnki import plan");
    }
    Ok(())
}

fn print_review_summary(plan: &CrowdAnkiImportPlan) {
    let automatic = plan
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                crowdanki::CrowdAnkiImportPlanStatus::Automatic
            )
        })
        .count();
    let unresolved = plan
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.status,
                crowdanki::CrowdAnkiImportPlanStatus::RequiresOverride
            )
        })
        .count();
    println!("  automatic suggestions: {automatic}");
    println!("  unresolved collisions: {unresolved}");
    for entry in &plan.entries {
        let decision = match &entry.decision {
            crowdanki::CrowdAnkiImportPlanDecision::Automatic => "automatic".to_owned(),
            crowdanki::CrowdAnkiImportPlanDecision::Override { stable_id } => {
                format!("override {stable_id}")
            }
            crowdanki::CrowdAnkiImportPlanDecision::Reject => "rejected".to_owned(),
        };
        println!(
            "  {}: {} -> {} [{}]",
            entry.source_path,
            entry.kind.name(),
            entry.suggested_id,
            decision
        );
    }
}

struct PlanArgs {
    deck_dir: PathBuf,
    out_path: PathBuf,
    media_root: Option<PathBuf>,
    reference_only: bool,
    force: bool,
    json: bool,
}
struct ReviewArgs {
    plan_path: PathBuf,
    json: bool,
}
struct ApplyArgs {
    deck_dir: PathBuf,
    plan_path: PathBuf,
    out_path: PathBuf,
    media_root: Option<PathBuf>,
    reference_only: bool,
    force: bool,
    approve_plan: bool,
    json: bool,
}

struct ImportFlags {
    out_path: Option<PathBuf>,
    media_root: Option<PathBuf>,
    reference_only: bool,
    force: bool,
    plan_path: Option<PathBuf>,
    approve_plan: bool,
    json: bool,
}

fn parse_plan_args(args: &[String]) -> Result<PlanArgs, String> {
    let (deck_dir, flags) = positional_and_flags(args)?;
    let flags = parse_import_flags(&flags)?;
    if flags.approve_plan || flags.plan_path.is_some() {
        return Err("plan generation does not accept --plan or --approve-plan".to_owned());
    }
    Ok(PlanArgs {
        deck_dir,
        out_path: flags.out_path.ok_or_else(|| "missing --out".to_owned())?,
        media_root: flags.media_root,
        reference_only: flags.reference_only,
        force: flags.force,
        json: flags.json,
    })
}
fn parse_review_args(args: &[String]) -> Result<ReviewArgs, String> {
    let (_positionals, flags) = split_positionals(args);
    if !_positionals.is_empty() {
        return Err(USAGE.to_owned());
    }
    let flags = parse_import_flags(&flags)?;
    if flags.force || flags.approve_plan || flags.media_root.is_some() || flags.reference_only {
        return Err("review accepts only --plan and --json".to_owned());
    }
    Ok(ReviewArgs {
        plan_path: flags.plan_path.ok_or_else(|| "missing --plan".to_owned())?,
        json: flags.json,
    })
}
fn parse_apply_args(args: &[String]) -> Result<ApplyArgs, String> {
    let (deck_dir, flags) = positional_and_flags(args)?;
    let flags = parse_import_flags(&flags)?;
    Ok(ApplyArgs {
        deck_dir,
        plan_path: flags.plan_path.ok_or_else(|| "missing --plan".to_owned())?,
        out_path: flags.out_path.ok_or_else(|| "missing --out".to_owned())?,
        media_root: flags.media_root,
        reference_only: flags.reference_only,
        force: flags.force,
        approve_plan: flags.approve_plan,
        json: flags.json,
    })
}

fn positional_and_flags(args: &[String]) -> Result<(PathBuf, Vec<String>), String> {
    let (positionals, flags) = split_positionals(args);
    if positionals.len() != 1 {
        return Err(USAGE.to_owned());
    }
    Ok((PathBuf::from(&positionals[0]), flags))
}
fn split_positionals(args: &[String]) -> (Vec<String>, Vec<String>) {
    let mut positionals = Vec::new();
    let mut flags = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index].starts_with('-') {
            flags.push(args[index].clone());
            if matches!(
                args[index].as_str(),
                "--out" | "--plan" | "--media-root" | "--media-mode"
            ) && index + 1 < args.len()
            {
                index += 1;
                flags.push(args[index].clone());
            }
        } else {
            positionals.push(args[index].clone());
        }
        index += 1;
    }
    (positionals, flags)
}
fn parse_import_flags(args: &[String]) -> Result<ImportFlags, String> {
    let mut out = None;
    let mut media_root = None;
    let mut reference_only = false;
    let mut force = false;
    let mut plan = None;
    let mut approve = false;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" if out.is_none() => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| "--out requires a path".to_owned())?;
                out = Some(PathBuf::from(value));
                index += 2;
            }
            "--plan" if plan.is_none() => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| "--plan requires a path".to_owned())?;
                plan = Some(PathBuf::from(value));
                index += 2;
            }
            "--media-root" if media_root.is_none() => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with('-'))
                    .ok_or_else(|| "--media-root requires a directory".to_owned())?;
                media_root = Some(PathBuf::from(value));
                index += 2;
            }
            "--media-mode" if !reference_only => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--media-mode requires reference-only".to_owned())?;
                if value != "reference-only" {
                    return Err("import supports only --media-mode reference-only".to_owned());
                }
                reference_only = true;
                index += 2;
            }
            "--force" if !force => {
                force = true;
                index += 1;
            }
            "--approve-plan" if !approve => {
                approve = true;
                index += 1;
            }
            "--json" if !json => {
                json = true;
                index += 1;
            }
            "--out" | "--plan" | "--media-root" | "--media-mode" | "--force" | "--approve-plan"
            | "--json" => {
                return Err(format!("duplicate import argument {:?}", args[index]));
            }
            other => return Err(format!("unexpected import argument {other:?}")),
        }
    }
    if media_root.is_some() && reference_only {
        return Err("--media-root cannot be combined with --media-mode reference-only".to_owned());
    }
    Ok(ImportFlags {
        out_path: out,
        media_root,
        reference_only,
        force,
        plan_path: plan,
        approve_plan: approve,
        json,
    })
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

fn read_import_media_bytes(
    deck_json: &[u8],
    source_path: &Path,
    media_root: Option<&Path>,
    reference_only: bool,
) -> Result<Vec<crowdanki::CrowdAnkiImportMediaBytes>, String> {
    let references = crowdanki::import_media_references(deck_json)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    if references.is_empty() || reference_only {
        return Ok(Vec::new());
    }
    let root = media_root.ok_or_else(|| {
        format!(
            "CrowdAnki import has {} declared media paths; pass --media-root <directory> to import verified bytes or --media-mode reference-only for explicit non-release source-only output",
            references.len()
        )
    })?;
    let authorizer = PathAuthorizer::new("CrowdAnki import media", root)?;
    references
        .into_iter()
        .map(|reference| {
            let path = authorizer
                .authorize_read(source_path, &reference.source_path, &reference.path)
                .map_err(|error| error.to_string())?
                .into_path_buf();
            let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            Ok(crowdanki::CrowdAnkiImportMediaBytes {
                path: reference.path,
                bytes,
            })
        })
        .collect()
}
