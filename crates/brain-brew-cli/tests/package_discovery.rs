use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .output()
        .expect("brainbrew runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_package(root: &Path, id: &str, target: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("deck.yaml"),
        format!(
            "deck:\n  id: deck.{id}\n  name: {id}\n  description: fixture\nnote_types: {{}}\nnotes: {{}}\nmedia: {{}}\ntombstones: []\n"
        ),
    )
    .unwrap();
    fs::write(
        root.join("brainbrew.yaml"),
        format!(
            "package:\n  id: {id}\n  version: 1.0.0\nbase: deck.yaml\noverlays: {{}}\ntargets:\n  {target}:\n    overlays: []\n"
        ),
    )
    .unwrap();
}

#[test]
fn package_discovery_prunes_builtins_and_safe_configured_patterns_deterministically() {
    let temp = tempfile::tempdir().unwrap();
    write_package(&temp.path().join("packages/a"), "example.a", "alpha");
    write_package(&temp.path().join("packages/z"), "example.z", "zulu");

    for ignored in [
        ".git",
        ".jj",
        ".devenv",
        "target",
        "build",
        "output",
        ".cache",
        ".brainbrew-transactions",
        "node_modules",
        "documentation/build",
        "documentation/.docusaurus",
        "site",
    ] {
        let path = temp.path().join(ignored);
        write_package(&path.join("bogus"), "example.bogus", "bogus");
    }
    write_package(
        &temp.path().join("vendor/generated/ignored"),
        "example.ignored",
        "ignored",
    );
    write_package(
        &temp.path().join("vendor/legitimate-generated-name"),
        "example.legitimate",
        "legitimate",
    );

    let root = temp.path().to_str().unwrap();
    let args = [
        "targets",
        "--package-root",
        root,
        "--package-ignore",
        "vendor/generated/**",
    ];
    let first = run(&args);
    let second = run(&args);
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(second.status.success(), "{}", stderr(&second));
    assert_eq!(first.stdout, second.stdout);
    let stdout = String::from_utf8(first.stdout).unwrap();
    assert!(stdout.contains("example.a:alpha"), "{stdout}");
    assert!(stdout.contains("example.z:zulu"), "{stdout}");
    assert!(stdout.contains("example.legitimate:legitimate"), "{stdout}");
    assert!(!stdout.contains("bogus"), "{stdout}");
    assert!(!stdout.contains("ignored:ignored"), "{stdout}");
}

#[cfg(unix)]
#[test]
fn package_discovery_rejects_symlinks_without_following_cycles() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    write_package(&temp.path().join("package"), "example.package", "base");
    symlink("..", temp.path().join("package/loop")).unwrap();

    let output = run(&["targets", "--package-root", temp.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("package discovery rejected symlink"),
        "{error}"
    );
    assert!(error.contains("package/loop"), "{error}");

    let alias = temp.path().join("alias");
    symlink(temp.path().join("package"), &alias).unwrap();
    let aliased_root = alias.join("nested-root");
    let output = run(&["targets", "--package-root", aliased_root.to_str().unwrap()]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("package discovery rejected symlink"),
        "{error}"
    );
    assert!(error.contains("/alias"), "{error}");
}

#[test]
fn package_discovery_depth_entry_and_manifest_budgets_are_actionable() {
    let temp = tempfile::tempdir().unwrap();
    write_package(
        &temp.path().join("one/two/three/package"),
        "example.deep",
        "base",
    );
    let root = temp.path().to_str().unwrap();

    for (flag, value, budget) in [
        ("--discovery-max-depth", "2", "depth"),
        ("--discovery-max-entries", "2", "entries"),
        ("--discovery-max-manifests", "1", "manifests"),
    ] {
        if budget == "manifests" {
            write_package(&temp.path().join("other"), "example.other", "other");
        }
        let output = run(&["targets", "--package-root", root, flag, value]);
        assert!(!output.status.success(), "{flag} unexpectedly succeeded");
        let error = stderr(&output);
        assert!(
            error.contains("package discovery budget exceeded"),
            "{error}"
        );
        assert!(error.contains(&format!("budget={budget}")), "{error}");
        assert!(error.contains("consumed="), "{error}");
        assert!(error.contains("limit="), "{error}");
        assert!(error.contains(flag), "{error}");
        assert!(error.contains(root), "{error}");
    }
}

#[test]
fn package_discovery_override_and_ignore_validation_rejects_zero_overflow_and_unsafe_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_str().unwrap();
    for (flag, value, expected) in [
        ("--discovery-max-depth", "0", "greater than zero"),
        (
            "--discovery-max-entries",
            "99999999999999999999999999999999999999",
            "positive decimal integer",
        ),
        (
            "--discovery-max-manifests",
            "unbounded",
            "positive decimal integer",
        ),
        (
            "--package-ignore",
            "../outside",
            "safe package-root-relative",
        ),
        (
            "--package-ignore",
            "/absolute",
            "safe package-root-relative",
        ),
    ] {
        let output = run(&["targets", "--package-root", root, flag, value]);
        assert!(
            !output.status.success(),
            "{flag} {value} unexpectedly succeeded"
        );
        assert!(stderr(&output).contains(expected), "{}", stderr(&output));
    }

    let duplicate = run(&[
        "targets",
        "--package-root",
        root,
        "--discovery-max-depth",
        "4",
        "--discovery-max-depth",
        "5",
    ]);
    assert!(!duplicate.status.success());
    assert!(
        stderr(&duplicate).contains("duplicate argument \"--discovery-max-depth\""),
        "{}",
        stderr(&duplicate)
    );
}

#[test]
fn every_package_root_command_route_uses_the_registry_discovery_policy() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("package");
    write_package(&package, "example.package", "base");
    let manifest = package.join("brainbrew.yaml");
    let root = temp.path().to_str().unwrap();
    let manifest = manifest.to_str().unwrap();
    let export = temp.path().join("export");
    let routes = vec![
        vec!["targets", "--manifest", manifest, "--package-root", root],
        vec![
            "validate",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "compose",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "explain",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "verify",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "translations",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "media",
            "images-to-refs",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
        ],
        vec![
            "export",
            "crowdanki",
            "--manifest",
            manifest,
            "--target",
            "base",
            "--package-root",
            root,
            "--out",
            export.to_str().unwrap(),
        ],
    ];
    for mut route in routes {
        let command = route[0].to_owned();
        route.extend(["--discovery-max-entries", "1"]);
        let output = run(&route);
        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        let error = stderr(&output);
        assert!(
            error.contains("package discovery budget exceeded"),
            "{command}: {error}"
        );
        assert!(
            error.contains("--discovery-max-entries"),
            "{command}: {error}"
        );
    }
}

#[test]
fn package_root_duplicate_manifest_identities_are_still_rejected_by_registry() {
    let temp = tempfile::tempdir().unwrap();
    write_package(&temp.path().join("a"), "example.same", "first");
    write_package(&temp.path().join("b"), "example.same", "second");
    let output = run(&["targets", "--package-root", temp.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("duplicate package identity"), "{error}");
    assert!(error.contains("/a/brainbrew.yaml"), "{error}");
    assert!(error.contains("/b/brainbrew.yaml"), "{error}");
}

#[cfg(unix)]
#[test]
fn package_discovery_rejects_special_entries_explicitly() {
    use std::os::unix::net::UnixListener;

    let temp = tempfile::tempdir().unwrap();
    write_package(&temp.path().join("package"), "example.package", "base");
    let socket_path = temp.path().join("package/discovery.sock");
    let _listener = UnixListener::bind(&socket_path).unwrap();

    let output = run(&["targets", "--package-root", temp.path().to_str().unwrap()]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("unsupported socket"), "{error}");
    assert!(error.contains("discovery.sock"), "{error}");
}
