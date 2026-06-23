use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Task, Theme};
use serde_json::Value;

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
    status: String,
}

#[derive(Clone, Debug)]
pub enum Message {
    RefreshWorkspace,
    WorkspaceLoaded(Result<WorkspaceSummary, String>),
}

impl WorkbenchApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                workspace: None,
                status: "Loading workspace metadata…".to_owned(),
            },
            Task::perform(fetch_workspace(), Message::WorkspaceLoaded),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RefreshWorkspace => {
                self.status = "Refreshing workspace metadata…".to_owned();
                Task::perform(fetch_workspace(), Message::WorkspaceLoaded)
            }
            Message::WorkspaceLoaded(Ok(summary)) => {
                self.status = "Workspace metadata loaded from /api/workspace.".to_owned();
                self.workspace = Some(summary);
                Task::none()
            }
            Message::WorkspaceLoaded(Err(error)) => {
                self.status = format!("Unable to load workspace metadata: {error}");
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

        let sidebar = panel(
            "Languages",
            column![
                text("Language dashboard").size(22),
                text(format!("{language_count} configured language(s)")),
                text("Source and target selection will appear here."),
            ],
        )
        .width(Length::Fixed(260.0));

        let canvas = panel(
            "Deck canvas",
            column![
                text("Note pivot placeholder").size(28),
                text("The first workbench slice will render source/target note context here."),
                text(format!("Manifest: {manifest}")),
                button("Refresh workspace metadata").on_press(Message::RefreshWorkspace),
            ],
        )
        .width(Length::Fill);

        let inspector = panel(
            "Inspector",
            column![
                text("Pending changes").size(22),
                text("No browser-local edits staged in this scaffold."),
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

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_workspace() -> Result<WorkspaceSummary, String> {
    Ok(WorkspaceSummary {
        manifest: "Run through `brainbrew workbench serve` to fetch /api/workspace".to_owned(),
        language_count: 0,
        target_count: 0,
        fingerprint_count: 0,
    })
}
