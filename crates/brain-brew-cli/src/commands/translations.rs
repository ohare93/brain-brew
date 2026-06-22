use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use brain_brew_core::{
    CanonicalDeck, Overlay, OverlayKind, TranslationCoverageCategory, TranslationCoverageEntry,
    TranslationCoverageReport,
};
use brain_brew_formats::manifest::FederatedDeckManifest;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
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
        let raw_mode = io::stdin().is_terminal() && io::stdout().is_terminal();
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = stdin.lock();
        let mut writer = stdout.lock();
        configure_interactively(&mut args, &mut reader, &mut writer, raw_mode)?;
    }

    let reports = collect_translation_reports(&args)?;

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
    language: Option<String>,
    overlay: Option<String>,
    note: Option<String>,
    field: Option<String>,
    path_prefixes: Vec<String>,
    apply: bool,
    json_output: bool,
    interactive: Option<bool>,
    full: bool,
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
        full: false,
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
            "--full" => {
                parsed.full = true;
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

fn configure_interactively<R: Read, W: Write>(
    args: &mut TranslationArgs,
    reader: &mut R,
    writer: &mut W,
    raw_mode: bool,
) -> Result<(), String> {
    let mut ui = TerminalUi::new(reader, writer, raw_mode)?;
    ui.message("Brain Brew translation coverage")?;

    if !args.manifest_path.exists() {
        let manifests = discover_nearby_manifests();
        if manifests.is_empty() {
            return Err(missing_manifest_error(&args.manifest_path));
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
    no_change_direct: BTreeSet<String>,
    no_change_contextual: BTreeMap<String, BTreeSet<String>>,
    ignore_paths: BTreeSet<String>,
}

impl OverlayEdits {
    fn is_empty(&self) -> bool {
        self.direct.is_empty()
            && self.contextual.is_empty()
            && self.no_change_direct.is_empty()
            && self.no_change_contextual.is_empty()
            && self.ignore_paths.is_empty()
    }

    fn count(&self) -> usize {
        self.direct.len()
            + self.contextual.values().map(BTreeSet::len).sum::<usize>()
            + self.no_change_direct.len()
            + self
                .no_change_contextual
                .values()
                .map(BTreeSet::len)
                .sum::<usize>()
            + self.ignore_paths.len()
    }
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
                "mark direct no-change for all {} selected missing translations",
                selected_missing.len()
            ),
            format!(
                "mark contextual no-change for all {} selected missing translations",
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
            0 => MissingApplyAction::NoChangeDirect,
            1 => MissingApplyAction::NoChangeContextual,
            2 => MissingApplyAction::Direct,
            3 => MissingApplyAction::Contextual,
            4 => MissingApplyAction::IgnorePath,
            5 => MissingApplyAction::DecidePerRow,
            6 => MissingApplyAction::Skip,
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
                    "{} at {} is stale/invalid; no safe automatic rewrite is applied, skipping.",
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
    NoChangeDirect,
    NoChangeContextual,
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
        "mark direct no-change".to_owned(),
        format!("mark contextual no-change at {context}"),
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
        0 => Ok(MissingApplyAction::NoChangeDirect),
        1 => Ok(MissingApplyAction::NoChangeContextual),
        2 => Ok(MissingApplyAction::Direct),
        3 => Ok(MissingApplyAction::Contextual),
        4 => Ok(MissingApplyAction::IgnorePath),
        5 => Ok(MissingApplyAction::Skip),
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
        MissingApplyAction::NoChangeDirect => {
            file_edits.no_change_direct.insert(entry.source.clone());
        }
        MissingApplyAction::NoChangeContextual => {
            file_edits
                .no_change_contextual
                .entry(contextual_context_for_entry(entry))
                .or_default()
                .insert(entry.source.clone());
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
                    TranslationCoverageCategory::StaleNoChangeDirectKey => {
                        translations.no_change.direct.remove(&entry.source);
                    }
                    TranslationCoverageCategory::StaleNoChangeContextualKey => {
                        if let Some(context) = &entry.context
                            && let Some(sources) =
                                translations.no_change.contextual.get_mut(context)
                        {
                            sources.remove(&entry.source);
                            if sources.is_empty() {
                                translations.no_change.contextual.remove(context);
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
        "`brainbrew translations` reports source-keyed translation dictionaries such as `translations.direct`, `translations.contextual`, and `translations.target_additions`."
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
        let plan = plan_manifest_target_with_packages(
            &args.manifest_path,
            &target,
            &args.include_paths,
            &args.package_roots,
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
            color_stdout("contextual overrides", "36"),
            all_counts.get("contextual_override").copied().unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("target-language additions", "35"),
            all_counts
                .get("target_language_addition")
                .copied()
                .unwrap_or(0)
        );
        println!(
            "  {}: {}",
            color_stdout("intentionally unchanged text", "34"),
            all_counts.get("no_change_direct").copied().unwrap_or(0)
                + all_counts.get("no_change_contextual").copied().unwrap_or(0)
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
            println!(
                "  - {} {} source={} translated={}",
                color_category(entry.category, display_category_name(entry.category)),
                entry.path,
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
            | TranslationCoverageCategory::StaleNoChangeDirectKey
            | TranslationCoverageCategory::StaleNoChangeContextualKey
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

fn category_counts_refs(entries: &[&TranslationCoverageEntry]) -> BTreeMap<&'static str, usize> {
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
    if !edits.no_change_direct.is_empty() || !edits.no_change_contextual.is_empty() {
        output = insert_no_change_lines(
            &output,
            &edits.no_change_direct,
            &edits.no_change_contextual,
        );
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

fn insert_no_change_lines(
    input: &str,
    direct: &BTreeSet<String>,
    contextual: &BTreeMap<String, BTreeSet<String>>,
) -> String {
    let spans = line_spans(input);
    let Some(translations_index) = find_translations_line(input, &spans) else {
        return insert_new_no_change_section(input, direct, contextual);
    };
    if find_translation_section(input, &spans, translations_index, "no_change").is_none() {
        return insert_new_no_change_section(input, direct, contextual);
    }

    let mut output = input.to_owned();
    if !direct.is_empty() {
        let entries = direct
            .iter()
            .map(|source| format!("      - {}\n", yaml_scalar(source)))
            .collect::<String>();
        output = insert_no_change_subsection(&output, "direct", &entries, &[]);
    }
    if !contextual.is_empty() {
        let mut entries = String::new();
        for (context, sources) in contextual {
            entries.push_str(&format!("      {}:\n", yaml_scalar(context)));
            for source in sources {
                entries.push_str(&format!("        - {}\n", yaml_scalar(source)));
            }
        }
        output = insert_no_change_subsection(&output, "contextual", &entries, &["direct"]);
    }
    output
}

fn insert_new_no_change_section(
    input: &str,
    direct: &BTreeSet<String>,
    contextual: &BTreeMap<String, BTreeSet<String>>,
) -> String {
    let mut entries = String::new();
    if !direct.is_empty() {
        entries.push_str("    direct:\n");
        for source in direct {
            entries.push_str(&format!("      - {}\n", yaml_scalar(source)));
        }
    }
    if !contextual.is_empty() {
        entries.push_str("    contextual:\n");
        for (context, sources) in contextual {
            entries.push_str(&format!("      {}:\n", yaml_scalar(context)));
            for source in sources {
                entries.push_str(&format!("        - {}\n", yaml_scalar(source)));
            }
        }
    }
    insert_translation_section(
        input,
        "no_change",
        &entries,
        &["contextual", "direct", "ignore_paths", "require_complete"],
    )
}

fn insert_no_change_subsection(
    input: &str,
    subsection_name: &str,
    entries: &str,
    preferred_after_subsections: &[&str],
) -> String {
    let spans = line_spans(input);
    let Some(translations_index) = find_translations_line(input, &spans) else {
        return input.to_owned();
    };
    let Some(no_change_index) =
        find_translation_section(input, &spans, translations_index, "no_change")
    else {
        return input.to_owned();
    };
    let no_change_end = translation_section_end(input, &spans, no_change_index);

    if let Some(section_index) = find_no_change_subsection(
        input,
        &spans,
        no_change_index,
        no_change_end,
        subsection_name,
    ) {
        let insert_at = indented_section_end(input, &spans, section_index, 4);
        return insert_at_byte(input, insert_at, entries);
    }

    for anchor in preferred_after_subsections {
        if let Some(section_index) =
            find_no_change_subsection(input, &spans, no_change_index, no_change_end, anchor)
        {
            let insert_at = indented_section_end(input, &spans, section_index, 4);
            return insert_at_byte(
                input,
                insert_at,
                &format!("    {subsection_name}:\n{entries}"),
            );
        }
    }

    let insert_at = spans[no_change_index].1;
    insert_at_byte(
        input,
        insert_at,
        &format!("    {subsection_name}:\n{entries}"),
    )
}

fn find_no_change_subsection(
    input: &str,
    spans: &[(usize, usize)],
    no_change_index: usize,
    no_change_end: usize,
    subsection_name: &str,
) -> Option<usize> {
    let needle = format!("    {subsection_name}:");
    spans
        .iter()
        .enumerate()
        .skip(no_change_index + 1)
        .take_while(|(_, (start, _))| *start < no_change_end)
        .find(|(_, (start, end))| input[*start..*end].trim_end() == needle)
        .map(|(index, _)| index)
}

fn indented_section_end(
    input: &str,
    spans: &[(usize, usize)],
    section_index: usize,
    section_indent: usize,
) -> usize {
    spans
        .iter()
        .enumerate()
        .skip(section_index + 1)
        .find(|(_, (start, end))| {
            let line = &input[*start..*end];
            !line.trim().is_empty()
                && !line.trim_start().starts_with('#')
                && leading_spaces(line) <= section_indent
        })
        .map(|(_, (start, _))| *start)
        .unwrap_or(input.len())
}

fn leading_spaces(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
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
    let Some(language) = candidate else {
        return;
    };
    if (2..=8).contains(&language.len())
        && language
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch == '-')
        && language != "translation"
    {
        languages.insert(language.to_owned());
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
        TranslationCoverageCategory::NoChangeDirect
        | TranslationCoverageCategory::NoChangeContextual => color_stdout(text, "34"),
        TranslationCoverageCategory::TargetLanguageAddition => color_stdout(text, "35"),
        TranslationCoverageCategory::VariableTranslation
        | TranslationCoverageCategory::AdapterIdTranslation
        | TranslationCoverageCategory::IgnoredSource => color_stdout(text, "2"),
        TranslationCoverageCategory::UntranslatedFallback => color_stdout(text, "31"),
        TranslationCoverageCategory::StaleDirectKey
        | TranslationCoverageCategory::StaleContextualKey
        | TranslationCoverageCategory::StaleNoChangeDirectKey
        | TranslationCoverageCategory::StaleNoChangeContextualKey
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
