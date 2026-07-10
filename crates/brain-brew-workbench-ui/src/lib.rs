use serde_json::Value;

#[cfg(target_arch = "wasm32")]
mod staging;

#[cfg(target_arch = "wasm32")]
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use std::sync::OnceLock;

#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
const STALE_WORKBENCH_REQUEST: &str = "__brainbrew_stale_workbench_request__";

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct WorkbenchSignals {
    selection_generation: RwSignal<u64>,
    selected_view: RwSignal<WorkbenchView>,
    top_level_error: RwSignal<Option<String>>,
    note_pivot: RwSignal<Option<Value>>,
    selected_note: RwSignal<Option<String>>,
    note_detail: RwSignal<NoteDetailState>,
    note_detail_token: RwSignal<u64>,
    card_list: RwSignal<LazyValueState>,
    selected_card: RwSignal<Option<String>>,
    card_detail: RwSignal<DetailValueState>,
    card_detail_token: RwSignal<u64>,
    source_string_list: RwSignal<LazyValueState>,
    selected_source_string: RwSignal<Option<String>>,
    source_string_detail: RwSignal<DetailValueState>,
    source_string_detail_token: RwSignal<u64>,
    optional_metadata: RwSignal<LazyValueState>,
    current_note_pivot: RwSignal<Option<Value>>,
}

#[cfg(target_arch = "wasm32")]
static WORKBENCH_SIGNALS: OnceLock<WorkbenchSignals> = OnceLock::new();

#[cfg(target_arch = "wasm32")]
fn install_workbench_signals(signals: WorkbenchSignals) {
    let _ = WORKBENCH_SIGNALS.set(signals);
}

#[cfg(target_arch = "wasm32")]
fn workbench_signals() -> Option<&'static WorkbenchSignals> {
    WORKBENCH_SIGNALS.get()
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkbenchView {
    Notes,
    Cards,
    SourceStrings,
    Metadata,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Default)]
enum NoteDetailState {
    #[default]
    Empty,
    Loading,
    Error(String),
    Loaded(Value),
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Default)]
enum LazyValueState {
    #[default]
    Unloaded,
    Loading(String),
    Error(String),
    Loaded(Value),
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Default)]
enum DetailValueState {
    #[default]
    Empty,
    Loading(String),
    Error(String),
    Loaded(Value),
}

#[cfg(target_arch = "wasm32")]
impl WorkbenchView {
    fn key(self) -> &'static str {
        match self {
            Self::Notes => "notes",
            Self::Cards => "cards",
            Self::SourceStrings => "source-strings",
            Self::Metadata => "metadata",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Notes => "Notes",
            Self::Cards => "Cards",
            Self::SourceStrings => "Source strings",
            Self::Metadata => "Metadata",
        }
    }
}

/// Leptos CSR root for the workbench chrome. Leptos owns the workbench panel,
/// view switcher, lazy pivot panes, editing controls, and top-level error surface.
#[cfg(target_arch = "wasm32")]
#[component]
pub fn App() -> impl IntoView {
    let selection_generation = RwSignal::new(0_u64);
    let selected_view = RwSignal::new(WorkbenchView::Notes);
    let top_level_error = RwSignal::new(None::<String>);
    let write_capability = RwSignal::new(WorkbenchWriteCapability::default());
    let note_pivot = RwSignal::new(None::<Value>);
    let selected_note_id = RwSignal::new(None::<String>);
    let note_detail = RwSignal::new(NoteDetailState::Empty);
    let note_detail_token = RwSignal::new(0_u64);
    let card_list = RwSignal::new(LazyValueState::Unloaded);
    let selected_card_id = RwSignal::new(None::<String>);
    let card_detail = RwSignal::new(DetailValueState::Empty);
    let card_detail_token = RwSignal::new(0_u64);
    let source_string_list = RwSignal::new(LazyValueState::Unloaded);
    let selected_source_string = RwSignal::new(None::<String>);
    let source_string_detail = RwSignal::new(DetailValueState::Empty);
    let source_string_detail_token = RwSignal::new(0_u64);
    let optional_metadata = RwSignal::new(LazyValueState::Unloaded);
    let current_note_pivot = RwSignal::new(None::<Value>);
    staging::provide(staging::Staging::new());

    install_workbench_signals(WorkbenchSignals {
        selection_generation,
        selected_view,
        top_level_error,
        note_pivot,
        selected_note: selected_note_id,
        note_detail,
        note_detail_token,
        card_list,
        selected_card: selected_card_id,
        card_detail,
        card_detail_token,
        source_string_list,
        selected_source_string,
        source_string_detail,
        source_string_detail_token,
        optional_metadata,
        current_note_pivot,
    });

    publish_workspace_probe("loading", "Loading workspace metadata…", None);

    spawn_local(async move {
        match fetch_workspace().await {
            Ok(summary) => {
                write_capability.set(summary.write_capability.clone());
                publish_workspace_probe(
                    "loaded",
                    "Workspace metadata loaded from /api/workspace.",
                    Some(&summary),
                );
            }
            Err(error) => publish_workspace_probe(
                "error",
                &format!("Unable to load workspace metadata: {error}"),
                None,
            ),
        }
    });

    spawn_local(async {
        match fetch_note_pivot().await {
            Ok(pivot) => publish_note_pivot_panel(&pivot),
            Err(error) => {
                if !is_stale_workbench_request(&error) {
                    publish_note_pivot_error(&format!("Unable to load note pivot: {error}"));
                }
            }
        }
    });

    view! {
        <section id="workbench-dom-panel">
            <section
                class="workbench-error"
                hidden=move || top_level_error.get().is_none()
                aria-live="polite"
            >
                {move || top_level_error.get().unwrap_or_default()}
            </section>
            <article id="workbench-current-workspace" class="workbench-panel">
                <WorkbenchAccessBanner capability=write_capability />
                <GlobalControls pivot=note_pivot />
                <nav id="workbench-view-switch" class="workbench-view-switch" aria-label="Workbench views">
                    <button
                        id="view-notes"
                        class=move || workbench_view_button_class(selected_view.get(), WorkbenchView::Notes)
                        type="button"
                        data-view="notes"
                        aria-current=move || workbench_view_aria_current(selected_view.get(), WorkbenchView::Notes)
                        on:click=move |_| open_workbench_view(WorkbenchView::Notes)
                    >
                        {WorkbenchView::Notes.label()}
                    </button>
                    <button
                        id="view-cards"
                        class=move || workbench_view_button_class(selected_view.get(), WorkbenchView::Cards)
                        type="button"
                        data-view="cards"
                        aria-current=move || workbench_view_aria_current(selected_view.get(), WorkbenchView::Cards)
                        on:click=move |_| open_workbench_view(WorkbenchView::Cards)
                    >
                        {WorkbenchView::Cards.label()}
                    </button>
                    <button
                        id="view-source-strings"
                        class=move || workbench_view_button_class(selected_view.get(), WorkbenchView::SourceStrings)
                        type="button"
                        data-view="source-strings"
                        aria-current=move || workbench_view_aria_current(selected_view.get(), WorkbenchView::SourceStrings)
                        on:click=move |_| open_workbench_view(WorkbenchView::SourceStrings)
                    >
                        {WorkbenchView::SourceStrings.label()}
                    </button>
                    <button
                        id="view-metadata"
                        class=move || workbench_view_button_class(selected_view.get(), WorkbenchView::Metadata)
                        type="button"
                        data-view="metadata"
                        aria-current=move || workbench_view_aria_current(selected_view.get(), WorkbenchView::Metadata)
                        on:click=move |_| open_workbench_view(WorkbenchView::Metadata)
                    >
                        {WorkbenchView::Metadata.label()}
                    </button>
                </nav>
                <div class="workbench-view-stack">
                    <section
                        id="view-panel-notes"
                        class=move || workbench_view_panel_class(selected_view.get(), WorkbenchView::Notes)
                        data-view="notes"
                        hidden=move || selected_view.get() != WorkbenchView::Notes
                    >
                        <div id="note-pivot-panel">
                            <NotePivotInterior
                                pivot=note_pivot
                                selected_note_id=selected_note_id
                                detail=note_detail
                            />
                        </div>
                    </section>
                    <section
                        id="view-panel-cards"
                        class=move || workbench_view_panel_class(selected_view.get(), WorkbenchView::Cards)
                        data-view="cards"
                        hidden=move || selected_view.get() != WorkbenchView::Cards
                    >
                        <CardPivotPanel
                            state=card_list
                            selected_card_id=selected_card_id
                            detail=card_detail
                        />
                    </section>
                    <section
                        id="view-panel-source-strings"
                        class=move || workbench_view_panel_class(selected_view.get(), WorkbenchView::SourceStrings)
                        data-view="source-strings"
                        hidden=move || selected_view.get() != WorkbenchView::SourceStrings
                    >
                        <SourceStringPivotPanel
                            state=source_string_list
                            selected_source=selected_source_string
                            detail=source_string_detail
                        />
                    </section>
                    <section
                        id="view-panel-metadata"
                        class=move || workbench_view_panel_class(selected_view.get(), WorkbenchView::Metadata)
                        data-view="metadata"
                        hidden=move || selected_view.get() != WorkbenchView::Metadata
                    >
                        <OptionalMetadataPanel state=optional_metadata />
                    </section>
                </div>
                <WorkbenchWorkflowPanels pivot=note_pivot capability=write_capability />
                <section id="workbench-apply-box" class="apply-box">
                    <ApplyBox pivot=note_pivot capability=write_capability />
                </section>
            </article>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
fn workbench_view_panel_class(current: WorkbenchView, view: WorkbenchView) -> &'static str {
    if current == view {
        "workbench-view active"
    } else {
        "workbench-view"
    }
}

#[cfg(target_arch = "wasm32")]
fn workbench_view_button_class(current: WorkbenchView, view: WorkbenchView) -> &'static str {
    if current == view {
        "workbench-view-button active"
    } else {
        "workbench-view-button"
    }
}

#[cfg(target_arch = "wasm32")]
fn workbench_view_aria_current(current: WorkbenchView, view: WorkbenchView) -> &'static str {
    if current == view { "page" } else { "" }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn WorkbenchAccessBanner(capability: RwSignal<WorkbenchWriteCapability>) -> impl IntoView {
    view! {
        <section
            id="workbench-access-banner"
            class=move || if capability.get().enabled { "workbench-access-banner development-write" } else { "workbench-access-banner read-only" }
            data-write-enabled=move || capability.get().enabled.to_string()
            data-write-mode=move || capability.get().mode
            role="status"
        >
            <strong>{move || if capability.get().enabled { "Unsafe development writes enabled" } else { "Read-only Workbench" }}</strong>
            <p>{move || capability.get().warning}</p>
            <p>{move || if capability.get().enabled {
                "This build has the development-only write capability and the server was started with explicit write opt-in. The mode is not supported for release use."
            } else {
                "Browse, compare, and Apply preview remain available. Edits are staged only in this browser's localStorage, survive refresh, and are not written to source files. Keep or copy drafts before clearing browser data."
            }}</p>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn GlobalControls(pivot: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        {move || match pivot.get() {
            Some(pivot) => view! { <LoadedGlobalControls pivot=pivot /> }.into_any(),
            None => "".into_any(),
        }}
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedGlobalControls(pivot: Value) -> impl IntoView {
    let language = pivot["language"]["code"].as_str().unwrap_or("").to_owned();
    let target = pivot["target"]["label"].as_str().unwrap_or("").to_owned();
    let overlay = pivot["overlay"]["label"].as_str().unwrap_or("").to_owned();
    let progress = pivot["progress"].clone();
    let metadata_progress = pivot["metadata_progress"].clone();
    let badges = pivot["overlay_badges"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let active_filter = pivot["filters"]["active"]
        .as_str()
        .unwrap_or("all")
        .to_owned();
    let prefixes = active_storage_prefixes(&pivot);

    let effect_language = language.clone();
    let effect_target = target.clone();
    let effect_overlay = overlay.clone();
    Effect::new(move |_| {
        if let Some(panel) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("workbench-current-workspace"))
        {
            let _ = panel.set_attribute("data-language", &effect_language);
            let _ = panel.set_attribute("data-target", &effect_target);
            let _ = panel.set_attribute("data-overlay", &effect_overlay);
        }
        update_staged_count_for_prefixes(&prefixes);
    });

    view! {
        <header class="workbench-panel__header">
            <h2>Deck Workbench Note pivot</h2>
            <SelectionSelect
                id="language-select".to_owned()
                label="Language".to_owned()
                options=pivot["selection_options"]["languages"].clone()
                value_key="code".to_owned()
                label_key="display_name".to_owned()
                fallback=language.clone()
                on_change=Rc::new(move |value| reload_note_pivot(non_empty(value), None, None, current_active_filter()))
            />
            <SelectionSelect
                id="target-select".to_owned()
                label="Target".to_owned()
                options=pivot["selection_options"]["targets"].clone()
                value_key="label".to_owned()
                label_key="label".to_owned()
                fallback=target.clone()
                on_change=Rc::new(move |_| reload_note_pivot(current_selected_value("language-select"), current_selected_value("target-select"), current_selected_value("overlay-select"), current_active_filter()))
            />
            <SelectionSelect
                id="overlay-select".to_owned()
                label="Overlay".to_owned()
                options=pivot["selection_options"]["overlays"].clone()
                value_key="label".to_owned()
                label_key="label".to_owned()
                fallback=overlay.clone()
                on_change=Rc::new(move |_| reload_note_pivot(current_selected_value("language-select"), current_selected_value("target-select"), current_selected_value("overlay-select"), current_active_filter()))
            />
        </header>
        <section
            id="translation-progress"
            data-complete=progress["complete"].as_u64().unwrap_or(0).to_string()
            data-total=progress["total"].as_u64().unwrap_or(0).to_string()
            data-missing=progress["missing"].as_u64().unwrap_or(0).to_string()
            data-stale=progress["stale"].as_u64().unwrap_or(0).to_string()
        >
            {format!(
                "Main note-field progress: {} / {} complete, {} missing, {} stale (",
                progress["complete"].as_u64().unwrap_or(0),
                progress["total"].as_u64().unwrap_or(0),
                progress["missing"].as_u64().unwrap_or(0),
                progress["stale"].as_u64().unwrap_or(0),
            )}
            <span id="staged-edit-count" data-count="0">0</span>
            " staged)"
        </section>
        <section
            id="optional-progress"
            data-complete=metadata_progress["complete"].as_u64().unwrap_or(0).to_string()
            data-total=metadata_progress["total"].as_u64().unwrap_or(0).to_string()
            data-missing=metadata_progress["missing"].as_u64().unwrap_or(0).to_string()
            data-stale=metadata_progress["stale"].as_u64().unwrap_or(0).to_string()
        >
            {format!(
                "Metadata: {} / {} complete, {} missing, {} stale",
                metadata_progress["complete"].as_u64().unwrap_or(0),
                metadata_progress["total"].as_u64().unwrap_or(0),
                metadata_progress["missing"].as_u64().unwrap_or(0),
                metadata_progress["stale"].as_u64().unwrap_or(0),
            )}
        </section>
        <nav class="overlay-badges">
            <For
                each=move || badges.clone()
                key=|badge| badge["label"].as_str().unwrap_or("overlay").to_owned()
                children=move |badge| {
                    let class = if badge["active"].as_bool().unwrap_or(false) { "overlay-badge active" } else { "overlay-badge" };
                    view! { <span class=class>{badge["label"].as_str().unwrap_or("overlay").to_owned()}</span> }
                }
            />
        </nav>
        <nav class="pivot-filters" aria-label="Note pivot filters">
            <For
                each=move || vec![
                    ("all".to_owned(), "All".to_owned()),
                    ("missing".to_owned(), "Missing".to_owned()),
                    ("stale".to_owned(), "Stale".to_owned()),
                    ("needs_work".to_owned(), "Needs work".to_owned()),
                ]
                key=|(filter, _)| filter.clone()
                children=move |(filter, label)| {
                    let class = if active_filter == filter { "active" } else { "" };
                    view! {
                        <button
                            type="button"
                            data-filter=filter.clone()
                            class=class
                            on:click=move |_| reload_note_pivot(
                                current_selected_value("language-select"),
                                current_selected_value("target-select"),
                                current_selected_value("overlay-select"),
                                Some(filter.clone()),
                            )
                        >{label}</button>
                    }
                }
            />
        </nav>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn SelectionSelect(
    id: String,
    label: String,
    options: Value,
    value_key: String,
    label_key: String,
    fallback: String,
    on_change: Rc<dyn Fn(String)>,
) -> impl IntoView {
    let mut rows = options.as_array().cloned().unwrap_or_default();
    if rows.is_empty() {
        rows.push(serde_json::json!({ value_key.clone(): fallback.clone(), label_key.clone(): fallback.clone(), "active": true }));
    }
    view! {
        <label>{label}" "
            <select id=id on:change=move |event| on_change(event_select_value(&event))>
                <For
                    each=move || rows.clone()
                    key={let value_key = value_key.clone(); let fallback = fallback.clone(); move |option| option[&value_key].as_str().unwrap_or(&fallback).to_owned()}
                    children=move |option| {
                        let value = option[&value_key].as_str().unwrap_or(&fallback).to_owned();
                        let option_label = option[&label_key].as_str().unwrap_or(&value).to_owned();
                        let active = option["active"].as_bool().unwrap_or(false);
                        view! { <option value=value selected=active>{option_label}</option> }
                    }
                />
            </select>
        </label>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NotePivotInterior(
    pivot: RwSignal<Option<Value>>,
    selected_note_id: RwSignal<Option<String>>,
    detail: RwSignal<NoteDetailState>,
) -> impl IntoView {
    view! {
        {move || match pivot.get() {
            Some(pivot) => view! {
                <NoteNavigation pivot=pivot.clone() selected_note_id=selected_note_id />
                <NoteDetailPanel detail=detail />
            }.into_any(),
            None => view! {
                <section class="note-navigation" data-total="0" data-limit="0" data-offset="0">
                    <h3>Notes</h3>
                    <p>Loading notes...</p>
                </section>
                <section id="note-detail-panel" class="note-detail-panel" aria-live="polite">
                    <p>Loading selected note...</p>
                </section>
            }.into_any(),
        }}
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NoteNavigation(pivot: Value, selected_note_id: RwSignal<Option<String>>) -> impl IntoView {
    let total = pivot["total"].as_u64().unwrap_or(0).to_string();
    let limit = pivot["limit"].as_u64().unwrap_or(0).to_string();
    let offset = pivot["offset"].as_u64().unwrap_or(0).to_string();
    let rows = pivot["rows"].as_array().cloned().unwrap_or_default();
    let has_more = pivot["has_more"].as_bool().unwrap_or(false);
    let (language, target, overlay) = pivot_selection_parts(&pivot);

    view! {
        <section class="note-navigation" data-total=total data-limit=limit data-offset=offset>
            <h3>Notes</h3>
            {if rows.is_empty() {
                view! { <p>No notes match the active filter.</p> }.into_any()
            } else {
                view! {
                    <ol class="note-navigation-list">
                        <For
                            each=move || rows.clone()
                            key=|row| row["note_id"].as_str().unwrap_or("").to_owned()
                            children=move |row| {
                                let note_id = row["note_id"].as_str().unwrap_or("").to_owned();
                                let title = row["title"].as_str().unwrap_or(&note_id).to_owned();
                                let status = row["status"].as_str().unwrap_or("unknown").to_owned();
                                let field_count = row["translatable_field_count"].as_u64().unwrap_or(0);
                                let missing_count = row["missing_count"].as_u64().unwrap_or(0);
                                let stale_count = row["stale_count"].as_u64().unwrap_or(0);
                                let language = language.clone();
                                let target = target.clone();
                                let overlay = overlay.clone();
                                let row_note_id = note_id.clone();
                                view! {
                                    <li>
                                        <button
                                            type="button"
                                            class=move || {
                                                if selected_note_id.get().as_deref() == Some(row_note_id.as_str()) {
                                                    "note-navigation-row active"
                                                } else {
                                                    "note-navigation-row"
                                                }
                                            }
                                            data-note-id=note_id.clone()
                                            on:click=move |_| {
                                                set_selected_note_state(Some(note_id.clone()));
                                                set_note_detail_state(NoteDetailState::Loading);
                                                load_note_detail_for_parts(
                                                    language.clone(),
                                                    target.clone(),
                                                    overlay.clone(),
                                                    note_id.clone(),
                                                );
                                            }
                                        >
                                            <strong>{title}</strong><br/>
                                            <span>{status}</span>
                                            {format!(" · {field_count} field(s), {missing_count} missing, {stale_count} stale")}
                                        </button>
                                    </li>
                                }
                            }
                        />
                    </ol>
                    {if has_more {
                        view! { <p class="note-navigation-more">More notes are available; pagination controls will load additional pages in the next slice.</p> }.into_any()
                    } else {
                        "".into_any()
                    }}
                }.into_any()
            }}
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NoteDetailPanel(detail: RwSignal<NoteDetailState>) -> impl IntoView {
    view! {
        <section id="note-detail-panel" class="note-detail-panel" aria-live="polite">
            {move || match detail.get() {
                NoteDetailState::Empty => view! { <p>No notes match the active filter.</p> }.into_any(),
                NoteDetailState::Loading => view! { <p>Loading selected note...</p> }.into_any(),
                NoteDetailState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
                NoteDetailState::Loaded(detail) => view! { <NoteDetail detail=detail /> }.into_any(),
            }}
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NoteDetail(detail: Value) -> impl IntoView {
    let note = detail["notes"]
        .as_array()
        .and_then(|notes| notes.first())
        .cloned();
    match note {
        Some(note) => view! { <LoadedNoteDetail detail=detail note=note /> }.into_any(),
        None => view! { <p>No note detail is available.</p> }.into_any(),
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedNoteDetail(detail: Value, note: Value) -> impl IntoView {
    let note_id = note["note_id"].as_str().unwrap_or("").to_owned();
    let status = note["status"].as_str().unwrap_or("unknown").to_owned();
    let fields = note["fields"].as_array().cloned().unwrap_or_default();
    let source_preview = preview_html(&note["source_preview"]);
    let target_preview = preview_html(&note["target_preview"]);
    let detail_for_effect = detail.clone();
    let detail_for_input = detail.clone();
    let note_id_for_input = note_id.clone();

    Effect::new(move |_| {
        decorate_note_detail_dom(&detail_for_effect);
    });

    view! {
        <section class="note-card" data-note-id=note_id.clone()>
            <h3>{format!("{}: {}", note_id, status)}</h3>
            <div class="preview-grid">
                <section>
                    <h4>Source preview</h4>
                    <div inner_html=source_preview></div>
                </section>
                <section>
                    <h4>Target preview</h4>
                    <div
                        class="target-preview"
                        inner_html=target_preview
                        on:input=move |event| handle_note_inline_target_input(&detail_for_input, &note_id_for_input, event)
                    ></div>
                </section>
            </div>
            <p class="inline-edit-hint">Click highlighted text in the target preview to edit it in place. Staged preview edits are outlined and still use Apply to write YAML.</p>
            <details class="field-editor-details">
                <summary>Advanced field controls</summary>
                <table class="field-editor">
                    <thead>
                        <tr>
                            <th>Field</th>
                            <th>Source / staged source edit</th>
                            <th>Target / staged edit</th>
                            <th>Status</th>
                            <th>Occurrences</th>
                            <th>Mode</th>
                        </tr>
                    </thead>
                    <tbody>
                        <For
                            each=move || fields.clone()
                            key=|field| field["path"].as_str().unwrap_or("").to_owned()
                            children=move |field| view! { <NoteFieldRow detail=detail.clone() note=note.clone() field=field /> }
                        />
                    </tbody>
                </table>
            </details>
        </section>
    }.into_any()
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NoteFieldRow(detail: Value, note: Value, field: Value) -> impl IntoView {
    let prefix = storage_prefix(&detail);
    let note_id = note["note_id"].as_str().unwrap_or("").to_owned();
    let path = field["path"].as_str().unwrap_or("").to_owned();
    let source = field["source"].as_str().unwrap_or("").to_owned();
    let field_id = field["field_id"].as_str().unwrap_or("").to_owned();
    let id = id_for_path(&path);
    let field_name = field["field_name"].as_str().unwrap_or("field").to_owned();
    let editable = field["editable"].as_bool().unwrap_or(false);
    let source_editable = field["source_editable"].as_bool().unwrap_or(false);
    let staged = staging::staged_translation_for_parts(&prefix, &path, &source);
    let staged_source = staging::staged_source_for_parts(&prefix, &path, &source);
    let target_initial = staged
        .as_ref()
        .and_then(|edit| edit["value"].as_str())
        .or_else(|| field["target"].as_str())
        .unwrap_or("")
        .to_owned();
    let source_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["value"].as_str())
        .unwrap_or(&source)
        .to_owned();
    let mode_initial = staged
        .as_ref()
        .and_then(|edit| edit["mode"].as_str())
        .unwrap_or("direct")
        .to_owned();
    let source_scope_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["scope"].as_str())
        .unwrap_or("field")
        .to_owned();
    let source_impact_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["impact_action"].as_str())
        .unwrap_or("stale_translation")
        .to_owned();
    let status_initial = staged
        .as_ref()
        .and_then(|edit| edit["mode"].as_str())
        .map(|mode| format!("staged_{mode}"))
        .or_else(|| staged_source.as_ref().map(|_| "staged_source".to_owned()))
        .unwrap_or_else(|| field["status"].as_str().unwrap_or("unknown").to_owned());
    let target_value = RwSignal::new(target_initial.clone());
    let source_value = RwSignal::new(source_initial.clone());
    let status_value = RwSignal::new(status_initial);
    let mode_value = RwSignal::new(mode_initial.clone());
    let source_scope = RwSignal::new(source_scope_initial.clone());
    let source_impact = RwSignal::new(source_impact_initial.clone());
    let source_readonly = RwSignal::new(staged_source.is_none());
    let occurrence_count = field["occurrence_count"].as_u64().unwrap_or(1);

    let target_input_id = format!("translation-input-{id}");
    let target_text_id = format!("target-text-{id}");
    let status_id = format!("status-text-{id}");
    let mode_id = format!("translation-mode-{id}");
    let source_input_id = format!("source-input-{id}");
    let source_text_id = format!("source-text-{id}");
    let source_toggle_id = format!("source-edit-toggle-{id}");
    let source_scope_id = format!("source-scope-{id}");
    let source_impact_id = format!("source-impact-{id}");

    let stage_target = Rc::new({
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let note_id = note_id.clone();
        let field_id = field_id.clone();
        let source_input_id = source_input_id.clone();
        let target_text_id = target_text_id.clone();
        let status_id = status_id.clone();
        move |value: String, mode: String| {
            let effective_source =
                effective_source_for_dom(&prefix, &path, &source, &source_input_id);
            staging::stage_translation(&prefix, &path, &source, &effective_source, &value, &mode);
            target_value.set(value.clone());
            status_value.set(format!("staged_{mode}"));
            set_element_text(&target_text_id, &value);
            set_element_text(&status_id, &format!("staged_{mode}"));
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_target_preview_field(&document, &note_id, &field_id, &value);
                mark_preview_field_staged(&document, &note_id, &field_id, true);
            }
            update_staged_count(&prefix);
            refresh_progress_from_dom();
        }
    });

    let stage_source = Rc::new({
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let note_id = note_id.clone();
        let field_id = field_id.clone();
        let source_text_id = source_text_id.clone();
        let status_id = status_id.clone();
        move |value: String, scope: String, impact: String| {
            staging::stage_source_edit(&prefix, &path, &source, &value, &scope, &impact);
            source_value.set(value.clone());
            status_value.set("staged_source".to_owned());
            source_readonly.set(false);
            set_element_text(&source_text_id, &value);
            set_element_text(&status_id, "staged_source");
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_source_preview_field(&document, &note_id, &field_id, &value);
            }
            update_staged_count(&prefix);
            refresh_progress_from_dom();
        }
    });

    let stage_source_for_input = stage_source.clone();
    let stage_source_for_scope = stage_source.clone();
    let stage_source_for_impact = stage_source.clone();
    let stage_target_for_input = stage_target.clone();
    let stage_target_for_mode = stage_target.clone();
    let source_input_id_for_toggle = source_input_id.clone();

    view! {
        <tr
            data-field-path=path.clone()
            data-note-id=note_id.clone()
            data-field-id=field_id.clone()
            data-editable=editable.to_string()
            data-original-status=field["status"].as_str().unwrap_or("unknown").to_owned()
        >
            <td>{field_name}</td>
            <td class="source-text">
                <span id=source_text_id.clone()>{move || source_value.get()}</span>
                <div class="source-edit-controls">
                    <button
                        id=source_toggle_id
                        type="button"
                        disabled=!source_editable
                        on:click=move |_| {
                            source_readonly.set(false);
                            focus_input(&source_input_id_for_toggle);
                        }
                    >Edit source</button>
                    <input
                        id=source_input_id.clone()
                        value=source_initial
                        data-path=path.clone()
                        data-source=source.clone()
                        data-note-id=note_id.clone()
                        data-field-id=field_id.clone()
                        readonly=move || source_readonly.get()
                        disabled=!source_editable
                        on:input=move |event| {
                            let value = event_input_value(&event);
                            stage_source_for_input(value, source_scope.get_untracked(), source_impact.get_untracked());
                        }
                    />
                </div>
            </td>
            <td>
                <input
                    id=target_input_id.clone()
                    value=target_initial
                    data-path=path.clone()
                    data-source=source.clone()
                    data-note-id=note_id.clone()
                    data-field-id=field_id.clone()
                    readonly=!editable
                    on:input=move |event| {
                        let value = event_input_value(&event);
                        stage_target_for_input(value, mode_value.get_untracked());
                    }
                />
                <div id=target_text_id>{move || target_value.get()}</div>
            </td>
            <td id=status_id>{move || status_value.get()}</td>
            <td>
                {format!("{occurrence_count} occurrence(s)")}<br/>
                <select
                    id=source_scope_id
                    disabled=!source_editable
                    on:change=move |event| {
                        let value = event_select_value(&event);
                        source_scope.set(value.clone());
                        stage_source_for_scope(source_value.get_untracked(), value, source_impact.get_untracked());
                    }
                >
                    <option value="field" selected=source_scope_initial == "field">This field only</option>
                    <option value="all_occurrences" selected=source_scope_initial == "all_occurrences">All occurrences</option>
                </select>
            </td>
            <td>
                <select
                    id=mode_id
                    disabled=!editable
                    on:change=move |event| {
                        let value = event_select_value(&event);
                        mode_value.set(value.clone());
                        stage_target_for_mode(target_value.get_untracked(), value);
                    }
                >
                    <option value="direct" selected=mode_initial == "direct">Direct</option>
                    <option value="contextual" selected=mode_initial == "contextual">Contextual</option>
                    <option value="no_change" selected=mode_initial == "no_change">No change</option>
                </select>
                <br/>
                <label>
                    Source impact
                    <select
                        id=source_impact_id
                        disabled=!source_editable
                        on:change=move |event| {
                            let value = event_select_value(&event);
                            source_impact.set(value.clone());
                            stage_source_for_impact(source_value.get_untracked(), source_scope.get_untracked(), value);
                        }
                    >
                        <option value="stale_translation" selected=source_impact_initial == "stale_translation">Create stale translation</option>
                        <option value="migrate_key" selected=source_impact_initial == "migrate_key">Migrate key</option>
                    </select>
                </label>
            </td>
        </tr>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn CardPivotPanel(
    state: RwSignal<LazyValueState>,
    selected_card_id: RwSignal<Option<String>>,
    detail: RwSignal<DetailValueState>,
) -> impl IntoView {
    view! {
        <section
            id="card-pivot-panel"
            class="card-pivot lazy-pivot-panel"
            data-lazy-state=move || lazy_state_key(&state.get())
            aria-live="polite"
            aria-busy=move || lazy_state_busy(&state.get())
        >
            {move || match state.get() {
                LazyValueState::Unloaded => view! {
                    <h3>Card pivot</h3>
                    <p>Cards are loaded on demand so language changes stay responsive.</p>
                    <button id="load-card-pivot-button" class="lazy-pivot-load-button" type="button" on:click=move |_| maybe_load_current_view(WorkbenchView::Cards)>Load cards</button>
                }.into_any(),
                LazyValueState::Loading(message) => view! { <p class="workbench-loading">{message}</p> }.into_any(),
                LazyValueState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
                LazyValueState::Loaded(list) => view! { <LoadedCardList list=list selected_card_id=selected_card_id detail=detail /> }.into_any(),
            }}
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedCardList(
    list: Value,
    selected_card_id: RwSignal<Option<String>>,
    detail: RwSignal<DetailValueState>,
) -> impl IntoView {
    let rows = list["rows"].as_array().cloned().unwrap_or_default();
    let active_group = list["filters"]["content_group"]
        .as_str()
        .unwrap_or("all")
        .to_owned();
    let active_filter = list["filters"]["active"]
        .as_str()
        .unwrap_or("all")
        .to_owned();
    let groups = list["filters"]["content_groups"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let total = list["total"].as_u64().unwrap_or(0);
    let (language, target, overlay) = pivot_selection_parts(&list);

    view! {
        <div class="card-pivot-view">
            <h3>Card pivot</h3>
            <label>Content group " "
                <select
                    id="card-content-group-filter"
                    on:change={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); move |event| {
                        load_card_list_for_parts(language.clone(), target.clone(), overlay.clone(), current_card_filter(), non_empty(event_select_value(&event)));
                    }}
                >
                    <option value="all" selected=active_group == "all">All groups</option>
                    <For
                        each=move || groups.clone()
                        key=|group| group.as_str().unwrap_or("").to_owned()
                        children={let active_group = active_group.clone(); move |group| {
                            let group = group.as_str().unwrap_or("").to_owned();
                            view! { <option value=group.clone() selected=active_group == group>{group.clone()}</option> }
                        }}
                    />
                </select>
            </label>
            <nav class="card-pivot-filters">
                <For
                    each=move || vec![
                        ("all".to_owned(), "All cards".to_owned()),
                        ("missing".to_owned(), "Missing".to_owned()),
                        ("stale".to_owned(), "Stale".to_owned()),
                        ("needs_work".to_owned(), "Needs work".to_owned()),
                    ]
                    key=|(filter, _)| filter.clone()
                    children={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); move |(filter, label)| {
                        let class = if active_filter == filter { "active" } else { "" };
                        view! {
                            <button
                                type="button"
                                data-filter=filter.clone()
                                class=class
                                on:click={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); let filter = filter.clone(); move |_| {
                                    load_card_list_for_parts(language.clone(), target.clone(), overlay.clone(), Some(filter.clone()), current_selected_value("card-content-group-filter"));
                                }}
                            >{label}</button>
                        }
                    }}
                />
            </nav>
            <p class="navigation-page-summary">{format!("Showing {} of {} card(s).", rows.len(), total)}</p>
            <div class="card-grid">
                <ol class="card-list">
                    <For
                        each=move || rows.clone()
                        key=|card| card["card_id"].as_str().unwrap_or("").to_owned()
                        children={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); move |card| {
                            let card_id = card["card_id"].as_str().unwrap_or("").to_owned();
                            let title = card["title"].as_str().unwrap_or("card").to_owned();
                            let template = card["template_name"].as_str().unwrap_or("template").to_owned();
                            let status = card["status"].as_str().unwrap_or("unknown").to_owned();
                            view! {
                                <li>
                                    <button
                                        class={let active_card_id = card_id.clone(); move || if selected_card_id.get().as_deref() == Some(active_card_id.as_str()) { "card-row active" } else { "card-row" }}
                                        type="button"
                                        data-card-id=card_id.clone()
                                        on:click={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); let card_id = card_id.clone(); move |_| {
                                            selected_card_id.set(Some(card_id.clone()));
                                            detail.set(DetailValueState::Loading("Loading selected card…".to_owned()));
                                            load_card_detail_for_parts(language.clone(), target.clone(), overlay.clone(), card_id.clone(), current_card_filter(), current_selected_value("card-content-group-filter"));
                                        }}
                                    >{format!("{title} · {template} · {status}")}</button>
                                </li>
                            }
                        }}
                    />
                </ol>
                <section id="card-detail-panel" class="card-detail-panel">
                    <CardDetailPanel detail=detail />
                </section>
            </div>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn CardDetailPanel(detail: RwSignal<DetailValueState>) -> impl IntoView {
    view! {
        {move || match detail.get() {
            DetailValueState::Empty => view! { <p>Select a card to load its preview and editable fields.</p> }.into_any(),
            DetailValueState::Loading(message) => view! { <p class="workbench-loading">{message}</p> }.into_any(),
            DetailValueState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
            DetailValueState::Loaded(pivot) => {
                if let Some(card) = pivot.get("selected_card").filter(|value| !value.is_null()).cloned() {
                    view! { <LoadedCardDetail pivot=pivot card=card /> }.into_any()
                } else {
                    view! { <p>No cards match the active filters.</p> }.into_any()
                }
            }
        }}
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedCardDetail(pivot: Value, card: Value) -> impl IntoView {
    let card_id = card["card_id"].as_str().unwrap_or("").to_owned();
    let title = card["title"].as_str().unwrap_or("card").to_owned();
    let template = card["template_name"]
        .as_str()
        .unwrap_or("template")
        .to_owned();
    let fields = card["fields"].as_array().cloned().unwrap_or_default();
    let source_preview = preview_html(&card["source_preview"]);
    let target_preview = preview_html(&card["target_preview"]);
    let pivot_for_effect = pivot.clone();
    let pivot_for_inline = pivot.clone();

    Effect::new(move |_| decorate_card_detail_dom(&pivot_for_effect));

    view! {
        <section class="card-detail" data-card-id=card_id>
            <h4>{format!("{title}: {template}")}</h4>
            <div class="preview-grid">
                <section><h5>Source card</h5><div class="card-source-preview" inner_html=source_preview></div></section>
                <section><h5>Target card</h5><div class="card-target-preview" inner_html=target_preview on:input=move |event| handle_card_inline_target_input(&pivot_for_inline, event)></div></section>
            </div>
            <p class="inline-edit-hint">Click highlighted text in the selected target card to stage a card-specific edit.</p>
            <details class="field-editor-details">
                <summary>Advanced card field controls</summary>
                <table class="card-field-editor"><thead><tr><th>Field</th><th>Source edit</th><th>Target edit</th><th>Status</th><th>Mode</th></tr></thead>
                    <tbody>
                        <For
                            each=move || fields.clone()
                            key=|field| field["path"].as_str().unwrap_or("").to_owned()
                            children=move |field| view! { <CardFieldRow pivot=pivot.clone() field=field /> }
                        />
                    </tbody>
                </table>
            </details>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn CardFieldRow(pivot: Value, field: Value) -> impl IntoView {
    let prefix = storage_prefix(&pivot);
    let path = field["path"].as_str().unwrap_or("").to_owned();
    let source = field["source"].as_str().unwrap_or("").to_owned();
    let field_id = field["field_id"].as_str().unwrap_or("").to_owned();
    let id = id_for_path(&path);
    let field_name = field["field_name"].as_str().unwrap_or("field").to_owned();
    let editable = field["editable"].as_bool().unwrap_or(false);
    let source_editable = field["source_editable"].as_bool().unwrap_or(false);
    let staged = staging::staged_translation_for_parts(&prefix, &path, &source);
    let staged_source = staging::staged_source_for_parts(&prefix, &path, &source);
    let source_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["value"].as_str())
        .unwrap_or(&source)
        .to_owned();
    let target_initial = staged
        .as_ref()
        .and_then(|edit| edit["value"].as_str())
        .or_else(|| field["target"].as_str())
        .unwrap_or(&source)
        .to_owned();
    let mode_initial = staged
        .as_ref()
        .and_then(|edit| edit["mode"].as_str())
        .unwrap_or("direct")
        .to_owned();
    let source_scope_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["scope"].as_str())
        .unwrap_or("field")
        .to_owned();
    let source_impact_initial = staged_source
        .as_ref()
        .and_then(|edit| edit["impact_action"].as_str())
        .unwrap_or("stale_translation")
        .to_owned();
    let source_value = RwSignal::new(source_initial.clone());
    let target_value = RwSignal::new(target_initial.clone());
    let mode_value = RwSignal::new(mode_initial.clone());
    let source_scope = RwSignal::new(source_scope_initial.clone());
    let source_impact = RwSignal::new(source_impact_initial.clone());
    let source_readonly = RwSignal::new(staged_source.is_none());
    let status_initial = staged
        .as_ref()
        .and_then(|edit| edit["mode"].as_str())
        .map(|mode| format!("staged_{mode}"))
        .or_else(|| staged_source.as_ref().map(|_| "staged_source".to_owned()))
        .unwrap_or_else(|| field["status"].as_str().unwrap_or("unknown").to_owned());
    let status_value = RwSignal::new(status_initial);
    let source_text_id = format!("card-source-text-{id}");
    let source_toggle_id = format!("card-source-edit-toggle-{id}");
    let source_input_id = format!("card-source-input-{id}");
    let source_scope_id = format!("card-source-scope-{id}");
    let source_impact_id = format!("card-source-impact-{id}");
    let target_input_id = format!("card-translation-input-{id}");
    let target_text_id = format!("card-target-text-{id}");
    let status_id = format!("card-status-text-{id}");
    let mode_id = format!("card-translation-mode-{id}");

    let stage_target = Rc::new({
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let field_id = field_id.clone();
        let source_input_id = source_input_id.clone();
        let target_text_id = target_text_id.clone();
        let status_id = status_id.clone();
        move |value: String, mode: String| {
            let effective_source =
                effective_source_for_dom(&prefix, &path, &source, &source_input_id);
            stage_translation_with_optional_context(
                &prefix,
                &path,
                &source,
                &effective_source,
                &value,
                &mode,
                mode == "contextual",
            );
            target_value.set(value.clone());
            status_value.set(format!("staged_{mode}"));
            set_element_text(&target_text_id, &value);
            set_element_text(&status_id, &format!("staged_{mode}"));
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_card_preview_field(&document, "card-target-preview", &field_id, &value);
                mark_card_preview_field_staged(&document, &field_id, true);
            }
            update_staged_count(&prefix);
        }
    });
    let stage_source = Rc::new({
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        let field_id = field_id.clone();
        let source_text_id = source_text_id.clone();
        let status_id = status_id.clone();
        move |value: String, scope: String, impact: String| {
            staging::stage_source_edit(&prefix, &path, &source, &value, &scope, &impact);
            source_value.set(value.clone());
            source_readonly.set(false);
            status_value.set("staged_source".to_owned());
            set_element_text(&source_text_id, &value);
            set_element_text(&status_id, "staged_source");
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_card_preview_field(&document, "card-source-preview", &field_id, &value);
            }
            update_staged_count(&prefix);
        }
    });

    view! {
        <tr class="card-field-row" data-path=path.clone() data-source=source.clone()>
            <td>{field_name}</td>
            <td>
                <span id=source_text_id.clone()>{move || source_value.get()}</span><br/>
                <button id=source_toggle_id type="button" disabled=!source_editable on:click={let source_input_id = source_input_id.clone(); move |_| { source_readonly.set(false); focus_input(&source_input_id); }}>Edit source</button> " "
                <input id=source_input_id.clone() value=source_initial readonly=move || source_readonly.get() disabled=!source_editable on:input={let stage_source = stage_source.clone(); move |event| stage_source(event_input_value(&event), source_scope.get_untracked(), source_impact.get_untracked())} />
                <br/>
                <select id=source_scope_id disabled=!source_editable on:change={let stage_source = stage_source.clone(); move |event| { let value = event_select_value(&event); source_scope.set(value.clone()); stage_source(source_value.get_untracked(), value, source_impact.get_untracked()); }}>
                    <option value="field" selected=source_scope_initial == "field">This field only</option>
                    <option value="all_occurrences" selected=source_scope_initial == "all_occurrences">All occurrences</option>
                </select> " "
                <select id=source_impact_id disabled=!source_editable on:change={let stage_source = stage_source.clone(); move |event| { let value = event_select_value(&event); source_impact.set(value.clone()); stage_source(source_value.get_untracked(), source_scope.get_untracked(), value); }}>
                    <option value="stale_translation" selected=source_impact_initial == "stale_translation">Create stale translation</option>
                    <option value="migrate_key" selected=source_impact_initial == "migrate_key">Migrate key</option>
                </select>
            </td>
            <td>
                <input id=target_input_id.clone() value=target_initial readonly=!editable on:input={let stage_target = stage_target.clone(); move |event| stage_target(event_input_value(&event), mode_value.get_untracked())} />
                <div id=target_text_id>{move || target_value.get()}</div>
            </td>
            <td id=status_id>{move || status_value.get()}</td>
            <td>
                <select id=mode_id disabled=!editable on:change={let stage_target = stage_target.clone(); move |event| { let value = event_select_value(&event); mode_value.set(value.clone()); stage_target(target_value.get_untracked(), value); }}>
                    <option value="direct" selected=mode_initial == "direct">Direct</option>
                    <option value="contextual" selected=mode_initial == "contextual">Contextual</option>
                    <option value="no_change" selected=mode_initial == "no_change">No change</option>
                </select>
            </td>
        </tr>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn SourceStringPivotPanel(
    state: RwSignal<LazyValueState>,
    selected_source: RwSignal<Option<String>>,
    detail: RwSignal<DetailValueState>,
) -> impl IntoView {
    view! {
        <section
            id="source-string-pivot-panel"
            class="source-string-pivot lazy-pivot-panel"
            data-lazy-state=move || lazy_state_key(&state.get())
            aria-live="polite"
            aria-busy=move || lazy_state_busy(&state.get())
        >
            {move || match state.get() {
                LazyValueState::Unloaded => view! {
                    <h3>Source String pivot</h3>
                    <p>Reusable source strings are loaded only when you need that workflow.</p>
                    <button id="load-source-string-pivot-button" class="lazy-pivot-load-button" type="button" on:click=move |_| maybe_load_current_view(WorkbenchView::SourceStrings)>Load source strings</button>
                }.into_any(),
                LazyValueState::Loading(message) => view! { <p class="workbench-loading">{message}</p> }.into_any(),
                LazyValueState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
                LazyValueState::Loaded(list) => view! { <LoadedSourceStringList list=list selected_source=selected_source detail=detail /> }.into_any(),
            }}
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedSourceStringList(
    list: Value,
    selected_source: RwSignal<Option<String>>,
    detail: RwSignal<DetailValueState>,
) -> impl IntoView {
    let rows = list["rows"].as_array().cloned().unwrap_or_default();
    let active_group = list["filters"]["content_group"]
        .as_str()
        .unwrap_or("all")
        .to_owned();
    let groups = list["filters"]["content_groups"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let total = list["total"].as_u64().unwrap_or(0);
    let (language, target, overlay) = pivot_selection_parts(&list);
    view! {
        <>
            <h3>Source String pivot</h3>
            <div class="source-string-filters"><label>Content group " "
                <select id="source-string-content-group-filter" on:change={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); move |event| load_source_string_list_for_parts(language.clone(), target.clone(), overlay.clone(), non_empty(event_select_value(&event)), None)}>
                    <option value="all" selected=active_group == "all">All groups</option>
                    <For each=move || groups.clone() key=|group| group.as_str().unwrap_or("").to_owned() children={let active_group = active_group.clone(); move |group| {
                        let group = group.as_str().unwrap_or("").to_owned();
                        view! { <option value=group.clone() selected=active_group == group>{group.clone()}</option> }
                    }} />
                </select>
            </label></div>
            <p class="navigation-page-summary">{format!("Showing {} of {} source string(s).", rows.len(), total)}</p>
            <div class="source-string-grid">
                <ol class="source-string-list">
                    <For
                        each=move || rows.clone()
                        key=|row| row["source"].as_str().unwrap_or("").to_owned()
                        children={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); move |row| {
                            let source = row["source"].as_str().unwrap_or("").to_owned();
                            let count = row["occurrence_count"].as_u64().unwrap_or(0);
                            let status = row["status"].as_str().unwrap_or("unknown").to_owned();
                            view! {
                                <li><button
                                    type="button"
                                    class={let active_source = source.clone(); move || if selected_source.get().as_deref() == Some(active_source.as_str()) { "source-string-row active" } else { "source-string-row" }}
                                    data-source=source.clone()
                                    on:click={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); let source = source.clone(); move |_| {
                                        selected_source.set(Some(source.clone()));
                                        detail.set(DetailValueState::Loading("Loading selected source string…".to_owned()));
                                        load_source_string_detail_for_parts(language.clone(), target.clone(), overlay.clone(), source.clone(), current_selected_value("source-string-content-group-filter"), None);
                                    }}
                                ><strong>{source.clone()}</strong><br/>{format!("{count} occurrence(s), {status}")}</button></li>
                            }
                        }}
                    />
                </ol>
                <section id="source-string-detail-panel" class="source-string-detail">
                    <SourceStringDetailPanel detail=detail />
                </section>
            </div>
        </>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn SourceStringDetailPanel(detail: RwSignal<DetailValueState>) -> impl IntoView {
    view! {
        {move || match detail.get() {
            DetailValueState::Empty => view! { <p>Select a source string to load occurrences.</p> }.into_any(),
            DetailValueState::Loading(message) => view! { <p class="workbench-loading">{message}</p> }.into_any(),
            DetailValueState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
            DetailValueState::Loaded(pivot) => view! { <LoadedSourceStringDetail pivot=pivot /> }.into_any(),
        }}
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedSourceStringDetail(pivot: Value) -> impl IntoView {
    let selected_source = pivot["selected_source"].as_str().unwrap_or("").to_owned();
    let occurrences = pivot["occurrences"].as_array().cloned().unwrap_or_default();
    let first = occurrences.first().cloned();
    let direct_value = first
        .as_ref()
        .map(|occurrence| {
            staging::staged_translation_for_parts(
                &storage_prefix(&pivot),
                occurrence["path"].as_str().unwrap_or(""),
                &selected_source,
            )
            .and_then(|edit| edit["value"].as_str().map(str::to_owned))
            .or_else(|| occurrence["target"].as_str().map(str::to_owned))
            .unwrap_or_else(|| selected_source.clone())
        })
        .unwrap_or_else(|| selected_source.clone());
    let direct_signal = RwSignal::new(direct_value.clone());
    let first_path = first
        .as_ref()
        .and_then(|occurrence| occurrence["path"].as_str())
        .unwrap_or("")
        .to_owned();
    let prefix = storage_prefix(&pivot);

    let stage_direct = Rc::new({
        let prefix = prefix.clone();
        let selected_source = selected_source.clone();
        let first_path = first_path.clone();
        move |value: String, mode: String| {
            stage_translation_with_optional_context(
                &prefix,
                &first_path,
                &selected_source,
                &selected_source,
                &value,
                &mode,
                false,
            );
            direct_signal.set(value.clone());
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_source_string_targets(&document, &selected_source, &value);
            }
            update_staged_count(&prefix);
        }
    });

    view! {
        <>
            <h4>{selected_source.clone()}</h4>
            {if first.is_some() { view! {
                <div class="source-string-global-edit">
                    <label>Reusable translation " "<input id="source-string-direct-input" value=direct_value on:input={let stage_direct = stage_direct.clone(); move |event| stage_direct(event_input_value(&event), "direct".to_owned())} /></label> " "
                    <button id="source-string-direct-stage" type="button" on:click={let stage_direct = stage_direct.clone(); let direct_source = selected_source.clone(); move |_| stage_direct(current_input_value("source-string-direct-input").unwrap_or_else(|| direct_source.clone()), "direct".to_owned())}> {format!("Stage direct translation for {} occurrence(s)", occurrences.len())}</button> " "
                    <button id="source-string-no-change" type="button" on:click={let stage_direct = stage_direct.clone(); let no_change_source = selected_source.clone(); move |_| stage_direct(no_change_source.clone(), "no_change".to_owned())}>{"Stage global no-change"}</button>
                </div>
            }.into_any() } else { "".into_any() }}
            <table class="source-string-occurrences"><thead><tr><th>Context</th><th>Source</th><th>Target / contextual override</th><th>Status</th></tr></thead>
                <tbody>
                    <For
                        each=move || occurrences.clone()
                        key=|occurrence| occurrence["path"].as_str().unwrap_or("").to_owned()
                        children={let pivot = pivot.clone(); let selected_source = selected_source.clone(); let prefix = prefix.clone(); move |occurrence| view! { <SourceStringOccurrenceRow pivot=pivot.clone() occurrence=occurrence selected_source=selected_source.clone() prefix=prefix.clone() /> }}
                    />
                </tbody>
            </table>
        </>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn SourceStringOccurrenceRow(
    pivot: Value,
    occurrence: Value,
    selected_source: String,
    prefix: String,
) -> impl IntoView {
    let path = occurrence["path"].as_str().unwrap_or("").to_owned();
    let id = id_for_path(&path);
    let source = occurrence["source"]
        .as_str()
        .unwrap_or(&selected_source)
        .to_owned();
    let initial =
        staging::staged_translation_for_parts(&storage_prefix(&pivot), &path, &selected_source)
            .and_then(|edit| edit["value"].as_str().map(str::to_owned))
            .or_else(|| occurrence["target"].as_str().map(str::to_owned))
            .unwrap_or_else(|| selected_source.clone());
    let target_value = RwSignal::new(initial.clone());
    let input_id = format!("source-string-contextual-input-{id}");
    let stage_id = format!("source-string-contextual-stage-{id}");
    let stage_contextual = Rc::new({
        let prefix = prefix.clone();
        let path = path.clone();
        let source = source.clone();
        move |value: String| {
            stage_translation_with_optional_context(
                &prefix,
                &path,
                &source,
                &source,
                &value,
                "contextual",
                true,
            );
            target_value.set(value.clone());
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                update_source_string_contextual_target(&document, &path, &value);
            }
            update_staged_count(&prefix);
        }
    });
    view! {
        <tr class="source-string-occurrence" data-source=selected_source.clone() data-path=path.clone()>
            <td>{occurrence["friendly_context"].as_str().unwrap_or(&path).to_owned()}<br/><small>{path.clone()}</small></td>
            <td>{selected_source.clone()}</td>
            <td>
                <div
                    id=input_id.clone()
                    class="source-string-contextual-input"
                    contenteditable="true"
                    role="textbox"
                    tabindex="0"
                    data-path=path.clone()
                    data-source=selected_source.clone()
                    on:input={let stage_contextual = stage_contextual.clone(); move |event| stage_contextual(event_text_content(&event))}
                >{initial}</div> " "
                <button id=stage_id type="button" on:click={let stage_contextual = stage_contextual.clone(); let input_id = input_id.clone(); move |_| stage_contextual(current_text_content(&input_id).unwrap_or_default())}>Contextual override</button>
                <div class="source-string-target-text">{move || target_value.get()}</div>
            </td>
            <td>{occurrence["status"].as_str().unwrap_or("unknown").to_owned()}</td>
        </tr>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn OptionalMetadataPanel(state: RwSignal<LazyValueState>) -> impl IntoView {
    view! {
        <section
            id="optional-metadata-panel"
            class="optional-metadata lazy-pivot-panel"
            data-lazy-state=move || lazy_state_key(&state.get())
            aria-live="polite"
            aria-busy=move || lazy_state_busy(&state.get())
        >
            {move || match state.get() {
                LazyValueState::Unloaded => view! {
                    <h3>Metadata checklist</h3>
                    <p>Metadata rows are loaded only when you open the checklist.</p>
                    <button id="load-optional-metadata-button" class="lazy-pivot-load-button" type="button" on:click=move |_| maybe_load_current_view(WorkbenchView::Metadata)>Load metadata</button>
                }.into_any(),
                LazyValueState::Loading(message) => view! { <p class="workbench-loading">{message}</p> }.into_any(),
                LazyValueState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
                LazyValueState::Loaded(optional) => view! { <LoadedOptionalMetadata optional=optional /> }.into_any(),
            }}
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn LoadedOptionalMetadata(optional: Value) -> impl IntoView {
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
    let prefix = storage_prefix_for_parts(&language, &target, &overlay);
    let progress = optional["metadata_progress"].clone();
    let items = optional["items"].as_array().cloned().unwrap_or_default();
    let prefix_for_effect = prefix.clone();
    Effect::new(move |_| {
        update_staged_count(&prefix_for_effect);
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            apply_pane_writability(
                checkbox_checked(&document, "source-pane-writable"),
                checkbox_checked(&document, "target-pane-writable"),
            );
        }
    });
    view! {
        <section id="optional-metadata-checklist" data-storage-prefix=prefix.clone()>
            <h3>Metadata checklist</h3>
            <p>{format!("Main note-field completion is separate. Metadata: {} / {} complete, {} missing, {} stale.", progress["complete"].as_u64().unwrap_or(0), progress["total"].as_u64().unwrap_or(0), progress["missing"].as_u64().unwrap_or(0), progress["stale"].as_u64().unwrap_or(0))}</p>
            <table><thead><tr><th>Category</th><th>Path</th><th>Source</th><th>Target</th><th>Status</th></tr></thead>
                <tbody>
                    <For each=move || items.clone() key=|item| item["path"].as_str().unwrap_or("").to_owned() children={let language = language.clone(); let target = target.clone(); let overlay = overlay.clone(); let prefix = prefix.clone(); move |item| view! { <OptionalMetadataRow item=item language=language.clone() target=target.clone() overlay=overlay.clone() prefix=prefix.clone() /> }} />
                </tbody>
            </table>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn OptionalMetadataRow(
    item: Value,
    language: String,
    target: String,
    overlay: String,
    prefix: String,
) -> impl IntoView {
    let path = item["path"].as_str().unwrap_or("").to_owned();
    let source = item["source"].as_str().unwrap_or("").to_owned();
    let id = id_for_path(&path);
    let staged = staging::staged_translation_for_parts(&prefix, &path, &source);
    let initial = staged
        .as_ref()
        .and_then(|edit| edit["value"].as_str())
        .or_else(|| item["target"].as_str())
        .unwrap_or(&source)
        .to_owned();
    let status_initial = staged
        .as_ref()
        .map(|_| "staged_direct".to_owned())
        .unwrap_or_else(|| item["status"].as_str().unwrap_or("unknown").to_owned());
    let status_value = RwSignal::new(status_initial);
    let warning = item["warning"].as_str().unwrap_or("").to_owned();
    let input_id = format!("optional-translation-input-{id}");
    let status_id = format!("optional-status-{id}");
    let stage_prefix = prefix.clone();
    let stage_language = language.clone();
    let stage_target = target.clone();
    let stage_overlay = overlay.clone();
    let stage_path = path.clone();
    let stage_source = source.clone();
    view! {
        <tr class="optional-metadata-row" data-path=path.clone() data-source=source.clone()>
            <td>{item["metadata_category"].as_str().unwrap_or("metadata").to_owned()}</td>
            <td>{path.clone()}</td>
            <td>{source.clone()}</td>
            <td><input id=input_id value=initial data-path=path.clone() data-source=source.clone() on:input=move |event| {
                stage_metadata_translation(&stage_prefix, &stage_language, &stage_target, &stage_overlay, &stage_path, &stage_source, &event_input_value(&event));
                status_value.set("staged_direct".to_owned());
                update_staged_count(&stage_prefix);
            } /></td>
            <td id=status_id>{move || format!("{} {}", status_value.get(), warning)}</td>
        </tr>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn WorkbenchWorkflowPanels(
    pivot: RwSignal<Option<Value>>,
    capability: RwSignal<WorkbenchWriteCapability>,
) -> impl IntoView {
    view! {
        {move || match pivot.get() {
            Some(pivot) => view! {
                <PaneLayoutPanel pivot=pivot.clone() />
                <NewLanguagePanel pivot=pivot capability=capability />
            }.into_any(),
            None => "".into_any(),
        }}
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ApplyBox(
    pivot: RwSignal<Option<Value>>,
    capability: RwSignal<WorkbenchWriteCapability>,
) -> impl IntoView {
    let output_text = RwSignal::new(String::new());
    let validation_ok = RwSignal::new(None::<bool>);

    let run_apply = move |write: bool| {
        if write && !capability.get_untracked().enabled {
            output_text.set(
                "Read-only: draft retained in this browser; no source files were changed."
                    .to_owned(),
            );
            validation_ok.set(Some(false));
            return;
        }
        let Some(pivot_value) = pivot.get_untracked() else {
            output_text.set("Apply failed: note pivot is not loaded".to_owned());
            validation_ok.set(Some(false));
            return;
        };
        output_text.set(if write {
            "Applying…".to_owned()
        } else {
            "Building apply preview…".to_owned()
        });
        validation_ok.set(None);
        spawn_local(async move {
            let edits = collect_staged_edits(&pivot_value);
            let request = serde_json::json!({
                "language": pivot_value["language"]["code"].as_str().unwrap_or(""),
                "target": pivot_value["target"]["label"].as_str().unwrap_or(""),
                "overlay": pivot_value["overlay"]["label"].as_str().unwrap_or(""),
                "edits": edits,
            });
            let endpoint = if write {
                "/api/workbench/apply"
            } else {
                "/api/workbench/apply-preview"
            };
            match post_json(endpoint, &request).await {
                Ok(value) => {
                    let text =
                        apply_result_text(if write { "Applied" } else { "Apply preview" }, &value);
                    let ok = value["validation"]["ok"].as_bool().unwrap_or(false);
                    output_text.set(text.clone());
                    validation_ok.set(Some(ok));
                    if write {
                        for prefix in active_storage_prefixes(&pivot_value) {
                            clear_staged_edits(&prefix);
                        }
                        if let Ok(updated) = fetch_note_pivot_for_pivot(&pivot_value).await {
                            publish_note_pivot_panel(&updated);
                        }
                        output_text.set(text);
                        validation_ok.set(Some(true));
                    }
                }
                Err(error) => {
                    output_text.set(format!("Apply failed: {error}"));
                    validation_ok.set(Some(false));
                }
            }
        });
    };
    let run_preview = run_apply;

    view! {
        <button id="apply-preview-button" type="button" on:click=move |_| run_preview(false)>Apply preview</button>
        " "
        <button
            id="apply-confirm-button"
            type="button"
            disabled=move || !capability.get().enabled
            title=move || if capability.get().enabled { "Unsafe development Apply" } else { "Read-only Workbench: drafts stay in browser localStorage" }
            on:click=move |_| run_apply(true)
        >
            {move || if capability.get().enabled { "Confirm unsafe development Apply" } else { "Apply unavailable — read-only" }}
        </button>
        <p class="draft-retention-copy">{"Drafts remain in this browser's localStorage until an enabled Apply succeeds or browser data is cleared."}</p>
        <pre
            id="apply-preview-output"
            data-validation-ok=move || validation_ok.get().map(|ok| ok.to_string()).unwrap_or_default()
        >{move || output_text.get()}</pre>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn PaneLayoutPanel(pivot: Value) -> impl IntoView {
    let source_writable = RwSignal::new(false);
    let target_writable = RwSignal::new(true);
    let selected_secondary_language = RwSignal::new(first_secondary_language(&pivot));
    let secondary_state = RwSignal::new(SecondaryPaneState::Empty);
    let target_label = pivot["target"]["label"].as_str().map(str::to_owned);

    Effect::new(move |_| {
        apply_pane_writability(source_writable.get(), target_writable.get());
    });

    let load_secondary = {
        let target_label = target_label.clone();
        move |_| {
            let language = selected_secondary_language.get_untracked();
            if language.is_empty() {
                return;
            }
            let target = target_label.clone();
            let selection_generation = current_selection_generation();
            secondary_state.set(SecondaryPaneState::Loading);
            spawn_local(async move {
                match fetch_comparison_pane_query(Some(language), target, Some("base".to_owned()))
                    .await
                {
                    Ok(pane) if is_current_selection_generation(selection_generation) => {
                        secondary_state.set(SecondaryPaneState::Loaded(pane));
                    }
                    Ok(_) => {}
                    Err(error) if is_current_selection_generation(selection_generation) => {
                        secondary_state.set(SecondaryPaneState::Error(error));
                    }
                    Err(_) => {}
                }
            });
        }
    };

    let languages = pivot["selection_options"]["languages"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    view! {
        <section id="pane-layout-panel" class="pane-layout-panel">
            <h3>Pane layout preset</h3>
            <label>
                <input
                    id="source-pane-writable"
                    type="checkbox"
                    checked=move || source_writable.get()
                    on:change=move |event| source_writable.set(event_checkbox_checked(&event))
                />
                " Source pane writable"
            </label>
            " "
            <label>
                <input
                    id="target-pane-writable"
                    type="checkbox"
                    checked=move || target_writable.get()
                    on:change=move |event| target_writable.set(event_checkbox_checked(&event))
                />
                " Selected target pane writable"
            </label>
            " "
            <label>
                "Additional target "
                <select
                    id="secondary-pane-language-select"
                    on:change=move |event| selected_secondary_language.set(event_select_value(&event))
                >
                    <For
                        each=move || {
                            languages
                                .clone()
                                .into_iter()
                                .filter(|language| !language["active"].as_bool().unwrap_or(false))
                                .collect::<Vec<_>>()
                        }
                        key=|language| language["code"].as_str().unwrap_or("").to_owned()
                        children=move |language| {
                            let code = language["code"].as_str().unwrap_or("").to_owned();
                            let label = language["display_name"].as_str().unwrap_or(&code).to_owned();
                            view! { <option value=code>{label}</option> }
                        }
                    />
                </select>
            </label>
            " "
            <button id="load-secondary-pane" type="button" on:click=load_secondary>Load target pane</button>
            <div id="secondary-target-pane">
                {move || match secondary_state.get() {
                    SecondaryPaneState::Empty => "".into_any(),
                    SecondaryPaneState::Loading => view! { <p class="workbench-loading">Loading target pane...</p> }.into_any(),
                    SecondaryPaneState::Error(error) => view! { <p class="workbench-error">{error}</p> }.into_any(),
                    SecondaryPaneState::Loaded(comparison) => view! { <SecondaryTargetPane comparison=comparison /> }.into_any(),
                }}
            </div>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Default)]
enum SecondaryPaneState {
    #[default]
    Empty,
    Loading,
    Error(String),
    Loaded(Value),
}

#[cfg(target_arch = "wasm32")]
#[component]
fn SecondaryTargetPane(comparison: Value) -> impl IntoView {
    let pane = comparison["note_pivot"].clone();
    let source_strings = comparison["source_string_pivot"]["strings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let cards_value = comparison["card_pivot"].clone();
    let cards = cards_value["cards"].as_array().cloned().unwrap_or_default();
    let selected_card = cards_value
        .get("selected_card")
        .filter(|value| !value.is_null())
        .cloned();
    let language = pane["language"]["code"].as_str().unwrap_or("").to_owned();
    let target = pane["target"]["label"].as_str().unwrap_or("").to_owned();
    let overlay = pane["overlay"]["label"].as_str().unwrap_or("").to_owned();
    let prefix = storage_prefix_for_parts(&language, &target, &overlay);
    let display_name = pane["language"]["display_name"]
        .as_str()
        .unwrap_or(&language)
        .to_owned();
    let fields = pane["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|note| note["fields"].as_array().into_iter().flatten())
        .filter(|field| field["editable"].as_bool().unwrap_or(false))
        .cloned()
        .collect::<Vec<_>>();
    let secondary_writable = RwSignal::new(true);
    let attr_language = language.clone();
    let attr_target = target.clone();
    let attr_overlay = overlay.clone();
    let row_language = language.clone();
    let row_target = target.clone();
    let row_overlay = overlay.clone();

    Effect::new(move |_| {
        apply_secondary_pane_writability(secondary_writable.get());
    });

    view! {
        <section
            class="workbench-pane secondary-target-pane"
            data-storage-prefix=prefix
            data-language=attr_language
            data-target=attr_target
            data-overlay=attr_overlay
        >
            <h4>{format!("{display_name} target pane")}</h4>
            <label>
                <input
                    id="secondary-pane-writable"
                    type="checkbox"
                    checked=move || secondary_writable.get()
                    on:change=move |event| secondary_writable.set(event_checkbox_checked(&event))
                />
                " Pane writable"
            </label>
            <table><tbody>
                <For
                    each=move || fields.clone()
                    key=|field| format!("{}::{}", field["path"].as_str().unwrap_or(""), field["source"].as_str().unwrap_or(""))
                    children=move |field| {
                        let path = field["path"].as_str().unwrap_or("").to_owned();
                        let source = field["source"].as_str().unwrap_or("").to_owned();
                        let id = format!("{}-{}", row_language, id_for_path(&path));
                        let value = staged_edit_for_pane(&row_language, &row_target, &row_overlay, &path, &source)
                            .as_ref()
                            .and_then(|edit| edit["value"].as_str().map(str::to_owned))
                            .or_else(|| field["target"].as_str().map(str::to_owned))
                            .unwrap_or_else(|| source.clone());
                        let field_name = field["field_name"].as_str().unwrap_or("field").to_owned();
                        let input_id = format!("secondary-translation-input-{id}");
                        let language = row_language.clone();
                        let target = row_target.clone();
                        let overlay = row_overlay.clone();
                        let input_path = path.clone();
                        let input_source = source.clone();
                        view! {
                            <tr class="secondary-field-row" data-path=path.clone() data-source=source.clone()>
                                <td>{field_name}</td>
                                <td>{source.clone()}</td>
                                <td>
                                    <input
                                        id=input_id
                                        value=value
                                        data-path=path.clone()
                                        data-source=source.clone()
                                        disabled=move || !secondary_writable.get()
                                        on:input=move |event| {
                                            stage_secondary_translation(
                                                &language,
                                                &target,
                                                &overlay,
                                                &input_path,
                                                &input_source,
                                                &event_input_value(&event),
                                            );
                                        }
                                    />
                                </td>
                            </tr>
                        }
                    }
                />
            </tbody></table>
            <section class="comparison-source-strings">
                <h5>Source String comparison</h5>
                <ul>
                    <For
                        each=move || source_strings.clone()
                        key=|string| string["source"].as_str().unwrap_or("").to_owned()
                        children=move |string| {
                            let source = string["source"].as_str().unwrap_or("").to_owned();
                            let target_preview = string["target_preview"].as_str().unwrap_or("").to_owned();
                            let status = string["status"].as_str().unwrap_or("unknown").to_owned();
                            view! {
                                <li class="comparison-source-string-row" data-source=source.clone()>
                                    {format!("{source} → {target_preview} ({status})")}
                                </li>
                            }
                        }
                    />
                </ul>
            </section>
            <section class="comparison-cards">
                <h5>Card comparison</h5>
                <ul>
                    <For
                        each=move || cards.clone()
                        key=|card| card["card_id"].as_str().unwrap_or("").to_owned()
                        children=move |card| {
                            let card_id = card["card_id"].as_str().unwrap_or("").to_owned();
                            let title = card["title"].as_str().unwrap_or("card").to_owned();
                            let template = card["template_name"].as_str().unwrap_or("template").to_owned();
                            let status = card["status"].as_str().unwrap_or("unknown").to_owned();
                            view! {
                                <li class="comparison-card-row" data-card-id=card_id>
                                    {format!("{title} · {template} · {status}")}
                                </li>
                            }
                        }
                    />
                </ul>
                {selected_card.map(|card| view! {
                    <div class="comparison-card-preview" inner_html=preview_html(&card["target_preview"])></div>
                })}
            </section>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NewLanguagePanel(pivot: Value, capability: RwSignal<WorkbenchWriteCapability>) -> impl IntoView {
    let languages = pivot["selection_options"]["languages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let initial_template = languages
        .iter()
        .find(|language| language["active"].as_bool().unwrap_or(false))
        .or_else(|| languages.first())
        .and_then(|language| language["code"].as_str())
        .unwrap_or("")
        .to_owned();
    let template = RwSignal::new(initial_template);
    let code = RwSignal::new(String::new());
    let display_name = RwSignal::new(String::new());
    let status = RwSignal::new(NewLanguageStatus::Idle);

    let preview = move |_| {
        status.set(NewLanguageStatus::Loading);
        let template = non_empty(template.get_untracked());
        let code = non_empty(code.get_untracked());
        let display_name = non_empty(display_name.get_untracked());
        spawn_local(async move {
            match fetch_new_language_preview(template, code, display_name).await {
                Ok(value) => status.set(NewLanguageStatus::Preview(value)),
                Err(error) => {
                    status.set(NewLanguageStatus::Error(format!("Preview failed: {error}")))
                }
            }
        });
    };

    let confirm = move |_| {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            status.set(NewLanguageStatus::Error(
                "Preview the new language before creating it.".to_owned(),
            ));
            return;
        };
        let Some(request) = collect_new_language_request(&document) else {
            status.set(NewLanguageStatus::Error(
                "Preview the new language before creating it.".to_owned(),
            ));
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
        status.set(NewLanguageStatus::Loading);
        spawn_local(async move {
            match post_json("/api/workbench/new-language", &request).await {
                Ok(value) => {
                    status.set(NewLanguageStatus::Created(new_language_result_text(
                        "Created", &value,
                    )));
                    let generation = begin_selection_request();
                    match fetch_note_pivot_query(Some(code), Some(target), Some(overlay), None)
                        .await
                    {
                        Ok(pivot) if is_current_selection_generation(generation) => {
                            publish_note_pivot_panel(&pivot)
                        }
                        Ok(_) => {}
                        Err(error) if is_current_selection_generation(generation) => {
                            status.set(NewLanguageStatus::Error(error));
                        }
                        Err(_) => {}
                    }
                }
                Err(error) => {
                    status.set(NewLanguageStatus::Error(format!("Create failed: {error}")))
                }
            }
        });
    };

    view! {
        <section id="new-language-panel" class="new-language-panel">
            <h3>New language scaffold</h3>
            <label>
                "Template language "
                <select id="new-language-template" on:change=move |event| template.set(event_select_value(&event))>
                    <For
                        each=move || languages.clone()
                        key=|language| language["code"].as_str().unwrap_or("").to_owned()
                        children=move |language| {
                            let code = language["code"].as_str().unwrap_or("").to_owned();
                            let label = language["display_name"].as_str().unwrap_or(&code).to_owned();
                            let selected = language["active"].as_bool().unwrap_or(false);
                            view! { <option value=code selected=selected>{label}</option> }
                        }
                    />
                </select>
            </label>
            <label>"Code "<input id="new-language-code" placeholder="nb" on:input=move |event| code.set(event_input_value(&event)) /></label>
            <label>"Display name "<input id="new-language-display-name" placeholder="Norwegian Bokmal" on:input=move |event| display_name.set(event_input_value(&event)) /></label>
            <button id="new-language-preview-button" type="button" on:click=preview>Preview new language</button>
            " "
            <button
                id="new-language-confirm-button"
                type="button"
                disabled=move || !capability.get().enabled
                title=move || if capability.get().enabled { "Unsafe development source mutation" } else { "Read-only Workbench" }
                on:click=confirm
            >
                {move || if capability.get().enabled { "Create language (unsafe development)" } else { "Create unavailable — read-only" }}
            </button>
            <div
                id="new-language-preview-output"
                data-validation-ok=move || matches!(status.get(), NewLanguageStatus::Preview(_)).to_string()
            >
                {move || view_new_language_status(status.get())}
            </div>
        </section>
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Default)]
enum NewLanguageStatus {
    #[default]
    Idle,
    Loading,
    Error(String),
    Preview(Value),
    Created(String),
}

#[cfg(target_arch = "wasm32")]
fn view_new_language_status(status: NewLanguageStatus) -> AnyView {
    match status {
        NewLanguageStatus::Idle => "".into_any(),
        NewLanguageStatus::Loading => {
            view! { <p class="workbench-loading">Loading...</p> }.into_any()
        }
        NewLanguageStatus::Error(error) => error.into_any(),
        NewLanguageStatus::Created(text) => text.into_any(),
        NewLanguageStatus::Preview(value) => {
            view! { <NewLanguagePreview value=value /> }.into_any()
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NewLanguagePreview(value: Value) -> impl IntoView {
    let code = value["language"]["code"].as_str().unwrap_or("").to_owned();
    let template = value["language"]["template_language"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let primary_target = value["language"]["primary_target"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    let affected_files = value["affected_files"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let groups = value["groups"].as_array().cloned().unwrap_or_default();
    let targets = value["targets"].as_array().cloned().unwrap_or_default();
    let manifest_yaml = value["manifest_yaml"].as_str().unwrap_or("").to_owned();

    view! {
        <div class="new-language-preview" data-code=code data-template=template data-primary-target=primary_target>
            <p>Preview ready. Affected files:</p>
            <ul>
                <For
                    each=move || affected_files.clone()
                    key=|file| file["path"].as_str().unwrap_or("unknown").to_owned()
                    children=move |file| view! { <li>{file["path"].as_str().unwrap_or("unknown").to_owned()}</li> }
                />
            </ul>
            <table><tbody>
                <For
                    each=move || groups.clone()
                    key=|group| group["label"].as_str().unwrap_or("").to_owned()
                    children=move |group| {
                        let label = group["label"].as_str().unwrap_or("").to_owned();
                        let template_overlay_id = group["template_overlay_id"].as_str().unwrap_or("").to_owned();
                        let selected = group["selected"].as_bool().unwrap_or(false);
                        let overlay_id = group["overlay_id"].as_str().unwrap_or("").to_owned();
                        let file = group["file"].as_str().unwrap_or("").to_owned();
                        let checkbox_id = format!("new-language-group-{label}");
                        view! {
                            <tr class="new-language-group-row" data-label=label.clone() data-template-overlay-id=template_overlay_id>
                                <td><label><input class="new-language-group-selected" id=checkbox_id type="checkbox" checked=selected />" "{label.clone()}</label></td>
                                <td><input class="new-language-group-overlay-id" value=overlay_id /></td>
                                <td><input class="new-language-group-file" value=file /></td>
                            </tr>
                        }
                    }
                />
            </tbody></table>
            <table><tbody>
                <For
                    each=move || targets.clone()
                    key=|target| target["label"].as_str().unwrap_or("").to_owned()
                    children=move |target| {
                        let label = target["label"].as_str().unwrap_or("").to_owned();
                        let target_id = target["target_id"].as_str().unwrap_or("").to_owned();
                        view! {
                            <tr class="new-language-target-row" data-label=label.clone()>
                                <td>{label.clone()}</td>
                                <td><input class="new-language-target-id" value=target_id /></td>
                            </tr>
                        }
                    }
                />
            </tbody></table>
            <pre>{manifest_yaml}</pre>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
fn first_secondary_language(pivot: &Value) -> String {
    pivot["selection_options"]["languages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|language| !language["active"].as_bool().unwrap_or(false))
        .and_then(|language| language["code"].as_str())
        .unwrap_or("")
        .to_owned()
}

#[cfg(target_arch = "wasm32")]
fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

#[cfg(target_arch = "wasm32")]
fn event_checkbox_checked(event: &web_sys::Event) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .is_some_and(|input| input.checked())
}

#[cfg(target_arch = "wasm32")]
fn event_input_value(event: &web_sys::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn event_text_content(event: &web_sys::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|element| element.text_content())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn event_select_value(event: &web_sys::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|select| select.value())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn focus_input(input_id: &str) {
    if let Some(input) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(input_id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.set_read_only(false);
        let _ = input.focus();
    }
}

#[cfg(target_arch = "wasm32")]
fn set_element_text(element_id: &str, value: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(element_id))
    {
        element.set_text_content(Some(value));
    }
}

#[cfg(target_arch = "wasm32")]
fn effective_source_for_dom(
    prefix: &str,
    path: &str,
    source: &str,
    source_input_id: &str,
) -> String {
    staging::staged_source_for_parts(prefix, path, source)
        .and_then(|edit| edit["value"].as_str().map(str::to_owned))
        .or_else(|| {
            web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id(source_input_id))
                .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
                .map(|input| input.value())
                .filter(|value| value != source)
        })
        .unwrap_or_else(|| source.to_owned())
}

#[cfg(target_arch = "wasm32")]
fn decorate_note_detail_dom(detail: &Value) {
    decorate_inline_target_fields(detail);
    restore_staged_dom_state(detail);
    update_staged_count_for_pivot(detail);
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        apply_pane_writability(
            checkbox_checked(&document, "source-pane-writable"),
            checkbox_checked(&document, "target-pane-writable"),
        );
    }
    refresh_progress_from_dom();
}

#[cfg(target_arch = "wasm32")]
fn decorate_inline_target_fields(detail: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(note) = detail["notes"].as_array().and_then(|notes| notes.first()) else {
        return;
    };
    let note_id = note["note_id"].as_str().unwrap_or("");
    for field in note["fields"].as_array().into_iter().flatten() {
        if !field["editable"].as_bool().unwrap_or(false) {
            continue;
        }
        let field_id = field["field_id"].as_str().unwrap_or("");
        let field_name = field["field_name"].as_str().unwrap_or("field");
        let path = field["path"].as_str().unwrap_or("");
        let source = field["source"].as_str().unwrap_or("");
        let staged = staged_edit_for(detail, path, source).is_some();
        let selector = format!(
            ".note-card[data-note-id=\"{}\"] .target-preview [data-preview-field-id=\"{}\"]",
            css_escape(note_id),
            css_escape(field_id),
        );
        let Ok(nodes) = document.query_selector_all(&selector) else {
            continue;
        };
        for index in 0..nodes.length() {
            let Some(node) = nodes.get(index) else {
                continue;
            };
            let Ok(element) = node.dyn_into::<web_sys::Element>() else {
                continue;
            };
            add_class(&element, "inline-target-field");
            let _ = element.set_attribute("contenteditable", "true");
            let _ = element.set_attribute("tabindex", "0");
            let _ = element.set_attribute("role", "textbox");
            let _ = element.set_attribute(
                "aria-label",
                &format!("Edit target translation for {field_name} inline"),
            );
            let _ = element.set_attribute("data-staged", if staged { "true" } else { "false" });
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn add_class(element: &web_sys::Element, class: &str) {
    let class_name = element.get_attribute("class").unwrap_or_default();
    if class_name
        .split_whitespace()
        .any(|existing| existing == class)
    {
        return;
    }
    let class_name = if class_name.is_empty() {
        class.to_owned()
    } else {
        format!("{class_name} {class}")
    };
    let _ = element.set_attribute("class", &class_name);
}

#[cfg(target_arch = "wasm32")]
fn handle_note_inline_target_input(detail: &Value, note_id: &str, event: web_sys::Event) {
    let Some(element) = event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let Some(field_id) = element.get_attribute("data-preview-field-id") else {
        return;
    };
    let Some(note) = detail["notes"].as_array().and_then(|notes| notes.first()) else {
        return;
    };
    let Some(field) = note["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|field| {
            field["field_id"].as_str().unwrap_or("") == field_id
                && field["editable"].as_bool().unwrap_or(false)
        })
    else {
        return;
    };
    let path = field["path"].as_str().unwrap_or("");
    let source = field["source"].as_str().unwrap_or("");
    let id = id_for_path(path);
    let prefix = storage_prefix(detail);
    let input_id = format!("translation-input-{id}");
    let mode_id = format!("translation-mode-{id}");
    let source_input_id = format!("source-input-{id}");
    let target_id = format!("target-text-{id}");
    let status_id = format!("status-text-{id}");
    let value = element.text_content().unwrap_or_default();
    let mode = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&mode_id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|select| select.value())
        .unwrap_or_else(|| "direct".to_owned());
    let effective_source = effective_source_for_dom(&prefix, path, source, &source_input_id);
    staging::stage_translation(&prefix, path, source, &effective_source, &value, &mode);
    if let Some(input) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(&input_id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
    {
        input.set_value(&value);
    }
    set_element_text(&target_id, &value);
    set_element_text(&status_id, &format!("staged_{mode}"));
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        update_target_preview_field(&document, note_id, &field_id, &value);
        mark_preview_field_staged(&document, note_id, &field_id, true);
    }
    update_staged_count(&prefix);
    refresh_progress_from_dom();
}

#[cfg(target_arch = "wasm32")]
fn open_workbench_view(view: WorkbenchView) {
    set_selected_view(view);
    maybe_load_current_view(view);
}

#[cfg(target_arch = "wasm32")]
fn set_selected_view(view: WorkbenchView) {
    if let Some(signals) = workbench_signals() {
        signals.selected_view.set(view);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_top_level_error(message: Option<String>) {
    if let Some(signals) = workbench_signals() {
        signals.top_level_error.set(message);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_note_pivot_state(pivot: Option<Value>) {
    if let Some(signals) = workbench_signals() {
        signals.note_pivot.set(pivot);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_selected_note_state(note_id: Option<String>) {
    if let Some(signals) = workbench_signals() {
        signals.selected_note.set(note_id);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_note_detail_state(state: NoteDetailState) {
    if let Some(signals) = workbench_signals() {
        signals.note_detail.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_card_list_state(state: LazyValueState) {
    if let Some(signals) = workbench_signals() {
        signals.card_list.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_selected_card_state(card_id: Option<String>) {
    if let Some(signals) = workbench_signals() {
        signals.selected_card.set(card_id);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_card_detail_state(state: DetailValueState) {
    if let Some(signals) = workbench_signals() {
        signals.card_detail.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_source_string_list_state(state: LazyValueState) {
    if let Some(signals) = workbench_signals() {
        signals.source_string_list.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_selected_source_string_state(source: Option<String>) {
    if let Some(signals) = workbench_signals() {
        signals.selected_source_string.set(source);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_source_string_detail_state(state: DetailValueState) {
    if let Some(signals) = workbench_signals() {
        signals.source_string_detail.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_optional_metadata_state(state: LazyValueState) {
    if let Some(signals) = workbench_signals() {
        signals.optional_metadata.set(state);
    }
}

#[cfg(target_arch = "wasm32")]
fn set_current_note_pivot(pivot: &Value) {
    if let Some(signals) = workbench_signals() {
        signals.current_note_pivot.set(Some(pivot.clone()));
    }
}

#[cfg(target_arch = "wasm32")]
fn maybe_load_current_view(view: WorkbenchView) {
    if let Some(pivot) =
        workbench_signals().and_then(|signals| signals.current_note_pivot.get_untracked())
    {
        maybe_load_view(&pivot, view.key());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchWriteCapability {
    pub enabled: bool,
    pub mode: String,
    pub development_build: bool,
    pub runtime_opt_in: bool,
    pub warning: String,
}

impl Default for WorkbenchWriteCapability {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: "read_only".to_owned(),
            development_build: false,
            runtime_opt_in: false,
            warning: "Loading server capability; source writes remain disabled.".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    pub manifest: String,
    pub language_count: usize,
    pub target_count: usize,
    pub fingerprint_count: usize,
    pub write_capability: WorkbenchWriteCapability,
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
            write_capability: WorkbenchWriteCapability {
                enabled: value["write_capability"]["enabled"]
                    .as_bool()
                    .unwrap_or(false),
                mode: value["write_capability"]["mode"]
                    .as_str()
                    .unwrap_or("read_only")
                    .to_owned(),
                development_build: value["write_capability"]["development_build"]
                    .as_bool()
                    .unwrap_or(false),
                runtime_opt_in: value["write_capability"]["runtime_opt_in"]
                    .as_bool()
                    .unwrap_or(false),
                warning: value["write_capability"]["warning"]
                    .as_str()
                    .unwrap_or("Workbench write capability is unavailable.")
                    .to_owned(),
            },
        }
    }
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
        let _ = element.set_attribute("data-write-mode", &workspace.write_capability.mode);
        let _ = element.set_attribute(
            "data-write-enabled",
            &workspace.write_capability.enabled.to_string(),
        );
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

#[cfg(target_arch = "wasm32")]
fn publish_note_pivot_error(message: &str) {
    set_top_level_error(Some(message.to_owned()));
}

#[cfg(target_arch = "wasm32")]
fn publish_note_pivot_panel(pivot: &Value) {
    set_top_level_error(None);
    set_selected_view(WorkbenchView::Notes);
    set_current_note_pivot(pivot);
    set_note_pivot_state(Some(pivot.clone()));
    reset_secondary_pivot_states();

    let first_note = first_note_id(pivot);
    set_selected_note_state(first_note.clone());
    set_note_detail_state(if first_note.is_some() {
        NoteDetailState::Loading
    } else {
        NoteDetailState::Empty
    });
    update_staged_count_for_pivot(pivot);
    if let Some(note_id) = first_note {
        let (language, target, overlay) = pivot_selection_parts(pivot);
        load_note_detail_for_parts(language, target, overlay, note_id);
    }
}

#[cfg(target_arch = "wasm32")]
fn reset_secondary_pivot_states() {
    set_card_list_state(LazyValueState::Unloaded);
    set_selected_card_state(None);
    set_card_detail_state(DetailValueState::Empty);
    set_source_string_list_state(LazyValueState::Unloaded);
    set_selected_source_string_state(None);
    set_source_string_detail_state(DetailValueState::Empty);
    set_optional_metadata_state(LazyValueState::Unloaded);
}

#[cfg(target_arch = "wasm32")]
fn first_note_id(pivot: &Value) -> Option<String> {
    pivot["rows"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(|row| row["note_id"].as_str())
        .map(str::to_owned)
}

#[cfg(target_arch = "wasm32")]
fn preview_html(preview: &Value) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        "<style>{}</style>",
        preview["styling"].as_str().unwrap_or("")
    ));
    for card in preview["cards"].as_array().into_iter().flatten() {
        let template_id = card["template_id"].as_str().unwrap_or("template");
        let template_name = card["template_name"].as_str().unwrap_or(template_id);
        html.push_str(&format!(
            "<div class=\"anki-card-preview\" data-template-id=\"{}\" data-template-name=\"{}\"><header class=\"card-template-header\"><span class=\"card-template-label\">{}</span><code>{}</code></header>",
            escape_html(template_id),
            escape_html(template_name),
            escape_html(template_name),
            escape_html(template_id),
        ));
        html.push_str("<div class=\"card-side card-side-question\"><strong>Question</strong>");
        html.push_str(card["question_html"].as_str().unwrap_or(""));
        html.push_str("</div><div class=\"card-side card-side-answer\"><strong>Answer</strong>");
        html.push_str(card["answer_html"].as_str().unwrap_or(""));
        html.push_str("</div></div>");
    }
    html
}

#[cfg(target_arch = "wasm32")]
fn css_escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_arch = "wasm32")]
fn load_note_detail_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    note_id: String,
) {
    let selection_generation = current_selection_generation();
    let detail_generation = begin_note_detail_request();
    spawn_local(async move {
        match fetch_note_detail_query(language, target, overlay, note_id).await {
            Ok(detail)
                if is_current_selection_generation(selection_generation)
                    && is_current_note_detail_generation(detail_generation) =>
            {
                publish_note_detail_panel(&detail);
            }
            Ok(_) => {}
            Err(error)
                if is_current_selection_generation(selection_generation)
                    && is_current_note_detail_generation(detail_generation) =>
            {
                set_note_detail_state(NoteDetailState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn publish_note_detail_panel(detail: &Value) {
    if detail["notes"]
        .as_array()
        .and_then(|notes| notes.first())
        .is_some()
    {
        set_note_detail_state(NoteDetailState::Loaded(detail.clone()));
    } else {
        set_note_detail_state(NoteDetailState::Empty);
    }
    update_staged_count_for_pivot(detail);
}

#[cfg(target_arch = "wasm32")]
fn load_card_list_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
) {
    let selection_generation = current_selection_generation();
    spawn_local(async move {
        match fetch_card_list_query(
            language.clone(),
            target.clone(),
            overlay.clone(),
            filter,
            content_group.clone(),
        )
        .await
        {
            Ok(list) if is_current_selection_generation(selection_generation) => {
                let first_card = list["rows"]
                    .as_array()
                    .and_then(|rows| rows.first())
                    .and_then(|row| row["card_id"].as_str())
                    .map(str::to_owned);
                set_card_list_state(LazyValueState::Loaded(list.clone()));
                set_selected_card_state(first_card.clone());
                if let Some(card_id) = first_card {
                    set_card_detail_state(DetailValueState::Loading(
                        "Loading selected card…".to_owned(),
                    ));
                    load_card_detail_for_parts(
                        language,
                        target,
                        overlay,
                        card_id,
                        list["filters"]["active"].as_str().map(str::to_owned),
                        list["filters"]["content_group"].as_str().map(str::to_owned),
                    );
                } else {
                    set_card_detail_state(DetailValueState::Empty);
                }
            }
            Ok(_) => {}
            Err(error) if is_current_selection_generation(selection_generation) => {
                set_card_list_state(LazyValueState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn load_card_detail_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    card: String,
    filter: Option<String>,
    content_group: Option<String>,
) {
    let selection_generation = current_selection_generation();
    let detail_generation = begin_card_detail_request();
    spawn_local(async move {
        match fetch_card_pivot_query(language, target, overlay, Some(card), filter, content_group)
            .await
        {
            Ok(pivot)
                if is_current_selection_generation(selection_generation)
                    && is_current_card_detail_generation(detail_generation) =>
            {
                set_card_detail_state(DetailValueState::Loaded(pivot));
            }
            Ok(_) => {}
            Err(error)
                if is_current_selection_generation(selection_generation)
                    && is_current_card_detail_generation(detail_generation) =>
            {
                set_card_detail_state(DetailValueState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn load_source_string_list_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
) {
    let selection_generation = current_selection_generation();
    spawn_local(async move {
        match fetch_source_string_list_query(
            language.clone(),
            target.clone(),
            overlay.clone(),
            content_group,
            status,
        )
        .await
        {
            Ok(list) if is_current_selection_generation(selection_generation) => {
                let first_source = list["rows"]
                    .as_array()
                    .and_then(|rows| rows.first())
                    .and_then(|row| row["source"].as_str())
                    .map(str::to_owned);
                set_source_string_list_state(LazyValueState::Loaded(list.clone()));
                set_selected_source_string_state(first_source.clone());
                if let Some(source) = first_source {
                    set_source_string_detail_state(DetailValueState::Loading(
                        "Loading selected source string…".to_owned(),
                    ));
                    load_source_string_detail_for_parts(
                        language,
                        target,
                        overlay,
                        source,
                        list["filters"]["content_group"].as_str().map(str::to_owned),
                        list["filters"]["status"].as_str().map(str::to_owned),
                    );
                } else {
                    set_source_string_detail_state(DetailValueState::Empty);
                }
            }
            Ok(_) => {}
            Err(error) if is_current_selection_generation(selection_generation) => {
                set_source_string_list_state(LazyValueState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn load_source_string_detail_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    source: String,
    content_group: Option<String>,
    status: Option<String>,
) {
    let selection_generation = current_selection_generation();
    let detail_generation = begin_source_string_detail_request();
    spawn_local(async move {
        match fetch_source_string_pivot_query(
            language,
            target,
            overlay,
            Some(source),
            content_group,
            status,
        )
        .await
        {
            Ok(source_pivot)
                if is_current_selection_generation(selection_generation)
                    && is_current_source_string_detail_generation(detail_generation) =>
            {
                set_source_string_detail_state(DetailValueState::Loaded(source_pivot));
            }
            Ok(_) => {}
            Err(error)
                if is_current_selection_generation(selection_generation)
                    && is_current_source_string_detail_generation(detail_generation) =>
            {
                set_source_string_detail_state(DetailValueState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn load_optional_metadata_for_parts(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
) {
    let selection_generation = current_selection_generation();
    spawn_local(async move {
        match fetch_optional_metadata_query(language, target, overlay).await {
            Ok(optional) if is_current_selection_generation(selection_generation) => {
                set_optional_metadata_state(LazyValueState::Loaded(optional));
            }
            Ok(_) => {}
            Err(error) if is_current_selection_generation(selection_generation) => {
                set_optional_metadata_state(LazyValueState::Error(error));
            }
            Err(_) => {}
        }
    });
}

#[cfg(target_arch = "wasm32")]
fn maybe_load_view(pivot: &Value, view: &str) {
    match view {
        "cards" if panel_is_unloaded("card-pivot-panel") => {
            set_card_list_state(LazyValueState::Loading("Loading card list…".to_owned()));
            let (language, target, overlay) = pivot_selection_parts(pivot);
            load_card_list_for_parts(language, target, overlay, None, None);
        }
        "source-strings" if panel_is_unloaded("source-string-pivot-panel") => {
            set_source_string_list_state(LazyValueState::Loading(
                "Loading source string list…".to_owned(),
            ));
            let (language, target, overlay) = pivot_selection_parts(pivot);
            load_source_string_list_for_parts(language, target, overlay, None, None);
        }
        "metadata" if panel_is_unloaded("optional-metadata-panel") => {
            set_optional_metadata_state(LazyValueState::Loading("Loading metadata…".to_owned()));
            let (language, target, overlay) = pivot_selection_parts(pivot);
            load_optional_metadata_for_parts(language, target, overlay);
        }
        _ => {}
    }
}

#[cfg(target_arch = "wasm32")]
fn panel_is_unloaded(panel_id: &str) -> bool {
    let Some(signals) = workbench_signals() else {
        return false;
    };
    match panel_id {
        "card-pivot-panel" => matches!(signals.card_list.get_untracked(), LazyValueState::Unloaded),
        "source-string-pivot-panel" => matches!(
            signals.source_string_list.get_untracked(),
            LazyValueState::Unloaded
        ),
        "optional-metadata-panel" => matches!(
            signals.optional_metadata.get_untracked(),
            LazyValueState::Unloaded
        ),
        _ => false,
    }
}

#[cfg(target_arch = "wasm32")]
fn lazy_state_key(state: &LazyValueState) -> &'static str {
    match state {
        LazyValueState::Unloaded => "unloaded",
        LazyValueState::Loading(_) => "loading",
        LazyValueState::Error(_) | LazyValueState::Loaded(_) => "loaded",
    }
}

#[cfg(target_arch = "wasm32")]
fn lazy_state_busy(state: &LazyValueState) -> &'static str {
    if matches!(state, LazyValueState::Loading(_)) {
        "true"
    } else {
        "false"
    }
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
        "input[id^='translation-input-'], select[id^='translation-mode-'], input[id^='card-translation-input-'], select[id^='card-translation-mode-'], input[id^='optional-translation-input'], input[id^='source-string-direct-input'], button[id^='source-string-direct-stage'], button[id^='source-string-no-change'], button[id^='source-string-contextual-stage-']",
        !target_writable,
    );
    set_contenteditable(
        &document,
        ".inline-target-field, .card-inline-target-field, .source-string-contextual-input",
        target_writable,
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
                && let Ok(element) = node.dyn_into::<web_sys::Element>()
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
fn set_contenteditable(document: &web_sys::Document, selector: &str, editable: bool) {
    if let Ok(nodes) = document.query_selector_all(selector) {
        for index in 0..nodes.length() {
            if let Some(node) = nodes.get(index)
                && let Ok(element) = node.dyn_into::<web_sys::Element>()
            {
                let _ = element
                    .set_attribute("contenteditable", if editable { "true" } else { "false" });
                let _ =
                    element.set_attribute("aria-disabled", if editable { "false" } else { "true" });
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
fn stage_translation_with_optional_context(
    prefix: &str,
    path: &str,
    storage_source: &str,
    effective_source: &str,
    value: &str,
    mode: &str,
    include_context_path: bool,
) {
    let mut edit = serde_json::json!({
        "kind": "translation",
        "path": path,
        "source": effective_source,
        "value": value,
        "mode": mode,
    });
    if include_context_path {
        edit["context_path"] = Value::String(path.to_owned());
    }
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(
            &staging::translation_key(prefix, path, storage_source),
            &edit.to_string(),
        );
    }
    staging::bump_current();
}

#[cfg(target_arch = "wasm32")]
fn stage_metadata_translation(
    prefix: &str,
    language: &str,
    target: &str,
    overlay: &str,
    path: &str,
    source: &str,
    value: &str,
) {
    let edit = serde_json::json!({
        "kind": "translation",
        "path": path,
        "source": source,
        "value": value,
        "mode": "direct",
        "language": language,
        "target": target,
        "overlay": overlay,
    });
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(
            &staging::translation_key(prefix, path, source),
            &edit.to_string(),
        );
    }
    staging::bump_current();
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
    staging::bump_current();
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
fn begin_selection_request() -> u64 {
    if let Some(signals) = workbench_signals() {
        let next = signals
            .selection_generation
            .get_untracked()
            .saturating_add(1);
        signals.selection_generation.set(next);
        next
    } else {
        0
    }
}

#[cfg(target_arch = "wasm32")]
fn current_selection_generation() -> u64 {
    workbench_signals()
        .map(|signals| signals.selection_generation.get_untracked())
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
fn next_signal_generation(signal: impl FnOnce(&WorkbenchSignals) -> RwSignal<u64>) -> u64 {
    if let Some(signals) = workbench_signals() {
        let signal = signal(signals);
        let next = signal.get_untracked().saturating_add(1);
        signal.set(next);
        next
    } else {
        0
    }
}

#[cfg(target_arch = "wasm32")]
fn signal_generation_is_current(
    signal: impl FnOnce(&WorkbenchSignals) -> RwSignal<u64>,
    generation: u64,
) -> bool {
    workbench_signals()
        .map(|signals| signal(signals).get_untracked() == generation)
        .unwrap_or(false)
}

#[cfg(target_arch = "wasm32")]
fn is_current_selection_generation(generation: u64) -> bool {
    current_selection_generation() == generation
}

#[cfg(target_arch = "wasm32")]
fn begin_note_detail_request() -> u64 {
    next_signal_generation(|signals| signals.note_detail_token)
}

#[cfg(target_arch = "wasm32")]
fn is_current_note_detail_generation(generation: u64) -> bool {
    signal_generation_is_current(|signals| signals.note_detail_token, generation)
}

#[cfg(target_arch = "wasm32")]
fn begin_card_detail_request() -> u64 {
    next_signal_generation(|signals| signals.card_detail_token)
}

#[cfg(target_arch = "wasm32")]
fn is_current_card_detail_generation(generation: u64) -> bool {
    signal_generation_is_current(|signals| signals.card_detail_token, generation)
}

#[cfg(target_arch = "wasm32")]
fn begin_source_string_detail_request() -> u64 {
    next_signal_generation(|signals| signals.source_string_detail_token)
}

#[cfg(target_arch = "wasm32")]
fn is_current_source_string_detail_generation(generation: u64) -> bool {
    signal_generation_is_current(|signals| signals.source_string_detail_token, generation)
}

#[cfg(target_arch = "wasm32")]
fn is_stale_workbench_request(error: &str) -> bool {
    error == STALE_WORKBENCH_REQUEST
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
fn current_selected_value(element_id: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| selected_value(&document, element_id))
}

#[cfg(target_arch = "wasm32")]
fn current_input_value(element_id: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| input_value(&document, element_id))
}

#[cfg(target_arch = "wasm32")]
fn current_text_content(element_id: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(element_id))
        .and_then(|element| element.text_content())
}

#[cfg(target_arch = "wasm32")]
fn current_active_filter() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| active_filter(&document))
}

#[cfg(target_arch = "wasm32")]
fn current_card_filter() -> Option<String> {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| {
            document
                .query_selector(".card-pivot-filters button.active")
                .ok()
                .flatten()
                .and_then(|element| element.get_attribute("data-filter"))
        })
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
    let generation = begin_selection_request();
    spawn_local(async move {
        match fetch_note_pivot_query(language, target, overlay, filter).await {
            Ok(pivot) if is_current_selection_generation(generation) => {
                publish_note_pivot_panel(&pivot)
            }
            Ok(_) => {}
            Err(error) if is_current_selection_generation(generation) => {
                publish_note_pivot_error(&error)
            }
            Err(_) => {}
        }
    });
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
            mark_preview_field_staged(&document, note_id, field_id, true);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn decorate_card_detail_dom(pivot: &Value) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(card) = pivot.get("selected_card").filter(|value| !value.is_null()) else {
        return;
    };
    for field in card["fields"].as_array().into_iter().flatten() {
        if !field["editable"].as_bool().unwrap_or(false) {
            continue;
        }
        let field_id = field["field_id"].as_str().unwrap_or("");
        let field_name = field["field_name"].as_str().unwrap_or("field");
        let path = field["path"].as_str().unwrap_or("");
        let source = field["source"].as_str().unwrap_or("");
        let staged = staged_edit_for(pivot, path, source).is_some();
        let selector = format!(
            "#card-pivot-panel .card-target-preview [data-preview-field-id=\"{}\"]",
            css_escape(field_id),
        );
        let Ok(nodes) = document.query_selector_all(&selector) else {
            continue;
        };
        for index in 0..nodes.length() {
            let Some(node) = nodes.get(index) else {
                continue;
            };
            let Ok(element) = node.dyn_into::<web_sys::Element>() else {
                continue;
            };
            append_class(&element, "card-inline-target-field");
            let _ = element.set_attribute("contenteditable", "true");
            let _ = element.set_attribute("tabindex", "0");
            let _ = element.set_attribute("role", "textbox");
            let _ = element.set_attribute(
                "aria-label",
                &format!("Edit card target translation for {field_name} inline"),
            );
            let _ = element.set_attribute("data-staged", if staged { "true" } else { "false" });
        }
    }
    apply_pane_writability(
        checkbox_checked(&document, "source-pane-writable"),
        checkbox_checked(&document, "target-pane-writable"),
    );
}

#[cfg(target_arch = "wasm32")]
fn handle_card_inline_target_input(pivot: &Value, event: web_sys::Event) {
    let Some(element) = event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|element| element.closest("[data-preview-field-id]").ok().flatten())
    else {
        return;
    };
    let Some(field_id) = element.get_attribute("data-preview-field-id") else {
        return;
    };
    let Some(card) = pivot.get("selected_card").filter(|value| !value.is_null()) else {
        return;
    };
    let Some(field) = card["fields"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|field| field["field_id"].as_str().unwrap_or("") == field_id)
    else {
        return;
    };
    let prefix = storage_prefix(pivot);
    let path = field["path"].as_str().unwrap_or("");
    let source = field["source"].as_str().unwrap_or("");
    let id = id_for_path(path);
    let value = element.text_content().unwrap_or_default();
    let mode_id = format!("card-translation-mode-{id}");
    let source_input_id = format!("card-source-input-{id}");
    let input_id = format!("card-translation-input-{id}");
    let target_id = format!("card-target-text-{id}");
    let status_id = format!("card-status-text-{id}");
    let mode = current_selected_value(&mode_id).unwrap_or_else(|| "direct".to_owned());
    let effective_source = effective_source_for_dom(&prefix, path, source, &source_input_id);
    stage_translation_with_optional_context(
        &prefix,
        path,
        source,
        &effective_source,
        &value,
        &mode,
        mode == "contextual",
    );
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Some(input) = document
            .get_element_by_id(&input_id)
            .and_then(|element| element.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            input.set_value(&value);
        }
        set_element_text(&target_id, &value);
        set_element_text(&status_id, &format!("staged_{mode}"));
        update_card_preview_field(&document, "card-target-preview", &field_id, &value);
        mark_card_preview_field_staged(&document, &field_id, true);
    }
    update_staged_count(&prefix);
}

#[cfg(target_arch = "wasm32")]
fn append_class(element: &web_sys::Element, class_name: &str) {
    let existing = element.get_attribute("class").unwrap_or_default();
    if existing.split_whitespace().any(|class| class == class_name) {
        return;
    }
    let next = if existing.is_empty() {
        class_name.to_owned()
    } else {
        format!("{existing} {class_name}")
    };
    let _ = element.set_attribute("class", &next);
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
        preview_class,
        css_escape(field_id)
    );
    let Ok(nodes) = document.query_selector_all(&selector) else {
        return;
    };
    let active_element = document.active_element();
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            if active_element.as_ref() == Some(&element) {
                continue;
            }
            element.set_text_content(Some(value));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn mark_card_preview_field_staged(document: &web_sys::Document, field_id: &str, staged: bool) {
    let selector = format!(
        "#card-pivot-panel .card-target-preview [data-preview-field-id=\"{}\"]",
        css_escape(field_id)
    );
    let Ok(nodes) = document.query_selector_all(&selector) else {
        return;
    };
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            let _ = element.set_attribute("data-staged", if staged { "true" } else { "false" });
        }
    }
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
fn mark_preview_field_staged(
    document: &web_sys::Document,
    note_id: &str,
    field_id: &str,
    staged: bool,
) {
    let selector = format!(
        ".note-card[data-note-id=\"{}\"] .target-preview [data-preview-field-id=\"{}\"]",
        css_escape(note_id),
        css_escape(field_id),
    );
    let Ok(nodes) = document.query_selector_all(&selector) else {
        return;
    };
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            let _ = element.set_attribute("data-staged", if staged { "true" } else { "false" });
        }
    }
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
    let active_element = document.active_element();
    for index in 0..nodes.length() {
        if let Some(node) = nodes.get(index)
            && let Ok(element) = node.dyn_into::<web_sys::Element>()
        {
            if active_element.as_ref() == Some(&element) {
                continue;
            }
            element.set_text_content(Some(value));
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
                "direct_translation" | "contextual_translation" | "no_change"
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
        if let Some(first_child) = progress.first_child() {
            first_child.set_node_value(Some(&format!(
                "Main note-field progress: {complete} / {total} complete, {missing} missing, {stale} stale ("
            )));
        }
        if let Some(staged_element) = document.get_element_by_id("staged-edit-count") {
            staged_element.set_text_content(Some(&staged));
            let _ = staged_element.set_attribute("data-count", &staged);
            if let Some(tail) = staged_element.next_sibling() {
                tail.set_node_value(Some(" staged)"));
            } else {
                let tail = document.create_text_node(" staged)");
                let _ = progress.append_child(&tail);
            }
        }
    }
}

#[allow(dead_code)]
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
    for diagnostic in value["validation"]["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let code = diagnostic["code"].as_str().unwrap_or("validation_failed");
        let category = diagnostic["category"].as_str().unwrap_or("validation");
        let path = diagnostic["path"]
            .as_str()
            .or_else(|| diagnostic["address"].as_str())
            .unwrap_or("unknown path");
        let message = diagnostic["message"]
            .as_str()
            .unwrap_or("validation failed");
        lines.push(format!("- {code} [{category}] at {path}: {message}"));
    }
    lines.join("\n")
}

#[cfg(target_arch = "wasm32")]
async fn post_json(url: &str, body: &Value) -> Result<Value, String> {
    let response = gloo_net::http::Request::post(url)
        .json(body)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let ok = response.ok();
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    if ok {
        Ok(value)
    } else {
        Err(structured_api_error(status, &value))
    }
}

#[cfg(target_arch = "wasm32")]
fn structured_api_error(status: u16, value: &Value) -> String {
    let error = &value["error"];
    let code = error["code"].as_str().unwrap_or("request_failed");
    let path = error["diagnostics"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["path"].as_str());
    let message = error["message"].as_str().unwrap_or("request failed");
    match path {
        Some(path) => format!("{code} ({status}) at {path}: {message}"),
        None => format!("{code} ({status}): {message}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn staged_edit_for(pivot: &Value, path: &str, source: &str) -> Option<Value> {
    staging::staged_translation_for_parts(&storage_prefix(pivot), path, source)
}

#[cfg(target_arch = "wasm32")]
fn staged_source_edit_for(pivot: &Value, path: &str, source: &str) -> Option<Value> {
    staging::staged_source_for_parts(&storage_prefix(pivot), path, source)
}

#[cfg(target_arch = "wasm32")]
fn collect_staged_edits(pivot: &Value) -> Vec<Value> {
    staging::collect_staged_edits_for_prefixes(&active_storage_prefixes(pivot))
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
    staging::clear_prefix(prefix);
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
    let count = staging::staged_count_for_prefixes(prefixes);
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("staged-edit-count"))
    {
        element.set_text_content(Some(&count.to_string()));
        let _ = element.set_attribute("data-count", &count.to_string());
    }
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
    staging::storage_prefix_for_parts(language, target, overlay)
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    staging::local_storage()
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
    let generation = begin_selection_request();
    let pivot = fetch_note_pivot_query(None, None, None, None).await?;
    if is_current_selection_generation(generation) {
        Ok(pivot)
    } else {
        Err(STALE_WORKBENCH_REQUEST.to_owned())
    }
}

#[cfg(target_arch = "wasm32")]
async fn fetch_note_pivot_for_pivot(pivot: &Value) -> Result<Value, String> {
    // Post-apply refreshes should not start a new user selection; they only publish
    // if the user has not switched language/target/overlay while the refresh ran.
    let generation = current_selection_generation();
    let pivot = fetch_note_pivot_query(
        pivot["language"]["code"].as_str().map(str::to_owned),
        pivot["target"]["label"].as_str().map(str::to_owned),
        pivot["overlay"]["label"].as_str().map(str::to_owned),
        pivot["filters"]["active"].as_str().map(str::to_owned),
    )
    .await?;
    if is_current_selection_generation(generation) {
        Ok(pivot)
    } else {
        Err(STALE_WORKBENCH_REQUEST.to_owned())
    }
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
    get_workbench_json("/api/workbench/note-list", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_note_detail_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    note_id: String,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    params.push(format!("note={}", encode_query_component(&note_id)));
    get_workbench_json("/api/workbench/note-detail", params).await
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
    get_workbench_json("/api/workbench/metadata", params).await
}

#[cfg(target_arch = "wasm32")]
async fn fetch_card_list_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    filter: Option<String>,
    content_group: Option<String>,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    if let Some(filter) = filter.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!("filter={}", encode_query_component(&filter)));
    }
    if let Some(content_group) = content_group.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!(
            "content_group={}",
            encode_query_component(&content_group)
        ));
    }
    get_workbench_json("/api/workbench/card-list", params).await
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
async fn fetch_source_string_list_query(
    language: Option<String>,
    target: Option<String>,
    overlay: Option<String>,
    content_group: Option<String>,
    status: Option<String>,
) -> Result<Value, String> {
    let mut params = workbench_selection_params(language, target, overlay);
    if let Some(content_group) = content_group.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!(
            "content_group={}",
            encode_query_component(&content_group)
        ));
    }
    if let Some(status) = status.filter(|value| !value.is_empty() && value != "all") {
        params.push(format!("status={}", encode_query_component(&status)));
    }
    get_workbench_json("/api/workbench/source-string-list", params).await
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
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let ok = response.ok();
    let value = response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    if ok {
        Ok(value)
    } else {
        Err(structured_api_error(status, &value))
    }
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

#[cfg(test)]
mod typed_apply_validation_tests {
    use super::*;

    #[test]
    fn apply_preview_renders_structured_diagnostics_without_legacy_error_strings() {
        let preview = serde_json::json!({
            "affected_files": [{"path": "deck.yaml"}],
            "file_groups": [],
            "changed_entries": [],
            "validation": {
                "schema_version": 1,
                "ok": false,
                "diagnostics": [{
                    "code": "invalid_html_content",
                    "category": "validation",
                    "path": "deck.description",
                    "message": "line 1: unclosed HTML tag <p>"
                }]
            }
        });

        let rendered = apply_result_text("Apply preview", &preview);
        assert!(rendered.contains(
            "invalid_html_content [validation] at deck.description: line 1: unclosed HTML tag <p>"
        ));
        assert!(!rendered.contains("unknown"));
    }
}
