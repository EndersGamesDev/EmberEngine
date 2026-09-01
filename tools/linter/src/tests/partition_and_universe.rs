// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Tracked-universe, exclusion-union, and owner-partition tests.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`the_universe_removes_nothing_implicitly`] | partition | The universe carries every tracked path at every depth and nothing else: an empty directory contributes nothing, and no ignore-file, hidden-path or generated-artifact convention removes a tracked entry. An implicit exclusion would be an ownership decision nobody ratified, so a path the repository committed stays in the universe even where an ignore rule names it. |
//! | [`the_version_control_store_is_absent_and_its_rule_is_idle`] | partition | The version-control store is absent from the universe because git does not track its own store, and the named rule that excused it stands and reaches nothing. An exclusion rule matching no path is legal: keeping it records that the exclusion was decided rather than inherited from whatever enumerates the tree, and a universe that later carried the store would find the rule already waiting. |
//! | [`a_corpus_that_cannot_be_taken_is_a_traversal_failure`] | partition | A root whose tracked corpus cannot be taken is a traversal failure naming the root, not an empty universe. An empty universe would let totality pass for the wrong reason: with nothing to account, every inclusion row is vacuously total and the check would report a clean partition of a repository it never read. |
//! | [`untracked_working_copy_content_is_not_the_repository`] | partition | Untracked working-copy content is not the repository and never enters the accounting universe. A checkout carries material nobody committed — build output, local configuration, a data directory the program filled at runtime — and a universe read from the physical tree made the partition verdict a property of whoever ran the check rather than of the commit. |
//! | [`a_path_that_is_not_text_refuses_topology`] | partition | A file whose name is not text refuses the topology once, carrying the reversible byte display rather than disappearing through lossy conversion. |
//! | [`the_excluded_set_is_an_orderless_union`] | partition | The excluded set is removed as a union, so no rule can shadow another and two overlapping rules remove exactly what either removes alone. Order is not part of the semantics, and reversing the rules changes nothing. |
//! | [`each_exclusion_rule_is_tallied_under_its_own_name`] | partition | Every exclusion rule's own reach is counted under its name, so a report can say which rule excused a path. Overlapping coverage makes the tallies sum to more than the union removed, and that excess is the permitted redundancy made visible rather than a double count of the universe. |
//! | [`an_unaccounted_path_is_named_rather_than_assumed`] | partition | A surviving file no inclusion row accounts is a finding naming the path, which is the corpus asking whose it is instead of assuming. The fallback that once answered is gone with nothing put in its place. |
//! | [`exclusivity_is_stated_at_the_row`] | partition | Exclusivity is stated at the row and not at the owner: two rows matching one file is a defect even when both name the same owner. An owner repeats in the relation only to express disjoint pieces of its set. |
//! | [`an_exclusion_and_inclusion_overlap_is_not_double_accounting`] | partition | An overlap between an exclusion rule and an inclusion row is legal, because a path the exclusion relation removed is never evaluated against the inclusion relation at all. That is what keeps the two relations from having to be disjoint from each other. |
//! | [`attribution_follows_the_one_accounting_row`] | partition | A path exactly one row accounts is attributed to that row's owner, and a path the verdict has already named — unaccounted or multiply accounted — is attributed to nobody rather than guessed at. |

use std::collections::BTreeMap;
use std::path::Path;

use tempfile::TempDir;

use crate::finding::Finding;
use crate::partition::{retiring_attribution as attribution, retiring_verify as verify};
use crate::pattern::BytePath;
use crate::plan::{CorpusPlan, TopologyDefect};
use crate::snapshot::{Configuration, Snapshot, configuration};
use crate::test_support::initialise_and_track;
use crate::universe::UniverseKind;

/// Write a tree of files, creating every parent directory each one needs.
fn tree(files: &[(&str, &str)]) -> TempDir {
    let root = TempDir::new().expect("a temporary root");

    write_all(root.path(), files);
    initialise_and_track(root.path());

    root
}

/// Write the given files under a root, creating every parent directory.
fn write_all(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }

        std::fs::write(path, text).expect("a file");
    }
}

/// Load a snapshot whose owner file and shape document carry these relations.
fn snapshot(root: &Path, partitions: &str, ignore: &str) -> Snapshot {
    let directory = root.join(".linter");
    std::fs::create_dir_all(&directory).expect("the declaration directory");

    std::fs::write(
            directory.join("owners.toml"),
            format!(
                "namespace = \"com.torrust.index.linter.owners\"\nversion = [1, 0, 0]\n\nowners = [\"INDEX\", \"ASSAYER\"]\npartitions = [{partitions}]\nmay_cite = []\n"
            ),
        )
        .expect("the owner file");
    std::fs::write(
        directory.join("environments.toml"),
        "namespace = \"com.torrust.index.linter.environments\"\nversion = [1, 0, 0]\n\nenvironments = []\n",
    )
    .expect("the environment file");
    std::fs::write(
        directory.join("policies.toml"),
        "namespace = \"com.torrust.index.linter.policies\"\nversion = [1, 0, 0]\n\npolicies = []\n",
    )
    .expect("the activation file");
    std::fs::write(
        directory.join("lists.toml"),
        "namespace = \"com.torrust.index.linter.lists\"\nversion = [1, 0, 0]\n",
    )
    .expect("the list file");
    std::fs::write(
            directory.join("shape.toml"),
            format!(
                "namespace = \"com.torrust.index.linter.shape\"\nversion = [1, 0, 0]\n\nuniverse = \"git-tracked\"\n\nignore = [{ignore}]\n"
            ),
        )
        .expect("the shape document");

    let Configuration::Present(snapshot) = configuration(root) else {
        panic!("expected the snapshot to load");
    };

    *snapshot
}

/// The accounting universe of a fixture root.
///
/// The declaration directory a fixture writes after `tree` is never tracked,
/// so it is absent from the universe without having to be filtered out of
/// one — which is the point of reading the tracked corpus rather than the
/// working copy.
fn surviving(root: &Path) -> Vec<BytePath> {
    let corpus = CorpusPlan::compile(root, UniverseKind::GitTracked, &[]).expect("text topology");

    assert!(
        corpus.findings().is_empty(),
        "the listing failed: {:?}",
        corpus.findings()
    );

    corpus.base().to_vec()
}

/// The universe carries every tracked path at every depth and nothing else:
/// an empty directory contributes nothing, and no ignore-file, hidden-path
/// or generated-artifact convention removes a tracked entry. An implicit
/// exclusion would be an ownership decision nobody ratified, so a path the
/// repository committed stays in the universe even where an ignore rule
/// names it.
///
/// ´claim:partition:the-universe-removes-nothing-implicitly´
/// ´test:crate:the-universe-removes-nothing-implicitly´
#[test]
fn the_universe_removes_nothing_implicitly() {
    // The ignore file names the generated directory, and the artifact under
    // it is committed anyway. Both conventions that might have excused it —
    // ignored, and generated — leave it exactly where it is.
    let root = tree(&[
        ("src/lib.rs", ""),
        ("src/deep/nested/file.rs", ""),
        (".gitignore", "target/\n"),
        (".hidden", ""),
        ("target/debug/artifact", ""),
    ]);

    std::fs::create_dir(root.path().join("empty")).expect("an empty directory");

    let paths: Vec<String> = surviving(root.path())
        .iter()
        .map(BytePath::display)
        .collect();

    assert_eq!(
        paths,
        vec![
            ".gitignore",
            ".hidden",
            "src/deep/nested/file.rs",
            "src/lib.rs",
            "target/debug/artifact",
        ]
    );
}

/// The version-control store is absent from the universe because git does
/// not track its own store, and the named rule that excused it stands and
/// reaches nothing. An exclusion rule matching no path is legal: keeping it
/// records that the exclusion was decided rather than inherited from
/// whatever enumerates the tree, and a universe that later carried the store
/// would find the rule already waiting.
///
/// ´claim:partition:the-version-control-store-is-absent-and-its-rule-is-idle´
/// ´test:crate:the-version-control-store-is-absent-and-its-rule-is-idle´
#[test]
fn the_version_control_store_is_absent_and_its_rule_is_idle() {
    let root = tree(&[("src/lib.rs", "")]);

    assert!(
        root.path().join(".git").is_dir(),
        "the store stands on disk, which is what makes its absence a fact about the universe"
    );

    let paths = surviving(root.path());

    assert!(
        !paths
            .iter()
            .any(|path| path.as_bytes().starts_with(b".git/")),
        "the store is not tracked content: {:?}",
        paths.iter().map(BytePath::display).collect::<Vec<String>>()
    );

    let vcs = "{ name = \"vcs-metadata\", pattern = '%s\".git\" [ \"/\" *VCHAR ]' }";
    let (counts, findings) = verify(
        &snapshot(
            root.path(),
            "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }",
            vcs,
        ),
        &paths,
    );

    assert_eq!(counts.universe, 1);
    assert_eq!(
        counts.excluded, 0,
        "the rule is idle rather than removing anything"
    );
    assert!(
        counts.excluded_by.is_empty(),
        "an idle rule tallies nothing"
    );
    assert_eq!(
        counts.accounted, 1,
        "and the partition is total without its help"
    );
    assert!(findings.is_empty(), "{findings:?}");
}

/// A root whose tracked corpus cannot be taken is a traversal failure naming
/// the root, not an empty universe. An empty universe would let totality
/// pass for the wrong reason: with nothing to account, every inclusion row
/// is vacuously total and the check would report a clean partition of a
/// repository it never read.
///
/// ´claim:partition:a-corpus-that-cannot-be-taken-is-a-traversal-failure´
/// ´test:crate:a-corpus-that-cannot-be-taken-is-a-traversal-failure´
#[test]
fn a_corpus_that_cannot_be_taken_is_a_traversal_failure() {
    let root = tree(&[("src/lib.rs", "")]);
    let absent = root.path().join("no-such-directory");

    let corpus =
        CorpusPlan::compile(&absent, UniverseKind::GitTracked, &[]).expect("empty failed topology");
    let paths = corpus.base();
    let findings = corpus.findings();

    assert!(paths.is_empty(), "nothing was read, so nothing is claimed");
    assert_eq!(findings.len(), 1, "{findings:?}");

    let [Finding::TraversalFailure { path, message }] = findings else {
        panic!("expected the failure to be reported as a traversal failure, got {findings:?}");
    };

    assert_eq!(
        path,
        &absent.display().to_string(),
        "the root is what failed"
    );
    assert!(
        message.starts_with("git ls-files: "),
        "git's own account of it is carried rather than paraphrased: {message}"
    );
}

/// Untracked working-copy content is not the repository and never enters the
/// accounting universe. A checkout carries material nobody committed — build
/// output, local configuration, a data directory the program filled at
/// runtime — and a universe read from the physical tree made the partition
/// verdict a property of whoever ran the check rather than of the commit.
///
/// ´claim:partition:untracked-working-copy-content-is-not-the-repository´
/// ´test:crate:untracked-working-copy-content-is-not-the-repository´
#[test]
fn untracked_working_copy_content_is_not_the_repository() {
    let root = tree(&[("src/lib.rs", ""), ("adr/001-one.md", "")]);

    // The shape of a real working checkout: local configuration, an
    // environment file, an editor dropping, and a runtime data directory
    // holding more files than the repository itself.
    write_all(
        root.path(),
        &[
            (".env", "SECRET=1"),
            (".claude/settings.json", "{}"),
            ("src/lib.rs.swp", ""),
            ("storage/database/one.db", ""),
            ("storage/database/two.db", ""),
            ("storage/uploads/deep/three.bin", ""),
        ],
    );

    let paths: Vec<String> = surviving(root.path())
        .iter()
        .map(BytePath::display)
        .collect();

    assert_eq!(
        paths,
        vec!["adr/001-one.md", "src/lib.rs"],
        "the universe is the tracked corpus, and none of the litter is in it"
    );

    // And the verdict is the one the tracked corpus earns: the rows that
    // account the tracked tree leave nothing unaccounted, however much
    // untracked material stands beside it.
    let partitions = "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }, { name = \"decision-records\", owner = \"INDEX\", pattern = '%s\"adr\" [ \"/\" *VCHAR ]' }";
    let paths = surviving(root.path());
    let (counts, findings) = verify(&snapshot(root.path(), partitions, ""), &paths);

    assert_eq!(counts.universe, 2);
    assert_eq!(counts.accounted, 2);
    assert_eq!(counts.unaccounted, 0);
    assert!(findings.is_empty(), "{findings:?}");
}

/// A file whose name is not text refuses the topology once, carrying the
/// reversible byte display rather than disappearing through lossy conversion.
///
/// ´claim:partition:a-path-that-is-not-text-refuses-topology´
/// ´test:crate:a-path-that-is-not-text-refuses-topology´
#[cfg(unix)]
#[test]
fn a_path_that_is_not_text_refuses_topology() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = TempDir::new().expect("a temporary root");
    std::fs::write(root.path().join(OsStr::from_bytes(b"od\xffd")), "").expect("an awkward file");
    initialise_and_track(root.path());

    let defects = CorpusPlan::compile(root.path(), UniverseKind::GitTracked, &[])
        .expect_err("non-text topology refuses");

    assert_eq!(defects.len(), 1);
    assert!(matches!(
        defects.as_slice(),
        [TopologyDefect::NonTextPath(_)]
    ));
    assert_eq!(
        defects[0].to_string(),
        "od%FFd: the path is not text, so no pattern decides it"
    );
}

/// The excluded set is removed as a union, so no rule can shadow another and
/// two overlapping rules remove exactly what either removes alone. Order is
/// not part of the semantics, and reversing the rules changes nothing.
///
/// ´claim:partition:the-excluded-set-is-an-orderless-union´
/// ´test:crate:the-excluded-set-is-an-orderless-union´
#[test]
fn the_excluded_set_is_an_orderless_union() {
    let root = tree(&[("a/b/c.rs", ""), ("a/d.rs", ""), ("src/lib.rs", "")]);
    let partitions =
        "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }";

    let wide = "{ name = \"wide\", pattern = '%s\"a\" [ \"/\" *VCHAR ]' }";
    let narrow = "{ name = \"narrow\", pattern = '%s\"a/b\" [ \"/\" *VCHAR ]' }";

    let forward = format!("{wide}, {narrow}");
    let backward = format!("{narrow}, {wide}");

    for excluded in [&forward, &backward] {
        let root = tree(&[("a/b/c.rs", ""), ("a/d.rs", ""), ("src/lib.rs", "")]);
        let paths = surviving(root.path());
        let (counts, findings) = verify(&snapshot(root.path(), partitions, excluded), &paths);

        assert_eq!(counts.excluded, 2);
        assert_eq!(counts.surviving, 1);
        assert_eq!(counts.accounted, 1);
        assert!(findings.is_empty(), "{findings:?}");
    }

    drop(root);
}

/// Every exclusion rule's own reach is counted under its name, so a report
/// can say which rule excused a path. Overlapping coverage makes the tallies
/// sum to more than the union removed, and that excess is the permitted
/// redundancy made visible rather than a double count of the universe.
///
/// ´claim:partition:each-exclusion-rule-is-tallied-under-its-own-name´
/// ´test:crate:each-exclusion-rule-is-tallied-under-its-own-name´
#[test]
fn each_exclusion_rule_is_tallied_under_its_own_name() {
    let root = tree(&[("a/b/c.rs", ""), ("a/d.rs", ""), ("src/lib.rs", "")]);
    let paths = surviving(root.path());
    let excluded = "{ name = \"wide\", pattern = '%s\"a\" [ \"/\" *VCHAR ]' }, \
             { name = \"narrow\", pattern = '%s\"a/b\" [ \"/\" *VCHAR ]' }";
    let (counts, findings) = verify(
        &snapshot(
            root.path(),
            "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }",
            excluded,
        ),
        &paths,
    );

    // Two paths left the universe, and the tallies name which rule excused
    // each: the wide rule reached both, the narrow one reached the nested
    // file as well. Three is larger than two by exactly the overlap.
    assert_eq!(counts.excluded, 2);
    assert_eq!(
        counts.excluded_by,
        BTreeMap::from([(String::from("wide"), 2), (String::from("narrow"), 1)])
    );
    assert_eq!(counts.excluded_by.values().sum::<usize>(), 3);
    assert!(findings.is_empty(), "{findings:?}");

    // A rule that excuses nothing is absent from the tally rather than
    // present at zero, because the map answers which rule excused a path.
    let quiet = "{ name = \"quiet\", pattern = '%s\"nothing\" [ \"/\" *VCHAR ]' }";
    let (counts, _findings) = verify(
        &snapshot(
            root.path(),
            "{ name = \"two-trees\", owner = \"INDEX\", pattern = '( %s\"src\" / %s\"a\" ) [ \"/\" *VCHAR ]' }",
            quiet,
        ),
        &paths,
    );

    assert_eq!(counts.excluded, 0);
    assert!(counts.excluded_by.is_empty());
}

/// A surviving file no inclusion row accounts is a finding naming the path,
/// which is the corpus asking whose it is instead of assuming. The fallback
/// that once answered is gone with nothing put in its place.
///
/// ´claim:partition:an-unaccounted-path-is-named-rather-than-assumed´
/// ´test:crate:an-unaccounted-path-is-named-rather-than-assumed´
#[test]
fn an_unaccounted_path_is_named_rather_than_assumed() {
    let root = tree(&[("src/lib.rs", ""), ("stray.md", "")]);
    let paths = surviving(root.path());
    let (counts, findings) = verify(
        &snapshot(
            root.path(),
            "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }",
            "",
        ),
        &paths,
    );

    assert_eq!(counts.unaccounted, 1);
    assert_eq!(counts.accounted, 1);
    assert_eq!(
        findings,
        vec![Finding::UnaccountedPath {
            path: String::from("stray.md")
        }]
    );
    assert_eq!(
        findings[0].to_string(),
        "owner partition: stray.md: unaccounted after exclusion preprocessing"
    );
}

/// Exclusivity is stated at the row and not at the owner: two rows matching
/// one file is a defect even when both name the same owner. An owner repeats
/// in the relation only to express disjoint pieces of its set.
///
/// ´claim:partition:exclusivity-is-stated-at-the-row´
/// ´test:crate:exclusivity-is-stated-at-the-row´
#[test]
fn exclusivity_is_stated_at_the_row() {
    let root = tree(&[("src/lib.rs", "")]);
    let paths = surviving(root.path());
    let partitions = "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }, { name = \"crate-root\", owner = \"INDEX\", pattern = '%s\"src/lib.rs\"' }";
    let (counts, findings) = verify(&snapshot(root.path(), partitions, ""), &paths);

    assert_eq!(counts.multiply_accounted, 1);
    assert_eq!(counts.accounted, 0);
    assert_eq!(
        findings[0].to_string(),
        "owner partition: src/lib.rs: matched 2 inclusion rows after exclusion preprocessing: \
             crate-root : INDEX : %s\"src/lib.rs\", crate-sources : INDEX : %s\"src\" [ \"/\" *VCHAR ]"
    );
}

/// An overlap between an exclusion rule and an inclusion row is legal,
/// because a path the exclusion relation removed is never evaluated against
/// the inclusion relation at all. That is what keeps the two relations from
/// having to be disjoint from each other.
///
/// ´claim:partition:an-exclusion-and-inclusion-overlap-is-not-double-accounting´
/// ´test:crate:an-exclusion-and-inclusion-overlap-is-not-double-accounting´
#[test]
fn an_exclusion_and_inclusion_overlap_is_not_double_accounting() {
    let root = tree(&[("src/lib.rs", ""), ("src/generated.rs", "")]);
    let paths = surviving(root.path());
    let (counts, findings) = verify(
        &snapshot(
            root.path(),
            "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }",
            "{ name = \"generated\", pattern = '%s\"src/generated.rs\"' }",
        ),
        &paths,
    );

    assert_eq!(counts.excluded, 1);
    assert_eq!(counts.accounted, 1);
    assert!(findings.is_empty(), "{findings:?}");
}

/// A path exactly one row accounts is attributed to that row's owner, and a
/// path the verdict has already named — unaccounted or multiply accounted —
/// is attributed to nobody rather than guessed at.
///
/// ´claim:partition:attribution-follows-the-one-accounting-row´
/// ´test:crate:attribution-follows-the-one-accounting-row´
#[test]
fn attribution_follows_the_one_accounting_row() {
    let root = tree(&[
        ("src/lib.rs", ""),
        ("packages/assayer/src/lib.rs", ""),
        ("stray.md", ""),
    ]);
    let paths = surviving(root.path());
    let partitions = "{ name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }, { name = \"assayer-package\", owner = \"ASSAYER\", pattern = '%s\"packages/assayer\" [ \"/\" *VCHAR ]' }";
    let snapshot = snapshot(root.path(), partitions, "");

    let attributed = attribution(&snapshot, &paths);
    let owners: Vec<(String, &str)> = attributed
        .into_iter()
        .map(|(path, owner)| (path.display(), owner))
        .collect();

    assert_eq!(
        owners,
        vec![
            (String::from("packages/assayer/src/lib.rs"), "ASSAYER"),
            (String::from("src/lib.rs"), "INDEX"),
        ]
    );
}
