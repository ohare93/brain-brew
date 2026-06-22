use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use brain_brew_core::{
    CanonicalDeck, Overlay, TranslationCoverageCategory, TranslationCoverageEntry,
    TranslationCoverageReport,
};
use brain_brew_formats::manifest::FederatedDeckManifest;
use serde_json::json;

use crate::help;
use crate::io::{PlannedOverlay, plan_manifest_target_with_packages, read_manifest};
use crate::output;

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
        return Err(missing_manifest_error(&args.manifest_path));
    }

    if interactive {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut writer = stdout.lock();
        configure_interactively(&mut args, &mut reader, &mut writer)?;
    }

    let reports = collect_translation_reports(&args)?;

    let mut edits_by_file = BTreeMap::<PathBuf, OverlayEdits>::new();
    if args.apply {
        if interactive {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let mut reader = stdin.lock();
            let mut writer = stdout.lock();
            edits_by_file = prompt_selective_apply(&reports, &mut reader, &mut writer)?;
        } else {
            edits_by_file = direct_stub_edits_from_reports(&reports);
        }
    }

    let mut applied = BTreeMap::new();
    for (path, edits) in &edits_by_file {
        let count = apply_overlay_edits(path, edits)?;
        if count > 0 {
            applied.insert(path.display().to_string(), count);
        }
    }

    if args.json_output {
        print_json_reports(&reports, &applied);
    } else {
        print_human_reports(&reports);
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
    language: Option<String>,
    overlay: Option<String>,
    note: Option<String>,
    field: Option<String>,
    path_prefixes: Vec<String>,
    apply: bool,
    json_output: bool,
    interactive: Option<bool>,
}

fn parse_translation_args(args: &[String]) -> Result<TranslationArgs, String> {
    let mut parsed = TranslationArgs {
        manifest_path: PathBuf::from("brainbrew.yaml"),
        target: None,
        all_targets: false,
        include_paths: Vec::new(),
        package_roots: Vec::new(),
        language: None,
        overlay: None,
        note: None,
        field: None,
        path_prefixes: Vec::new(),
        apply: false,
        json_output: false,
        interactive: None,
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
    Ok(parsed)
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

fn configure_interactively<R: BufRead, W: Write>(
    args: &mut TranslationArgs,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    writeln!(
        writer,
        "{}",
        color_stdout("Brain Brew translation coverage", "1;36")
    )
    .map_err(|error| error.to_string())?;
    writeln!(writer).map_err(|error| error.to_string())?;

    if !args.manifest_path.exists() {
        let manifests = discover_nearby_manifests();
        if manifests.is_empty() {
            return Err(missing_manifest_error(&args.manifest_path));
        }
        let labels = manifests
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let choice = choose_index(reader, writer, "Manifest", &labels, 0)?;
        args.manifest_path = manifests[choice].clone();
    }

    let manifest = read_manifest(&args.manifest_path)?;
    if args.target.is_none() && !args.all_targets {
        let mut target_values = manifest.targets.keys().cloned().collect::<Vec<_>>();
        target_values.sort();
        let mut labels = target_values.clone();
        labels.push("all targets".to_owned());
        let choice = choose_index(reader, writer, "Target", &labels, 0)?;
        if choice == target_values.len() {
            args.all_targets = true;
        } else {
            args.target = Some(target_values[choice].clone());
        }
    }

    if args.language.is_none() {
        let languages = inferred_languages(&manifest);
        if !languages.is_empty() {
            let mut labels = vec!["all languages".to_owned()];
            labels.extend(languages.iter().cloned());
            let choice = choose_index(reader, writer, "Language filter", &labels, 0)?;
            if choice > 0 {
                args.language = Some(languages[choice - 1].clone());
            }
        }
    }

    if args.overlay.is_none() {
        let overlays = translation_overlay_choices(args)?;
        if !overlays.is_empty() {
            let mut labels = vec!["all translation overlays".to_owned()];
            labels.extend(overlays.iter().map(|overlay| overlay.label.clone()));
            let choice = choose_index(reader, writer, "Translation overlay", &labels, 0)?;
            if choice > 0 {
                args.overlay = Some(overlays[choice - 1].id.clone());
            }
        }
    }

    if args.note.is_none() && args.field.is_none() && args.path_prefixes.is_empty() {
        let reports = collect_translation_reports(args)?;
        configure_scope_interactively(args, &reports, reader, writer)?;
    }

    if !args.apply {
        let labels = vec![
            "report only".to_owned(),
            "apply selected source→source stubs".to_owned(),
        ];
        let choice = choose_index(reader, writer, "Mode", &labels, 0)?;
        if choice == 1 {
            args.apply = true;
        }
    }

    writeln!(writer).map_err(|error| error.to_string())?;
    writeln!(writer, "Equivalent command:").map_err(|error| error.to_string())?;
    writeln!(writer, "  {}", equivalent_command(args)).map_err(|error| error.to_string())?;
    writeln!(writer).map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn configure_scope_interactively<R: BufRead, W: Write>(
    args: &mut TranslationArgs,
    reports: &[ScopedTranslationReport],
    reader: &mut R,
    writer: &mut W,
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
    let choice = choose_index(reader, writer, "Scope", &labels, 0)?;
    match choice {
        0 => {}
        1 => {
            let note_labels = notes.into_iter().collect::<Vec<_>>();
            if note_labels.is_empty() {
                writeln!(writer, "No note paths were found; using whole overlay.")
                    .map_err(|error| error.to_string())?;
            } else {
                let choice = choose_index(reader, writer, "Note", &note_labels, 0)?;
                args.note = Some(note_labels[choice].clone());
            }
        }
        2 => {
            let field_labels = fields.into_iter().collect::<Vec<_>>();
            if field_labels.is_empty() {
                writeln!(writer, "No field paths were found; using whole overlay.")
                    .map_err(|error| error.to_string())?;
            } else {
                let choice = choose_index(reader, writer, "Field", &field_labels, 0)?;
                args.field = Some(field_labels[choice].clone());
            }
        }
        3 => {
            if !problem_paths.is_empty() {
                writeln!(writer, "Known problem paths:").map_err(|error| error.to_string())?;
                for path in problem_paths.iter().take(12) {
                    writeln!(writer, "  - {path}").map_err(|error| error.to_string())?;
                }
            }
            let prefix = prompt_line(reader, writer, "Path prefix")?;
            if !prefix.trim().is_empty() {
                args.path_prefixes.push(prefix.trim().to_owned());
            }
        }
        4 => {
            let prefixes = prompt_line(
                reader,
                writer,
                "Paste changed deck path prefixes, comma-separated",
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

fn choose_index<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    options: &[String],
    default_index: usize,
) -> Result<usize, String> {
    if options.is_empty() {
        return Err(format!("no {label} options are available"));
    }
    writeln!(writer, "{label}:").map_err(|error| error.to_string())?;
    for (index, option) in options.iter().enumerate() {
        let marker = if index == default_index { ">" } else { " " };
        writeln!(writer, "  {marker} {}. {option}", index + 1)
            .map_err(|error| error.to_string())?;
    }
    loop {
        write!(writer, "Select {label} [{}]: ", default_index + 1)
            .map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
        let answer = read_trimmed_line(reader)?;
        if answer.is_empty() {
            return Ok(default_index);
        }
        if let Ok(number) = answer.parse::<usize>()
            && (1..=options.len()).contains(&number)
        {
            return Ok(number - 1);
        }
        if let Some((index, _)) = options
            .iter()
            .enumerate()
            .find(|(_, option)| option.eq_ignore_ascii_case(&answer))
        {
            return Ok(index);
        }
        writeln!(writer, "Enter a number between 1 and {}.", options.len())
            .map_err(|error| error.to_string())?;
    }
}

fn prompt_line<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
) -> Result<String, String> {
    write!(writer, "{prompt}: ").map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())?;
    read_trimmed_line(reader)
}

fn read_trimmed_line<R: BufRead>(reader: &mut R) -> Result<String, String> {
    let mut input = String::new();
    let bytes = reader
        .read_line(&mut input)
        .map_err(|error| error.to_string())?;
    if bytes == 0 {
        return Err("interactive input ended before a selection was made".to_owned());
    }
    Ok(input.trim().to_owned())
}

#[derive(Clone)]
struct ScopedTranslationReport {
    target: String,
    overlay_id: String,
    overlay_file: String,
    overlay_path: PathBuf,
    report: TranslationCoverageReport,
}

fn collect_translation_reports(
    args: &TranslationArgs,
) -> Result<Vec<ScopedTranslationReport>, String> {
    let manifest = read_manifest(&args.manifest_path)?;
    let target_names = selected_target_names(&manifest, args);

    let mut reports = Vec::new();
    for target in &target_names {
        let plan = plan_manifest_target_with_packages(
            &args.manifest_path,
            target,
            &args.include_paths,
            &args.package_roots,
        )?;
        let mut current = plan.base.clone();
        for (planned, overlay) in &plan.overlays {
            if overlay.translations.is_some() {
                if overlay_matches_scope(target, planned, overlay, args)
                    && let Some(report) = current.translation_coverage(overlay)
                {
                    let entries = report
                        .entries
                        .into_iter()
                        .filter(|entry| entry_matches_scope(entry, args))
                        .collect::<Vec<_>>();
                    reports.push(ScopedTranslationReport {
                        target: target.clone(),
                        overlay_id: planned.id.clone(),
                        overlay_file: planned.display_file.clone(),
                        overlay_path: planned.file.clone(),
                        report: TranslationCoverageReport {
                            overlay_id: report.overlay_id,
                            entries,
                        },
                    });
                }
                current = compose_lenient_translation_overlay(&current, overlay)?;
            } else {
                current = current
                    .compose(std::slice::from_ref(overlay))
                    .map_err(|error| {
                        format!(
                            "failed to compose overlay {} for target {target}: {error}",
                            planned.id
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
    ignore_paths: BTreeSet<String>,
}

impl OverlayEdits {
    fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.contextual.is_empty() && self.ignore_paths.is_empty()
    }

    fn count(&self) -> usize {
        self.direct.len()
            + self.contextual.values().map(BTreeSet::len).sum::<usize>()
            + self.ignore_paths.len()
    }
}

fn direct_stub_edits_from_reports(
    reports: &[ScopedTranslationReport],
) -> BTreeMap<PathBuf, OverlayEdits> {
    let mut edits = BTreeMap::<PathBuf, OverlayEdits>::new();
    for report in reports {
        let stubs = report
            .report
            .entries
            .iter()
            .filter(|entry| entry.category == TranslationCoverageCategory::UntranslatedFallback)
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

fn prompt_selective_apply<R: BufRead, W: Write>(
    reports: &[ScopedTranslationReport],
    reader: &mut R,
    writer: &mut W,
) -> Result<BTreeMap<PathBuf, OverlayEdits>, String> {
    let rows = reports
        .iter()
        .enumerate()
        .flat_map(|(report_index, report)| {
            report
                .report
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.category.is_problem())
                .map(move |(entry_index, _)| (report_index, entry_index))
        })
        .collect::<Vec<_>>();

    if rows.is_empty() {
        writeln!(
            writer,
            "No missing, stale, or invalid translation rows to apply."
        )
        .map_err(|error| error.to_string())?;
        return Ok(BTreeMap::new());
    }

    writeln!(writer, "Selectable translation rows:").map_err(|error| error.to_string())?;
    for (row_index, (report_index, entry_index)) in rows.iter().enumerate() {
        let report = &reports[*report_index];
        let entry = &report.report.entries[*entry_index];
        writeln!(
            writer,
            "  {}. {} {} {} source={}",
            row_index + 1,
            report.target,
            report.overlay_id,
            entry.category.as_str(),
            yaml_scalar(&entry.source)
        )
        .map_err(|error| error.to_string())?;
        writeln!(writer, "     {}", entry.path).map_err(|error| error.to_string())?;
    }

    let selected = loop {
        let answer = prompt_line(
            reader,
            writer,
            "Select rows to edit (numbers, all, none) [all]",
        )?;
        let answer = if answer.is_empty() {
            "all".to_owned()
        } else {
            answer
        };
        match parse_row_selection(&answer, rows.len()) {
            Ok(selection) => break selection,
            Err(error) => writeln!(writer, "{error}").map_err(|error| error.to_string())?,
        }
    };

    let mut edits = BTreeMap::<PathBuf, OverlayEdits>::new();
    for row_index in selected {
        let (report_index, entry_index) = rows[row_index];
        let report = &reports[report_index];
        let entry = &report.report.entries[entry_index];
        match entry.category {
            TranslationCoverageCategory::UntranslatedFallback => {
                let context = contextual_context_for_entry(entry);
                writeln!(
                    writer,
                    "{} at {} source={}",
                    entry.category.as_str(),
                    entry.path,
                    yaml_scalar(&entry.source)
                )
                .map_err(|error| error.to_string())?;
                writeln!(writer, "  d. add direct source→source stub")
                    .map_err(|error| error.to_string())?;
                writeln!(
                    writer,
                    "  c. add contextual source→source stub at {context}"
                )
                .map_err(|error| error.to_string())?;
                writeln!(writer, "  i. add ignore path for this deck path")
                    .map_err(|error| error.to_string())?;
                writeln!(writer, "  s. skip").map_err(|error| error.to_string())?;
                let action =
                    prompt_action(reader, writer, "Action [d]", &["d", "c", "i", "s"], "d")?;
                let file_edits = edits.entry(report.overlay_path.clone()).or_default();
                match action.as_str() {
                    "d" => {
                        file_edits.direct.insert(entry.source.clone());
                    }
                    "c" => {
                        file_edits
                            .contextual
                            .entry(context)
                            .or_default()
                            .insert(entry.source.clone());
                    }
                    "i" => {
                        file_edits.ignore_paths.insert(entry.path.clone());
                    }
                    "s" => {}
                    _ => unreachable!(),
                }
            }
            _ => {
                writeln!(
                    writer,
                    "{} at {} is stale/invalid; no safe automatic rewrite is applied, skipping.",
                    entry.category.as_str(),
                    entry.path
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }

    edits.retain(|_, edits| !edits.is_empty());
    if edits.is_empty() {
        return Ok(edits);
    }

    let confirm = prompt_action(
        reader,
        writer,
        "Apply selected changes? [y]",
        &["y", "n"],
        "y",
    )?;
    if confirm == "y" {
        Ok(edits)
    } else {
        Ok(BTreeMap::new())
    }
}

fn parse_row_selection(input: &str, row_count: usize) -> Result<Vec<usize>, String> {
    let input = input.trim();
    if input.eq_ignore_ascii_case("none") || input.eq_ignore_ascii_case("n") {
        return Ok(Vec::new());
    }
    if input.eq_ignore_ascii_case("all") || input.eq_ignore_ascii_case("a") {
        return Ok((0..row_count).collect());
    }
    let mut selected = BTreeSet::new();
    for part in input.split(',') {
        let part = part.trim();
        let number = part
            .parse::<usize>()
            .map_err(|_| format!("invalid row selection {part:?}"))?;
        if !(1..=row_count).contains(&number) {
            return Err(format!("row {number} is outside 1..={row_count}"));
        }
        selected.insert(number - 1);
    }
    Ok(selected.into_iter().collect())
}

fn prompt_action<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    prompt: &str,
    valid: &[&str],
    default: &str,
) -> Result<String, String> {
    loop {
        let answer = prompt_line(reader, writer, prompt)?;
        let action = if answer.is_empty() {
            default.to_owned()
        } else {
            answer.to_ascii_lowercase()
        };
        if valid.iter().any(|candidate| *candidate == action) {
            return Ok(action);
        }
        writeln!(writer, "Choose one of: {}", valid.join(", "))
            .map_err(|error| error.to_string())?;
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

fn entry_matches_scope(entry: &TranslationCoverageEntry, args: &TranslationArgs) -> bool {
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

fn compose_lenient_translation_overlay(
    current: &CanonicalDeck,
    overlay: &Overlay,
) -> Result<CanonicalDeck, String> {
    let mut sanitized = overlay.clone();
    if let Some(translations) = &mut sanitized.translations {
        translations.require_complete = false;
        if let Some(report) = current.translation_coverage(overlay) {
            for entry in report.entries {
                match entry.category {
                    TranslationCoverageCategory::StaleDirectKey => {
                        translations.direct.remove(&entry.source);
                    }
                    TranslationCoverageCategory::StaleContextualKey => {
                        if let Some(context) = &entry.context
                            && let Some(replacements) = translations.contextual.get_mut(context)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.contextual.remove(context);
                            }
                        }
                    }
                    TranslationCoverageCategory::StaleTargetAddition
                    | TranslationCoverageCategory::InvalidTargetAddition => {
                        let path = entry.context.as_deref().unwrap_or(&entry.path);
                        translations.target_additions.remove(path);
                    }
                    TranslationCoverageCategory::StaleVariableKey => {
                        if let Some(variable_key) = &entry.context
                            && let Some(replacements) = translations.variables.get_mut(variable_key)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.variables.remove(variable_key);
                            }
                        }
                    }
                    TranslationCoverageCategory::StaleAdapterIdKey => {
                        if let Some(adapter_key) = &entry.context
                            && let Some(replacements) =
                                translations.adapter_ids.get_mut(adapter_key)
                        {
                            replacements.remove(&entry.source);
                            if replacements.is_empty() {
                                translations.adapter_ids.remove(adapter_key);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    current.compose(&[sanitized]).map_err(|error| {
        format!(
            "failed to compose translation overlay {}: {error}",
            overlay.id
        )
    })
}

fn print_human_reports(reports: &[ScopedTranslationReport]) {
    for report in reports {
        let counts = category_counts(&report.report.entries);
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
            counts.get("direct_translation").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("contextual overrides", "36"),
            counts.get("contextual_override").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("target-language additions", "35"),
            counts.get("target_language_addition").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("ignored entries", "2"),
            counts.get("ignored_source").copied().unwrap_or(0)
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
        println!(
            "  {}: {}",
            color_stdout("missing/untranslated fallbacks", "31"),
            counts.get("untranslated_fallback").copied().unwrap_or(0)
        );
        for entry in report.report.problem_entries() {
            println!(
                "  - {} {} source={} translated={}",
                color_category(entry.category, entry.category.as_str()),
                entry.path,
                yaml_scalar(&entry.source),
                entry
                    .translated
                    .as_deref()
                    .map(yaml_scalar)
                    .unwrap_or_else(|| "''".to_owned())
            );
        }
    }
}

fn is_stale_or_invalid(category: TranslationCoverageCategory) -> bool {
    matches!(
        category,
        TranslationCoverageCategory::StaleDirectKey
            | TranslationCoverageCategory::StaleContextualKey
            | TranslationCoverageCategory::StaleTargetAddition
            | TranslationCoverageCategory::StaleVariableKey
            | TranslationCoverageCategory::StaleAdapterIdKey
            | TranslationCoverageCategory::InvalidTargetAddition
    )
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
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "reports": reports_json,
            "applied": applied,
        }))
        .expect("translation report JSON serializes")
    );
}

fn entry_json(entry: &TranslationCoverageEntry) -> serde_json::Value {
    json!({
        "category": entry.category.as_str(),
        "path": entry.path,
        "source": entry.source,
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

fn apply_overlay_edits(path: &Path, edits: &OverlayEdits) -> Result<usize, String> {
    if edits.is_empty() {
        return Ok(0);
    }
    let input = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut output = input.clone();
    if !edits.ignore_paths.is_empty() {
        output = insert_ignore_path_lines(&output, &edits.ignore_paths);
    }
    if !edits.direct.is_empty() {
        output = insert_direct_stub_lines(&output, &edits.direct);
    }
    if !edits.contextual.is_empty() {
        output = insert_contextual_stub_lines(&output, &edits.contextual);
    }
    if output != input {
        fs::write(path, output).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(edits.count())
}

fn insert_direct_stub_lines(input: &str, stubs: &BTreeSet<String>) -> String {
    let entries = stubs
        .iter()
        .map(|source| format!("    {}: {}\n", yaml_scalar(source), yaml_scalar(source)))
        .collect::<String>();
    insert_translation_section(
        input,
        "direct",
        &entries,
        &["ignore_paths", "require_complete"],
    )
}

fn insert_contextual_stub_lines(input: &str, stubs: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut entries = String::new();
    for (context, sources) in stubs {
        entries.push_str(&format!("    {}:\n", yaml_scalar(context)));
        for source in sources {
            entries.push_str(&format!(
                "      {}: {}\n",
                yaml_scalar(source),
                yaml_scalar(source)
            ));
        }
    }
    insert_translation_section(
        input,
        "contextual",
        &entries,
        &["direct", "ignore_paths", "require_complete"],
    )
}

fn insert_ignore_path_lines(input: &str, paths: &BTreeSet<String>) -> String {
    let entries = paths
        .iter()
        .map(|path| format!("    - {}\n", yaml_scalar(path)))
        .collect::<String>();
    insert_translation_section(input, "ignore_paths", &entries, &["require_complete"])
}

fn insert_translation_section(
    input: &str,
    section_name: &str,
    entries: &str,
    preferred_after_sections: &[&str],
) -> String {
    let spans = line_spans(input);
    let Some(translations_index) = find_translations_line(input, &spans) else {
        let insert_after = spans
            .iter()
            .find(|(start, end)| input[*start..*end].trim_start().starts_with("kind:"))
            .map(|(_, end)| *end)
            .or_else(|| spans.first().map(|(_, end)| *end))
            .unwrap_or(0);
        return insert_at_byte(
            input,
            insert_after,
            &format!("translations:\n  {section_name}:\n{entries}"),
        );
    };

    if let Some(section_index) =
        find_translation_section(input, &spans, translations_index, section_name)
    {
        let insert_at = translation_section_end(input, &spans, section_index);
        return insert_at_byte(input, insert_at, entries);
    }

    for anchor in preferred_after_sections {
        if let Some(section_index) =
            find_translation_section(input, &spans, translations_index, anchor)
        {
            let insert_at = translation_section_end(input, &spans, section_index);
            return insert_at_byte(input, insert_at, &format!("  {section_name}:\n{entries}"));
        }
    }

    let insert_at = spans[translations_index].1;
    insert_at_byte(input, insert_at, &format!("  {section_name}:\n{entries}"))
}

fn find_translations_line(input: &str, spans: &[(usize, usize)]) -> Option<usize> {
    spans.iter().position(|(start, end)| {
        let line = &input[*start..*end];
        line.trim_end() == "translations:"
    })
}

fn find_translation_section(
    input: &str,
    spans: &[(usize, usize)],
    translations_index: usize,
    section_name: &str,
) -> Option<usize> {
    let needle = format!("  {section_name}:");
    spans
        .iter()
        .enumerate()
        .skip(translations_index + 1)
        .take_while(|(_, (start, end))| {
            let line = &input[*start..*end];
            line.trim().is_empty() || line.trim_start().starts_with('#') || line.starts_with(' ')
        })
        .find(|(_, (start, end))| input[*start..*end].trim_end() == needle)
        .map(|(index, _)| index)
}

fn translation_section_end(input: &str, spans: &[(usize, usize)], section_index: usize) -> usize {
    spans
        .iter()
        .enumerate()
        .skip(section_index + 1)
        .find(|(_, (start, end))| {
            let line = &input[*start..*end];
            !line.trim().is_empty()
                && !line.trim_start().starts_with('#')
                && ((line.starts_with("  ") && !line.starts_with("    ")) || !line.starts_with(' '))
        })
        .map(|(_, (start, _))| *start)
        .unwrap_or(input.len())
}

fn insert_at_byte(input: &str, index: usize, insertion: &str) -> String {
    let mut output = String::with_capacity(input.len() + insertion.len() + 1);
    output.push_str(&input[..index]);
    if index > 0 && !input[..index].ends_with('\n') {
        output.push('\n');
    }
    output.push_str(insertion);
    output.push_str(&input[index..]);
    output
}

fn line_spans(input: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (index, ch) in input.char_indices() {
        if ch == '\n' {
            spans.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < input.len() || input.is_empty() {
        spans.push((start, input.len()));
    }
    spans
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
        let plan = plan_manifest_target_with_packages(
            &args.manifest_path,
            &target,
            &args.include_paths,
            &args.package_roots,
        )?;
        for (planned, overlay) in &plan.overlays {
            if overlay.translations.is_some()
                && args
                    .language
                    .as_ref()
                    .is_none_or(|language| language_matches(language, &target, planned, overlay))
            {
                choices.insert(OverlayChoice {
                    id: planned.id.clone(),
                    label: format!("{}  {}", planned.id, planned.display_file),
                });
            }
        }
    }
    Ok(choices.into_iter().collect())
}

fn inferred_languages(manifest: &FederatedDeckManifest) -> Vec<String> {
    let mut languages = BTreeSet::new();
    for target in manifest.targets.keys() {
        if let Some((language, _)) = target.split_once('-')
            && (2..=8).contains(&language.len())
            && language.chars().all(|ch| ch.is_ascii_lowercase())
        {
            languages.insert(language.to_owned());
        }
    }
    for (id, overlay) in &manifest.overlays {
        if let Some(language) = id.rsplit(['.', '-']).next()
            && (2..=8).contains(&language.len())
            && language.chars().all(|ch| ch.is_ascii_lowercase())
        {
            languages.insert(language.to_owned());
        }
        if let Some(stem) = Path::new(&overlay.file)
            .file_stem()
            .and_then(|stem| stem.to_str())
            && (2..=8).contains(&stem.len())
            && stem.chars().all(|ch| ch.is_ascii_lowercase())
        {
            languages.insert(stem.to_owned());
        }
    }
    languages.into_iter().collect()
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

fn discover_nearby_manifests() -> Vec<PathBuf> {
    let mut results = BTreeSet::new();
    if let Ok(current) = env::current_dir() {
        discover_manifests_under(&current, &current, 0, &mut results);
        for ancestor in current.ancestors().skip(1).take(4) {
            let candidate = ancestor.join("brainbrew.yaml");
            if candidate.exists() {
                results.insert(relative_to_current(&candidate));
            }
        }
    }
    results.into_iter().take(20).collect()
}

fn discover_manifests_under(
    root: &Path,
    dir: &Path,
    depth: usize,
    results: &mut BTreeSet<PathBuf>,
) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == "target"
            || name == ".git"
            || name == ".jj"
            || name == ".devenv"
            || name == ".direnv"
            || name == "node_modules"
        {
            continue;
        }
        if path.is_file() && name == "brainbrew.yaml" {
            results.insert(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        } else if path.is_dir() {
            discover_manifests_under(root, &path, depth + 1, results);
        }
    }
}

fn relative_to_current(path: &Path) -> PathBuf {
    env::current_dir()
        .ok()
        .and_then(|current| path.strip_prefix(current).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

fn missing_manifest_error(path: &Path) -> String {
    let mut message = format!("No Brain Brew manifest found at {}.", path.display());
    if path == Path::new("brainbrew.yaml") {
        message.push_str("\n\nRun from a deck workspace, or pass --manifest <path>.");
    }
    let manifests = discover_nearby_manifests();
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
    message
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

fn yaml_scalar(value: &str) -> String {
    if !value.is_empty()
        && !value.starts_with([
            ' ', '-', '?', ':', '@', '`', '&', '*', '!', '|', '>', '#', '{', '[', ',',
        ])
        && !value.ends_with(' ')
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | ',' | '_' | '-' | '/' | ':')
        })
        && !value.chars().all(|ch| ch.is_ascii_digit())
        && !matches!(
            value,
            "true" | "false" | "True" | "False" | "TRUE" | "FALSE" | "null" | "Null" | "NULL"
        )
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

fn color_category(category: TranslationCoverageCategory, text: &str) -> String {
    match category {
        TranslationCoverageCategory::DirectTranslation => color_stdout(text, "32"),
        TranslationCoverageCategory::ContextualOverride => color_stdout(text, "36"),
        TranslationCoverageCategory::TargetLanguageAddition => color_stdout(text, "35"),
        TranslationCoverageCategory::VariableTranslation
        | TranslationCoverageCategory::AdapterIdTranslation
        | TranslationCoverageCategory::IgnoredSource => color_stdout(text, "2"),
        TranslationCoverageCategory::UntranslatedFallback => color_stdout(text, "31"),
        TranslationCoverageCategory::StaleDirectKey
        | TranslationCoverageCategory::StaleContextualKey
        | TranslationCoverageCategory::StaleTargetAddition
        | TranslationCoverageCategory::StaleVariableKey
        | TranslationCoverageCategory::StaleAdapterIdKey
        | TranslationCoverageCategory::InvalidTargetAddition => color_stdout(text, "33"),
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
