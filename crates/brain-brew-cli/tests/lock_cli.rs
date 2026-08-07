use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{Builder, EntryType, Header};

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

#[test]
fn lock_verify_rejects_a_manually_removed_hash_before_reading_the_package() {
    let root = temp_dir("lock-removed-hash");
    let package = root.join("package");
    let consumer = root.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "authenticated source");
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

    let weakened = fs::read_to_string(&lock_path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with("nar_hash:"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&lock_path, weakened).unwrap();
    fs::write(package.join("brainbrew.yaml"), "not: a package manifest\n").unwrap();

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );
    let error = stderr(&verify);
    assert!(!verify.status.success());
    assert!(error.contains("nar_hash"), "{error}");
    assert!(!error.contains("no package metadata"), "{error}");
}

#[test]
fn lock_verify_rejects_v1_with_regeneration_guidance() {
    let root = temp_dir("lock-v1-migration");
    let lock_path = root.join("brainbrew.lock");
    fs::write(&lock_path, "version: 1\npackages:\n  old.package:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    locked:\n      type: path\n      path: ../package\n").unwrap();

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &root.join("cache"),
    );
    let error = stderr(&verify);
    assert!(!verify.status.success());
    assert!(error.contains("version 1"), "{error}");
    assert!(error.contains("insecure"), "{error}");
    assert!(error.contains("brainbrew lock update"), "{error}");

    let update = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "old.package",
            "--path",
            root.join("missing-package").to_str().unwrap(),
        ],
        &root.join("cache"),
    );
    let update_error = stderr(&update);
    assert!(!update.status.success());
    assert!(update_error.contains("version 1"), "{update_error}");
    assert!(!update_error.contains("missing-package"), "{update_error}");
}

#[test]
fn lock_verify_rehashes_and_rejects_a_tampered_cache_tree() {
    let root = temp_dir("lock-cache-tamper");
    let package = root.join("package");
    let consumer = root.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "authenticated source");
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

    let cache_entry = fs::read_dir(cache.join("sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(cache_entry.join("source.txt"), "tampered cache bytes").unwrap();

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );
    let error = stderr(&verify);
    assert!(!verify.status.success());
    assert!(error.contains("cached source"), "{error}");
    assert!(error.contains("nar_hash mismatch"), "{error}");
}

#[cfg(unix)]
#[test]
fn lock_update_rejects_symlinks_and_special_files_in_path_snapshots() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    for hostile_entry in ["symlink", "hard link", "socket"] {
        let short_root = tempfile::Builder::new()
            .prefix("bb-lock-")
            .tempdir()
            .unwrap();
        let root = short_root.path().to_path_buf();
        let package = root.join("package");
        fs::create_dir_all(&package).unwrap();
        write_package(&package, "0.1.0", "safe source");
        let outside = root.join("outside-secret");
        fs::write(&outside, "must not be read").unwrap();
        let _socket = match hostile_entry {
            "symlink" => {
                symlink(&outside, package.join("hostile")).unwrap();
                None
            }
            "hard link" => {
                fs::hard_link(&outside, package.join("hostile")).unwrap();
                None
            }
            "socket" => Some(UnixListener::bind(package.join("hostile")).unwrap()),
            _ => unreachable!(),
        };
        let lock_path = root.join("brainbrew.lock");
        let cache = root.join("cache");

        let output = run_with_cache(
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
        let error = stderr(&output);
        assert!(
            !output.status.success(),
            "{hostile_entry} unexpectedly succeeded"
        );
        assert!(error.contains("package tree policy"), "{error}");
        assert!(error.contains("hostile"), "{error}");
        assert!(error.contains(hostile_entry), "{error}");
        assert!(!lock_path.exists());
        assert!(!cache.join("sources").exists());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "must not be read");
    }
}

#[cfg(unix)]
#[test]
fn lock_update_rejects_a_path_source_selected_through_a_symlink() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("lock-path-root-symlink");
    let package = root.join("package");
    fs::create_dir_all(&package).unwrap();
    write_package(&package, "0.1.0", "safe source");
    let linked_package = root.join("linked-package");
    symlink(&package, &linked_package).unwrap();
    let lock_path = root.join("brainbrew.lock");
    let output = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            linked_package.to_str().unwrap(),
        ],
        &root.join("cache"),
    );
    let error = stderr(&output);
    assert!(!output.status.success());
    assert!(error.contains("root"), "{error}");
    assert!(error.contains("symlink"), "{error}");
    assert!(!lock_path.exists());
}

#[cfg(unix)]
#[test]
fn warm_cache_rejects_a_symlink_even_when_its_target_matches_the_locked_bytes() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("lock-cache-symlink");
    let package = root.join("package");
    fs::create_dir_all(&package).unwrap();
    write_package(&package, "0.1.0", "safe source");
    let lock_path = root.join("brainbrew.lock");
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
    let cache_entry = fs::read_dir(cache.join("sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_file(cache_entry.join("source.txt")).unwrap();
    symlink("deck.yaml", cache_entry.join("source.txt")).unwrap();

    let verify = run_with_cache(
        ["lock", "verify", "--lock", lock_path.to_str().unwrap()],
        &cache,
    );
    let error = stderr(&verify);
    assert!(!verify.status.success());
    assert!(error.contains("cached package tree"), "{error}");
    assert!(error.contains("source.txt"), "{error}");
    assert!(error.contains("symlink"), "{error}");
}

#[test]
fn lock_update_rejects_hostile_tar_entry_types_duplicates_and_paths() {
    let cases = [
        (
            "symlink",
            EntryType::Symlink,
            "pkg/brainbrew.yaml",
            Some("/etc/passwd"),
        ),
        (
            "hard link",
            EntryType::Link,
            "pkg/brainbrew.yaml",
            Some("pkg/deck.yaml"),
        ),
        ("fifo", EntryType::Fifo, "pkg/fifo", None),
        ("device", EntryType::Char, "pkg/device", None),
        ("sparse", EntryType::GNUSparse, "pkg/sparse", None),
        ("unknown", EntryType::new(b'Z'), "pkg/unknown", None),
    ];
    for (expected, entry_type, path, link) in cases {
        let root = temp_dir(&format!("lock-tar-{}", expected.replace(' ', "-")));
        let archive = root.join("hostile.tar.gz");
        write_hostile_tar(&archive, &[(path, entry_type, link, b"")]);
        assert_tar_rejected(&root, &archive, expected);
    }

    let root = temp_dir("lock-tar-duplicate");
    let archive = root.join("duplicate.tar.gz");
    write_hostile_tar(
        &archive,
        &[
            ("pkg/duplicate", EntryType::Regular, None, b"first"),
            ("pkg/duplicate", EntryType::Regular, None, b"second"),
        ],
    );
    assert_tar_rejected(&root, &archive, "duplicate");

    let root = temp_dir("lock-tar-gnu-long-parent");
    let archive = root.join("hostile.tar.gz");
    write_hostile_tar(
        &archive,
        &[
            ("LongLink", EntryType::GNULongName, None, b"../outside\0"),
            ("placeholder", EntryType::Regular, None, b"hostile"),
        ],
    );
    assert_tar_rejected(&root, &archive, "archive path");

    let root = temp_dir("lock-tar-pax-parent");
    let archive = root.join("hostile.tar.gz");
    let pax = pax_record("path", "../outside");
    write_hostile_tar_owned(
        &archive,
        vec![
            ("PaxHeader", EntryType::XHeader, None, pax),
            ("placeholder", EntryType::Regular, None, b"hostile".to_vec()),
        ],
    );
    assert_tar_rejected(&root, &archive, "archive path");

    let root = temp_dir("lock-tar-pax-sparse");
    let archive = root.join("hostile.tar.gz");
    let pax = pax_record("GNU.sparse.map", "0,1");
    write_hostile_tar_owned(
        &archive,
        vec![
            ("PaxHeader", EntryType::XHeader, None, pax),
            ("file", EntryType::Regular, None, b"x".to_vec()),
        ],
    );
    assert_tar_rejected(&root, &archive, "sparse");

    for (name, raw_path) in [
        ("parent", "../outside-written"),
        ("absolute", "/tmp/brainbrew-outside-written"),
        ("dot", "pkg/./file"),
        ("backslash", r"pkg\outside"),
        ("drive", "C:/outside"),
        ("unc", "//server/share"),
    ] {
        let root = temp_dir(&format!("lock-tar-{name}"));
        let archive = root.join("hostile.tar.gz");
        write_hostile_tar(
            &archive,
            &[(raw_path, EntryType::Regular, None, b"hostile")],
        );
        assert_tar_rejected(&root, &archive, "archive path");
        assert!(!root.join("outside-written").exists());
    }
}

#[cfg(unix)]
#[test]
fn package_snapshot_permissions_discard_setid_and_preserve_only_executable_intent() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("lock-normalized-permissions");
    let package = root.join("package");
    fs::create_dir_all(package.join("nested")).unwrap();
    write_package(&package, "0.1.0", "executable source");
    fs::write(package.join("nested/plain"), "plain").unwrap();
    // Nix's restricted build sandbox may deny setting set-id bits even though
    // it permits ordinary executable modes. Exercise set-id normalization when
    // the host allows it and preserve the executable-mode assertion otherwise.
    if let Err(error) = fs::set_permissions(
        package.join("source.txt"),
        fs::Permissions::from_mode(0o6755),
    ) {
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        fs::set_permissions(
            package.join("source.txt"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    fs::set_permissions(
        package.join("nested/plain"),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    fs::set_permissions(package.join("nested"), fs::Permissions::from_mode(0o777)).unwrap();
    let cache = root.join("cache");
    let output = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            root.join("brainbrew.lock").to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--path",
            package.to_str().unwrap(),
        ],
        &cache,
    );
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let cached = fs::read_dir(cache.join("sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        fs::metadata(cached.join("source.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    assert_eq!(
        fs::metadata(cached.join("nested/plain"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o644
    );
    assert_eq!(
        fs::metadata(cached.join("nested"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
}

#[test]
fn rejected_archive_cannot_replace_or_poison_a_prior_valid_cache() {
    let root = temp_dir("lock-cache-survives-rejection");
    let package = root.join("package");
    fs::create_dir_all(&package).unwrap();
    write_package(&package, "0.1.0", "valid cached source");
    let lock_path = root.join("brainbrew.lock");
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
    let cache_entry = fs::read_dir(cache.join("sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let before = fs::read(cache_entry.join("source.txt")).unwrap();

    let archive = root.join("hostile.tar.gz");
    write_hostile_tar(
        &archive,
        &[(
            "pkg/brainbrew.yaml",
            EntryType::Symlink,
            Some("/etc/passwd"),
            b"",
        )],
    );
    let rejected = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "anki-geo.ultimate-geography",
            "--tarball",
            &format!("file://{}", archive.display()),
        ],
        &cache,
    );
    assert!(!rejected.status.success());
    assert!(stderr(&rejected).contains("symlink"));
    assert_eq!(fs::read(cache_entry.join("source.txt")).unwrap(), before);
    assert!(fs::read_dir(cache.join("sources")).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".publish-")
    }));
}

#[test]
fn lock_update_output_is_byte_idempotent() {
    let root = temp_dir("lock-idempotent");
    let package = root.join("package");
    let consumer = root.join("consumer");
    fs::create_dir_all(&package).unwrap();
    fs::create_dir_all(&consumer).unwrap();
    write_package(&package, "0.1.0", "authenticated source");
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
    let once = fs::read(&lock_path).unwrap();

    let format = run_with_cache(["fmt", lock_path.to_str().unwrap()], &cache);
    assert!(format.status.success(), "stderr: {}", stderr(&format));
    assert_eq!(fs::read(&lock_path).unwrap(), once);
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

fn write_hostile_tar(path: &Path, entries: &[(&str, EntryType, Option<&str>, &[u8])]) {
    write_hostile_tar_owned(
        path,
        entries
            .iter()
            .map(|(path, entry_type, link, bytes)| (*path, *entry_type, *link, bytes.to_vec()))
            .collect(),
    );
}

fn write_hostile_tar_owned(path: &Path, entries: Vec<(&str, EntryType, Option<&str>, Vec<u8>)>) {
    let output = fs::File::create(path).unwrap();
    let encoder = GzEncoder::new(output, Compression::default());
    let mut builder = Builder::new(encoder);
    for (path, entry_type, link, bytes) in entries {
        let mut header = Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_mode(0o7777);
        header.set_uid(1234);
        header.set_gid(5678);
        header.set_size(bytes.len() as u64);
        set_raw_tar_path(&mut header, path.as_bytes());
        if let Some(link) = link {
            header.set_link_name(link).unwrap();
        }
        header.set_cksum();
        builder.append(&header, bytes.as_slice()).unwrap();
    }
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();
}

fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body = format!(" {key}={value}\n");
    let mut length = body.len() + 1;
    loop {
        let record = format!("{length}{body}");
        if record.len() == length {
            return record.into_bytes();
        }
        length = record.len();
    }
}

fn set_raw_tar_path(header: &mut Header, raw: &[u8]) {
    assert!(raw.len() <= 100);
    let bytes = header.as_mut_bytes();
    bytes[..100].fill(0);
    bytes[..raw.len()].copy_from_slice(raw);
}

fn assert_tar_rejected(root: &Path, archive: &Path, expected: &str) {
    let lock_path = root.join("brainbrew.lock");
    let cache = root.join("cache");
    let output = run_with_cache(
        [
            "lock",
            "update",
            "--lock",
            lock_path.to_str().unwrap(),
            "--package",
            "package.hostile",
            "--tarball",
            &format!("file://{}", archive.display()),
        ],
        &cache,
    );
    let error = stderr(&output);
    assert!(
        !output.status.success(),
        "archive unexpectedly succeeded: {error}"
    );
    assert!(
        error.contains(expected),
        "expected {expected:?} in {error:?}"
    );
    assert!(!lock_path.exists());
    assert!(!cache.join("sources").exists());
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
