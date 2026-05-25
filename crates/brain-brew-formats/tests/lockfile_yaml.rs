use brain_brew_formats::lockfile;

#[test]
fn parses_and_formats_federation_lock_with_git_nar_hash() {
    let formatted = lockfile::format_str(
        r#"
packages:
  anki-geo.ultimate-geography:
    locked:
      nar_hash: sha256-example
      rev: ccf150a1b21e
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    original:
      ref: main
      url: https://github.com/anki-geo/ultimate-geography.git
      type: git
    package:
      version: 0.1.0
    manifest: brainbrew.yaml
version: 1
"#,
    )
    .expect("lock formats");

    assert_eq!(
        formatted,
        r#"version: 1
packages:
  anki-geo.ultimate-geography:
    manifest: brainbrew.yaml
    package:
      version: 0.1.0
    original:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      ref: main
    locked:
      type: git
      url: https://github.com/anki-geo/ultimate-geography.git
      rev: ccf150a1b21e
      nar_hash: sha256-example
"#
    );

    let lock = lockfile::from_str(&formatted).expect("formatted lock parses");
    let package = &lock.packages["anki-geo.ultimate-geography"];
    assert_eq!(package.package.version, "0.1.0");
    assert_eq!(package.locked.source_type, "git");
    assert_eq!(package.locked.rev.as_deref(), Some("ccf150a1b21e"));
    assert_eq!(package.locked.nar_hash.as_deref(), Some("sha256-example"));
}

#[test]
fn rejects_unknown_lock_fields() {
    let error = lockfile::from_str(
        r#"
version: 1
unknown: true
"#,
    )
    .expect_err("unknown fields are rejected");

    assert!(error.to_string().contains("unknown field `unknown`"));
}
