// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Claim-coverage reporting tests over census and calculus results.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`counts_the_claims_the_census_holds`] | coverage | The report counts the claim census as the staging leaves it: how many tests are covered, how many carry a statement, how many carry none, and how the statements divide into the minted and the cited. These are the figures the authoring waves are steered by, so they come from the census the check judges against rather than from a second reading of it. |
//! | [`lists_the_claims_no_test_cites`] | coverage | A statement no sibling test cites is counted and listed: most statements are established once and referred to never, so this is a fact about the corpus rather than a defect of it, and the listing is bounded because the uncited outnumber the cited many times over. |
//! | [`witnesses_an_intent_a_test_carries`] | coverage | An intent is a claim minted in prose, and it is witnessed exactly when a covered test of the same owner carries that label. The two registries stay apart — the prose mint stands in the carrier, the test's in the census — so one statement may be written in both places without colliding, and writing the test is the whole of what turns an intent into coverage. |
//! | [`witnesses_an_intent_a_citation_carries`] | coverage | A statement a test only cites still witnesses the intent that named it: the intent asks whether anything establishes the statement, and a citing test establishes it as surely as the sibling it points at. Otherwise an intent minted for shared coverage would read as unkept whenever the mint happened to sit in another file. |
//! | [`counts_claim_coverage_per_owner_and_per_area`] | coverage | The claims are partitioned two ways at once — by the owner whose corpus holds them and by the area they name — and each part carries its own counts of statements, citations, intents and unkept promises, so a reviewer can ask which subject is written and which is only meant. |
//! | [`reduces_the_summary_to_the_checks_counts`] | coverage | The check report carries the figures without the listings, exactly as it carries the burn families without naming them: a gate's result reports what a corpus came to, and the enumerations belong to the command whose whole result they are. |
//! | [`reports_no_intents_where_prose_mints_none`] | coverage | A corpus whose prose mints no claim has no intents to report and is not a corpus with a problem: the report lands before any intent is written, and says so in zeroes rather than in silence. |

use std::path::Path;

use crate::adoption::{Adoption, index_adoption as build_index_adoption};
use crate::carrier::Source;
use crate::census::{Census, scan_source};
use crate::claim::analyze_claims;
use crate::code::CodeSurface;
use crate::engine::analyze;
use crate::profile::{CoveredAsset, cover};
use crate::workspace::Package;
use crate::{DEFAULT_UNCITED, summarise_coverage};

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

/// The acute the code syntax delimits an occurrence with.
const ACUTE: char = '\u{b4}';

fn packages() -> Vec<Package> {
    vec![Package::new("torrust-demo", "packages/demo")]
}

/// Cover one package's sources the way the check covers them.
fn assets_of(sources: &[(&str, &str)]) -> Vec<CoveredAsset> {
    let mut tests = Vec::new();

    for (path, text) in sources {
        tests.extend(scan_source("torrust-demo", Path::new(path), text).expect("a Rust source"));
    }

    let (assets, findings) = cover(&packages(), &Census::from_tests(tests, sources.len()));

    assert!(
        findings.is_empty(),
        "the fixture covers cleanly: {findings:?}"
    );

    assets
}

/// Summarise one fixture's Rust sources against one fixture's prose.
fn summarise(rust: &[(&str, &str)], prose: &[Source]) -> crate::CoverageSummary {
    let assets = assets_of(rust);
    let adoption = index_adoption(
        &packages(),
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let analysis = analyze(
        &adoption,
        prose,
        &CodeSurface::default().with_tests(&assets),
    );
    let (analysis_of_claims, claims, _findings) = analyze_claims(&assets, &[]);

    summarise_coverage(
        &analysis,
        &adoption,
        &claims,
        analysis_of_claims.covered,
        DEFAULT_UNCITED,
    )
}

/// One test minting, one citing it, one nothing cites, and one unclaimed.
fn corpus() -> String {
    format!(
        "/// The statement.\n///\n/// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
             /// {ACUTE}test:crate:mints{ACUTE}\n#[test]\nfn mints() {{}}\n\n\
             /// ({ACUTE}claim:resonance:the-statement{ACUTE})\n\
             /// {ACUTE}test:crate:cites{ACUTE}\n#[test]\nfn cites() {{}}\n\n\
             /// A statement nothing refers to.\n///\n\
             /// {ACUTE}claim:decay:the-lonely-statement{ACUTE}\n\
             /// {ACUTE}test:crate:alone{ACUTE}\n#[test]\nfn alone() {{}}\n\n\
             /// {ACUTE}test:crate:bare{ACUTE}\n#[test]\nfn bare() {{}}\n"
    )
}

/// The fixture's Rust sources, as the covering pass wants them.
const fn sources(text: &str) -> [(&str, &str); 1] {
    [("packages/demo/src/tests/demo.rs", text)]
}

/// A readme minting one intent a test witnesses and one nothing does.
fn prose() -> Vec<Source> {
    vec![Source::new(
        "packages/demo/tests/README.md",
        "**Claim (The statement)** · `claim:resonance:the-statement`\n\n\
             Said once, where the test is.\n\n\
             **Claim (The promise)** · `claim:decay:a-promise-nobody-has-kept`\n\n\
             Stale state dissolves on a bounded schedule.\n",
    )]
}

/// The report counts the claim census as the staging leaves it: how many
/// tests are covered, how many carry a statement, how many carry none, and
/// how the statements divide into the minted and the cited. These are the
/// figures the authoring waves are steered by, so they come from the census
/// the check judges against rather than from a second reading of it.
///
/// ´claim:coverage:the-report-counts-the-census-the-check-judges-against´
/// ´test:crate:counts-the-claims-the-census-holds´
#[test]
fn counts_the_claims_the_census_holds() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &prose());

    assert_eq!(summary.covered, 4);
    assert_eq!(summary.claimed, 3);
    assert_eq!(summary.unclaimed, 1);
    assert_eq!(summary.mints, 2);
    assert_eq!(summary.citations, 1);
}

/// A statement no sibling test cites is counted and listed: most statements
/// are established once and referred to never, so this is a fact about the
/// corpus rather than a defect of it, and the listing is bounded because the
/// uncited outnumber the cited many times over.
///
/// ´claim:coverage:a-statement-nothing-cites-is-counted-and-listed-bounded´
/// ´test:crate:lists-the-claims-no-test-cites´
#[test]
fn lists_the_claims_no_test_cites() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &prose());

    assert_eq!(summary.uncited, 1, "the cited statement is not among them");

    let uncited: Vec<String> = summary
        .uncited_claims
        .iter()
        .map(|site| site.label.to_string())
        .collect();

    assert_eq!(
        uncited,
        ["claim:decay:the-lonely-statement"].map(String::from)
    );
    assert_eq!(summary.by_area["resonance"].uncited, 0);
    assert_eq!(summary.by_area["decay"].uncited, 1);
}

/// An intent is a claim minted in prose, and it is witnessed exactly when a
/// covered test of the same owner carries that label. The two registries stay
/// apart — the prose mint stands in the carrier, the test's in the census —
/// so one statement may be written in both places without colliding, and
/// writing the test is the whole of what turns an intent into coverage.
///
/// ´claim:coverage:an-intent-is-witnessed-by-a-covered-test-carrying-its-label´
/// ´test:crate:witnesses-an-intent-a-test-carries´
#[test]
fn witnesses_an_intent_a_test_carries() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &prose());

    assert_eq!(summary.intents, 2);
    assert_eq!(summary.unwitnessed, 1);

    let unwitnessed: Vec<String> = summary
        .intent_sites
        .iter()
        .filter(|site| !site.witnessed)
        .map(|site| site.label.to_string())
        .collect();

    assert_eq!(
        unwitnessed,
        ["claim:decay:a-promise-nobody-has-kept"].map(String::from)
    );
}

/// A statement a test only cites still witnesses the intent that named it:
/// the intent asks whether anything establishes the statement, and a citing
/// test establishes it as surely as the sibling it points at. Otherwise an
/// intent minted for shared coverage would read as unkept whenever the mint
/// happened to sit in another file.
///
/// ´claim:coverage:a-citing-test-witnesses-an-intent-as-a-minting-one-does´
/// ´test:crate:witnesses-an-intent-a-citation-carries´
#[test]
fn witnesses_an_intent_a_citation_carries() {
    let text = format!(
        "/// The statement.\n///\n/// {ACUTE}claim:resonance:minted-elsewhere{ACUTE}\n\
             /// {ACUTE}test:crate:mints{ACUTE}\n#[test]\nfn mints() {{}}\n\n\
             /// ({ACUTE}claim:resonance:minted-elsewhere{ACUTE})\n\
             /// {ACUTE}test:crate:cites{ACUTE}\n#[test]\nfn cites() {{}}\n"
    );
    let rust = sources(&text);
    let prose = [Source::new(
        "packages/demo/tests/README.md",
        "**Claim (Minted elsewhere)** · `claim:resonance:minted-elsewhere`\n\nThe statement.\n",
    )];

    let summary = summarise(&rust, &prose);

    assert_eq!(summary.intents, 1);
    assert_eq!(
        summary.unwitnessed, 0,
        "a citation carries the label as a mint does"
    );
}

/// The claims are partitioned two ways at once — by the owner whose corpus
/// holds them and by the area they name — and each part carries its own
/// counts of statements, citations, intents and unkept promises, so a
/// reviewer can ask which subject is written and which is only meant.
///
/// ´claim:coverage:the-claims-are-counted-per-owner-and-per-area´
/// ´test:crate:counts-claim-coverage-per-owner-and-per-area´
#[test]
fn counts_claim_coverage_per_owner_and_per_area() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &prose());

    let demo = &summary.by_owner["torrust-demo"];

    assert_eq!(demo.claims, 2);
    assert_eq!(demo.citations, 1);
    assert_eq!(demo.intents, 2);
    assert_eq!(demo.unwitnessed, 1);

    assert_eq!(summary.by_area["resonance"].claims, 1);
    assert_eq!(summary.by_area["resonance"].intents, 1);
    assert_eq!(summary.by_area["decay"].unwitnessed, 1);
}

/// The check report carries the figures without the listings, exactly as it
/// carries the burn families without naming them: a gate's result reports
/// what a corpus came to, and the enumerations belong to the command whose
/// whole result they are.
///
/// ´claim:coverage:the-check-report-carries-the-figures-without-the-listings´
/// ´test:crate:reduces-the-summary-to-the-checks-counts´
#[test]
fn reduces_the_summary_to_the_checks_counts() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &prose());
    let counts = summary.counts();

    assert_eq!(counts.covered, summary.covered);
    assert_eq!(counts.mints, summary.mints);
    assert_eq!(counts.uncited, summary.uncited);
    assert_eq!(counts.intents, summary.intents);
    assert_eq!(counts.unwitnessed, summary.unwitnessed);
}

/// A corpus whose prose mints no claim has no intents to report and is not a
/// corpus with a problem: the report lands before any intent is written, and
/// says so in zeroes rather than in silence.
///
/// ´claim:coverage:a-corpus-with-no-intents-reports-zero-of-them´
/// ´test:crate:reports-no-intents-where-prose-mints-none´
#[test]
fn reports_no_intents_where_prose_mints_none() {
    let corpus = corpus();
    let summary = summarise(&sources(&corpus), &[]);

    assert_eq!(summary.intents, 0);
    assert_eq!(summary.unwitnessed, 0);
    assert!(summary.intent_sites.is_empty());
    assert_eq!(summary.mints, 2, "the tests are counted all the same");
}
