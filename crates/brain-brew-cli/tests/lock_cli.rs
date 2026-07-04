use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn lock_verify_rehashes_live_path_source_even_when_cache_is_warm() {
    let root = temp_dir("lock-drift-warm-cache");
    let package = root.join("package");
    let consumer = root.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "original source text");
    let lock_path = consumer.join("brainbrew.lock");
    let cache = root.join("cache");

    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            package.to_str().unwrap(),
        ],
        &cache,
    );
    assert!(update.status.success(), "stderr: {}", stderr(&update));

    fs::write(package.join("source.txt"), "mutated source text").unwrap();
    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );

    assert!(!verify.status.success(), "verify unexpectedly succeeded");
    assert!(stderr(&verify).contains("nar_hash mismatch"));
}

#[test]
fn path_locks_are_lock_relative_and_verify_after_relocating_pair() {
    let root = temp_dir("lock-relative-relocate");
    let original = root.join("original");
    let package = original.join("package");
    let consumer = original.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "portable source text");
    let lock_path = consumer.join("brainbrew.lock");
    let cache = root.join("cache");

    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            package.to_str().unwrap(),
        ],
        &cache,
    );
    assert!(update.status.success(), "stderr: {}", stderr(&update));

    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("path: ../package"), "{lock_source}");
    assert!(
        !lock_source.contains(&root.display().to_string()),
        "{lock_source}"
    );
    assert!(!lock_source.contains(&package.canonicalize().unwrap().display().to_string()));

    let relocated = root.join("relocated");
    fs::rename(&original, &relocated).unwrap();
    let relocated_lock = relocated.join("consumer").join("brainbrew.lock");
    let verify = run_with_cache(
        ["lock", "verify", "--lock", relocated_lock.to_str().unwrap()],
        &cache,
    );

    assert!(verify.status.success(), "stderr: {}", stderr(&verify));
    assert!(stdout(&verify).contains("verified 1 locked package"));
}

#[test]
fn lock_update_with_default_lock_path_writes_relative_path() {
    let root = temp_dir("lock-default-path");
    let package = root.join("package");
    let consumer = root.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "default lock path source");
    let cache = root.join("cache");

    let update = run_with_cache_current_dir(
        [
            "lock",
            "update",
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            "../package",
        ],
        &cache,
        &consumer,
    );
    assert!(update.status.success(), "stderr: {}", stderr(&update));

    let lock_path = consumer.join("brainbrew.lock");
    let lock_source = fs::read_to_string(&lock_path).unwrap();
    assert!(lock_source.contains("path: ../package"), "{lock_source}");
    assert!(
        !lock_source.contains(&root.display().to_string()),
        "{lock_source}"
    );
    assert!(!lock_source.contains(&package.canonicalize().unwrap().display().to_string()));
}

fn run_with_cache<const N: usize>(args: [&str; N], cache: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("command runs")
}

fn run_with_cache_current_dir<const N: usize>(
    args: [&str; N],
    cache: &Path,
    current_dir: &Path,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_brainbrew"))
        .args(args)
        .current_dir(current_dir)
        .env("BRAINBREW_CACHE_DIR", cache)
        .output()
        .expect("command runs")
}

fn write_package(dir: &Path, version: &str, source_text: &str) {
    fs::write(
        dir.join("brainbrew.yaml"),
        format!(
            r#"package:
  id: anki-geo.ultimate-geography
  version: {version}
base: deck.yaml
overlays: {{}}
targets: {{}}
"#
        ),
    )
    .unwrap();
    fs::write(dir.join("deck.yaml"), "id: deck\nname: Test Deck\n").unwrap();
    fs::write(dir.join("source.txt"), source_text).unwrap();
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{unique}"));
    fs::create_dir_all(&path).unwrap();
    path
}
