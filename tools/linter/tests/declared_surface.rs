// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`the_repository_snapshot_loads_as_one_exact_surface`] | declarations | The repository exposes exactly the fixed and parameter declaration members, every envelope parses independently, and the atomic loader accepts their joined snapshot. The declared environment relation is also the exact set projected from ADR 011, with repository title genres held as extensions. # Panics Panics only when this checkout cannot be read or parsed, or when its declared surface differs from the repository facts asserted here. |

//! Repository declaration-loading integration tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use linter::{KindRegistry, Pair, Rows, configuration};

const DOCUMENTS: [(&str, &str); 21] = [
    ("environments.toml", "com.torrust.index.linter.environments"),
    ("lists.toml", "com.torrust.index.linter.lists"),
    ("owners.toml", "com.torrust.index.linter.owners"),
    ("policies.toml", "com.torrust.index.linter.policies"),
    (
        "policy-assembly-publications.toml",
        "com.torrust.index.linter.policy.assembly-publications",
    ),
    (
        "policy-interchange.toml",
        "com.torrust.index.linter.policy.interchange",
    ),
    (
        "policy-legacy-implementation.toml",
        "com.torrust.index.linter.policy.legacy.implementation",
    ),
    (
        "policy-legacy-record-references-repository.toml",
        "com.torrust.index.linter.policy.legacy.record-references-repository",
    ),
    (
        "policy-legacy-record-references.toml",
        "com.torrust.index.linter.policy.legacy.record-references",
    ),
    (
        "policy-legacy-section-references-repository.toml",
        "com.torrust.index.linter.policy.legacy.section-references-repository",
    ),
    (
        "policy-legacy-section-references.toml",
        "com.torrust.index.linter.policy.legacy.section-references",
    ),
    (
        "policy-legacy-tag-references.toml",
        "com.torrust.index.linter.policy.legacy.tag-references",
    ),
    (
        "policy-legacy-todos.toml",
        "com.torrust.index.linter.policy.legacy.todos",
    ),
    (
        "policy-legacy-unprefixed-record-references.toml",
        "com.torrust.index.linter.policy.legacy.unprefixed-record-references",
    ),
    (
        "policy-owner-names.toml",
        "com.torrust.index.linter.policy.owner.names",
    ),
    (
        "policy-references-divisions.toml",
        "com.torrust.index.linter.policy.references.divisions",
    ),
    (
        "policy-references-path-linking.toml",
        "com.torrust.index.linter.policy.references.path-linking",
    ),
    (
        "policy-references-prefix-numbers.toml",
        "com.torrust.index.linter.policy.references.prefix-numbers",
    ),
    (
        "policy-references-scenarios.toml",
        "com.torrust.index.linter.policy.references.scenarios",
    ),
    ("policy-spdx.toml", "com.torrust.index.linter.policy.spdx"),
    ("shape.toml", "com.torrust.index.linter.shape"),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The repository exposes exactly the fixed and parameter declaration members,
/// every envelope parses independently, and the atomic loader accepts their
/// joined snapshot. The declared environment relation is also the exact set
/// projected from ADR 011, with repository title genres held as extensions.
///
/// # Panics
///
/// Panics only when this checkout cannot be read or parsed, or when its declared
/// surface differs from the repository facts asserted here.
///
/// ´claim:declarations:the-repository-snapshot-loads-as-one-exact-surface´
/// ´test:integration:the-repository-snapshot-loads-as-one-exact-surface´
#[test]
fn the_repository_snapshot_loads_as_one_exact_surface() {
    let root = repository_root();
    let declaration_root = root.join(".linter");
    let expected: BTreeMap<String, String> = DOCUMENTS
        .into_iter()
        .map(|(file, namespace)| (file.to_owned(), namespace.to_owned()))
        .collect();
    let mut found = BTreeMap::new();

    for entry in fs::read_dir(&declaration_root).expect("read declaration directory") {
        let entry = entry.expect("read declaration entry");
        let file = entry
            .file_name()
            .into_string()
            .expect("declaration filename is text");
        let text = fs::read_to_string(entry.path()).expect("read declaration");
        let envelope = toml::from_str::<toml::Table>(&text).expect("parse declaration");
        let namespace = envelope
            .get("namespace")
            .and_then(toml::Value::as_str)
            .expect("declaration namespace")
            .to_owned();
        let version: Vec<i64> = envelope
            .get("version")
            .and_then(toml::Value::as_array)
            .expect("declaration version")
            .iter()
            .map(|part| part.as_integer().expect("numeric version part"))
            .collect();

        assert_eq!(version, [1, 0, 0]);
        assert!(found.insert(file, namespace).is_none());
    }

    assert_eq!(found, expected);

    let loaded = configuration(&root);
    assert!(loaded.refusals().is_empty(), "{:?}", loaded.refusals());
    let snapshot = loaded.snapshot().expect("accepted declaration snapshot");

    assert_eq!(snapshot.owners().len(), 13);
    assert_eq!(snapshot.partitions().len(), 20);
    assert_eq!(snapshot.may_cite().len(), 10);
    assert_eq!(snapshot.environments().len(), 349);
    assert_eq!(snapshot.environment_extensions().len(), 9);
    let list_keys = snapshot.list_keys();
    let gateway_projections = [
        Pair::singleton("LANEGATEWAY", "projection.test-indexes-current"),
        Pair::singleton("LANEGATEWAY", "projection.test-matrices-current"),
    ];

    assert_eq!(snapshot.policies().len(), 152);
    assert_eq!(list_keys.len(), 163);
    for pair in gateway_projections {
        assert!(snapshot.policies().contains(&pair));
        assert!(list_keys.contains(&pair));
    }
    assert_eq!(snapshot.lists().values().map(Rows::len).sum::<usize>(), 0);

    let registry_text = fs::read_to_string(root.join("adr/011-environment-kinds.md"))
        .expect("read human kind registry");
    let registry = KindRegistry::parse(&registry_text);
    let human_rows: BTreeSet<(String, String)> = registry
        .rows()
        .iter()
        .map(|row| (row.name().to_owned(), row.kind().to_owned()))
        .collect();
    let declared_rows: BTreeSet<(String, String)> = snapshot
        .environments()
        .iter()
        .map(|row| (row.environment.clone(), row.kind.clone()))
        .collect();
    let human_reserved: BTreeSet<String> = registry.reserved_kinds().iter().cloned().collect();

    assert_eq!(declared_rows, human_rows);
    assert_eq!(snapshot.reserved_kinds(), &human_reserved);

    let extensions: BTreeSet<(String, String)> = snapshot
        .environment_extensions()
        .iter()
        .map(|row| (row.environment.clone(), row.kind.clone()))
        .collect();
    let expected_extensions = [
        ("Constant", "const"),
        ("Document", "guide"),
        ("Document", "plan"),
        ("Document", "proposal"),
        ("Document", "rec"),
        ("Document", "reg"),
        ("Document", "rep"),
        ("Document", "spec"),
        ("To-do", "todo"),
    ]
    .into_iter()
    .map(|(environment, kind)| (environment.to_owned(), kind.to_owned()))
    .collect();

    assert_eq!(extensions, expected_extensions);
    assert_eq!(
        snapshot.reserved_extensions(),
        &BTreeSet::from(["const".to_owned(), "todo".to_owned()])
    );
}
