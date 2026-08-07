use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

#[test]
fn manifest_base_rejects_portable_unsafe_forms_with_typed_context() {
    for raw in [
        "../outside.yaml",
        "./deck.yaml",
        "/etc/passwd",
        "C:/Windows/win.ini",
        r"\\server\share\deck.yaml",
        r"overlays\deck.yaml",
    ] {
        let workspace = TempDir::new().unwrap();
        write_manifest(workspace.path(), raw, None);
        let output = brainbrew(
            workspace.path(),
            &[
                "validate",
                "--manifest",
                "brainbrew.yaml",
                "--target",
                "standard",
            ],
        );
        assert!(!output.status.success(), "{raw:?} unexpectedly succeeded");
        let error = stderr(&output);
        assert!(
            error.contains("brainbrew.yaml:base path"),
            "{raw:?}: {error}"
        );
        assert!(error.contains(&format!("{raw:?}")), "{raw:?}: {error}");
        assert!(error.contains("package root"), "{raw:?}: {error}");
    }
}

#[test]
fn manifest_overlay_file_rejects_parent_path_before_read() {
    let workspace = TempDir::new().unwrap();
    fs::copy(
        fixture("fixtures/ug-style/deck.yaml"),
        workspace.path().join("deck.yaml"),
    )
    .unwrap();
    write_manifest(workspace.path(), "deck.yaml", Some("../outside.yaml"));
    let output = brainbrew(
        workspace.path(),
        &[
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("brainbrew.yaml:overlays.overlay.test.file path"),
        "{error}"
    );
    assert!(error.contains("\"../outside.yaml\""), "{error}");
}

#[cfg(unix)]
#[test]
fn manifest_base_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::copy(
        fixture("fixtures/ug-style/deck.yaml"),
        outside.path().join("deck.yaml"),
    )
    .unwrap();
    symlink(outside.path(), workspace.path().join("escape")).unwrap();
    write_manifest(workspace.path(), "escape/deck.yaml", None);

    let output = brainbrew(
        workspace.path(),
        &[
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("resolved path"), "{error}");
    assert!(error.contains("escapes the selected root"), "{error}");
}

#[test]
fn scalar_include_rejects_parent_path_with_schema_context() {
    let workspace = TempDir::new().unwrap();
    fs::write(
        workspace.path().join("deck.yaml"),
        "deck:\n  id: deck.safe-path-test\n  name: Safe path test\n  description: !include ../secret.txt\nnote_types: {}\nnotes: {}\nmedia: {}\n",
    )
    .unwrap();
    write_manifest(workspace.path(), "deck.yaml", None);

    let output = brainbrew(
        workspace.path(),
        &[
            "validate",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
        ],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("deck.yaml:deck.description path"), "{error}");
    assert!(error.contains("\"../secret.txt\""), "{error}");
    assert!(error.contains("parent-directory"), "{error}");
}

#[test]
fn media_hash_rejects_unsafe_declaration_before_asset_read() {
    let workspace = TempDir::new().unwrap();
    let deck = fs::read_to_string(fixture("fixtures/ug-style/deck.yaml"))
        .unwrap()
        .replacen("path: flags/at.png", "path: ../outside.png", 1);
    fs::write(workspace.path().join("deck.yaml"), deck).unwrap();
    write_manifest(workspace.path(), "deck.yaml", None);
    fs::create_dir(workspace.path().join("assets")).unwrap();

    let output = brainbrew(
        workspace.path(),
        &[
            "media",
            "hash",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
            "--media-root",
            "assets",
        ],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("media.media.flags-at-png.path"), "{error}");
    assert!(error.contains("\"../outside.png\""), "{error}");
    assert!(error.contains("parent-directory"), "{error}");
}

#[cfg(unix)]
#[test]
fn media_hash_rejects_asset_under_escaping_symlink_parent() {
    use std::os::unix::fs::symlink;

    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("new.png"), "outside").unwrap();
    let deck = fs::read_to_string(fixture("fixtures/ug-style/deck.yaml"))
        .unwrap()
        .replacen("path: flags/at.png", "path: escape/new.png", 1);
    fs::write(workspace.path().join("deck.yaml"), deck).unwrap();
    write_manifest(workspace.path(), "deck.yaml", None);
    let assets = workspace.path().join("assets");
    fs::create_dir(&assets).unwrap();
    symlink(outside.path(), assets.join("escape")).unwrap();

    let output = brainbrew(
        workspace.path(),
        &[
            "media",
            "hash",
            "--manifest",
            "brainbrew.yaml",
            "--target",
            "standard",
            "--media-root",
            "assets",
        ],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("escapes the selected root"), "{error}");
    assert_eq!(
        fs::read(outside.path().join("new.png")).unwrap(),
        b"outside"
    );
}

#[test]
fn lock_package_manifest_is_confined_to_fetched_source() {
    let workspace = TempDir::new().unwrap();
    let source = workspace.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("payload"), "hashed source").unwrap();
    fs::write(workspace.path().join("outside.yaml"), "outside").unwrap();
    let cache = workspace.path().join("cache");

    let output = Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .current_dir(workspace.path())
        .env("BRAINBREW_CACHE_DIR", cache)
        .args([
            "lock",
            "update",
            "--package",
            "pkg.test",
            "--path",
            "source",
            "--package-manifest",
            "../outside.yaml",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("brainbrew.lock:packages.<updated>.manifest path"),
        "{error}"
    );
    assert!(error.contains("fetched package root"), "{error}");
}

fn write_manifest(root: &Path, base: &str, overlay: Option<&str>) {
    let overlays = overlay.map_or_else(
        || "overlays: {}\n".to_owned(),
        |path| {
            format!(
                "overlays:\n  overlay.test:\n    file: '{}'\n    kind: patch\n",
                path.replace('\'', "''")
            )
        },
    );
    let selected = if overlay.is_some() {
        "\n      - overlay.test"
    } else {
        " []"
    };
    fs::write(
        root.join("brainbrew.yaml"),
        format!(
            "base: '{}'\n{overlays}targets:\n  standard:\n    overlays:{selected}\n",
            base.replace('\'', "''")
        ),
    )
    .unwrap();
}

fn brainbrew(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn fixture(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}
