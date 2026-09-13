use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::models::{GameCatalog, HostBook, HostEntry, Mirror, boundary_descriptions};
use crate::{Boundary, Registry, View, json_schema};

#[derive(Boundary, serde::Deserialize)]
#[boundary(direction = "input")]
#[serde(deny_unknown_fields)]
struct StrictInput {
    value: u8,
}

fn workspace(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read(path: &str) -> String {
    fs::read_to_string(workspace(path)).expect("fixture is readable")
}

fn validate<T: Boundary>(value: &Value) {
    let schema = json_schema::<T>(View::Input);
    let validator = jsonschema::validator_for(&schema).expect("generated schema compiles");
    if let Err(error) = validator.validate(value) {
        panic!("generated schema rejected fixture: {error}");
    }
}

fn round_trip<T>(source: &str)
where
    T: DeserializeOwned + PartialEq + Serialize + std::fmt::Debug,
{
    let model: T = serde_json::from_str(source).expect("fixture deserializes");
    let encoded = serde_json::to_string(&model).expect("model serializes");
    assert_eq!(
        serde_json::from_str::<T>(&encoded).expect("round trip decodes"),
        model
    );
}

#[test]
fn real_catalog_is_owned_by_the_model_and_schema() {
    let source = read("web/games.json");
    let value = serde_json::from_str(&source).expect("catalog is JSON");
    validate::<GameCatalog>(&value);
    let catalog: GameCatalog = serde_json::from_str(&source).expect("catalog matches model");
    assert!(catalog.games.iter().all(|game| !game.versions.is_empty()));
    assert!(
        catalog
            .games
            .iter()
            .flat_map(|game| &game.versions)
            .all(|release| release.bytes.is_none())
    );
}

#[test]
fn current_server_and_mirror_fixtures_validate() {
    let book = read("crates/ember-boundary/tests/fixtures/server.json");
    let mirror = read("crates/ember-boundary/tests/fixtures/mirror.json");
    validate::<HostBook>(&serde_json::from_str(&book).expect("book JSON"));
    validate::<HostEntry>(&serde_json::from_str(&mirror).expect("mirror JSON"));
    round_trip::<HostBook>(&book);
    round_trip::<HostEntry>(&mirror);
}

#[test]
fn hostile_host_inputs_round_trip_without_widening_types() {
    let source = read("crates/ember-boundary/tests/fixtures/hostile-book.json");
    round_trip::<HostBook>(&source);
    let wrong_types = read("crates/ember-boundary/tests/fixtures/hostile-types.json");
    assert!(serde_json::from_str::<HostBook>(&wrong_types).is_err());
}

#[test]
fn registry_is_filled_only_from_derived_descriptions() {
    let mut registry = Registry::new();
    registry.register::<GameCatalog>();
    registry.register::<HostBook>();
    registry.register::<Mirror>();
    assert_eq!(registry.entries().len(), 3);
    assert_eq!(registry.entries()[0].name, "GameCatalog");
}

#[test]
fn every_data_model_is_enumerated() {
    assert_eq!(boundary_descriptions().len(), 7);
}

#[test]
fn deny_unknown_fields_is_preserved_by_the_input_schema() {
    let value = serde_json::json!({ "value": 1, "future": true });
    let schema = json_schema::<StrictInput>(View::Input);
    assert!(
        jsonschema::validator_for(&schema)
            .expect("schema compiles")
            .validate(&value)
            .is_err()
    );
    assert!(serde_json::from_value::<StrictInput>(value).is_err());
    assert_eq!(
        serde_json::from_value::<StrictInput>(serde_json::json!({ "value": 1 }))
            .expect("known field deserializes")
            .value,
        1
    );
}
