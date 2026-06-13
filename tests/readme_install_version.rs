fn package_major_minor_version() -> String {
    let cargo_toml = include_str!("../Cargo.toml");
    let version = cargo_toml
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|version| version.split('"').next())
        .expect("Cargo.toml package version should be present");
    let mut parts = version.split('.');
    let major = parts.next().expect("package version should have a major");
    let minor = parts.next().expect("package version should have a minor");

    format!("{major}.{minor}")
}

#[test]
fn readme_install_examples_match_package_major_minor_version() {
    let readme = include_str!("../README.md");
    let expected_version = package_major_minor_version();

    for expected_snippet in [
        format!("polyforge = \"{expected_version}\""),
        format!("polyforge = {{ version = \"{expected_version}\","),
    ] {
        assert!(
            readme.contains(&expected_snippet),
            "README.md should include dependency snippet: {expected_snippet}"
        );
    }

    assert!(
        !readme.contains("polyforge = \"2.0\"")
            && !readme.contains("polyforge = { version = \"2.0\","),
        "README.md should not point new installs at the previous major version"
    );
}
