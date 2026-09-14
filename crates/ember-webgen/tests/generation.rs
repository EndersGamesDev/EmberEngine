use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ember_webgen::{Manifest, RenderedBundle};
use serde_json::Value;
use sha2::{Digest, Sha256};

const TEMPLATE_HASH: &str = "a44386e48d4722ee1cbbf0ff781fde7041b52394115f23f0c5cdec2abc551b69";

fn repository_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn bundle() -> RenderedBundle {
    let ids = ember_webgen::game_ids(&repository_path("web/games.json"))
        .expect("the production catalog is valid");
    ember_webgen::render("golden", &ids).expect("production templates render")
}

fn golden_name(path: &str) -> String {
    format!("{}.golden", path.replace('/', "__"))
}

fn digest(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in Sha256::digest(bytes) {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

#[test]
fn every_production_template_is_deterministic() {
    let first = bundle();
    let second = bundle();
    assert_eq!(first, second);
    assert_eq!(first.template_source_hash, TEMPLATE_HASH);
    assert_eq!(ember_webgen::template_source_hash(), TEMPLATE_HASH);
}

#[test]
fn every_emitted_file_matches_its_golden_bytes() {
    let bundle = bundle();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let actual_names = bundle
        .files
        .iter()
        .map(|file| golden_name(&file.path))
        .collect::<BTreeSet<_>>();
    let expected_names = fs::read_dir(&directory)
        .expect("golden directory exists")
        .map(|entry| entry.expect("golden entry is readable").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name != "README.md")
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_names, expected_names);
    for file in bundle.files {
        let expected = fs::read(directory.join(golden_name(&file.path)))
            .expect("every rendered path has a golden file");
        assert_eq!(file.bytes, expected, "golden drift for {}", file.path);
    }
}

#[test]
fn every_emitted_file_has_exactly_one_final_newline() {
    let bundle = bundle();
    for file in &bundle.files {
        assert!(
            file.bytes.ends_with(b"\n"),
            "{} lacks a final newline",
            file.path
        );
        assert!(
            !file.bytes.ends_with(b"\n\n"),
            "{} ends with a blank line",
            file.path
        );
    }
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let out = temporary.path().join("target/web-generated/golden");
    ember_webgen::write(&out, &bundle).expect("bundle writes");
    let manifest = fs::read(out.join("manifest.json")).expect("manifest is readable");
    assert!(manifest.ends_with(b"\n"));
    assert!(!manifest.ends_with(b"\n\n"));
}

#[test]
fn every_union_count_is_pinned_to_its_descriptor() {
    let expected = vec![
        ("arena-core".into(), "C2S".into(), 7),
        ("arena-core".into(), "S2C".into(), 14),
        ("fire-core".into(), "Phase".into(), 4),
        ("fire-core".into(), "C2S".into(), 10),
        ("fire-core".into(), "S2C".into(), 10),
        ("kings-core".into(), "Kind".into(), 9),
        ("kings-core".into(), "Phase".into(), 3),
        ("kings-core".into(), "EndReason".into(), 5),
        ("kings-core".into(), "ActionKind".into(), 7),
        ("kings-core".into(), "C2S".into(), 9),
        ("kings-core".into(), "S2C".into(), 10),
        ("league-core".into(), "Phase".into(), 3),
        ("league-core".into(), "Cmd".into(), 8),
        ("league-core".into(), "C2S".into(), 9),
        ("league-core".into(), "S2C".into(), 11),
        ("ember-loader".into(), "Phase".into(), 7),
        ("ember-loader".into(), "Status".into(), 4),
    ];
    assert_eq!(ember_webgen::union_variant_counts(), expected);
}

#[test]
fn real_catalog_validates_against_the_generated_schema() {
    let bundle = bundle();
    let schema = bundle
        .files
        .iter()
        .find(|file| file.path == "schema/games.schema.json")
        .expect("games schema is emitted");
    let schema: Value = serde_json::from_slice(&schema.bytes).expect("schema is JSON");
    let catalog: Value = serde_json::from_slice(
        &fs::read(repository_path("web/games.json")).expect("catalog is readable"),
    )
    .expect("catalog is JSON");
    jsonschema::validator_for(&schema)
        .expect("schema compiles")
        .validate(&catalog)
        .expect("real catalog matches generated schema");
}

#[test]
fn written_manifest_matches_every_file_and_fixed_compiler_copy() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let out = temporary.path().join("target/web-generated/golden");
    let sentinel = out.join("pkg-types/sentinel");
    fs::create_dir_all(sentinel.parent().expect("sentinel has a parent"))
        .expect("pkg-types directory is writable");
    fs::write(&sentinel, b"keep").expect("sentinel is writable");
    let manifest = ember_webgen::write(&out, &bundle()).expect("bundle writes");
    let decoded: Manifest =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).expect("manifest is readable"))
            .expect("manifest is JSON");
    assert_eq!(manifest, decoded);
    assert_eq!(manifest.source_sha, "golden");
    assert_eq!(manifest.template_source_hash, TEMPLATE_HASH);
    assert!(sentinel.is_file(), "generation preserves staged ABI types");
    assert_eq!(manifest.integer_64_fields.len(), 11);
    assert_eq!(manifest.files.len(), 11);
    assert!(
        manifest
            .files
            .iter()
            .all(|file| !file.path.ends_with("manifest.json"))
    );
    for file in manifest.files {
        let bytes = fs::read(&file.path).expect("manifest path is readable");
        assert_eq!(bytes.len(), file.bytes);
        assert_eq!(digest(&bytes), file.sha256);
        if let Some(compiler_path) = file.compiler_path {
            assert_eq!(
                bytes,
                fs::read(compiler_path).expect("fixed compiler copy is readable")
            );
        }
    }
}

#[test]
fn declarations_preserve_direction_and_special_shapes() {
    let bundle = bundle();
    let boundary = text(&bundle, "ts/ember-boundary.d.ts");
    let loader = text(&bundle, "ts/ember-loader.d.ts");
    let league = text(&bundle, "ts/league-core.d.ts");
    let compat = text(&bundle, "ts/wasm-bindgen-compat.d.ts");
    assert!(!boundary.contains("[key: string]"));
    assert!(boundary.contains("`${G}_ws`"));
    assert!(
        boundary
            .contains("export type ServerGameId = \"arena\" | \"fire\" | \"kings\" | \"league\";")
    );
    assert!(boundary.contains("GameHostAddressKey<G extends ServerGameId = ServerGameId>"));
    assert!(boundary.contains("export type GameCatalog = GameCatalogInput;"));
    assert!(boundary.contains("export type GameRelease = GameReleaseInput;"));
    let event_output = loader
        .split("export type EventOutput = {")
        .nth(1)
        .and_then(|rest| rest.split("\n};").next())
        .expect("EventOutput is emitted");
    assert!(event_output.contains("\"loaded\"?: number;"));
    assert!(!event_output.contains("\"loaded\"?: number | null;"));
    assert!(league.contains("{\n  \"t\": \"cmd\";\n} & CmdInput"));
    assert!(boundary.contains("JavaScript numbers are exact only through 2^53 - 1"));
    assert!(compat.contains("readonly dispose: unique symbol;"));
}

fn text<'a>(bundle: &'a RenderedBundle, path: &str) -> &'a str {
    let bytes = &bundle
        .files
        .iter()
        .find(|file| file.path == path)
        .expect("declaration is emitted")
        .bytes;
    std::str::from_utf8(bytes).expect("declarations are UTF-8")
}
