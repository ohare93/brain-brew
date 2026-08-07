use std::path::Path;

use brain_brew_core::{ComposeReport, ValidationReport};
use serde_json::{Value, json};

use crate::args::{parse_manifest_target_args, split_json_flag};
use crate::help;
use crate::io::{read_and_compose_deck, read_deck_and_overlays};
use crate::output;
use crate::planner::plan_manifest_target;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| arg == "--manifest" || arg == "--target")
    {
        let (json_output, rest) = split_json_flag(args);
        let manifest_args = parse_manifest_target_args(&rest)?;
        let deck = if json_output {
            let plan = plan_manifest_target(
                &manifest_args.manifest_path,
                &manifest_args.target,
                &manifest_args.include_paths,
                &manifest_args.package_roots,
                &manifest_args.discovery_policy,
            )?;
            let overlays = plan
                .overlays
                .iter()
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>();
            match plan.base.compose(&overlays) {
                Ok(deck) => deck,
                Err(report) => return report_json_compose_failure(report, "invalid deck"),
            }
        } else {
            plan_manifest_target(
                &manifest_args.manifest_path,
                &manifest_args.target,
                &manifest_args.include_paths,
                &manifest_args.package_roots,
                &manifest_args.discovery_policy,
            )?
            .compose()
            .map_err(|report| {
                output::compose_error(
                    "validate",
                    json!({
                        "manifest": manifest_args.manifest_path,
                        "target": manifest_args.target,
                    }),
                    &report,
                )
            })?
        };
        let mut details = vec![
            (
                "manifest",
                manifest_args.manifest_path.display().to_string(),
            ),
            ("target", manifest_args.target.clone()),
        ];
        details.extend(output::deck_stats(&deck));
        return report_validation(
            deck.validate(),
            json_output,
            format!("valid target {}", manifest_args.target),
            details,
        );
    }

    let mut json_output = false;
    let mut deck_path = None;
    let mut overlay_paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json_output = true;
                index += 1;
            }
            "--overlay" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--overlay requires a path".to_owned());
                };
                overlay_paths.push(path.clone());
                index += 2;
            }
            value if deck_path.is_none() => {
                deck_path = Some(value.to_owned());
                index += 1;
            }
            other => return Err(format!("unexpected validate argument {other:?}")),
        }
    }
    let Some(deck_path) = deck_path else {
        return Err(help::usage_error(
            "validate",
            "usage: brainbrew validate <deck.yaml> [--overlay overlay.yaml ...] [--json]",
        ));
    };

    let deck = if json_output {
        let (base, overlays) = read_deck_and_overlays(Path::new(&deck_path), &overlay_paths)?;
        let overlays = overlays
            .into_iter()
            .map(|(_, overlay)| overlay)
            .collect::<Vec<_>>();
        match base.compose(&overlays) {
            Ok(deck) => deck,
            Err(report) => return report_json_compose_failure(report, "invalid deck"),
        }
    } else {
        read_and_compose_deck(Path::new(&deck_path), &overlay_paths)?
    };
    let mut details = vec![("source", deck_path.clone())];
    if !overlay_paths.is_empty() {
        details.push(("overlays", overlay_paths.len().to_string()));
    }
    details.extend(output::deck_stats(&deck));
    report_validation(
        deck.validate(),
        json_output,
        "valid deck".to_owned(),
        details,
    )
}

fn report_validation(
    result: Result<(), ValidationReport>,
    json_output: bool,
    success_message: String,
    details: Vec<(&str, String)>,
) -> Result<(), String> {
    match result {
        Ok(()) => {
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({"status": "valid", "errors": []}))
                        .unwrap()
                );
            } else {
                output::print_success(success_message, &details);
            }
            Ok(())
        }
        Err(report) => {
            let diagnostics = report
                .errors
                .iter()
                .map(|error| error.diagnostic())
                .collect::<Vec<_>>();
            if json_output {
                report_json_diagnostics("invalid deck", diagnostics)
            } else {
                eprintln!("{}", output::render_diagnostics(&diagnostics));
                Err("invalid deck".to_owned())
            }
        }
    }
}

fn report_json_compose_failure(report: ComposeReport, message: &str) -> Result<(), String> {
    report_json_diagnostics(
        message,
        report
            .errors
            .iter()
            .map(|error| error.diagnostic())
            .collect(),
    )
}

fn report_json_diagnostics(
    message: &str,
    diagnostics: Vec<brain_brew_core::DomainDiagnostic>,
) -> Result<(), String> {
    output::print_json_diagnostic_error("validate", Value::Null, message, &diagnostics);
    Err(output::JSON_ERROR_ALREADY_PRINTED.to_owned())
}
