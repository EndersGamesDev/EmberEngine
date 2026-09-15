use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ember_webgen::{Manifest, RenderedBundle};
use serde_json::Value;
use sha2::{Digest, Sha256};

const TEMPLATE_HASH: &str = "c4b777fd539a89b41501ba538ecc5471ba108aaf34fa7975b0579d828cf71f24";

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

fn has_golden(path: &str) -> bool {
    // Behaviour inputs and line maps use the exact source-plus-preamble and
    // adjacent-map assertions below instead of duplicated golden files.
    path.starts_with("schema/") || path.ends_with(".d.ts") || path == "ts/exhaustive-consumer.ts"
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
        .filter(|file| has_golden(&file.path))
        .map(|file| golden_name(&file.path))
        .collect::<BTreeSet<_>>();
    let expected_names = fs::read_dir(&directory)
        .expect("golden directory exists")
        .map(|entry| entry.expect("golden entry is readable").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name != "README.md")
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_names, expected_names);
    for file in bundle
        .files
        .into_iter()
        .filter(|file| has_golden(&file.path))
    {
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
    assert_eq!(manifest.files.len(), 13);
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

fn assert_behaviour_input(
    bundle: &RenderedBundle,
    rendered_path: &str,
    template_path: &str,
    expected_preamble: &str,
) {
    let rendered = text(bundle, rendered_path);
    let line_map = bundle
        .files
        .iter()
        .find(|file| file.path == format!("{rendered_path}.map.json"))
        .expect("the line map is emitted beside its input");
    let line_map: ember_webgen::RenderedLineMap =
        serde_json::from_slice(&line_map.bytes).expect("the line map is valid JSON");
    assert_eq!(line_map.rendered_path, rendered_path);
    assert_eq!(line_map.template_path, template_path);
    let first_body_line = line_map
        .lines
        .iter()
        .position(Option::is_some)
        .expect("a behaviour template has a body");
    assert_eq!(first_body_line, expected_preamble.lines().count());
    let mut expected = expected_preamble.as_bytes().to_vec();
    expected.extend(
        fs::read(repository_path(template_path)).expect("the behaviour template is readable"),
    );
    assert_eq!(rendered.as_bytes(), expected);
    assert!(
        line_map.lines[..first_body_line]
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(
        line_map.lines[first_body_line..],
        (1..=fs::read_to_string(repository_path(template_path))
            .expect("template is readable")
            .lines()
            .count())
            .map(Some)
            .collect::<Vec<_>>()
    );
}

#[test]
fn behaviour_inputs_have_exact_bodies_and_adjacent_template_line_maps() {
    let bundle = bundle();
    assert_behaviour_input(
        &bundle,
        "ts/behavior-placeholder.ts",
        "crates/ember-webgen/templates/behavior-placeholder.ts.j2",
        "// Generated from crates/ember-webgen/templates/behavior-placeholder.ts.j2 at golden. Do not edit.\nimport type { Phase } from './ember-loader.js';\n\n",
    );
}

#[test]
fn behaviour_input_check_rejects_boundary_import_drift() {
    let mut bundle = bundle();
    let rendered = bundle
        .files
        .iter_mut()
        .find(|file| file.path == "ts/behavior-placeholder.ts")
        .expect("the behaviour input is rendered");
    let source = String::from_utf8(rendered.bytes.clone()).expect("the rendered input is UTF-8");
    rendered.bytes = source
        .replace("import type { Phase }", "import type { Status }")
        .into_bytes();
    let result = std::panic::catch_unwind(|| {
        assert_behaviour_input(
            &bundle,
            "ts/behavior-placeholder.ts",
            "crates/ember-webgen/templates/behavior-placeholder.ts.j2",
            "// Generated from crates/ember-webgen/templates/behavior-placeholder.ts.j2 at golden. Do not edit.\nimport type { Phase } from './ember-loader.js';\n\n",
        );
    });
    assert!(
        result.is_err(),
        "boundary import drift passed the exact-byte check"
    );
}

#[test]
fn diagnostics_keep_rendered_coordinates_and_add_template_coordinates() {
    let temporary = tempfile::tempdir().expect("temporary directory is available");
    let out = temporary.path().join("target/web-generated/golden");
    ember_webgen::write(&out, &bundle()).expect("bundle writes");
    let rendered = out.join("ts/behavior-placeholder.ts");
    let input = format!(
        "{}(4,14): error TS2322: deliberate\n{}(2,1): error TS1000: preamble\n",
        rendered.display(),
        rendered.display()
    );
    let mapped = ember_webgen::map_diagnostics(&input).expect("diagnostics map");
    assert!(mapped.contains(
        "crates/ember-webgen/templates/behavior-placeholder.ts.j2(1,14): error TS2322: deliberate"
    ));
    assert!(mapped.contains(&format!("[rendered {}(4,14)]", rendered.display())));
    assert!(mapped.contains(&format!(
        "{}(2,1): error TS1000: preamble",
        rendered.display()
    )));
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
