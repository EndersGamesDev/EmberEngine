// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Burn-family census and declaration-driven ratchet tests.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`counts_a_retired_identity_wrapped_across_two_comment_lines`] | burn | A retired identity the corpus wrapped across two comment lines is one occurrence, because the commentary is read through the tokenization layer that joins adjacent comment lines into one region. A census reading one comment at a time could not see the corpus's own wrapped spelling, and would have closed its register with that debt still standing. |
//! | [`counts_a_family_over_prose_and_over_comments`] | burn | A legacy reference family is censused across both surfaces at once — the prose of the documentation tree and the commentary of the sources — and the tally is kept per file, because a debt is owed by a document rather than by the corpus in aggregate. |
//! | [`counts_nothing_a_string_literal_carries`] | burn | A form a program carries as data is not a reference the program makes: a legacy shape inside a string literal, raw or plain, contributes nothing to the debt. |
//! | [`counts_nothing_displayed_prose_carries`] | burn | The same boundary holds in prose: a form shown in code font or inside a fenced block is displayed rather than referred to, and only the form actually written into a sentence is counted as debt. |
//! | [`never_reads_an_excluded_tree`] | burn | Declared exclusions are never read at all: the linter's own fixtures and the burn registers themselves are not the corpus's debt, and counting them would make a register that grows every time it records a shrinkage. They are the corpus's statement rather than this binary's, so a list given none reads every tree its reaches name. |
//! | [`counts_only_the_notices_that_carry_no_label`] | burn | The family of unlabelled notices is exactly the notices carrying no label: labelling one takes it out of the family, and a notice written inside a string literal was never in it. The occurrence carries the notice's own words, so a reviewer sees what is owed. |
//! | [`counts_only_the_legacy_sites_that_carry_no_label`] | burn | A marked production site enters the legacy-implementation remainder only while it carries no derived label. Test-tree markers and strings stay outside the inventory read by the family. |
//! | [`every_family_walks_exactly_the_trees_its_compiled_table_named`] | burn | Every fictional family resolves the prose, code, and exclusion trees its own policy document declares. The equivalence is asked over the resolved places rather than over the patterns. Both exclusions are derived here rather than written anywhere, and the two rules are visible in the two answers. Every family cuts out the checker's own share, which is self-exemption reading the declared partition; the two repository-scoped families cut out the Quarry's share beside it, which is sibling-activation exclusion reading the activation document. No document carries an exclusion row at all, and the burn-register rows that used to stand in them cut nothing when they were retired because the directories they named had already been deleted. |

use std::fs;
use std::path::{Path, PathBuf};

use crate::burn::{Recognizer, family_spans_corpus};
use crate::legacy::LegacyRule;
use crate::plan::CorpusPlan;
use crate::program::LiteralSet;
use crate::retired::RetiredFamily;
use crate::snapshot::{Configuration, DIRECTORY, configuration};
use crate::universe::UniverseKind;
use crate::{BurnList, Shape, census, index_burn_lists};

fn topology(root: &Path) -> CorpusPlan {
    CorpusPlan::compile(root, UniverseKind::AsWritten, &[]).expect("fixture topology")
}

/// The one division name these fixtures are written around.
///
/// A census reads for what a document declares, so a list built outside a
/// snapshot is handed the payload directly. One sentence is enough here: what
/// these tests hold is where the census looks and how it tallies, not which
/// sentences the retiring matrix happened to name.
fn divisions() -> Recognizer {
    Recognizer::new(
        [],
        Some(LiteralSet::new(["The fast path stays fast"]).expect("one nonempty unbroken value")),
        [],
    )
}

fn list(prose: &[&str], code: &[&str]) -> BurnList {
    BurnList::new(
        "section references",
        Shape::Legacy(&[LegacyRule::SectionNumber]),
        prose.iter().map(PathBuf::from).collect(),
        code.iter().map(PathBuf::from).collect(),
    )
}

fn retired_list(code: &[&str]) -> BurnList {
    BurnList::new(
        "retired division names",
        Shape::Retired(RetiredFamily::DivisionName),
        Vec::new(),
        code.iter().map(PathBuf::from).collect(),
    )
    .reading(divisions())
}

fn todo_list(code: &[&str]) -> BurnList {
    BurnList::new(
        "unlabelled to-do notices",
        Shape::UnlabelledTodo,
        Vec::new(),
        code.iter().map(PathBuf::from).collect(),
    )
}

fn legacy_implementation_list(code: &[&str]) -> BurnList {
    BurnList::new(
        "unlabelled legacy implementations",
        Shape::UnlabelledLegacy,
        Vec::new(),
        code.iter().map(PathBuf::from).collect(),
    )
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("create");
    fs::write(path, text).expect("write");
}

/// A retired identity the corpus wrapped across two comment lines is one
/// occurrence, because the commentary is read through the tokenization layer
/// that joins adjacent comment lines into one region. A census reading one
/// comment at a time could not see the corpus's own wrapped spelling, and
/// would have closed its register with that debt still standing.
///
/// ´claim:burn:a-retired-identity-wrapped-across-comment-lines-is-one-occurrence´
/// ´test:crate:counts-a-retired-identity-wrapped-across-two-comment-lines´
#[test]
fn counts_a_retired_identity_wrapped_across_two_comment_lines() {
    let root = tempfile::tempdir().expect("temporary directory");

    // The identity stands whole on one line, wrapped over two adjacent
    // lines, and split over a boundary that is not commentary at all.
    write(
        root.path(),
        "src/whole.rs",
        "// The fast path stays fast, we hope.
fn f() {}
",
    );
    write(
        root.path(),
        "src/wrapped.rs",
        "// The fast path
// stays fast.
fn f() {}
",
    );
    write(
        root.path(),
        "src/broken.rs",
        "// The fast path
fn f() {}
// stays fast.
",
    );

    let (taken, findings) = census(root.path(), &retired_list(&["src"]), &topology(root.path()));

    assert_eq!(findings, Vec::new(), "a readable tree raises nothing");
    assert_eq!(
        taken
            .rows()
            .iter()
            .map(|row| (row.path.as_str(), row.count))
            .collect::<Vec<_>>(),
        [("src/whole.rs", 1), ("src/wrapped.rs", 1)]
    );

    // The third file is the boundary the join deliberately keeps: a run of
    // commentary ends at a line of program text, so an identity spanning
    // one is not an identity the corpus wrote.
    assert_eq!(taken.total(), 2);
}

/// A legacy reference family is censused across both surfaces at once — the
/// prose of the documentation tree and the commentary of the sources — and
/// the tally is kept per file, because a debt is owed by a document rather
/// than by the corpus in aggregate.
///
/// ´claim:burn:a-family-is-censused-over-prose-and-commentary-alike-per-file´
/// ´test:crate:counts-a-family-over-prose-and-over-comments´
#[test]
fn counts_a_family_over_prose_and_over_comments() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "docs/note.md",
        "As §10.3 requires, and §6.1 too.\n",
    );
    write(
        root.path(),
        "src/lib.rs",
        "//! Per §4.2.\nfn f() {} // and §7\n",
    );

    let (taken, findings) = census(
        root.path(),
        &list(&["docs"], &["src"]),
        &topology(root.path()),
    );

    assert_eq!(findings, Vec::new(), "a readable tree raises nothing");
    assert_eq!(taken.total(), 4);
    assert_eq!(taken.files_scanned(), 2);
    assert_eq!(
        taken
            .rows()
            .iter()
            .map(|row| (row.path.as_str(), row.count))
            .collect::<Vec<_>>(),
        [("docs/note.md", 2), ("src/lib.rs", 2)]
    );
}

/// A form a program carries as data is not a reference the program makes: a
/// legacy shape inside a string literal, raw or plain, contributes nothing
/// to the debt.
///
/// ´claim:burn:a-form-inside-a-string-literal-is-data-and-not-a-reference´
/// ´test:crate:counts-nothing-a-string-literal-carries´
#[test]
fn counts_nothing_a_string_literal_carries() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "src/lib.rs",
        "fn f() {\n    let message = \"see §10.3 for this\";\n    let raw = r#\"and §6.1\"#;\n}\n",
    );

    let (taken, _findings) = census(root.path(), &list(&[], &["src"]), &topology(root.path()));

    assert_eq!(
        taken.total(),
        0,
        "a form a program carries is data, not a reference"
    );
}

/// The same boundary holds in prose: a form shown in code font or inside a
/// fenced block is displayed rather than referred to, and only the form
/// actually written into a sentence is counted as debt.
///
/// ´claim:burn:a-form-displayed-in-prose-is-not-counted-as-debt´
/// ´test:crate:counts-nothing-displayed-prose-carries´
#[test]
fn counts_nothing_displayed_prose_carries() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "docs/note.md",
        "The lint covers `§10.3` and kin.\n\n```text\n§6.1 shown\n```\n\nBut §4.2 is written.\n",
    );

    let (taken, _findings) = census(root.path(), &list(&["docs"], &[]), &topology(root.path()));

    assert_eq!(
        taken.total(),
        1,
        "code font and fenced blocks display rather than reference"
    );
}

/// Declared exclusions are never read at all: the linter's own fixtures and
/// the burn registers themselves are not the corpus's debt, and counting
/// them would make a register that grows every time it records a shrinkage.
/// They are the corpus's statement rather than this binary's, so a list
/// given none reads every tree its reaches name.
///
/// ´claim:burn:declared-exclusions-are-never-censused´
/// ´test:crate:never-reads-an-excluded-tree´
#[test]
fn never_reads_an_excluded_tree() {
    let root = tempfile::tempdir().expect("temporary directory");
    write(
        root.path(),
        "packages/linter/docs/reject.md",
        "A §10.3 fixture.\n",
    );
    write(
        root.path(),
        "packages/assayer/docs/plans/burn/self.md",
        "A §6.1 row.\n",
    );
    write(
        root.path(),
        "packages/assayer/docs/kept.md",
        "A §4.2 reference.\n",
    );

    let excluded = ["packages/linter", "packages/assayer/docs/plans/burn"];
    let list = list(&["packages"], &[]).with_excluded(excluded.iter().map(PathBuf::from));
    let (taken, _findings) = census(root.path(), &list, &topology(root.path()));

    assert_eq!(
        taken.total(),
        1,
        "the tool's own fixtures are not the corpus's debt"
    );
    assert_eq!(taken.rows()[0].path, "packages/assayer/docs/kept.md");

    // The exclusions are the corpus's statement rather than this binary's,
    // so a list given none reads every tree its reaches name.
    let (unbounded, _findings) = census(
        root.path(),
        &list.with_excluded(Vec::new()),
        &topology(root.path()),
    );

    assert_eq!(
        unbounded.total(),
        3,
        "a list excluding nothing reads all three"
    );
}

/// The family of unlabelled notices is exactly the notices carrying no
/// label: labelling one takes it out of the family, and a notice written
/// inside a string literal was never in it. The occurrence carries the
/// notice's own words, so a reviewer sees what is owed.
///
/// ´claim:burn:labelling-a-notice-removes-it-from-the-unlabelled-family´
/// ´test:crate:counts-only-the-notices-that-carry-no-label´
#[test]
fn counts_only_the_notices_that_carry_no_label() {
    let root = tempfile::tempdir().expect("temporary directory");
    let acute = '\u{b4}';

    write(
        root.path(),
        "src/lib.rs",
        &format!(
            "// TODO: read the flag\n\
                 // TODO {acute}todo:code:already-labelled{acute}: already labelled\n\
                 let quiet = \"// TODO: a literal carries no notice\";\n"
        ),
    );

    let (taken, findings) = census(root.path(), &todo_list(&["src"]), &topology(root.path()));

    assert_eq!(findings, Vec::new());
    assert_eq!(taken.total(), 1, "a labelled notice has left the family");
    assert_eq!(taken.rows()[0].path, "src/lib.rs");
    assert!(
        taken.occurrences()[0]
            .text()
            .starts_with("TODO: read the flag"),
        "the occurrence names the notice: {:?}",
        taken.occurrences()[0].text()
    );
}

/// A marked production site enters the legacy-implementation remainder only
/// while it carries no derived label. Test-tree markers and strings stay
/// outside the inventory read by the family.
///
/// ´claim:burn:labelling-a-legacy-site-removes-it-from-the-unlabelled-family´
/// ´test:crate:counts-only-the-legacy-sites-that-carry-no-label´
#[test]
fn counts_only_the_legacy_sites_that_carry_no_label() {
    let root = tempfile::tempdir().expect("temporary directory");
    let acute = '\u{b4}';

    write(
        root.path(),
        "packages/demo/src/lib.rs",
        &format!(
            "// LEGACY: read the old result\n\
             // LEGACY {acute}legacy:code:already-labelled{acute}: already labelled\n\
             let quiet = \"// LEGACY: a literal carries no marker\";\n"
        ),
    );
    write(
        root.path(),
        "packages/demo/src/tests/fixtures.rs",
        "// LEGACY: test vocabulary is not shipped\n",
    );

    let list = legacy_implementation_list(&["packages/demo/src"]);
    let (taken, findings) = census(root.path(), &list, &topology(root.path()));

    assert_eq!(findings, Vec::new());
    assert_eq!(taken.total(), 1, "a labelled marker has left the family");
    assert_eq!(taken.rows()[0].path, "packages/demo/src/lib.rs");
    assert_eq!(taken.occurrences()[0].text(), "LEGACY: read the old result");
}

/// Fictional surface documents carried by this package's fixtures.
const SURFACE_DOCUMENTS: [(&str, &str); 11] = [
    (
        "policy-legacy-section-references.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-section-references.toml"),
    ),
    (
        "policy-legacy-record-references.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-record-references.toml"),
    ),
    (
        "policy-legacy-unprefixed-record-references.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-unprefixed-record-references.toml"),
    ),
    (
        "policy-legacy-tag-references.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-tag-references.toml"),
    ),
    (
        "policy-legacy-todos.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-todos.toml"),
    ),
    (
        "policy-legacy-implementation.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-implementation.toml"),
    ),
    (
        "policy-legacy-section-references-repository.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-section-references-repository.toml"),
    ),
    (
        "policy-legacy-record-references-repository.toml",
        include_str!("../../tests/fixtures/burn/policy-legacy-record-references-repository.toml"),
    ),
    (
        "policy-references-scenarios.toml",
        include_str!("../../tests/fixtures/burn/policy-references-scenarios.toml"),
    ),
    (
        "policy-references-divisions.toml",
        include_str!("../../tests/fixtures/burn/policy-references-divisions.toml"),
    ),
    (
        "policy-references-prefix-numbers.toml",
        include_str!("../../tests/fixtures/burn/policy-references-prefix-numbers.toml"),
    ),
];

/// The activations the fictional corpus declares for its censused families.
///
/// The pairs matter to the equivalence rather than decorating it: a
/// repository-scoped family cuts out the shares whose owners activate its
/// per-owner sibling, and that is read from these rows at runtime.
const ACTIVATIONS: &str = "namespace = \"com.torrust.index.linter.policies\"\n\
         version = [1, 0, 0]\n\
         \n\
         policies = [\
         { owner = \"QUARRY\", policy = \"labels.mints-well-formed\" }, \
         { owner = \"INDEX\", policy = \"labels.mints-well-formed\" }, \
         { owner = \"QUARRY\", policy = \"legacy.section-references\" }, \
         { owner = \"QUARRY\", policy = \"legacy.record-references\" }, \
         { owner = \"QUARRY\", policy = \"legacy.unprefixed-record-references\" }, \
         { owner = \"QUARRY\", policy = \"legacy.tag-references\" }, \
         { owner = \"QUARRY\", policy = \"legacy.scenario-numbers\" }, \
         { owner = \"QUARRY\", policy = \"legacy.division-names\" }, \
         { owner = \"QUARRY\", policy = \"legacy.residual-litter\" }, \
         { owner = \"QUARRY\", policy = \"legacy.implementation\" }, \
         { owner = \"INDEX\", policy = \"legacy.todos\" }, \
         { owner = \"INDEX\", policy = \"legacy.section-references-repository\" }, \
         { owner = \"INDEX\", policy = \"legacy.record-references-repository\" }]\n";

/// The empty ratchet every activated pair carries, because an activation wants one.
///
/// The equivalence here is over where a census walks rather than over what it
/// found, so every ceiling is empty: the fixture holds no source at all, and a
/// pair activating a census still has to carry the list its verdict is read
/// against.
const RATCHETS: &str = "namespace = \"com.torrust.index.linter.lists\"\n\
         version = [1, 0, 0]\n\
         \n\
         [QUARRY.\"labels.mints-well-formed\"]\nallowances = []\n\
         [INDEX.\"labels.mints-well-formed\"]\nallowances = []\n\
         [QUARRY.\"legacy.section-references\"]\npath_counts = []\n\
         [QUARRY.\"legacy.record-references\"]\npath_counts = []\n\
         [QUARRY.\"legacy.unprefixed-record-references\"]\npath_counts = []\n\
         [QUARRY.\"legacy.tag-references\"]\npath_counts = []\n\
         [QUARRY.\"legacy.scenario-numbers\"]\npath_counts = []\n\
         [QUARRY.\"legacy.division-names\"]\npath_counts = []\n\
         [QUARRY.\"legacy.residual-litter\"]\npath_counts = []\n\
         [QUARRY.\"legacy.implementation\"]\npath_counts = []\n\
         [INDEX.\"legacy.todos\"]\npath_counts = []\n\
         [INDEX.\"legacy.section-references-repository\"]\npath_counts = []\n\
         [INDEX.\"legacy.record-references-repository\"]\npath_counts = []\n";

/// Write the fictional declaration and its owner geography.
fn declared_geography(root: &Path) {
    let core = [
        (
            "owners.toml",
            "namespace = \"com.torrust.index.linter.owners\"\n\
                 version = [1, 0, 0]\n\
                 \n\
                 owners = [\"INDEX\", \"QUARRY\", \"LINTER\"]\n\
                 partitions = [{ name = \"quarry-package\", owner = \"QUARRY\", pattern = '%s\"packages/quarry\" [ \"/\" *VCHAR ]' }, \
                 { name = \"linter-tool\", owner = \"LINTER\", pattern = '%s\"tools/linter\" [ \"/\" *VCHAR ]' }, \
                 { name = \"documentation\", owner = \"INDEX\", pattern = '%s\"docs\" [ \"/\" *VCHAR ]' }, \
                 { name = \"crate-sources\", owner = \"INDEX\", pattern = '%s\"src\" [ \"/\" *VCHAR ]' }]\n\
                 may_cite = []\n",
        ),
        (
            "environments.toml",
            "namespace = \"com.torrust.index.linter.environments\"\n\
                 version = [1, 0, 0]\n\
                 \n\
                 environments = []\n",
        ),
        ("policies.toml", ACTIVATIONS),
        ("lists.toml", RATCHETS),
        (
            "shape.toml",
            "namespace = \"com.torrust.index.linter.shape\"\n\
                 version = [1, 0, 0]\n\
                 \n\
                 universe = \"git-tracked\"\n\
                 \n\
                 ignore = []\n",
        ),
    ];

    for (file, text) in core.into_iter().chain(SURFACE_DOCUMENTS) {
        write(root, &format!("{DIRECTORY}/{file}"), text);
    }
}

/// The trees one family is counted over, as the report renders them.
fn surfaces(lists: &[BurnList], family: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let list = lists
        .iter()
        .find(|list| list.family() == family)
        .expect("the family is adopted");
    let rendered = |paths: &[PathBuf]| {
        paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    };

    (
        rendered(list.prose()),
        rendered(list.code()),
        rendered(list.excluded()),
    )
}

/// A run of declared paths, as the report renders them.
fn named(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_owned()).collect()
}

/// Every fictional family resolves the prose, code, and exclusion trees its
/// own policy document declares. The equivalence is asked over the resolved
/// places rather than over the patterns.
///
/// Both exclusions are derived here rather than written anywhere, and the two
/// rules are visible in the two answers. Every family cuts out the checker's
/// own share, which is self-exemption reading the declared partition; the two
/// repository-scoped families cut out the Quarry's share beside it, which is
/// sibling-activation exclusion reading the activation document. No document
/// carries an exclusion row at all, and the burn-register rows that used to
/// stand in them cut nothing when they were retired because the directories
/// they named had already been deleted.
///
/// ´claim:burn:every-family-walks-the-trees-its-compiled-table-named´
/// ´test:crate:every-family-walks-exactly-the-trees-its-compiled-table-named´
#[test]
fn every_family_walks_exactly_the_trees_its_compiled_table_named() {
    let root = tempfile::tempdir().expect("temporary directory");
    declared_geography(root.path());

    let loaded = configuration(root.path());

    let Configuration::Present(snapshot) = loaded else {
        panic!("the declared geography loads as a snapshot: {loaded:?}")
    };

    let lists = index_burn_lists(&snapshot);

    assert_eq!(
        lists.len(),
        SURFACE_DOCUMENTS.len(),
        "every adopted family resolves its reaches"
    );

    let prose = ["packages/quarry/docs", "packages/quarry/notes"];
    let wide = [
        "packages/quarry/docs",
        "packages/quarry/notes",
        "packages/quarry/src",
        "packages/quarry/tests",
    ];
    let code = [
        "packages/quarry/src",
        "packages/quarry/tests",
        "packages/quarry/benches",
        "packages/quarry/examples",
    ];
    let scripted = [
        "packages/quarry/src",
        "packages/quarry/tests",
        "packages/quarry/benches",
        "packages/quarry/examples",
        "packages/quarry/ci",
    ];
    // Self-exemption, derived: the checker's own share, wherever the
    // partition puts it, and no row in any document says so.
    let own = ["tools/linter"];

    for family in [
        "section-sign references",
        "retired record numbers",
        "ambiguous unprefixed record numbers",
    ] {
        assert_eq!(
            surfaces(&lists, family),
            (named(&prose), named(&code), named(&own)),
            "{family}"
        );
    }

    assert_eq!(
        surfaces(&lists, "superseded tag forms"),
        (named(&prose), Vec::new(), named(&own)),
        "a family the scheme never reached in code declares no code reach"
    );

    for family in ["retired scenario numbers", "retired division names"] {
        assert_eq!(
            surfaces(&lists, family),
            (named(&wide), named(&code), named(&own)),
            "{family}"
        );
    }

    assert_eq!(
        surfaces(&lists, "residual litter"),
        (named(&prose), named(&scripted), named(&own)),
        "the residual family reads the script tree beside the sources"
    );

    // The to-do family is counted over the repository rather than over one
    // member, so its surface is the root owner's. It has no per-owner
    // sibling, so the only share it cuts out is the checker's own.
    assert_eq!(
        surfaces(&lists, "unlabelled to-do notices"),
        (
            Vec::new(),
            named(&["src", "tests", "packages"]),
            named(&own)
        )
    );

    assert_eq!(
        surfaces(&lists, "unlabelled legacy implementations"),
        (Vec::new(), named(&["packages/quarry/src"]), named(&own))
    );

    let corpus = ["."];
    // Sibling activation first, then self-exemption: the Quarry holds the
    // per-owner half of both programs, so the repository-wide half does not
    // count its share a second time.
    let carved = ["packages/quarry", "tools/linter"];

    for family in [
        "section-sign references (repository)",
        "retired record numbers (repository)",
    ] {
        assert_eq!(
            surfaces(&lists, family),
            (named(&corpus), named(&corpus), named(&carved)),
            "{family}"
        );
        assert!(
            family_spans_corpus(family),
            "a family counted over the root owner's whole share is one repository-wide artifact"
        );
    }

    assert!(
        !family_spans_corpus("unlabelled to-do notices"),
        "and a family counted over named trees of it is not, however wide those trees reach"
    );
    assert!(!family_spans_corpus("unlabelled legacy implementations"));
}
