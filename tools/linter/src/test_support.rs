// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Test fixture operations shared by inline and crate-level tests.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::census::{Census, scan_source};
use crate::declaration::AbnfPattern;
use crate::finding::Finding;
use crate::profile::{CoveredAsset, analyze_profile};
use crate::snapshot::{Configuration, OwnerRow, Pair, Snapshot, configuration};
use crate::todo::{CoveredNotice, TodoCensus, cover_todos, scan_todos};
use crate::workspace::Package;
use crate::{FixOutcome, fix_profile, fix_todos};

#[path = "../tests/support/git.rs"]
mod git_fixture;

/// Load a minimal declared surface carrying an invented owner partition.
///
/// # Panics
///
/// Panics if the temporary surface cannot be written or does not load.
pub fn snapshot_with_partition(partitions: &[OwnerRow]) -> Snapshot {
    let root = tempfile::tempdir().expect("temporary declaration root");
    write_surface(root.path(), partitions, &[]);

    let Configuration::Present(snapshot) = configuration(root.path()) else {
        panic!("the invented surface must load");
    };

    *snapshot
}

/// Load an explicit fictional snapshot that activates all projections over
/// every path.
///
/// # Panics
///
/// Panics if the temporary surface cannot be written or does not load.
pub fn projection_snapshot() -> Snapshot {
    let root = tempfile::tempdir().expect("temporary declaration root");
    let partitions = [fictional_partition()];
    let policies = [
        Pair::singleton("INDEX", "labels.mints-well-formed"),
        Pair::singleton("INDEX", "profile.tests-conform"),
        Pair::singleton("INDEX", "profile.constants-conform"),
        Pair::singleton("INDEX", crate::subscribe::TEST_INDEXES_POLICY),
        Pair::singleton("INDEX", crate::subscribe::TEST_MATRICES_POLICY),
        Pair::singleton("INDEX", crate::subscribe::CONSTANT_PINS_POLICY),
    ];
    write_surface(root.path(), &partitions, &policies);

    let Configuration::Present(snapshot) = configuration(root.path()) else {
        panic!("the projection surface must load");
    };

    *snapshot
}

/// Write an explicit fictional label surface over every path in a fixture.
///
/// # Panics
///
/// Panics if the fixture surface cannot be written.
pub fn declare_label_surface(root: &Path) {
    let partitions = [fictional_partition()];
    let policies = [Pair::singleton("INDEX", "labels.mints-well-formed")];

    write_surface(root, &partitions, &policies);
}

fn fictional_partition() -> OwnerRow {
    OwnerRow {
        name: String::from("fictional-root"),
        owner: String::from("INDEX"),
        pattern: AbnfPattern::parse("*VCHAR").expect("the wildcard pattern parses"),
    }
}

fn write_surface(root: &Path, partitions: &[OwnerRow], policies: &[Pair]) {
    let directory = root.join(".linter");
    fs::create_dir_all(&directory).expect("declaration directory");

    let owners: BTreeSet<&str> = partitions.iter().map(|row| row.owner.as_str()).collect();
    let mut owner_text = String::from(
        "namespace = \"com.ember.index.linter.owners\"\nversion = [1, 0, 0]\n\nowners = [",
    );

    for (index, owner) in owners.iter().enumerate() {
        if index > 0 {
            owner_text.push_str(", ");
        }
        write!(owner_text, "{owner:?}").expect("writing a string cannot fail");
    }

    owner_text.push_str("]\npartitions = [");

    for (index, row) in partitions.iter().enumerate() {
        if index > 0 {
            owner_text.push_str(", ");
        }
        write!(
            owner_text,
            "{{ name = {:?}, owner = {:?}, pattern = {:?} }}",
            row.name,
            row.owner,
            row.pattern.source()
        )
        .expect("writing a string cannot fail");
    }

    owner_text.push_str("]\nmay_cite = []\n");
    fs::write(directory.join("owners.toml"), owner_text).expect("owner document");
    fs::write(
        directory.join("environments.toml"),
        "namespace = \"com.ember.index.linter.environments\"\nversion = [1, 0, 0]\n\nenvironments = []\n",
    )
    .expect("environment document");
    let mut policy_text = String::from(
        "namespace = \"com.ember.index.linter.policies\"\nversion = [1, 0, 0]\n\npolicies = [",
    );
    let mut list_text =
        String::from("namespace = \"com.ember.index.linter.lists\"\nversion = [1, 0, 0]\n");

    for (index, pair) in policies.iter().enumerate() {
        if index > 0 {
            policy_text.push_str(", ");
        }
        write!(
            policy_text,
            "{{ owner = {:?}, policy = {:?} }}",
            pair.owner, pair.policy
        )
        .expect("writing a string cannot fail");
        write!(
            list_text,
            "\n[{}.{:?}]\nallowances = []\n",
            pair.owner, pair.policy
        )
        .expect("writing a string cannot fail");
    }

    policy_text.push_str("]\n");
    fs::write(directory.join("policies.toml"), policy_text).expect("activation document");
    fs::write(directory.join("lists.toml"), list_text).expect("list document");
    fs::write(
        directory.join("shape.toml"),
        "namespace = \"com.ember.index.linter.shape\"\nversion = [1, 0, 0]\n\nuniverse = \"git-tracked\"\nignore = []\n",
    )
    .expect("shape document");
}

/// Make a fixture root a repository and track every file currently in it.
///
/// # Panics
///
/// Panics if the fixture repository cannot be constructed or indexed.
pub fn initialise_and_track(root: &Path) {
    git_fixture::initialise_repository(root);
    track_all(root);
}

/// Add every fixture file directly to the repository index.
///
/// # Panics
///
/// Panics if the fixture repository cannot be indexed.
pub fn track_all(root: &Path) {
    git_fixture::track_all(root);
}

/// Track exactly the fixture-relative paths supplied.
///
/// # Panics
///
/// Panics if the fixture repository cannot be constructed or indexed.
pub fn track_paths(root: &Path, paths: &[std::path::PathBuf]) {
    git_fixture::track_paths(root, paths);
}

/// Write one test source into a temporary root and sweep its derived labels.
///
/// # Panics
///
/// Panics if the temporary fixture cannot be created, written, read, or parsed.
pub fn sweep_profile(text: &str, dry_run: bool) -> (String, FixOutcome, Vec<Finding>) {
    let root = tempfile::tempdir().expect("temporary directory");
    let relative = Path::new("src/demo.rs");
    fs::create_dir_all(root.path().join("src")).expect("create");
    fs::write(root.path().join(relative), text).expect("write");

    let (outcome, findings) = fix_profile(root.path(), &test_assets(text), dry_run);
    let rewritten = fs::read_to_string(root.path().join(relative)).expect("read");

    (rewritten, outcome, findings)
}

/// Cover the test functions carried by one invented package source.
///
/// # Panics
///
/// Panics if the invented Rust source cannot be parsed.
pub fn test_assets(text: &str) -> Vec<CoveredAsset> {
    let packages = vec![Package::new("ember-demo", "")];
    let tests = scan_source("ember-demo", Path::new("src/demo.rs"), text).expect("a Rust source");
    let census = Census::from_tests(tests, 1);
    let (_analysis, assets, _findings) = analyze_profile(&packages, &census);

    assets
}

/// Write one source into a temporary root and sweep its notice labels.
///
/// # Panics
///
/// Panics if the temporary fixture cannot be created, written, or read.
pub fn sweep_notices(text: &str, dry_run: bool) -> (String, FixOutcome, Vec<Finding>) {
    let root = tempfile::tempdir().expect("temporary directory");
    let relative = Path::new("src/demo.rs");
    fs::create_dir_all(root.path().join("src")).expect("create");
    fs::write(root.path().join(relative), text).expect("write");

    let (outcome, findings) = fix_todos(root.path(), &notices_of(text), dry_run);
    let rewritten = fs::read_to_string(root.path().join(relative)).expect("read");

    (rewritten, outcome, findings)
}

/// Cover the to-do notices carried by one invented package source.
pub fn notices_of(text: &str) -> Vec<CoveredNotice> {
    let packages = vec![Package::new("ember-demo", "")];
    let (notices, _orphans) = scan_todos("ember-demo", Path::new("src/demo.rs"), text);
    let census = TodoCensus::from_notices(notices, 1);
    let (covered, _findings) = cover_todos(&packages, &census);

    covered
}
