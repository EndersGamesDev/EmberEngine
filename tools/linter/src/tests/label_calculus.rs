// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Whole-calculus resolution tests across prose, code, and derived mints.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`resolves_a_mint_and_its_citation`] | check | The ordinary case is clean: a head mints, a sentence below cites it, the citation resolves, and the pair becomes an edge of the reference graph. A document may refer to what it has just established. |
//! | [`resolves_across_files_in_either_traversal_order`] | check | The order the corpus is read in decides nothing: a citation in one file of a mint in another resolves whichever file is visited first, and both orders yield the same counts. Resolution is staged rather than sequential, so no document need be written before another. |
//! | [`duplicate_mints_fail_with_both_locations`] | check | A statement is minted once: minting it in two documents fails with both locations named, and only one mint is counted. An author is shown the pair to reconcile rather than told that a name is unavailable. |
//! | [`unresolved_citations_fail`] | check | Resolution is total: a citation reaching no mint fails the run, so the corpus cannot refer to a statement nobody made. |
//! | [`unshaped_spans_never_fail`] | check | Text is never a failure: parenthesised spans holding a command, a file name, or a colon-separated pair that is not a label leave the corpus clean and mint nothing. Adopting the calculus does not make ordinary documentation risky to write. |
//! | [`reserved_kinds_fail_as_unwarranted`] | check | A reserved kind may not be minted by hand: writing one at a head fails as unwarranted and mints nothing, because a kind set aside for derivation is filled by the derivation and by nothing else. |
//! | [`refuses_a_reserved_kind_written_at_a_source_title`] | check | A source's title is no exception to that rule and no exception is written for it: a reserved token at a Title head is refused by the same bare occurrence rule, because a kind set aside for derivation is set aside wherever it is written, and titling a document neither widens nor narrows what may be minted at its head. |
//! | [`imported_citations_fail_under_an_empty_signature`] | check | An imported citation naming a prefix the signature does not register fails: importing is reaching into a named corpus, and a corpus nobody registered cannot be reached into. |
//! | [`self_qualified_imports_fail`] | check | Importing from one's own owner fails even when the prefix is registered and the label resolves: a citation within an owner is written in the plain form, and qualifying it with one's own name says something untrue about where the statement lives. |
//! | [`edges_run_from_the_containing_environment`] | check | An edge runs from the environment a citation stands inside to the environment it names, not from the document or from the citation itself, so the graph records which statement depends on which. |
//! | [`resolves_a_prose_citation_of_a_derived_test_label`] | check | Prose may cite a test: the labels derived from the census are seeded into the registries, so a document naming a test as its witness resolves cleanly and edges to it. Documentation can point at the code that establishes what it says. |
//! | [`fails_a_citation_of_a_test_that_is_not_in_the_census`] | check | Citing a test holds the prose to the corpus as it actually is: a citation of a test that is not in the census fails, naming the label, so renaming or deleting a test breaks the documentation that witnessed it instead of leaving a quiet lie behind. |
//! | [`refuses_a_hand_authored_test_mint_although_the_label_is_seeded`] | check | Seeding a derived label does not make it authorable: writing one at a head is refused as an unwarranted reserved kind rather than reported as a duplicate of the seeded mint, because the kind is checked before the registry is consulted. Nothing is authored into that space. |
//! | [`reaches_a_derived_label_of_another_owner_through_its_prefix`] | check | A derived label of another owner is reachable through the ordinary import form: the repository's prose can cite a package's test by qualifying it with that package's prefix, and no new mechanism is needed for it. |
//! | [`counts_derived_mints_apart_from_the_carriers`] | check | Derived mints are counted apart from authored ones while standing in the same graph, so an inventory that outnumbers the prose cannot inflate the figure describing how much the corpus was written. |
//! | [`seeds_the_registries_in_either_traversal_order`] | check | cites (´claim:check:the-traversal-order-decides-nothing´) |
//! | [`resolves_a_prose_citation_of_a_notice`] | check | A document may cite a notice: the labels the todo census derives are seeded like any others, so prose naming a deficiency resolves cleanly and edges to the code the deficiency stands in. The profile's promise that a document can cite a notice is kept by the resolver rather than by care. |
//! | [`resolves_a_comment_citation_of_a_notice`] | check | A comment may cite a notice on the same terms: the citation stands on the other surface and reaches the same mint, because there is one resolution space and a citation's surface decides nothing about what it may reach. |
//! | [`fails_a_citation_of_a_notice_no_source_carries`] | check | Citing a notice holds the citation to the corpus as it actually is: a citation of a notice no source carries fails, naming the label, so resolving a deficiency breaks the sentence that pointed at it instead of leaving a quiet lie behind. This is the whole point of citing rather than displaying. |
//! | [`refuses_a_hand_authored_notice_mint_and_mints_nothing_by_citing`] | check | Seeding a notice's label does not make it authorable, and citing one never mints: a document writing the label at a head is refused as an unwarranted reserved kind, while the same document's citation of the same label resolves and mints nothing. The notice's identity stays where the notice is. |
//! | [`leaves_a_displayed_notice_label_inert`] | check | Displaying a label is not citing it, and stays inert beside a citation that is: a document showing one notice label in a non-participating span while citing another resolves the citation, mints nothing extra, and does not fail on the display — even where the displayed notice exists nowhere. A record may quote a name it is not making a claim about. |
//! | [`resolves_a_prose_citation_of_a_claim_minted_in_code`] | check | A claim minted where its test is may be cited from prose: the ruling is about the space rather than about a kind, so an authored mint standing in code is reachable exactly as a derived one is. A document may name the statement a test establishes. |
//! | [`resolves_a_comment_citation_of_a_derived_test_label`] | check | A comment may cite a derived test label, which closes the last of the four pairings: both kinds of mint reachable from both surfaces. Code may point at the test that establishes what it says. |
//! | [`fails_a_comment_citation_of_a_claim_nobody_minted`] | check | Resolution is total on the code surface too: a comment citing a statement nobody minted fails, so commentary cannot refer a claim into existence any more than prose can. |
//! | [`resolves_a_generated_citation_without_minting`] | check | A generated citation resolves like any other, against the registries the generator emitted from: a matrix row citing a censused test edges to it, mints nothing, and feeds nothing it presents — the document's mint count stays the head's alone, and the census the row was generated from is untouched by the row resolving against it. |
//! | [`fails_a_dangling_citation_in_a_generated_region`] | check | A generated citation that resolves nowhere is never the author's slip: the register is stale after a transition or its generator wrote a citation of nothing, so it fails under its own code, beside the exactness check, rather than as an unresolved citation wanting an edit inside a region no hand may write. |
//! | [`refuses_a_bare_span_in_a_generated_region`] | check | A bare span in a generated region mints nothing and fails as the generator's: a generated occurrence is a mint only where a profile sets its standard place in the register, and no adopted profile does, so nothing enters the registry and no-self-support stays a theorem — a register row cannot sustain its own membership however it is written. |
//! | [`leaves_a_worked_example_outside_a_readme_authored`] | check | The same head-and-region shape standing outside a folder readme is authored prose about the projection, not the projection: a worked example in a record participates as authored occurrences do, and a citation dangling there is the author's ordinary unresolved citation. |
//! | [`resolves_a_same_owner_citation_of_a_document_mint`] | check | A document mint is a mint like any other once its head is recognised: a title minted at a source's first level-one heading enters its owner's registry, a sentence elsewhere in that owner cites it in the plain form, and the citation resolves into an edge. The Title class changes no resolution judgment. |
//! | [`resolves_an_imported_citation_of_a_document_mint`] | check | Another owner reaches a document mint by the ordinary imported form, through the prefix its corpus is registered under. Uniqueness stays the pair of owner and complete label, so a title is named across owners the way every other environment is. |

use std::collections::BTreeMap;
use std::path::Path;

use crate::adoption::{Adoption, Owner, index_adoption};
use crate::analyze;
use crate::carrier::Source;
use crate::census::{Census, scan_source};
use crate::claim::{CoveredClaim, analyze_claims};
use crate::code::{CodeSurface, scan_code_citations};
use crate::finding::Finding;
use crate::label::Prefix;
use crate::profile::{CoveredAsset, cover};
use crate::registry::{KindRegistry, fixture_kind_registry};
use crate::todo::{CoveredNotice, TodoCensus, cover_todos, scan_todos};
use crate::workspace::Package;

/// The acute the code syntax delimits an occurrence with.
const ACUTE: char = '\u{b4}';

/// The environment rows a fixture corpus declares beside the adopted edition.
///
/// The extension set is a corpus's own declaration rather than a compiled
/// copy, so a fixture wanting the three rows this module's heads pair with
/// declares them (´gram:isolation:declaration´).
const DECLARED: [(&str, &str); 3] = [
    ("Document", "doc"),
    ("To-do", "todo"),
    ("Constant", "const"),
];

/// Adoption data for a fixture corpus, extended by the rows it declares.
fn adoption(packages: &[Package]) -> Adoption {
    index_adoption(
        packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
        fixture_kind_registry().with_declared(DECLARED),
    )
}

/// The effective relation a fixture corpus validates its heads against.
fn extended() -> KindRegistry {
    fixture_kind_registry().with_declared(DECLARED)
}

fn source(path: &str, text: &str) -> Source {
    Source::new(path, text)
}

/// The covered notices of one Rust source, taken the way the check takes them.
fn notices(path: &str, text: &str) -> Vec<CoveredNotice> {
    let packages = [Package::new("torrust-demo", "packages/demo")];
    let (found, orphans) = scan_todos("torrust-demo", Path::new(path), text);

    assert!(
        orphans.is_empty(),
        "the fixture carries no orphan label: {orphans:?}"
    );

    let (covered, findings) = cover_todos(&packages, &TodoCensus::from_notices(found, 1));

    assert!(
        findings.is_empty(),
        "the fixture covers cleanly: {findings:?}"
    );

    covered
}

/// The covered claims of an inventory, taken the way the check takes them.
fn claims(assets: &[CoveredAsset]) -> Vec<CoveredClaim> {
    let (_analysis, covered, findings) = analyze_claims(assets, &[]);

    assert!(
        findings.is_empty(),
        "the fixture claims cleanly: {findings:?}"
    );

    covered
}

/// One package's covered assets, taken the way the check takes them.
fn assets(package: &str, directory: &str, path: &str, text: &str) -> Vec<CoveredAsset> {
    let packages = [Package::new(package, directory)];
    let tests = scan_source(package, Path::new(path), text).expect("a Rust source");
    let (covered, findings) = cover(&packages, &Census::from_tests(tests, 1));

    assert!(
        findings.is_empty(),
        "the fixture covers cleanly: {findings:?}"
    );

    covered
}

/// The demonstration package's one covered test, and the adoption knowing it.
fn demo() -> (Adoption, Vec<CoveredAsset>) {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = assets(
        "torrust-demo",
        "packages/demo",
        "packages/demo/src/engine.rs",
        "/// Decodes a header.\n/// \u{b4}test:unit:decodes-a-header\u{b4}\n#[test]\nfn decodes_a_header() {}\n",
    );

    (adoption, covered)
}

/// The same package, its one test also carrying the claim it establishes.
fn demo_with_claim() -> (Adoption, Vec<CoveredAsset>) {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = assets(
        "torrust-demo",
        "packages/demo",
        "packages/demo/src/engine.rs",
        "/// Decodes a header.\n\
             ///\n\
             /// \u{b4}claim:demo:a-header-decodes\u{b4}\n\
             /// \u{b4}test:unit:decodes-a-header\u{b4}\n\
             #[test]\nfn decodes_a_header() {}\n",
    );

    (adoption, covered)
}

/// The ordinary case is clean: a head mints, a sentence below cites it, the
/// citation resolves, and the pair becomes an edge of the reference graph.
/// A document may refer to what it has just established.
///
/// ´claim:check:a-mint-and-a-citation-of-it-resolve-into-an-edge´
/// ´test:crate:resolves-a-mint-and-its-citation´
#[test]
fn resolves_a_mint_and_its_citation() {
    let sources = [source(
        "one.md",
        "## Head · `sec:labels:syntax`\n\nBody cites (`sec:labels:syntax`).\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.mints(), 1);
    assert_eq!(analysis.citations_resolved(), 1);
    assert_eq!(analysis.graph().edge_count(), 1);
}

/// The order the corpus is read in decides nothing: a citation in one file
/// of a mint in another resolves whichever file is visited first, and both
/// orders yield the same counts. Resolution is staged rather than
/// sequential, so no document need be written before another.
///
/// ´claim:check:the-traversal-order-decides-nothing´
/// ´test:crate:resolves-across-files-in-either-traversal-order´
#[test]
fn resolves_across_files_in_either_traversal_order() {
    let minting = source("mint.md", "## Head · `sec:labels:syntax`\n");
    let citing = source(
        "cite.md",
        "## Other · `sec:labels:other`\n\nCites (`sec:labels:syntax`).\n",
    );

    let forward = analyze(
        &adoption(&[]),
        &[minting.clone(), citing.clone()],
        &CodeSurface::default(),
    );
    let backward = analyze(&adoption(&[]), &[citing, minting], &CodeSurface::default());

    assert!(forward.is_clean());
    assert!(backward.is_clean());
    assert_eq!(forward.citations_resolved(), backward.citations_resolved());
    assert_eq!(forward.mints(), backward.mints());
}

/// A statement is minted once: minting it in two documents fails with both
/// locations named, and only one mint is counted. An author is shown the
/// pair to reconcile rather than told that a name is unavailable.
///
/// ´claim:check:minting-one-label-twice-fails-with-both-locations´
/// ´test:crate:duplicate-mints-fail-with-both-locations´
#[test]
fn duplicate_mints_fail_with_both_locations() {
    let sources = [
        source("one.md", "## Head · `sec:labels:syntax`\n"),
        source("two.md", "## Again · `sec:labels:syntax`\n"),
    ];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    let [Finding::DuplicateMint { first, second, .. }] = analysis.findings() else {
        panic!("expected one duplicate mint, got {:?}", analysis.findings());
    };

    assert_eq!(first.path(), Path::new("one.md"));
    assert_eq!(second.path(), Path::new("two.md"));
    assert_eq!(analysis.mints(), 1);
}

/// Resolution is total: a citation reaching no mint fails the run, so the
/// corpus cannot refer to a statement nobody made.
///
/// ´claim:check:a-citation-reaching-no-mint-fails-the-run´
/// ´test:crate:unresolved-citations-fail´
#[test]
fn unresolved_citations_fail() {
    let sources = [source("one.md", "Cites (`sec:labels:missing`).\n")];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(matches!(
        analysis.findings(),
        [Finding::UnresolvedCitation { .. }]
    ));
    assert!(!analysis.is_clean());
}

/// Text is never a failure: parenthesised spans holding a command, a file
/// name, or a colon-separated pair that is not a label leave the corpus
/// clean and mint nothing. Adopting the calculus does not make ordinary
/// documentation risky to write.
///
/// ´claim:check:spans-that-are-text-never-fail-a-run´
/// ´test:crate:unshaped-spans-never-fail´
#[test]
fn unshaped_spans_never_fail() {
    let sources = [source(
        "one.md",
        "Run (`cargo test`) and read (`Cargo.toml`) and (`one:two`).\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.mints(), 0);
}

/// A reserved kind may not be minted by hand: writing one at a head fails
/// as unwarranted and mints nothing, because a kind set aside for
/// derivation is filled by the derivation and by nothing else.
///
/// ´claim:check:a-reserved-kind-cannot-be-minted-by-hand´
/// ´test:crate:reserved-kinds-fail-as-unwarranted´
#[test]
fn reserved_kinds_fail_as_unwarranted() {
    let sources = [source(
        "one.md",
        "## Head · `test:integration:decode-roundtrip`\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(matches!(
        analysis.findings(),
        [Finding::UnwarrantedReservedKind { .. }]
    ));
    assert_eq!(analysis.mints(), 0);
}

/// A source's title is no exception to that rule and no exception is
/// written for it: a reserved token at a Title head is refused by the same
/// bare occurrence rule, because a kind set aside for derivation is set
/// aside wherever it is written, and titling a document neither widens nor
/// narrows what may be minted at its head.
///
/// ´claim:check:a-reserved-kind-cannot-be-minted-at-a-source-title-either´
/// ´test:crate:refuses-a-reserved-kind-written-at-a-source-title´
#[test]
fn refuses_a_reserved_kind_written_at_a_source_title() {
    let sources = [source(
        "one.md",
        "# A Titled Document · `test:integration:decode-roundtrip`\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(matches!(
        analysis.findings(),
        [Finding::UnwarrantedReservedKind { .. }]
    ));
    assert_eq!(analysis.mints(), 0);
}

/// An imported citation naming a prefix the signature does not register
/// fails: importing is reaching into a named corpus, and a corpus nobody
/// registered cannot be reached into.
///
/// ´claim:check:an-import-of-an-unregistered-prefix-fails´
/// ´test:crate:imported-citations-fail-under-an-empty-signature´
#[test]
fn imported_citations_fail_under_an_empty_signature() {
    let sources = [source(
        "one.md",
        "Imports (`[SPEC-def:parser:tokenizer]`).\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(matches!(
        analysis.findings(),
        [Finding::UnregisteredPrefix { .. }]
    ));
}

/// Importing from one's own owner fails even when the prefix is registered
/// and the label resolves: a citation within an owner is written in the
/// plain form, and qualifying it with one's own name says something untrue
/// about where the statement lives.
///
/// ´claim:check:importing-from-ones-own-owner-fails´
/// ´test:crate:self-qualified-imports-fail´
#[test]
fn self_qualified_imports_fail() {
    let mut prefixes = BTreeMap::new();
    prefixes.insert(
        Prefix::parse("INDEX").expect("well-formed"),
        Owner::new("index"),
    );
    let adoption = Adoption::new(prefixes, extended(), Owner::new("index"));

    let sources = [source(
        "one.md",
        "## Head · `sec:labels:syntax`\n\nImports (`[INDEX-sec:labels:syntax]`).\n",
    )];
    let analysis = analyze(&adoption, &sources, &CodeSurface::default());

    assert!(matches!(
        analysis.findings(),
        [Finding::SelfQualifiedImport { .. }]
    ));
}

/// An edge runs from the environment a citation stands inside to the
/// environment it names, not from the document or from the citation itself,
/// so the graph records which statement depends on which.
///
/// ´claim:check:an-edge-runs-from-the-citing-environment-to-the-cited-one´
/// ´test:crate:edges-run-from-the-containing-environment´
#[test]
fn edges_run_from_the_containing_environment() {
    let sources = [source(
        "one.md",
        "## First · `sec:labels:first`\n\nProse.\n\n## Second · `sec:labels:second`\n\nCites (`sec:labels:first`).\n",
    )];
    let analysis = analyze(&adoption(&[]), &sources, &CodeSurface::default());

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.graph().edge_count(), 1);

    let graph = analysis.graph();
    let edge = graph.edge_indices().next().expect("one edge");
    let (from, to) = graph.edge_endpoints(edge).expect("endpoints");

    assert_eq!(graph[from].label.to_string(), "sec:labels:second");
    assert_eq!(graph[to].label.to_string(), "sec:labels:first");
}

/// Prose may cite a test: the labels derived from the census are seeded
/// into the registries, so a document naming a test as its witness resolves
/// cleanly and edges to it. Documentation can point at the code that
/// establishes what it says.
///
/// ´claim:check:prose-may-cite-a-test-through-its-derived-label´
/// ´test:crate:resolves-a-prose-citation-of-a-derived-test-label´
#[test]
fn resolves_a_prose_citation_of_a_derived_test_label() {
    let (adoption, covered) = demo();
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n\nThe witness is (`test:unit:decodes-a-header`).\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
    assert_eq!(
        analysis.graph().edge_count(),
        1,
        "the citing environment edges to the test it names"
    );
}

/// Citing a test holds the prose to the corpus as it actually is: a
/// citation of a test that is not in the census fails, naming the label, so
/// renaming or deleting a test breaks the documentation that witnessed it
/// instead of leaving a quiet lie behind.
///
/// ´claim:check:citing-a-test-that-is-not-in-the-census-fails´
/// ´test:crate:fails-a-citation-of-a-test-that-is-not-in-the-census´
#[test]
fn fails_a_citation_of_a_test_that_is_not_in_the_census() {
    let (adoption, covered) = demo();
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n\nThe witness is (`test:unit:decodes-a-trailer`).\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    let [Finding::UnresolvedCitation { label, .. }] = analysis.findings() else {
        panic!(
            "expected one unresolved citation, got {:?}",
            analysis.findings()
        );
    };

    assert_eq!(label.to_string(), "test:unit:decodes-a-trailer");
    assert!(
        !analysis.is_clean(),
        "a renamed or deleted test is a failure"
    );
}

/// Seeding a derived label does not make it authorable: writing one at a
/// head is refused as an unwarranted reserved kind rather than reported as
/// a duplicate of the seeded mint, because the kind is checked before the
/// registry is consulted. Nothing is authored into that space.
///
/// ´claim:check:a-seeded-derived-label-still-cannot-be-minted-by-hand´
/// ´test:crate:refuses-a-hand-authored-test-mint-although-the-label-is-seeded´
#[test]
fn refuses_a_hand_authored_test_mint_although_the_label_is_seeded() {
    let (adoption, covered) = demo();
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `test:unit:decodes-a-header`\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(
        matches!(
            analysis.findings(),
            [Finding::UnwarrantedReservedKind { .. }]
        ),
        "a reserved kind is refused before the registry is consulted, so this is \
             never reported as a duplicate of the seeded mint: {:?}",
        analysis.findings()
    );
    assert_eq!(
        analysis.mints(),
        0,
        "and nothing was authored into that space"
    );
}

/// A derived label of another owner is reachable through the ordinary
/// import form: the repository's prose can cite a package's test by
/// qualifying it with that package's prefix, and no new mechanism is needed
/// for it.
///
/// ´claim:check:another-owners-derived-label-is-reachable-by-import´
/// ´test:crate:reaches-a-derived-label-of-another-owner-through-its-prefix´
#[test]
fn reaches_a_derived_label_of_another_owner_through_its_prefix() {
    let (adoption, covered) = demo();
    let sources = [source(
        "adr/001-one.md",
        "## Head · `sec:index:witnesses`\n\nThe witness is (`[DEMO-test:unit:decodes-a-header]`).\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
}

/// Derived mints are counted apart from authored ones while standing in the
/// same graph, so an inventory that outnumbers the prose cannot inflate the
/// figure describing how much the corpus was written.
///
/// ´claim:check:derived-mints-are-counted-apart-from-authored-ones´
/// ´test:crate:counts-derived-mints-apart-from-the-carriers´
#[test]
fn counts_derived_mints_apart_from_the_carriers() {
    let (adoption, covered) = demo();
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert_eq!(analysis.mints(), 1, "one head was written");
    assert_eq!(analysis.derived_mints(), 1, "and one label was derived");
    assert_eq!(analysis.graph().node_count(), 2, "both stand in the graph");
}

/// Order independence survives the seeding of the derived inventory: a
/// corpus citing a test yields the same counts whichever document is read
/// first, derived mints included.
///
/// (´claim:check:the-traversal-order-decides-nothing´)
/// ´test:crate:seeds-the-registries-in-either-traversal-order´
#[test]
fn seeds_the_registries_in_either_traversal_order() {
    let (adoption, covered) = demo();
    let citing = source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n\nThe witness is (`test:unit:decodes-a-header`).\n",
    );
    let other = source(
        "packages/demo/docs/other.md",
        "## Head · `sec:demo:other`\n",
    );

    let forward = analyze(
        &adoption,
        &[citing.clone(), other.clone()],
        &CodeSurface::default().with_tests(&covered),
    );
    let backward = analyze(
        &adoption,
        &[other, citing],
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(forward.is_clean());
    assert!(backward.is_clean());
    assert_eq!(forward.citations_resolved(), backward.citations_resolved());
    assert_eq!(forward.mints(), backward.mints());
    assert_eq!(forward.derived_mints(), backward.derived_mints());
}

/// A document may cite a notice: the labels the todo census derives are
/// seeded like any others, so prose naming a deficiency resolves cleanly and
/// edges to the code the deficiency stands in. The profile's promise that a
/// document can cite a notice is kept by the resolver rather than by care.
///
/// ´claim:check:prose-may-cite-a-notice-through-its-derived-label´
/// ´test:crate:resolves-a-prose-citation-of-a-notice´
#[test]
fn resolves_a_prose_citation_of_a_notice() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = notices("packages/demo/src/engine.rs", "// TODO: read the flag\n");
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:worklist`\n\nThe deficiency is (`todo:code:read-the-flag`).\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_notices(&covered),
    );

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
    assert_eq!(
        analysis.graph().edge_count(),
        1,
        "the citing environment edges to the notice"
    );
}

/// A comment may cite a notice on the same terms: the citation stands on the
/// other surface and reaches the same mint, because there is one resolution
/// space and a citation's surface decides nothing about what it may reach.
///
/// ´claim:check:a-comment-cites-a-notice-on-the-same-terms-as-prose´
/// ´test:crate:resolves-a-comment-citation-of-a-notice´
#[test]
fn resolves_a_comment_citation_of_a_notice() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = notices("packages/demo/src/engine.rs", "// TODO: read the flag\n");
    let commentary = format!("// blocked on ({ACUTE}todo:code:read-the-flag{ACUTE}) for now\n");
    let (citations, _findings) =
        scan_code_citations(Path::new("packages/demo/src/other.rs"), &commentary, &[]);

    assert_eq!(citations.len(), 1, "the fixture stands one citation");

    let code = CodeSurface::default()
        .with_notices(&covered)
        .with_citations(citations);
    let analysis = analyze(&adoption, &[], &code);

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
}

/// Citing a notice holds the citation to the corpus as it actually is: a
/// citation of a notice no source carries fails, naming the label, so
/// resolving a deficiency breaks the sentence that pointed at it instead of
/// leaving a quiet lie behind. This is the whole point of citing rather than
/// displaying.
///
/// ´claim:check:citing-a-notice-no-source-carries-fails´
/// ´test:crate:fails-a-citation-of-a-notice-no-source-carries´
#[test]
fn fails_a_citation_of_a_notice_no_source_carries() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = notices("packages/demo/src/engine.rs", "// TODO: read the flag\n");
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:worklist`\n\nThe deficiency is (`todo:code:read-the-banner`).\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_notices(&covered),
    );

    let [Finding::UnresolvedCitation { label, .. }] = analysis.findings() else {
        panic!(
            "expected one unresolved citation, got {:?}",
            analysis.findings()
        );
    };

    assert_eq!(label.to_string(), "todo:code:read-the-banner");
    assert!(!analysis.is_clean(), "a notice nobody carries is a failure");
}

/// Seeding a notice's label does not make it authorable, and citing one
/// never mints: a document writing the label at a head is refused as an
/// unwarranted reserved kind, while the same document's citation of the same
/// label resolves and mints nothing. The notice's identity stays where the
/// notice is.
///
/// ´claim:check:citing-a-notice-resolves-while-authoring-one-is-refused´
/// ´test:crate:refuses-a-hand-authored-notice-mint-and-mints-nothing-by-citing´
#[test]
fn refuses_a_hand_authored_notice_mint_and_mints_nothing_by_citing() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = notices("packages/demo/src/engine.rs", "// TODO: read the flag\n");

    let authored = [source(
        "packages/demo/docs/note.md",
        "## Head · `todo:code:read-the-flag`\n",
    )];
    let refused = analyze(
        &adoption,
        &authored,
        &CodeSurface::default().with_notices(&covered),
    );

    assert!(
        matches!(
            refused.findings(),
            [Finding::UnwarrantedReservedKind { .. }]
        ),
        "a reserved kind is refused before the registry is consulted: {:?}",
        refused.findings()
    );

    let citing = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:worklist`\n\nThe deficiency is (`todo:code:read-the-flag`).\n",
    )];
    let accepted = analyze(
        &adoption,
        &citing,
        &CodeSurface::default().with_notices(&covered),
    );

    assert!(accepted.is_clean(), "findings: {:?}", accepted.findings());
    assert_eq!(accepted.mints(), 1, "the head is the document's only mint");
}

/// Displaying a label is not citing it, and stays inert beside a citation
/// that is: a document showing one notice label in a non-participating span
/// while citing another resolves the citation, mints nothing extra, and does
/// not fail on the display — even where the displayed notice exists nowhere.
/// A record may quote a name it is not making a claim about.
///
/// ´claim:check:a-displayed-label-stays-inert-beside-a-citation-that-resolves´
/// ´test:crate:leaves-a-displayed-notice-label-inert´
#[test]
fn leaves_a_displayed_notice_label_inert() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let covered = notices("packages/demo/src/engine.rs", "// TODO: read the flag\n");
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:worklist`\n\nThe notice ``todo:code:read-the-banner`` is shown, \
             and (`todo:code:read-the-flag`) is cited.\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_notices(&covered),
    );

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(
        analysis.citations_resolved(),
        1,
        "the citation resolves and the display does not"
    );
    assert_eq!(analysis.mints(), 1, "and the display mints nothing");
}

/// A claim minted where its test is may be cited from prose: the ruling is
/// about the space rather than about a kind, so an authored mint standing in
/// code is reachable exactly as a derived one is. A document may name the
/// statement a test establishes.
///
/// ´claim:check:prose-may-cite-a-claim-minted-where-its-test-is´
/// ´test:crate:resolves-a-prose-citation-of-a-claim-minted-in-code´
#[test]
fn resolves_a_prose_citation_of_a_claim_minted_in_code() {
    let (adoption, covered) = demo_with_claim();
    let sources = [source(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n\nThe statement is (`claim:demo:a-header-decodes`).\n",
    )];

    let code = CodeSurface::default()
        .with_tests(&covered)
        .with_claims(&claims(&covered));
    let analysis = analyze(&adoption, &sources, &code);

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
}

/// A comment may cite a derived test label, which closes the last of the
/// four pairings: both kinds of mint reachable from both surfaces. Code may
/// point at the test that establishes what it says.
///
/// ´claim:check:a-comment-cites-a-derived-test-label´
/// ´test:crate:resolves-a-comment-citation-of-a-derived-test-label´
#[test]
fn resolves_a_comment_citation_of_a_derived_test_label() {
    let (adoption, covered) = demo();
    let commentary =
        format!("// witnessed by ({ACUTE}test:unit:decodes-a-header{ACUTE}) next door\n");
    let (citations, _findings) =
        scan_code_citations(Path::new("packages/demo/src/other.rs"), &commentary, &[]);

    let code = CodeSurface::default()
        .with_tests(&covered)
        .with_citations(citations);
    let analysis = analyze(&adoption, &[], &code);

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
}

/// Resolution is total on the code surface too: a comment citing a statement
/// nobody minted fails, so commentary cannot refer a claim into existence any
/// more than prose can.
///
/// ´claim:check:a-comment-citing-what-nobody-minted-fails´
/// ´test:crate:fails-a-comment-citation-of-a-claim-nobody-minted´
#[test]
fn fails_a_comment_citation_of_a_claim_nobody_minted() {
    let (adoption, covered) = demo_with_claim();
    let commentary = format!("// as ({ACUTE}claim:demo:a-trailer-decodes{ACUTE}) has it\n");
    let (citations, _findings) =
        scan_code_citations(Path::new("packages/demo/src/other.rs"), &commentary, &[]);

    let code = CodeSurface::default()
        .with_tests(&covered)
        .with_claims(&claims(&covered))
        .with_citations(citations);
    let analysis = analyze(&adoption, &[], &code);

    let [Finding::UnresolvedCitation { label, .. }] = analysis.findings() else {
        panic!(
            "expected one unresolved citation, got {:?}",
            analysis.findings()
        );
    };

    assert_eq!(label.to_string(), "claim:demo:a-trailer-decodes");
}

/// A folder readme whose matrix region carries the given rows.
fn matrix_readme(rows: &str) -> Source {
    Source::new(
        "packages/demo/src/tests/README.md",
        format!(
            "## Crate test matrix · `tab:demo:crate-test-matrix`\n\n\
                 **Table (Crate test matrix)**\n\n\
                 | Test | Area | Claim |\n|------|------|-------|\n{rows}\n"
        )
        .as_str(),
    )
}

/// A generated citation resolves like any other, against the registries
/// the generator emitted from: a matrix row citing a censused test edges
/// to it, mints nothing, and feeds nothing it presents — the document's
/// mint count stays the head's alone, and the census the row was generated
/// from is untouched by the row resolving against it.
///
/// ´claim:check:a-generated-citation-resolves-against-the-completed-registries´
/// ´test:crate:resolves-a-generated-citation-without-minting´
#[test]
fn resolves_a_generated_citation_without_minting() {
    let (adoption, covered) = demo();
    let sources = [matrix_readme(
        "| (`test:unit:decodes-a-header`) | demo | The header decodes. |",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(
        analysis.citations_resolved(),
        1,
        "the row's citation resolves"
    );
    assert_eq!(analysis.mints(), 1, "the head is the document's only mint");
    assert_eq!(
        analysis.derived_mints(),
        1,
        "the census seeded exactly what it held"
    );
}

/// A generated citation that resolves nowhere is never the author's slip:
/// the register is stale after a transition or its generator wrote a
/// citation of nothing, so it fails under its own code, beside the
/// exactness check, rather than as an unresolved citation wanting an edit
/// inside a region no hand may write.
///
/// ´claim:check:a-dangling-generated-citation-blames-the-register-not-the-author´
/// ´test:crate:fails-a-dangling-citation-in-a-generated-region´
#[test]
fn fails_a_dangling_citation_in_a_generated_region() {
    let (adoption, covered) = demo();
    let sources = [matrix_readme(
        "| (`test:unit:decodes-a-trailer`) | demo | A test that is gone. |",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    let [Finding::DanglingGeneratedCitation { label, .. }] = analysis.findings() else {
        panic!(
            "expected one dangling generated citation, got {:?}",
            analysis.findings()
        );
    };

    assert_eq!(label.to_string(), "test:unit:decodes-a-trailer");
    assert!(!analysis.is_clean(), "a stale register is a failure");
}

/// A bare span in a generated region mints nothing and fails as the
/// generator's: a generated occurrence is a mint only where a profile sets
/// its standard place in the register, and no adopted profile does, so
/// nothing enters the registry and no-self-support stays a theorem — a
/// register row cannot sustain its own membership however it is written.
///
/// ´claim:check:a-bare-span-in-a-generated-region-mints-nothing-and-fails´
/// ´test:crate:refuses-a-bare-span-in-a-generated-region´
#[test]
fn refuses_a_bare_span_in_a_generated_region() {
    let (adoption, covered) = demo();
    let sources = [matrix_readme(
        "| `sec:demo:witnesses` | demo | A mint the generator invented. |",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    let [Finding::BareGeneratedOccurrence { label, .. }] = analysis.findings() else {
        panic!(
            "expected one bare generated occurrence, got {:?}",
            analysis.findings()
        );
    };

    assert_eq!(label.to_string(), "sec:demo:witnesses");
    assert_eq!(
        analysis.mints(),
        1,
        "the head is the document's only mint, and the row entered no registry"
    );
}

/// The same head-and-region shape standing outside a folder readme is
/// authored prose about the projection, not the projection: a worked
/// example in a record participates as authored occurrences do, and a
/// citation dangling there is the author's ordinary unresolved citation.
///
/// ´claim:check:only-a-folder-readme-carries-a-generated-matrix-region´
/// ´test:crate:leaves-a-worked-example-outside-a-readme-authored´
#[test]
fn leaves_a_worked_example_outside_a_readme_authored() {
    let (adoption, covered) = demo();
    let sources = [Source::new(
        "packages/demo/docs/policy.md",
        "## Crate test matrix · `tab:demo:crate-test-matrix`\n\n\
             **Table (Crate test matrix)**\n\n\
             | Test | Area | Claim |\n|------|------|-------|\n\
             | (`test:unit:decodes-a-trailer`) | demo | A worked example. |\n",
    )];

    let analysis = analyze(
        &adoption,
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );

    assert!(
        matches!(analysis.findings(), [Finding::UnresolvedCitation { .. }]),
        "outside a readme the citation is authored and fails as itself: {:?}",
        analysis.findings()
    );
}

/// A document mint is a mint like any other once its head is recognised: a
/// title minted at a source's first level-one heading enters its owner's
/// registry, a sentence elsewhere in that owner cites it in the plain form,
/// and the citation resolves into an edge. The Title class changes no
/// resolution judgment.
///
/// ´claim:check:a-document-mint-resolves-a-same-owner-citation´
/// ´test:crate:resolves-a-same-owner-citation-of-a-document-mint´
#[test]
fn resolves_a_same_owner_citation_of_a_document_mint() {
    let minting = source(
        "adr/014-label-calculus.md",
        "# A Calculus of Labels · `doc:labels:calculus`\n",
    );
    let citing = source(
        "adr/024-document-title-labels.md",
        "# Document-Title Labels · `doc:labels:document-title-labels`\n\n\
             ## Context · `sec:doctitles:context`\n\n\
             The calculus is (`doc:labels:calculus`).\n",
    );

    let analysis = analyze(&adoption(&[]), &[minting, citing], &CodeSurface::default());

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.mints(), 3);
    assert_eq!(analysis.citations_resolved(), 1);
    assert_eq!(analysis.graph().edge_count(), 1);
}

/// Another owner reaches a document mint by the ordinary imported form,
/// through the prefix its corpus is registered under. Uniqueness stays the
/// pair of owner and complete label, so a title is named across owners the
/// way every other environment is.
///
/// ´claim:check:a-document-mint-resolves-an-imported-citation´
/// ´test:crate:resolves-an-imported-citation-of-a-document-mint´
#[test]
fn resolves_an_imported_citation_of_a_document_mint() {
    let adoption = adoption(&[Package::new("torrust-demo", "packages/demo")]);
    let minting = source(
        "packages/demo/docs/note.md",
        "# The Demonstration Note · `doc:demo:note`\n",
    );
    let citing = source(
        "adr/001-one.md",
        "## Head · `sec:index:witnesses`\n\nThe note is (`[DEMO-doc:demo:note]`).\n",
    );

    let analysis = analyze(&adoption, &[minting, citing], &CodeSurface::default());

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(analysis.citations_resolved(), 1);
    assert_eq!(analysis.graph().edge_count(), 1);
}
