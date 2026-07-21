use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use brain_brew_core::{
    Overlay, OverlayKind, TranslationContextUnit, TranslationContextView,
    TranslationCoverageCategory, TranslationCoverageEntry, TranslationCoverageReport,
    TranslationMessageContext,
};
use brain_brew_formats::manifest::FederatedDeckManifest;
use brain_brew_formats::overlay_source_document::{OverlaySourceDocument, TranslationStubs};
use brain_brew_formats::yaml_scalar::scalar as yaml_scalar;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use serde_json::json;

use crate::commands::translation_overlay::compose_lenient_translation_overlay;
use crate::help;
use crate::io::{manifest_root, overlay_from_source_text, overlay_source_document, read_manifest};
use crate::output;
use crate::package_resolver::{DiscoveryPolicy, apply_discovery_option};
use crate::planner::{ManifestRegistry, PlannedOverlay, plan_manifest_target};
use crate::workspace_mutation::{PlannedWorkspaceFile, commit_workspace_files, recover_workspace};

const TRANSLATION_JSON_SCHEMA_VERSION: u32 = 1;

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        print!(
            "{}",
            help::command("translations").expect("translations help exists")
        );
        return Ok(());
    }

    let mut args = parse_translation_args(args)?;
    let interactive = should_use_interactive(&args);
    if !args.manifest_path.exists() && !interactive {
        return Err(missing_manifest_error(
            &args.manifest_path,
            &args.discovery_policy,
        )?);
    }

    if interactive {
        let raw_mode = io::stdin().is_terminal() && io::stdout().is_terminal();
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut writer = stdout.lock();
        configure_interactively(&mut args, &mut reader, &mut writer, raw_mode)?;
    }

    if args.apply || args.resolve_action.is_some() {
        recover_workspace(&manifest_root(&args.manifest_path))?;
    }

    let reports = collect_translation_reports(&args)?;

    if args.resolve_action.is_some() {
        let applied = resolve_stale_translations(&args, &reports)?;
        if args.json_output {
            print_translation_json("stale_resolution", json!({ "resolved": applied }));
        } else {
            let total = applied.values().sum::<usize>();
            let noun = if total == 1 {
                "translation"
            } else {
                "translations"
            };
            let details = applied
                .iter()
                .map(|(file, count)| (file.as_str(), count.to_string()))
                .collect::<Vec<_>>();
            output::print_success(format!("resolved {total} stale {noun}"), &details);
        }
        return Ok(());
    }

    let mut edits_by_file = BTreeMap::<PathBuf, OverlayEdits>::new();
    if args.apply {
        if interactive {
            let raw_mode = io::stdin().is_terminal() && io::stdout().is_terminal();
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut reader = stdin.lock();
            let mut writer = stdout.lock();
            edits_by_file =
                prompt_selective_apply(&reports, &mut reader, &mut writer, raw_mode, args.full)?;
        } else {
            edits_by_file = direct_stub_edits_from_reports(&reports, args.full);
        }
    }

    let planned = edits_by_file
        .iter()
        .map(|(path, edits)| plan_overlay_edits(path, edits))
        .collect::<Result<Vec<_>, String>>()?;
    let applied = commit_translation_documents(&args, planned)?;

    if args.context {
        if args.json_output {
            print_json_contexts(&reports);
        } else if reports.is_empty() {
            print_no_translation_reports(&args)?;
        } else {
            print_human_contexts(&reports, args.full);
        }
    } else if args.summary {
        if args.json_output {
            print_json_summary(&reports);
        } else if reports.is_empty() {
            print_no_translation_reports(&args)?;
        } else {
            print_human_summary(&reports, args.full);
        }
    } else if args.json_output {
        print_json_reports(&reports, &applied);
    } else {
        if reports.is_empty() {
            print_no_translation_reports(&args)?;
        } else {
            print_human_reports(&reports, args.full);
        }
        if args.apply {
            let total = applied.values().sum::<usize>();
            let details = applied
                .iter()
                .map(|(file, count)| (file.as_str(), count.to_string()))
                .collect::<Vec<_>>();
            output::print_success(format!("applied {total} translation edit(s)"), &details);
        }
    }

    Ok(())
}

#[derive(Clone)]
struct TranslationArgs {
    manifest_path: PathBuf,
    target: Option<String>,
    all_targets: bool,
    include_paths: Vec<PathBuf>,
    package_roots: Vec<PathBuf>,
    discovery_policy: DiscoveryPolicy,
    language: Option<String>,
    overlay: Option<String>,
    note: Option<String>,
    field: Option<String>,
    path_prefixes: Vec<String>,
    apply: bool,
    json_output: bool,
    interactive: Option<bool>,
    full: bool,
    summary: bool,
    context: bool,
    source: Option<String>,
    duplicates: bool,
    status: Option<String>,
    resolve_action: Option<StaleResolveAction>,
    old_source: Option<String>,
    new_source: Option<String>,
    stale_context: Option<String>,
    replacement_translation: Option<String>,
}

fn parse_translation_args(args: &[String]) -> Result<TranslationArgs, String> {
    let mut parsed = TranslationArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        target: None,
        all_targets: false,
        include_paths: Vec::new(),
        package_roots: Vec::new(),
        discovery_policy: DiscoveryPolicy::default(),
        language: None,
        overlay: None,
        note: None,
        field: None,
        path_prefixes: Vec::new(),
        apply: false,
        json_output: false,
        interactive: None,
        full: false,
        summary: false,
        context: false,
        source: None,
        duplicates: false,
        status: None,
        resolve_action: None,
        old_source: None,
        new_source: None,
        stale_context: None,
        replacement_translation: None,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--manifest requires a path".to_owned());
                };
                parsed.manifest_path = PathBuf::from(path);
                index += 2;
            }
            "--target" => {
                let Some(name) = args.get(index + 1) else {
                    return Err("--target requires a name".to_owned());
                };
                parsed.target = Some(name.clone());
                index += 2;
            }
            "--all-targets" => {
                parsed.all_targets = true;
                index += 1;
            }
            "--include" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--include requires a path".to_owned());
                };
                parsed.include_paths.push(PathBuf::from(path));
                index += 2;
            }
            "--package-root" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--package-root requires a path".to_owned());
                };
                parsed.package_roots.push(PathBuf::from(path));
                index += 2;
            }
            flag @ ("--discovery-max-depth"
            | "--discovery-max-entries"
            | "--discovery-max-manifests"
            | "--package-ignore") => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                apply_discovery_option(flag, value, &mut parsed.discovery_policy)?;
                index += 2;
            }
            "--language" => {
                let Some(language) = args.get(index + 1) else {
                    return Err("--language requires a code".to_owned());
                };
                parsed.language = Some(language.clone());
                index += 2;
            }
            "--overlay" => {
                let Some(overlay) = args.get(index + 1) else {
                    return Err("--overlay requires an id or file fragment".to_owned());
                };
                parsed.overlay = Some(overlay.clone());
                index += 2;
            }
            "--note" => {
                let Some(note) = args.get(index + 1) else {
                    return Err("--note requires a note id".to_owned());
                };
                parsed.note = Some(note.clone());
                index += 2;
            }
            "--field" => {
                let Some(field) = args.get(index + 1) else {
                    return Err("--field requires a field id".to_owned());
                };
                parsed.field = Some(field.clone());
                index += 2;
            }
            "--path-prefix" => {
                let Some(prefix) = args.get(index + 1) else {
                    return Err("--path-prefix requires a deck path prefix".to_owned());
                };
                parsed.path_prefixes.push(prefix.clone());
                index += 2;
            }
            "--apply" => {
                parsed.apply = true;
                index += 1;
            }
            "--json" => {
                parsed.json_output = true;
                index += 1;
            }
            "--full" => {
                parsed.full = true;
                index += 1;
            }
            "--summary" => {
                parsed.summary = true;
                index += 1;
            }
            "--context" => {
                parsed.context = true;
                index += 1;
            }
            "--source" => {
                let Some(source) = args.get(index + 1) else {
                    return Err("--source requires text to match".to_owned());
                };
                parsed.source = Some(source.clone());
                index += 2;
            }
            "--duplicates" => {
                parsed.duplicates = true;
                index += 1;
            }
            "--status" => {
                let Some(status) = args.get(index + 1) else {
                    return Err("--status requires a status filter".to_owned());
                };
                validate_status_filter(status)?;
                parsed.status = Some(status.clone());
                index += 2;
            }
            "--resolve" => {
                let Some(action) = args.get(index + 1) else {
                    return Err("--resolve requires confirm or replace".to_owned());
                };
                parsed.resolve_action = Some(parse_stale_resolve_action(action)?);
                index += 2;
            }
            "--old-source" => {
                let Some(source) = args.get(index + 1) else {
                    return Err("--old-source requires source text".to_owned());
                };
                parsed.old_source = Some(source.clone());
                index += 2;
            }
            "--new-source" => {
                let Some(source) = args.get(index + 1) else {
                    return Err("--new-source requires source text".to_owned());
                };
                parsed.new_source = Some(source.clone());
                index += 2;
            }
            "--stale-context" => {
                let Some(context) = args.get(index + 1) else {
                    return Err("--stale-context requires a dictionary context path".to_owned());
                };
                parsed.stale_context = Some(context.clone());
                index += 2;
            }
            "--translation" => {
                let Some(translation) = args.get(index + 1) else {
                    return Err("--translation requires translated text".to_owned());
                };
                parsed.replacement_translation = Some(translation.clone());
                index += 2;
            }
            "--interactive" => {
                parsed.interactive = Some(true);
                index += 1;
            }
            "--no-interactive" => {
                parsed.interactive = Some(false);
                index += 1;
            }
            other => return Err(format!("unexpected translations argument {other:?}")),
        }
    }
    if parsed.all_targets && parsed.target.is_some() {
        return Err("choose --all-targets or --target, not both".to_owned());
    }
    if parsed.interactive == Some(true) && parsed.json_output {
        return Err("choose --interactive or --json, not both".to_owned());
    }
    if parsed.summary && parsed.apply {
        return Err("choose --summary or --apply, not both".to_owned());
    }
    if parsed.summary && parsed.context {
        return Err("choose --summary or --context, not both".to_owned());
    }
    if parsed.resolve_action.is_some() {
        if parsed.apply {
            return Err("choose --resolve or --apply, not both".to_owned());
        }
        if parsed.summary || parsed.context {
            return Err("choose --resolve or reporting views, not both".to_owned());
        }
        if parsed.resolve_action == Some(StaleResolveAction::Replace)
            && parsed.replacement_translation.is_none()
        {
            return Err("--resolve replace requires --translation".to_owned());
        }
        if parsed.resolve_action == Some(StaleResolveAction::Confirm)
            && parsed.replacement_translation.is_some()
        {
            return Err("--translation is only valid with --resolve replace".to_owned());
        }
    } else if parsed.old_source.is_some()
        || parsed.new_source.is_some()
        || parsed.stale_context.is_some()
        || parsed.replacement_translation.is_some()
    {
        return Err(
            "--old-source, --new-source, --stale-context, and --translation require --resolve"
                .to_owned(),
        );
    }
    Ok(parsed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaleResolveAction {
    Confirm,
    Replace,
}

fn parse_stale_resolve_action(value: &str) -> Result<StaleResolveAction, String> {
    match value {
        "confirm" => Ok(StaleResolveAction::Confirm),
        "replace" => Ok(StaleResolveAction::Replace),
        other => Err(format!(
            "invalid stale translation resolve action {other:?}; expected confirm or replace"
        )),
    }
}

fn should_use_interactive(args: &TranslationArgs) -> bool {
    match args.interactive {
        Some(true) => !args.json_output,
        Some(false) => false,
        None => {
            !args.json_output
                && io::stdin().is_terminal()
                && io::stdout().is_terminal()
                && (!args.manifest_path.exists() || (args.target.is_none() && !args.all_targets))
        }
    }
}

fn configure_interactively<R: Read, W: Write>(
    args: &mut TranslationArgs,
    reader: &mut R,
    writer: &mut W,
    raw_mode: bool,
) -> Result<(), String> {
    let mut ui = TerminalUi::new(reader, writer, raw_mode)?;
    ui.message("Brain Brew translation coverage")?;

    if !args.manifest_path.exists() {
        let manifests = discover_nearby_manifests(&args.discovery_policy)?;
        if manifests.is_empty() {
            return Err(missing_manifest_error(
                &args.manifest_path,
                &args.discovery_policy,
            )?);
        }
        let labels = manifests
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let choice = ui.select_one("Manifest", &labels, 0)?;
        args.manifest_path = manifests[choice].clone();
    }

    let manifest = read_manifest(&args.manifest_path)?;
    if args.target.is_none() && !args.all_targets {
        let mut target_values = manifest.targets.keys().cloned().collect::<Vec<_>>();
        target_values.sort();
        let mut labels = target_values.clone();
        labels.push("all targets".to_owned());
        let choice = ui.select_one("Target", &labels, 0)?;
        if choice == target_values.len() {
            args.all_targets = true;
        } else {
            args.target = Some(target_values[choice].clone());
        }
    }

    if args.language.is_none() && args.all_targets {
        let languages = inferred_languages(&manifest);
        if !languages.is_empty() {
            let mut labels = vec!["all languages".to_owned()];
            labels.extend(languages.iter().cloned());
            let choice = ui.select_one("Language filter", &labels, 0)?;
            if choice > 0 {
                args.language = Some(languages[choice - 1].clone());
            }
        }
    }

    if args.overlay.is_none() {
        let overlays = translation_overlay_choices(args)?;
        if overlays.len() > 1 {
            let mut labels = vec!["all translation overlays".to_owned()];
            labels.extend(overlays.iter().map(|overlay| overlay.label.clone()));
            let choice = ui.select_one("Translation overlay", &labels, 0)?;
            if choice > 0 {
                args.overlay = Some(overlays[choice - 1].id.clone());
            }
        }
    }

    if args.note.is_none() && args.field.is_none() && args.path_prefixes.is_empty() {
        let reports = collect_translation_reports(args)?;
        configure_scope_interactively(args, &reports, &mut ui)?;
    }

    if !args.apply {
        let labels = vec![
            "report only".to_owned(),
            "apply selected source→source stubs".to_owned(),
        ];
        let choice = ui.select_one("Mode", &labels, 0)?;
        if choice == 1 {
            args.apply = true;
        }
    }

    ui.finish_with_equivalent_command(args)
}

fn configure_scope_interactively<R: Read, W: Write>(
    args: &mut TranslationArgs,
    reports: &[ScopedTranslationReport],
    ui: &mut TerminalUi<'_, R, W>,
) -> Result<(), String> {
    let mut notes = BTreeSet::new();
    let mut fields = BTreeSet::new();
    let mut problem_paths = BTreeSet::new();
    for report in reports {
        for entry in &report.report.entries {
            if let Some(note) = note_id_from_path(&entry.path) {
                notes.insert(note);
            }
            if let Some(field) = field_id_from_path(&entry.path) {
                fields.insert(field);
            }
            if entry.category.is_problem() {
                problem_paths.insert(entry.path.clone());
            }
        }
    }

    let labels = vec![
        "whole overlay".to_owned(),
        "select note".to_owned(),
        "select field".to_owned(),
        "select path prefix".to_owned(),
        "changed-path prefixes from diff".to_owned(),
    ];
    let choice = ui.select_one("Scope", &labels, 0)?;
    match choice {
        0 => {}
        1 => {
            let note_labels = notes.into_iter().collect::<Vec<_>>();
            if note_labels.is_empty() {
                ui.message("No note paths were found; using whole overlay.")?;
            } else {
                let choice = ui.select_one("Note", &note_labels, 0)?;
                args.note = Some(note_labels[choice].clone());
            }
        }
        2 => {
            let field_labels = fields.into_iter().collect::<Vec<_>>();
            if field_labels.is_empty() {
                ui.message("No field paths were found; using whole overlay.")?;
            } else {
                let choice = ui.select_one("Field", &field_labels, 0)?;
                args.field = Some(field_labels[choice].clone());
            }
        }
        3 => {
            let mut body = Vec::new();
            if !problem_paths.is_empty() {
                body.push("Known problem paths:".to_owned());
                for path in problem_paths.iter().take(12) {
                    body.push(format!("  - {path}"));
                }
            }
            let prefix = ui.prompt_text("Path prefix", &body)?;
            if !prefix.trim().is_empty() {
                args.path_prefixes.push(prefix.trim().to_owned());
            }
        }
        4 => {
            let prefixes = ui.prompt_text(
                "Changed deck path prefixes",
                &["Paste changed deck path prefixes, comma-separated.".to_owned()],
            )?;
            args.path_prefixes.extend(
                prefixes
                    .split(',')
                    .map(str::trim)
                    .filter(|prefix| !prefix.is_empty())
                    .map(str::to_owned),
            );
        }
        _ => unreachable!(),
    }
    Ok(())
}

struct TerminalUi<'a, R: Read, W: Write> {
    reader: &'a mut R,
    writer: &'a mut W,
    raw_mode: bool,
    max_option_rows: Option<usize>,
    terminal_cols: Option<usize>,
}

impl<'a, R: Read, W: Write> TerminalUi<'a, R, W> {
    fn new(reader: &'a mut R, writer: &'a mut W, raw_mode: bool) -> Result<Self, String> {
        let (terminal_cols, max_option_rows) = if raw_mode {
            enable_raw_mode().map_err(|error| error.to_string())?;
            write!(writer, "\x1b[?25l").map_err(|error| error.to_string())?;
            match crossterm::terminal::size() {
                Ok((cols, rows)) => (
                    Some(cols as usize),
                    Some(usize::from(rows.saturating_sub(6).max(3))),
                ),
                Err(_) => (None, Some(12)),
            }
        } else {
            (None, None)
        };
        Ok(Self {
            reader,
            writer,
            raw_mode,
            max_option_rows,
            terminal_cols,
        })
    }

    fn message(&mut self, message: &str) -> Result<(), String> {
        self.clear_if_raw()?;
        self.write_line(&color_stdout(message, "1;36"))?;
        self.blank_line()?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    fn finish_with_equivalent_command(&mut self, args: &TranslationArgs) -> Result<(), String> {
        self.clear_if_raw()?;
        self.write_line("Equivalent command:")?;
        self.write_line(&format!("  {}", equivalent_command(args)))?;
        self.blank_line()?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    fn select_one(
        &mut self,
        label: &str,
        options: &[String],
        default_index: usize,
    ) -> Result<usize, String> {
        if options.is_empty() {
            return Err(format!("no {label} options are available"));
        }
        let mut selected = default_index.min(options.len() - 1);
        loop {
            self.render_select_one(label, options, selected)?;
            match self.read_key()? {
                TuiKey::Up => selected = selected.saturating_sub(1),
                TuiKey::Down => selected = (selected + 1).min(options.len() - 1),
                TuiKey::PageUp => {
                    selected = selected.saturating_sub(self.visible_option_rows(options.len()))
                }
                TuiKey::PageDown => {
                    selected =
                        (selected + self.visible_option_rows(options.len())).min(options.len() - 1)
                }
                TuiKey::Home => selected = 0,
                TuiKey::End => selected = options.len() - 1,
                TuiKey::Enter => return Ok(selected),
                TuiKey::Char('k') => selected = selected.saturating_sub(1),
                TuiKey::Char('j') => selected = (selected + 1).min(options.len() - 1),
                TuiKey::Char('q') | TuiKey::Esc => {
                    return Err("interactive selection cancelled".to_owned());
                }
                _ => {}
            }
        }
    }

    fn select_many(
        &mut self,
        label: &str,
        options: &[String],
        selected_by_default: bool,
    ) -> Result<Vec<usize>, String> {
        if options.is_empty() {
            return Ok(Vec::new());
        }
        let mut cursor = 0;
        let mut selected = vec![selected_by_default; options.len()];
        loop {
            self.render_select_many(label, options, &selected, cursor)?;
            match self.read_key()? {
                TuiKey::Up => cursor = cursor.saturating_sub(1),
                TuiKey::Down => cursor = (cursor + 1).min(options.len() - 1),
                TuiKey::PageUp => {
                    cursor = cursor.saturating_sub(self.visible_option_rows(options.len()))
                }
                TuiKey::PageDown => {
                    cursor =
                        (cursor + self.visible_option_rows(options.len())).min(options.len() - 1)
                }
                TuiKey::Home => cursor = 0,
                TuiKey::End => cursor = options.len() - 1,
                TuiKey::Char('k') => cursor = cursor.saturating_sub(1),
                TuiKey::Char('j') => cursor = (cursor + 1).min(options.len() - 1),
                TuiKey::Space => selected[cursor] = !selected[cursor],
                TuiKey::Char('a') => {
                    let all_selected = selected.iter().all(|value| *value);
                    selected.fill(!all_selected);
                }
                TuiKey::Enter => {
                    return Ok(selected
                        .iter()
                        .enumerate()
                        .filter_map(|(index, selected)| selected.then_some(index))
                        .collect());
                }
                TuiKey::Char('q') | TuiKey::Esc => {
                    return Err("interactive selection cancelled".to_owned());
                }
                _ => {}
            }
        }
    }

    fn prompt_text(&mut self, label: &str, body: &[String]) -> Result<String, String> {
        let mut input = String::new();
        loop {
            self.render_text_prompt(label, body, &input)?;
            match self.read_key()? {
                TuiKey::Enter => return Ok(input),
                TuiKey::Backspace => {
                    input.pop();
                }
                TuiKey::Char(ch) => input.push(ch),
                TuiKey::Esc => return Err("interactive text entry cancelled".to_owned()),
                _ => {}
            }
        }
    }

    fn render_select_one(
        &mut self,
        label: &str,
        options: &[String],
        selected: usize,
    ) -> Result<(), String> {
        self.clear_if_raw()?;
        self.write_line(&color_stdout(label, "1;36"))?;
        self.blank_line()?;
        let (start, end, scrolled) = self.option_window(options.len(), selected);
        if scrolled {
            self.write_line(&format!(
                "Showing {}–{} of {}",
                start + 1,
                end,
                options.len()
            ))?;
            self.blank_line()?;
        }
        for (relative_index, option) in options[start..end].iter().enumerate() {
            let index = start + relative_index;
            let marker = if index == selected { "›" } else { " " };
            let option = self.truncate_option(option, 4);
            let option = if index == selected {
                color_stdout(&option, "1;32")
            } else {
                option
            };
            self.write_line(&format!("  {marker} {option}"))?;
        }
        self.blank_line()?;
        self.write_line("↑/↓ move • PgUp/PgDn jump • Enter select • q cancel")?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    fn render_select_many(
        &mut self,
        label: &str,
        options: &[String],
        selected: &[bool],
        cursor: usize,
    ) -> Result<(), String> {
        self.clear_if_raw()?;
        self.write_line(&color_stdout(label, "1;36"))?;
        self.blank_line()?;
        let (start, end, scrolled) = self.option_window(options.len(), cursor);
        if scrolled {
            self.write_line(&format!(
                "Showing {}–{} of {}",
                start + 1,
                end,
                options.len()
            ))?;
            self.blank_line()?;
        }
        for (relative_index, option) in options[start..end].iter().enumerate() {
            let index = start + relative_index;
            let cursor_marker = if index == cursor { "›" } else { " " };
            let selected_marker = if selected[index] { "[x]" } else { "[ ]" };
            let option = self.truncate_option(option, 8);
            let line = format!("{selected_marker} {option}");
            let line = if index == cursor {
                color_stdout(&line, "1;32")
            } else {
                line
            };
            self.write_line(&format!("  {cursor_marker} {line}"))?;
        }
        self.blank_line()?;
        self.write_line(
            "↑/↓ move • PgUp/PgDn jump • Space toggle • a toggle all • Enter confirm • q cancel",
        )?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    fn render_text_prompt(
        &mut self,
        label: &str,
        body: &[String],
        input: &str,
    ) -> Result<(), String> {
        self.clear_if_raw()?;
        self.write_line(&color_stdout(label, "1;36"))?;
        self.blank_line()?;
        for line in body {
            self.write_line(line)?;
        }
        if !body.is_empty() {
            self.blank_line()?;
        }
        self.write_line(&format!("> {input}"))?;
        self.blank_line()?;
        self.write_line("Type text • Enter confirm • Esc cancel")?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    fn option_window(&self, option_count: usize, cursor: usize) -> (usize, usize, bool) {
        if option_count == 0 {
            return (0, 0, false);
        }
        let visible = self
            .visible_option_rows(option_count)
            .min(option_count)
            .max(1);
        if visible >= option_count {
            return (0, option_count, false);
        }
        let half = visible / 2;
        let mut start = cursor.saturating_sub(half);
        if start + visible > option_count {
            start = option_count - visible;
        }
        (start, start + visible, true)
    }

    fn visible_option_rows(&self, option_count: usize) -> usize {
        self.max_option_rows
            .unwrap_or(option_count)
            .min(option_count)
            .max(1)
    }

    fn truncate_option(&self, option: &str, prefix_width: usize) -> String {
        let Some(cols) = self.terminal_cols else {
            return option.to_owned();
        };
        let width = cols.saturating_sub(prefix_width).max(1);
        truncate_chars(option, width)
    }

    fn clear_if_raw(&mut self) -> Result<(), String> {
        if self.raw_mode {
            self.clear()?;
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), String> {
        write!(self.writer, "\x1b[2J\x1b[H").map_err(|error| error.to_string())
    }

    fn write_line(&mut self, text: &str) -> Result<(), String> {
        write!(self.writer, "{}{}", text, terminal_line_end(self.raw_mode))
            .map_err(|error| error.to_string())
    }

    fn blank_line(&mut self) -> Result<(), String> {
        write!(self.writer, "{}", terminal_line_end(self.raw_mode))
            .map_err(|error| error.to_string())
    }

    fn read_key(&mut self) -> Result<TuiKey, String> {
        if self.raw_mode {
            return read_terminal_key();
        }
        read_scripted_key(self.reader)
    }
}

fn terminal_line_end(raw_mode: bool) -> &'static str {
    if raw_mode { "\r\n" } else { "\n" }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_owned();
    }
    if max_chars <= 1 {
        return "…".to_owned();
    }
    value.chars().take(max_chars - 1).chain(['…']).collect()
}

impl<R: Read, W: Write> Drop for TerminalUi<'_, R, W> {
    fn drop(&mut self) {
        if self.raw_mode {
            let _ = write!(self.writer, "\x1b[?25h");
            let _ = self.writer.flush();
            let _ = disable_raw_mode();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TuiKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Space,
    Backspace,
    Esc,
    Char(char),
    Other,
}

fn read_terminal_key() -> Result<TuiKey, String> {
    loop {
        let event = crossterm::event::read().map_err(|error| error.to_string())?;
        let crossterm::event::Event::Key(key) = event else {
            continue;
        };
        use crossterm::event::KeyCode;
        return Ok(match key.code {
            KeyCode::Up => TuiKey::Up,
            KeyCode::Down => TuiKey::Down,
            KeyCode::PageUp => TuiKey::PageUp,
            KeyCode::PageDown => TuiKey::PageDown,
            KeyCode::Home => TuiKey::Home,
            KeyCode::End => TuiKey::End,
            KeyCode::Enter => TuiKey::Enter,
            KeyCode::Char(' ') => TuiKey::Space,
            KeyCode::Char(ch) => TuiKey::Char(ch),
            KeyCode::Backspace => TuiKey::Backspace,
            KeyCode::Esc => TuiKey::Esc,
            _ => TuiKey::Other,
        });
    }
}

fn read_scripted_key<R: Read>(reader: &mut R) -> Result<TuiKey, String> {
    let mut byte = [0_u8; 1];
    let count = reader.read(&mut byte).map_err(|error| error.to_string())?;
    if count == 0 {
        return Err("interactive input ended before a selection was made".to_owned());
    }
    match byte[0] {
        b'\n' | b'\r' => Ok(TuiKey::Enter),
        b' ' => Ok(TuiKey::Space),
        8 | 127 => Ok(TuiKey::Backspace),
        27 => read_escape_sequence(reader),
        byte if byte.is_ascii() && !byte.is_ascii_control() => Ok(TuiKey::Char(byte as char)),
        _ => Ok(TuiKey::Other),
    }
}

fn read_escape_sequence<R: Read>(reader: &mut R) -> Result<TuiKey, String> {
    let mut rest = [0_u8; 2];
    let count = reader.read(&mut rest).map_err(|error| error.to_string())?;
    if count < 2 || rest[0] != b'[' {
        return Ok(TuiKey::Esc);
    }
    Ok(match rest[1] {
        b'A' => TuiKey::Up,
        b'B' => TuiKey::Down,
        _ => TuiKey::Other,
    })
}

#[derive(Clone)]
struct ScopedTranslationReport {
    target: String,
    overlay_id: String,
    overlay_file: String,
    overlay_path: PathBuf,
    report: TranslationCoverageReport,
    context: TranslationContextView,
}

fn collect_translation_reports(
    args: &TranslationArgs,
) -> Result<Vec<ScopedTranslationReport>, String> {
    let manifest = read_manifest(&args.manifest_path)?;
    let target_names = selected_target_names(&manifest, args);

    let mut reports = Vec::new();
    for target in &target_names {
        let plan = plan_manifest_target(
            &args.manifest_path,
            target,
            &args.include_paths,
            &args.package_roots,
            &args.discovery_policy,
        )?;
        let mut current = plan.base.clone();
        for (planned, overlay) in &plan.overlays {
            if overlay.translations.is_some() {
                if overlay_matches_scope(target, planned, overlay, args) {
                    let report = current
                        .translation_coverage(overlay)
                        .map_err(|error| format!("failed to resolve translation source fields for target {target}: {error}"))?;
                    let full_context = current.translation_context(&report).map_err(|error| {
                        format!("failed to build translation context for target {target}: {error}")
                    })?;
                    let mut entries = report
                        .entries
                        .iter()
                        .filter(|entry| entry_matches_scope(entry, args))
                        .cloned()
                        .collect::<Vec<_>>();
                    if args.duplicates {
                        retain_duplicate_source_entries(&mut entries);
                    }
                    let scoped_report = TranslationCoverageReport {
                        overlay_id: report.overlay_id,
                        entries,
                    };
                    let context = filter_context_to_report(full_context, &scoped_report);
                    reports.push(ScopedTranslationReport {
                        target: target.clone(),
                        overlay_id: planned.id.clone(),
                        overlay_file: planned.display_file.clone(),
                        overlay_path: planned.file.clone(),
                        report: scoped_report,
                        context,
                    });
                }
                current = compose_lenient_translation_overlay(&current, overlay)?;
            } else {
                current = current
                    .compose(std::slice::from_ref(overlay))
                    .map_err(|report| {
                        output::compose_error(
                            "translations",
                            json!({"target": target, "overlay": planned.id}),
                            &report,
                        )
                    })?;
            }
        }
    }
    Ok(reports)
}

fn selected_target_names(manifest: &FederatedDeckManifest, args: &TranslationArgs) -> Vec<String> {
    if args.all_targets || args.target.is_none() {
        manifest.targets.keys().cloned().collect::<Vec<_>>()
    } else {
        vec![args.target.clone().expect("target exists")]
    }
}

#[derive(Clone, Debug, Default)]
struct OverlayEdits {
    direct: BTreeSet<String>,
    contextual: BTreeMap<String, BTreeSet<String>>,
    no_change: BTreeSet<String>,
    ignore_paths: BTreeSet<String>,
}

impl OverlayEdits {
    fn is_empty(&self) -> bool {
        self.direct.is_empty()
            && self.contextual.is_empty()
            && self.no_change.is_empty()
            && self.ignore_paths.is_empty()
    }

    fn count(&self) -> usize {
        self.direct.len()
            + self.contextual.values().map(BTreeSet::len).sum::<usize>()
            + self.no_change.len()
            + self.ignore_paths.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct StaleResolution {
    old_source: String,
    new_source: String,
    context: Option<String>,
    replacement: Option<String>,
}

fn resolve_stale_translations(
    args: &TranslationArgs,
    reports: &[ScopedTranslationReport],
) -> Result<BTreeMap<String, usize>, String> {
    let action = args
        .resolve_action
        .expect("resolve action checked before resolving stale translations");
    let mut resolutions_by_file = BTreeMap::<PathBuf, BTreeSet<StaleResolution>>::new();
    let mut stale_record_matched_filter = false;
    let mut non_stale_source_match = None::<String>;

    for report in reports {
        let shadowing_entries = report
            .report
            .entries
            .iter()
            .filter(|entry| entry_can_shadow_stale_translation(entry))
            .collect::<Vec<_>>();
        for entry in &report.report.entries {
            if !entry_matches_stale_resolution_filter(entry, args) {
                if entry.category != TranslationCoverageCategory::StaleTranslation
                    && args
                        .new_source
                        .as_ref()
                        .is_some_and(|source| source == &entry.source)
                    && non_stale_source_match.is_none()
                {
                    non_stale_source_match = Some(entry.source.clone());
                }
                continue;
            }

            if entry.category != TranslationCoverageCategory::StaleTranslation {
                non_stale_source_match.get_or_insert_with(|| entry.source.clone());
                continue;
            }

            stale_record_matched_filter = true;
            let shadowed = shadowing_entries
                .iter()
                .any(|candidate| entry_shadows_stale_record(candidate, entry));
            if entry.path.starts_with("translations.stale_translations.") && !shadowed {
                continue;
            }

            let old_source = entry.old_source.clone().ok_or_else(|| {
                format!(
                    "stale translation at {} is missing old_source; refusing to resolve",
                    entry.path
                )
            })?;
            resolutions_by_file
                .entry(report.overlay_path.clone())
                .or_default()
                .insert(StaleResolution {
                    old_source,
                    new_source: entry.source.clone(),
                    context: entry.context.clone(),
                    replacement: args.replacement_translation.clone(),
                });
        }
    }

    if resolutions_by_file.is_empty() {
        if stale_record_matched_filter {
            return Err(stale_resolution_mismatched_current_source_error(args));
        }
        if let Some(source) = non_stale_source_match {
            return Err(format!(
                "source {source:?} is not stale; refusing to resolve"
            ));
        }
        return Err(stale_resolution_no_match_error(args));
    }

    let planned = resolutions_by_file
        .into_iter()
        .map(|(path, resolutions)| plan_stale_resolutions(&path, action, &resolutions))
        .collect::<Result<Vec<_>, String>>()?;
    commit_translation_documents(args, planned)
}

fn entry_can_shadow_stale_translation(entry: &TranslationCoverageEntry) -> bool {
    matches!(
        entry.category,
        TranslationCoverageCategory::DirectTranslation
            | TranslationCoverageCategory::ContextualTranslation
            | TranslationCoverageCategory::NoChange
    )
}

fn entry_shadows_stale_record(
    candidate: &TranslationCoverageEntry,
    stale: &TranslationCoverageEntry,
) -> bool {
    candidate.source == stale.source
        && match candidate.category {
            TranslationCoverageCategory::DirectTranslation
            | TranslationCoverageCategory::NoChange => true,
            TranslationCoverageCategory::ContextualTranslation => stale
                .context
                .as_deref()
                .is_none_or(|context| context_matches_path(context, &candidate.path)),
            _ => false,
        }
}

fn context_matches_path(context_path: &str, path: &str) -> bool {
    path == context_path
        || path
            .strip_prefix(context_path)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn entry_matches_stale_resolution_filter(
    entry: &TranslationCoverageEntry,
    args: &TranslationArgs,
) -> bool {
    if let Some(old_source) = &args.old_source
        && entry.old_source.as_deref() != Some(old_source.as_str())
    {
        return false;
    }
    if let Some(new_source) = &args.new_source
        && entry.source != *new_source
    {
        return false;
    }
    if let Some(context) = &args.stale_context
        && entry.context.as_deref() != Some(context.as_str())
    {
        return false;
    }
    true
}

fn stale_resolution_mismatched_current_source_error(args: &TranslationArgs) -> String {
    let mut parts = Vec::new();
    if let Some(old_source) = &args.old_source {
        parts.push(format!("old_source={old_source:?}"));
    }
    if let Some(new_source) = &args.new_source {
        parts.push(format!("new_source={new_source:?}"));
    }
    if let Some(context) = &args.stale_context {
        parts.push(format!("context={context:?}"));
    }
    if parts.is_empty() {
        "matched stale translation record does not match the current base text; refusing to resolve"
            .to_owned()
    } else {
        format!(
            "stale translation {} does not match the current base text; refusing to resolve",
            parts.join(" ")
        )
    }
}

fn stale_resolution_no_match_error(args: &TranslationArgs) -> String {
    if let Some(new_source) = &args.new_source {
        format!("no stale translation matched new_source={new_source:?}")
    } else {
        "no stale translations matched the selected target and scope".to_owned()
    }
}

fn plan_stale_resolutions(
    path: &Path,
    action: StaleResolveAction,
    resolutions: &BTreeSet<StaleResolution>,
) -> Result<PlannedTranslationDocument, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut document = overlay_source_document(path, &input)?;
    for resolution in resolutions {
        let replacement = (action == StaleResolveAction::Replace).then(|| {
            resolution
                .replacement
                .as_deref()
                .expect("replace action requires replacement translation")
        });
        document
            .resolve_stale_translation(
                &resolution.old_source,
                &resolution.new_source,
                resolution.context.as_deref(),
                replacement,
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(PlannedTranslationDocument {
        path: path.to_path_buf(),
        input,
        document,
        count: resolutions.len(),
    })
}

fn direct_stub_edits_from_reports(
    reports: &[ScopedTranslationReport],
    full: bool,
) -> BTreeMap<PathBuf, OverlayEdits> {
    let mut edits = BTreeMap::<PathBuf, OverlayEdits>::new();
    for report in reports {
        let stubs = report
            .report
            .entries
            .iter()
            .filter(|entry| {
                entry.category == TranslationCoverageCategory::UntranslatedFallback
                    && (full || default_report_includes_entry(entry))
            })
            .map(|entry| entry.source.clone())
            .collect::<BTreeSet<_>>();
        if !stubs.is_empty() {
            edits
                .entry(report.overlay_path.clone())
                .or_default()
                .direct
                .extend(stubs);
        }
    }
    edits
}

fn prompt_selective_apply<R: Read, W: Write>(
    reports: &[ScopedTranslationReport],
    reader: &mut R,
    writer: &mut W,
    raw_mode: bool,
    full: bool,
) -> Result<BTreeMap<PathBuf, OverlayEdits>, String> {
    let mut ui = TerminalUi::new(reader, writer, raw_mode)?;
    let rows = reports
        .iter()
        .enumerate()
        .flat_map(|(report_index, report)| {
            report
                .report
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.category.is_problem()
                        && (entry.category != TranslationCoverageCategory::UntranslatedFallback
                            || full
                            || default_report_includes_entry(entry))
                })
                .map(move |(entry_index, _)| (report_index, entry_index))
        })
        .collect::<Vec<_>>();

    if rows.is_empty() {
        ui.message("No missing, stale, or invalid translation rows to apply.")?;
        return Ok(BTreeMap::new());
    }

    let row_labels = rows
        .iter()
        .map(|(report_index, entry_index)| {
            let report = &reports[*report_index];
            let entry = &report.report.entries[*entry_index];
            format!(
                "{} {} {} source={} — {}",
                report.target,
                report.overlay_id,
                entry.category.as_str(),
                yaml_scalar(&entry.source),
                entry.path
            )
        })
        .collect::<Vec<_>>();
    let selected = ui.select_many("Rows to edit", &row_labels, true)?;

    let selected_missing = selected
        .iter()
        .copied()
        .filter(|row_index| {
            let (_, entry_index) = rows[*row_index];
            reports[rows[*row_index].0].report.entries[entry_index].category
                == TranslationCoverageCategory::UntranslatedFallback
        })
        .collect::<Vec<_>>();

    let bulk_action = if selected_missing.is_empty() {
        MissingApplyAction::Skip
    } else {
        let labels = vec![
            format!(
                "mark no-change for all {} selected missing translations",
                selected_missing.len()
            ),
            format!(
                "add direct source→source translation stubs for all {} selected missing translations",
                selected_missing.len()
            ),
            format!(
                "add contextual source→source translation stubs for all {} selected missing translations",
                selected_missing.len()
            ),
            format!(
                "add ignore_paths entries for all {} selected missing translations",
                selected_missing.len()
            ),
            "decide per row".to_owned(),
            "skip selected missing translations".to_owned(),
        ];
        match ui.select_one("Action for selected missing translations", &labels, 0)? {
            0 => MissingApplyAction::NoChange,
            1 => MissingApplyAction::Direct,
            2 => MissingApplyAction::Contextual,
            3 => MissingApplyAction::IgnorePath,
            4 => MissingApplyAction::DecidePerRow,
            5 => MissingApplyAction::Skip,
            _ => unreachable!(),
        }
    };

    let mut edits = BTreeMap::<PathBuf, OverlayEdits>::new();
    for row_index in selected {
        let (report_index, entry_index) = rows[row_index];
        let report = &reports[report_index];
        let entry = &report.report.entries[entry_index];
        match entry.category {
            TranslationCoverageCategory::UntranslatedFallback => {
                let action = match bulk_action {
                    MissingApplyAction::DecidePerRow => {
                        prompt_missing_apply_action(&mut ui, entry)?
                    }
                    other => other,
                };
                apply_missing_translation_action(&mut edits, &report.overlay_path, entry, action);
            }
            _ => {
                ui.message(&format!(
                    "{} at {} is stale/invalid; no safe automatic rewrite is applied, skipping. Use `brainbrew translations --resolve confirm` when the existing translation is still correct, or `brainbrew translations --resolve replace --translation <text>` to retire the stale record with a new translation.",
                    entry.category.as_str(),
                    entry.path
                ))?;
            }
        }
    }

    edits.retain(|_, edits| !edits.is_empty());
    if edits.is_empty() {
        return Ok(edits);
    }

    let confirm_labels = vec!["apply selected changes".to_owned(), "cancel".to_owned()];
    if ui.select_one("Apply selected changes?", &confirm_labels, 0)? == 0 {
        Ok(edits)
    } else {
        Ok(BTreeMap::new())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingApplyAction {
    NoChange,
    Direct,
    Contextual,
    IgnorePath,
    DecidePerRow,
    Skip,
}

fn prompt_missing_apply_action<R: Read, W: Write>(
    ui: &mut TerminalUi<'_, R, W>,
    entry: &TranslationCoverageEntry,
) -> Result<MissingApplyAction, String> {
    let context = contextual_context_for_entry(entry);
    let action_labels = vec![
        "mark no-change".to_owned(),
        "add direct source→source translation stub".to_owned(),
        format!("add contextual source→source translation stub at {context}"),
        "add ignore path for this deck path".to_owned(),
        "skip".to_owned(),
    ];
    match ui.select_one(
        &format!(
            "{} at {} source={}",
            entry.category.as_str(),
            entry.path,
            yaml_scalar(&entry.source)
        ),
        &action_labels,
        0,
    )? {
        0 => Ok(MissingApplyAction::NoChange),
        1 => Ok(MissingApplyAction::Direct),
        2 => Ok(MissingApplyAction::Contextual),
        3 => Ok(MissingApplyAction::IgnorePath),
        4 => Ok(MissingApplyAction::Skip),
        _ => unreachable!(),
    }
}

fn apply_missing_translation_action(
    edits: &mut BTreeMap<PathBuf, OverlayEdits>,
    overlay_path: &Path,
    entry: &TranslationCoverageEntry,
    action: MissingApplyAction,
) {
    let file_edits = edits.entry(overlay_path.to_path_buf()).or_default();
    match action {
        MissingApplyAction::NoChange => {
            file_edits.no_change.insert(entry.source.clone());
        }
        MissingApplyAction::Direct => {
            file_edits.direct.insert(entry.source.clone());
        }
        MissingApplyAction::Contextual => {
            file_edits
                .contextual
                .entry(contextual_context_for_entry(entry))
                .or_default()
                .insert(entry.source.clone());
        }
        MissingApplyAction::IgnorePath => {
            file_edits.ignore_paths.insert(entry.path.clone());
        }
        MissingApplyAction::DecidePerRow | MissingApplyAction::Skip => {}
    }
}

fn contextual_context_for_entry(entry: &TranslationCoverageEntry) -> String {
    context_parent_candidate(&entry.path).unwrap_or_else(|| entry.path.clone())
}

fn overlay_matches_scope(
    target: &str,
    planned: &PlannedOverlay,
    overlay: &Overlay,
    args: &TranslationArgs,
) -> bool {
    if let Some(overlay_filter) = &args.overlay {
        let overlay_id = overlay.id.as_str();
        if !planned.id.contains(overlay_filter)
            && !planned.display_file.contains(overlay_filter)
            && !overlay_id.contains(overlay_filter)
        {
            return false;
        }
    }
    if let Some(language) = &args.language
        && !language_matches(language, target, planned, overlay)
    {
        return false;
    }
    true
}

fn language_matches(
    language: &str,
    target: &str,
    planned: &PlannedOverlay,
    overlay: &Overlay,
) -> bool {
    let language = language.trim();
    if language.is_empty() {
        return true;
    }
    let target_has_language = target == language
        || target.starts_with(&format!("{language}-"))
        || target.ends_with(&format!("-{language}"));
    let overlay_id = overlay.id.as_str();
    let file_stem = Path::new(&planned.display_file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    target_has_language
        || planned.id.contains(&format!(".{language}"))
        || planned.id.contains(&format!("-{language}"))
        || overlay_id.contains(&format!(".{language}"))
        || overlay_id.contains(&format!("-{language}"))
        || file_stem == language
        || planned
            .display_file
            .split(['/', '\\'])
            .any(|part| part == language || part == format!("{language}.yaml"))
}

fn filter_context_to_report(
    mut context: TranslationContextView,
    report: &TranslationCoverageReport,
) -> TranslationContextView {
    context.units.retain(|unit| {
        report.entries.iter().any(|entry| {
            entry.path == unit.path
                && entry.category == unit.category
                && entry.source == unit.source
                && entry.context.as_deref() == unit.context.as_deref()
        })
    });
    context
}

fn retain_duplicate_source_entries(entries: &mut Vec<TranslationCoverageEntry>) {
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in entries.iter().filter(|entry| !entry.source.is_empty()) {
        *counts.entry(entry.source.clone()).or_insert(0) += 1;
    }
    entries.retain(|entry| counts.get(&entry.source).copied().unwrap_or(0) > 1);
}

fn entry_matches_scope(entry: &TranslationCoverageEntry, args: &TranslationArgs) -> bool {
    if let Some(source) = &args.source
        && !entry.source.contains(source)
    {
        return false;
    }
    if let Some(status) = &args.status
        && !status_matches_filter(entry.category, status)
    {
        return false;
    }
    if let Some(note) = &args.note {
        let full = if note.starts_with("note.") {
            note.clone()
        } else {
            format!("note.{note}")
        };
        if !entry.path.starts_with(&format!("notes.{note}."))
            && !entry.path.starts_with(&format!("notes.{full}."))
            && !entry.path.contains(&format!(".notes.{note}."))
            && !entry.path.contains(&format!(".notes.{full}."))
        {
            return false;
        }
    }
    if let Some(field) = &args.field {
        let full = if field.starts_with("field.") {
            field.clone()
        } else {
            format!("field.{field}")
        };
        if !entry.path.contains(&format!(".fields.{field}"))
            && !entry.path.contains(&format!(".fields.{full}"))
        {
            return false;
        }
    }
    if !args.path_prefixes.is_empty()
        && !args
            .path_prefixes
            .iter()
            .any(|prefix| entry.path == *prefix || entry.path.starts_with(&format!("{prefix}.")))
    {
        return false;
    }
    true
}

fn print_no_translation_reports(args: &TranslationArgs) -> Result<(), String> {
    let non_dictionary = non_dictionary_translation_overlays(args)?;
    if non_dictionary.is_empty() {
        println!(
            "{}",
            color_stdout(
                "No translation coverage entries matched the selected target and scope.",
                "33"
            )
        );
        println!(
            "Try a broader scope, or run `brainbrew translations --manifest {} --all-targets`.",
            args.manifest_path.display()
        );
        return Ok(());
    }

    println!(
        "{}",
        color_stdout(
            "No translation dictionary coverage reports matched the selected target and scope.",
            "33"
        )
    );
    println!();
    println!(
        "The selected target includes translation overlays that do not use a `translations:` dictionary yet:"
    );
    for overlay in non_dictionary {
        println!("  - {} ({})", overlay.id, overlay.label);
    }
    println!();
    println!(
        "`brainbrew translations` reports source-keyed translation dictionaries such as `translations.direct`, `translations.contextual`, plus path-scoped `target_adaptations`."
    );
    println!(
        "Patch-style translation overlays still compose, but they do not expose coverage data for this workflow yet."
    );
    Ok(())
}

fn non_dictionary_translation_overlays(
    args: &TranslationArgs,
) -> Result<Vec<OverlayChoice>, String> {
    let manifest = read_manifest(&args.manifest_path)?;
    let target_names = selected_target_names(&manifest, args);
    let mut choices = BTreeSet::<OverlayChoice>::new();
    for target in target_names {
        let plan = plan_manifest_target(
            &args.manifest_path,
            &target,
            &args.include_paths,
            &args.package_roots,
            &args.discovery_policy,
        )?;
        for (planned, overlay) in &plan.overlays {
            if overlay.kind == OverlayKind::Translation
                && overlay.translations.is_none()
                && overlay_matches_scope(&target, planned, overlay, args)
            {
                choices.insert(OverlayChoice {
                    id: planned.id.clone(),
                    label: planned.display_file.clone(),
                });
            }
        }
    }
    Ok(choices.into_iter().collect())
}

fn print_human_reports(reports: &[ScopedTranslationReport], full: bool) {
    for report in reports {
        let display_entries = display_entries(&report.report.entries, full);
        let display_counts = category_counts_refs(&display_entries);
        let all_counts = category_counts(&report.report.entries);
        let hidden_untranslated = hidden_untranslated_fallback_count(&report.report.entries, full);
        println!(
            "{}",
            color_stdout(
                &format!(
                    "Translation coverage for target {} overlay {} ({})",
                    report.target, report.overlay_id, report.overlay_file
                ),
                "1;36"
            )
        );
        println!(
            "  {}: {}",
            color_stdout("direct translations", "32"),
            all_counts.get("direct_translation").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("contextual translations", "36"),
            all_counts
                .get("contextual_translation")
                .copied()
                .unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("target adaptations", "35"),
            all_counts.get("target_adaptation").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("intentionally unchanged text", "34"),
            all_counts.get("no_change").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("missing text translations", "31"),
            display_counts
                .get("untranslated_fallback")
                .copied()
                .unwrap_or(0)
        );
        if hidden_untranslated > 0 {
            println!(
                "  {}: {}",
                color_stdout("hidden structural/media/tag fallbacks", "2"),
                hidden_untranslated
            );
        }
        println!(
            "  {}: {}",
            color_stdout("ignored entries", "2"),
            all_counts.get("ignored_source").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("stale translations", "33"),
            all_counts.get("stale_translation").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("stale/invalid keys", "33"),
            report
                .report
                .entries
                .iter()
                .filter(|entry| is_stale_or_invalid(entry.category))
                .count()
        );
        if hidden_untranslated > 0 && !full {
            println!("  hint: use --full to include structural/media/tag fallbacks");
        }

        let problem_entries = display_entries
            .iter()
            .filter(|entry| entry.category.is_problem())
            .copied()
            .collect::<Vec<_>>();
        let shown_problem_limit = 40;
        for entry in problem_entries.iter().take(shown_problem_limit) {
            let old_source = entry
                .old_source
                .as_deref()
                .map(|old_source| format!(" old_source={}", yaml_scalar(old_source)))
                .unwrap_or_default();
            println!(
                "  - {} {}{} source={} translated={}",
                color_category(entry.category, display_category_name(entry.category)),
                entry.path,
                old_source,
                yaml_scalar(&entry.source),
                entry
                    .translated
                    .as_deref()
                    .map(yaml_scalar)
                    .unwrap_or_else(|| "''".to_owned())
            );
        }
        if problem_entries.len() > shown_problem_limit {
            println!(
                "  … {} more problem entries not shown; narrow the scope or use --json",
                problem_entries.len() - shown_problem_limit
            );
        }
    }
}

fn display_entries(
    entries: &[TranslationCoverageEntry],
    full: bool,
) -> Vec<&TranslationCoverageEntry> {
    entries
        .iter()
        .filter(|entry| full || default_report_includes_entry(entry))
        .collect()
}

fn hidden_untranslated_fallback_count(entries: &[TranslationCoverageEntry], full: bool) -> usize {
    if full {
        return 0;
    }
    entries
        .iter()
        .filter(|entry| {
            entry.category == TranslationCoverageCategory::UntranslatedFallback
                && !default_report_includes_entry(entry)
        })
        .count()
}

fn default_report_includes_entry(entry: &TranslationCoverageEntry) -> bool {
    if entry.category != TranslationCoverageCategory::UntranslatedFallback {
        return true;
    }
    default_report_includes_untranslated_fallback(entry)
}

fn default_report_includes_untranslated_fallback(entry: &TranslationCoverageEntry) -> bool {
    let path = entry.path.as_str();
    path.starts_with("notes.")
        && path.contains(".fields.")
        && !is_media_or_structural_field_path(path)
        && !looks_like_media_markup(&entry.source)
}

fn is_media_or_structural_field_path(path: &str) -> bool {
    path.ends_with(".fields.field.flag") || path.ends_with(".fields.field.map")
}

fn looks_like_media_markup(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.starts_with("<img ") || trimmed.starts_with("<img")
}

fn display_category_name(category: TranslationCoverageCategory) -> &'static str {
    match category {
        TranslationCoverageCategory::UntranslatedFallback => "missing_translation",
        other => other.as_str(),
    }
}

fn is_stale_or_invalid(category: TranslationCoverageCategory) -> bool {
    matches!(
        category,
        TranslationCoverageCategory::StaleDirectKey
            | TranslationCoverageCategory::StaleContextualKey
            | TranslationCoverageCategory::StaleNoChangeKey
            | TranslationCoverageCategory::StaleTargetAdaptation
            | TranslationCoverageCategory::StaleVariableKey
            | TranslationCoverageCategory::StaleAdapterIdKey
            | TranslationCoverageCategory::StaleTranslation
            | TranslationCoverageCategory::InvalidTargetAdaptation
            | TranslationCoverageCategory::StructuralFieldNameTranslation
    )
}

fn validate_status_filter(status: &str) -> Result<(), String> {
    if status_filter_is_known(status) {
        Ok(())
    } else {
        Err(format!(
            "invalid translation status {status:?}; expected missing, stale, translated, changed, direct, contextual, no-change, adaptation, variable, adapter-id, ignored, or a coverage category name"
        ))
    }
}

fn status_filter_is_known(status: &str) -> bool {
    let normalized = normalized_status_filter(status);
    matches!(
        normalized.as_str(),
        "missing"
            | "untranslated"
            | "stale"
            | "invalid"
            | "translated"
            | "covered"
            | "changed"
            | "direct"
            | "contextual"
            | "no_change"
            | "adaptation"
            | "variable"
            | "adapter_id"
            | "ignored"
            | "direct_translation"
            | "contextual_translation"
            | "target_adaptation"
            | "target_deletion"
            | "variable_translation"
            | "adapter_id_translation"
            | "ignored_source"
            | "untranslated_fallback"
            | "stale_direct_key"
            | "stale_contextual_key"
            | "stale_no_change_key"
            | "stale_target_adaptation"
            | "stale_variable_key"
            | "stale_adapter_id_key"
            | "stale_translation"
            | "invalid_target_adaptation"
            | "structural_field_name_translation"
    )
}

fn status_matches_filter(category: TranslationCoverageCategory, status: &str) -> bool {
    let normalized = normalized_status_filter(status);
    match normalized.as_str() {
        "missing" | "untranslated" => category == TranslationCoverageCategory::UntranslatedFallback,
        "stale" | "invalid" => is_stale_or_invalid(category),
        "translated" | "covered" => matches!(
            category,
            TranslationCoverageCategory::DirectTranslation
                | TranslationCoverageCategory::ContextualTranslation
                | TranslationCoverageCategory::NoChange
                | TranslationCoverageCategory::TargetAdaptation
                | TranslationCoverageCategory::TargetDeletion
                | TranslationCoverageCategory::VariableTranslation
                | TranslationCoverageCategory::AdapterIdTranslation
        ),
        "changed" => matches!(
            category,
            TranslationCoverageCategory::DirectTranslation
                | TranslationCoverageCategory::ContextualTranslation
                | TranslationCoverageCategory::TargetAdaptation
                | TranslationCoverageCategory::TargetDeletion
                | TranslationCoverageCategory::VariableTranslation
        ),
        "direct" => category == TranslationCoverageCategory::DirectTranslation,
        "contextual" => category == TranslationCoverageCategory::ContextualTranslation,
        "no_change" => category == TranslationCoverageCategory::NoChange,
        "adaptation" => matches!(
            category,
            TranslationCoverageCategory::TargetAdaptation
                | TranslationCoverageCategory::TargetDeletion
        ),
        "variable" => category == TranslationCoverageCategory::VariableTranslation,
        "adapter_id" => category == TranslationCoverageCategory::AdapterIdTranslation,
        "ignored" => category == TranslationCoverageCategory::IgnoredSource,
        other => category.as_str() == other,
    }
}

fn normalized_status_filter(status: &str) -> String {
    status.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

/// Versioned envelope shared by every successful `translations --json` mode.
///
/// Keep this intentionally small: mode-specific objects remain additive while
/// scripts can validate the schema version and result kind before reading them.
fn print_translation_json(kind: &str, payload: serde_json::Value) {
    let mut value = payload;
    let object = value
        .as_object_mut()
        .expect("translation JSON payload is always an object");
    object.insert(
        "schema_version".to_owned(),
        json!(TRANSLATION_JSON_SCHEMA_VERSION),
    );
    object.insert("kind".to_owned(), json!(kind));
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("translation JSON serializes")
    );
}

fn print_json_reports(reports: &[ScopedTranslationReport], applied: &BTreeMap<String, usize>) {
    let reports_json = reports
        .iter()
        .map(|report| {
            json!({
                "target": report.target,
                "overlay": report.overlay_id,
                "file": report.overlay_file,
                "summary": category_counts(&report.report.entries),
                "entries": report.report.entries.iter().map(entry_json).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    print_translation_json(
        "translation_report",
        json!({
            "reports": reports_json,
            "applied": applied,
        }),
    );
}

fn print_json_contexts(reports: &[ScopedTranslationReport]) {
    let contexts_json = reports
        .iter()
        .map(|report| {
            json!({
                "target": report.target,
                "language": inferred_report_language(report),
                "overlay": report.overlay_id,
                "file": report.overlay_file,
                "units": report.context.units.iter().map(context_unit_json).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    print_translation_json("translation_context", json!({ "contexts": contexts_json }));
}

fn context_unit_json(unit: &TranslationContextUnit) -> serde_json::Value {
    json!({
        "status": unit.category.as_str(),
        "path": unit.path,
        "source": unit.source,
        "old_source": unit.old_source,
        "translated": unit.translated,
        "context": unit.context,
        "note_id": unit.note_id.as_ref().map(|id| id.as_str()),
        "note_type_id": unit.note_type_id.as_ref().map(|id| id.as_str()),
        "field_id": unit.field_id.as_ref().map(|id| id.as_str()),
        "field_name": unit.field_name,
        "source_occurrences": unit.source_occurrences,
        "note_fields": unit.note_fields.iter().map(|field| json!({
            "field_id": field.field_id.as_str(),
            "field_name": field.field_name,
            "source": field.source,
            "translated": field.translated,
            "status": field.category.map(|category| category.as_str()),
        })).collect::<Vec<_>>(),
        "message": unit.message.as_ref().map(|message| json!({
            "source": message.source,
            "translated": message.translated,
            "format": message.format.as_ref().map(|component| json!({
                "index": component.index,
                "name": component.name,
                "kind": component.kind.as_str(),
                "path": component.path,
                "source": component.source,
                "translated": component.translated,
                "reference": component.reference,
                "status": component.category.map(|category| category.as_str()),
            })),
            "components": message.components.iter().map(|component| json!({
                "index": component.index,
                "name": component.name,
                "kind": component.kind.as_str(),
                "path": component.path,
                "source": component.source,
                "translated": component.translated,
                "reference": component.reference,
                "status": component.category.map(|category| category.as_str()),
            })).collect::<Vec<_>>(),
        })),
        "card_templates": unit.card_templates.iter().map(|card| json!({
            "template_id": card.template_id.as_str(),
            "template_name": card.template_name,
            "sides": card.sides.iter().map(|side| side.as_str()).collect::<Vec<_>>(),
            "question_format": card.question_format,
            "answer_format": card.answer_format,
        })).collect::<Vec<_>>(),
    })
}

fn print_human_contexts(reports: &[ScopedTranslationReport], full: bool) {
    for report in reports {
        let language = inferred_report_language(report);
        let units = report
            .context
            .units
            .iter()
            .filter(|unit| full || default_context_includes_unit(unit))
            .collect::<Vec<_>>();
        println!(
            "{}",
            color_stdout(
                &format!(
                    "Translation context for target {} language {} overlay {} ({})",
                    report.target, language, report.overlay_id, report.overlay_file
                ),
                "1;36"
            )
        );
        if units.is_empty() {
            println!("  no context units matched this scope");
            continue;
        }
        for unit in units {
            print_context_unit(unit, &language);
        }
    }
}

fn default_context_includes_unit(unit: &TranslationContextUnit) -> bool {
    is_stale_or_invalid(unit.category)
        || (unit.note_id.is_some()
            && unit.field_id.is_some()
            && !is_media_or_structural_field_path(&unit.path)
            && !looks_like_media_markup(&unit.source))
}

fn print_context_unit(unit: &TranslationContextUnit, language: &str) {
    println!(
        "  - {} {}",
        color_category(unit.category, display_category_name(unit.category)),
        unit.path
    );
    if unit.source_occurrences > 1 {
        println!(
            "    duplicate source group: {} occurrence(s) of {}",
            unit.source_occurrences,
            yaml_scalar(&unit.source)
        );
    }
    if let Some(note_id) = &unit.note_id {
        println!("    note: {note_id}");
    }
    if let Some(note_type_id) = &unit.note_type_id {
        println!("    note type: {note_type_id}");
    }
    if let Some(field_id) = &unit.field_id {
        let field_name = unit
            .field_name
            .as_ref()
            .map(|name| format!(" ({name})"))
            .unwrap_or_default();
        println!("    field: {field_id}{field_name}");
    }
    if !unit.note_fields.is_empty() {
        print_note_field_context(unit, language);
    }
    if let Some(message) = &unit.message {
        print_message_context(message, language);
    }
    if let Some(context) = &unit.context {
        println!("    dictionary context: {context}");
    }
    if !unit.card_templates.is_empty() {
        let cards = unit
            .card_templates
            .iter()
            .map(|card| {
                let sides = card
                    .sides
                    .iter()
                    .map(|side| side.as_str())
                    .collect::<Vec<_>>()
                    .join("+");
                format!("{} [{}]", card.template_id, sides)
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("    cards: {cards}");
    }
    for line in side_by_side_lines(
        "source/en",
        &unit.source,
        &format!("target/{language}"),
        unit.translated.as_deref().unwrap_or(""),
    ) {
        println!("    {line}");
    }
}

fn print_note_field_context(unit: &TranslationContextUnit, language: &str) {
    println!("    note fields (source/en | target/{language}):");
    for field in unit.note_fields.iter().filter(|field| {
        !field.source.is_empty()
            && !looks_like_media_markup(&field.source)
            && !is_media_or_structural_field_id(field.field_id.as_str())
    }) {
        let marker = if unit.field_id.as_ref() == Some(&field.field_id) {
            "*"
        } else {
            " "
        };
        println!(
            "    {marker} {:<24} {} | {}",
            format!("{} ({})", field.field_id, field.field_name),
            compact_context_value(&field.source),
            compact_context_value(&field.translated)
        );
    }
}

fn print_message_context(message: &TranslationMessageContext, language: &str) {
    println!("    structured message (source/en | target/{language}):");
    println!(
        "      resolved {} | {}",
        compact_context_value(&message.source),
        compact_context_value(&message.translated)
    );
    if let Some(format) = &message.format {
        println!(
            "      {:<32} {} | {}",
            "format",
            compact_context_value(&format.source),
            compact_context_value(&format.translated)
        );
    }
    for component in &message.components {
        let label_prefix = component
            .name
            .as_deref()
            .map_or_else(|| format!("[{}]", component.index), ToOwned::to_owned);
        let label = match component.reference.as_deref() {
            Some(reference) => {
                format!("{} {} {}", label_prefix, component.kind.as_str(), reference)
            }
            None => format!("{} {}", label_prefix, component.kind.as_str()),
        };
        println!(
            "      {:<32} {} | {}",
            label,
            compact_context_value(&component.source),
            compact_context_value(&component.translated)
        );
    }
}

fn is_media_or_structural_field_id(field_id: &str) -> bool {
    field_id == "field.flag" || field_id == "field.map"
}

fn compact_context_value(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 42;
    if collapsed.chars().count() <= LIMIT {
        collapsed
    } else {
        let mut truncated = collapsed
            .chars()
            .take(LIMIT.saturating_sub(1))
            .collect::<String>();
        truncated.push('…');
        truncated
    }
}

fn side_by_side_lines(
    left_header: &str,
    left: &str,
    right_header: &str,
    right: &str,
) -> Vec<String> {
    const COLUMN_WIDTH: usize = 38;
    let mut lines = vec![format!(
        "{left_header:<COLUMN_WIDTH$} | {right_header:<COLUMN_WIDTH$}"
    )];
    let left_lines = wrap_context_cell(left, COLUMN_WIDTH);
    let right_lines = wrap_context_cell(right, COLUMN_WIDTH);
    let row_count = left_lines.len().max(right_lines.len());
    for index in 0..row_count {
        let left_cell = left_lines
            .get(index)
            .map(String::as_str)
            .unwrap_or_default();
        let right_cell = right_lines
            .get(index)
            .map(String::as_str)
            .unwrap_or_default();
        lines.push(format!(
            "{left_cell:<COLUMN_WIDTH$} | {right_cell:<COLUMN_WIDTH$}"
        ));
    }
    lines
}

fn wrap_context_cell(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in text.lines().flat_map(|line| line.split("\n")) {
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            let separator = usize::from(!current.is_empty());
            if current.chars().count() + separator + word.chars().count() > width
                && !current.is_empty()
            {
                lines.push(current);
                current = String::new();
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
struct TranslationSummaryCounts {
    direct_translation: usize,
    contextual_translation: usize,
    no_change: usize,
    target_adaptation: usize,
    target_deletion: usize,
    variable_translation: usize,
    adapter_id_translation: usize,
    untranslated_fallback: usize,
    missing_text_translation: usize,
    hidden_untranslated_fallback: usize,
    ignored_source: usize,
    stale_invalid: usize,
}

struct TranslationSummaryRow {
    language: String,
    targets: BTreeSet<String>,
    overlay_id: String,
    overlay_file: String,
    counts: TranslationSummaryCounts,
}

fn print_json_summary(reports: &[ScopedTranslationReport]) {
    let summaries = translation_summary_rows(reports)
        .into_iter()
        .map(|row| {
            json!({
                "language": row.language,
                "targets": row.targets.into_iter().collect::<Vec<_>>(),
                "overlay": row.overlay_id,
                "file": row.overlay_file,
                "direct_translation": row.counts.direct_translation,
                "contextual_translation": row.counts.contextual_translation,
                "no_change": row.counts.no_change,
                "target_adaptation": row.counts.target_adaptation,
                "target_deletion": row.counts.target_deletion,
                "variable_translation": row.counts.variable_translation,
                "adapter_id_translation": row.counts.adapter_id_translation,
                "untranslated_fallback": row.counts.untranslated_fallback,
                "missing_text_translation": row.counts.missing_text_translation,
                "hidden_untranslated_fallback": row.counts.hidden_untranslated_fallback,
                "ignored_source": row.counts.ignored_source,
                "stale_invalid": row.counts.stale_invalid,
            })
        })
        .collect::<Vec<_>>();
    print_translation_json("translation_summary", json!({ "summaries": summaries }));
}

fn print_human_summary(reports: &[ScopedTranslationReport], full: bool) {
    let rows = translation_summary_rows(reports);
    println!("{}", color_stdout("Translation coverage summary", "1;36"));
    if full {
        print_full_human_summary_table(rows);
    } else {
        print_compact_human_summary_table(rows);
        println!("Legend: ctxt=contextual, same=no-change, miss=actionable missing text.");
        println!("        hidden/raw=untranslated fallbacks; --summary --full shows overlay/file.");
    }
}

fn print_compact_human_summary_table(rows: Vec<TranslationSummaryRow>) {
    let mut table = vec![vec![
        "lang".to_owned(),
        "tgt".to_owned(),
        "direct".to_owned(),
        "ctxt".to_owned(),
        "same".to_owned(),
        "adapt".to_owned(),
        "delete".to_owned(),
        "vars".to_owned(),
        "ids".to_owned(),
        "miss".to_owned(),
        "hidden".to_owned(),
        "raw".to_owned(),
        "ign".to_owned(),
        "stale".to_owned(),
    ]];
    for row in rows {
        table.push(vec![
            row.language,
            row.targets.len().to_string(),
            row.counts.direct_translation.to_string(),
            row.counts.contextual_translation.to_string(),
            row.counts.no_change.to_string(),
            row.counts.target_adaptation.to_string(),
            row.counts.variable_translation.to_string(),
            row.counts.adapter_id_translation.to_string(),
            row.counts.missing_text_translation.to_string(),
            row.counts.hidden_untranslated_fallback.to_string(),
            row.counts.untranslated_fallback.to_string(),
            row.counts.ignored_source.to_string(),
            row.counts.stale_invalid.to_string(),
        ]);
    }
    let right_aligned = [
        false, true, true, true, true, true, true, true, true, true, true, true, true, true,
    ];
    for line in aligned_table_lines(&table, &right_aligned) {
        println!("{line}");
    }
}

fn print_full_human_summary_table(rows: Vec<TranslationSummaryRow>) {
    let mut table = vec![vec![
        "language".to_owned(),
        "targets".to_owned(),
        "overlay".to_owned(),
        "file".to_owned(),
        "direct".to_owned(),
        "contextual".to_owned(),
        "no-change".to_owned(),
        "adaptations".to_owned(),
        "deletions".to_owned(),
        "variables".to_owned(),
        "adapter-ids".to_owned(),
        "missing-text".to_owned(),
        "hidden-fallbacks".to_owned(),
        "raw-untranslated".to_owned(),
        "ignored".to_owned(),
        "stale/invalid".to_owned(),
    ]];
    for row in rows {
        table.push(vec![
            row.language,
            row.targets.len().to_string(),
            row.overlay_id,
            row.overlay_file,
            row.counts.direct_translation.to_string(),
            row.counts.contextual_translation.to_string(),
            row.counts.no_change.to_string(),
            row.counts.target_adaptation.to_string(),
            row.counts.variable_translation.to_string(),
            row.counts.adapter_id_translation.to_string(),
            row.counts.missing_text_translation.to_string(),
            row.counts.hidden_untranslated_fallback.to_string(),
            row.counts.untranslated_fallback.to_string(),
            row.counts.ignored_source.to_string(),
            row.counts.stale_invalid.to_string(),
        ]);
    }
    let right_aligned = [
        false, true, false, false, true, true, true, true, true, true, true, true, true, true,
        true, true,
    ];
    for line in aligned_table_lines(&table, &right_aligned) {
        println!("{line}");
    }
}

fn aligned_table_lines(rows: &[Vec<String>], right_aligned: &[bool]) -> Vec<String> {
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; column_count];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (index, width) in widths.iter().enumerate() {
                if index > 0 {
                    line.push_str("  ");
                }
                let cell = row.get(index).map(String::as_str).unwrap_or_default();
                let padding = width.saturating_sub(cell.chars().count());
                if right_aligned.get(index).copied().unwrap_or(false) {
                    line.push_str(&" ".repeat(padding));
                    line.push_str(cell);
                } else {
                    line.push_str(cell);
                    line.push_str(&" ".repeat(padding));
                }
            }
            line.trim_end().to_owned()
        })
        .collect()
}

fn translation_summary_rows(reports: &[ScopedTranslationReport]) -> Vec<TranslationSummaryRow> {
    let mut grouped =
        BTreeMap::<(String, String, String, TranslationSummaryCounts), BTreeSet<String>>::new();
    for report in reports {
        let language = inferred_report_language(report);
        let counts = translation_summary_counts(&report.report.entries);
        grouped
            .entry((
                language,
                report.overlay_id.clone(),
                report.overlay_file.clone(),
                counts,
            ))
            .or_default()
            .insert(report.target.clone());
    }
    grouped
        .into_iter()
        .map(
            |((language, overlay_id, overlay_file, counts), targets)| TranslationSummaryRow {
                language,
                targets,
                overlay_id,
                overlay_file,
                counts,
            },
        )
        .collect()
}

fn translation_summary_counts(entries: &[TranslationCoverageEntry]) -> TranslationSummaryCounts {
    let counts = category_counts(entries);
    let untranslated_fallback = counts.get("untranslated_fallback").copied().unwrap_or(0);
    let missing_text_translation = entries
        .iter()
        .filter(|entry| {
            entry.category == TranslationCoverageCategory::UntranslatedFallback
                && default_report_includes_untranslated_fallback(entry)
        })
        .count();
    TranslationSummaryCounts {
        direct_translation: counts.get("direct_translation").copied().unwrap_or(0),
        contextual_translation: counts.get("contextual_translation").copied().unwrap_or(0),
        no_change: counts.get("no_change").copied().unwrap_or(0),
        target_adaptation: counts.get("target_adaptation").copied().unwrap_or(0),
        target_deletion: counts.get("target_deletion").copied().unwrap_or(0),
        variable_translation: counts.get("variable_translation").copied().unwrap_or(0),
        adapter_id_translation: counts.get("adapter_id_translation").copied().unwrap_or(0),
        untranslated_fallback,
        missing_text_translation,
        hidden_untranslated_fallback: untranslated_fallback
            .saturating_sub(missing_text_translation),
        ignored_source: counts.get("ignored_source").copied().unwrap_or(0),
        stale_invalid: entries
            .iter()
            .filter(|entry| is_stale_or_invalid(entry.category))
            .count(),
    }
}

fn inferred_report_language(report: &ScopedTranslationReport) -> String {
    let stem = Path::new(&report.overlay_file)
        .file_stem()
        .and_then(|stem| stem.to_str());
    if let Some(language) = language_candidate(stem) {
        return language;
    }
    if let Some(language) = stem
        .and_then(|stem| stem.strip_prefix("translation-"))
        .and_then(|candidate| language_candidate(Some(candidate)))
    {
        return language;
    }
    if let Some((_, suffix)) = report.overlay_id.rsplit_once(".translation.") {
        if let Some(language) = language_candidate(Some(suffix)) {
            return language;
        }
        if let Some(language) = suffix
            .rsplit_once('.')
            .and_then(|(_, last_segment)| language_candidate(Some(last_segment)))
        {
            return language;
        }
    }
    if let Some(language) = language_candidate(Some(trim_target_language_suffixes(&report.target)))
    {
        return language;
    }
    if let Some(language) = stem
        .and_then(|stem| stem.rsplit_once(['-', '.']))
        .and_then(|(_, suffix)| language_candidate(Some(suffix)))
    {
        return language;
    }
    "unknown".to_owned()
}

fn trim_target_language_suffixes(target: &str) -> &str {
    let mut candidate = target;
    loop {
        let Some((prefix, suffix)) = candidate.rsplit_once('-') else {
            return candidate;
        };
        if matches!(
            suffix,
            "standard" | "extended" | "experimental" | "hardcore" | "release"
        ) {
            candidate = prefix;
        } else {
            return candidate;
        }
    }
}

fn entry_json(entry: &TranslationCoverageEntry) -> serde_json::Value {
    json!({
        "category": entry.category.as_str(),
        "path": entry.path,
        "source": entry.source,
        "old_source": entry.old_source,
        "translated": entry.translated,
        "context": entry.context,
    })
}

fn category_counts(entries: &[TranslationCoverageEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.as_str()).or_insert(0) += 1;
    }
    counts
}

fn category_counts_refs(entries: &[&TranslationCoverageEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.category.as_str()).or_insert(0) += 1;
    }
    counts
}

struct PlannedTranslationDocument {
    path: PathBuf,
    input: String,
    document: OverlaySourceDocument,
    count: usize,
}

fn plan_overlay_edits(
    path: &Path,
    edits: &OverlayEdits,
) -> Result<PlannedTranslationDocument, String> {
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut document = overlay_source_document(path, &input)?;
    document
        .add_translation_stubs(TranslationStubs {
            direct: edits.direct.clone(),
            contextual: edits.contextual.clone(),
            no_change: edits.no_change.clone(),
            ignore_paths: edits.ignore_paths.clone(),
        })
        .map_err(|error| error.to_string())?;
    Ok(PlannedTranslationDocument {
        path: path.to_path_buf(),
        input,
        document,
        count: edits.count(),
    })
}

fn commit_translation_documents(
    args: &TranslationArgs,
    planned: Vec<PlannedTranslationDocument>,
) -> Result<BTreeMap<String, usize>, String> {
    let mut outputs = Vec::new();
    let mut updated_overlays = BTreeMap::new();
    let mut applied = BTreeMap::new();

    for planned_document in planned {
        if planned_document.count == 0 {
            continue;
        }
        let emission = planned_document
            .document
            .emit()
            .map_err(|error| error.to_string())?;
        if !emission.included().is_empty() {
            return Err(format!(
                "{}: translation dictionary operation unexpectedly edited an included source",
                planned_document.path.display()
            ));
        }
        let output = emission.root().text().to_owned();
        let canonical_path = fs::canonicalize(&planned_document.path)
            .map_err(|error| format!("{}: {error}", planned_document.path.display()))?;
        let overlay = overlay_from_source_text(&planned_document.path, &output)?;
        updated_overlays.insert(canonical_path, overlay);
        if output != planned_document.input {
            let validation_path = planned_document.path.clone();
            outputs.push(PlannedWorkspaceFile::validated(
                &planned_document.path,
                planned_document.input.into_bytes(),
                output.into_bytes(),
                move |bytes| {
                    let source = std::str::from_utf8(bytes).map_err(|error| {
                        format!(
                            "{}: replacement is not UTF-8: {error}",
                            validation_path.display()
                        )
                    })?;
                    overlay_source_document(&validation_path, source).and_then(|document| {
                        document
                            .emit()
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    })
                },
            )?);
        }
        applied.insert(
            planned_document.path.display().to_string(),
            planned_document.count,
        );
    }

    validate_final_translation_composition(args, &updated_overlays)?;
    commit_workspace_files(&manifest_root(&args.manifest_path), outputs)?;
    Ok(applied)
}

fn validate_final_translation_composition(
    args: &TranslationArgs,
    updated_overlays: &BTreeMap<PathBuf, Overlay>,
) -> Result<(), String> {
    if updated_overlays.is_empty() {
        return Ok(());
    }
    let manifest = read_manifest(&args.manifest_path)?;
    for target in selected_target_names(&manifest, args) {
        let plan = plan_manifest_target(
            &args.manifest_path,
            &target,
            &args.include_paths,
            &args.package_roots,
            &args.discovery_policy,
        )?;
        let mut current = plan.base.clone();
        for (planned, original_overlay) in &plan.overlays {
            let canonical_path = fs::canonicalize(&planned.file)
                .map_err(|error| format!("{}: {error}", planned.file.display()))?;
            let overlay = updated_overlays
                .get(&canonical_path)
                .unwrap_or(original_overlay);
            if overlay.translations.is_some() {
                current = compose_lenient_translation_overlay(&current, overlay)?;
            } else {
                current = current
                    .compose(std::slice::from_ref(overlay))
                    .map_err(|report| {
                        output::compose_error(
                            "translations",
                            json!({"target": target, "overlay": planned.id}),
                            &report,
                        )
                    })?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct OverlayChoice {
    id: String,
    label: String,
}

fn translation_overlay_choices(args: &TranslationArgs) -> Result<Vec<OverlayChoice>, String> {
    let manifest = read_manifest(&args.manifest_path)?;
    let target_names = selected_target_names(&manifest, args);
    let mut choices = BTreeSet::<OverlayChoice>::new();
    for target in target_names {
        let plan = plan_manifest_target(
            &args.manifest_path,
            &target,
            &args.include_paths,
            &args.package_roots,
            &args.discovery_policy,
        )?;
        for (planned, overlay) in &plan.overlays {
            if overlay.kind == OverlayKind::Translation
                && args
                    .language
                    .as_ref()
                    .is_none_or(|language| language_matches(language, &target, planned, overlay))
            {
                let suffix = if overlay.translations.is_some() {
                    ""
                } else {
                    "  (no translations: dictionary)"
                };
                choices.insert(OverlayChoice {
                    id: planned.id.clone(),
                    label: format!("{}  {}{}", planned.id, planned.display_file, suffix),
                });
            }
        }
    }
    Ok(choices.into_iter().collect())
}

fn inferred_languages(manifest: &FederatedDeckManifest) -> Vec<String> {
    let mut languages = BTreeSet::new();
    for (id, overlay) in &manifest.overlays {
        let is_translation_overlay = overlay.kind.as_deref() == Some("translation")
            || id.contains("translation.")
            || id.contains("translation-");
        if !is_translation_overlay {
            continue;
        }
        maybe_insert_language(id.rsplit(['.', '-']).next(), &mut languages);
        let stem = Path::new(&overlay.file)
            .file_stem()
            .and_then(|stem| stem.to_str());
        if let Some(stem) = stem {
            if let Some((_, suffix)) = stem.rsplit_once(['-', '.']) {
                maybe_insert_language(Some(suffix), &mut languages);
            } else {
                maybe_insert_language(Some(stem), &mut languages);
            }
        }
    }
    languages.into_iter().collect()
}

fn maybe_insert_language(candidate: Option<&str>, languages: &mut BTreeSet<String>) {
    if let Some(language) = language_candidate(candidate) {
        languages.insert(language);
    }
}

fn language_candidate(candidate: Option<&str>) -> Option<String> {
    let language = candidate?;
    if (2..=8).contains(&language.len())
        && language
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch == '-')
        && language != "translation"
    {
        Some(language.to_owned())
    } else {
        None
    }
}

fn note_id_from_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("notes.")?;
    let end = [".fields.", ".tags.", ".variables.", ".adapter_ids."]
        .into_iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn field_id_from_path(path: &str) -> Option<String> {
    let rest = path.split_once(".fields.")?.1;
    let end = [".variables.", ".adapter_ids."]
        .into_iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].to_owned())
}

fn context_parent_candidate(path: &str) -> Option<String> {
    if let Some((note_context, _)) = path.split_once(".fields.")
        && note_context.starts_with("notes.")
    {
        return Some(note_context.to_owned());
    }
    if let Some((note_context, _)) = path.split_once(".tags.")
        && note_context.starts_with("notes.")
    {
        return Some(note_context.to_owned());
    }
    if let Some((note_context, _)) = path.split_once(".variables.")
        && note_context.starts_with("notes.")
    {
        return Some(note_context.to_owned());
    }
    if let Some((note_type_context, _)) = path.split_once(".fields.")
        && note_type_context.starts_with("note_types.")
    {
        return Some(note_type_context.to_owned());
    }
    if let Some((note_type_context, _)) = path.split_once(".card_templates.")
        && note_type_context.starts_with("note_types.")
    {
        return Some(note_type_context.to_owned());
    }
    None
}

fn discover_nearby_manifests(policy: &DiscoveryPolicy) -> Result<Vec<PathBuf>, String> {
    let current = env::current_dir().map_err(|error| {
        format!("package discovery failed closed: could not read current directory: {error}")
    })?;
    let mut results =
        ManifestRegistry::discover_manifest_paths(std::slice::from_ref(&current), policy)?
            .into_iter()
            .map(|path| relative_to_current(&path))
            .collect::<BTreeSet<_>>();
    for directory in current.ancestors().skip(1).take(4) {
        let candidate = directory.join("brainbrew.yaml");
        match fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                results.insert(relative_to_current(&candidate));
            }
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "package discovery rejected symlink {} while checking nearby manifests",
                    candidate.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "package discovery failed closed while inspecting nearby manifest {}: {error}",
                    candidate.display()
                ));
            }
        }
    }
    Ok(results.into_iter().take(20).collect())
}

fn relative_to_current(path: &Path) -> PathBuf {
    env::current_dir()
        .ok()
        .and_then(|current| path.strip_prefix(current).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

fn missing_manifest_error(path: &Path, policy: &DiscoveryPolicy) -> Result<String, String> {
    let mut message = format!("No Brain Brew manifest found at {}.", path.display());
    if path == Path::new("brainbrew.yaml") {
        message.push_str("\n\nRun from a deck workspace, or pass --manifest <path>.");
    }
    let manifests = discover_nearby_manifests(policy)?;
    if !manifests.is_empty() {
        message.push_str("\n\nFound possible manifests:");
        for manifest in &manifests {
            message.push_str(&format!("\n  - {}", manifest.display()));
        }
        if let Some(first) = manifests.first() {
            message.push_str("\n\nTry:");
            message.push_str(&format!(
                "\n  brainbrew translations --manifest {} --target <target>",
                shell_word(&first.display().to_string())
            ));
            message.push_str("\n  brainbrew translations --interactive");
        }
    }
    Ok(message)
}

fn equivalent_command(args: &TranslationArgs) -> String {
    let mut parts = vec!["brainbrew".to_owned(), "translations".to_owned()];
    push_flag_value(
        &mut parts,
        "--manifest",
        &args.manifest_path.display().to_string(),
    );
    if args.all_targets {
        parts.push("--all-targets".to_owned());
    } else if let Some(target) = &args.target {
        push_flag_value(&mut parts, "--target", target);
    }
    if let Some(language) = &args.language {
        push_flag_value(&mut parts, "--language", language);
    }
    if let Some(overlay) = &args.overlay {
        push_flag_value(&mut parts, "--overlay", overlay);
    }
    if let Some(note) = &args.note {
        push_flag_value(&mut parts, "--note", note);
    }
    if let Some(field) = &args.field {
        push_flag_value(&mut parts, "--field", field);
    }
    for prefix in &args.path_prefixes {
        push_flag_value(&mut parts, "--path-prefix", prefix);
    }
    if args.apply {
        parts.push("--apply".to_owned());
    }
    if args.full {
        parts.push("--full".to_owned());
    }
    parts.join(" ")
}

fn push_flag_value(parts: &mut Vec<String>, flag: &str, value: &str) {
    parts.push(flag.to_owned());
    parts.push(shell_word(value));
}

fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn color_category(category: TranslationCoverageCategory, text: &str) -> String {
    match category {
        TranslationCoverageCategory::DirectTranslation => color_stdout(text, "32"),
        TranslationCoverageCategory::ContextualTranslation => color_stdout(text, "36"),
        TranslationCoverageCategory::NoChange => color_stdout(text, "34"),
        TranslationCoverageCategory::TargetAdaptation => color_stdout(text, "35"),
        TranslationCoverageCategory::TargetDeletion => color_stdout(text, "31"),
        TranslationCoverageCategory::VariableTranslation
        | TranslationCoverageCategory::AdapterIdTranslation
        | TranslationCoverageCategory::IgnoredSource => color_stdout(text, "2"),
        TranslationCoverageCategory::UntranslatedFallback => color_stdout(text, "31"),
        TranslationCoverageCategory::StaleDirectKey
        | TranslationCoverageCategory::StaleContextualKey
        | TranslationCoverageCategory::StaleTranslation
        | TranslationCoverageCategory::StaleNoChangeKey
        | TranslationCoverageCategory::StaleTargetAdaptation
        | TranslationCoverageCategory::StaleVariableKey
        | TranslationCoverageCategory::StaleAdapterIdKey
        | TranslationCoverageCategory::InvalidTargetAdaptation
        | TranslationCoverageCategory::StructuralFieldNameTranslation => color_stdout(text, "33"),
    }
}

fn color_stdout(text: &str, code: &str) -> String {
    if color_enabled(io::stdout().is_terminal()) {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

fn color_enabled(is_terminal: bool) -> bool {
    match env::var("BRAINBREW_COLOR") {
        Ok(value) if value == "always" => true,
        Ok(value) if value == "never" => false,
        _ => env::var_os("NO_COLOR").is_none() && is_terminal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_mode_uses_crlf_line_endings() {
        assert_eq!(terminal_line_end(false), "\n");
        assert_eq!(terminal_line_end(true), "\r\n");
    }

    #[test]
    fn raw_mode_rendering_returns_cursor_to_column_zero() {
        let mut input = io::empty();
        let mut output = Vec::new();
        {
            let mut ui = TerminalUi {
                reader: &mut input,
                writer: &mut output,
                raw_mode: true,
                max_option_rows: None,
                terminal_cols: None,
            };
            ui.render_select_one("Title", &["one".to_owned(), "two".to_owned()], 0)
                .unwrap();
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Title\r\n\r\n"));
        assert!(output.contains("  › one\r\n"));
        assert!(output.contains("    two\r\n"));
    }

    #[test]
    fn long_raw_mode_lists_scroll_to_keep_selection_visible() {
        let mut input = io::empty();
        let mut output = Vec::new();
        {
            let mut ui = TerminalUi {
                reader: &mut input,
                writer: &mut output,
                raw_mode: true,
                max_option_rows: Some(3),
                terminal_cols: Some(40),
            };
            let options = (1..=10)
                .map(|index| format!("option-{index}"))
                .collect::<Vec<_>>();
            ui.render_select_one("Title", &options, 8).unwrap();
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Showing 8–10 of 10\r\n"));
        assert!(!output.contains("option-1\r\n"));
        assert!(output.contains("  › option-9\r\n"));
    }

    #[test]
    fn long_options_are_truncated_to_prevent_wrapping() {
        assert_eq!(truncate_chars("abcdef", 4), "abc…");
        let mut input = io::empty();
        let mut output = Vec::new();
        {
            let mut ui = TerminalUi {
                reader: &mut input,
                writer: &mut output,
                raw_mode: true,
                max_option_rows: None,
                terminal_cols: Some(14),
            };
            ui.render_select_one("Title", &["very-long-option".to_owned()], 0)
                .unwrap();
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("  › very-long…\r\n"));
    }
}
