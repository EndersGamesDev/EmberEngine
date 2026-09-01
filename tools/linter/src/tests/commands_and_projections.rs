// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Check-command and generated-projection lifecycle tests.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reports_a_clean_carrier`] | report | A corpus in good standing reports itself as clean, carrying the counts a reader wants — sources scanned, mints, citations resolved — and no findings at all. A tree of prose alone covers no assets, which is a count of zero rather than a defect. |
//! | [`carries_the_profile_counts_beside_the_prose_counts`] | report | The report carries the inventory profile's counts beside the prose ones, in the structure and in the serialised form alike: how many tests are covered, how many labelled, how many missing, and the split by area. A covered test carrying no label fails the run. |
//! | [`counts_both_projections_without_writing_anything`] | projection | Projecting without asking for a write counts what would change and touches nothing: both projections are considered, the work each needs is reported, and the sources on disk are byte-for-byte what they were. |
//! | [`bootstraps_both_projections_on_a_fixture_tree`] | projection | The first write creates both projections from nothing: the in-file index gains its header and a row per test carrying the author's own gloss, with a citing test rendered as the citation itself; the folder matrix gains its titled head and its rows. A folder with no tests says so in words, because emptiness written down is emptiness a reader can scan. |
//! | [`settles_after_one_bootstrap`] | projection | Projection settles after one pass: a second write rewrites nothing and bootstraps nothing, reporting every projection unchanged, and a verification afterwards agrees and finds no failure. Regenerating is therefore safe to run at any time and produces no churn. |
//! | [`regenerates_a_projection_a_hand_edited`] | projection | A projection edited by hand is staleness: verification reports it as a failure, and a write puts back what the labels say. The generated tables are owned by the labels rather than by whoever last typed in them, so a hand-edit is undone rather than honoured. |
//! | [`settles_every_projection_and_leaves_the_tree_byte_for_byte`] | projection | All three projections settle together, and settling is a statement about the tree's bytes: a second write rewrites nothing and bootstraps nothing, the constant sweep included, and every file of the tree is byte for byte what the first write left. The verify path afterwards agrees — no failure, the same counts, and no side effect of its own — so a check run after a write means something for each of the three. |
//! | [`restores_a_hand_edited_test_index_byte_for_byte`] | projection | An in-file test index edited by hand is staleness the verify path reports and a write undoes to the byte. The cell edited is inside the generated region and nowhere else, so what is restored is the generator's own prior output rather than a rederivation from an author's altered gloss: the region belongs to the labels, and a hand-edit inside it is undone. |
//! | [`flags_and_restores_a_hand_edited_constant_pin`] | projection | A constant pin edited by hand is staleness on the same terms as a projection's: the verify path reports it and a write restores the derived pin to the byte. The constant sweep is the third projection and had no coverage at the tree at all — every test of this command ran it over a tree carrying no constant — so a value and its pin could drift apart with the command reporting nothing. |
//! | [`leaves_an_unsubscribed_owners_share_uncensused`] | projection | An owner that has not activated the projection family contributes no figure to any of the three sweeps: its covered sources, its folders and its covered constants are absent from the counts, not counted and excused. The same tree projected under an explicit wildcard snapshot counts both packages, so what removes the quiet share is the narrower declared subscription rather than anything about the tree. |
//! | [`writes_nothing_into_an_unsubscribed_owners_share`] | projection | A write under the declared surface leaves an unsubscribed owner's share byte for byte as it stood: no readme is bootstrapped into it, no index is injected, no pin is written — while the subscribed share gains its projections in the same run. |
//! | [`leaves_an_artifact_standing_in_an_unsubscribed_share_unread`] | projection | A projection artifact already standing in an unsubscribed owner's share is simply not read: staleness in it is neither reported nor repaired, and a write leaves it exactly as it stands. An unrouted run over the same tree reports the staleness, so what silences it is the subscription and nothing else. |

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::report::{check, project_with};
use crate::test_support::{declare_label_surface, initialise_and_track};

/// A corpus in good standing reports itself as clean, carrying the counts a
/// reader wants — sources scanned, mints, citations resolved — and no
/// findings at all. A tree of prose alone covers no assets, which is a count
/// of zero rather than a defect.
///
/// ´claim:report:a-corpus-in-good-standing-reports-clean-with-its-counts´
/// ´test:crate:reports-a-clean-carrier´
#[test]
fn reports_a_clean_carrier() {
    let root = tempfile::tempdir().expect("temporary directory");
    fs::create_dir_all(root.path().join("adr")).expect("create");
    fs::write(
        root.path().join("adr/001-one.md"),
        "## Head · `sec:demo:head`\n\nCites (`sec:demo:head`).\n",
    )
    .expect("write");
    declare_label_surface(root.path());
    initialise_and_track(root.path());

    let report = check(root.path());

    assert!(report.clean, "findings: {:?}", report.findings);
    assert_eq!(report.sources_scanned, 1);
    assert_eq!(report.mints, 1);
    assert_eq!(report.citations_resolved, 1);
    assert_eq!(report.findings.len(), 0);
    assert_eq!(
        report.profile.covered, 0,
        "a prose-only tree covers no assets"
    );
}

/// The report carries the inventory profile's counts beside the prose ones,
/// in the structure and in the serialised form alike: how many tests are
/// covered, how many labelled, how many missing, and the split by area. A
/// covered test carrying no label fails the run.
///
/// ´claim:report:the-profile-counts-stand-beside-the-prose-counts´
/// ´test:crate:carries-the-profile-counts-beside-the-prose-counts´
#[test]
fn carries_the_profile_counts_beside_the_prose_counts() {
    let root = tempfile::tempdir().expect("temporary directory");
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\".\"]\n\n[package]\nname = \"torrust-demo\"\n",
    )
    .expect("write");
    fs::create_dir_all(root.path().join("src")).expect("create");
    fs::write(
        root.path().join("src/lib.rs"),
        "/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n\n#[test]\nfn bare() {}\n",
    )
    .expect("write");
    declare_label_surface(root.path());
    initialise_and_track(root.path());

    let report = check(root.path());
    let json = serde_json::to_value(&report).expect("serializable");

    assert_eq!(report.profile.covered, 2);
    assert_eq!(report.profile.labelled, 1);
    assert_eq!(report.profile.missing, 1);
    assert_eq!(report.profile.by_area["unit"], 2);
    assert!(!report.clean, "an unlabelled test is a failure");
    assert_eq!(json["profile"]["covered"], 2);
    assert_eq!(json["findings"][0]["code"], "missing_inventory_label");
}

/// A one-package fixture tree: two claimed tests, and a folder with none.
/// The reconciliation these projection fixtures are named by.
fn projection_names() -> crate::roster::OwnerNames {
    crate::roster::OwnerNames::new("torrust-", [])
}

fn fixture_tree() -> tempfile::TempDir {
    let acute = '\u{b4}';
    let root = tempfile::tempdir().expect("temporary directory");

    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"packages/demo\"]\n",
    )
    .expect("write");
    fs::create_dir_all(root.path().join("packages/demo/src/tests")).expect("create");
    fs::create_dir_all(root.path().join("packages/demo/src/maths")).expect("create");
    fs::write(
        root.path().join("packages/demo/Cargo.toml"),
        "[package]\nname = \"torrust-demo\"\n",
    )
    .expect("write");
    fs::write(
        root.path().join("packages/demo/src/tests/resonance.rs"),
        format!(
            "/// The widths are identical across the sweep.\n///\n\
                 /// {acute}claim:resonance:crossover-widths{acute}\n\
                 /// {acute}test:crate:widths-are-identical{acute}\n\
                 #[test]\nfn widths_are_identical() {{}}\n\n\
                 /// ({acute}claim:resonance:crossover-widths{acute})\n\
                 /// {acute}test:crate:widths-survive-the-surface{acute}\n\
                 #[test]\nfn widths_survive_the_surface() {{}}\n"
        ),
    )
    .expect("write");
    fs::write(
        root.path().join("packages/demo/src/maths/norms.rs"),
        "pub fn norm() {}\n",
    )
    .expect("write");
    initialise_and_track(root.path());

    root
}

/// Projecting without asking for a write counts what would change and
/// touches nothing: both projections are considered, the work each needs is
/// reported, and the sources on disk are byte-for-byte what they were.
///
/// ´claim:projection:projecting-without-a-write-counts-and-changes-nothing´
/// ´test:crate:counts-both-projections-without-writing-anything´
#[test]
fn counts_both_projections_without_writing_anything() {
    let root = fixture_tree();
    let before =
        fs::read_to_string(root.path().join("packages/demo/src/tests/resonance.rs")).expect("read");

    let report = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        report.index.considered, 1,
        "one source carries a covered test"
    );
    assert_eq!(report.index.bootstrapped, 1, "and carries no index yet");
    assert_eq!(
        report.matrix.considered, 2,
        "two folders carry a Rust source"
    );
    assert_eq!(report.matrix.bootstrapped, 2);
    assert_eq!(
        report.failures, 0,
        "the staging reports nothing: {:?}",
        report.findings
    );
    assert_eq!(
        fs::read_to_string(root.path().join("packages/demo/src/tests/resonance.rs")).expect("read"),
        before,
        "the default mode forms no side effect"
    );
}

/// The first write creates both projections from nothing: the in-file index
/// gains its header and a row per test carrying the author's own gloss,
/// with a citing test rendered as the citation itself; the folder matrix
/// gains its titled head and its rows. A folder with no tests says so in
/// words, because emptiness written down is emptiness a reader can scan.
///
/// ´claim:projection:the-first-write-creates-both-projections-from-the-labels´
/// ´test:crate:bootstraps-both-projections-on-a-fixture-tree´
#[test]
fn bootstraps_both_projections_on_a_fixture_tree() {
    let root = fixture_tree();

    let report = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(report.index.bootstrapped, 1);
    assert_eq!(report.matrix.bootstrapped, 2);
    assert_eq!(report.failures, 0, "findings: {:?}", report.findings);

    let source =
        fs::read_to_string(root.path().join("packages/demo/src/tests/resonance.rs")).expect("read");

    assert!(source.contains("//! | Test | Area | Claim |"), "{source}");
    assert!(
        source.contains("//! | [`widths_are_identical`] | resonance | The widths are identical across the sweep. |"),
        "{source}"
    );
    assert!(
        source.contains(
            "//! | [`widths_survive_the_surface`] | resonance | cites (\u{b4}claim:resonance:crossover-widths\u{b4}) |"
        ),
        "{source}"
    );

    let matrix =
        fs::read_to_string(root.path().join("packages/demo/src/tests/README.md")).expect("read");

    assert!(
        matrix.starts_with("## Crate test matrix · `tab:demo:crate-test-matrix`\n"),
        "{matrix}"
    );
    assert!(
        matrix.contains("| (`test:crate:widths-are-identical`) | resonance |"),
        "{matrix}"
    );

    let empty =
        fs::read_to_string(root.path().join("packages/demo/src/maths/README.md")).expect("read");

    assert!(
        empty.contains("No unit tests in this folder."),
        "emptiness that is written down is scannable: {empty}"
    );
}

/// Projection settles after one pass: a second write rewrites nothing and
/// bootstraps nothing, reporting every projection unchanged, and a
/// verification afterwards agrees and finds no failure. Regenerating is
/// therefore safe to run at any time and produces no churn.
///
/// ´claim:projection:projection-settles-after-one-pass´
/// ´test:crate:settles-after-one-bootstrap´
#[test]
fn settles_after_one_bootstrap() {
    let root = fixture_tree();

    let _first = project_with(root.path(), Some(&projection_names()), &[], None, true);
    let second = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(second.index.rewritten, 0);
    assert_eq!(second.index.bootstrapped, 0);
    assert_eq!(second.matrix.rewritten, 0);
    assert_eq!(second.matrix.bootstrapped, 0);
    assert_eq!(second.index.unchanged, 1);
    assert_eq!(second.matrix.unchanged, 2);

    let verified = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        verified.failures, 0,
        "and the check agrees: {:?}",
        verified.findings
    );
    assert_eq!(verified.index.unchanged, 1);
    assert_eq!(verified.matrix.unchanged, 2);
}

/// A projection edited by hand is staleness: verification reports it as a
/// failure, and a write puts back what the labels say. The generated tables
/// are owned by the labels rather than by whoever last typed in them, so a
/// hand-edit is undone rather than honoured.
///
/// ´claim:projection:a-hand-edited-projection-is-stale-and-is-regenerated´
/// ´test:crate:regenerates-a-projection-a-hand-edited´
#[test]
fn regenerates_a_projection_a_hand_edited() {
    let root = fixture_tree();
    let readme = root.path().join("packages/demo/src/tests/README.md");

    let _first = project_with(root.path(), Some(&projection_names()), &[], None, true);

    let written = fs::read_to_string(&readme).expect("read");
    fs::write(&readme, written.replace("resonance", "decay")).expect("write");

    let reported = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(reported.matrix.rewritten, 1, "the hand-edit is staleness");
    assert_eq!(reported.failures, 1);

    let repaired = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(repaired.matrix.rewritten, 1, "and the write repairs it");
    assert!(
        fs::read_to_string(&readme)
            .expect("read")
            .contains("| resonance |"),
        "the labels decide the cells"
    );
    assert_eq!(
        fs::read_to_string(&readme).expect("read"),
        written,
        "and repairs it to the byte, not merely to a readme carrying the right cell"
    );
    assert_eq!(
        project_with(root.path(), Some(&projection_names()), &[], None, false).failures,
        0,
        "and the check is clean again"
    );
}

/// Every file of a tree, keyed by its path relative to the root.
///
/// A projection's no-op is a statement about the whole tree rather than about
/// the counters a run reports, so the comparison is made over the bytes: a
/// generator that rewrote a file to identical content would still be a
/// generator that touched it, and one that quietly rewrote a file its counters
/// do not mention would pass every count this suite asserts.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read the directory") {
            let path = entry.expect("an entry").path();

            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_path_buf();

                files.insert(relative, fs::read(&path).expect("read"));
            }
        }
    }

    files
}

/// The fixture tree, with one adopted constant standing in its maths folder.
///
/// The tree the other projection tests use carries no constant at all, so the
/// third sweep of ADR-T-018 ran over nothing in every one of them and its
/// outcome was never asserted. This tree gives it something to pin.
fn constant_tree() -> tempfile::TempDir {
    let acute = '\u{b4}';
    let root = fixture_tree();

    fs::write(
        root.path().join("packages/demo/src/maths/norms.rs"),
        format!(
            "/// Caps the width of a norm.\n///\n\
                 /// Bounded by the measurement ({acute}sec:demo:witness{acute}).\n\
                 /// {acute}const:demo:norm-width{acute} ({acute}[INDEX-alg:const:count]{acute})\n\
                 const WIDTH: usize = 32;\n\npub fn norm() {{}}\n"
        ),
    )
    .expect("write");

    root
}

/// All three projections settle together, and settling is a statement about
/// the tree's bytes: a second write rewrites nothing and bootstraps nothing,
/// the constant sweep included, and every file of the tree is byte for byte
/// what the first write left. The verify path afterwards agrees — no failure,
/// the same counts, and no side effect of its own — so a check run after a
/// write means something for each of the three.
///
/// ´claim:projection:all-three-projections-settle-and-the-tree-stops-changing´
/// ´test:crate:settles-every-projection-and-leaves-the-tree-byte-for-byte´
#[test]
fn settles_every_projection_and_leaves_the_tree_byte_for_byte() {
    let root = constant_tree();

    let first = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(
        first.constant.considered, 1,
        "the adopted constant is covered: {:?}",
        first.findings
    );
    assert_eq!(first.constant.bootstrapped, 1, "and its pin was owed");
    assert_eq!(
        first.failures, 0,
        "the first write leaves nothing failing: {:?}",
        first.findings
    );

    let settled = snapshot(root.path());
    let second = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(second.index.rewritten + second.index.bootstrapped, 0);
    assert_eq!(second.matrix.rewritten + second.matrix.bootstrapped, 0);
    assert_eq!(second.constant.rewritten + second.constant.bootstrapped, 0);
    assert_eq!(
        second.constant.unchanged, 1,
        "the pin the first write wrote stands"
    );
    assert_eq!(
        snapshot(root.path()),
        settled,
        "a second write changes no byte of the tree"
    );

    let verified = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        verified.failures, 0,
        "and the verify path agrees: {:?}",
        verified.findings
    );
    assert_eq!(verified.index.unchanged, 1);
    assert_eq!(verified.matrix.unchanged, 2);
    assert_eq!(verified.constant.unchanged, 1);
    assert_eq!(
        snapshot(root.path()),
        settled,
        "and forms no side effect of its own"
    );
}

/// An in-file test index edited by hand is staleness the verify path reports
/// and a write undoes to the byte. The cell edited is inside the generated
/// region and nowhere else, so what is restored is the generator's own prior
/// output rather than a rederivation from an author's altered gloss: the
/// region belongs to the labels, and a hand-edit inside it is undone.
///
/// ´claim:projection:a-hand-edited-test-index-is-flagged-and-restored-to-the-byte´
/// ´test:crate:restores-a-hand-edited-test-index-byte-for-byte´
#[test]
fn restores_a_hand_edited_test_index_byte_for_byte() {
    let root = constant_tree();
    let source = root.path().join("packages/demo/src/tests/resonance.rs");

    let _first = project_with(root.path(), Some(&projection_names()), &[], None, true);
    let written = fs::read_to_string(&source).expect("read");
    let edited = written.replace(
        "| resonance | The widths are identical across the sweep. |",
        "| decay | The widths are identical across the sweep. |",
    );

    assert_ne!(
        edited, written,
        "the edit lands inside the generated region"
    );
    fs::write(&source, edited).expect("write");

    let reported = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        reported.index.rewritten, 1,
        "the verify path sees the staleness"
    );
    assert_eq!(
        reported.failures, 1,
        "and reports it: {:?}",
        reported.findings
    );

    let repaired = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(repaired.index.rewritten, 1, "and the write undoes it");
    assert_eq!(
        fs::read_to_string(&source).expect("read"),
        written,
        "restoring the bytes the generator wrote"
    );
    assert_eq!(
        project_with(root.path(), Some(&projection_names()), &[], None, false).failures,
        0,
        "and the check is clean again"
    );
}

/// A constant pin edited by hand is staleness on the same terms as a
/// projection's: the verify path reports it and a write restores the derived
/// pin to the byte. The constant sweep is the third projection and had no
/// coverage at the tree at all — every test of this command ran it over a tree
/// carrying no constant — so a value and its pin could drift apart with the
/// command reporting nothing.
///
/// ´claim:projection:a-hand-edited-constant-pin-is-flagged-and-restored-to-the-byte´
/// ´test:crate:flags-and-restores-a-hand-edited-constant-pin´
#[test]
fn flags_and_restores_a_hand_edited_constant_pin() {
    let root = constant_tree();
    let source = root.path().join("packages/demo/src/maths/norms.rs");

    let _first = project_with(root.path(), Some(&projection_names()), &[], None, true);
    let written = fs::read_to_string(&source).expect("read");

    assert!(
        written.contains("const:demo:norm-width-count-32"),
        "the sweep pinned the value it found: {written}"
    );

    fs::write(
        &source,
        written.replace("norm-width-count-32", "norm-width-count-16"),
    )
    .expect("write");

    let reported = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        reported.constant.rewritten, 1,
        "the verify path sees the stale pin"
    );
    assert_eq!(
        reported.failures, 1,
        "and reports it: {:?}",
        reported.findings
    );

    let repaired = project_with(root.path(), Some(&projection_names()), &[], None, true);

    assert_eq!(repaired.constant.rewritten, 1, "and the write undoes it");
    assert_eq!(
        fs::read_to_string(&source).expect("read"),
        written,
        "restoring the pin the value derives, and nothing else"
    );
    assert_eq!(
        project_with(root.path(), Some(&projection_names()), &[], None, false).failures,
        0,
        "and the check is clean again"
    );
}

/// The routed fixture: the constant tree joined by a second package whose
/// owner subscribes to nothing, under a declared surface subscribing only
/// the first to the projection family.
///
/// The quiet package mirrors the demo one — a covered test and an adopted
/// constant — so every figure it would contribute is a figure the routing
/// must remove rather than one that was never there.
fn routed_tree() -> tempfile::TempDir {
    let acute = '\u{b4}';
    let root = constant_tree();

    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"packages/demo\", \"packages/quiet\"]\n",
    )
    .expect("write");
    fs::create_dir_all(root.path().join("packages/quiet/src/tests")).expect("create");
    fs::create_dir_all(root.path().join("packages/quiet/src/maths")).expect("create");
    fs::write(
        root.path().join("packages/quiet/Cargo.toml"),
        "[package]\nname = \"torrust-quiet\"\n",
    )
    .expect("write");
    fs::write(
        root.path().join("packages/quiet/src/tests/hush.rs"),
        format!(
            "/// The hush holds under load.\n///\n\
                 /// {acute}claim:hush:holds-under-load{acute}\n\
                 /// {acute}test:crate:hush-holds-under-load{acute}\n\
                 #[test]\nfn hush_holds_under_load() {{}}\n"
        ),
    )
    .expect("write");
    fs::write(
        root.path().join("packages/quiet/src/maths/still.rs"),
        format!(
            "/// Caps the stillness of a hush.\n///\n\
                 /// Bounded by the measurement ({acute}sec:quiet:witness{acute}).\n\
                 /// {acute}const:quiet:still-width{acute} ({acute}[INDEX-alg:const:count]{acute})\n\
                 const STILL: usize = 16;\n\npub fn still() {{}}\n"
        ),
    )
    .expect("write");
    declare_routed_surface(root.path());
    crate::test_support::track_all(root.path());

    root
}

/// Write the declared surface the routed fixtures read: both owners
/// registered and partitioned, and only DEMO activating the family.
fn declare_routed_surface(root: &Path) {
    let directory = root.join(crate::DIRECTORY);

    fs::create_dir_all(&directory).expect("create the declaration directory");

    let envelope = |schema: &str| {
        format!("namespace = \"com.torrust.index.linter.{schema}\"\nversion = [1, 0, 0]\n\n")
    };
    let owners = "owners = [\"DEMO\", \"QUIET\"]\n\
            partitions = [\n\
              { name = \"demo-package\", owner = \"DEMO\", pattern = '%s\"packages/demo\" [ \"/\" *VCHAR ]' },\n\
              { name = \"quiet-package\", owner = \"QUIET\", pattern = '%s\"packages/quiet\" [ \"/\" *VCHAR ]' },\n\
            ]\n\
            may_cite = []\n";
    let policies = "policies = [\n\
            { owner = \"DEMO\", policy = \"labels.mints-well-formed\" },\n\
            { owner = \"DEMO\", policy = \"profile.tests-conform\" },\n\
            { owner = \"DEMO\", policy = \"profile.constants-conform\" },\n\
            { owner = \"DEMO\", policy = \"projection.test-indexes-current\" },\n\
            { owner = \"DEMO\", policy = \"projection.test-matrices-current\" },\n\
            { owner = \"DEMO\", policy = \"projection.constant-pins-current\" },\n\
            ]\n";
    let lists = "[DEMO.\"labels.mints-well-formed\"]\nallowances = []\n\n\
            [DEMO.\"profile.tests-conform\"]\nallowances = []\n\n\
            [DEMO.\"profile.constants-conform\"]\nallowances = []\n\n\
            [DEMO.\"projection.test-indexes-current\"]\nallowances = []\n\n\
            [DEMO.\"projection.test-matrices-current\"]\nallowances = []\n\n\
            [DEMO.\"projection.constant-pins-current\"]\nallowances = []\n";

    for (file, text) in [
        (
            crate::OWNERS_FILE,
            format!("{}{owners}", envelope("owners")),
        ),
        (
            crate::ENVIRONMENTS_FILE,
            format!(
                "{}environments = [{{ environment = \"Rule\", kind = \"rule\" }}]\n",
                envelope("environments")
            ),
        ),
        (
            crate::POLICIES_FILE,
            format!("{}{policies}", envelope("policies")),
        ),
        (crate::LISTS_FILE, format!("{}{lists}", envelope("lists"))),
        (
            crate::SHAPE_FILE,
            format!(
                "{}universe = \"git-tracked\"\n\nignore = []\n",
                envelope("shape")
            ),
        ),
        (
            "policy-spdx.toml",
            format!(
                "{}[set.identifier]\n\n[set.copyright]\n",
                envelope("policy.spdx")
            ),
        ),
        ("policy-interchange.toml", envelope("policy.interchange")),
        (
            "policy-references.toml",
            envelope("policy.references.path-linking"),
        ),
    ] {
        fs::write(directory.join(file), text).expect("a declared file");
    }
}

/// An owner that has not activated the projection family contributes no
/// figure to any of the three sweeps: its covered sources, its folders and
/// its covered constants are absent from the counts, not counted and
/// excused. The same tree projected under an explicit wildcard snapshot counts
/// both packages, so what removes the quiet share is the narrower declared
/// subscription rather than anything about the tree.
///
/// ´claim:projection:an-unsubscribed-owners-share-is-not-censused´
/// ´test:crate:leaves-an-unsubscribed-owners-share-uncensused´
#[test]
fn leaves_an_unsubscribed_owners_share_uncensused() {
    let root = routed_tree();
    let configured = crate::configuration(root.path());
    let declared = configured.snapshot().expect("the declared surface loads");

    let unrouted = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        unrouted.index.considered, 2,
        "the wildcard snapshot counts both sources"
    );
    assert_eq!(
        unrouted.matrix.considered, 4,
        "the wildcard snapshot counts both packages' folders"
    );
    assert_eq!(
        unrouted.constant.considered, 2,
        "the wildcard snapshot counts both constants"
    );

    let routed = project_with(
        root.path(),
        Some(&projection_names()),
        &[],
        Some(declared),
        false,
    );

    assert_eq!(
        routed.index.considered, 1,
        "the quiet source is not censused"
    );
    assert_eq!(
        routed.index.bootstrapped, 1,
        "and what is owed is the demo index alone"
    );
    assert_eq!(
        routed.matrix.considered, 2,
        "the quiet folders are not walked"
    );
    assert_eq!(
        routed.matrix.bootstrapped, 2,
        "and what is owed is the demo readmes alone"
    );
    assert_eq!(
        routed.constant.considered, 1,
        "the quiet constant derives no pin census"
    );
    assert_eq!(
        routed.constant.bootstrapped, 1,
        "and what is owed is the demo pin alone"
    );
    assert_eq!(
        routed.failures, 1,
        "the demo pin is still owed: {:?}",
        routed.findings
    );
    assert!(
        routed
            .findings
            .iter()
            .all(|finding| finding.message.contains("packages/demo/")),
        "no finding names the quiet share: {:?}",
        routed.findings
    );
}

/// A write under the declared surface leaves an unsubscribed owner's share
/// byte for byte as it stood: no readme is bootstrapped into it, no index
/// is injected, no pin is written — while the subscribed share gains its
/// projections in the same run.
///
/// ´claim:projection:a-write-does-not-touch-an-unsubscribed-owners-share´
/// ´test:crate:writes-nothing-into-an-unsubscribed-owners-share´
#[test]
fn writes_nothing_into_an_unsubscribed_owners_share() {
    let root = routed_tree();
    let configured = crate::configuration(root.path());
    let declared = configured.snapshot().expect("the declared surface loads");
    let quiet = root.path().join("packages/quiet");
    let before = snapshot(&quiet);

    let written = project_with(
        root.path(),
        Some(&projection_names()),
        &[],
        Some(declared),
        true,
    );

    assert_eq!(
        written.failures, 0,
        "the write leaves nothing failing: {:?}",
        written.findings
    );
    assert_eq!(
        snapshot(&quiet),
        before,
        "the unsubscribed share is byte for byte untouched"
    );
    assert!(
        root.path()
            .join("packages/demo/src/tests/README.md")
            .exists(),
        "while the subscribed share gains its readme"
    );
    assert!(
        fs::read_to_string(root.path().join("packages/demo/src/tests/resonance.rs"))
            .expect("read")
            .contains("# Test index"),
        "and its index"
    );
}

/// A projection artifact already standing in an unsubscribed owner's share
/// is simply not read: staleness in it is neither reported nor repaired,
/// and a write leaves it exactly as it stands. An unrouted run over the
/// same tree reports the staleness, so what silences it is the subscription
/// and nothing else.
///
/// ´claim:projection:an-artifact-in-an-unsubscribed-share-is-not-read´
/// ´test:crate:leaves-an-artifact-standing-in-an-unsubscribed-share-unread´
#[test]
fn leaves_an_artifact_standing_in_an_unsubscribed_share_unread() {
    let root = routed_tree();
    let source = root.path().join("packages/quiet/src/tests/hush.rs");

    let _bootstrap = project_with(root.path(), Some(&projection_names()), &[], None, true);
    let written = fs::read_to_string(&source).expect("read");
    let edited = written.replace(
        "| hush | The hush holds under load. |",
        "| roar | The hush holds under load. |",
    );

    assert_ne!(
        edited, written,
        "the edit lands inside the generated region"
    );
    fs::write(&source, edited).expect("write");

    let unrouted = project_with(root.path(), Some(&projection_names()), &[], None, false);

    assert_eq!(
        unrouted.failures, 1,
        "without the surface the staleness is a failure"
    );

    let configured = crate::configuration(root.path());
    let declared = configured.snapshot().expect("the declared surface loads");
    let routed = project_with(
        root.path(),
        Some(&projection_names()),
        &[],
        Some(declared),
        false,
    );

    assert_eq!(routed.index.rewritten, 0, "the stale artifact is not read");
    assert_eq!(
        routed.failures, 0,
        "so nothing is reported: {:?}",
        routed.findings
    );

    let quiet = root.path().join("packages/quiet");
    let before = snapshot(&quiet);
    let _written = project_with(
        root.path(),
        Some(&projection_names()),
        &[],
        Some(declared),
        true,
    );

    assert_eq!(
        snapshot(&quiet),
        before,
        "and a write leaves it exactly as it stands"
    );
}
