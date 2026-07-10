use brain_brew_formats::lockfile::{self, LockedSource, OriginalSource};

const ZERO_SRI: &str = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const REV: &str = "ccf150a1b21e0000000000000000000000000000";

#[test]
fn parses_and_formats_all_typed_lock_sources_canonically() {
    let formatted = lockfile::format_str(&format!(
        r#"
packages:
  package.tarball:
    locked:
      nar_hash: {ZERO_SRI}
      url: https://example.org/package.tar.gz
      type: tarball
    original:
      url: https://example.org/package.tar.gz
      type: tarball
    package:
      version: 3.0.0
    manifest: manifests/package.yaml
  package.path:
    locked:
      nar_hash: {ZERO_SRI}
      path: ../package
      type: path
    original:
      path: ../package
      type: path
    package:
      version: 1.0.0
    manifest: brainbrew.yaml
  package.git:
    locked:
      nar_hash: {ZERO_SRI}
      rev: {REV}
      url: https://github.com/example/package.git
      type: git
    original:
      ref: main
      url: https://github.com/example/package.git
      type: git
    package:
      version: 2.0.0
    manifest: brainbrew.yaml
version: 2
"#,
    ))
    .expect("lock formats");

    assert_eq!(
        formatted,
        format!(
            r#"version: 2
packages:
  package.git:
    manifest: brainbrew.yaml
    package:
      version: 2.0.0
    original:
      type: git
      url: https://github.com/example/package.git
      ref: main
    locked:
      type: git
      url: https://github.com/example/package.git
      rev: {REV}
      nar_hash: '{ZERO_SRI}'
  package.path:
    manifest: brainbrew.yaml
    package:
      version: 1.0.0
    original:
      type: path
      path: ../package
    locked:
      type: path
      path: ../package
      nar_hash: '{ZERO_SRI}'
  package.tarball:
    manifest: manifests/package.yaml
    package:
      version: 3.0.0
    original:
      type: tarball
      url: https://example.org/package.tar.gz
    locked:
      type: tarball
      url: https://example.org/package.tar.gz
      nar_hash: '{ZERO_SRI}'
"#,
        )
    );

    let lock = lockfile::from_str(&formatted).expect("formatted lock parses");
    assert!(matches!(
        lock.packages["package.path"].locked,
        LockedSource::Path { ref path, ref nar_hash }
            if path == "../package" && nar_hash == ZERO_SRI
    ));
    assert!(matches!(
        lock.packages["package.git"].original,
        OriginalSource::Git { ref reference, .. }
            if reference.as_deref() == Some("main")
    ));
    assert_eq!(lockfile::format_str(&formatted).unwrap(), formatted);
}

#[test]
fn rejects_cross_variant_field_smuggling_and_unknown_source_types() {
    let cases = [
        (
            "path with url",
            "type: path\n      path: ../pkg\n      url: https://example.org/pkg",
        ),
        (
            "git with path",
            &format!(
                "type: git\n      url: https://github.com/example/pkg.git\n      rev: {REV}\n      path: ../pkg"
            ),
        ),
        (
            "tarball with rev",
            &format!("type: tarball\n      url: https://example.org/pkg.tar.gz\n      rev: {REV}"),
        ),
        (
            "unknown type",
            "type: registry\n      url: https://example.org/pkg",
        ),
    ];

    for (name, source) in cases {
        let yaml = format!(
            "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n    locked:\n      {source}\n      nar_hash: '{ZERO_SRI}'\n"
        );
        let error = lockfile::from_str(&yaml).expect_err(name);
        let message = error.to_string();
        assert!(
            message.contains("unknown field") || message.contains("unknown variant"),
            "{name}: {message}"
        );
    }

    let original_hash = format!(
        "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n      nar_hash: '{ZERO_SRI}'\n    locked:\n      type: path\n      path: ../pkg\n      nar_hash: '{ZERO_SRI}'\n"
    );
    let error = lockfile::from_str(&original_hash)
        .expect_err("hash is allowed exactly once and only on the locked source");
    assert!(error.to_string().contains("unknown field `nar_hash`"));
}

#[test]
fn requires_source_specific_identity_fields() {
    let cases = [
        ("path", "type: path"),
        ("git url", &format!("type: git\n      rev: {REV}")),
        (
            "git rev",
            "type: git\n      url: https://github.com/example/pkg.git",
        ),
        ("tarball url", "type: tarball"),
    ];

    for (name, source) in cases {
        let yaml = format!(
            "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n    locked:\n      {source}\n      nar_hash: '{ZERO_SRI}'\n"
        );
        assert!(lockfile::from_str(&yaml).is_err(), "{name} was accepted");
    }
}

#[test]
fn rejects_missing_empty_malformed_and_noncanonical_sha256_sri() {
    let malformed = [
        "",
        "sha1-AAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "sha256-",
        "sha256-not-base64",
        "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
        "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n",
        "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA_A=",
        "SHA256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
    ];

    let missing = "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n    locked:\n      type: path\n      path: ../pkg\n";
    let missing_error = lockfile::from_str(missing).expect_err("missing hash is rejected");
    assert!(missing_error.to_string().contains("nar_hash"));

    let duplicate = format!(
        "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n    locked:\n      type: path\n      path: ../pkg\n      nar_hash: '{ZERO_SRI}'\n      nar_hash: '{ZERO_SRI}'\n"
    );
    let duplicate_error = lockfile::from_str(&duplicate).expect_err("duplicate hash is rejected");
    assert!(
        duplicate_error
            .to_string()
            .contains("duplicate key \"nar_hash\"")
    );

    for hash in malformed {
        let yaml = format!(
            "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: path\n      path: ../pkg\n    locked:\n      type: path\n      path: ../pkg\n      nar_hash: {:?}\n",
            hash
        );
        let error = lockfile::from_str(&yaml).expect_err(hash);
        assert!(
            error.to_string().contains("canonical SRI SHA-256"),
            "{hash:?}: {error}"
        );
    }
}

#[test]
fn rejects_nonimmutable_git_revision() {
    let yaml = format!(
        "version: 2\npackages:\n  package.strict:\n    manifest: brainbrew.yaml\n    package:\n      version: 1.0.0\n    original:\n      type: git\n      url: https://github.com/example/pkg.git\n      ref: main\n    locked:\n      type: git\n      url: https://github.com/example/pkg.git\n      rev: main\n      nar_hash: '{ZERO_SRI}'\n"
    );
    let error = lockfile::from_str(&yaml).expect_err("mutable git ref is rejected");
    assert!(
        error
            .to_string()
            .contains("full lowercase 40-character Git commit")
    );
}

#[test]
fn reports_v1_as_insecure_with_actionable_migration_guidance_before_old_fields() {
    let error = lockfile::from_str(
        r#"version: 1
packages:
  old.package:
    manifest: brainbrew.yaml
    package:
      version: 1.0.0
    locked:
      type: path
      path: ../pkg
"#,
    )
    .expect_err("v1 is rejected");

    let message = error.to_string();
    assert!(message.contains("version 1"), "{message}");
    assert!(message.contains("insecure"), "{message}");
    assert!(message.contains("brainbrew lock update"), "{message}");
}

#[test]
fn rejects_duplicate_lock_package_keys_with_schema_path() {
    let yaml = format!(
        r#"version: 2
packages:
  example.package:
    manifest: one.yaml
    package:
      version: 1.0.0
    original:
      type: path
      path: one
    locked:
      type: path
      path: one
      nar_hash: '{ZERO_SRI}'
  example.package:
    manifest: two.yaml
    package:
      version: 2.0.0
    original:
      type: path
      path: two
    locked:
      type: path
      path: two
      nar_hash: '{ZERO_SRI}'
"#,
    );

    let error = lockfile::from_str(&yaml).expect_err("duplicate package is rejected");
    let message = error.to_string();
    assert!(
        message.contains("duplicate key \"example.package\""),
        "{message}"
    );
    assert!(message.contains("packages.example.package"), "{message}");
    assert!(lockfile::format_str(&yaml).is_err());
}

#[test]
fn rejects_unknown_top_level_fields_and_future_versions() {
    let unknown = lockfile::from_str("version: 2\npackages: {}\nunknown: true\n")
        .expect_err("unknown fields are rejected");
    assert!(unknown.to_string().contains("unknown field `unknown`"));

    let future =
        lockfile::from_str("version: 3\npackages: {}\n").expect_err("future versions are rejected");
    assert!(matches!(
        future,
        lockfile::LockfileError::UnsupportedVersion(3)
    ));
}

#[test]
fn reports_corrupt_lock_yaml_as_parse_error() {
    let error = lockfile::from_str("version: [\n").expect_err("corrupt lock YAML is rejected");
    assert!(matches!(error, lockfile::LockfileError::Parse(_)));
    assert!(error.to_string().contains("failed to parse lock YAML"));
}
