use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Task, Theme};
use serde_json::Value;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;

pub fn run() -> iced::Result {
    iced::application(WorkbenchApp::new, WorkbenchApp::update, WorkbenchApp::view)
        .title("Brain Brew Deck Workbench")
        .theme(theme)
        .run()
}

fn theme(_state: &WorkbenchApp) -> Theme {
    Theme::Light
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    pub manifest: String,
    pub language_count: usize,
    pub target_count: usize,
    pub fingerprint_count: usize,
}

impl WorkspaceSummary {
    pub fn from_workspace_json(value: &Value) -> Self {
        Self {
            manifest: value["manifest"]
                .as_str()
                .unwrap_or("unknown manifest")
                .to_owned(),
            language_count: value["languages"]
                .as_object()
                .map_or(0, serde_json::Map::len),
            target_count: value["targets"].as_object().map_or(0, serde_json::Map::len),
            fingerprint_count: value["fingerprints"].as_array().map_or(0, Vec::len),
        }
    }
}

#[derive(Debug)]
pub struct WorkbenchApp {
    workspace: Option<WorkspaceSummary>,
    note_pivot: Option<Value>,
    status: String,
}

#[derive(Clone, Debug)]
pub enum Message {
    RefreshWorkspace,
    WorkspaceLoaded(Result<WorkspaceSummary, String>),
    NotePivotLoaded(Result<Value, String>),
}

impl WorkbenchApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                workspace: None,
                note_pivot: None,
                status: "Loading workspace metadata…".to_owned(),
            },
            Task::batch([
                Task::perform(fetch_workspace(), Message::WorkspaceLoaded),
                Task::perform(fetch_note_pivot(), Message::NotePivotLoaded),
            ]),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshWorkspace => {
                self.status = "Refreshing workspace metadata…".to_owned();
                publish_workspace_probe("loading", self.status.as_str(), None);
                Task::batch([
                    Task::perform(fetch_workspace(), Message::WorkspaceLoaded),
                    Task::perform(fetch_note_pivot(), Message::NotePivotLoaded),
                ])
            }
            Message::WorkspaceLoaded(Ok(summary)) => {
                self.status = "Workspace metadata loaded from /api/workspace.".to_owned();
                publish_workspace_probe("loaded", self.status.as_str(), Some(&summary));
                self.workspace = Some(summary);
                Task::none()
            }
            Message::WorkspaceLoaded(Err(error)) => {
                self.status = format!("Unable to load workspace metadata: {error}");
                publish_workspace_probe("error", self.status.as_str(), None);
                Task::none()
            }
            Message::NotePivotLoaded(Ok(pivot)) => {
                self.status = "Note pivot loaded from /api/workbench/note-pivot.".to_owned();
                publish_note_pivot_panel(&pivot);
                self.note_pivot = Some(pivot);
                Task::none()
            }
            Message::NotePivotLoaded(Err(error)) => {
                self.status = format!("Unable to load note pivot: {error}");
                publish_note_pivot_error(self.status.as_str());
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let summary = self.workspace.as_ref();
        let manifest = summary
            .map(|workspace| workspace.manifest.as_str())
            .unwrap_or("waiting for /api/workspace");
        let language_count = summary.map_or("—".to_owned(), |workspace| {
            workspace.language_count.to_string()
        });
        let target_count = summary.map_or("—".to_owned(), |workspace| {
            workspace.target_count.to_string()
        });
        let fingerprint_count = summary.map_or("—".to_owned(), |workspace| {
            workspace.fingerprint_count.to_string()
        });
        let note_count = self
            .note_pivot
            .as_ref()
            .and_then(|pivot| pivot["notes"].as_array())
            .map_or("—".to_owned(), |notes| notes.len().to_string());

        let sidebar = panel(
            "Languages",
            column![
                text("Language dashboard").size(22),
                text(format!("{language_count} configured language(s)")),
                text("Target language, target, and overlay controls are available in the browser workbench panel."),
            ],
        )
        .width(Length::Fixed(260.0));

        let canvas = panel(
            "Deck canvas",
            column![
                text("Note pivot").size(28),
                text(format!(
                    "{note_count} note(s) loaded for target translation editing."
                )),
                text(format!("Manifest: {manifest}")),
                button("Refresh workspace metadata").on_press(Message::RefreshWorkspace),
            ],
        )
        .width(Length::Fill);

        let inspector = panel(
            "Inspector",
            column![
                text("Pending changes").size(22),
                text("Browser-local staged edits persist in localStorage until Apply."),
                text(format!("{target_count} target(s)")),
                text(format!("{fingerprint_count} watched file fingerprint(s)")),
            ],
        )
        .width(Length::Fixed(300.0));

        container(
            column![
                container(
                    row![
                        text("Brain Brew Deck Workbench").size(32),
                        text(self.status.as_str()).size(16),
                    ]
                    .spacing(24)
                    .align_y(iced::Alignment::Center),
                )
                .padding(20)
                .width(Length::Fill),
                row![sidebar, canvas, inspector]
                    .spacing(18)
                    .padding(18)
                    .height(Length::Fill),
            ]
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn panel<'a>(
    title: &'a str,
    body: iced::widget::Column<'a, Message>,
) -> iced::widget::Container<'a, Message> {
    container(scrollable(
        column![text(title).size(14), body.spacing(12)].spacing(16),
    ))
    .padding(18)
    .height(Length::Fill)
}

#[cfg(target_arch = "wasm32")]
fn publish_workspace_probe(status: &str, message: &str, workspace: Option<&WorkspaceSummary>) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(element) = document.get_element_by_id("brainbrew-workbench-e2e") else {
        return;
    };

    let _ = element.set_attribute("data-status", status);
    let _ = element.set_attribute("data-message", message);
    if let Some(workspace) = workspace {
        let language_count = workspace.language_count.to_string();
        let target_count = workspace.target_count.to_string();
        let fingerprint_count = workspace.fingerprint_count.to_string();
        let _ = element.set_attribute("data-manifest", workspace.manifest.as_str());
        let _ = element.set_attribute("data-language-count", language_count.as_str());
        let _ = element.set_attribute("data-target-count", target_count.as_str());
        let _ = element.set_attribute("data-fingerprint-count", fingerprint_count.as_str());
        element.set_text_content(Some(&format!(
            "Brain Brew Deck Workbench loaded {} language(s), {} target(s), and {} watched file(s) from {}",
            workspace.language_count,
            workspace.target_count,
            workspace.fingerprint_count,
            workspace.manifest
        )));
    } else {
        element.set_text_content(Some(message));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_workspace_probe(_status: &str, _message: &str, _workspace: Option<&WorkspaceSummary>) {}

#[cfg(target_arch = "wasm32")]
fn publish_note_pivot_error(message: &str) {
    if let Some(document) = web_sys::window().and_then(|window| window.document())
        && let Some(element) = document.get_element_by_id("workbench-dom-panel")
    {
        element.set_inner_html(&format!(
            "<section class=\"workbench-error\">{}</section>",
            escape_html(message)
        ));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_note_pivot_error(_message: &str) {}

#[cfg(target_arch = "wasm32")]
fn publish_note_pivot_panel(pivot: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let panel = document
        .get_element_by_id("workbench-dom-panel")
        .or_else(|| {
            let element = document.create_element("section").ok()?;
            element.set_id("workbench-dom-panel");
            document.body()?.append_child(&element).ok()?;
            Some(element)
        });
    let Some(panel) = panel else {
        return;
    };

    let language = pivot["language"]["code"].as_str().unwrap_or("");
    let target = pivot["target"]["label"].as_str().unwrap_or("");
    let overlay = pivot["overlay"]["label"].as_str().unwrap_or("");
    let progress = &pivot["progress"];
    let mut html = String::new();
    html.push_str(&format!(
        "<article class=\"workbench-panel\" data-language=\"{}\" data-target=\"{}\" data-overlay=\"{}\">",
        escape_html(language),
        escape_html(target),
        escape_html(overlay),
    ));
    html.push_str("<header class=\"workbench-panel__header\">");
    html.push_str("<h2>Deck Workbench Note pivot</h2>");
    html.push_str(&select_options_html(
        "language-select",
        "Language",
        &pivot["selection_options"]["languages"],
        "code",
        "display_name",
        language,
    ));
    html.push_str(&select_options_html(
        "target-select",
        "Target",
        &pivot["selection_options"]["targets"],
        "label",
        "label",
        target,
    ));
    html.push_str(&select_options_html(
        "overlay-select",
        "Overlay",
        &pivot["selection_options"]["overlays"],
        "label",
        "label",
        overlay,
    ));
    html.push_str("</header>");
    html.push_str(&format!(
        "<section id=\"translation-progress\" data-complete=\"{}\" data-total=\"{}\" data-missing=\"{}\" data-stale=\"{}\">Main note-field progress: {} / {} complete, {} missing, {} stale (<span id=\"staged-edit-count\">0</span> staged)</section>",
        progress["complete"].as_u64().unwrap_or(0),
        progress["total"].as_u64().unwrap_or(0),
        progress["missing"].as_u64().unwrap_or(0),
        progress["stale"].as_u64().unwrap_or(0),
        progress["complete"].as_u64().unwrap_or(0),
        progress["total"].as_u64().unwrap_or(0),
        progress["missing"].as_u64().unwrap_or(0),
        progress["stale"].as_u64().unwrap_or(0),
    ));
    let optional_progress = &pivot["optional_progress"];
    html.push_str(&format!(
        "<section id=\"optional-progress\" data-complete=\"{}\" data-total=\"{}\" data-missing=\"{}\" data-stale=\"{}\">Optional metadata: {} / {} complete, {} missing, {} stale</section>",
        optional_progress["complete"].as_u64().unwrap_or(0),
        optional_progress["total"].as_u64().unwrap_or(0),
        optional_progress["missing"].as_u64().unwrap_or(0),
        optional_progress["stale"].as_u64().unwrap_or(0),
        optional_progress["complete"].as_u64().unwrap_or(0),
        optional_progress["total"].as_u64().unwrap_or(0),
        optional_progress["missing"].as_u64().unwrap_or(0),
        optional_progress["stale"].as_u64().unwrap_or(0),
    ));
    html.push_str("<nav class=\"overlay-badges\">");
    for badge in pivot["overlay_badges"].as_array().into_iter().flatten() {
        let active = if badge["active"].as_bool().unwrap_or(false) {
            " active"
        } else {
            ""
        };
        html.push_str(&format!(
            "<span class=\"overlay-badge{}\">{}</span>",
            active,
            escape_html(badge["label"].as_str().unwrap_or("overlay"))
        ));
    }
    html.push_str("</nav>");
    html.push_str(&filter_buttons_html(pivot));
    html.push_str(&pane_layout_panel_html(pivot));
    html.push_str(&new_language_panel_html(pivot));

    if let Some(notes) = pivot["notes"].as_array() {
        if notes.is_empty() {
            html.push_str("<p>No notes match the active filter.</p>");
        } else {
            for note in notes {
                html.push_str(&note_html(pivot, note));
            }
        }
    } else {
        html.push_str("<p>No notes match the active filter.</p>");
    }

    html.push_str(&lazy_secondary_pivot_panels_html());
    html.push_str("<section class=\"apply-box\"><button id=\"apply-preview-button\" type=\"button\">Apply preview</button> <button id=\"apply-confirm-button\" type=\"button\">Confirm Apply</button><pre id=\"apply-preview-output\"></pre></section>");
    html.push_str("</article>");
    panel.set_inner_html(&html);

    register_control_handlers(pivot);
    register_pane_layout_handlers(pivot);
    register_new_language_handlers(pivot);
    register_field_handlers(pivot);
    register_lazy_pivot_load_handlers(pivot);
    register_apply_handlers(pivot);
    restore_staged_dom_state(pivot);
    update_staged_count_for_pivot(pivot);
    refresh_progress_from_dom();
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_note_pivot_panel(_pivot: &Value) {}

#[cfg(target_arch = "wasm32")]
fn lazy_secondary_pivot_panels_html() -> String {
    [
        lazy_secondary_pivot_panel_html(
            "card-pivot-panel",
            "card-pivot",
            "Card pivot",
            "Cards are loaded on demand so language changes stay responsive.",
            "load-card-pivot-button",
            "Load cards",
        ),
        lazy_secondary_pivot_panel_html(
            "source-string-pivot-panel",
            "source-string-pivot",
            "Source String pivot",
            "Reusable source strings are loaded only when you need that workflow.",
            "load-source-string-pivot-button",
            "Load source strings",
        ),
        lazy_secondary_pivot_panel_html(
            "optional-metadata-panel",
            "optional-metadata",
            "Optional metadata checklist",
            "Optional metadata rows are loaded only when you open the checklist.",
            "load-optional-metadata-button",
            "Load optional metadata",
        ),
    ]
    .join("")
}

#[cfg(target_arch = "wasm32")]
fn lazy_secondary_pivot_panel_html(
    panel_id: &str,
    class_name: &str,
    title: &str,
    description: &str,
    button_id: &str,
    button_label: &str,
) -> String {
    format!(
        "<section id=\"{}\" class=\"{} lazy-pivot-panel\" data-lazy-state=\"unloaded\" aria-live=\"polite\" aria-busy=\"false\"><h3>{}</h3><p>{}</p><button id=\"{}\" class=\"lazy-pivot-load-button\" type=\"button\">{}</button></section>",
        escape_html(panel_id),
        escape_html(class_name),
        escape_html(title),
        escape_html(description),
        escape_html(button_id),
        escape_html(button_label),
    )
}

#[cfg(target_arch = "wasm32")]
fn pane_layout_panel_html(pivot: &Value) -> String {
    let mut html = String::new();
    html.push_str("<section id=\"pane-layout-panel\" class=\"pane-layout-panel\">");
    html.push_str("<h3>Pane layout preset</h3>");
    html.push_str("<label><input id=\"source-pane-writable\" type=\"checkbox\"> Source pane writable</label> ");
    html.push_str("<label><input id=\"target-pane-writable\" type=\"checkbox\" checked> Selected target pane writable</label> ");
    html.push_str("<label>Additional target <select id=\"secondary-pane-language-select\">");
    for language in pivot["selection_options"]["languages"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let code = language["code"].as_str().unwrap_or("");
        if language["active"].as_bool().unwrap_or(false) {
            continue;
        }
        let label = language["display_name"].as_str().unwrap_or(code);
        html.push_str(&format!(
            "<option value=\"{}\">{}</option>",
            escape_html(code),
            escape_html(label)
        ));
    }
    html.push_str("</select></label> <button id=\"load-secondary-pane\" type=\"button\">Load target pane</button>");
    html.push_str("<div id=\"secondary-target-pane\"></div>");
    html.push_str("</section>");
    html
}

#[cfg(target_arch = "wasm32")]
fn new_language_panel_html(pivot: &Value) -> String {
    let mut html = String::new();
    html.push_str("<section id=\"new-language-panel\" class=\"new-language-panel\">");
    html.push_str("<h3>New language scaffold</h3>");
    html.push_str("<label>Template language <select id=\"new-language-template\">");
    for language in pivot["selection_options"]["languages"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let code = language["code"].as_str().unwrap_or("");
        let label = language["display_name"].as_str().unwrap_or(code);
        let selected = if language["active"].as_bool().unwrap_or(false) {
            " selected"
        } else {
            ""
        };
        html.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape_html(code),
            selected,
            escape_html(label)
        ));
    }
    html.push_str("</select></label>");
    html.push_str("<label>Code <input id=\"new-language-code\" placeholder=\"nb\"></label>");
    html.push_str("<label>Display name <input id=\"new-language-display-name\" placeholder=\"Norwegian Bokmal\"></label>");
    html.push_str(
        "<button id=\"new-language-preview-button\" type=\"button\">Preview new language</button> ",
    );
    html.push_str(
        "<button id=\"new-language-confirm-button\" type=\"button\">Create language</button>",
    );
    html.push_str("<div id=\"new-language-preview-output\" data-validation-ok=\"false\"></div>");
    html.push_str("</section>");
    html
}

#[cfg(target_arch = "wasm32")]
fn select_options_html(
    id: &str,
    label: &str,
    options: &Value,
    value_key: &str,
    label_key: &str,
    fallback: &str,
) -> String {
    let mut html = format!(
        "<label>{} <select id=\"{}\">",
        escape_html(label),
        escape_html(id)
    );
    let mut rendered_any = false;
    for option in options.as_array().into_iter().flatten() {
        rendered_any = true;
        let value = option[value_key].as_str().unwrap_or(fallback);
        let label = option[label_key].as_str().unwrap_or(value);
        let selected = if option["active"].as_bool().unwrap_or(false) {
            " selected"
        } else {
            ""
        };
        html.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape_html(value),
            selected,
            escape_html(label)
        ));
    }
    if !rendered_any {
        html.push_str(&format!(
            "<option value=\"{}\" selected>{}</option>",
            escape_html(fallback),
            escape_html(fallback)
        ));
    }
    html.push_str("</select></label>");
    html
}

#[cfg(target_arch = "wasm32")]
fn filter_buttons_html(pivot: &Value) -> String {
    let active = pivot["filters"]["active"].as_str().unwrap_or("all");
    let mut html = "<nav class=\"pivot-filters\" aria-label=\"Note pivot filters\">".to_owned();
    for (filter, label) in [
        ("all", "All"),
        ("missing", "Missing"),
        ("stale", "Stale"),
        ("needs_work", "Needs work"),
    ] {
        let class = if active == filter {
            " class=\"active\""
        } else {
            ""
        };
        html.push_str(&format!(
            "<button type=\"button\" data-filter=\"{}\"{}>{}</button>",
            filter, class, label
        ));
    }
    html.push_str("</nav>");
    html
}

#[cfg(target_arch = "wasm32")]
fn note_html(pivot: &Value, note: &Value) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        "<section class=\"note-card\" data-note-id=\"{}\"><h3>{}: {}</h3>",
        escape_html(note["note_id"].as_str().unwrap_or("")),
        escape_html(note["note_id"].as_str().unwrap_or("note")),
        escape_html(note["status"].as_str().unwrap_or("unknown"))
    ));
    html.push_str("<div class=\"preview-grid\"><section><h4>Source preview</h4>");
    html.push_str(&preview_html(&note["source_preview"]));
    html.push_str("</section><section><h4>Target preview</h4><div class=\"target-preview\">");
    html.push_str(&preview_html(&note["target_preview"]));
    html.push_str("</div></section></div>");
    html.push_str("<table class=\"field-editor\"><thead><tr><th>Field</th><th>Source / staged source edit</th><th>Target / staged edit</th><th>Status</th><th>Occurrences</th><th>Mode</th></tr></thead><tbody>");
    for field in note["fields"].as_array().into_iter().flatten() {
        let path = field["path"].as_str().unwrap_or("");
        let source = field["source"].as_str().unwrap_or("");
        let staged = staged_edit_for(pivot, path, source);
        let staged_source = staged_source_edit_for(pivot, path, source);
        let value = staged
            .as_ref()
            .and_then(|edit| edit["value"].as_str())
            .unwrap_or_else(|| field["target"].as_str().unwrap_or(""));
        let source_value = staged_source
            .as_ref()
            .and_then(|edit| edit["value"].as_str())
            .unwrap_or(source);
        let mode = staged
            .as_ref()
            .and_then(|edit| edit["mode"].as_str())
            .unwrap_or("direct");
        let source_scope = staged_source
            .as_ref()
            .and_then(|edit| edit["scope"].as_str())
            .unwrap_or("field");
        let source_impact = staged_source
            .as_ref()
            .and_then(|edit| edit["impact_action"].as_str())
            .unwrap_or("stale_record");
        let id = id_for_path(path);
        let editable = field["editable"].as_bool().unwrap_or(false);
        let source_editable = field["source_editable"].as_bool().unwrap_or(false);
        let readonly = if editable { "" } else { " readonly" };
        let source_readonly = if staged_source.is_some() {
            ""
        } else {
            " readonly"
        };
        let source_controls_disabled = if source_editable { "" } else { " disabled" };
        let target_status = staged
            .as_ref()
            .and_then(|edit| edit["mode"].as_str())
            .map(|mode| format!("staged_{mode}"));
        let status = target_status
            .or_else(|| staged_source.as_ref().map(|_| "staged_source".to_owned()))
            .unwrap_or_else(|| field["status"].as_str().unwrap_or("unknown").to_owned());
        let occurrence_count = field["occurrence_count"].as_u64().unwrap_or(1);
        html.push_str(&format!(
            "<tr data-field-path=\"{}\" data-note-id=\"{}\" data-field-id=\"{}\" data-editable=\"{}\" data-original-status=\"{}\"><td>{}</td><td class=\"source-text\"><span id=\"source-text-{}\">{}</span><div class=\"source-edit-controls\"><button id=\"source-edit-toggle-{}\" type=\"button\"{}>Edit source</button><input id=\"source-input-{}\" value=\"{}\" data-path=\"{}\" data-source=\"{}\" data-note-id=\"{}\" data-field-id=\"{}\"{}{} /></div></td><td><input id=\"translation-input-{}\" value=\"{}\" data-path=\"{}\" data-source=\"{}\" data-note-id=\"{}\" data-field-id=\"{}\"{} /><div id=\"target-text-{}\">{}</div></td><td id=\"status-text-{}\">{}</td><td>{} occurrence(s)<br><select id=\"source-scope-{}\"{}><option value=\"field\"{}>This field only</option><option value=\"all_occurrences\"{}>All occurrences</option></select></td><td><select id=\"translation-mode-{}\"{}><option value=\"direct\"{}>Direct</option><option value=\"contextual\"{}>Contextual</option><option value=\"no_change\"{}>No change</option></select><br><label>Source impact <select id=\"source-impact-{}\"{}><option value=\"stale_record\"{}>Create stale record</option><option value=\"migrate_key\"{}>Migrate key</option></select></label></td></tr>",
            escape_html(path),
            escape_html(note["note_id"].as_str().unwrap_or("")),
            escape_html(field["field_id"].as_str().unwrap_or("")),
            editable,
            escape_html(field["status"].as_str().unwrap_or("unknown")),
            escape_html(field["field_name"].as_str().unwrap_or("field")),
            id,
            escape_html(source_value),
            id,
            source_controls_disabled,
            id,
            escape_html(source_value),
            escape_html(path),
            escape_html(source),
            escape_html(note["note_id"].as_str().unwrap_or("")),
            escape_html(field["field_id"].as_str().unwrap_or("")),
            source_readonly,
            source_controls_disabled,
            id,
            escape_html(value),
            escape_html(path),
            escape_html(source),
            escape_html(note["note_id"].as_str().unwrap_or("")),
            escape_html(field["field_id"].as_str().unwrap_or("")),
            readonly,
            id,
            escape_html(value),
            id,
            escape_html(&status),
            occurrence_count,
            id,
            source_controls_disabled,
            selected_attr(source_scope, "field"),
            selected_attr(source_scope, "all_occurrences"),
            id,
            if editable { "" } else { " disabled" },
            selected_attr(mode, "direct"),
            selected_attr(mode, "contextual"),
            selected_attr(mode, "no_change"),
            id,
            source_controls_disabled,
            selected_attr(source_impact, "stale_record"),
            selected_attr(source_impact, "migrate_key"),
        ));
    }
    html.push_str("</tbody></table></section>");
    html
}

#[cfg(target_arch = "wasm32")]
fn selected_attr(current: &str, value: &str) -> &'static str {
    if current == value { " selected" } else { "" }
}

#[cfg(target_arch = "wasm32")]
fn preview_html(preview: &Value) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        "<style>{}</style>",
        preview["styling"].as_str().unwrap_or("")
    ));
    for card in preview["cards"].as_array().into_iter().flatten() {
        html.push_str("<div class=\"anki-card-preview\">");
        html.push_str("<strong>Question</strong>");
        html.push_str(card["question_html"].as_str().unwrap_or(""));
        html.push_str("<strong>Answer</strong>");
        html.push_str(card["answer_html"].as_str().unwrap_or(""));
        html.push_str("</div>");
    }
    html
}

#[cfg(target_arch = "wasm32")]
fn publish_source_string_pivot_panel(pivot: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(panel) = document.get_element_by_id("source-string-pivot-panel") else {
        return;
    };
    let mut html = String::new();
    html.push_str("<h3>Source String pivot</h3>");
    html.push_str("<div class=\"source-string-filters\"><label>Content group <select id=\"source-string-content-group-filter\">");
    let active_group = pivot["filters"]["content_group"].as_str().unwrap_or("all");
    html.push_str(&format!(
        "<option value=\"all\"{}>All groups</option>",
        selected_attr(active_group, "all")
    ));
    for group in pivot["filters"]["content_groups"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let group = group.as_str().unwrap_or("");
        html.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape_html(group),
            selected_attr(active_group, group),
            escape_html(group)
        ));
    }
    html.push_str("</select></label></div>");
    html.push_str("<div class=\"source-string-grid\"><ol class=\"source-string-list\">");
    for string in pivot["strings"].as_array().into_iter().flatten() {
        let source = string["source"].as_str().unwrap_or("");
        let selected = if string["selected"].as_bool().unwrap_or(false) {
            " active"
        } else {
            ""
        };
        html.push_str(&format!(
            "<li><button type=\"button\" class=\"source-string-row{}\" data-source=\"{}\"><strong>{}</strong><br>{} occurrence(s), {}</button></li>",
            selected,
            escape_html(source),
            escape_html(source),
            string["occurrence_count"].as_u64().unwrap_or(0),
            escape_html(string["status"].as_str().unwrap_or("unknown"))
        ));
    }
    html.push_str("</ol><section class=\"source-string-detail\">");
    let selected_source = pivot["selected_source"].as_str().unwrap_or("");
    html.push_str(&format!("<h4>{}</h4>", escape_html(selected_source)));
    if let Some(first_occurrence) = pivot["occurrences"]
        .as_array()
        .and_then(|items| items.first())
    {
        let direct_value = staged_edit_for(
            pivot,
            first_occurrence["path"].as_str().unwrap_or(""),
            selected_source,
        )
        .and_then(|edit| edit["value"].as_str().map(str::to_owned))
        .or_else(|| first_occurrence["target"].as_str().map(str::to_owned))
        .unwrap_or_else(|| selected_source.to_owned());
        html.push_str(&format!(
            "<div class=\"source-string-global-edit\"><label>Reusable translation <input id=\"source-string-direct-input\" value=\"{}\" /></label> <button id=\"source-string-direct-stage\" type=\"button\">Stage direct translation for {} occurrence(s)</button> <button id=\"source-string-no-change\" type=\"button\">Stage global no-change</button></div>",
            escape_html(&direct_value),
            pivot["occurrences"].as_array().map_or(0, Vec::len),
        ));
    }
    html.push_str("<table class=\"source-string-occurrences\"><thead><tr><th>Context</th><th>Source</th><th>Target / contextual override</th><th>Status</th></tr></thead><tbody>");
    for occurrence in pivot["occurrences"].as_array().into_iter().flatten() {
        let path = occurrence["path"].as_str().unwrap_or("");
        let id = id_for_path(path);
        let target = staged_edit_for(pivot, path, selected_source)
            .and_then(|edit| edit["value"].as_str().map(str::to_owned))
            .or_else(|| occurrence["target"].as_str().map(str::to_owned))
            .unwrap_or_else(|| selected_source.to_owned());
        html.push_str(&format!(
            "<tr class=\"source-string-occurrence\" data-source=\"{}\" data-path=\"{}\"><td>{}<br><small>{}</small></td><td>{}</td><td><input id=\"source-string-contextual-input-{}\" value=\"{}\" data-path=\"{}\" data-source=\"{}\" /> <button id=\"source-string-contextual-stage-{}\" type=\"button\">Contextual override</button><div class=\"source-string-target-text\">{}</div></td><td>{}</td></tr>",
            escape_html(selected_source),
            escape_html(path),
            escape_html(occurrence["friendly_context"].as_str().unwrap_or(path)),
            escape_html(path),
            escape_html(selected_source),
            id,
            escape_html(&target),
            escape_html(path),
            escape_html(selected_source),
            id,
            escape_html(&target),
            escape_html(occurrence["status"].as_str().unwrap_or("unknown"))
        ));
    }
    html.push_str("</tbody></table></section></div>");
    panel.set_inner_html(&html);
    mark_panel_loaded(&panel);
    register_source_string_handlers(pivot);
}

#[cfg(target_arch = "wasm32")]
fn publish_card_pivot_panel(pivot: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(panel) = document.get_element_by_id("card-pivot-panel") else {
        return;
    };
    let mut html = String::new();
    html.push_str("<div class=\"card-pivot-view\"><h3>Card pivot</h3>");
    html.push_str("<label>Content group <select id=\"card-content-group-filter\"><option value=\"all\">All groups</option>");
    let active_group = pivot["filters"]["content_group"].as_str().unwrap_or("all");
    for group in pivot["filters"]["content_groups"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let group = group.as_str().unwrap_or("");
        html.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape_html(group),
            selected_attr(active_group, group),
            escape_html(group)
        ));
    }
    html.push_str("</select></label>");
    html.push_str("<nav class=\"card-pivot-filters\">");
    let active_filter = pivot["filters"]["active"].as_str().unwrap_or("all");
    for (filter, label) in [
        ("all", "All cards"),
        ("missing", "Missing"),
        ("stale", "Stale"),
        ("needs_work", "Needs work"),
    ] {
        let class = if active_filter == filter {
            " class=\"active\""
        } else {
            ""
        };
        html.push_str(&format!(
            "<button type=\"button\" data-filter=\"{}\"{}>{}</button>",
            filter, class, label
        ));
    }
    html.push_str("</nav><ol class=\"card-list\">");
    let selected_card_id = pivot["selected_card_id"].as_str().unwrap_or("");
    for card in pivot["cards"].as_array().into_iter().flatten() {
        let card_id = card["card_id"].as_str().unwrap_or("");
        let active = if card_id == selected_card_id {
            " active"
        } else {
            ""
        };
        html.push_str(&format!(
            "<li><button class=\"card-row{}\" type=\"button\" data-card-id=\"{}\">{} · {} · {}</button></li>",
            active,
            escape_html(card_id),
            escape_html(card["title"].as_str().unwrap_or("card")),
            escape_html(card["template_name"].as_str().unwrap_or("template")),
            escape_html(card["status"].as_str().unwrap_or("unknown")),
        ));
    }
    html.push_str("</ol>");
    if let Some(card) = pivot.get("selected_card").filter(|value| !value.is_null()) {
        html.push_str(&format!(
            "<section class=\"card-detail\" data-card-id=\"{}\"><h4>{}: {}</h4>",
            escape_html(card["card_id"].as_str().unwrap_or("")),
            escape_html(card["title"].as_str().unwrap_or("card")),
            escape_html(card["template_name"].as_str().unwrap_or("template")),
        ));
        html.push_str("<div class=\"preview-grid\"><section><h5>Source card</h5><div class=\"card-source-preview\">");
        html.push_str(&preview_html(&card["source_preview"]));
        html.push_str(
            "</div></section><section><h5>Target card</h5><div class=\"card-target-preview\">",
        );
        html.push_str(&preview_html(&card["target_preview"]));
        html.push_str("</div></section></div>");
        html.push_str("<table class=\"card-field-editor\"><thead><tr><th>Field</th><th>Source edit</th><th>Target edit</th><th>Status</th><th>Mode</th></tr></thead><tbody>");
        for field in card["fields"].as_array().into_iter().flatten() {
            let path = field["path"].as_str().unwrap_or("");
            let source = field["source"].as_str().unwrap_or("");
            let id = id_for_path(path);
            let staged = staged_edit_for(pivot, path, source);
            let staged_source = staged_source_edit_for(pivot, path, source);
            let source_value = staged_source
                .as_ref()
                .and_then(|edit| edit["value"].as_str())
                .unwrap_or(source);
            let value = staged
                .as_ref()
                .and_then(|edit| edit["value"].as_str())
                .or_else(|| field["target"].as_str())
                .unwrap_or(source);
            let mode = staged
                .as_ref()
                .and_then(|edit| edit["mode"].as_str())
                .unwrap_or("direct");
            let source_scope = staged_source
                .as_ref()
                .and_then(|edit| edit["scope"].as_str())
                .unwrap_or("field");
            let source_impact = staged_source
                .as_ref()
                .and_then(|edit| edit["impact_action"].as_str())
                .unwrap_or("stale_record");
            let status = staged
                .as_ref()
                .and_then(|edit| edit["mode"].as_str())
                .map(|mode| format!("staged_{mode}"))
                .or_else(|| staged_source.as_ref().map(|_| "staged_source".to_owned()))
                .unwrap_or_else(|| field["status"].as_str().unwrap_or("unknown").to_owned());
            let editable = field["editable"].as_bool().unwrap_or(false);
            let source_editable = field["source_editable"].as_bool().unwrap_or(false);
            html.push_str(&format!(
                "<tr class=\"card-field-row\" data-path=\"{}\" data-source=\"{}\"><td>{}</td><td><span id=\"card-source-text-{}\">{}</span><br><button id=\"card-source-edit-toggle-{}\" type=\"button\"{}>Edit source</button> <input id=\"card-source-input-{}\" value=\"{}\"{}{}><br><select id=\"card-source-scope-{}\"{}><option value=\"field\"{}>This field only</option><option value=\"all_occurrences\"{}>All occurrences</option></select> <select id=\"card-source-impact-{}\"{}><option value=\"stale_record\"{}>Create stale record</option><option value=\"migrate_key\"{}>Migrate key</option></select></td><td><input id=\"card-translation-input-{}\" value=\"{}\"{}><div id=\"card-target-text-{}\">{}</div></td><td id=\"card-status-text-{}\">{}</td><td><select id=\"card-translation-mode-{}\"{}><option value=\"direct\"{}>Direct</option><option value=\"contextual\"{}>Contextual</option><option value=\"no_change\"{}>No change</option></select></td></tr>",
                escape_html(path),
                escape_html(source),
                escape_html(field["field_name"].as_str().unwrap_or("field")),
                id,
                escape_html(source_value),
                id,
                if source_editable { "" } else { " disabled" },
                id,
                escape_html(source_value),
                if staged_source.is_some() { "" } else { " readonly" },
                if source_editable { "" } else { " disabled" },
                id,
                if source_editable { "" } else { " disabled" },
                selected_attr(source_scope, "field"),
                selected_attr(source_scope, "all_occurrences"),
                id,
                if source_editable { "" } else { " disabled" },
                selected_attr(source_impact, "stale_record"),
                selected_attr(source_impact, "migrate_key"),
                id,
                escape_html(value),
                if editable { "" } else { " readonly" },
                id,
                escape_html(value),
                id,
                escape_html(&status),
                id,
                if editable { "" } else { " disabled" },
                selected_attr(mode, "direct"),
                selected_attr(mode, "contextual"),
                selected_attr(mode, "no_change"),
            ));
        }
        html.push_str("</tbody></table></section>");
    } else {
        html.push_str("<p>No cards match the active filters.</p>");
    }
    html.push_str("</div>");
    panel.set_inner_html(&html);
    mark_panel_loaded(&panel);
    register_card_pivot_handlers(pivot);
}

#[cfg(target_arch = "wasm32")]
fn register_card_pivot_handlers(pivot: &Value) {
    attach_card_filter_handlers(pivot);
    for card in pivot["cards"].as_array().into_iter().flatten() {
        if let Some(card_id) = card["card_id"].as_str() {
            attach_card_select_handler(pivot, card_id.to_owned());
        }
    }
    if let Some(card) = pivot.get("selected_card").filter(|value| !value.is_null()) {
        for field in card["fields"].as_array().into_iter().flatten() {
            let path = field["path"].as_str().unwrap_or("").to_owned();
            let source = field["source"].as_str().unwrap_or("").to_owned();
            let note_id = field["note_id"].as_str().unwrap_or("").to_owned();
            let field_id = field["field_id"].as_str().unwrap_or("").to_owned();
            if field["source_editable"].as_bool().unwrap_or(false) {
                attach_card_source_stage_handler(
                    pivot,
                    path.clone(),
                    source.clone(),
                    note_id.clone(),
                    field_id.clone(),
                );
            }
            attach_card_stage_handler(pivot, path, source, note_id, field_id);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_card_filter_handlers(pivot: &Value) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let group_language = language.clone();
    let group_target = target.clone();
    let group_overlay = overlay.clone();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        load_card_pivot_for_parts(
            group_language.clone(),
            group_target.clone(),
            group_overlay.clone(),
            None,
            active_card_filter(&document),
            selected_value(&document, "card-content-group-filter"),
        );
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("card-content-group-filter"))
    {
        let _ =
            element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    }
    closure.forget();

    for filter in ["all", "missing", "stale", "needs_work"] {
        let language = language.clone();
        let target = target.clone();
        let overlay = overlay.clone();
        let filter = filter.to_owned();
        let selector = format!(".card-pivot-filters button[data-filter=\"{filter}\"]");
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let group = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| selected_value(&document, "card-content-group-filter"));
            load_card_pivot_for_parts(
                language.clone(),
                target.clone(),
                overlay.clone(),
                None,
                Some(filter.clone()),
                group,
            );
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.query_selector(&selector).ok().flatten())
        {
            let _ =
                element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn active_card_filter(document: &web_sys::Document) -> Option<String> {
    document
        .query_selector(".card-pivot-filters button.active")
        .ok()
        .flatten()
        .and_then(|element| element.get_attribute("data-filter"))
}

#[cfg(target_arch = "wasm32")]
fn attach_card_select_handler(pivot: &Value, card_id: String) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let selector = format!(".card-row[data-card-id=\"{}\"]", css_escape(&card_id));
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let document = web_sys::window().and_then(|window| window.document());
        let filter = document.as_ref().and_then(active_card_filter);
        let group = document
            .as_ref()
            .and_then(|document| selected_value(document, "card-content-group-filter"));
        load_card_pivot_for_parts(
            language.clone(),
            target.clone(),
            overlay.clone(),
            Some(card_id.clone()),
            filter,
            group,
        );
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.query_selector(&selector).ok().flatten())
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_card_stage_handler(
    pivot: &Value,
    path: String,
    source: String,
    _note_id: String,
    field_id: String,
) {
    let id = id_for_path(&path);
    for element_id in [
        format!("card-translation-input-{id}"),
        format!("card-translation-mode-{id}"),
    ] {
        let pivot = pivot.clone();
        let path = path.clone();
        let source = source.clone();
        let field_id = field_id.clone();
        let input_id = format!("card-translation-input-{id}");
        let mode_id = format!("card-translation-mode-{id}");
        let source_input_id = format!("card-source-input-{id}");
        let target_id = format!("card-target-text-{id}");
        let status_id = format!("card-status-text-{id}");
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let value = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                .map(|input| input.value())
                .unwrap_or_default();
            let mode = document
                .get_element_by_id(&mode_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|select| select.value())
                .unwrap_or_else(|| "direct".to_owned());
            let source_key = source_edit_storage_key(&pivot, &path, &source);
            let effective_source = local_storage()
                .and_then(|storage| storage.get_item(&source_key).ok().flatten())
                .and_then(|stored| serde_json::from_str::<Value>(&stored).ok())
                .and_then(|edit| edit["value"].as_str().map(str::to_owned))
                .or_else(|| {
                    document
                        .get_element_by_id(&source_input_id)
                        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                        .map(|input| input.value())
                        .filter(|value| value != &source)
                })
                .unwrap_or_else(|| source.clone());
            stage_card_translation(&pivot, &path, &source, &effective_source, &value, &mode);
            if let Some(target) = document.get_element_by_id(&target_id) {
                target.set_text_content(Some(&value));
            }
            if let Some(status) = document.get_element_by_id(&status_id) {
                status.set_text_content(Some(&format!("staged_{mode}")));
            }
            update_card_preview_field(&document, "card-target-preview", &field_id, &value);
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&element_id))
        {
            let event = if element_id.starts_with("card-translation-input-") {
                "input"
            } else {
                "change"
            };
            let _ =
                element.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_card_source_stage_handler(
    pivot: &Value,
    path: String,
    source: String,
    _note_id: String,
    field_id: String,
) {
    let id = id_for_path(&path);
    let input_id = format!("card-source-input-{id}");
    let toggle_id = format!("card-source-edit-toggle-{id}");
    let scope_id = format!("card-source-scope-{id}");
    let impact_id = format!("card-source-impact-{id}");
    let source_text_id = format!("card-source-text-{id}");
    let status_id = format!("card-status-text-{id}");

    {
        let input_id = input_id.clone();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            if let Some(input) = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                input.set_read_only(false);
                let _ = input.focus();
            }
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&toggle_id))
        {
            let _ =
                element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    for element_id in [input_id.clone(), scope_id.clone(), impact_id.clone()] {
        let pivot = pivot.clone();
        let path = path.clone();
        let source = source.clone();
        let field_id = field_id.clone();
        let input_id = input_id.clone();
        let scope_id = scope_id.clone();
        let impact_id = impact_id.clone();
        let source_text_id = source_text_id.clone();
        let status_id = status_id.clone();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let value = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                .map(|input| input.value())
                .unwrap_or_else(|| source.clone());
            let scope = document
                .get_element_by_id(&scope_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|select| select.value())
                .unwrap_or_else(|| "field".to_owned());
            let impact = document
                .get_element_by_id(&impact_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|select| select.value())
                .unwrap_or_else(|| "stale_record".to_owned());
            stage_card_source_edit(&pivot, &path, &source, &value, &scope, &impact);
            if let Some(source_text) = document.get_element_by_id(&source_text_id) {
                source_text.set_text_content(Some(&value));
            }
            if let Some(status) = document.get_element_by_id(&status_id) {
                status.set_text_content(Some("staged_source"));
            }
            update_card_preview_field(&document, "card-source-preview", &field_id, &value);
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&element_id))
        {
            let event = if element_id.starts_with("card-source-input-") {
                "input"
            } else {
                "change"
            };
            let _ =
                element.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn stage_card_translation(
    pivot: &Value,
    path: &str,
    storage_source: &str,
    source: &str,
    value: &str,
    mode: &str,
) {
    let mut edit = serde_json::json!({
        "kind": "translation",
        "path": path,
        "source": source,
        "value": value,
        "mode": mode,
    });
    if mode == "contextual" {
        edit["context_path"] = Value::String(path.to_owned());
    }
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(
            &edit_storage_key(pivot, path, storage_source),
            &edit.to_string(),
        );
    }
    update_staged_count_for_pivot(pivot);
}

#[cfg(target_arch = "wasm32")]
fn stage_card_source_edit(
    pivot: &Value,
    path: &str,
    source: &str,
    value: &str,
    scope: &str,
    impact: &str,
) {
    let edit = serde_json::json!({
        "kind": "source",
        "path": path,
        "source": source,
        "value": value,
        "scope": scope,
        "impact_action": impact,
    });
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(
            &source_edit_storage_key(pivot, path, source),
            &edit.to_string(),
        );
        if let Some(stored_translation) = storage
            .get_item(&edit_storage_key(pivot, path, source))
            .ok()
            .flatten()
            && let Ok(mut translation_edit) = serde_json::from_str::<Value>(&stored_translation)
        {
            translation_edit["source"] = Value::String(value.to_owned());
            let _ = storage.set_item(
                &edit_storage_key(pivot, path, source),
                &translation_edit.to_string(),
            );
        }
    }
    update_staged_count_for_pivot(pivot);
}

#[cfg(target_arch = "wasm32")]
fn update_card_preview_field(
    document: &web_sys::Document,
    preview_class: &str,
    field_id: &str,
    value: &str,
) {
    let selector = format!(
        "#card-pivot-panel .{} [data-preview-field-id=\"{}\"]",
        preview_class, field_id
    );
    let Ok(nodes) = document.query_selector_all(&selector) else {
        return;
    };
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            element.set_inner_html(value);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_card_pivot_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    card: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch_card_pivot_query(language, target, overlay, card, filter, content_group).await {
            Ok(pivot) => publish_card_pivot_panel(&pivot),
            Err(error) => {
                if let Some(panel) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("card-pivot-panel"))
                {
                    panel.set_inner_html(&format!(
                        "<p class=\"workbench-error\">{}</p>",
                        escape_html(&error)
                    ));
                }
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn register_source_string_handlers(pivot: &Value) {
    attach_source_string_filter_handler(pivot);
    for string in pivot["strings"].as_array().into_iter().flatten() {
        let source = string["source"].as_str().unwrap_or("").to_owned();
        attach_source_string_select_handler(pivot, source);
    }
    attach_source_string_direct_handlers(pivot);
    for occurrence in pivot["occurrences"].as_array().into_iter().flatten() {
        let path = occurrence["path"].as_str().unwrap_or("").to_owned();
        let source = occurrence["source"].as_str().unwrap_or("").to_owned();
        attach_source_string_contextual_handler(pivot, path, source);
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_source_string_filter_handler(pivot: &Value) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let content_group = selected_value(&document, "source-string-content-group-filter");
        load_source_string_pivot_for_parts(
            language.clone(),
            target.clone(),
            overlay.clone(),
            None,
            content_group,
            None,
        );
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("source-string-content-group-filter"))
    {
        let _ =
            element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_source_string_select_handler(pivot: &Value, source: String) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let selector = format!(
        ".source-string-row[data-source=\"{}\"]",
        css_escape(&source)
    );
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        load_source_string_pivot_for_parts(
            language.clone(),
            target.clone(),
            overlay.clone(),
            Some(source.clone()),
            None,
            None,
        );
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.query_selector(&selector).ok().flatten())
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_source_string_direct_handlers(pivot: &Value) {
    let Some(first_occurrence) = pivot["occurrences"]
        .as_array()
        .and_then(|items| items.first())
    else {
        return;
    };
    let path = first_occurrence["path"].as_str().unwrap_or("").to_owned();
    let source = first_occurrence["source"].as_str().unwrap_or("").to_owned();
    for (element_id, mode) in [
        ("source-string-direct-input", "direct"),
        ("source-string-direct-stage", "direct"),
        ("source-string-no-change", "no_change"),
    ] {
        let pivot = pivot.clone();
        let path = path.clone();
        let source = source.clone();
        let mode = mode.to_owned();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let value = if mode == "no_change" {
                source.clone()
            } else {
                document
                    .get_element_by_id("source-string-direct-input")
                    .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .map(|input| input.value())
                    .unwrap_or_else(|| source.clone())
            };
            stage_source_string_translation(&pivot, &path, &source, &value, &mode);
            update_source_string_targets(&document, &source, &value);
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(element_id))
        {
            let event = if element_id == "source-string-direct-input" {
                "input"
            } else {
                "click"
            };
            let _ =
                element.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_source_string_contextual_handler(pivot: &Value, path: String, source: String) {
    let id = id_for_path(&path);
    for element_id in [
        format!("source-string-contextual-input-{id}"),
        format!("source-string-contextual-stage-{id}"),
    ] {
        let pivot = pivot.clone();
        let path = path.clone();
        let source = source.clone();
        let input_id = format!("source-string-contextual-input-{id}");
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let value = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                .map(|input| input.value())
                .unwrap_or_else(|| source.clone());
            stage_source_string_translation(&pivot, &path, &source, &value, "contextual");
            update_source_string_contextual_target(&document, &path, &value);
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&element_id))
        {
            let event = if element_id.starts_with("source-string-contextual-input-") {
                "input"
            } else {
                "click"
            };
            let _ =
                element.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn stage_source_string_translation(
    pivot: &Value,
    path: &str,
    source: &str,
    value: &str,
    mode: &str,
) {
    let mut edit = serde_json::json!({
        "kind": "translation",
        "path": path,
        "source": source,
        "value": value,
        "mode": mode,
    });
    if mode == "contextual" {
        edit["context_path"] = Value::String(path.to_owned());
    }
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(&edit_storage_key(pivot, path, source), &edit.to_string());
    }
    update_staged_count_for_pivot(pivot);
}

#[cfg(target_arch = "wasm32")]
fn update_source_string_targets(document: &web_sys::Document, source: &str, value: &str) {
    let selector = format!(
        ".source-string-occurrence[data-source=\"{}\"] .source-string-target-text",
        css_escape(source)
    );
    if let Ok(nodes) = document.query_selector_all(&selector) {
        for index in 0..nodes.length() {
            if let Some(node) = nodes.get(index)
                && let Ok(element) = node.dyn_into::<web_sys::Element>()
            {
                element.set_text_content(Some(value));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn update_source_string_contextual_target(document: &web_sys::Document, path: &str, value: &str) {
    let selector = format!(
        ".source-string-occurrence[data-path=\"{}\"] .source-string-target-text",
        css_escape(path)
    );
    if let Some(element) = document.query_selector(&selector).ok().flatten() {
        element.set_text_content(Some(value));
    }
}

#[cfg(target_arch = "wasm32")]
fn load_source_string_pivot_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    source: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch_source_string_pivot_query(
            language,
            target,
            overlay,
            source,
            content_group,
            status,
        )
        .await
        {
            Ok(source_pivot) => publish_source_string_pivot_panel(&source_pivot),
            Err(error) => {
                if let Some(document) = web_sys::window().and_then(|window| window.document())
                    && let Some(panel) = document.get_element_by_id("source-string-pivot-panel")
                {
                    panel.set_inner_html(&format!(
                        "<p class=\"workbench-error\">{}</p>",
                        escape_html(&error)
                    ));
                }
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn load_optional_metadata_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch_optional_metadata_query(language, target, overlay).await {
            Ok(optional) => publish_optional_metadata_panel(&optional),
            Err(error) => {
                if let Some(document) = web_sys::window().and_then(|window| window.document())
                    && let Some(panel) = document.get_element_by_id("optional-metadata-panel")
                {
                    panel.set_inner_html(&format!(
                        "<p class=\"workbench-error\">{}</p>",
                        escape_html(&error)
                    ));
                }
            }
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn publish_optional_metadata_panel(optional: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(panel) = document.get_element_by_id("optional-metadata-panel") else {
        return;
    };
    let language = optional["language"]["code"].as_str().unwrap_or("");
    let target = optional["target"]["label"].as_str().unwrap_or("");
    let overlay = optional["overlay"]["label"].as_str().unwrap_or("");
    let prefix = storage_prefix_for_parts(language, target, overlay);
    let progress = &optional["optional_progress"];
    let mut html = format!(
        "<section id=\"optional-metadata-checklist\" data-storage-prefix=\"{}\"><h3>Optional metadata checklist</h3><p>Main note-field completion is separate. Optional metadata: {} / {} complete, {} missing, {} stale.</p>",
        escape_html(&prefix),
        progress["complete"].as_u64().unwrap_or(0),
        progress["total"].as_u64().unwrap_or(0),
        progress["missing"].as_u64().unwrap_or(0),
        progress["stale"].as_u64().unwrap_or(0),
    );
    html.push_str("<table><thead><tr><th>Category</th><th>Path</th><th>Source</th><th>Target</th><th>Status</th></tr></thead><tbody>");
    for item in optional["items"].as_array().into_iter().flatten() {
        let path = item["path"].as_str().unwrap_or("");
        let source = item["source"].as_str().unwrap_or("");
        let id = id_for_path(path);
        let staged = staged_edit_for_pane(language, target, overlay, path, source);
        let value = staged
            .as_ref()
            .and_then(|edit| edit["value"].as_str())
            .or_else(|| item["target"].as_str())
            .unwrap_or(source);
        let status = staged
            .as_ref()
            .map(|_| "staged_direct".to_owned())
            .unwrap_or_else(|| item["status"].as_str().unwrap_or("unknown").to_owned());
        let warning = item["warning"].as_str().unwrap_or("");
        html.push_str(&format!(
            "<tr class=\"optional-metadata-row\" data-path=\"{}\" data-source=\"{}\"><td>{}</td><td>{}</td><td>{}</td><td><input id=\"optional-translation-input-{}\" value=\"{}\" data-path=\"{}\" data-source=\"{}\"></td><td id=\"optional-status-{}\">{} {}</td></tr>",
            escape_html(path),
            escape_html(source),
            escape_html(item["metadata_category"].as_str().unwrap_or("metadata")),
            escape_html(path),
            escape_html(source),
            id,
            escape_html(value),
            escape_html(path),
            escape_html(source),
            id,
            escape_html(&status),
            escape_html(warning),
        ));
    }
    html.push_str("</tbody></table></section>");
    panel.set_inner_html(&html);
    mark_panel_loaded(&panel);
    register_optional_metadata_handlers(optional);
    let target_writable = checkbox_checked(&document, "target-pane-writable");
    apply_pane_writability(
        checkbox_checked(&document, "source-pane-writable"),
        target_writable,
    );
    update_staged_count(&prefix);
}

#[cfg(target_arch = "wasm32")]
fn css_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_arch = "wasm32")]
fn register_control_handlers(_pivot: &Value) {
    attach_select_reload_handler("language-select", true);
    attach_select_reload_handler("target-select", false);
    attach_select_reload_handler("overlay-select", false);
    for filter in ["all", "missing", "stale", "needs_work"] {
        attach_filter_reload_handler(filter);
    }
}

#[cfg(target_arch = "wasm32")]
fn register_lazy_pivot_load_handlers(pivot: &Value) {
    attach_card_pivot_load_handler(pivot);
    attach_source_string_pivot_load_handler(pivot);
    attach_optional_metadata_load_handler(pivot);
}

#[cfg(target_arch = "wasm32")]
fn attach_card_pivot_load_handler(pivot: &Value) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        set_panel_loading("card-pivot-panel", "Loading card pivot…");
        load_card_pivot_for_parts(
            language.clone(),
            target.clone(),
            overlay.clone(),
            None,
            None,
            None,
        );
    }));
    attach_click_handler("load-card-pivot-button", closure);
}

#[cfg(target_arch = "wasm32")]
fn attach_source_string_pivot_load_handler(pivot: &Value) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        set_panel_loading("source-string-pivot-panel", "Loading source string pivot…");
        load_source_string_pivot_for_parts(
            language.clone(),
            target.clone(),
            overlay.clone(),
            None,
            None,
            None,
        );
    }));
    attach_click_handler("load-source-string-pivot-button", closure);
}

#[cfg(target_arch = "wasm32")]
fn attach_optional_metadata_load_handler(pivot: &Value) {
    let (language, target, overlay) = pivot_selection_parts(pivot);
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        set_panel_loading("optional-metadata-panel", "Loading optional metadata…");
        load_optional_metadata_for_parts(language.clone(), target.clone(), overlay.clone());
    }));
    attach_click_handler("load-optional-metadata-button", closure);
}

#[cfg(target_arch = "wasm32")]
fn attach_click_handler(element_id: &str, closure: Closure<dyn FnMut(web_sys::Event)>) {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(element_id))
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn set_panel_loading(panel_id: &str, message: &str) {
    if let Some(panel) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(panel_id))
    {
        let _ = panel.set_attribute("data-lazy-state", "loading");
        let _ = panel.set_attribute("aria-busy", "true");
        panel.set_inner_html(&format!(
            "<p class=\"workbench-loading\">{}</p>",
            escape_html(message)
        ));
    }
}

#[cfg(target_arch = "wasm32")]
fn mark_panel_loaded(panel: &web_sys::Element) {
    let _ = panel.set_attribute("data-lazy-state", "loaded");
    let _ = panel.set_attribute("aria-busy", "false");
}

#[cfg(target_arch = "wasm32")]
fn register_pane_layout_handlers(pivot: &Value) {
    apply_pane_writability(false, true);
    attach_pane_writable_toggle("source-pane-writable");
    attach_pane_writable_toggle("target-pane-writable");
    attach_secondary_pane_loader(pivot);
}

#[cfg(target_arch = "wasm32")]
fn attach_pane_writable_toggle(element_id: &str) {
    let element_id = element_id.to_owned();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let source_writable = checkbox_checked(&document, "source-pane-writable");
        let target_writable = checkbox_checked(&document, "target-pane-writable");
        apply_pane_writability(source_writable, target_writable);
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&element_id))
    {
        let _ =
            element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn checkbox_checked(document: &web_sys::Document, element_id: &str) -> bool {
    document
        .get_element_by_id(element_id)
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
        .is_some_and(|input| input.checked())
}

#[cfg(target_arch = "wasm32")]
fn apply_pane_writability(source_writable: bool, target_writable: bool) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    set_controls_disabled(
        &document,
        "button[id^='source-edit-toggle-'], input[id^='source-input-'], select[id^='source-scope-'], select[id^='source-impact-'], button[id^='card-source-edit-toggle-'], input[id^='card-source-input-'], select[id^='card-source-scope-'], select[id^='card-source-impact-']",
        !source_writable,
    );
    set_inputs_readonly(
        &document,
        "input[id^='source-input-'], input[id^='card-source-input-']",
        !source_writable,
    );
    set_controls_disabled(
        &document,
        "input[id^='translation-input-'], select[id^='translation-mode-'], input[id^='card-translation-input-'], select[id^='card-translation-mode-'], input[id^='optional-translation-input-'], input[id^='source-string-direct-input'], button[id^='source-string-direct-stage'], button[id^='source-string-no-change'], input[id^='source-string-contextual-input-'], button[id^='source-string-contextual-stage-']",
        !target_writable,
    );
}

#[cfg(target_arch = "wasm32")]
fn apply_secondary_pane_writability(writable: bool) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    set_controls_disabled(
        &document,
        "input[id^='secondary-translation-input-']",
        !writable,
    );
}

#[cfg(target_arch = "wasm32")]
fn set_controls_disabled(document: &web_sys::Document, selector: &str, disabled: bool) {
    if let Ok(nodes) = document.query_selector_all(selector) {
        for index in 0..nodes.length() {
            if let Some(node) = nodes.get(index)
                && let Ok(element) = node.dyn_into::<web_sys::HtmlElement>()
            {
                let _ = if disabled {
                    element.set_attribute("disabled", "")
                } else {
                    element.remove_attribute("disabled")
                };
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn set_inputs_readonly(document: &web_sys::Document, selector: &str, readonly: bool) {
    if let Ok(nodes) = document.query_selector_all(selector) {
        for index in 0..nodes.length() {
            if let Some(node) = nodes.get(index)
                && let Ok(input) = node.dyn_into::<web_sys::HtmlInputElement>()
            {
                input.set_read_only(readonly);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_secondary_pane_loader(pivot: &Value) {
    let pivot = pivot.clone();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let Some(language) = selected_value(&document, "secondary-pane-language-select") else {
            return;
        };
        let target = pivot["target"]["label"].as_str().map(str::to_owned);
        wasm_bindgen_futures::spawn_local(async move {
            match fetch_comparison_pane_query(Some(language), target, Some("base".to_owned())).await
            {
                Ok(pane) => publish_secondary_target_pane(&pane),
                Err(error) => render_secondary_pane_error(&error),
            }
        });
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("load-secondary-pane"))
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn publish_secondary_target_pane(comparison: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(container) = document.get_element_by_id("secondary-target-pane") else {
        return;
    };
    let pane = &comparison["note_pivot"];
    let source_strings = &comparison["source_string_pivot"];
    let cards = &comparison["card_pivot"];
    let language = pane["language"]["code"].as_str().unwrap_or("");
    let target = pane["target"]["label"].as_str().unwrap_or("");
    let overlay = pane["overlay"]["label"].as_str().unwrap_or("");
    let prefix = storage_prefix_for_parts(language, target, overlay);
    let mut html = format!(
        "<section class=\"workbench-pane secondary-target-pane\" data-storage-prefix=\"{}\" data-language=\"{}\" data-target=\"{}\" data-overlay=\"{}\"><h4>{} target pane</h4><label><input id=\"secondary-pane-writable\" type=\"checkbox\" checked> Pane writable</label>",
        escape_html(&prefix),
        escape_html(language),
        escape_html(target),
        escape_html(overlay),
        escape_html(
            pane["language"]["display_name"]
                .as_str()
                .unwrap_or(language)
        ),
    );
    html.push_str("<table><tbody>");
    for note in pane["notes"].as_array().into_iter().flatten() {
        for field in note["fields"].as_array().into_iter().flatten() {
            if !field["editable"].as_bool().unwrap_or(false) {
                continue;
            }
            let path = field["path"].as_str().unwrap_or("");
            let source = field["source"].as_str().unwrap_or("");
            let id = format!("{}-{}", language, id_for_path(path));
            let staged = staged_edit_for_pane(language, target, overlay, path, source);
            let value = staged
                .as_ref()
                .and_then(|edit| edit["value"].as_str())
                .or_else(|| field["target"].as_str())
                .unwrap_or(source);
            html.push_str(&format!(
                "<tr class=\"secondary-field-row\" data-path=\"{}\" data-source=\"{}\"><td>{}</td><td>{}</td><td><input id=\"secondary-translation-input-{}\" value=\"{}\" data-path=\"{}\" data-source=\"{}\"></td></tr>",
                escape_html(path),
                escape_html(source),
                escape_html(field["field_name"].as_str().unwrap_or("field")),
                escape_html(source),
                escape_html(&id),
                escape_html(value),
                escape_html(path),
                escape_html(source),
            ));
        }
    }
    html.push_str("</tbody></table>");
    html.push_str(
        "<section class=\"comparison-source-strings\"><h5>Source String comparison</h5><ul>",
    );
    for string in source_strings["strings"].as_array().into_iter().flatten() {
        html.push_str(&format!(
            "<li class=\"comparison-source-string-row\" data-source=\"{}\">{} → {} ({})</li>",
            escape_html(string["source"].as_str().unwrap_or("")),
            escape_html(string["source"].as_str().unwrap_or("")),
            escape_html(string["target_preview"].as_str().unwrap_or("")),
            escape_html(string["status"].as_str().unwrap_or("unknown")),
        ));
    }
    html.push_str("</ul></section>");
    html.push_str("<section class=\"comparison-cards\"><h5>Card comparison</h5><ul>");
    for card in cards["cards"].as_array().into_iter().flatten() {
        html.push_str(&format!(
            "<li class=\"comparison-card-row\" data-card-id=\"{}\">{} · {} · {}</li>",
            escape_html(card["card_id"].as_str().unwrap_or("")),
            escape_html(card["title"].as_str().unwrap_or("card")),
            escape_html(card["template_name"].as_str().unwrap_or("template")),
            escape_html(card["status"].as_str().unwrap_or("unknown")),
        ));
    }
    html.push_str("</ul>");
    if let Some(card) = cards.get("selected_card").filter(|value| !value.is_null()) {
        html.push_str("<div class=\"comparison-card-preview\">");
        html.push_str(&preview_html(&card["target_preview"]));
        html.push_str("</div>");
    }
    html.push_str("</section></section>");
    container.set_inner_html(&html);
    register_secondary_target_handlers(pane);
    attach_secondary_writable_handler();
    let target_writable = checkbox_checked(&document, "target-pane-writable");
    apply_pane_writability(
        checkbox_checked(&document, "source-pane-writable"),
        target_writable,
    );
    apply_secondary_pane_writability(checkbox_checked(&document, "secondary-pane-writable"));
}

#[cfg(target_arch = "wasm32")]
fn attach_secondary_writable_handler() {
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        apply_secondary_pane_writability(checkbox_checked(&document, "secondary-pane-writable"));
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("secondary-pane-writable"))
    {
        let _ =
            element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn render_secondary_pane_error(error: &str) {
    if let Some(container) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("secondary-target-pane"))
    {
        container.set_inner_html(&format!(
            "<p class=\"workbench-error\">{}</p>",
            escape_html(error)
        ));
    }
}

#[cfg(target_arch = "wasm32")]
fn register_secondary_target_handlers(pane: &Value) {
    let language = pane["language"]["code"].as_str().unwrap_or("").to_owned();
    let target = pane["target"]["label"].as_str().unwrap_or("").to_owned();
    let overlay = pane["overlay"]["label"].as_str().unwrap_or("").to_owned();
    for note in pane["notes"].as_array().into_iter().flatten() {
        for field in note["fields"].as_array().into_iter().flatten() {
            let path = field["path"].as_str().unwrap_or("").to_owned();
            let source = field["source"].as_str().unwrap_or("").to_owned();
            let input_id = format!(
                "secondary-translation-input-{}-{}",
                language,
                id_for_path(&path)
            );
            let input_id_for_handler = input_id.clone();
            let language = language.clone();
            let target = target.clone();
            let overlay = overlay.clone();
            let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
                let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                    return;
                };
                let value = document
                    .get_element_by_id(&input_id_for_handler)
                    .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .map(|input| input.value())
                    .unwrap_or_default();
                stage_secondary_translation(&language, &target, &overlay, &path, &source, &value);
            }));
            if let Some(element) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id(&input_id))
            {
                let _ = element
                    .add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            }
            closure.forget();
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn stage_secondary_translation(
    language: &str,
    target: &str,
    overlay: &str,
    path: &str,
    source: &str,
    value: &str,
) {
    let edit = serde_json::json!({
        "kind": "translation",
        "language": language,
        "target": target,
        "overlay": overlay,
        "path": path,
        "source": source,
        "value": value,
        "mode": "direct",
    });
    if let Some(storage) = local_storage() {
        let key = format!(
            "{}translation::{}::{}",
            storage_prefix_for_parts(language, target, overlay),
            path,
            source
        );
        let _ = storage.set_item(&key, &edit.to_string());
    }
    let prefixes =
        active_storage_prefixes_for_default(&storage_prefix_for_parts(language, target, overlay));
    update_staged_count_for_prefixes(&prefixes);
}

#[cfg(target_arch = "wasm32")]
fn staged_edit_for_pane(
    language: &str,
    target: &str,
    overlay: &str,
    path: &str,
    source: &str,
) -> Option<Value> {
    local_storage()
        .and_then(|storage| {
            let key = format!(
                "{}translation::{}::{}",
                storage_prefix_for_parts(language, target, overlay),
                path,
                source
            );
            storage.get_item(&key).ok().flatten()
        })
        .and_then(|value| serde_json::from_str(&value).ok())
}

#[cfg(target_arch = "wasm32")]
fn register_new_language_handlers(_pivot: &Value) {
    attach_new_language_preview_handler();
    attach_new_language_confirm_handler();
}

#[cfg(target_arch = "wasm32")]
fn attach_new_language_preview_handler() {
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let template = selected_value(&document, "new-language-template");
        let code = input_value(&document, "new-language-code");
        let display_name = input_value(&document, "new-language-display-name");
        wasm_bindgen_futures::spawn_local(async move {
            let result = fetch_new_language_preview(template, code, display_name).await;
            render_new_language_preview(result);
        });
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("new-language-preview-button"))
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_new_language_confirm_handler() {
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let Some(request) = collect_new_language_request(&document) else {
            render_new_language_status("Preview the new language before creating it.", false);
            return;
        };
        let code = request["code"].as_str().unwrap_or("").to_owned();
        let target = request["primary_target"].as_str().unwrap_or("").to_owned();
        let overlay = request["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|group| group["selected"].as_bool().unwrap_or(false))
            .and_then(|group| group["label"].as_str())
            .unwrap_or("base")
            .to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            match post_json("/api/workbench/new-language", &request).await {
                Ok(value) => {
                    render_new_language_status(&new_language_result_text("Created", &value), true);
                    match fetch_note_pivot_query(Some(code), Some(target), Some(overlay), None)
                        .await
                    {
                        Ok(pivot) => publish_note_pivot_panel(&pivot),
                        Err(error) => render_new_language_status(&escape_html(&error), false),
                    }
                }
                Err(error) => render_new_language_status(
                    &escape_html(&format!("Create failed: {error}")),
                    false,
                ),
            }
        });
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("new-language-confirm-button"))
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_select_reload_handler(element_id: &str, reset_dependent_selection: bool) {
    let element_id = element_id.to_owned();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let language = selected_value(&document, "language-select");
        let target = (!reset_dependent_selection)
            .then(|| selected_value(&document, "target-select"))
            .flatten();
        let overlay = (!reset_dependent_selection)
            .then(|| selected_value(&document, "overlay-select"))
            .flatten();
        let filter = active_filter(&document);
        reload_note_pivot(language, target, overlay, filter);
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&element_id))
    {
        let _ =
            element.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_filter_reload_handler(filter: &str) {
    let filter = filter.to_owned();
    let selector = format!(".pivot-filters button[data-filter=\"{filter}\"]");
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        reload_note_pivot(
            selected_value(&document, "language-select"),
            selected_value(&document, "target-select"),
            selected_value(&document, "overlay-select"),
            Some(filter.clone()),
        );
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.query_selector(&selector).ok().flatten())
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn selected_value(document: &web_sys::Document, element_id: &str) -> Option<String> {
    document
        .get_element_by_id(element_id)
        .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|select| select.value())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
fn input_value(document: &web_sys::Document, element_id: &str) -> Option<String> {
    document
        .get_element_by_id(element_id)
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .filter(|value| !value.is_empty())
}

#[cfg(target_arch = "wasm32")]
fn render_new_language_preview(result: Result<Value, String>) {
    match result {
        Ok(value) => {
            let mut html = String::new();
            html.push_str(&format!(
                "<div class=\"new-language-preview\" data-code=\"{}\" data-template=\"{}\" data-primary-target=\"{}\">",
                escape_html(value["language"]["code"].as_str().unwrap_or("")),
                escape_html(value["language"]["template_language"].as_str().unwrap_or("")),
                escape_html(value["language"]["primary_target"].as_str().unwrap_or("")),
            ));
            html.push_str("<p>Preview ready. Affected files:</p><ul>");
            for file in value["affected_files"].as_array().into_iter().flatten() {
                html.push_str(&format!(
                    "<li>{}</li>",
                    escape_html(file["path"].as_str().unwrap_or("unknown"))
                ));
            }
            html.push_str("</ul><table><tbody>");
            for group in value["groups"].as_array().into_iter().flatten() {
                let label = group["label"].as_str().unwrap_or("");
                let checked = if group["selected"].as_bool().unwrap_or(false) {
                    " checked"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "<tr class=\"new-language-group-row\" data-label=\"{}\" data-template-overlay-id=\"{}\"><td><label><input class=\"new-language-group-selected\" id=\"new-language-group-{}\" type=\"checkbox\"{}> {}</label></td><td><input class=\"new-language-group-overlay-id\" value=\"{}\"></td><td><input class=\"new-language-group-file\" value=\"{}\"></td></tr>",
                    escape_html(label),
                    escape_html(group["template_overlay_id"].as_str().unwrap_or("")),
                    escape_html(label),
                    checked,
                    escape_html(label),
                    escape_html(group["overlay_id"].as_str().unwrap_or("")),
                    escape_html(group["file"].as_str().unwrap_or("")),
                ));
            }
            html.push_str("</tbody></table><table><tbody>");
            for target in value["targets"].as_array().into_iter().flatten() {
                let label = target["label"].as_str().unwrap_or("");
                html.push_str(&format!(
                    "<tr class=\"new-language-target-row\" data-label=\"{}\"><td>{}</td><td><input class=\"new-language-target-id\" value=\"{}\"></td></tr>",
                    escape_html(label),
                    escape_html(label),
                    escape_html(target["target_id"].as_str().unwrap_or("")),
                ));
            }
            html.push_str("</tbody></table><pre>");
            html.push_str(&escape_html(value["manifest_yaml"].as_str().unwrap_or("")));
            html.push_str("</pre></div>");
            render_new_language_status(&html, true);
        }
        Err(error) => {
            render_new_language_status(&escape_html(&format!("Preview failed: {error}")), false)
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn render_new_language_status(html: &str, ok: bool) {
    if let Some(output) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("new-language-preview-output"))
    {
        output.set_inner_html(html);
        let _ = output.set_attribute("data-validation-ok", if ok { "true" } else { "false" });
    }
}

#[cfg(target_arch = "wasm32")]
fn collect_new_language_request(document: &web_sys::Document) -> Option<Value> {
    let preview = document
        .query_selector(".new-language-preview")
        .ok()
        .flatten()?;
    let code = preview.get_attribute("data-code")?;
    let template_language = preview.get_attribute("data-template")?;
    let primary_target = preview.get_attribute("data-primary-target")?;
    let display_name =
        input_value(document, "new-language-display-name").unwrap_or_else(|| code.clone());
    let mut groups = Vec::new();
    let group_rows = document
        .query_selector_all(".new-language-group-row")
        .ok()?;
    for index in 0..group_rows.length() {
        let Some(row) = group_rows.item(index) else {
            continue;
        };
        let Ok(row) = row.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let selected = row
            .query_selector(".new-language-group-selected")
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            .is_some_and(|input| input.checked());
        let overlay_id = row
            .query_selector(".new-language-group-overlay-id")
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            .map(|input| input.value())
            .unwrap_or_default();
        let file = row
            .query_selector(".new-language-group-file")
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            .map(|input| input.value())
            .unwrap_or_default();
        groups.push(serde_json::json!({
            "label": row.get_attribute("data-label").unwrap_or_default(),
            "template_overlay_id": row.get_attribute("data-template-overlay-id").unwrap_or_default(),
            "overlay_id": overlay_id,
            "file": file,
            "selected": selected,
        }));
    }
    let mut targets = Vec::new();
    let target_rows = document
        .query_selector_all(".new-language-target-row")
        .ok()?;
    for index in 0..target_rows.length() {
        let Some(row) = target_rows.item(index) else {
            continue;
        };
        let Ok(row) = row.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let target_id = row
            .query_selector(".new-language-target-id")
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            .map(|input| input.value())
            .unwrap_or_default();
        targets.push(serde_json::json!({
            "label": row.get_attribute("data-label").unwrap_or_default(),
            "target_id": target_id,
        }));
    }
    Some(serde_json::json!({
        "code": code,
        "display_name": display_name,
        "template_language": template_language,
        "primary_target": primary_target,
        "groups": groups,
        "targets": targets,
    }))
}

#[cfg(target_arch = "wasm32")]
fn new_language_result_text(heading: &str, value: &Value) -> String {
    let mut lines = vec![heading.to_owned()];
    lines.push(format!(
        "Language: {}",
        value["language"].as_str().unwrap_or("unknown")
    ));
    if let Some(files) = value["workspace"]["fingerprints"].as_array() {
        lines.push(format!("Workspace now tracks {} file(s).", files.len()));
    }
    escape_html(&lines.join("\n"))
}

#[cfg(target_arch = "wasm32")]
fn active_filter(document: &web_sys::Document) -> Option<String> {
    document
        .query_selector(".pivot-filters button.active")
        .ok()
        .flatten()
        .and_then(|element| element.get_attribute("data-filter"))
}

#[cfg(target_arch = "wasm32")]
fn reload_note_pivot(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch_note_pivot_query(language, target, overlay, filter).await {
            Ok(pivot) => publish_note_pivot_panel(&pivot),
            Err(error) => publish_note_pivot_error(&error),
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn register_field_handlers(pivot: &Value) {
    for note in pivot["notes"].as_array().into_iter().flatten() {
        let note_id = note["note_id"].as_str().unwrap_or("").to_owned();
        for field in note["fields"].as_array().into_iter().flatten() {
            let path = field["path"].as_str().unwrap_or("").to_owned();
            let source = field["source"].as_str().unwrap_or("").to_owned();
            let field_id = field["field_id"].as_str().unwrap_or("").to_owned();
            let id = id_for_path(&path);
            if field["source_editable"].as_bool().unwrap_or(false) {
                attach_source_stage_handler(
                    pivot,
                    &id,
                    path.clone(),
                    source.clone(),
                    note_id.clone(),
                    field_id.clone(),
                );
            }
            if field["editable"].as_bool().unwrap_or(false) {
                attach_stage_handler(pivot, &id, path, source, note_id.clone(), field_id);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn register_optional_metadata_handlers(optional: &Value) {
    let language = optional["language"]["code"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let target = optional["target"]["label"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let overlay = optional["overlay"]["label"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    for item in optional["items"].as_array().into_iter().flatten() {
        let path = item["path"].as_str().unwrap_or("").to_owned();
        let source = item["source"].as_str().unwrap_or("").to_owned();
        let id = id_for_path(&path);
        attach_optional_metadata_handler(
            language.clone(),
            target.clone(),
            overlay.clone(),
            id,
            path,
            source,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_optional_metadata_handler(
    language: String,
    target: String,
    overlay: String,
    id: String,
    path: String,
    source: String,
) {
    let input_id = format!("optional-translation-input-{id}");
    let status_id = format!("optional-status-{id}");
    let prefix = storage_prefix_for_parts(&language, &target, &overlay);
    let key = format!("{prefix}translation::{path}::{source}");
    let closure_input_id = input_id.clone();
    let closure_status_id = status_id.clone();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let Some(input) = document
            .get_element_by_id(&closure_input_id)
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let edit = serde_json::json!({
            "kind": "translation",
            "path": path.clone(),
            "source": source.clone(),
            "value": input.value(),
            "mode": "direct",
            "language": language.clone(),
            "target": target.clone(),
            "overlay": overlay.clone(),
        });
        if let Some(storage) = local_storage() {
            let _ = storage.set_item(&key, &edit.to_string());
        }
        if let Some(status) = document.get_element_by_id(&closure_status_id) {
            status.set_text_content(Some("staged_direct"));
        }
        update_staged_count(&prefix);
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&input_id))
    {
        let _ = element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
fn attach_stage_handler(
    pivot: &Value,
    id: &str,
    path: String,
    source: String,
    note_id: String,
    field_id: String,
) {
    let key = edit_storage_key(pivot, &path, &source);
    let prefix = storage_prefix(pivot);
    let input_id = format!("translation-input-{id}");
    let mode_id = format!("translation-mode-{id}");
    let source_input_id = format!("source-input-{id}");
    let target_id = format!("target-text-{id}");
    let status_id = format!("status-text-{id}");
    for element_id in [input_id.clone(), mode_id.clone()] {
        let key = key.clone();
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let input_id = input_id.clone();
        let mode_id = mode_id.clone();
        let source_input_id = source_input_id.clone();
        let target_id = target_id.clone();
        let status_id = status_id.clone();
        let note_id = note_id.clone();
        let field_id = field_id.clone();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let Some(input) = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            else {
                return;
            };
            let Some(mode) = document
                .get_element_by_id(&mode_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
            else {
                return;
            };
            let value = input.value();
            let source_key = source_edit_storage_key_from_parts(&prefix, &path, &source);
            let effective_source = local_storage()
                .and_then(|storage| storage.get_item(&source_key).ok().flatten())
                .and_then(|stored| serde_json::from_str::<Value>(&stored).ok())
                .and_then(|edit| edit["value"].as_str().map(str::to_owned))
                .or_else(|| {
                    document
                        .get_element_by_id(&source_input_id)
                        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                        .map(|input| input.value())
                        .filter(|value| value != &source)
                })
                .unwrap_or_else(|| source.clone());
            let edit = serde_json::json!({
                "kind": "translation",
                "path": path,
                "source": effective_source,
                "value": value,
                "mode": mode.value(),
            });
            if let Some(storage) = local_storage() {
                let _ = storage.set_item(&key, &edit.to_string());
            }
            if let Some(target) = document.get_element_by_id(&target_id) {
                target.set_text_content(Some(&input.value()));
            }
            if let Some(status) = document.get_element_by_id(&status_id) {
                status.set_text_content(Some(&format!("staged_{}", mode.value())));
            }
            update_target_preview_field(&document, &note_id, &field_id, &input.value());
            update_staged_count(&prefix);
            refresh_progress_from_dom();
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&element_id))
        {
            let _ =
                element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            let _ = element
                .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn attach_source_stage_handler(
    pivot: &Value,
    id: &str,
    path: String,
    source: String,
    note_id: String,
    field_id: String,
) {
    let key = source_edit_storage_key(pivot, &path, &source);
    let prefix = storage_prefix(pivot);
    let input_id = format!("source-input-{id}");
    let scope_id = format!("source-scope-{id}");
    let impact_id = format!("source-impact-{id}");
    let source_text_id = format!("source-text-{id}");
    let status_id = format!("status-text-{id}");
    let toggle_id = format!("source-edit-toggle-{id}");
    let translation_key = edit_storage_key(pivot, &path, &source);

    {
        let input_id = input_id.clone();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            if let Some(input) = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                input.set_read_only(false);
                let _ = input.focus();
            }
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&toggle_id))
        {
            let _ =
                element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }

    for element_id in [input_id.clone(), scope_id.clone(), impact_id.clone()] {
        let key = key.clone();
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let input_id = input_id.clone();
        let scope_id = scope_id.clone();
        let impact_id = impact_id.clone();
        let source_text_id = source_text_id.clone();
        let status_id = status_id.clone();
        let translation_key = translation_key.clone();
        let note_id = note_id.clone();
        let field_id = field_id.clone();
        let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
            let Some(document) = web_sys::window().and_then(|window| window.document()) else {
                return;
            };
            let Some(input) = document
                .get_element_by_id(&input_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
            else {
                return;
            };
            let Some(scope) = document
                .get_element_by_id(&scope_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
            else {
                return;
            };
            let Some(impact) = document
                .get_element_by_id(&impact_id)
                .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
            else {
                return;
            };
            let value = input.value();
            let edit = serde_json::json!({
                "kind": "source",
                "path": path,
                "source": source,
                "value": value,
                "scope": scope.value(),
                "impact_action": impact.value(),
            });
            if let Some(storage) = local_storage() {
                let _ = storage.set_item(&key, &edit.to_string());
                if let Some(stored_translation) = storage.get_item(&translation_key).ok().flatten()
                    && let Ok(mut translation_edit) =
                        serde_json::from_str::<Value>(&stored_translation)
                {
                    translation_edit["source"] = Value::String(value.clone());
                    let _ = storage.set_item(&translation_key, &translation_edit.to_string());
                }
            }
            if let Some(source_text) = document.get_element_by_id(&source_text_id) {
                source_text.set_text_content(Some(&input.value()));
            }
            if let Some(status) = document.get_element_by_id(&status_id) {
                status.set_text_content(Some("staged_source"));
            }
            update_source_preview_field(&document, &note_id, &field_id, &input.value());
            update_staged_count(&prefix);
            refresh_progress_from_dom();
        }));
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&element_id))
        {
            let _ =
                element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            let _ = element
                .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn restore_staged_dom_state(pivot: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    for note in pivot["notes"].as_array().into_iter().flatten() {
        let note_id = note["note_id"].as_str().unwrap_or("");
        for field in note["fields"].as_array().into_iter().flatten() {
            let path = field["path"].as_str().unwrap_or("");
            let source = field["source"].as_str().unwrap_or("");
            let field_id = field["field_id"].as_str().unwrap_or("");
            let id = id_for_path(path);
            if let Some(staged_source) = staged_source_edit_for(pivot, path, source) {
                let value = staged_source["value"].as_str().unwrap_or(source);
                if let Some(input) = document
                    .get_element_by_id(&format!("source-input-{id}"))
                    .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                {
                    input.set_value(value);
                    input.set_read_only(false);
                }
                if let Some(source_text) = document.get_element_by_id(&format!("source-text-{id}"))
                {
                    source_text.set_text_content(Some(value));
                }
                if let Some(status) = document.get_element_by_id(&format!("status-text-{id}")) {
                    status.set_text_content(Some("staged_source"));
                }
                update_source_preview_field(&document, note_id, field_id, value);
            }
            let Some(staged) = staged_edit_for(pivot, path, source) else {
                continue;
            };
            let value = staged["value"].as_str().unwrap_or("");
            let mode = staged["mode"].as_str().unwrap_or("direct");
            if let Some(target) = document.get_element_by_id(&format!("target-text-{id}")) {
                target.set_text_content(Some(value));
            }
            if let Some(status) = document.get_element_by_id(&format!("status-text-{id}")) {
                status.set_text_content(Some(&format!("staged_{mode}")));
            }
            update_target_preview_field(&document, note_id, field_id, value);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn update_target_preview_field(
    document: &web_sys::Document,
    note_id: &str,
    field_id: &str,
    value: &str,
) {
    update_preview_field(document, note_id, field_id, ".target-preview", value);
}

#[cfg(target_arch = "wasm32")]
fn update_source_preview_field(
    document: &web_sys::Document,
    note_id: &str,
    field_id: &str,
    value: &str,
) {
    update_preview_field(document, note_id, field_id, "section:first-child", value);
}

#[cfg(target_arch = "wasm32")]
fn update_preview_field(
    document: &web_sys::Document,
    note_id: &str,
    field_id: &str,
    preview_selector: &str,
    value: &str,
) {
    let selector = format!(
        ".note-card[data-note-id=\"{}\"] {} [data-preview-field-id=\"{}\"]",
        note_id, preview_selector, field_id
    );
    let Ok(nodes) = document.query_selector_all(&selector) else {
        return;
    };
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            element.set_inner_html(value);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn refresh_progress_from_dom() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(rows) = document.query_selector_all("tr[data-editable=\"true\"]") else {
        return;
    };
    let mut complete = 0;
    let mut missing = 0;
    for index in 0..rows.length() {
        let Some(node) = rows.get(index) else {
            continue;
        };
        let Ok(row) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let status = row
            .get_attribute("data-field-path")
            .and_then(|path| {
                document.get_element_by_id(&format!("status-text-{}", id_for_path(&path)))
            })
            .and_then(|element| element.text_content())
            .unwrap_or_default();
        if status.starts_with("staged_")
            || matches!(
                status.as_str(),
                "direct_translation" | "contextual_override" | "no_change"
            )
        {
            complete += 1;
        } else if status == "untranslated_fallback" {
            missing += 1;
        }
    }
    if let Some(progress) = document.get_element_by_id("translation-progress") {
        let total = rows.length();
        let stale = progress
            .get_attribute("data-stale")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let staged = document
            .get_element_by_id("staged-edit-count")
            .and_then(|element| element.text_content())
            .unwrap_or_else(|| "0".to_owned());
        let _ = progress.set_attribute("data-complete", &complete.to_string());
        let _ = progress.set_attribute("data-missing", &missing.to_string());
        progress.set_inner_html(&format!(
            "Main note-field progress: {complete} / {total} complete, {missing} missing, {stale} stale (<span id=\"staged-edit-count\" data-count=\"{staged}\">{staged}</span> staged)"
        ));
    }
}

#[cfg(target_arch = "wasm32")]
fn register_apply_handlers(pivot: &Value) {
    attach_apply_handler(pivot, "apply-preview-button", false);
    attach_apply_handler(pivot, "apply-confirm-button", true);
}

#[cfg(target_arch = "wasm32")]
fn attach_apply_handler(pivot: &Value, button_id: &str, write: bool) {
    let pivot = pivot.clone();
    let button_id = button_id.to_owned();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let pivot = pivot.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let edits = collect_staged_edits(&pivot);
            let request = serde_json::json!({
                "language": pivot["language"]["code"].as_str().unwrap_or(""),
                "target": pivot["target"]["label"].as_str().unwrap_or(""),
                "overlay": pivot["overlay"]["label"].as_str().unwrap_or(""),
                "edits": edits,
            });
            let endpoint = if write {
                "/api/workbench/apply"
            } else {
                "/api/workbench/apply-preview"
            };
            let result = post_json(endpoint, &request).await;
            render_apply_result(&pivot, write, result).await;
        });
    }));
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&button_id))
    {
        let _ = element.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}

#[cfg(target_arch = "wasm32")]
async fn render_apply_result(pivot: &Value, write: bool, result: Result<Value, String>) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(output) = document.get_element_by_id("apply-preview-output") else {
        return;
    };
    match result {
        Ok(value) => {
            output.set_text_content(Some(&apply_result_text(
                if write { "Applied" } else { "Apply preview" },
                &value,
            )));
            let _ =
                output.set_attribute("data-validation-ok", &value["validation"]["ok"].to_string());
            if write {
                for prefix in active_storage_prefixes(pivot) {
                    clear_staged_edits(&prefix);
                }
                if let Ok(updated) = fetch_note_pivot_for_pivot(pivot).await {
                    publish_note_pivot_panel(&updated);
                }
                if let Some(output) = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("apply-preview-output"))
                {
                    output.set_text_content(Some(&apply_result_text("Applied", &value)));
                    let _ = output.set_attribute("data-validation-ok", "true");
                }
            }
        }
        Err(error) => {
            output.set_text_content(Some(&format!("Apply failed: {error}")));
            let _ = output.set_attribute("data-validation-ok", "false");
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_result_text(heading: &str, value: &Value) -> String {
    let mut lines = vec![heading.to_owned(), "Affected files:".to_owned()];
    for file in value["affected_files"].as_array().into_iter().flatten() {
        lines.push(format!(
            "- {}",
            file["path"].as_str().unwrap_or("unknown overlay")
        ));
    }
    lines.push("Grouped changes:".to_owned());
    for file_group in value["file_groups"].as_array().into_iter().flatten() {
        lines.push(format!(
            "- {}",
            file_group["file"].as_str().unwrap_or("unknown file")
        ));
        for group in file_group["content_groups"]
            .as_array()
            .into_iter()
            .flatten()
        {
            lines.push(format!(
                "  - {}: {} change(s)",
                group["name"].as_str().unwrap_or("workspace"),
                group["change_count"].as_u64().unwrap_or(0)
            ));
        }
    }
    lines.push("Changed entries:".to_owned());
    for entry in value["changed_entries"].as_array().into_iter().flatten() {
        let old_value = entry["source"]
            .as_str()
            .or_else(|| entry["old_source"].as_str())
            .or_else(|| entry["old"].as_str())
            .unwrap_or("");
        let new_value = entry["new"]
            .as_str()
            .or_else(|| entry["new_source"].as_str())
            .or_else(|| entry["target"].as_str())
            .unwrap_or("");
        lines.push(format!(
            "- {} {}: {} -> {}",
            entry["mode"].as_str().unwrap_or("edit"),
            entry["path"].as_str().unwrap_or("unknown path"),
            old_value,
            new_value
        ));
    }
    lines.push(format!(
        "Validation: {}",
        value["validation"]["ok"].as_bool().unwrap_or(false)
    ));
    for error in value["validation"]["errors"]
        .as_array()
        .into_iter()
        .flatten()
    {
        lines.push(format!(
            "- validation error: {}",
            error.as_str().unwrap_or("unknown")
        ));
    }
    lines.join("\n")
}

#[cfg(target_arch = "wasm32")]
async fn post_json(url: &str, body: &Value) -> Result<Value, String> {
    gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn staged_edit_for(pivot: &Value, path: &str, source: &str) -> Option<Value> {
    local_storage()
        .and_then(|storage| {
            storage
                .get_item(&edit_storage_key(pivot, path, source))
                .ok()
                .flatten()
        })
        .and_then(|value| serde_json::from_str(&value).ok())
}

#[cfg(target_arch = "wasm32")]
fn staged_source_edit_for(pivot: &Value, path: &str, source: &str) -> Option<Value> {
    local_storage()
        .and_then(|storage| {
            storage
                .get_item(&source_edit_storage_key(pivot, path, source))
                .ok()
                .flatten()
        })
        .and_then(|value| serde_json::from_str(&value).ok())
}

#[cfg(target_arch = "wasm32")]
fn collect_staged_edits(pivot: &Value) -> Vec<Value> {
    let prefixes = active_storage_prefixes(pivot);
    let Some(storage) = local_storage() else {
        return Vec::new();
    };
    let mut edits = Vec::new();
    let length = storage.length().unwrap_or(0);
    for index in 0..length {
        let Some(key) = storage.key(index).ok().flatten() else {
            continue;
        };
        if !prefixes.iter().any(|prefix| {
            key.starts_with(&format!("{prefix}translation::"))
                || key.starts_with(&format!("{prefix}source::"))
        }) {
            continue;
        }
        if let Some(value) = storage.get_item(&key).ok().flatten()
            && let Ok(edit) = serde_json::from_str::<Value>(&value)
        {
            edits.push(edit);
        }
    }
    edits
}

#[cfg(target_arch = "wasm32")]
fn active_storage_prefixes(pivot: &Value) -> Vec<String> {
    active_storage_prefixes_for_default(&storage_prefix(pivot))
}

#[cfg(target_arch = "wasm32")]
fn active_storage_prefixes_for_default(default_prefix: &str) -> Vec<String> {
    let mut prefixes = vec![default_prefix.to_owned()];
    if let Some(document) = web_sys::window().and_then(|window| window.document())
        && let Ok(nodes) = document.query_selector_all("[data-storage-prefix]")
    {
        for index in 0..nodes.length() {
            if let Some(node) = nodes.get(index)
                && let Ok(element) = node.dyn_into::<web_sys::Element>()
                && let Some(prefix) = element.get_attribute("data-storage-prefix")
                && !prefixes.iter().any(|existing| existing == &prefix)
            {
                prefixes.push(prefix);
            }
        }
    }
    prefixes
}

#[cfg(target_arch = "wasm32")]
fn clear_staged_edits(prefix: &str) {
    let Some(storage) = local_storage() else {
        return;
    };
    let keys = (0..storage.length().unwrap_or(0))
        .filter_map(|index| storage.key(index).ok().flatten())
        .filter(|key| key.starts_with(prefix))
        .collect::<Vec<_>>();
    for key in keys {
        let _ = storage.remove_item(&key);
    }
}

#[cfg(target_arch = "wasm32")]
fn update_staged_count_for_pivot(pivot: &Value) {
    update_staged_count_for_prefixes(&active_storage_prefixes(pivot));
}

#[cfg(target_arch = "wasm32")]
fn update_staged_count(prefix: &str) {
    update_staged_count_for_prefixes(&[prefix.to_owned()]);
}

#[cfg(target_arch = "wasm32")]
fn update_staged_count_for_prefixes(prefixes: &[String]) {
    let count = local_storage().map_or(0, |storage| {
        (0..storage.length().unwrap_or(0))
            .filter_map(|index| storage.key(index).ok().flatten())
            .filter(|key| prefixes.iter().any(|prefix| key.starts_with(prefix)))
            .count()
    });
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("staged-edit-count"))
    {
        element.set_text_content(Some(&count.to_string()));
        let _ = element.set_attribute("data-count", &count.to_string());
    }
}

#[cfg(target_arch = "wasm32")]
fn edit_storage_key(pivot: &Value, path: &str, source: &str) -> String {
    format!("{}translation::{}::{}", storage_prefix(pivot), path, source)
}

#[cfg(target_arch = "wasm32")]
fn source_edit_storage_key(pivot: &Value, path: &str, source: &str) -> String {
    source_edit_storage_key_from_parts(&storage_prefix(pivot), path, source)
}

#[cfg(target_arch = "wasm32")]
fn source_edit_storage_key_from_parts(prefix: &str, path: &str, source: &str) -> String {
    format!("{prefix}source::{path}::{source}")
}

#[cfg(target_arch = "wasm32")]
fn storage_prefix(pivot: &Value) -> String {
    storage_prefix_for_parts(
        pivot["language"]["code"].as_str().unwrap_or("language"),
        pivot["target"]["label"].as_str().unwrap_or("target"),
        pivot["overlay"]["label"].as_str().unwrap_or("overlay"),
    )
}

#[cfg(target_arch = "wasm32")]
fn pivot_selection_parts(pivot: &Value) -> (Option<String>, Option<String>, Option<String>) {
    (
        pivot["language"]["code"].as_str().map(str::to_owned),
        pivot["target"]["label"].as_str().map(str::to_owned),
        pivot["overlay"]["label"].as_str().map(str::to_owned),
    )
}

#[cfg(target_arch = "wasm32")]
fn storage_prefix_for_parts(language: &str, target: &str, overlay: &str) -> String {
    format!("brainbrew.workbench.staged.{language}.{target}.{overlay}::")
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

#[cfg(target_arch = "wasm32")]
fn id_for_path(path: &str) -> String {
    path.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(target_arch = "wasm32")]
async fn fetch_workspace() -> Result<WorkspaceSummary, String> {
    let value = gloo_net::http::Request::get("/api/workspace")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    Ok(WorkspaceSummary::from_workspace_json(&value))
}

#[cfg(target_arch = "wasm32")]
async fn fetch_note_pivot() -> Result<Value, String> {
    fetch_note_pivot_query(None, None, None, None).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_note_pivot_for_pivot(pivot: &Value) -> Result<Value, String> {
    fetch_note_pivot_query(
        pivot["language"]["code"].as_str().map(str::to_owned),
        pivot["target"]["label"].as_str().map(str::to_owned),
        pivot["overlay"]["label"].as_str().map(str::to_owned),
        pivot["filters"]["active"].as_str().map(str::to_owned),
    )
    .await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_note_pivot_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    if let Some(filter) = filter.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!("filter={}", encode_query_component(&filter)));
    }
    get_workbench_json("/api/workbench/note-pivot", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_comparison_pane_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
) -> Result<Value, String> {
    let params = workbench_selection_params(language, target, overlay);
    get_workbench_json("/api/workbench/comparison-pane", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_optional_metadata_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
) -> Result<Value, String> {
    let params = workbench_selection_params(language, target, overlay);
    get_workbench_json("/api/workbench/optional-metadata", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_card_pivot_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    card: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    if let Some(card) = card.filter(|value| !value.is_empty()) {
        params.push(format!("card={}", encode_query_component(&card)));
    }
    if let Some(filter) = filter.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!("filter={}", encode_query_component(&filter)));
    }
    if let Some(content_group) = content_group.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!(
            "content_group={}",
            encode_query_component(&content_group)
        ));
    }
    get_workbench_json("/api/workbench/card-pivot", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_new_language_preview(
    template: Option<String>,
    code: Option<String>,
    display_name: Option<String>,
) -> Result<Value, String> {
    let mut params = Vec::new();
    if let Some(template) = template.filter(|value| !value.is_empty()) {
        params.push(format!("template={}", encode_query_component(&template)));
    }
    if let Some(code) = code.filter(|value| !value.is_empty()) {
        params.push(format!("code={}", encode_query_component(&code)));
    }
    if let Some(display_name) = display_name.filter(|value| !value.is_empty()) {
        params.push(format!(
            "display_name={}",
            encode_query_component(&display_name)
        ));
    }
    get_workbench_json("/api/workbench/new-language-preview", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_source_string_pivot_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    source: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    if let Some(source) = source.filter(|value| !value.is_empty()) {
        params.push(format!("source={}", encode_query_component(&source)));
    }
    if let Some(content_group) = content_group.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!(
            "content_group={}",
            encode_query_component(&content_group)
        ));
    }
    if let Some(status) = status.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!("status={}", encode_query_component(&status)));
    }
    get_workbench_json("/api/workbench/source-string-pivot", params).await
}

#[cfg(target_arch = "wasm32")]
fn workbench_selection_params(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
) -> Vec<String> {
    let mut params = Vec::new();
    if let Some(language) = language.filter(|value| !value.is_empty()) {
        params.push(format!("language={}", encode_query_component(&language)));
    }
    if let Some(target) = target.filter(|value| !value.is_empty()) {
        params.push(format!("target={}", encode_query_component(&target)));
    }
    if let Some(overlay) = overlay.filter(|value| !value.is_empty()) {
        params.push(format!("overlay={}", encode_query_component(&overlay)));
    }
    params
}

#[cfg(target_arch = "wasm32")]
async fn get_workbench_json(base_url: &str, params: Vec<String>) -> Result<Value, String> {
    let url = if params.is_empty() {
        base_url.to_owned()
    } else {
        format!("{}?{}", base_url, params.join("&"))
    };
    gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn encode_query_component(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_workspace() -> Result<WorkspaceSummary, String> {
    Ok(WorkspaceSummary {
        manifest: "Run through `brainbrew workbench serve` to fetch /api/workspace".to_owned(),
        language_count: 0,
        target_count: 0,
        fingerprint_count: 0,
    })
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_note_pivot() -> Result<Value, String> {
    Ok(serde_json::json!({"notes": []}))
}
