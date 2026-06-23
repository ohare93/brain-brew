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

#[tokio::test]
async fn workbench_edits_target_translation_persists_refresh_and_applies_yaml() -> Result<()> {
    let artifacts = ArtifactDir::new("edit-apply")?;
    let workspace = TempDir::new().context("create E2E workspace")?;
    write_small_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let edit_result = run_edit_apply_smoke(&driver, &server, workspace.path()).await;
    if let Err(error) = &edit_result {
        let _ = artifacts.save_browser_failure(&driver, error).await;
    }
    let quit_result = driver.quit().await.context("quit browser session");

    edit_result?;
    quit_result?;
    Ok(())
}

#[tokio::test]
async fn workbench_source_string_pivot_stages_direct_translation() -> Result<()> {
    let artifacts = ArtifactDir::new("source-string")?;
    let workspace = TempDir::new().context("create source-string E2E workspace")?;
    write_small_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let edit_result = run_source_string_direct_smoke(&driver, &server, workspace.path()).await;
    if let Err(error) = &edit_result {
        let _ = artifacts.save_browser_failure(&driver, error).await;
    }
    let quit_result = driver.quit().await.context("quit browser session");

    edit_result?;
    quit_result?;
    Ok(())
}

#[tokio::test]
async fn workbench_edits_source_field_persists_refresh_and_creates_stale_record() -> Result<()> {
    let artifacts = ArtifactDir::new("source-edit")?;
    let workspace = TempDir::new().context("create source-edit E2E workspace")?;
    write_source_edit_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let edit_result = run_source_edit_smoke(&driver, &server, workspace.path()).await;
    if let Err(error) = &edit_result {
        let _ = artifacts.save_browser_failure(&driver, error).await;
    }
    let quit_result = driver.quit().await.context("quit browser session");

    edit_result?;
    quit_result?;
    Ok(())
}

#[tokio::test]
async fn workbench_mixed_source_and_target_browser_apply_uses_new_source() -> Result<()> {
    let artifacts = ArtifactDir::new("mixed-source-target")?;
    let workspace = TempDir::new().context("create mixed source/target E2E workspace")?;
    write_source_edit_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let edit_result = run_mixed_source_target_smoke(&driver, &server, workspace.path()).await;
    if let Err(error) = &edit_result {
        let _ = artifacts.save_browser_failure(&driver, error).await;
    }
    let quit_result = driver.quit().await.context("quit browser session");

    edit_result?;
    quit_result?;
    Ok(())
}

#[tokio::test]
async fn workbench_loads_ug_like_repeated_source_smoke_path() -> Result<()> {
    let artifacts = ArtifactDir::new("ug-like")?;
    let workspace = TempDir::new().context("create UG-like E2E workspace")?;
    write_ug_like_workbench_fixture(workspace.path())?;

    let server = RunningWorkbenchServer::spawn(
        workspace.path().join("brainbrew.yaml"),
        dev_assets_path(),
        artifacts.path(),
    )?;
    let driver = new_driver().await.context("connect to WebDriver")?;

    let smoke_result = run_ug_like_smoke(&driver, &server).await;
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
    wait_for_element(
        driver,
        "#translation-input-notes_note_finland_fields_field_capital",
    )
    .await
    .context("note pivot field input appears")?;
    wait_for_element(driver, ".pivot-filters button[data-filter='stale']")
        .await?
        .click()
        .await
        .context("click stale filter")?;
    wait_for_text(driver, "No notes match the active filter.").await?;
    wait_for_element(driver, ".pivot-filters button[data-filter='all']")
        .await?
        .click()
        .await
        .context("return to all filter")?;
    wait_for_element(
        driver,
        "#translation-input-notes_note_finland_fields_field_capital",
    )
    .await
    .context("note pivot field input returns after filter reset")?;
    Ok(())
}

async fn run_edit_apply_smoke(
    driver: &WebDriver,
    server: &RunningWorkbenchServer,
    workspace: &Path,
) -> Result<()> {
    run_app_shell_smoke(driver, server).await?;
    let input_id = "translation-input-notes_note_finland_fields_field_capital";
    let input = wait_for_element(driver, &format!("#{input_id}")).await?;
    input
        .clear()
        .await
        .context("clear target translation input")?;
    input
        .send_keys("Helsingfors")
        .await
        .context("type target translation")?;
    wait_for_text(driver, "Helsingfors").await?;
    wait_for_text(driver, "Progress: 2 / 2 complete").await?;

    driver.refresh().await.context("refresh browser")?;
    wait_for_loaded_probe(driver).await?;
    wait_for_element(driver, &format!("#{input_id}"))
        .await
        .context("input returns after refresh")?;
    assert_eq!(element_value(driver, input_id).await?, "Helsingfors");

    wait_for_element(driver, "#apply-preview-button")
        .await?
        .click()
        .await
        .context("click apply preview")?;
    wait_for_apply_output(driver, "Apply preview").await?;
    wait_for_apply_output(driver, "da.yaml").await?;
    wait_for_apply_output(driver, "Helsinki -> Helsingfors").await?;
    let preview_ok = wait_for_element(driver, "#apply-preview-output")
        .await?
        .attr("data-validation-ok")
        .await?;
    assert_eq!(preview_ok.as_deref(), Some("true"));
    assert!(!fs::read_to_string(workspace.join("da.yaml"))?.contains("Helsingfors"));

    wait_for_element(driver, "#apply-confirm-button")
        .await?
        .click()
        .await
        .context("click confirm apply")?;
    wait_for_apply_output(driver, "Applied").await?;
    assert!(fs::read_to_string(workspace.join("da.yaml"))?.contains("Helsinki: Helsingfors"));
    Ok(())
}

async fn run_source_string_direct_smoke(
    driver: &WebDriver,
    server: &RunningWorkbenchServer,
    workspace: &Path,
) -> Result<()> {
    driver
        .goto(server.url("/"))
        .await
        .context("open source-string workbench")?;
    wait_for_loaded_probe(driver).await?;
    wait_for_element(driver, "#source-string-pivot-panel").await?;
    driver
        .execute(
            "document.querySelector(\".source-string-row[data-source='Helsinki']\").click();",
            Vec::new(),
        )
        .await
        .context("select Helsinki source string")?;
    wait_for_text(driver, "Stage direct translation for 1 occurrence(s)").await?;
    let input = wait_for_element(driver, "#source-string-direct-input").await?;
    input
        .clear()
        .await
        .context("clear source string direct input")?;
    input
        .send_keys("Helsingfors via strings")
        .await
        .context("type source string direct translation")?;
    wait_for_text(driver, "Helsingfors via strings").await?;

    wait_for_element(driver, "#apply-preview-button")
        .await?
        .click()
        .await
        .context("preview source string edit")?;
    wait_for_apply_output(driver, "Apply preview").await?;
    wait_for_apply_output(driver, "Helsinki -> Helsingfors via strings").await?;
    assert!(!fs::read_to_string(workspace.join("da.yaml"))?.contains("via strings"));

    wait_for_element(driver, "#apply-confirm-button")
        .await?
        .click()
        .await
        .context("apply source string edit")?;
    wait_for_apply_output(driver, "Applied").await?;
    let overlay = fs::read_to_string(workspace.join("da.yaml"))?;
    assert!(overlay.contains("Helsinki: Helsingfors via strings"));
    Ok(())
}

async fn run_source_edit_smoke(
    driver: &WebDriver,
    server: &RunningWorkbenchServer,
    workspace: &Path,
) -> Result<()> {
    driver
        .goto(server.url("/"))
        .await
        .context("open source-edit workbench")?;
    wait_for_loaded_probe(driver).await?;
    let source_id = "source-input-notes_note_georgia_country_fields_field_country";
    let toggle_id = "source-edit-toggle-notes_note_georgia_country_fields_field_country";
    wait_for_text(driver, "2 occurrence(s)").await?;
    wait_for_element(driver, &format!("#{toggle_id}"))
        .await?
        .click()
        .await
        .context("toggle source edit")?;
    let input = wait_for_element(driver, &format!("#{source_id}")).await?;
    input.clear().await.context("clear source input")?;
    input
        .send_keys("Sakartvelo")
        .await
        .context("type source edit")?;
    wait_for_text(driver, "staged_source").await?;
    wait_for_text(driver, "Sakartvelo").await?;
    let source_preview = driver
        .execute(
            "return document.querySelector(\".note-card[data-note-id='note.georgia-country'] .preview-grid section:first-child [data-preview-field-id='field.country']\").innerHTML;",
            Vec::new(),
        )
        .await
        .context("read live source preview")?;
    assert_eq!(source_preview.json().as_str(), Some("Sakartvelo"));

    driver.refresh().await.context("refresh browser")?;
    wait_for_loaded_probe(driver).await?;
    wait_for_element(driver, &format!("#{source_id}"))
        .await
        .context("source input returns after refresh")?;
    assert_eq!(element_value(driver, source_id).await?, "Sakartvelo");

    wait_for_element(driver, "#apply-preview-button")
        .await?
        .click()
        .await
        .context("click source apply preview")?;
    wait_for_apply_output(driver, "Apply preview").await?;
    wait_for_apply_output(driver, "deck.yaml").await?;
    wait_for_apply_output(driver, "da.yaml").await?;
    wait_for_apply_output(driver, "Georgia -> Sakartvelo").await?;
    assert!(!fs::read_to_string(workspace.join("deck.yaml"))?.contains("Sakartvelo"));
    assert!(!fs::read_to_string(workspace.join("da.yaml"))?.contains("stale_records"));

    wait_for_element(driver, "#apply-confirm-button")
        .await?
        .click()
        .await
        .context("confirm source apply")?;
    wait_for_apply_output(driver, "Applied").await?;
    let deck = fs::read_to_string(workspace.join("deck.yaml"))?;
    assert!(deck.contains("field.country: Sakartvelo"));
    assert!(deck.contains("field.country: Georgia"));
    let overlay = fs::read_to_string(workspace.join("da.yaml"))?;
    assert!(overlay.contains("stale_records:"));
    assert!(overlay.contains("old_source: Georgia"));
    assert!(overlay.contains("new_source: Sakartvelo"));
    assert!(overlay.contains("context: notes.note.georgia-country"));

    wait_for_element(driver, ".pivot-filters button[data-filter='stale']").await?;
    driver
        .execute(
            "document.querySelector(\".pivot-filters button[data-filter='stale']\").click();",
            Vec::new(),
        )
        .await
        .context("filter stale source edit result")?;
    wait_for_text(driver, "Sakartvelo").await?;
    Ok(())
}

async fn run_mixed_source_target_smoke(
    driver: &WebDriver,
    server: &RunningWorkbenchServer,
    workspace: &Path,
) -> Result<()> {
    driver
        .goto(server.url("/"))
        .await
        .context("open mixed source/target workbench")?;
    wait_for_loaded_probe(driver).await?;
    let suffix = "notes_note_georgia_country_fields_field_country";
    wait_for_element(driver, &format!("#source-edit-toggle-{suffix}"))
        .await?
        .click()
        .await
        .context("toggle source edit")?;
    let source = wait_for_element(driver, &format!("#source-input-{suffix}")).await?;
    source.clear().await.context("clear source input")?;
    source
        .send_keys("Sakartvelo")
        .await
        .context("type source edit")?;
    let target = wait_for_element(driver, &format!("#translation-input-{suffix}")).await?;
    target.clear().await.context("clear target input")?;
    target
        .send_keys("Sakartvelo på dansk")
        .await
        .context("type target edit after source edit")?;

    wait_for_element(driver, "#apply-confirm-button")
        .await?
        .click()
        .await
        .context("confirm mixed apply")?;
    wait_for_apply_output(driver, "Applied").await?;
    let deck = fs::read_to_string(workspace.join("deck.yaml"))?;
    assert!(deck.contains("field.country: Sakartvelo"));
    let overlay = fs::read_to_string(workspace.join("da.yaml"))?;
    assert!(overlay.contains("Sakartvelo:"));
    assert!(overlay.contains("Sakartvelo på dansk"));
    assert!(!overlay.contains("stale_records"));
    Ok(())
}

async fn run_ug_like_smoke(driver: &WebDriver, server: &RunningWorkbenchServer) -> Result<()> {
    driver
        .goto(server.url("/"))
        .await
        .context("open UG-like workbench")?;
    wait_for_loaded_probe(driver).await?;
    wait_for_element(driver, "#workbench-dom-panel").await?;
    wait_for_text(driver, "2 occurrence(s)").await?;
    wait_for_text(driver, "Georgia").await?;
    Ok(())
}

async fn wait_for_loaded_probe(driver: &WebDriver) -> Result<WebElement> {
    wait_for_element(driver, "#brainbrew-workbench-e2e[data-status='loaded']")
        .await
        .context("wait for workbench metadata probe")
}

async fn wait_for_element(driver: &WebDriver, css: &str) -> Result<WebElement> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match driver.find(By::Css(css)).await {
            Ok(element) => return Ok(element),
            Err(error) if Instant::now() < deadline => {
                let _last_error = error;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error).with_context(|| format!("wait for element {css}")),
        }
    }
}

async fn wait_for_text(driver: &WebDriver, expected: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let body = wait_for_element(driver, "body").await?;
        let text = body.text().await.unwrap_or_default();
        if text.contains(expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for text {expected:?}; body was {text:?}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_apply_output(driver: &WebDriver, expected: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let output = wait_for_element(driver, "#apply-preview-output").await?;
        let text = output.text().await.unwrap_or_default();
        if text.contains(expected) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for apply output {expected:?}; output was {text:?}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn element_value(driver: &WebDriver, id: &str) -> Result<String> {
    let script = format!("return document.getElementById({id:?}).value;");
    let value = driver
        .execute(script.as_str(), Vec::new())
        .await
        .context("read element value")?;
    Ok(value.json().as_str().unwrap_or_default().to_owned())
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
    write_translation_overlay_and_manifest(dir)
}

fn write_ug_like_workbench_fixture(dir: &Path) -> Result<()> {
    fs::write(dir.join("deck.yaml"), UG_LIKE_CANONICAL_YAML)?;
    write_translation_overlay_and_manifest(dir)
}

fn write_source_edit_workbench_fixture(dir: &Path) -> Result<()> {
    fs::write(dir.join("deck.yaml"), UG_LIKE_CANONICAL_YAML)?;
    write_translation_overlay_and_manifest_with_direct_entries(
        dir,
        r#"    Georgia: Georgien
    Finland: Finland
"#,
    )
}

fn write_translation_overlay_and_manifest(dir: &Path) -> Result<()> {
    write_translation_overlay_and_manifest_with_direct_entries(
        dir,
        r#"    Finland: Finland
"#,
    )
}

fn write_translation_overlay_and_manifest_with_direct_entries(
    dir: &Path,
    direct_entries: &str,
) -> Result<()> {
    fs::write(
        dir.join("da.yaml"),
        format!(
            "id: overlay.translation.da\nkind: translation\ntranslations:\n  direct:\n{direct_entries}"
        ),
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

const UG_LIKE_CANONICAL_YAML: &str = r#"deck:
  id: deck.ug-like-workbench-smoke
  name: UG-like Workbench Smoke
  description: A reduced Ultimate Geography-style E2E deck fixture.
  adapter_ids:
    crowdanki:uuid: 53c5ba66-9a65-11e8-90c9-a0481cc15658
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
        answer_format: '{{FrontSide}}<hr id=answer>{{#Flag}}<div class="flag">{{Flag}}</div>{{/Flag}}{{Capital}}'
        adapter_ids: {}
    styling: |
      .card { font-family: sans-serif; }
      .flag { opacity: 0.8; }
    adapter_ids:
      crowdanki:uuid: bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb
notes:
  note.georgia-country:
    note_type_id: note-type.country
    fields:
      field.capital: Tbilisi
      field.country: Georgia
      field.flag: '<img src="flags/ge-country.png">'
    tags:
      - Asia
    adapter_ids:
      crowdanki:guid: smoke-georgia-country-guid
  note.georgia-state:
    note_type_id: note-type.country
    fields:
      field.capital: Atlanta
      field.country: Georgia
      field.flag: '<img src="flags/ge-state.png">'
    tags:
      - NorthAmerica
    adapter_ids:
      crowdanki:guid: smoke-georgia-state-guid
media:
  media.flags-ge-country-png:
    path: flags/ge-country.png
    sha256: ''
  media.flags-ge-state-png:
    path: flags/ge-state.png
    sha256: ''
tombstones: []
"#;
