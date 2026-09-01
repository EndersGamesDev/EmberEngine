// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`the_repository_owner_surface_reconciles_in_both_directions`] | owners | The declared partition attributes every tracked path exactly once to the thirteen owners — twelve derived from manifests and the crateless repository owner — its five crate-to-crate may-cite rows equal the internal path dependencies after workspace inheritance is resolved, and its five edges to the crateless owner are declaration-authoritative. # Panics Panics only when this checkout cannot be read or parsed, or when its owner, partition, crate-name, or direct-reach facts diverge from the declaration. |

//! Repository owner-partition and manifest-reach integration tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use linter::{ExecutionPlan, configuration};

const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn manifest(path: &Path) -> toml::Table {
    let text = fs::read_to_string(path).expect("read manifest");
    toml::from_str(&text).expect("parse manifest")
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(component) => normalized.push(component),
            Component::Prefix(_) | Component::RootDir => {
                panic!("repository dependency paths are relative")
            }
        }
    }

    normalized
}

fn reconcile_partition(plan: &ExecutionPlan) -> BTreeMap<PathBuf, String> {
    let snapshot = plan.snapshot();
    let corpus = plan.topology().corpus();
    let partition = plan.topology().partition();

    assert!(corpus.findings().is_empty(), "{:?}", corpus.findings());
    assert!(
        partition.findings().is_empty(),
        "{:?}",
        partition.findings()
    );
    assert_eq!(partition.counts().excluded, 0);
    assert_eq!(partition.counts().unaccounted, 0);
    assert_eq!(partition.counts().multiply_accounted, 0);
    assert_eq!(partition.counts().surviving, partition.counts().accounted);
    assert_eq!(corpus.participating().len(), partition.attribution().len());

    let names = snapshot
        .declared_owner_names()
        .expect("declared owner-name derivation");
    let mut owner_by_directory = BTreeMap::new();

    for package in plan.workspace().packages() {
        let owner = names
            .derive(package.name())
            .expect("derivable crate owner")
            .to_string();
        assert!(
            owner_by_directory
                .insert(package.directory().to_path_buf(), owner)
                .is_none()
        );
    }

    let derived_owners: BTreeSet<String> = owner_by_directory.values().cloned().collect();
    let declared_owners: BTreeSet<String> = snapshot.owners().iter().cloned().collect();
    let mut expected_owners = derived_owners;
    expected_owners.insert("ORCHESTRATION".to_owned());
    assert_eq!(expected_owners, declared_owners);
    assert_eq!(owner_by_directory.len(), 12);
    assert_eq!(plan.workspace().findings().len(), 0);

    let mut path_counts: BTreeMap<String, usize> = BTreeMap::new();
    for (path, owner) in partition.attribution() {
        let native = Path::new(std::str::from_utf8(path.as_bytes()).expect("tracked text path"));
        let expected = owner_by_directory
            .iter()
            .find_map(|(directory, owner)| native.starts_with(directory).then_some(owner))
            .map_or("ORCHESTRATION", String::as_str);

        assert_eq!(owner, expected, "{}", path.display());
        *path_counts.entry(owner.clone()).or_default() += 1;
    }
    assert_eq!(path_counts.len(), 13);
    assert!(path_counts.values().all(|count| *count > 0));

    owner_by_directory
}

fn manifest_reach(
    root: &Path,
    plan: &ExecutionPlan,
    owner_by_directory: &BTreeMap<PathBuf, String>,
) -> BTreeSet<(String, String)> {
    let root_manifest = manifest(&root.join("Cargo.toml"));
    let workspace_dependencies = root_manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
        .expect("workspace dependencies");
    let internal_by_directory: BTreeMap<PathBuf, String> = owner_by_directory
        .iter()
        .map(|(directory, owner)| (normalize(directory), owner.clone()))
        .collect();
    let mut derived_reach = BTreeSet::new();

    for package in plan.workspace().packages() {
        let source = owner_by_directory
            .get(package.directory())
            .expect("source owner");
        let member_manifest = manifest(&root.join(package.directory()).join("Cargo.toml"));

        for table_name in DEPENDENCY_TABLES {
            let Some(dependencies) = member_manifest
                .get(table_name)
                .and_then(toml::Value::as_table)
            else {
                continue;
            };

            for (dependency_name, dependency) in dependencies {
                let Some(dependency) = dependency.as_table() else {
                    continue;
                };
                let inherited = dependency
                    .get("workspace")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or_default();
                let declared_path = if inherited {
                    workspace_dependencies
                        .get(dependency_name)
                        .and_then(toml::Value::as_table)
                        .and_then(|entry| entry.get("path"))
                        .and_then(toml::Value::as_str)
                        .map(PathBuf::from)
                } else {
                    dependency
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .map(|path| package.directory().join(path))
                };
                let Some(target) = declared_path
                    .as_deref()
                    .map(normalize)
                    .and_then(|path| internal_by_directory.get(&path))
                else {
                    continue;
                };

                if source != target {
                    derived_reach.insert((source.clone(), target.clone()));
                }
            }
        }
    }

    derived_reach
}

/// The declared partition attributes every tracked path exactly once to the
/// thirteen owners — twelve derived from manifests and the crateless repository
/// owner — its five crate-to-crate may-cite rows equal the internal path
/// dependencies after workspace inheritance is resolved, and its five edges to
/// the crateless owner are declaration-authoritative.
///
/// # Panics
///
/// Panics only when this checkout cannot be read or parsed, or when its owner,
/// partition, crate-name, or direct-reach facts diverge from the declaration.
///
/// ´claim:owners:the-repository-owner-surface-reconciles-in-both-directions´
/// ´test:integration:the-repository-owner-surface-reconciles-in-both-directions´
#[test]
fn the_repository_owner_surface_reconciles_in_both_directions() {
    let root = repository_root();
    let plan = ExecutionPlan::compile(&root, configuration(&root)).expect("repository plan");
    let snapshot = plan.snapshot();
    let owner_by_directory = reconcile_partition(&plan);
    let derived_reach = manifest_reach(&root, &plan, &owner_by_directory);

    let declared_reach: BTreeSet<(String, String)> = snapshot
        .may_cite()
        .iter()
        .map(|row| (row.owner.clone(), row.target.clone()))
        .collect();

    let (crateless_reach, crate_reach): (BTreeSet<_>, BTreeSet<_>) = declared_reach
        .iter()
        .cloned()
        .partition(|(_owner, target)| target == "ORCHESTRATION");
    let expected_crateless_reach = BTreeSet::from([
        ("CODEXLANE".to_owned(), "ORCHESTRATION".to_owned()),
        ("HEARTBEATGUARD".to_owned(), "ORCHESTRATION".to_owned()),
        ("LANEVERIFY".to_owned(), "ORCHESTRATION".to_owned()),
        ("LINTER".to_owned(), "ORCHESTRATION".to_owned()),
        ("ORCHESTRATORHOOK".to_owned(), "ORCHESTRATION".to_owned()),
    ]);

    assert_eq!(derived_reach, crate_reach);
    assert_eq!(crateless_reach, expected_crateless_reach);
    assert_eq!(declared_reach.len(), 10);
}
