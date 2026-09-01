// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! # Label-calculus corpus integration tests
//!
//! Every test here runs against a corpus the test itself builds: an invented
//! workspace under a temporary root, or sources named in the test and handed
//! straight to the analysis. None of them locates the checkout they are running
//! in, and the counts they assert are the fixture's own and therefore exact.
//!
//! Whole-tree conformance is not asserted here at all. It is one invocation of
//! the shipped `check` command in the root pipeline, which judges the
//! repository's real declarations; a package that judged them too would be
//! asserting a property of a corpus it is not entitled to know exists
//! (´req:isolation:boundary´).
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`every_covered_asset_of_a_fixture_seeds_one_derived_mint`] | check | Every covered asset seeds exactly one derived mint, so the count of derived mints is the count of covered assets and neither an asset nor a mint is lost between the census and the analysis. The relation is one-to-one rather than merely bounded, so a collision that folded two assets into one label would show as a shortfall here. |
//! | [`a_document_citing_a_live_test_resolves_and_a_dead_one_does_not`] | check | Against a census taken from an invented workspace, all four ways of naming a test behave as the calculus says: a citation of a live test resolves, a citation of a name nobody wrote fails, a document of another owner reaches it by importing with the owner's prefix, and writing the label bare mints nothing. |
//! | [`a_cross_owner_citation_without_brackets_fails`] | check | A citation crossing an ownership boundary must say so: written bare it fails, and the failure names the owner that minted the label and spells out the imported form to write instead — delimiters included, since a rendering without them is the one form the grammar reads as nothing. Written that way it resolves. The reader is handed the repair rather than the rule. |
//! | [`a_head_the_registry_denies_fails_the_check`] | head | cites (´claim:head:a-head-whose-kind-contradicts-its-name-is-a-failure´) |
//! | [`an_outline_and_its_document_track_both_ways`] | check | Outline tracking holds end to end and in both directions: a matching pair passes the command, a head the outline claims going missing fails at the claiming row, and a head no row claims fails at the head with the omitting outline named beside it. Each failure points where the repair belongs. |
//! | [`the_report_command_describes_the_graph_without_judging_it`] | cli | The report describes and never judges: an uncited mint and a dangling citation are both listed and the command still exits zero, while the check over the same tree fails that citation. A reverse lookup answers when asked, and a malformed question is a usage failure that prints nothing. |
//! | [`a_small_declared_graph_reports_totals_orphans_dangles_and_hubs`] | graph | The report over a small declared graph is internally consistent: the totals are the mints and citations the sources carry, a citation of a name nobody minted is listed as dangling, the mints nobody cites are the orphans and not every mint is one, the hubs come back ranked by how often they are cited, and within every owner the mints cited and the mints uncited add up to the mints there are. |
//! | [`an_assembled_publication_stales_when_a_part_changes`] | assembly | The assemble and check commands agree through the whole cycle: unpublished parts are stale and the check mode writes nothing, writing settles them in the manifest's order, editing a part stales the publication again, reassembly repairs it, and a further write finds nothing to do. |
//! | [`an_unlisted_part_is_reported_rather_than_dropped`] | assembly | cites (´claim:assembly:a-part-no-row-names-is-reported-and-the-rest-still-assemble´) |
//! | [`a_draft_assembly_forms_no_freshness_verdict_and_refuses_write`] | assembly | A draft assembly is checked for membership but not for freshness, and writing one is refused outright so a partial assembly cannot overwrite the standing publication. An unlisted part still fires while the manifest is a draft, and removing the marker restores the freshness verdict over the very same tree. |
//! | [`the_shape_command_measures_without_judging`] | cli | The shape report measures and never judges: it counts environments, marks which are divisions, distributes the two kinds apart, and exits zero regardless. It compares the corpus with nothing, so an invented tree is reported exactly as the repository is. |
//! | [`a_fixture_census_covers_and_derives_every_test_function`] | census | The census over an invented workspace classifies and derives a label for every test function it holds, with none left underivable, and the invented manifests populate every area the profile supports. What is asserted is completeness rather than magnitude: the classification reaches the whole census, and no area goes unexercised because the fixture forgot to write one. |
//! | [`every_edge_of_a_fixture_reference_graph_lands_on_a_mint`] | graph | Every edge of a reference graph lands on a mint at both ends, and there are several edges to check rather than one: the analysis carries no edge whose endpoint is not a node of the graph it belongs to. |
//! | [`a_fixture_corpus_resolves_across_files`] | check | cites (´claim:check:the-traversal-order-decides-nothing´) |
//! | [`a_fixture_corpus_reports_every_rule_it_breaks`] | check | A corpus breaking every rule at once is told about every one of them, not merely the first: each defect is reported with its own code, and the findings arrive ordered by source and then by position, which is the order a reader would walk the tree in. |
//! | [`displayed_material_never_participates`] | prose | cites (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´) |
//! | [`the_check_command_emits_json_and_exits_clean`] | cli | The check command emits exactly one JSON object on standard output, carrying the run's counts and stating no version of itself, and exits zero over a clean tree. A caller can parse the result without reading prose. |
//! | [`the_check_command_exits_nonzero_on_findings`] | cli | A tree with findings exits with the documented failure code and still emits its report, so a caller learns both that the run failed and precisely what failed from the same invocation. |
//! | [`a_fixture_package_tree_checks_and_then_fixes_clean`] | cli | Check, sweep, and check again leaves a tree clean and settled: the first check counts what is missing, wrong and already right; the sweep inserts, repairs and leaves alone exactly those; the second check passes; the author's own line survives with the label indented to match; and a further sweep changes nothing. |
//! | [`the_fix_command_writes_nothing_on_a_dry_run`] | fix | cites (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´) |
//! | [`the_fix_command_refuses_a_dirty_tree`] | cli | The sweep refuses to run over a tree carrying uncommitted changes and writes nothing, so its edits always arrive as a diff a reviewer can read against a clean baseline. |
//! | [`a_fixture_notice_tree_checks_and_then_fixes_clean`] | cli | cites (´claim:cli:check-then-sweep-then-check-leaves-a-tree-clean-and-settled´) |
//! | [`the_todo_sweep_writes_nothing_on_a_dry_run`] | fix | cites (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´) |
//! | [`a_burn_list_fails_on_growth_and_on_a_stale_row`] | burn | The ratchet binds in both directions: the census as found passes, one more reference fails as growth with the new occurrence named, and one fewer fails as a stale row until the list falls with it. A file losing its last reference leaves the list rather than standing at zero. |
//! | [`the_burn_command_emits_json_and_writes_nothing_by_default`] | cli | The burn command reports and writes nothing unless asked: every adopted family is registered at its census, none is rewritten, and the trees excluded from each census are named in the report rather than left to be remembered. |
//! | [`an_absent_declaration_refuses_without_running_the_command`] | cli | A tree with no declaration directory refuses with an empty stdout before command dispatch, because the required shape document is absent and no universe or global-ignore relation can be resolved. |
//! | [`a_refused_snapshot_exits_one_with_an_empty_stdout`] | cli | A declaration that is not a snapshot refuses the command entire: the exit is the shared failure class rather than the findings class, and stdout is empty. A command that cannot read its own configuration has no standing to say anything about the corpus, so it says nothing rather than reporting a verdict it could not have formed. |
//! | [`a_dependent_policy_without_label_prerequisites_refuses_loudly`] | cli | A profile activated without the label calculus it reads refuses every command before dispatch. The shared configuration-failure surface keeps stdout empty and names the missing same-owner label prerequisite on stderr, so an incomplete declaration cannot silently run a partial policy. |
//! | [`a_parsed_snapshot_that_disagrees_is_judged_rather_than_refused`] | cli | A snapshot whose declaration is complete but whose partition disagrees with the tree is judged rather than refused: the run happens, the object is complete, and the exit is the findings class with the unaccounted path named in the report. |
//! | [`a_writing_mode_refuses_a_declaration_that_disagrees_with_the_tree`] | cli | Every writing mode refuses to mutate under a declaration that disagrees with the tree. That is stricter than the exit codes alone require and deliberately so: a writer that proceeded would record a conclusion drawn from a question the corpus had not answered, so it exits the failure class with an empty stdout instead. |
//! | [`a_title_minting_the_departed_document_kind_fails_at_the_catalogue`] | head | cites (´claim:head:a-title-validates-on-every-kind-the-relation-rows-its-environment´) |
//! | [`characterizes_no_surface_bytes`] | harness | Every command class refuses with the same empty data streams when the required shape document is absent from an invented root. |
//! | [`characterizes_check_bytes`] | harness | A check over this deliberately undeclared fixture refuses before findings can be ordered. |
//! | [`characterizes_informational_bytes`] | harness | Each informational command refuses before dispatch over this deliberately undeclared fixture. |
//! | [`characterizes_burn_bytes`] | harness | The burn report fixes its census rows and every surrounding report byte. |
//! | [`characterizes_control_bytes`] | harness | The control response file and stdout are one frozen byte sequence. |
//! | [`characterizes_projection_bytes`] | harness | Projection fixes the report and every file the writer owns. |
//! | [`characterizes_assembly_bytes`] | harness | Assembly fixes both its report and the generated publication. |
//! | [`characterizes_fix_bytes`] | harness | Both fix profiles refuse before writing over deliberately undeclared fixtures, freezing the refusal and the untouched sources. |

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use linter::{
    Adoption, Assembly, BurnList, CodeSurface, ExecutionPlan, Finding, LegacyRule, Location,
    OwnerNames, Package, RegisterRow, Shape, Source, analyze, analyze_profile, census,
    configuration, fixture_kind_registry, index_adoption as build_index_adoption, take_census,
    verify,
};
use sha2::{Digest, Sha256};

#[path = "support/git.rs"]
mod git_fixture;

fn index_adoption(
    packages: &[Package],
    names: Option<&OwnerNames>,
    assemblies: &[Assembly],
) -> Adoption {
    build_index_adoption(packages, names, assemblies, fixture_kind_registry())
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("create");
    fs::write(path, text).expect("write");
}

fn codes(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(Finding::code).collect()
}

fn plan(root: &Path) -> ExecutionPlan {
    ensure_fixture_surface(root);

    ExecutionPlan::compile(root, configuration(root)).expect("fixture topology")
}

/// Every covered asset seeds exactly one derived mint, so the count of derived
/// mints is the count of covered assets and neither an asset nor a mint is lost
/// between the census and the analysis. The relation is one-to-one rather than
/// merely bounded, so a collision that folded two assets into one label would
/// show as a shortfall here.
///
/// ´claim:check:every-covered-asset-seeds-exactly-one-derived-mint´
/// ´test:integration:every-covered-asset-of-a-fixture-seeds-one-derived-mint´
#[test]
fn every_covered_asset_of_a_fixture_seeds_one_derived_mint() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());
    track(root.path());

    let plan = plan(root.path());
    let packages = plan.workspace().packages().to_vec();
    let (census, _census_findings) = take_census(root.path(), plan.profiles().sources());
    let (profile, assets, _profile_findings) = analyze_profile(&packages, &census);

    let analysis = analyze(
        &index_adoption(
            &packages,
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &[],
        &CodeSurface::default().with_tests(&assets),
    );

    assert!(
        profile.covered > 0,
        "the fixture carries covered assets to seed from"
    );
    assert_eq!(
        profile.collision_groups, 0,
        "and no two of them derive one label"
    );
    assert_eq!(
        analysis.derived_mints(),
        profile.covered,
        "every covered asset seeds exactly one mint, since the profile reports no collision"
    );
}

/// Against a census taken from an invented workspace, all four ways of naming a
/// test behave as the calculus says: a citation of a live test resolves, a
/// citation of a name nobody wrote fails, a document of another owner reaches it
/// by importing with the owner's prefix, and writing the label bare mints
/// nothing.
///
/// ´claim:check:citing-a-live-test-resolves-and-citing-a-dead-one-does-not´
/// ´test:integration:a-document-citing-a-live-test-resolves-and-a-dead-one-does-not´
#[test]
fn a_document_citing_a_live_test_resolves_and_a_dead_one_does_not() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());
    track(root.path());

    let plan = plan(root.path());
    let packages = plan.workspace().packages().to_vec();
    let adoption = index_adoption(
        &packages,
        Some(&linter::OwnerNames::new(
            "torrust-",
            [linter::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let (census, _census_findings) = take_census(root.path(), plan.profiles().sources());
    let (_profile, assets, _profile_findings) = analyze_profile(&packages, &census);

    let witness = assets
        .iter()
        .find(|asset| asset.test().package() == "torrust-demo")
        .expect("the demo member carries a covered test");
    let label = witness.label().to_string();

    let cited = Source::new(
        "packages/demo/docs/note.md",
        format!("## Head · `sec:demo:note`\n\nThe witness is (`{label}`).\n"),
    );
    let resolved = analyze(
        &adoption,
        &[cited],
        &CodeSurface::default().with_tests(&assets),
    );

    assert_eq!(codes(resolved.findings()), Vec::<&str>::new());
    assert_eq!(
        resolved.citations_resolved(),
        1,
        "the citation is counted as resolved"
    );

    let dead = Source::new(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:note`\n\nThe witness is (`test:integration:a-test-nobody-ever-wrote`).\n",
    );
    let refused = analyze(
        &adoption,
        &[dead],
        &CodeSurface::default().with_tests(&assets),
    );

    assert_eq!(
        codes(refused.findings()),
        ["unresolved_citation"],
        "a name the census does not carry fails, naming the label and its place"
    );

    let imported = Source::new(
        "adr/001-fixture.md",
        format!("## Head · `sec:fixture:record`\n\nThe witness is (`[DEMO-{label}]`).\n"),
    );
    let reached = analyze(
        &adoption,
        &[imported],
        &CodeSurface::default().with_tests(&assets),
    );

    assert_eq!(codes(reached.findings()), Vec::<&str>::new());
    assert_eq!(
        reached.citations_resolved(),
        1,
        "and a document of the other owner reaches it"
    );

    let authored = Source::new(
        "packages/demo/docs/note.md",
        format!("## Head · `{label}`\n"),
    );
    let unwarranted = analyze(
        &adoption,
        &[authored],
        &CodeSurface::default().with_tests(&assets),
    );

    assert_eq!(
        codes(unwarranted.findings()),
        ["unwarranted_reserved_kind"],
        "while writing the label bare is still no way to mint one"
    );
}

/// A citation crossing an ownership boundary must say so: written bare it fails,
/// and the failure names the owner that minted the label and spells out the
/// imported form to write instead — delimiters included, since a rendering
/// without them is the one form the grammar reads as nothing. Written that way
/// it resolves. The reader is handed the repair rather than the rule.
///
/// ´claim:check:a-citation-crossing-an-ownership-boundary-must-be-imported´
/// ´test:integration:a-cross-owner-citation-without-brackets-fails´
#[test]
fn a_cross_owner_citation_without_brackets_fails() {
    let packages = [
        Package::new("torrust-index", ""),
        Package::new("torrust-assayer", "packages/assayer"),
    ];
    let adoption = index_adoption(
        &packages,
        Some(&linter::OwnerNames::new(
            "torrust-",
            [linter::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );

    let record = Source::new(
        "adr/014-label-calculus.md",
        "## Syntax · `sec:labels:syntax`\n",
    );
    let bare = Source::new(
        "packages/assayer/docs/note.md",
        "## Note · `sec:assayer:note`\n\nCites (`sec:labels:syntax`) across the boundary.\n",
    );
    let imported = Source::new(
        "packages/assayer/docs/note.md",
        "## Note · `sec:assayer:note`\n\nCites (`[INDEX-sec:labels:syntax]`) across the boundary.\n",
    );

    let refused = analyze(&adoption, &[record.clone(), bare], &CodeSurface::default());

    let [
        Finding::UnresolvedCitationWantingImport {
            minting_owner,
            suggestion,
            ..
        },
    ] = refused.findings()
    else {
        panic!(
            "expected the bracket-free crossing to fail, got {:?}",
            refused.findings()
        );
    };

    assert_eq!(minting_owner, "index");
    assert_eq!(suggestion, "(`[INDEX-sec:labels:syntax]`)");

    let accepted = analyze(&adoption, &[record, imported], &CodeSurface::default());

    assert_eq!(codes(accepted.findings()), Vec::<&str>::new());
    assert_eq!(accepted.citations_resolved(), 1);
}

/// Head validation reaches the command: a head whose kind the registry denies
/// its name fails the run, reporting the base word and the kinds catalogued for
/// it, and the catalogued pairing passes.
///
/// (´claim:head:a-head-whose-kind-contradicts-its-name-is-a-failure´)
/// ´test:integration:a-head-the-registry-denies-fails-the-check´
#[test]
fn a_head_the_registry_denies_fails_the_check() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "**Theorem (Warrant lapse)** · `def:fixture:warrant-lapse`\n",
    );
    track(root.path());

    let (code, report) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "a head the registry denies is a failure: {report}"
    );
    assert_eq!(report["findings"][0]["code"], "misclassified_head");
    assert_eq!(report["findings"][0]["base"], "Theorem");
    assert_eq!(report["findings"][0]["catalogued"][0], "thm");

    write(
        root.path(),
        "adr/001-one.md",
        "**Meta-theorem (Warrant lapse)** · `metathm:fixture:warrant-lapse`\n",
    );

    let (code, repaired) = run(&["check"], root.path());

    assert_eq!(code, Some(0), "and the catalogued pair passes: {repaired}");
    assert_eq!(repaired["heads_validated"], 1);
}

/// The outline half of the tracking fixture, given the rows it declares.
fn outline_document(rows: &str) -> String {
    format!(
        "# The fixture outline\n\n\
         **Convention (Tracking)** · `conv:fixture:tracking`\n\n\
         | Entry | Head | Document |\n| --- | --- | --- |\n{rows}\n\n\
         **Entry (First)** · `entry:fixture:first`\n\n\
         The first environment the tracked document carries.\n\n\
         **Entry (Second)** · `entry:fixture:second`\n\n\
         The second environment the tracked document carries.\n"
    )
}

/// Both rows of the passing fixture pair.
fn both_tracking_rows() -> String {
    concat!(
        "| ``entry:fixture:first`` | ``sec:fixture:first`` | ``docs/tracked.md`` |\n",
        "| ``entry:fixture:second`` | ``sec:fixture:second`` | ``docs/tracked.md`` |",
    )
    .to_owned()
}

/// Outline tracking holds end to end and in both directions: a matching pair
/// passes the command, a head the outline claims going missing fails at the
/// claiming row, and a head no row claims fails at the head with the omitting
/// outline named beside it. Each failure points where the repair belongs.
///
/// ´claim:check:outline-tracking-holds-end-to-end-in-both-directions´
/// ´test:integration:an-outline-and-its-document-track-both-ways´
#[test]
fn an_outline_and_its_document_track_both_ways() {
    let both_heads = "# The tracked document\n\n\
                      ## First · `sec:fixture:first`\n\nProse.\n\n\
                      ## Second · `sec:fixture:second`\n\nProse.\n";

    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "docs/outline.md",
        &outline_document(&both_tracking_rows()),
    );
    write(root.path(), "docs/tracked.md", both_heads);
    track(root.path());

    let (code, tracked) = run(&["check"], root.path());

    assert_eq!(code, Some(0), "the fixture pair tracks: {tracked}");
    assert_eq!(tracked["failures"], 0);

    // Drift one way: the tracked document loses a head the outline claims.
    write(
        root.path(),
        "docs/tracked.md",
        "# The tracked document\n\n## First · `sec:fixture:first`\n\nProse.\n",
    );

    let (code, unfulfilled) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "a head the outline claims went missing: {unfulfilled}"
    );
    assert_eq!(
        unfulfilled["findings"][0]["code"],
        "unfulfilled_outline_entry"
    );
    assert_eq!(unfulfilled["findings"][0]["head"], "sec:fixture:second");
    assert_eq!(
        unfulfilled["findings"][0]["location"]["path"], "docs/outline.md",
        "the claiming row is where the reader repairs it"
    );
    assert_eq!(
        unfulfilled["findings"][0]["document"], "docs/tracked.md",
        "and the document that should have carried it is named beside it"
    );

    // Drift the other way: the document keeps both heads, the outline drops a row.
    write(root.path(), "docs/tracked.md", both_heads);
    write(
        root.path(),
        "docs/outline.md",
        &outline_document(
            "| ``entry:fixture:first`` | ``sec:fixture:first`` | ``docs/tracked.md`` |",
        ),
    );

    let (code, unclaimed) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "a head no entry claims is drift too: {unclaimed}"
    );
    assert_eq!(unclaimed["findings"][0]["code"], "unclaimed_head");
    assert_eq!(unclaimed["findings"][0]["head"], "sec:fixture:second");
    assert_eq!(
        unclaimed["findings"][0]["location"]["path"], "docs/tracked.md",
        "the head stands in the tracked document"
    );
    assert_eq!(
        unclaimed["findings"][0]["declaration"]["path"], "docs/outline.md",
        "and the outline that omits it is the finding's other end"
    );
}

/// The report describes and never judges: an uncited mint and a dangling
/// citation are both listed and the command still exits zero, while the check
/// over the same tree fails that citation. A reverse lookup answers when asked,
/// and a malformed question is a usage failure that prints nothing.
///
/// ´claim:cli:the-report-command-describes-without-judging´
/// ´test:integration:the-report-command-describes-the-graph-without-judging-it´
#[test]
fn the_report_command_describes_the_graph_without_judging_it() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Hub · `sec:fixture:hub`\n\nProse.\n\n\
         ## Leaf · `sec:fixture:leaf`\n\nCites (`sec:fixture:hub`) and (`sec:fixture:missing`).\n",
    );
    track(root.path());

    let (code, report) = run(&["report"], root.path());

    assert_eq!(
        code,
        Some(0),
        "an uncited mint and a dangling citation are both reported, not judged: {report}"
    );
    assert!(
        report.get("schema").is_none(),
        "the graph report states no version of itself"
    );
    assert_eq!(report["mints"], 2);
    assert_eq!(report["citations"], 1);
    assert_eq!(report["orphans"][0]["label"], "sec:fixture:leaf");
    assert_eq!(report["hubs"][0]["label"], "sec:fixture:hub");
    assert_eq!(report["hubs"][0]["citations"], 1);
    assert_eq!(report["dangling"][0]["code"], "unresolved_citation");
    assert!(report["reverse"].is_null(), "no lookup was asked for");

    assert_eq!(
        run(&["check"], root.path()).0,
        Some(3),
        "while the check still fails the same dangling citation"
    );

    let (code, looked_up) = run(&["report", "--cites", "sec:fixture:hub"], root.path());

    assert_eq!(code, Some(0));
    assert_eq!(looked_up["reverse"]["label"], "sec:fixture:hub");
    assert_eq!(
        looked_up["reverse"]["citers"][0]["from"],
        "sec:fixture:leaf"
    );

    let (code, refused) = run(&["report", "--cites", "not a label"], root.path());

    assert_eq!(code, Some(2), "a malformed question is a usage failure");
    assert!(refused.is_null(), "and stdout stays empty");
}

/// The report over a small declared graph is internally consistent: the totals
/// are the mints and citations the sources carry, a citation of a name nobody
/// minted is listed as dangling, the mints nobody cites are the orphans and not
/// every mint is one, the hubs come back ranked by how often they are cited, and
/// within every owner the mints cited and the mints uncited add up to the mints
/// there are.
///
/// ´claim:graph:a-declared-graphs-report-is-internally-consistent´
/// ´test:integration:a-small-declared-graph-reports-totals-orphans-dangles-and-hubs´
#[test]
fn a_small_declared_graph_reports_totals_orphans_dangles_and_hubs() {
    let root = tempfile::tempdir().expect("temporary directory");

    write(
        root.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\".\", \"packages/demo\"]\n\n[package]\nname = \"torrust-fixture\"\n",
    );
    write(
        root.path(),
        "packages/demo/Cargo.toml",
        "[package]\nname = \"torrust-demo\"\n",
    );
    write(
        root.path(),
        "adr/001-hub.md",
        "## Hub · `sec:fixture:hub`\n\nProse.\n\n## Second · `sec:fixture:second`\n\nProse.\n",
    );
    write(
        root.path(),
        "adr/002-cites.md",
        "## Cites · `sec:fixture:cites`\n\nCites (`sec:fixture:hub`), then (`sec:fixture:second`), then (`sec:fixture:hub`) again.\n",
    );
    write(
        root.path(),
        "adr/003-dangles.md",
        "## Dangles · `sec:fixture:dangles`\n\nCites (`sec:fixture:gone`), which nobody minted.\n",
    );
    write(
        root.path(),
        "packages/demo/docs/note.md",
        "## Note · `sec:demo:note`\n\nProse.\n",
    );
    track(root.path());
    ensure_fixture_surface(root.path());

    let report = linter::report(root.path(), None, linter::DEFAULT_HUBS);

    assert_eq!(report.summary.mints, 5, "the five heads the fixture writes");
    assert_eq!(
        report.dangling.len(),
        1,
        "the one citation nobody minted: {:?}",
        report.dangling
    );
    assert!(
        !report.summary.orphans.is_empty(),
        "some mints are legitimately uncited"
    );
    assert!(
        report.summary.orphans.len() < report.summary.mints,
        "though not all of them: {} of {}",
        report.summary.orphans.len(),
        report.summary.mints
    );
    assert_eq!(
        report.summary.hubs[0].label.to_string(),
        "sec:fixture:hub",
        "the most-cited head leads"
    );
    assert!(
        report
            .summary
            .hubs
            .windows(2)
            .all(|pair| pair[0].citations >= pair[1].citations),
        "the hubs are ranked: {:?}",
        report.summary.hubs
    );

    for cell in report.summary.by_owner.values() {
        assert_eq!(
            cell.mints,
            cell.cited + cell.orphans,
            "every mint is cited or is not"
        );
    }
}

/// The parts directory of the one adopted assembly, and the document it publishes.
const SPEC_PARTS: &str = "packages/assayer/docs/spec";
const SPEC_TARGET: &str = "packages/assayer/docs/spec.md";

/// The fictional kind registry shared by command-level corpus fixtures.
const FIXTURE_ENVIRONMENTS: &str = "reserved_kinds = [\"test\"]\n\
    reserved_extensions = [\"const\", \"legacy\", \"todo\"]\n\n\
    environments = [\n\
      { environment = \"Appendix\", kind = \"app\" },\n\
      { environment = \"Assertion\", kind = \"claim\" },\n\
      { environment = \"Convention\", kind = \"conv\" },\n\
      { environment = \"Document\", kind = \"doc\" },\n\
      { environment = \"Entry\", kind = \"entry\" },\n\
      { environment = \"Invariant\", kind = \"inv\" },\n\
      { environment = \"Meta-theorem\", kind = \"metathm\" },\n\
      { environment = \"Note\", kind = \"rem\" },\n\
      { environment = \"Part\", kind = \"part\" },\n\
      { environment = \"Section\", kind = \"sec\" },\n\
      { environment = \"Table\", kind = \"tab\" },\n\
      { environment = \"Theorem\", kind = \"thm\" },\n\
    ]\n\n\
    extensions = [\n\
      { environment = \"Constant\", kind = \"const\" },\n\
      { environment = \"To-do\", kind = \"todo\" },\n\
    ]\n";

/// One part of the fixture specification.
fn spec_part(name: &str, body: &str) -> String {
    format!(
        "## The {name} part · `sec:fixture:{name}`\n\n**Invariant ({name})** · `inv:fixture:{name}`\n\n{body}\n"
    )
}

/// The fixture manifest, given the rows it lists.
fn spec_manifest(rows: &str) -> String {
    format!(
        "# The specification manifest\n\n\
         **Convention (Assembly)** · `conv:fixture:assembly`\n\n\
         | Part |\n| --- |\n{rows}\n"
    )
}

/// Lay out a parts directory carrying two parts and listing both.
/// The owner file the assembly fixtures are partitioned by.
///
/// Wider than the starting owner file because these fixtures stand under the
/// package tree rather than under the record tree, and every tracked path must
/// be accounted for the partition to be total.
const SPEC_OWNERS: &str = "owners = [\"INDEX\"]\n\
    partitions = [{ name = \"package-trees\", owner = \"INDEX\", pattern = '%s\"packages\" [ \"/\" *VCHAR ]' }, \
    { name = \"declared-surface\", owner = \"INDEX\", pattern = '%s\".linter\" [ \"/\" *VCHAR ]' }]\n\
    may_cite = []\n";

/// Declare the publication the assembly fixtures assemble.
///
/// The parts and the target used to be compiled into the linter, so a fixture
/// carrying the tree was a fixture the tool already knew how to assemble. They
/// are the corpus's to state now, which is why an assembly fixture declares one:
/// a tree that has not said what it publishes has nothing to publish.
fn declare_spec_publication(root: &Path) {
    declare(
        root,
        SPEC_OWNERS,
        FIXTURE_ENVIRONMENTS,
        DECLARED_POLICIES,
        DECLARED_LISTS,
    );
    write(
        root,
        ".linter/policy-assembly-publications.toml",
        &format!(
            "namespace = \"com.torrust.index.linter.policy.assembly-publications\"\nversion = [1, 0, 0]\n\n\
             [owners.INDEX]\nspec = {{ parts = \"{SPEC_PARTS}\", target = \"{SPEC_TARGET}\" }}\n"
        ),
    );
}

fn spec_fixture(root: &Path) {
    declare_spec_publication(root);

    write(
        root,
        &format!("{SPEC_PARTS}/assembly.md"),
        &spec_manifest("| ``alpha.md`` |\n| ``beta.md`` |"),
    );
    write(
        root,
        &format!("{SPEC_PARTS}/alpha.md"),
        &spec_part("alpha", "The first part."),
    );
    write(
        root,
        &format!("{SPEC_PARTS}/beta.md"),
        &spec_part("beta", "The second part."),
    );
}

/// The assemble and check commands agree through the whole cycle: unpublished
/// parts are stale and the check mode writes nothing, writing settles them in
/// the manifest's order, editing a part stales the publication again,
/// reassembly repairs it, and a further write finds nothing to do.
///
/// ´claim:assembly:the-assemble-and-check-commands-agree-through-publish-stale-and-repair´
/// ´test:integration:an-assembled-publication-stales-when-a-part-changes´
#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "the test walks one fixture through assemble, stale, and repair in order; splitting it would hide the sequence it exists to pin"
)]
fn an_assembled_publication_stales_when_a_part_changes() {
    let root = tempfile::tempdir().expect("temporary directory");
    spec_fixture(root.path());
    track(root.path());

    let (code, before) = run(&["assemble"], root.path());

    assert_eq!(code, Some(3), "nothing is published yet: {before}");
    assert!(
        before.get("schema").is_none(),
        "the assemble report states no version of itself"
    );
    assert_eq!(before["write"], false);
    assert_eq!(before["assemblies"][0]["dormant"], false);
    assert_eq!(before["assemblies"][0]["assembled"], 2);
    assert_eq!(before["assemblies"][0]["target"], SPEC_TARGET);
    assert_eq!(before["findings"][0]["code"], "stale_assembly");
    assert!(
        !root.path().join(SPEC_TARGET).exists(),
        "and the check mode wrote nothing"
    );

    let (code, written) = run(&["assemble", "--write"], root.path());

    assert_eq!(code, Some(0), "writing settles it: {written}");
    assert_eq!(written["assemblies"][0]["written"], true);

    let published = fs::read_to_string(root.path().join(SPEC_TARGET)).expect("the publication");

    assert!(
        published.starts_with("<!-- Assembled from packages/assayer/docs/spec/ under"),
        "got {published:?}"
    );
    assert!(
        published.find("sec:fixture:alpha") < published.find("sec:fixture:beta"),
        "the manifest's order is the publication's order"
    );

    let (code, fresh) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(0),
        "the parts and their publication agree: {fresh}"
    );
    assert_eq!(fresh["assembly"]["verified"], 1);
    assert_eq!(fresh["assembly"]["dormant"], 0);
    assert_eq!(fresh["assembly"]["parts"], 2);

    // Editing a part stales the publication, and the check is where it shows.
    write(
        root.path(),
        &format!("{SPEC_PARTS}/alpha.md"),
        &spec_part("alpha", "The first part, rewritten."),
    );

    let (code, stale) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "an edited part stales the publication: {stale}"
    );
    assert_eq!(stale["findings"][0]["code"], "stale_assembly");
    assert_eq!(stale["findings"][0]["target"], SPEC_TARGET);

    let (code, repaired) = run(&["assemble", "--write"], root.path());

    assert_eq!(code, Some(0), "and reassembly repairs it: {repaired}");
    assert_eq!(run(&["check"], root.path()).0, Some(0));

    // Assembly is byte-stable: a second write changes nothing and reports fresh.
    let settled = fs::read_to_string(root.path().join(SPEC_TARGET)).expect("the publication");
    let (code, again) = run(&["assemble", "--write"], root.path());

    assert_eq!(code, Some(0));
    assert_eq!(
        again["assemblies"][0]["written"], false,
        "there was nothing to write"
    );
    assert_eq!(again["assemblies"][0]["fresh"], true);
    assert_eq!(
        fs::read_to_string(root.path().join(SPEC_TARGET)).expect("the publication"),
        settled
    );
}

/// Through the command as through the library, a part no manifest row names is
/// reported rather than silently dropped from the publication.
///
/// (´claim:assembly:a-part-no-row-names-is-reported-and-the-rest-still-assemble´)
/// ´test:integration:an-unlisted-part-is-reported-rather-than-dropped´
#[test]
fn an_unlisted_part_is_reported_rather_than_dropped() {
    let root = tempfile::tempdir().expect("temporary directory");
    spec_fixture(root.path());
    write(
        root.path(),
        &format!("{SPEC_PARTS}/assembly.md"),
        &spec_manifest("| ``alpha.md`` |"),
    );

    let (code, report) = run(&["assemble"], root.path());

    let codes: Vec<&str> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| finding["code"].as_str().expect("a code"))
        .collect();

    assert_eq!(
        code,
        Some(3),
        "a part nothing assembles is a finding: {report}"
    );
    assert!(codes.contains(&"unassembled_part"), "got {codes:?}");
}

/// The same fixture manifest, carrying the draft marker before its table.
fn spec_draft_manifest(rows: &str) -> String {
    format!(
        "# The specification manifest\n\n\
         **Draft.**\n\n\
         **Convention (Assembly)** · `conv:fixture:assembly`\n\n\
         | Part |\n| --- |\n{rows}\n"
    )
}

/// A draft assembly is checked for membership but not for freshness, and writing
/// one is refused outright so a partial assembly cannot overwrite the standing
/// publication. An unlisted part still fires while the manifest is a draft, and
/// removing the marker restores the freshness verdict over the very same tree.
///
/// ´claim:assembly:writing-a-draft-assembly-is-refused-and-the-target-left-standing´
/// ´test:integration:a-draft-assembly-forms-no-freshness-verdict-and-refuses-write´
#[test]
fn a_draft_assembly_forms_no_freshness_verdict_and_refuses_write() {
    let root = tempfile::tempdir().expect("temporary directory");
    spec_fixture(root.path());
    write(
        root.path(),
        &format!("{SPEC_PARTS}/assembly.md"),
        &spec_draft_manifest("| ``alpha.md`` |\n| ``beta.md`` |"),
    );
    write(
        root.path(),
        SPEC_TARGET,
        "# The old, unrelated specification\n",
    );
    track(root.path());

    // A draft assembly is checked for membership but forms no freshness verdict,
    // so the standing target — deliberately not what the parts assemble into —
    // is not reported stale.
    let (code, clean) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(0),
        "a draft assembly forms no freshness verdict: {clean}"
    );
    assert_eq!(clean["assembly"]["dormant"], 0, "the rewrite has started");
    assert_eq!(clean["assembly"]["verified"], 1);

    // Writing a draft assembly is refused: it would overwrite the standing
    // target with a partial assembly rather than update it.
    let (code, refused) = run(&["assemble", "--write"], root.path());

    assert_eq!(
        code,
        Some(3),
        "writing a draft assembly is refused: {refused}"
    );
    assert_eq!(refused["assemblies"][0]["draft"], true);
    assert_eq!(refused["assemblies"][0]["written"], false);
    assert!(
        refused["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "draft_assembly_write"),
        "got {refused}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join(SPEC_TARGET)).expect("the old target"),
        "# The old, unrelated specification\n",
        "the standing target was not overwritten"
    );

    // Membership is still checked both ways in draft mode: a part standing in
    // the directory that no row lists is still reported.
    write(
        root.path(),
        &format!("{SPEC_PARTS}/assembly.md"),
        &spec_draft_manifest("| ``alpha.md`` |"),
    );

    let (code, unassembled) = run(&["check"], root.path());
    let codes: Vec<&str> = unassembled["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| finding["code"].as_str().expect("a code"))
        .collect();

    assert_eq!(
        code,
        Some(3),
        "an unlisted part still fires in draft mode: {unassembled}"
    );
    assert!(codes.contains(&"unassembled_part"), "got {codes:?}");

    // Removing the marker restores full membership and live freshness: the same
    // old target is now exactly what draft mode was declining to judge.
    write(
        root.path(),
        &format!("{SPEC_PARTS}/assembly.md"),
        &spec_manifest("| ``alpha.md`` |\n| ``beta.md`` |"),
    );

    let (code, stale) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "the marker is gone, so freshness binds again: {stale}"
    );
    assert!(
        stale["findings"]
            .as_array()
            .expect("findings")
            .iter()
            .any(|finding| finding["code"] == "stale_assembly"),
        "got {stale}"
    );
}

/// The shape report measures and never judges: it counts environments, marks
/// which are divisions, distributes the two kinds apart, and exits zero
/// regardless. It compares the corpus with nothing, so an invented tree is
/// reported exactly as the repository is.
///
/// ´claim:cli:the-shape-command-measures-without-judging´
/// ´test:integration:the-shape-command-measures-without-judging´
#[test]
fn the_shape_command_measures_without_judging() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## A division · `sec:fixture:division`\n\n\
         **Invariant (Short)** · `inv:fixture:short`\n\nOne.\n\n\
         **Invariant (Long)** · `inv:fixture:long`\n\n\
         Cites (`inv:fixture:short`) at length, over many more words than its sibling carries.\n",
    );
    track(root.path());

    let (code, report) = run(&["shape"], root.path());

    assert_eq!(code, Some(0), "size is measured and never judged: {report}");
    assert!(
        report.get("schema").is_none(),
        "the shape report states no version of itself"
    );
    assert_eq!(report["documents_measured"], 1);
    assert_eq!(report["environments"], 3);
    assert_eq!(
        report["named"], 2,
        "the division is measured apart from the two named"
    );
    assert_eq!(report["by_document"][0]["document"], "adr/001-one.md");
    assert_eq!(
        report["by_document"][0]["environments"][0]["division"],
        true
    );
    assert_eq!(
        report["words"]["count"], 2,
        "the two named environments are distributed"
    );
    assert_eq!(
        report["division_words"]["count"], 1,
        "and the division in its own"
    );
    assert!(
        report.get("band").is_none()
            && report.get("long").is_none()
            && report.get("short").is_none(),
        "the report carries no yardstick and calls nothing an outlier: {report}"
    );
}

/// The census over an invented workspace classifies and derives a label for
/// every test function it holds, with none left underivable, and the invented
/// manifests populate every area the profile supports. What is asserted is
/// completeness rather than magnitude: the classification reaches the whole
/// census, and no area goes unexercised because the fixture forgot to write one.
///
/// ´claim:census:a-census-covers-and-derives-every-test-function-it-holds´
/// ´test:integration:a-fixture-census-covers-and-derives-every-test-function´
#[test]
fn a_fixture_census_covers_and_derives_every_test_function() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());
    track(root.path());

    let plan = plan(root.path());
    let packages = plan.workspace().packages().to_vec();
    let (census, findings) = take_census(root.path(), plan.profiles().sources());
    let (analysis, _assets, _profile_findings) = analyze_profile(&packages, &census);

    assert_eq!(
        codes(&findings),
        Vec::<&str>::new(),
        "every invented source parses"
    );
    assert!(
        !census.tests().is_empty(),
        "the fixture holds tests to classify"
    );
    assert_eq!(
        analysis.covered,
        census.tests().len(),
        "every censused test is classified and derived"
    );
    assert_eq!(
        analysis.underivable, 0,
        "every identifier transforms into a label name"
    );

    for area in ["unit", "crate", "integration"] {
        assert!(
            analysis.by_area[area] > 0,
            "the fixture exercises the {area} area, found {}",
            analysis.by_area[area]
        );
    }
}

/// Every edge of a reference graph lands on a mint at both ends, and there are
/// several edges to check rather than one: the analysis carries no edge whose
/// endpoint is not a node of the graph it belongs to.
///
/// ´claim:graph:every-edge-of-a-reference-graph-lands-on-a-mint´
/// ´test:integration:every-edge-of-a-fixture-reference-graph-lands-on-a-mint´
#[test]
fn every_edge_of_a_fixture_reference_graph_lands_on_a_mint() {
    let sources = [
        Source::new(
            "adr/001-one.md",
            "## Hub · `sec:fixture:hub`\n\nProse.\n\n## Second · `sec:fixture:second`\n\nCites (`sec:fixture:hub`).\n",
        ),
        Source::new(
            "adr/002-two.md",
            "## Third · `sec:fixture:third`\n\nCites (`sec:fixture:hub`) and (`sec:fixture:second`).\n",
        ),
    ];

    let analysis = analyze(
        &index_adoption(
            &[],
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default(),
    );
    let graph = analysis.graph();

    assert_eq!(
        codes(analysis.findings()),
        Vec::<&str>::new(),
        "the fixture dangles nothing"
    );
    assert_eq!(graph.edge_count(), 3, "three citations, three edges");

    for edge in graph.edge_indices() {
        let (from, to) = graph.edge_endpoints(edge).expect("endpoints");

        assert!(graph.node_weight(from).is_some(), "edge source is a mint");
        assert!(graph.node_weight(to).is_some(), "edge target is a mint");
    }
}

/// Two documents citing each other resolve whichever is read first, with the
/// same counts either way, so mutual reference across files is not an ordering
/// problem.
///
/// (´claim:check:the-traversal-order-decides-nothing´)
/// ´test:integration:a-fixture-corpus-resolves-across-files´
#[test]
fn a_fixture_corpus_resolves_across_files() {
    let minting = Source::new(
        "adr/one.md",
        "## Head · `sec:fixture:head`\n\nProse citing (`sec:fixture:other`).\n",
    );
    let citing = Source::new(
        "adr/two.md",
        "## Other · `sec:fixture:other`\n\nProse citing (`sec:fixture:head`).\n",
    );

    let forward = analyze(
        &index_adoption(
            &[],
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &[minting.clone(), citing.clone()],
        &CodeSurface::default(),
    );
    let backward = analyze(
        &index_adoption(
            &[],
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &[citing, minting],
        &CodeSurface::default(),
    );

    assert_eq!(codes(forward.findings()), Vec::<&str>::new());
    assert_eq!(codes(backward.findings()), Vec::<&str>::new());
    assert_eq!(forward.citations_resolved(), 2);
    assert_eq!(backward.citations_resolved(), 2);
    assert_eq!(forward.mints(), backward.mints());
}

/// A corpus breaking every rule at once is told about every one of them, not
/// merely the first: each defect is reported with its own code, and the findings
/// arrive ordered by source and then by position, which is the order a reader
/// would walk the tree in.
///
/// ´claim:check:every-rule-a-corpus-breaks-is-reported-in-reading-order´
/// ´test:integration:a-fixture-corpus-reports-every-rule-it-breaks´
#[test]
fn a_fixture_corpus_reports_every_rule_it_breaks() {
    let sources = [
        Source::new("adr/one.md", "## Head · `sec:fixture:head`\n"),
        Source::new("adr/two.md", "## Repeat · `sec:fixture:head`\n"),
        Source::new("adr/three.md", "Cites (`sec:fixture:missing`).\n"),
        Source::new("adr/four.md", "## Asset · `test:unit:decode`\n"),
        Source::new("adr/five.md", "Imports (`[SPEC-sec:fixture:head]`).\n"),
        Source::new(
            "adr/six.md",
            "Bare import `[SPEC-sec:fixture:head]` here.\n",
        ),
        Source::new("adr/seven.md", "An unpaired ` backtick.\n"),
        Source::new("adr/eight.md", "Nearly (`Sec:fixture:head`).\n"),
    ];

    let analysis = analyze(
        &index_adoption(
            &[],
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default(),
    );

    assert_eq!(
        codes(analysis.findings()),
        [
            // Ordered by source path, then by position within the source.
            "near_miss",                 // adr/eight.md
            "unregistered_prefix",       // adr/five.md
            "unwarranted_reserved_kind", // adr/four.md
            "unpaired_backtick",         // adr/seven.md
            "non_parenthesized_import",  // adr/six.md
            "unresolved_citation",       // adr/three.md
            "duplicate_mint",            // adr/two.md
        ]
    );

    let duplicate = analysis
        .findings()
        .iter()
        .find(|finding| matches!(finding, Finding::DuplicateMint { .. }))
        .expect("a duplicate mint");

    let Finding::DuplicateMint { first, second, .. } = duplicate else {
        panic!("expected a duplicate mint");
    };

    assert_eq!(first.path(), Path::new("adr/one.md"));
    assert_eq!(second.path(), Path::new("adr/two.md"));
}

/// Displayed material never participates, end to end: labels shown in doubled
/// spans or inside a fenced block yield no citation and no finding, while the
/// one head written for real still mints.
///
/// (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´)
/// ´test:integration:displayed-material-never-participates´
#[test]
fn displayed_material_never_participates() {
    let sources = [Source::new(
        "adr/one.md",
        concat!(
            "## Head · `sec:fixture:head`\n\n",
            "Displayed inline: ``sec:fixture:nowhere`` and (``sec:fixture:nowhere``).\n\n",
            "```text\n",
            "`sec:fixture:nowhere`\n",
            "(`sec:fixture:nowhere`)\n",
            "(`[SPEC-sec:fixture:nowhere]`)\n",
            "```\n",
        ),
    )];

    let analysis = analyze(
        &index_adoption(
            &[],
            Some(&linter::OwnerNames::new(
                "torrust-",
                [linter::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default(),
    );

    assert_eq!(codes(analysis.findings()), Vec::<&str>::new());
    assert_eq!(analysis.mints(), 1);
    assert_eq!(analysis.citations_resolved(), 0);
}

/// The check command emits exactly one JSON object on standard output, carrying
/// the run's counts and stating no version of itself, and exits zero over a
/// clean tree. A caller can parse the result without reading prose.
///
/// ´claim:cli:the-check-command-emits-one-json-object-and-exits-zero-when-clean´
/// ´test:integration:the-check-command-emits-json-and-exits-clean´
#[test]
fn the_check_command_emits_json_and_exits_clean() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Head · `sec:fixture:head`\n\nCites (`sec:fixture:head`).\n",
    );
    track(root.path());
    ensure_fixture_surface(root.path());

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(["check", "--root"])
        .arg(root.path())
        .output()
        .expect("run the linter");

    assert_eq!(output.status.code(), Some(0));

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");

    assert!(
        report.get("schema").is_none(),
        "the check report states no version of itself"
    );
    assert_eq!(report["clean"], true);
    assert_eq!(report["mints"], 1);
    assert_eq!(report["citations_resolved"], 1);
}

/// A tree with findings exits with the documented failure code and still emits
/// its report, so a caller learns both that the run failed and precisely what
/// failed from the same invocation.
///
/// ´claim:cli:a-failing-check-exits-with-its-documented-code-and-still-reports´
/// ´test:integration:the-check-command-exits-nonzero-on-findings´
#[test]
fn the_check_command_exits_nonzero_on_findings() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "Cites (`sec:fixture:missing`).\n",
    );
    track(root.path());
    ensure_fixture_surface(root.path());

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(["check", "--root"])
        .arg(root.path())
        .output()
        .expect("run the linter");

    assert_eq!(output.status.code(), Some(3));

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");

    assert_eq!(report["clean"], false);
    assert_eq!(report["failures"], 1);
    assert_eq!(report["findings"][0]["code"], "unresolved_citation");
}

/// Build a two-package fixture workspace exercising all three areas.
///
/// One test of each area is unlabelled, one already carries a correct label,
/// and one carries a stale one, so a sweep must insert twice, repair once, and
/// leave one alone.
fn fixture_workspace(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\".\", \"packages/demo\"]\n\n[package]\nname = \"torrust-fixture\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    /// Checks it.\n    #[test]\n    fn a_unit_test() {}\n}\n",
    );
    write(
        root,
        "src/tests/mod.rs",
        "/// \u{b4}test:crate:a-stale-name\u{b4}\n#[test]\nfn stale_label_carrier() {}\n",
    );
    write(
        root,
        "packages/demo/Cargo.toml",
        "[package]\nname = \"torrust-demo\"\n",
    );
    write(
        root,
        "packages/demo/tests/end_to_end.rs",
        "/// \u{b4}test:integration:already-right\u{b4}\n#[test]\nfn already_right() {}\n",
    );
}

/// Run the linter binary and return its exit code and parsed stdout.
fn run(arguments: &[&str], root: &Path) -> (Option<i32>, serde_json::Value) {
    ensure_fixture_surface(root);

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(arguments)
        .arg("--root")
        .arg(root)
        .output()
        .expect("run the linter");

    let parsed = serde_json::from_slice(&output.stdout).unwrap_or(serde_json::Value::Null);

    (output.status.code(), parsed)
}

/// Check, sweep, and check again leaves a tree clean and settled: the first
/// check counts what is missing, wrong and already right; the sweep inserts,
/// repairs and leaves alone exactly those; the second check passes; the author's
/// own line survives with the label indented to match; and a further sweep
/// changes nothing.
///
/// ´claim:cli:check-then-sweep-then-check-leaves-a-tree-clean-and-settled´
/// ´test:integration:a-fixture-package-tree-checks-and-then-fixes-clean´
#[test]
fn a_fixture_package_tree_checks_and_then_fixes_clean() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());
    track(root.path());

    let (code, before) = run(&["check"], root.path());

    assert_eq!(code, Some(3), "the fixture wants labels");
    assert_eq!(before["profile"]["covered"], 3);
    assert_eq!(before["profile"]["by_area"]["unit"], 1);
    assert_eq!(before["profile"]["by_area"]["crate"], 1);
    assert_eq!(before["profile"]["by_area"]["integration"], 1);
    assert_eq!(
        before["profile"]["labelled"], 1,
        "one test is already right"
    );
    assert_eq!(
        before["profile"]["missing"], 1,
        "one test has no label at all"
    );
    assert_eq!(
        before["profile"]["wrong"], 1,
        "one test attests the wrong label"
    );

    let (code, swept) = run(
        &["fix", "--profile", "test", "--write", "--allow-dirty"],
        root.path(),
    );

    assert_eq!(code, Some(0), "the sweep completes: {swept}");
    assert_eq!(swept["dry_run"], false);
    assert_eq!(swept["inserted"], 1);
    assert_eq!(swept["repaired"], 1);
    assert_eq!(swept["unchanged"], 1);
    assert_eq!(swept["refused"], 0);

    let (code, after) = run(&["check"], root.path());

    assert_eq!(code, Some(0), "the sweep leaves the fixture clean: {after}");
    assert_eq!(after["profile"]["labelled"], 3);
    assert_eq!(after["profile"]["missing"], 0);
    assert_eq!(after["profile"]["wrong"], 0);

    let unit = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");
    assert_eq!(
        unit,
        concat!(
            "#[cfg(test)]\nmod tests {\n",
            "    /// Checks it.\n    ///\n    /// \u{b4}test:unit:a-unit-test\u{b4}\n    #[test]\n    fn a_unit_test() {}\n}\n",
        ),
        "the author's line is preserved and the label indented with the attribute"
    );

    let (code, again) = run(
        &["fix", "--profile", "test", "--write", "--allow-dirty"],
        root.path(),
    );

    assert_eq!(code, Some(0));
    assert_eq!(again["inserted"], 0, "the sweep is idempotent");
    assert_eq!(again["repaired"], 0);
    assert_eq!(again["unchanged"], 3);
}

/// Through the command as through the library, the default dry run reports
/// what it would insert and leaves every file byte-for-byte as it was.
///
/// (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´)
/// ´test:integration:the-fix-command-writes-nothing-on-a-dry-run´
#[test]
fn the_fix_command_writes_nothing_on_a_dry_run() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());
    track(root.path());

    let before = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");
    let (code, report) = run(&["fix", "--profile", "test"], root.path());
    let after = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");

    assert_eq!(code, Some(0));
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["inserted"], 1, "it still counts what it would do");
    assert_eq!(before, after, "and leaves every file alone");
}

/// The sweep refuses to run over a tree carrying uncommitted changes and writes
/// nothing, so its edits always arrive as a diff a reviewer can read against a
/// clean baseline.
///
/// ´claim:cli:the-sweep-refuses-a-tree-with-uncommitted-changes´
/// ´test:integration:the-fix-command-refuses-a-dirty-tree´
#[test]
fn the_fix_command_refuses_a_dirty_tree() {
    let root = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(root.path());

    git_fixture::initialise_repository(root.path());

    let before = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");
    let (code, _report) = run(&["fix", "--profile", "test", "--write"], root.path());
    let after = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");

    assert_eq!(code, Some(1), "an unswept tree with changes is refused");
    assert_eq!(before, after, "and nothing is written");
}

/// Build a workspace of deficiency notices exercising the to-do sweep's cases.
///
/// One notice of each area is unlabelled, one already carries its derived label,
/// and one carries a stale one, so a sweep must write twice, rewrite once, and
/// leave one alone. One of the unlabelled notices carries the qualifier this
/// corpus already writes, and one stands after code as a trailing comment.
fn notice_workspace(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\".\", \"packages/demo\"]\n\n[package]\nname = \"torrust-fixture\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "// TODO(ADR-T-016 2026-08-19): read the policy flag before deciding\n\
         // and then act on what it says.\n\
         pub const LIMIT: usize = 0; // FIXME: raise the limit once measured\n",
    );
    write(
        root,
        "src/tests/mod.rs",
        "// TODO \u{b4}todo:test:a-stale-name\u{b4}: cover the recovery path\n",
    );
    write(
        root,
        "packages/demo/Cargo.toml",
        "[package]\nname = \"torrust-demo\"\n",
    );
    write(
        root,
        "packages/demo/src/lib.rs",
        "// TODO \u{b4}todo:code:already-right\u{b4}: already right\n",
    );
}

/// The same cycle settles the notice profile: the check counts the labelled, the
/// unlabelled and the wrongly attested; the sweep writes, rewrites and leaves
/// alone exactly those; the qualifier, the continuation line and the code before
/// a trailing marker all stand; and a further sweep changes nothing.
///
/// (´claim:cli:check-then-sweep-then-check-leaves-a-tree-clean-and-settled´)
/// ´test:integration:a-fixture-notice-tree-checks-and-then-fixes-clean´
#[test]
fn a_fixture_notice_tree_checks_and_then_fixes_clean() {
    let root = tempfile::tempdir().expect("temporary directory");
    notice_workspace(root.path());
    track(root.path());

    let (code, before) = run(&["check"], root.path());

    assert_eq!(
        code,
        Some(3),
        "the stale attestation fails from the first commit"
    );
    assert_eq!(before["todo"]["covered"], 4);
    assert_eq!(before["todo"]["by_area"]["code"], 3);
    assert_eq!(before["todo"]["by_area"]["test"], 1);
    assert_eq!(before["todo"]["labelled"], 1, "one notice is already right");
    assert_eq!(
        before["todo"]["unlabelled"], 2,
        "two are the register's family"
    );
    assert_eq!(
        before["todo"]["wrong"], 1,
        "one attests a label it does not derive"
    );

    let (code, swept) = run(
        &["fix", "--profile", "todo", "--write", "--allow-dirty"],
        root.path(),
    );

    assert_eq!(code, Some(0), "the sweep completes: {swept}");
    assert_eq!(swept["profile"], "todo");
    assert_eq!(swept["dry_run"], false);
    assert_eq!(swept["inserted"], 2);
    assert_eq!(swept["repaired"], 1);
    assert_eq!(swept["unchanged"], 1);
    assert_eq!(swept["refused"], 0);

    let (code, after) = run(&["check"], root.path());

    assert_eq!(code, Some(0), "the sweep leaves the fixture clean: {after}");
    assert_eq!(after["todo"]["labelled"], 4);
    assert_eq!(
        after["todo"]["unlabelled"], 0,
        "the register's family is empty"
    );
    assert_eq!(after["todo"]["wrong"], 0);

    let swept_source = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");
    assert_eq!(
        swept_source,
        concat!(
            "// TODO(ADR-T-016 2026-08-19) \u{b4}todo:code:read-the-policy-flag-before-deciding\u{b4}: ",
            "read the policy flag before deciding\n",
            "// and then act on what it says.\n",
            "pub const LIMIT: usize = 0; // FIXME \u{b4}todo:code:raise-the-limit-once-measured\u{b4}: ",
            "raise the limit once measured\n",
        ),
        "the qualifier, the continuation, and the code before a trailing marker all stand"
    );

    let (code, again) = run(
        &["fix", "--profile", "todo", "--write", "--allow-dirty"],
        root.path(),
    );

    assert_eq!(code, Some(0));
    assert_eq!(again["inserted"], 0, "the sweep is idempotent");
    assert_eq!(again["repaired"], 0);
    assert_eq!(again["unchanged"], 4);
    assert_eq!(again["files_changed"], 0);
}

/// The notice sweep reports its mode truthfully: without `--write` it counts
/// what it would change and leaves every file alone, while with `--write` it
/// reports a write and applies the changes.
///
/// (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´)
/// ´test:integration:the-todo-sweep-writes-nothing-on-a-dry-run´
#[test]
fn the_todo_sweep_writes_nothing_on_a_dry_run() {
    let root = tempfile::tempdir().expect("temporary directory");
    notice_workspace(root.path());
    track(root.path());

    let before = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");
    let (code, report) = run(&["fix", "--profile", "todo"], root.path());
    let after = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");

    assert_eq!(code, Some(0));
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["inserted"], 2, "it still counts what it would do");
    assert_eq!(before, after, "and leaves every file alone");

    let (code, report) = run(
        &["fix", "--profile", "todo", "--write", "--allow-dirty"],
        root.path(),
    );
    let written = fs::read_to_string(root.path().join("src/lib.rs")).expect("read");

    assert_eq!(code, Some(0));
    assert_eq!(report["dry_run"], false);
    assert_ne!(before, written, "and the write mode applies its changes");
}

/// The burn list of one fixture tree: prose under its documentation and comments
/// under its sources.
fn fixture_burn_list() -> BurnList {
    BurnList::new(
        "section references",
        Shape::Legacy(&[LegacyRule::SectionNumber]),
        vec![PathBuf::from("docs")],
        vec![PathBuf::from("src")],
    )
}

/// The ratchet a fixture declares, in the rows the canonical list carries.
fn declared(rows: &[(&str, usize)]) -> Vec<RegisterRow> {
    rows.iter()
        .map(|(path, count)| {
            RegisterRow::new(*path, *count, Location::new(".linter/lists.toml", "", 0))
        })
        .collect()
}

/// Census a fixture tree and judge its declared ratchet, as the check does.
fn ratchet(root: &Path, list: &BurnList, rows: &[(&str, usize)]) -> Vec<&'static str> {
    let plan = plan(root);
    let (taken, census_findings) = census(root, list, plan.topology().corpus());

    assert_eq!(codes(&census_findings), Vec::<&str>::new());

    codes(&verify(list, &taken, &declared(rows)))
}

/// The ratchet binds in both directions: the census as found passes, one more
/// reference fails as growth with the new occurrence named, and one fewer fails
/// as a stale row until the list falls with it. A file losing its last reference
/// leaves the list rather than standing at zero.
///
/// ´claim:burn:the-ratchet-fails-on-growth-and-on-a-list-that-overstates´
/// ´test:integration:a-burn-list-fails-on-growth-and-on-a-stale-row´
#[test]
fn a_burn_list_fails_on_growth_and_on_a_stale_row() {
    let root = tempfile::tempdir().expect("temporary directory");
    let list = fixture_burn_list();

    write(
        root.path(),
        "docs/note.md",
        "As §10.3 requires, and §6.1 too.\n",
    );
    write(root.path(), "src/lib.rs", "//! Per §4.2.\nfn f() {}\n");
    track(root.path());

    let held = [("docs/note.md", 2), ("src/lib.rs", 1)];

    assert_eq!(
        ratchet(root.path(), &list, &held),
        Vec::<&str>::new(),
        "the census as found is the ratchet, so the gate is clean"
    );

    // Growth: one more reference than the list accounts for.
    write(
        root.path(),
        "docs/note.md",
        "As §10.3 requires, and §6.1 too, and §7.4.\n",
    );
    assert_eq!(
        ratchet(root.path(), &list, &held),
        ["burn_list_growth"],
        "adding a section reference anywhere fails"
    );

    // And the finding names the occurrence that took the file over.
    let plan = plan(root.path());
    let (grown, _findings) = census(root.path(), &list, plan.topology().corpus());
    let rendered: Vec<String> = verify(&list, &grown, &declared(&held))
        .iter()
        .map(ToString::to_string)
        .collect();

    assert!(
        rendered
            .iter()
            .any(|finding| finding.contains("§7.4") && finding.contains("docs/note.md")),
        "the growth names the new occurrence: {rendered:#?}"
    );

    // Shrinkage: fewer references than the list accounts for, and the list is
    // stale until it falls with them.
    write(root.path(), "docs/note.md", "As §10.3 requires.\n");
    assert_eq!(
        ratchet(root.path(), &list, &held),
        ["stale_burn_entry"],
        "removing a section reference fails until the list shrinks"
    );

    assert_eq!(
        ratchet(
            root.path(),
            &list,
            &[("docs/note.md", 1), ("src/lib.rs", 1)]
        ),
        Vec::<&str>::new(),
        "and passes once the list shrinks with it"
    );

    // A file that loses its last reference leaves the list outright.
    write(root.path(), "docs/note.md", "No reference at all.\n");
    assert_eq!(
        ratchet(
            root.path(),
            &list,
            &[("docs/note.md", 1), ("src/lib.rs", 1)]
        ),
        ["stale_burn_entry"]
    );
    assert_eq!(
        ratchet(root.path(), &list, &[("src/lib.rs", 1)]),
        Vec::<&str>::new(),
        "the emptied file leaves the list rather than standing at zero"
    );
}

/// The burn command reports and writes nothing unless asked: every adopted
/// family is registered at its census, none is rewritten, and the trees excluded
/// from each census are named in the report rather than left to be remembered.
///
/// ´claim:cli:the-burn-command-reports-and-writes-nothing-by-default´
/// ´test:integration:the-burn-command-emits-json-and-writes-nothing-by-default´
#[test]
fn the_burn_command_emits_json_and_writes_nothing_by_default() {
    let root = tempfile::tempdir().expect("temporary directory");

    // A tree with nothing for any family to count. What is being read here is
    // the report and the mutation, and a census at zero says both without
    // pinning a population: the run either emits its object and leaves the tree
    // alone or it does not.
    // A census looks where the corpus says it does, and it says so in the
    // document of the policy that activates it, so every family's surface is
    // declared here rather than remembered by the binary. What it never reads is
    // derived from the same declaration and written down nowhere: the checker's
    // own share, and for a repository-scoped family the shares whose owners hold
    // the per-owner sibling of its program.
    declare(
        root.path(),
        "owners = [\"INDEX\", \"ASSAYER\", \"LINTER\"]\n\
         partitions = [{ name = \"declared-surface\", owner = \"INDEX\", pattern = '%s\".linter\" [ \"/\" *VCHAR ]' }, \
         { name = \"assayer-package\", owner = \"ASSAYER\", pattern = '%s\"packages/assayer\" [ \"/\" *VCHAR ]' }, \
         { name = \"linter-package\", owner = \"LINTER\", pattern = '%s\"packages/linter\" [ \"/\" *VCHAR ]' }]\n\
         may_cite = []\n",
        "environments = []\n",
        &censused_policies(),
        &censused_lists(),
    );

    for (domain, owner, prose, code) in CENSUS_SURFACES {
        let mut text = format!(
            "namespace = \"com.torrust.index.linter.policy.{domain}\"\nversion = [1, 0, 0]\n\n[owners.{owner}]\n"
        );

        for (key, pattern) in [("prose", prose), ("code", code)] {
            if !pattern.is_empty() {
                let _ignored = writeln!(text, "{key} = '{pattern}'");
            }
        }

        write(
            root.path(),
            &format!(".linter/policy-{}.toml", domain.replace('.', "-")),
            &text,
        );
    }

    let before = tree(root.path());
    let (code, report) = run(&["burn"], root.path());

    assert_eq!(
        code,
        Some(0),
        "a census the ratchet accounts for exits clean: {report}"
    );
    assert!(
        report.get("schema").is_none(),
        "the burn report states no version of itself"
    );
    assert_eq!(report["write"], false);
    assert_eq!(report["failures"], 0);

    let families = report["families"].as_array().expect("families");

    assert!(!families.is_empty(), "every adopted family is reported");

    for family in families {
        assert_eq!(
            family["occurrences"], family["registered"],
            "{} is registered at its census",
            family["family"]
        );
        assert!(
            !family["excluded"]
                .as_array()
                .expect("exclusions")
                .is_empty(),
            "{} names the trees it does not read rather than leaving them to be remembered",
            family["family"]
        );
    }

    assert_eq!(
        tree(root.path()),
        before,
        "a default run writes not one byte"
    );
}

/// The censused families, by the domain whose document declares each surface.
///
/// Seven stand in the Assayer's share and one is the to-do census over the
/// members, which is the root owner's; the last two are the repository-scoped
/// halves, which name the corpus entire and are the only ones a sibling
/// activation cuts anything out of. Three of the ten stand in a domain named for
/// the shape they read rather than for their own policy, because a census of a
/// shape belongs beside the declaration of that shape.
const CENSUS_SURFACES: [(&str, &str, &str, &str); 11] = [
    (
        "legacy.section-references",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    (
        "legacy.record-references",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    (
        "legacy.unprefixed-record-references",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    ("legacy.tag-references", "ASSAYER", ASSAYER_PROSE, ""),
    (
        "references.scenarios",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    (
        "references.divisions",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    (
        "references.prefix-numbers",
        "ASSAYER",
        ASSAYER_PROSE,
        ASSAYER_CODE,
    ),
    ("legacy.todos", "INDEX", "", MEMBER_CODE),
    ("legacy.implementation", "ASSAYER", "", ASSAYER_CODE),
    (
        "legacy.section-references-repository",
        "INDEX",
        CORPUS_SURFACE,
        CORPUS_SURFACE,
    ),
    (
        "legacy.record-references-repository",
        "INDEX",
        CORPUS_SURFACE,
        CORPUS_SURFACE,
    ),
];

/// The prose tree the Assayer's censuses walk in this fixture.
const ASSAYER_PROSE: &str = r#"%s"packages/assayer/docs" [ "/" *VCHAR ]"#;

/// The code tree the Assayer's censuses walk in this fixture.
const ASSAYER_CODE: &str = r#"%s"packages/assayer/src" [ "/" *VCHAR ]"#;

/// The trees the to-do census walks, which are the members rather than one share.
const MEMBER_CODE: &str = r#"( %s"src" / %s"packages" ) [ "/" *VCHAR ]"#;

/// The corpus entire, which is how a repository-scoped census states its surface.
const CORPUS_SURFACE: &str = "*VCHAR";

/// The activations the ten censuses stand at, each at the owner whose debt it is.
fn censused_policies() -> String {
    let rows: Vec<String> = census_activations()
        .map(|(owner, policy)| format!("{{ owner = \"{owner}\", policy = \"{policy}\" }}"))
        .collect();

    format!(
        "policies = [{{ owner = \"ASSAYER\", policy = \"labels.mints-well-formed\" }}, {}]\n",
        rows.join(", ")
    )
}

/// The empty ratchet each of those activations carries, because an activation wants one.
fn censused_lists() -> String {
    census_activations().fold(
        String::from("[ASSAYER.\"labels.mints-well-formed\"]\nallowances = []\n"),
        |mut lists, (owner, policy)| {
            let _ignored = writeln!(lists, "[{owner}.\"{policy}\"]\npath_counts = []");
            lists
        },
    )
}

/// The owner and policy of each censused family, joined as the catalog joins them.
fn census_activations() -> impl Iterator<Item = (&'static str, &'static str)> {
    [
        ("ASSAYER", "legacy.section-references"),
        ("ASSAYER", "legacy.record-references"),
        ("ASSAYER", "legacy.unprefixed-record-references"),
        ("ASSAYER", "legacy.tag-references"),
        ("ASSAYER", "legacy.scenario-numbers"),
        ("ASSAYER", "legacy.division-names"),
        ("ASSAYER", "legacy.residual-litter"),
        ("ASSAYER", "legacy.todos"),
        ("ASSAYER", "legacy.implementation"),
        ("INDEX", "legacy.section-references-repository"),
        ("INDEX", "legacy.record-references-repository"),
    ]
    .into_iter()
}

/// Every file of a tree with its bytes, for the assertion that a run wrote none.
fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .expect("read the fixture tree")
            .flatten()
        {
            let path = entry.path();

            if path.is_dir() {
                pending.push(path);
            } else {
                let held = fs::read(&path).expect("read a fixture file");

                found.push((path, held));
            }
        }
    }

    found.sort();
    found
}

/// Write the five declared files into a tree's declaration directory.
///
/// The four declarations are written under the envelope allocated for each
/// schema, because the loader requires one of every declared file before it
/// interprets a single declaration. What a caller passes is therefore the
/// declaration it is testing rather than the header every declared file repeats.
fn declare(root: &Path, owners: &str, environments: &str, policies: &str, lists: &str) {
    for (file, schema, text) in [
        ("owners", "owners", owners),
        ("environments", "environments", environments),
        ("policies", "policies", policies),
        ("lists", "lists", lists),
        (
            "shape",
            "shape",
            "universe = \"git-tracked\"\n\nignore = []\n",
        ),
    ] {
        write(
            root,
            &format!(".linter/{file}.toml"),
            &format!(
                "namespace = \"com.torrust.index.linter.{schema}\"\nversion = [1, 0, 0]\n\n{text}"
            ),
        );
    }
}

/// Give legacy semantic fixtures an explicit, untracked declaration without
/// changing the tracked corpus they exercise.
fn ensure_fixture_surface(root: &Path) {
    if root.join(".linter").is_dir() {
        return;
    }

    if root.join("packages/demo").is_dir() {
        declare(
            root,
            "owners = [\"INDEX\", \"DEMO\"]\n\
             partitions = [\
               { name = \"fixture-root\", owner = \"INDEX\", pattern = '(%s\"Cargo.toml\" / %s\"README.md\" / %s\"adr\" [ \"/\" *VCHAR ] / %s\"docs\" [ \"/\" *VCHAR ] / %s\"src\" [ \"/\" *VCHAR ])' },\
               { name = \"demo-package\", owner = \"DEMO\", pattern = '%s\"packages/demo\" [ \"/\" *VCHAR ]' }\
             ]\n\
             may_cite = []\n",
            FIXTURE_ENVIRONMENTS,
            "policies = [\
               { owner = \"INDEX\", policy = \"labels.mints-well-formed\" },\
               { owner = \"DEMO\", policy = \"labels.mints-well-formed\" }\
             ]\n",
            "[INDEX.\"labels.mints-well-formed\"]\nallowances = []\n\n\
             [DEMO.\"labels.mints-well-formed\"]\nallowances = []\n",
        );
    } else {
        declare(
            root,
            "owners = [\"INDEX\"]\n\
             partitions = [{ name = \"fixture\", owner = \"INDEX\", pattern = '*VCHAR' }]\n\
             may_cite = []\n",
            FIXTURE_ENVIRONMENTS,
            "policies = [{ owner = \"INDEX\", policy = \"labels.mints-well-formed\" }]\n",
            "[INDEX.\"labels.mints-well-formed\"]\nallowances = []\n",
        );
    }
}

/// Make a fixture tree a repository whose tracked corpus is everything in it.
///
/// The partition's accounting universe is the tracked corpus rather than the
/// physical tree, so a fixture that means to be partitioned has to be a
/// repository. An untracked tree has an empty universe, against which every
/// inclusion row is vacuously total and no path can be unaccounted — the
/// opposite of what these fixtures exist to show.
///
/// The index is built directly so neither an external executable nor ambient
/// ignore configuration can decide what the fixture tracks.
fn track(root: &Path) {
    git_fixture::track_all(root);
}

/// The owner file a declared fixture starts from, accounting the whole tree.
const DECLARED_OWNERS: &str = "owners = [\"INDEX\"]\n\
    partitions = [{ name = \"decision-records\", owner = \"INDEX\", pattern = '%s\"adr\" [ \"/\" *VCHAR ]' }, \
    { name = \"declared-surface\", owner = \"INDEX\", pattern = '%s\".linter\" [ \"/\" *VCHAR ]' }]\n\
    may_cite = []\n";

/// The activation file a declared fixture starts from.
const DECLARED_POLICIES: &str =
    "policies = [{ owner = \"INDEX\", policy = \"labels.mints-well-formed\" }]\n";

/// The list file a declared fixture starts from.
const DECLARED_LISTS: &str = "[INDEX.\"labels.mints-well-formed\"]\nallowances = []\n";

/// A tree with no declaration directory refuses with an empty stdout before
/// command dispatch, because the required shape document is absent and no
/// universe or global-ignore relation can be resolved.
///
/// ´claim:cli:an-absent-declaration-refuses-without-running-the-command´
/// ´test:integration:an-absent-declaration-refuses-without-running-the-command´
#[test]
fn an_absent_declaration_refuses_without_running_the_command() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Head · `sec:fixture:head`\n\nCites (`sec:fixture:head`).\n",
    );
    track(root.path());

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(["check", "--root"])
        .arg(root.path())
        .output()
        .expect("run the linter");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"");

    let refusal: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("one refusal record on stderr");

    assert_eq!(
        refusal["fields"]["refusal"],
        ".linter/shape.toml: the snapshot requires this file"
    );
    assert_eq!(
        refusal["fields"]["message"],
        "the declared configuration is not a snapshot"
    );
}

/// A declaration that is not a snapshot refuses the command entire: the exit is
/// the shared failure class rather than the findings class, and stdout is
/// empty. A command that cannot read its own configuration has no standing to
/// say anything about the corpus, so it says nothing rather than reporting a
/// verdict it could not have formed.
///
/// ´claim:cli:a-refused-snapshot-exits-one-with-an-empty-stdout´
/// ´test:integration:a-refused-snapshot-exits-one-with-an-empty-stdout´
#[test]
fn a_refused_snapshot_exits_one_with_an_empty_stdout() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Head · `sec:fixture:head`\n",
    );

    declare(
        root.path(),
        DECLARED_OWNERS,
        "environments = []\n",
        "policies = [{ owner = \"INDEX\", policy = \"labels.mints-unheard-of\" }]\n",
        DECLARED_LISTS,
    );

    for command in ["check", "burn", "coverage", "shape"] {
        let output = Command::new(env!("CARGO_BIN_EXE_linter"))
            .args([command, "--root"])
            .arg(root.path())
            .output()
            .expect("run the linter");

        assert_eq!(output.status.code(), Some(1), "{command}");
        assert!(output.stdout.is_empty(), "{command}");
    }
}

/// A profile activated without the label calculus it reads refuses every
/// command before dispatch. The shared configuration-failure surface keeps
/// stdout empty and names the missing same-owner label prerequisite on stderr,
/// so an incomplete declaration cannot silently run a partial policy.
///
/// ´claim:cli:a-dependent-policy-without-label-prerequisites-refuses-loudly´
/// ´test:integration:a-dependent-policy-without-label-prerequisites-refuses-loudly´
#[test]
fn a_dependent_policy_without_label_prerequisites_refuses_loudly() {
    let root = tempfile::tempdir().expect("temporary directory");

    declare(
        root.path(),
        "owners = [\"INDEX\"]\n\
         partitions = [{ name = \"fixture\", owner = \"INDEX\", pattern = '*VCHAR' }]\n\
         may_cite = []\n",
        "environments = []\n",
        "policies = [{ owner = \"INDEX\", policy = \"profile.tests-conform\" }]\n",
        "[INDEX.\"profile.tests-conform\"]\nallowances = []\n",
    );
    track(root.path());

    for (name, arguments) in [
        ("check", &["check"][..]),
        ("report", &["report"][..]),
        ("coverage", &["coverage"][..]),
        ("shape", &["shape"][..]),
        ("assemble", &["assemble"][..]),
        ("burn", &["burn"][..]),
        ("project", &["project"][..]),
        ("fix-test", &["fix", "--profile", "test"][..]),
        ("fix-todo", &["fix", "--profile", "todo"][..]),
        (
            "burn-audit",
            &["burn", "--audit", "--output", "response.json"][..],
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_linter"))
            .env("RUST_LOG", "error")
            .args(arguments)
            .args(["--root"])
            .arg(root.path())
            .output()
            .expect("run the linter");

        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty(), "{name}: {:?}", output.stdout);

        let stderr = String::from_utf8(output.stderr).expect("JSON diagnostics are UTF-8");
        assert!(
            stderr.contains(
                "policy dependency: INDEX : profile.tests-conform: missing same-owner pair INDEX : labels.mints-well-formed"
            ),
            "{name}: {stderr}"
        );
    }
}

/// A snapshot whose declaration is complete but whose partition disagrees with
/// the tree is judged rather than refused: the run happens, the object is
/// complete, and the exit is the findings class with the unaccounted path named
/// in the report.
///
/// ´claim:cli:a-parsed-snapshot-that-disagrees-is-judged-rather-than-refused´
/// ´test:integration:a-parsed-snapshot-that-disagrees-is-judged-rather-than-refused´
#[test]
fn a_parsed_snapshot_that_disagrees_is_judged_rather_than_refused() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Head · `sec:fixture:head`\n",
    );
    write(root.path(), "stray.md", "nothing accounts for this\n");

    declare(
        root.path(),
        DECLARED_OWNERS,
        "environments = []\n",
        DECLARED_POLICIES,
        DECLARED_LISTS,
    );
    track(root.path());

    let output = Command::new(env!("CARGO_BIN_EXE_linter"))
        .args(["check", "--root"])
        .arg(root.path())
        .output()
        .expect("run the linter");

    assert_eq!(output.status.code(), Some(3));

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON object on stdout");

    assert_eq!(report["clean"], false);
    assert_eq!(report["configuration"]["pairs"], 1);
    assert_eq!(report["configuration"]["partition"]["unaccounted"], 1);
    assert_eq!(report["configuration"]["missing_dependencies"], 0);

    let raised: Vec<&str> = report["findings"]
        .as_array()
        .expect("an array of findings")
        .iter()
        .filter_map(|finding| finding["code"].as_str())
        .collect();

    assert!(raised.contains(&"unaccounted_path"), "{raised:?}");
}

/// Every writing mode refuses to mutate under a declaration that disagrees with
/// the tree. That is stricter than the exit codes alone require and
/// deliberately so: a writer that proceeded would record a conclusion drawn
/// from a question the corpus had not answered, so it exits the failure class
/// with an empty stdout instead.
///
/// ´claim:cli:a-writing-mode-refuses-a-declaration-that-disagrees-with-the-tree´
/// ´test:integration:a-writing-mode-refuses-a-declaration-that-disagrees-with-the-tree´
#[test]
fn a_writing_mode_refuses_a_declaration_that_disagrees_with_the_tree() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "## Head · `sec:fixture:head`\n",
    );
    write(root.path(), "stray.md", "nothing accounts for this\n");

    declare(
        root.path(),
        DECLARED_OWNERS,
        "environments = []\n",
        DECLARED_POLICIES,
        DECLARED_LISTS,
    );
    track(root.path());

    for command in ["burn", "project", "assemble"] {
        let refused = Command::new(env!("CARGO_BIN_EXE_linter"))
            .args([command, "--root"])
            .arg(root.path())
            .arg("--write")
            .output()
            .expect("run the linter");

        assert_eq!(refused.status.code(), Some(1), "{command}");
        assert!(refused.stdout.is_empty(), "{command}");

        let read_only = Command::new(env!("CARGO_BIN_EXE_linter"))
            .args([command, "--root"])
            .arg(root.path())
            .output()
            .expect("run the linter");

        assert!(!read_only.stdout.is_empty(), "{command}");
    }
}

/// The nine genre rows are the whole of the Document arrangement, so a title
/// minting the kind that once stood beside them is now answered for at the
/// catalogue like any other head: the name is catalogued, the kind is not
/// catalogued under it, and the nine senses the name does carry are named back.
/// Nothing is written specially for the departed kind — the row that admitted it
/// is simply gone, and its absence is the whole of the judgment.
///
/// (´claim:head:a-title-validates-on-every-kind-the-relation-rows-its-environment´)
/// ´test:integration:a-title-minting-the-departed-document-kind-fails-at-the-catalogue´
#[test]
fn a_title_minting_the_departed_document_kind_fails_at_the_catalogue() {
    const GENRES: &str = "environments = [\
        { environment = \"Document\", kind = \"rec\" }, \
        { environment = \"Document\", kind = \"rep\" }, \
        { environment = \"Document\", kind = \"reg\" }, \
        { environment = \"Document\", kind = \"log\" }, \
        { environment = \"Document\", kind = \"proposal\" }, \
        { environment = \"Document\", kind = \"spec\" }, \
        { environment = \"Document\", kind = \"thesis\" }, \
        { environment = \"Document\", kind = \"plan\" }, \
        { environment = \"Document\", kind = \"guide\" }]\n";

    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-one.md",
        "# A Titled Document · `doc:fixture:document`\n",
    );
    declare(
        root.path(),
        DECLARED_OWNERS,
        GENRES,
        DECLARED_POLICIES,
        DECLARED_LISTS,
    );
    track(root.path());

    let (code, report) = run(&["check"], root.path());

    assert_eq!(code, Some(3), "no row admits the departed kind: {report}");
    assert_eq!(report["findings"][0]["code"], "misclassified_head");
    assert_eq!(report["findings"][0]["base"], "Document");

    let catalogued = report["findings"][0]["catalogued"]
        .as_array()
        .expect("the kinds the name does carry")
        .len();

    assert_eq!(
        catalogued, 9,
        "and the name answers with its nine senses: {report}"
    );

    write(
        root.path(),
        "adr/001-one.md",
        "# A Titled Document · `rec:fixture:document`\n",
    );
    track(root.path());

    let (code, repaired) = run(&["check"], root.path());

    assert_eq!(code, Some(0), "while a rowed genre passes: {repaired}");
    assert_eq!(repaired["heads_validated"], 1);
}

/// Run one command from inside its fixture so every reported root is `.`.
fn observable(root: &Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_linter"))
        .env("RUST_LOG", "off")
        .current_dir(root)
        .args(arguments)
        .args(["--root", "."])
        .output()
        .expect("run the characterized command")
}

/// Run one command with caller-supplied standard input from inside its fixture.
fn observable_with_input(root: &Path, arguments: &[&str], input: &[u8]) -> std::process::Output {
    use std::io::Write as _;

    let mut child = Command::new(env!("CARGO_BIN_EXE_linter"))
        .env("RUST_LOG", "off")
        .current_dir(root)
        .args(arguments)
        .args(["--root", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the characterized command");

    child
        .stdin
        .take()
        .expect("piped standard input")
        .write_all(input)
        .expect("write the characterized request");

    child
        .wait_with_output()
        .expect("run the characterized command")
}

/// The lowercase SHA-256 spelling used by every byte oracle.
fn byte_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        digest.push(char::from(HEX[usize::from(byte >> 4)]));
        digest.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    digest
}

/// Capture base bytes while blessing, otherwise compare them with the frozen oracle.
fn frozen_bytes(name: &str, bytes: &[u8], expected_len: usize, expected_digest: &str) {
    if let Some(directory) = std::env::var_os("LINTER_CHARACTERIZATION_CAPTURE") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).expect("create the characterization capture directory");
        fs::write(directory.join(name), bytes).expect("capture characterized bytes");
    } else {
        assert_eq!(bytes.len(), expected_len, "{name} byte length");
        assert_eq!(byte_digest(bytes), expected_digest, "{name} byte digest");
    }
}

/// Fix one subprocess's three observable channels.
fn frozen_observable(
    name: &str,
    output: &std::process::Output,
    expected_status: i32,
    expected_len: usize,
    expected_digest: &str,
) {
    assert_eq!(
        output.status.code(),
        Some(expected_status),
        "{name} status: stdout={:?}, stderr={:?}",
        output.stdout,
        output.stderr
    );
    assert_eq!(output.stderr, b"", "{name} stderr");
    frozen_bytes(
        &format!("{name}.stdout"),
        &output.stdout,
        expected_len,
        expected_digest,
    );
    frozen_bytes(
        &format!("{name}.status"),
        format!("{expected_status}\n").as_bytes(),
        2,
        &byte_digest(format!("{expected_status}\n").as_bytes()),
    );
    frozen_bytes(
        &format!("{name}.stderr"),
        &output.stderr,
        0,
        &byte_digest(b""),
    );
}

/// A two-finding prose tree used to pin ordering as well as the report bytes.
fn characterization_prose_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "adr/001-first.md",
        "Cites (`sec:fixture:first-missing`).\n",
    );
    write(
        root.path(),
        "adr/002-second.md",
        "Cites (`sec:fixture:second-missing`).\n",
    );
    track(root.path());
    root
}

/// Every command class refuses with the same empty data streams when the
/// required shape document is absent from an invented root.
///
/// ´claim:harness:no-surface-refusal-fixes-every-observable-byte´
/// ´test:integration:characterizes-no-surface-bytes´
#[test]
fn characterizes_no_surface_bytes() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "Cargo.toml",
        "[package]\nname = \"fictional-root\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    );
    write(root.path(), "src/lib.rs", "pub fn invented() {}\n");
    write(root.path(), "README.md", "# Fictional root\n");
    track(root.path());

    for (name, arguments) in [
        ("no-surface-check", &["check"][..]),
        ("no-surface-report", &["report"][..]),
        ("no-surface-coverage", &["coverage"][..]),
        ("no-surface-shape", &["shape"][..]),
        ("no-surface-burn", &["burn"][..]),
        (
            "no-surface-control",
            &["burn", "--audit", "--output", "response.json"][..],
        ),
        ("no-surface-project", &["project"][..]),
        ("no-surface-assemble", &["assemble"][..]),
        ("no-surface-fix-test", &["fix", "--profile", "test"][..]),
        ("no-surface-fix-todo", &["fix", "--profile", "todo"][..]),
    ] {
        let output = observable(root.path(), arguments);
        frozen_observable(
            name,
            &output,
            1,
            0,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }
}

/// A declared burn surface with one registered occurrence and nine empty families.
fn characterization_census_fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary directory");
    let lists = censused_lists().replacen(
        "path_counts = []",
        "path_counts = [\n  { path = \"packages/assayer/docs/debt.md\", maximum = 1 },\n]",
        1,
    );

    declare(
        root.path(),
        "owners = [\"INDEX\", \"ASSAYER\", \"LINTER\"]\n\
         partitions = [{ name = \"declared-surface\", owner = \"INDEX\", pattern = '%s\".linter\" [ \"/\" *VCHAR ]' }, \
         { name = \"assayer-package\", owner = \"ASSAYER\", pattern = '%s\"packages/assayer\" [ \"/\" *VCHAR ]' }, \
         { name = \"linter-package\", owner = \"LINTER\", pattern = '%s\"packages/linter\" [ \"/\" *VCHAR ]' }]\n\
         may_cite = []\n",
        "environments = []\n",
        &censused_policies(),
        &lists,
    );

    for (domain, owner, prose, code) in CENSUS_SURFACES {
        let mut text = format!(
            "namespace = \"com.torrust.index.linter.policy.{domain}\"\nversion = [1, 0, 0]\n\n[owners.{owner}]\n"
        );

        for (key, pattern) in [("prose", prose), ("code", code)] {
            if !pattern.is_empty() {
                let _ignored = writeln!(text, "{key} = '{pattern}'");
            }
        }

        write(
            root.path(),
            &format!(".linter/policy-{}.toml", domain.replace('.', "-")),
            &text,
        );
    }

    write(
        root.path(),
        "packages/assayer/docs/debt.md",
        "The retired locator is §1.\n",
    );
    track(root.path());
    root
}

/// A projection fixture with one authored claim, one citing test, and an empty test folder.
fn characterization_projection_fixture() -> tempfile::TempDir {
    let acute = '\u{b4}';
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"packages/demo\"]\n",
    );
    write(
        root.path(),
        "packages/demo/Cargo.toml",
        "[package]\nname = \"torrust-demo\"\n",
    );
    write(
        root.path(),
        "packages/demo/src/tests/resonance.rs",
        &format!(
            "/// The widths are identical across the sweep.\n///\n\
             /// {acute}claim:resonance:crossover-widths{acute}\n\
             /// {acute}test:crate:widths-are-identical{acute}\n\
             #[test]\nfn widths_are_identical() {{}}\n\n\
             /// ({acute}claim:resonance:crossover-widths{acute})\n\
             /// {acute}test:crate:widths-survive-the-surface{acute}\n\
             #[test]\nfn widths_survive_the_surface() {{}}\n"
        ),
    );
    write(
        root.path(),
        "packages/demo/src/maths/norms.rs",
        "pub fn norm() {}\n",
    );
    declare(
        root.path(),
        "owners = [\"DEMO\"]\n\
         partitions = [{ name = \"fixture\", owner = \"DEMO\", pattern = '*VCHAR' }]\n\
         may_cite = []\n",
        "environments = []\n",
        "policies = [\n\
         { owner = \"DEMO\", policy = \"labels.mints-well-formed\" },\n\
         { owner = \"DEMO\", policy = \"profile.tests-conform\" },\n\
         { owner = \"DEMO\", policy = \"profile.constants-conform\" },\n\
         { owner = \"DEMO\", policy = \"projection.test-indexes-current\" },\n\
         { owner = \"DEMO\", policy = \"projection.test-matrices-current\" },\n\
         { owner = \"DEMO\", policy = \"projection.constant-pins-current\" },\n\
         ]\n",
        "[DEMO.\"labels.mints-well-formed\"]\nallowances = []\n\n\
         [DEMO.\"profile.tests-conform\"]\nallowances = []\n\n\
         [DEMO.\"profile.constants-conform\"]\nallowances = []\n\n\
         [DEMO.\"projection.test-indexes-current\"]\nallowances = []\n\n\
         [DEMO.\"projection.test-matrices-current\"]\nallowances = []\n\n\
         [DEMO.\"projection.constant-pins-current\"]\nallowances = []\n",
    );
    write(
        root.path(),
        ".linter/policy-owner-names.toml",
        "namespace = \"com.torrust.index.linter.policy.owner.names\"\n\
         version = [1, 0, 0]\n\n\
         [set.name-prefix-ignore]\n\
         torrust = \"torrust-\"\n",
    );
    track(root.path());
    root
}

/// A check over this deliberately undeclared fixture refuses before findings
/// can be ordered.
///
/// ´claim:harness:check-fixes-every-observable-byte´
/// ´test:integration:characterizes-check-bytes´
#[test]
fn characterizes_check_bytes() {
    let root = characterization_prose_fixture();
    let output = observable(root.path(), &["check"]);
    frozen_observable(
        "check",
        &output,
        1,
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

/// Each informational command refuses before dispatch over this deliberately
/// undeclared fixture.
///
/// ´claim:harness:informational-commands-have-independent-byte-oracles´
/// ´test:integration:characterizes-informational-bytes´
#[test]
fn characterizes_informational_bytes() {
    let root = characterization_prose_fixture();

    for (name, arguments) in [
        ("report", &["report"][..]),
        ("coverage", &["coverage"][..]),
        ("shape", &["shape"][..]),
    ] {
        let output = observable(root.path(), arguments);
        frozen_observable(
            name,
            &output,
            1,
            0,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }
}

/// The burn report fixes its census rows and every surrounding report byte.
///
/// ´claim:harness:burn-fixes-census-rows-and-report-bytes´
/// ´test:integration:characterizes-burn-bytes´
#[test]
fn characterizes_burn_bytes() {
    let root = characterization_census_fixture();
    let output = observable(root.path(), &["burn"]);
    frozen_observable(
        "burn",
        &output,
        0,
        2360,
        "0d5bc5325df9ce9ce9492b31b9f804c9c369d6f35937006c54f30287652a620b",
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("burn JSON");
    let families = report["families"].as_array().expect("families");
    assert_eq!(families.len(), CENSUS_SURFACES.len());
    assert_eq!(
        families[0]["rows"][0]["path"],
        "packages/assayer/docs/debt.md"
    );
    assert_eq!(families[0]["rows"][0]["count"], 1);
}

/// The control response file and stdout are one frozen byte sequence.
///
/// ´claim:harness:control-response-and-stdout-are-one-byte-sequence´
/// ´test:integration:characterizes-control-bytes´
#[test]
fn characterizes_control_bytes() {
    let root = characterization_census_fixture();
    let request = r#"{"schema":1,"operation":"audit","targets":[{"owner":"INDEX","policy":"legacy.section-references-repository","syntax":"path-count","rows":[],"authority":{"authorized_by":"the owner","ruling":"bite 1 characterization","reason":"freeze the control bytes"}}]}"#;

    let output = observable_with_input(
        root.path(),
        &["burn", "--audit", "--output", "response.json"],
        request.as_bytes(),
    );
    frozen_observable(
        "control",
        &output,
        0,
        612,
        "6ad64e0bf57f7eb120f2582319d545b67238ed0dd6b89deffcb4afca9086a2b9",
    );
    let response = fs::read(root.path().join("response.json")).expect("control response");
    assert_eq!(output.stdout, response);
    frozen_bytes(
        "control.response",
        &response,
        612,
        "6ad64e0bf57f7eb120f2582319d545b67238ed0dd6b89deffcb4afca9086a2b9",
    );
}

/// Projection fixes the report and every file the writer owns.
///
/// ´claim:harness:projection-fixes-report-and-generated-file-bytes´
/// ´test:integration:characterizes-projection-bytes´
#[test]
fn characterizes_projection_bytes() {
    let root = characterization_projection_fixture();
    let output = observable(root.path(), &["project", "--write"]);
    frozen_observable(
        "projection",
        &output,
        0,
        267,
        "2949efc2ac292b94f923613417fccda0b65f6a4316dd6ecaa05d16cf04049ab3",
    );

    for (relative, expected_len, expected_digest) in [
        (
            "packages/demo/src/tests/resonance.rs",
            568,
            "527194c916b8ff543391d3982e1b475112569ef6486b5d7cb46ad4f4bf33aee6",
        ),
        (
            "packages/demo/src/tests/README.md",
            333,
            "bd610beed4752877bc5b5d25b57c0ccd9e8f0fb646df483fd533c68e9e31deb6",
        ),
        (
            "packages/demo/src/maths/README.md",
            118,
            "74737039afad2661c146aa3d4408dfa9c4d526c5668d46a419197f509877bb58",
        ),
    ] {
        let bytes = fs::read(root.path().join(relative)).expect("generated projection");
        frozen_bytes(
            &format!("projection.{}", relative.replace('/', "-")),
            &bytes,
            expected_len,
            expected_digest,
        );
    }
}

/// Assembly fixes both its report and the generated publication.
///
/// ´claim:harness:assembly-fixes-report-and-publication-bytes´
/// ´test:integration:characterizes-assembly-bytes´
#[test]
fn characterizes_assembly_bytes() {
    let root = tempfile::tempdir().expect("temporary directory");
    spec_fixture(root.path());
    track(root.path());

    let output = observable(root.path(), &["assemble", "--write"]);
    frozen_observable(
        "assembly",
        &output,
        0,
        285,
        "4661633013c3c8bed4ca7bbc5f3c15a428effa17e89f76d0f07544b0e21ed95e",
    );
    let publication = fs::read(root.path().join(SPEC_TARGET)).expect("assembled publication");
    frozen_bytes(
        "assembly.publication",
        &publication,
        336,
        "ac2689cc8e8b21adeb6e4cf5d422012fb69e6ad6fba4994c87114fdd5af52e26",
    );
}

/// Both fix profiles refuse before writing over deliberately undeclared
/// fixtures, freezing the refusal and the untouched sources.
///
/// ´claim:harness:fix-profiles-freeze-report-and-source-bytes´
/// ´test:integration:characterizes-fix-bytes´
#[test]
fn characterizes_fix_bytes() {
    let tests = tempfile::tempdir().expect("temporary directory");
    fixture_workspace(tests.path());
    track(tests.path());
    let test_output = observable(
        tests.path(),
        &["fix", "--profile", "test", "--write", "--allow-dirty"],
    );
    frozen_observable(
        "fix-test",
        &test_output,
        1,
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );

    for (relative, expected_len, expected_digest) in [
        (
            "src/lib.rs",
            82,
            "c2409ca5eca55ce77741376390328424d5f17546e2c2ab419f496dd38132948e",
        ),
        (
            "src/tests/mod.rs",
            68,
            "c42c104d924cf994a51d33764ddd86ff30881b50874ebf009cf097bc6689551f",
        ),
    ] {
        let bytes = fs::read(tests.path().join(relative)).expect("test profile output");
        frozen_bytes(
            &format!("fix-test.{}", relative.replace('/', "-")),
            &bytes,
            expected_len,
            expected_digest,
        );
    }

    let todos = tempfile::tempdir().expect("temporary directory");
    notice_workspace(todos.path());
    track(todos.path());
    let todo_output = observable(
        todos.path(),
        &["fix", "--profile", "todo", "--write", "--allow-dirty"],
    );
    frozen_observable(
        "fix-todo",
        &todo_output,
        1,
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );

    for (relative, expected_len, expected_digest) in [
        (
            "src/lib.rs",
            169,
            "7ea1704b5a2a094e27f646d562400ff652e17f03b85c091e8968188712cebfe8",
        ),
        (
            "src/tests/mod.rs",
            60,
            "abb3e4adc0ddf74d995de2335c31f840e0460037a9f5a79226d24f32955f603a",
        ),
    ] {
        let bytes = fs::read(todos.path().join(relative)).expect("todo profile output");
        frozen_bytes(
            &format!("fix-todo.{}", relative.replace('/', "-")),
            &bytes,
            expected_len,
            expected_digest,
        );
    }
}
