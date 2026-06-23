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
    html.push_str("<article class=\"workbench-panel\">");
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
        "<section id=\"translation-progress\" data-complete=\"{}\" data-total=\"{}\" data-missing=\"{}\" data-stale=\"{}\">Progress: {} / {} complete, {} missing, {} stale (<span id=\"staged-edit-count\">0</span> staged)</section>",
        progress["complete"].as_u64().unwrap_or(0),
        progress["total"].as_u64().unwrap_or(0),
        progress["missing"].as_u64().unwrap_or(0),
        progress["stale"].as_u64().unwrap_or(0),
        progress["complete"].as_u64().unwrap_or(0),
        progress["total"].as_u64().unwrap_or(0),
        progress["missing"].as_u64().unwrap_or(0),
        progress["stale"].as_u64().unwrap_or(0),
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

    html.push_str("<section id=\"source-string-pivot-panel\" class=\"source-string-pivot\"><p>Loading source string pivot…</p></section>");
    html.push_str("<section class=\"apply-box\"><button id=\"apply-preview-button\" type=\"button\">Apply preview</button> <button id=\"apply-confirm-button\" type=\"button\">Confirm Apply</button><pre id=\"apply-preview-output\"></pre></section>");
    html.push_str("</article>");
    panel.set_inner_html(&html);

    register_control_handlers(pivot);
    register_field_handlers(pivot);
    register_apply_handlers(pivot);
    restore_staged_dom_state(pivot);
    update_staged_count_for_pivot(pivot);
    refresh_progress_from_dom();
    load_source_string_pivot_for_pivot(pivot, None, None, None);
}

#[cfg(not(target_arch = "wasm32"))]
fn publish_note_pivot_panel(_pivot: &Value) {}

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
    register_source_string_handlers(pivot);
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
    let pivot = pivot.clone();
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let content_group = selected_value(&document, "source-string-content-group-filter");
        load_source_string_pivot_for_pivot(&pivot, None, content_group, None);
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
    let pivot = pivot.clone();
    let selector = format!(
        ".source-string-row[data-source=\"{}\"]",
        css_escape(&source)
    );
    let closure = Closure::<dyn FnMut(_)>::wrap(Box::new(move |_event: web_sys::Event| {
        load_source_string_pivot_for_pivot(&pivot, Some(source.clone()), None, None);
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
fn load_source_string_pivot_for_pivot(
    pivot: &Value,
    source: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
) {
    let language = pivot["language"]["code"].as_str().map(str::to_owned);
    let target = pivot["target"]["label"].as_str().map(str::to_owned);
    let overlay = pivot["overlay"]["label"].as_str().map(str::to_owned);
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
            "Progress: {complete} / {total} complete, {missing} missing, {stale} stale (<span id=\"staged-edit-count\" data-count=\"{staged}\">{staged}</span> staged)"
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
                clear_staged_edits(&storage_prefix(pivot));
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
    let prefix = storage_prefix(pivot);
    let Some(storage) = local_storage() else {
        return Vec::new();
    };
    let mut edits = Vec::new();
    let length = storage.length().unwrap_or(0);
    for index in 0..length {
        let Some(key) = storage.key(index).ok().flatten() else {
            continue;
        };
        if !key.starts_with(&format!("{prefix}translation::"))
            && !key.starts_with(&format!("{prefix}source::"))
        {
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
    update_staged_count(&storage_prefix(pivot));
}

#[cfg(target_arch = "wasm32")]
fn update_staged_count(prefix: &str) {
    let count = local_storage().map_or(0, |storage| {
        (0..storage.length().unwrap_or(0))
            .filter_map(|index| storage.key(index).ok().flatten())
            .filter(|key| key.starts_with(prefix))
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
    format!(
        "brainbrew.workbench.staged.{}.{}.{}::",
        pivot["language"]["code"].as_str().unwrap_or("language"),
        pivot["target"]["label"].as_str().unwrap_or("target"),
        pivot["overlay"]["label"].as_str().unwrap_or("overlay"),
    )
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
