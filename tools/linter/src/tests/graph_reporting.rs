// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Label-graph reporting tests over completed calculus analyses.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`counts_mints_and_citations_over_the_graph`] | graph | The review summary counts the corpus as a graph: every mint is a node and every citation an edge, and both totals come out of the graph rather than from a second traversal of the sources. A reverse lookup appears only when one was asked for. |
//! | [`lists_the_mints_nothing_cites`] | graph | An orphan is a mint nothing cites, and citing others does not exempt a mint from being one: the listing is about in-degree alone. It is advisory, so a long list is something a reviewer looks at rather than something a run fails on. |
//! | [`ranks_the_most_cited_mints`] | graph | The hubs are the cited mints ranked by how many cite them, uncited ones being no part of the ranking, and the listing is bounded by the caller's limit — asking for one yields the top one, asking for none yields none. |
//! | [`counts_coverage_per_owner_and_per_area`] | graph | The corpus is partitioned two ways at once — by the owner a mint belongs to and by the area it names — and each part carries its own counts of mints, citations and orphans, so a reviewer can ask where the corpus is dense and where it is uncited without reading it. |
//! | [`looks_a_label_up_in_reverse`] | graph | A reverse lookup answers who cites a label: where it is minted and every site citing it, presented in the order a reader would visit them — by source, then by position within it — and each site says whether the citation crossed an ownership boundary. |
//! | [`lists_the_citations_that_reach_no_mint`] | graph | Citations resolving nowhere are collected for the reviewer as what the check already found, carrying the check's own code rather than a verdict of the report's own. One rule keeps one verdict. |
//! | [`leaves_derived_mints_out_of_every_listing`] | graph | The derived inventory of tests stays out of every listing and every count of the review: an uncited test is not an orphan of the prose, is no hub, and contributes no area. A citation of a test is still an edge, so prose may cite a test without the inventory swamping the report it would otherwise outnumber several times over. |

use std::path::Path;

use crate::adoption::{Adoption, index_adoption as build_index_adoption};
use crate::carrier::Source;
use crate::census::{Census, scan_source};
use crate::code::CodeSurface;
use crate::engine::{Analysis, analyze};
use crate::finding::Finding;
use crate::label::Label;
use crate::profile::cover;
use crate::workspace::Package;
use crate::{DEFAULT_HUBS, dangling, summarise};

fn index_adoption(
    packages: &[Package],
    names: Option<&crate::roster::OwnerNames>,
    assemblies: &[crate::assembly::Assembly],
) -> Adoption {
    build_index_adoption(
        packages,
        names,
        assemblies,
        crate::registry::fixture_kind_registry(),
    )
}

fn label(text: &str) -> Label {
    Label::parse(text).expect("well-formed")
}

/// One head cited twice, one head cited once, one head cited by nothing.
fn corpus() -> Analysis {
    let sources = [
        Source::new("adr/one.md", "## Hub · `sec:fixture:hub`\n\nProse.\n"),
        Source::new(
            "adr/two.md",
            "## Middle · `sec:fixture:middle`\n\nCites (`sec:fixture:hub`).\n",
        ),
        Source::new(
            "adr/three.md",
            "## Leaf · `sec:other:leaf`\n\nCites (`sec:fixture:hub`) and (`sec:fixture:middle`).\n",
        ),
        Source::new("adr/four.md", "## Orphan · `sec:other:orphan`\n\nProse.\n"),
    ];

    analyze(
        &index_adoption(
            &[],
            Some(&crate::roster::OwnerNames::new(
                "torrust-",
                [crate::roster::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default(),
    )
}

/// The review summary counts the corpus as a graph: every mint is a node
/// and every citation an edge, and both totals come out of the graph rather
/// than from a second traversal of the sources. A reverse lookup appears
/// only when one was asked for.
///
/// ´claim:graph:the-summary-counts-mints-and-citations-over-the-graph´
/// ´test:crate:counts-mints-and-citations-over-the-graph´
#[test]
fn counts_mints_and_citations_over_the_graph() {
    let summary = summarise(&corpus(), None, DEFAULT_HUBS);

    assert_eq!(summary.mints, 4);
    assert_eq!(summary.citations, 3);
    assert!(summary.reverse.is_none(), "no lookup was asked for");
}

/// An orphan is a mint nothing cites, and citing others does not exempt a
/// mint from being one: the listing is about in-degree alone. It is
/// advisory, so a long list is something a reviewer looks at rather than
/// something a run fails on.
///
/// ´claim:graph:an-orphan-is-a-mint-nothing-cites´
/// ´test:crate:lists-the-mints-nothing-cites´
#[test]
fn lists_the_mints_nothing_cites() {
    let summary = summarise(&corpus(), None, DEFAULT_HUBS);
    let orphans: Vec<String> = summary
        .orphans
        .iter()
        .map(|site| site.label.to_string())
        .collect();

    assert_eq!(
        orphans,
        ["sec:other:leaf", "sec:other:orphan"].map(String::from),
        "the two cited mints are not orphans, and one of the orphans does cite"
    );
}

/// The hubs are the cited mints ranked by how many cite them, uncited ones
/// being no part of the ranking, and the listing is bounded by the caller's
/// limit — asking for one yields the top one, asking for none yields none.
///
/// ´claim:graph:hubs-are-cited-mints-ranked-and-bounded´
/// ´test:crate:ranks-the-most-cited-mints´
#[test]
fn ranks_the_most_cited_mints() {
    let summary = summarise(&corpus(), None, DEFAULT_HUBS);

    assert_eq!(summary.hubs.len(), 2, "only cited mints are hubs");
    assert_eq!(summary.hubs[0].label.to_string(), "sec:fixture:hub");
    assert_eq!(summary.hubs[0].citations, 2);
    assert_eq!(summary.hubs[1].citations, 1);

    assert_eq!(
        summarise(&corpus(), None, 1).hubs.len(),
        1,
        "the listing is bounded"
    );
    assert_eq!(summarise(&corpus(), None, 0).hubs.len(), 0);
}

/// The corpus is partitioned two ways at once — by the owner a mint belongs
/// to and by the area it names — and each part carries its own counts of
/// mints, citations and orphans, so a reviewer can ask where the corpus is
/// dense and where it is uncited without reading it.
///
/// ´claim:graph:the-corpus-is-counted-per-owner-and-per-area´
/// ´test:crate:counts-coverage-per-owner-and-per-area´
#[test]
fn counts_coverage_per_owner_and_per_area() {
    let summary = summarise(&corpus(), None, DEFAULT_HUBS);

    let index = &summary.by_owner["index"];
    assert_eq!(index.mints, 4, "the repository's prose is one owner");
    assert_eq!(index.citations, 3);

    let fixture = &summary.by_area["fixture"];
    assert_eq!(fixture.mints, 2);
    assert_eq!(fixture.cited, 2);
    assert_eq!(fixture.orphans, 0);

    let other = &summary.by_area["other"];
    assert_eq!(other.mints, 2);
    assert_eq!(other.orphans, 2, "the leaf cites but is never cited");
}

/// A reverse lookup answers who cites a label: where it is minted and every
/// site citing it, presented in the order a reader would visit them — by
/// source, then by position within it — and each site says whether the
/// citation crossed an ownership boundary.
///
/// ´claim:graph:a-reverse-lookup-names-the-mint-and-its-citers-in-reading-order´
/// ´test:crate:looks-a-label-up-in-reverse´
#[test]
fn looks_a_label_up_in_reverse() {
    let summary = summarise(&corpus(), Some(&label("sec:fixture:hub")), DEFAULT_HUBS);
    let reverse = summary.reverse.expect("a lookup was asked for");

    assert_eq!(reverse.mints.len(), 1);
    assert_eq!(reverse.citers.len(), 2);

    let citing: Vec<String> = reverse
        .citers
        .iter()
        .map(|site| site.from.to_string())
        .collect();
    assert_eq!(
        citing,
        ["sec:other:leaf", "sec:fixture:middle"].map(String::from),
        "citers are presented where a reader would visit them: by source, then position"
    );
    assert!(reverse.citers.iter().all(|site| !site.imported));
}

/// Citations resolving nowhere are collected for the reviewer as what the
/// check already found, carrying the check's own code rather than a verdict
/// of the report's own. One rule keeps one verdict.
///
/// ´claim:graph:dangling-citations-are-relayed-from-the-check-not-re-derived´
/// ´test:crate:lists-the-citations-that-reach-no-mint´
#[test]
fn lists_the_citations_that_reach_no_mint() {
    let sources = [Source::new(
        "adr/one.md",
        "## Head · `sec:fixture:head`\n\nCites (`sec:fixture:missing`).\n",
    )];
    let analysis = analyze(
        &index_adoption(
            &[],
            Some(&crate::roster::OwnerNames::new(
                "torrust-",
                [crate::roster::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default(),
    );

    let codes: Vec<&str> = dangling(&analysis).iter().map(Finding::code).collect();

    assert_eq!(codes, ["unresolved_citation"]);
}

/// The derived inventory of tests stays out of every listing and every
/// count of the review: an uncited test is not an orphan of the prose, is
/// no hub, and contributes no area. A citation of a test is still an edge,
/// so prose may cite a test without the inventory swamping the report it
/// would otherwise outnumber several times over.
///
/// ´claim:graph:derived-mints-are-absent-from-every-listing-and-count´
/// ´test:crate:leaves-derived-mints-out-of-every-listing´
#[test]
fn leaves_derived_mints_out_of_every_listing() {
    let packages = [Package::new("torrust-demo", "packages/demo")];
    let tests = scan_source(
        "torrust-demo",
        Path::new("packages/demo/src/engine.rs"),
        "/// \u{b4}test:unit:reports-a-header\u{b4}\n#[test]\nfn reports_a_header() {}\n\
             /// \u{b4}test:unit:reports-a-trailer\u{b4}\n#[test]\nfn reports_a_trailer() {}\n",
    )
    .expect("a Rust source");
    let (covered, _findings) = cover(&packages, &Census::from_tests(tests, 1));

    let sources = [Source::new(
        "packages/demo/docs/note.md",
        "## Head · `sec:demo:witnesses`\n\nOne witness is (`test:unit:reports-a-header`).\n",
    )];
    let analysis = analyze(
        &index_adoption(
            &packages,
            Some(&crate::roster::OwnerNames::new(
                "torrust-",
                [crate::roster::UnbuiltMember::new(
                    "torrust-notime",
                    "packages/notime",
                )],
            )),
            &[],
        ),
        &sources,
        &CodeSurface::default().with_tests(&covered),
    );
    let summary = summarise(&analysis, None, DEFAULT_HUBS);

    assert!(analysis.is_clean(), "findings: {:?}", analysis.findings());
    assert_eq!(
        summary.mints, 1,
        "the derived pair is no part of the corpus reported"
    );
    assert_eq!(
        summary.citations, 1,
        "though the citation of one of them is an edge"
    );

    let orphans: Vec<String> = summary
        .orphans
        .iter()
        .map(|site| site.label.to_string())
        .collect();

    assert_eq!(
        orphans,
        ["sec:demo:witnesses"].map(String::from),
        "the uncited test is not an orphan of the prose, and would swamp the listing if it were"
    );
    assert!(summary.hubs.is_empty(), "a derived mint is no hub either");
    assert!(
        !summary.by_area.contains_key("unit"),
        "nor an area of the corpus"
    );
    assert_eq!(summary.by_owner["torrust-demo"].mints, 1);
}
