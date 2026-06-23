use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use tempfile::TempDir;
use thirtyfour::prelude::*;

#[tokio::test]
async fn workbench_app_shell_loads_workspace_metadata() -> Result<()> {
    let artifacts = ArtifactDir::new("app-shell")?;
    let workspace = TempDir::new().context("create E2E workspace")?;
    write_small_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let smoke_result = run_app_shell_smoke(&driver, &server).await;
    if let Err(error) = &smoke_result {
        let _ = artifacts.save_browser_failure(&driver, error).await;
    }
    let quit_result = driver.quit().await.context("quit browser session");

    smoke_result?;
    quit_result?;
    Ok(())
}

async fn run_app_shell_smoke(driver: &WebDriver, server: &RunningWorkbenchServer) -> Result<()> {
    driver
        .goto(server.url("/"))
        .await
        .context("open workbench")?;
    let probe = wait_for_loaded_probe(driver).await?;

    let title = driver.title().await.context("read page title")?;
    assert_eq!(title, "Brain Brew Deck Workbench");

    let text = probe.text().await.context("read workbench probe text")?;
    assert!(
        text.contains("Brain Brew Deck Workbench loaded 2 language(s)"),
        "unexpected probe text: {text}"
    );
    assert!(
        text.contains("brainbrew.yaml"),
        "manifest name missing from probe text: {text}"
    );
    assert_eq!(probe.attr("data-status").await?.as_deref(), Some("loaded"));
    assert_eq!(
        probe.attr("data-language-count").await?.as_deref(),
        Some("2")
    );
    assert_eq!(probe.attr("data-target-count").await?.as_deref(), Some("2"));

    let wasm_loaded = driver
        .execute("return Boolean(window.wasmBindings);", Vec::new())
        .await
        .context("inspect WASM bindings")?;
    assert_eq!(wasm_loaded.json().as_bool(), Some(true));
    Ok(())
}

async fn wait_for_loaded_probe(driver: &WebDriver) -> Result<WebElement> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match driver
            .find(By::Css("#brainbrew-workbench-e2e[data-status='loaded']"))
            .await
        {
            Ok(element) => return Ok(element),
            Err(error) if Instant::now() < deadline => {
                let _last_error = error;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error).context("wait for workbench metadata probe"),
        }
    }
}

async fn new_driver() -> Result<WebDriver> {
    let webdriver_url =
        std::env::var("WEBDRIVER_URL").unwrap_or_else(|_| "http://127.0.0.1:9515".to_owned());
    let mut caps = DesiredCapabilities::chrome();
    caps.set_headless()?;
    caps.add_arg("--no-sandbox")?;
    caps.add_arg("--disable-dev-shm-usage")?;
    caps.add_arg("--disable-gpu")?;
    caps.add_arg("--window-size=1280,900")?;
    if let Ok(binary) = std::env::var("BRAINBREW_CHROME_BINARY") {
        caps.set_binary(binary.as_str())?;
    }
    WebDriver::new(webdriver_url, caps)
        .await
        .map_err(Into::into)
}

fn dev_assets_path() -> Option<PathBuf> {
    std::env::var_os("BRAINBREW_E2E_DEV_ASSETS").map(PathBuf::from)
}

struct RunningWorkbenchServer {
    child: Child,
    base_url: String,
    _stdout: BufReader<ChildStdout>,
}

impl RunningWorkbenchServer {
    fn spawn(manifest: PathBuf, dev_assets: Option<PathBuf>, artifact_dir: &Path) -> Result<Self> {
        let brainbrew = std::env::var_os("BRAINBREW_E2E_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root().join("target/debug/brainbrew"));
        if !brainbrew.is_file() {
            bail!(
                "brainbrew test binary not found at {}; run `devenv shell e2e`",
                brainbrew.display()
            );
        }

        let stderr = fs::File::create(artifact_dir.join("brainbrew-server.stderr.log"))
            .context("create server stderr artifact")?;
        let mut command = Command::new(brainbrew);
        command.args([
            "workbench",
            "serve",
            "--manifest",
            manifest
                .to_str()
                .ok_or_else(|| anyhow!("manifest path is UTF-8"))?,
            "--port",
            "0",
            "--no-open",
        ]);
        if let Some(dev_assets) = dev_assets {
            command.arg("--dev-assets").arg(dev_assets);
        }
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .context("spawn workbench server")?;
        let stdout = child.stdout.take().context("server stdout is piped")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("read server listen URL")?;
        fs::write(
            artifact_dir.join("brainbrew-server.stdout.log"),
            line.as_bytes(),
        )
        .context("write server stdout artifact")?;
        let Some(base_url) = line.strip_prefix("Workbench listening at ") else {
            bail!("unexpected workbench server line: {line:?}");
        };
        Ok(Self {
            child,
            base_url: base_url.trim().to_owned(),
            _stdout: reader,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for RunningWorkbenchServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct ArtifactDir {
    path: PathBuf,
}

impl ArtifactDir {
    fn new(test_name: &str) -> Result<Self> {
        let root = std::env::var_os("BRAINBREW_E2E_ARTIFACT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root().join("target/workbench-e2e-artifacts"));
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = root.join(format!("{test_name}-{timestamp}"));
        fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    async fn save_browser_failure(&self, driver: &WebDriver, error: &anyhow::Error) -> Result<()> {
        fs::write(self.path.join("failure.txt"), format!("{error:#}"))?;
        if let Ok(png) = driver.screenshot_as_png().await {
            fs::write(self.path.join("failure.png"), png)?;
        }
        if let Ok(source) = driver.source().await {
            fs::write(self.path.join("page.html"), source)?;
        }
        Ok(())
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/brain-brew-workbench-e2e")
        .to_path_buf()
}

fn write_small_workbench_fixture(dir: &Path) -> Result<()> {
    fs::write(dir.join("deck.yaml"), SAMPLE_CANONICAL_YAML)?;
    fs::write(
        dir.join("da.yaml"),
        r#"id: overlay.translation.da
kind: translation
translations:
  direct:
    Finland: Finland
"#,
    )?;
    fs::write(
        dir.join("brainbrew.yaml"),
        r#"base: deck.yaml
overlays:
  overlay.translation.da:
    file: da.yaml
    kind: translation
targets:
  da-standard:
    overlays:
      - overlay.translation.da
  en-standard:
    overlays: []
languages:
  da:
    display_name: Danish
    translation_overlays:
      base: overlay.translation.da
    primary_target: standard
    targets:
      standard: da-standard
  en:
    display_name: English
    source: true
    primary_target: standard
    targets:
      standard: en-standard
translation_profile:
  structural_fields:
    - field.flag
  optional_paths:
    - deck.*
"#,
    )?;
    Ok(())
}

const SAMPLE_CANONICAL_YAML: &str = r#"deck:
  id: deck.workbench-smoke
  name: Workbench Smoke
  description: A small E2E deck fixture.
  adapter_ids:
    crowdanki:uuid: 43c5ba66-9a65-11e8-90c9-a0481cc15658
note_types:
  note-type.country:
    name: Country
    field_order:
      - field.country
      - field.capital
      - field.flag
    fields:
      field.capital:
        name: Capital
      field.country:
        name: Country
      field.flag:
        name: Flag
    card_template_order:
      - template.country-capital
    card_templates:
      template.country-capital:
        name: Country - Capital
        question_format: '{{Country}}'
        answer_format: '{{FrontSide}}<hr id=answer>{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
    adapter_ids:
      crowdanki:uuid: aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa
notes:
  note.finland:
    note_type_id: note-type.country
    fields:
      field.capital: Helsinki
      field.country: Finland
      field.flag: '<img src="fi.png">'
    tags:
      - Europe
      - Nordic
    adapter_ids:
      crowdanki:guid: smoke-finland-guid
media:
  media.flags-fi-png:
    path: flags/fi.png
    sha256: ''
tombstones: []
"#;
