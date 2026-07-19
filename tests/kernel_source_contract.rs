use std::collections::BTreeSet;

fn variants(source: &str, declaration: &str) -> BTreeSet<String> {
    let start = source
        .find(declaration)
        .unwrap_or_else(|| panic!("missing {declaration}"));
    let source = &source[start + declaration.len()..];
    let mut depth = 1i32;
    let mut result = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if depth == 1
            && trimmed
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
        {
            let name = trimmed.split(['{', '(', ',']).next().unwrap().trim();
            if !name.is_empty() {
                result.insert(name.to_owned());
            }
        }
        depth += trimmed.matches('{').count() as i32;
        depth -= trimmed.matches('}').count() as i32;
        if depth == 0 {
            break;
        }
    }
    result
}

#[test]
fn public_error_vocabulary_matches_the_closed_invalid_case_fixture() {
    let fixture: toml::Value = toml::from_str(include_str!("fixtures/kernel-invalid-cases.toml"))
        .expect("valid kernel error fixture");
    let sources = [
        include_str!("../src/doc.rs"),
        include_str!("../src/prepare.rs"),
        include_str!("../src/kernel.rs"),
        include_str!("../src/render.rs"),
        include_str!("../src/table.rs"),
    ];
    let declarations = [
        ("TextError", "pub enum TextError {"),
        ("PrepareError", "pub enum PrepareError {"),
        ("ReserveError", "pub enum ReserveError {"),
        ("RenderError", "pub enum RenderError {"),
        ("VisitError", "pub enum VisitError<E> {"),
        ("OwnedRenderError", "pub enum OwnedRenderError {"),
        ("TableError", "pub enum TableError {"),
    ];

    for (name, declaration) in declarations {
        let source = sources
            .iter()
            .find(|source| source.contains(declaration))
            .unwrap_or_else(|| panic!("missing source for {name}"));
        let live = variants(source, declaration);
        let expected = fixture[name]["variants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(live, expected, "{name}");
    }
}

#[test]
fn default_source_has_no_big_integer_or_downstream_console_dependency_edge() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("num-bigint = { version = \"0.4.6\", optional = true }"));
    assert!(manifest.contains("research = [\"dep:num-bigint\"]"));
    let forbidden = ["ter", "mosaic"].concat();
    assert!(!manifest.to_ascii_lowercase().contains(&forbidden));

    for source in [
        include_str!("../src/doc.rs"),
        include_str!("../src/prepare.rs"),
        include_str!("../src/kernel.rs"),
        include_str!("../src/render.rs"),
    ] {
        assert!(!source.contains("BigUint"));
        assert!(!source.to_ascii_lowercase().contains(&forbidden));
    }
}

#[test]
fn package_metadata_is_publishable_and_exact() {
    let manifest: toml::Value =
        toml::from_str(include_str!("../Cargo.toml")).expect("valid package manifest");
    let package = manifest["package"].as_table().expect("package table");
    assert_eq!(package["name"].as_str(), Some("laidout"));
    assert_eq!(package["version"].as_str(), Some("0.2.0"));
    assert_eq!(package["rust-version"].as_str(), Some("1.85"));
    assert_eq!(package["license"].as_str(), Some("MIT"));
    assert_eq!(package["readme"].as_str(), Some("README.md"));
    assert_eq!(
        package["repository"].as_str(),
        Some("https://github.com/bryanmatteson/laidout")
    );
}

#[test]
fn benchmark_transition_parser_is_document_aware() {
    let source = include_str!("../benches/support/harness_v1.rs");
    assert!(source.contains("toml::from_str(&transition_text)"));
    assert!(!source.contains("let transition: toml::Value = fs::read_to_string"));
    assert!(source.contains("38c55341e02fd9e4869e060d36e88886c7ed1ea3:src"));
    assert!(source.contains("38c55341e02fd9e4869e060d36e88886c7ed1ea3:Cargo.toml"));
    let production_identity = source
        .split_once("fn production_source_digest")
        .expect("production source identity function")
        .1
        .split_once("fn sha256_path")
        .expect("bounded production source identity function")
        .0;
    assert!(!production_identity.contains("\"HEAD:src\""));
    assert!(!production_identity.contains("\"HEAD:Cargo.toml\""));

    let transition: toml::Value = toml::from_str(include_str!(
        "../docs/benchmark-dependency-transition-0.2.toml"
    ))
    .expect("valid dependency transition manifest");
    assert_eq!(transition["schema_version"].as_integer(), Some(1));
    assert!(transition["direct_change"]
        .as_array()
        .is_some_and(|changes| !changes.is_empty()));
}
