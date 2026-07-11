use std::fs;
use std::path::{Path, PathBuf};

use brain_brew_formats::canonical_source_document::CanonicalSourceDocument;
use brain_brew_formats::crowdanki::{self, CrowdAnkiImportPlan};
use brain_brew_formats::source_document::{SourceFile, SourceProvenance};
use serde_json::json;

use crate::workspace_mutation::{
    PlannedWorkspaceFile, commit_workspace_files, nearest_existing_ancestor, recover_workspace,
    write_output_file,
};

const USAGE: &str = "usage:\n  brainbrew import crowdanki plan <deck-folder> --out import-plan.json [--force] [--json]\n  brainbrew import crowdanki review --plan import-plan.json [--json]\n  brainbrew import crowdanki apply <deck-folder> --plan import-plan.json --approve-plan --out deck.yaml [--force] [--json]";

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
    let plan = crowdanki::plan_import(&source)
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
    let output_path = absolute_output_path(&parsed.out_path)?;
    let output_root = output_root(&output_path)?;
    recover_workspace(&output_root)?;
    let deck_json_path = parsed.deck_dir.join("deck.json");
    let deck_json = fs::read(&deck_json_path)
        .map_err(|error| format!("{}: {error}", deck_json_path.display()))?;
    let plan_bytes = fs::read(&parsed.plan_path)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    let plan = CrowdAnkiImportPlan::from_bytes(&plan_bytes)
        .map_err(|error| format!("{}: {error}", parsed.plan_path.display()))?;
    let deck = crowdanki::apply_import_plan(&deck_json, &plan, parsed.approve_plan)
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
    let planned = planned_output(&output_path, parsed.force, replacement)?;
    commit_workspace_files(&output_root, vec![planned])?;
    if parsed.json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "schema_version": 1,
                "action": "apply",
                "plan": parsed.plan_path,
                "out": parsed.out_path,
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
    force: bool,
    approve_plan: bool,
    json: bool,
}

struct ImportFlags {
    out_path: Option<PathBuf>,
    force: bool,
    plan_path: Option<PathBuf>,
    approve_plan: bool,
    json: bool,
}

fn parse_plan_args(args: &[String]) -> Result<PlanArgs, String> {
    let (deck_dir, flags) = positional_and_flags(args)?;
    let flags = parse_import_flags(&flags)?;
    if flags.approve_plan || flags.plan_path.is_some() {
        return Err("plan generation accepts only --out, --force, and --json".to_owned());
    }
    Ok(PlanArgs {
        deck_dir,
        out_path: flags.out_path.ok_or_else(|| "missing --out".to_owned())?,
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
    if flags.force || flags.approve_plan {
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
            if matches!(args[index].as_str(), "--out" | "--plan") && index + 1 < args.len() {
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
            "--out" | "--plan" | "--force" | "--approve-plan" | "--json" => {
                return Err(format!("duplicate import argument {:?}", args[index]));
            }
            other => return Err(format!("unexpected import argument {other:?}")),
        }
    }
    Ok(ImportFlags {
        out_path: out,
        force,
        plan_path: plan,
        approve_plan: approve,
        json,
    })
}

fn planned_output(
    path: &Path,
    force: bool,
    replacement: Vec<u8>,
) -> Result<PlannedWorkspaceFile, String> {
    match fs::symlink_metadata(path) {
        Ok(_) if !force => Err(format!(
            "refusing to overwrite existing import output {}; pass --force to replace an existing regular file",
            path.display()
        )),
        Ok(metadata) if !metadata.file_type().is_file() => Err(format!(
            "refusing to replace non-file import output {}",
            path.display()
        )),
        Ok(_) => PlannedWorkspaceFile::validated(
            path,
            fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?,
            replacement,
            canonical_import_validator(path),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            PlannedWorkspaceFile::validated_new(path, replacement, canonical_import_validator(path))
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}
fn output_root(out_path: &Path) -> Result<PathBuf, String> {
    nearest_existing_ancestor(
        out_path
            .parent()
            .ok_or_else(|| format!("import output {} has no parent", out_path.display()))?,
    )
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
