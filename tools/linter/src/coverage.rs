// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The claim-coverage report: what is claimed, what witnesses it, and what does
//! not.
//!
//! ADR-L-017 names the coverage report twice and gives it two jobs. It is the
//! staging instrument — a covered test with no claim is counted here and reported
//! nowhere until its package's authoring wave closes — and it is the instrument
//! the scenario matrix retires behind, the thing that proves nothing was dropped
//! when a document carrying ninety-one promises stops existing.
//!
//! The first job the claim profile already does, per package, in the counts it
//! keeps. This module does the second, and the second needs one fact the first
//! cannot hold: an intent.
//!
//! # An intent is a claim nothing witnesses
//!
//! A claim minted in a test says the statement is established, and the test is
//! the establishing. A claim minted in a readme's prose says only that somebody
//! meant it: the statement is written down, named, and answerable, and no test
//! carries it yet. The record calls that an intent, and rules that its
//! uncoveredness is a report line rather than a tracker column — which is exactly
//! this report, and exactly why it replaces the hand-written coverage tracker
//! rather than regenerating it.
//!
//! The two live in two registries and that is what makes the join possible at
//! all. A claim mint in a Rust comment stands in the claim census, which is not
//! the carrier; a claim mint in prose stands in the carrier, at a head, like
//! every other authored mint. So the same label may be minted once in each
//! without colliding, and an intent is witnessed precisely when some covered test
//! of the same owner carries that label — minting it, or citing a sibling that
//! did. Writing the test is therefore the whole of the migration from intent to
//! coverage: the prose mint stays where it is, and stops being reported here.
//!
//! # Why nothing here fails
//!
//! An unwitnessed intent is a promise nobody has kept yet, and a corpus is
//! allowed to have those — a corpus that could not would have no way to write
//! down what it means to do. An uncited claim is not even that: most statements
//! are established once and never referred to again, exactly as most mints of the
//! prose corpus are orphans. Both are facts, both are counted, and neither moves
//! an exit status. The report's whole force is that the facts are visible and are
//! computed from the same census the check judges against, so a reviewer and a
//! gate can never be reading two different corpora.
//!
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::adoption::Adoption;
use crate::claim::{CLAIM_KIND, CoveredClaim};
use crate::engine::{Analysis, Surface};
use crate::finding::Location;
use crate::label::Label;

/// How many uncited claims the report lists by default.
///
/// The listing is bounded for the reason the hub listing is bounded and the
/// unwitnessed listing is not: a corpus mints far more statements than it cites,
/// so an unbounded listing of the uncited would be thousands long and would bury
/// the handful of lines a reviewer came for. The count beside it is unbounded and
/// is the figure to reason from.
///
/// TODO ´todo:code:that-argument-fixes-only-that-the´: that argument fixes only that the listing is bounded somewhere, and no
/// record fixes where. The figure wants a measurement of what a reviewer
/// actually reads before scrolling past, which nothing here has taken.
///
/// ´const:emberlinter:uncited-listing-bound´ (´[EMBER-alg:const:count]´)
/// ´const:emberlinter:uncited-listing-bound-count-20´
pub const DEFAULT_UNCITED: usize = 20;

/// One claim mint standing in a test, as the report names it.
#[derive(Debug, Clone, Serialize)]
pub struct ClaimSite {
    /// The owner whose census holds the mint.
    pub owner: String,
    /// The minted claim.
    pub label: Label,
    /// The test that established the statement.
    pub test: String,
    /// Where the test stands.
    pub location: Location,
}

/// One intent: a claim minted in prose, as the report names it.
#[derive(Debug, Clone, Serialize)]
pub struct IntentSite {
    /// The owner whose prose minted the intent.
    pub owner: String,
    /// The minted claim.
    pub label: Label,
    /// Where the minting head stands.
    pub location: Location,
    /// Whether a covered test of that owner carries the label.
    pub witnessed: bool,
}

/// The counts for one cell of a partition of the claims.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaimCoverage {
    /// How many statements the cell's tests mint.
    pub claims: usize,
    /// How many citations of them stand in the cell's tests.
    pub citations: usize,
    /// How many of those mints no sibling test cites.
    pub uncited: usize,
    /// How many intents the cell's prose mints.
    pub intents: usize,
    /// How many of those intents no covered test witnesses.
    pub unwitnessed: usize,
}

/// What the coverage report says about the claims of a corpus.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageSummary {
    /// How many covered tests the claim census considered.
    pub covered: usize,
    /// How many of them carry a claim at the standard place.
    pub claimed: usize,
    /// How many carry none, which the staging counts rather than reports.
    pub unclaimed: usize,
    /// How many statements the corpus's tests mint.
    pub mints: usize,
    /// How many citations of them stand in the corpus's tests.
    pub citations: usize,
    /// How many minted statements no sibling test cites.
    pub uncited: usize,
    /// How many intents stand in the corpus's prose.
    pub intents: usize,
    /// How many of those intents no covered test witnesses.
    pub unwitnessed: usize,
    /// The counts, per owner.
    pub by_owner: BTreeMap<String, ClaimCoverage>,
    /// The counts, per claim area.
    pub by_area: BTreeMap<String, ClaimCoverage>,
    /// Every intent, witnessed or not, in owner and label order.
    ///
    /// The listing is unbounded because it is the report's reason to exist: an
    /// intent is a promise written down, and a promise nobody can enumerate is a
    /// promise nobody will keep.
    pub intent_sites: Vec<IntentSite>,
    /// The minted statements nothing cites, bounded by the caller's limit.
    pub uncited_claims: Vec<ClaimSite>,
}

/// The coverage figures the check report carries, without the listings.
///
/// The check report counts and does not enumerate, exactly as it counts the burn
/// families without naming them: the enumerations belong to the command whose
/// whole result they are, and a check report carrying thousands of lines of
/// advisory listing would be a worse report for every reader of it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CoverageCounts {
    /// How many covered tests the claim census considered.
    pub covered: usize,
    /// How many of them carry a claim at the standard place.
    pub claimed: usize,
    /// How many carry none.
    pub unclaimed: usize,
    /// How many statements the corpus's tests mint.
    pub mints: usize,
    /// How many citations of them stand in the corpus's tests.
    pub citations: usize,
    /// How many minted statements no sibling test cites.
    pub uncited: usize,
    /// How many intents stand in the corpus's prose.
    pub intents: usize,
    /// How many of those intents no covered test witnesses.
    pub unwitnessed: usize,
}

impl CoverageSummary {
    /// Reduce the summary to the figures the check report carries.
    #[must_use]
    pub const fn counts(&self) -> CoverageCounts {
        CoverageCounts {
            covered: self.covered,
            claimed: self.claimed,
            unclaimed: self.unclaimed,
            mints: self.mints,
            citations: self.citations,
            uncited: self.uncited,
            intents: self.intents,
            unwitnessed: self.unwitnessed,
        }
    }
}

/// Summarise the claim coverage of one analysis and one claim census.
///
/// The owner of a claim is taken from the adoption's own partition, over the
/// source the test stands in, rather than from the package name the census
/// carries. The two agree everywhere but the root package, and where they
/// disagree the partition is the one that agrees with the carrier — so a prose
/// intent and the test that witnesses it are counted under one name rather than
/// under two that happen to describe the same code.
///
/// The limit bounds the uncited listing alone; every count is taken over the
/// whole corpus, and the intents are listed entire.
#[must_use]
pub fn summarise_coverage(
    analysis: &Analysis,
    adoption: &Adoption,
    claims: &[CoveredClaim],
    covered: usize,
    limit: usize,
) -> CoverageSummary {
    let mut summary = CoverageSummary {
        covered,
        claimed: claims.len(),
        unclaimed: covered.saturating_sub(claims.len()),
        ..CoverageSummary::default()
    };

    let mut minted: BTreeMap<(String, Label), ClaimSite> = BTreeMap::new();
    let mut carried: BTreeSet<(String, Label)> = BTreeSet::new();
    let mut cited: BTreeSet<(String, Label)> = BTreeSet::new();

    for claim in claims {
        let owner = adoption
            .owner_of(claim.asset().test().location().path())
            .as_str()
            .to_owned();
        let label = claim.claim().label().clone();
        let key = (owner.clone(), label.clone());

        let _admitted = carried.insert(key.clone());

        if claim.claim().is_mint() {
            summary.mints += 1;
            let _first = minted.entry(key).or_insert_with(|| ClaimSite {
                owner,
                label,
                test: claim.asset().test().function().to_owned(),
                location: claim.asset().test().location().clone(),
            });
        } else {
            summary.citations += 1;
            let _seen = cited.insert(key);
        }
    }

    for (key, site) in &minted {
        let area = site.label.area().to_owned();

        summary
            .by_owner
            .entry(site.owner.clone())
            .or_default()
            .claims += 1;
        summary.by_area.entry(area.clone()).or_default().claims += 1;

        if cited.contains(key) {
            continue;
        }

        summary.uncited += 1;
        summary
            .by_owner
            .entry(site.owner.clone())
            .or_default()
            .uncited += 1;
        summary.by_area.entry(area).or_default().uncited += 1;
        summary.uncited_claims.push(site.clone());
    }

    for claim in claims {
        if claim.claim().is_mint() {
            continue;
        }

        let owner = adoption
            .owner_of(claim.asset().test().location().path())
            .as_str()
            .to_owned();

        summary.by_owner.entry(owner).or_default().citations += 1;
        summary
            .by_area
            .entry(claim.claim().label().area().to_owned())
            .or_default()
            .citations += 1;
    }

    admit_intents(analysis, &carried, &mut summary);

    summary.uncited_claims.truncate(limit);
    summary
}

/// Enter every claim minted in the carrier's prose, witnessed or not.
///
/// A mint standing in code is passed over for the reason the graph report passes
/// over it: the code surface is not the corpus a reviewer is reading. That is
/// the whole of the test, and it must be the surface rather than the warrant —
/// a claim minted where its test is was authored by a person too, and reading it
/// as an intent would count every witness as its own intention.
///
/// Every remaining claim-kind node stands at a head somebody wrote in a
/// document, which is what an intent is.
fn admit_intents(
    analysis: &Analysis,
    carried: &BTreeSet<(String, Label)>,
    summary: &mut CoverageSummary,
) {
    let graph = analysis.graph();

    for node in graph.node_indices() {
        let mint = &graph[node];

        if mint.surface != Surface::Document || mint.label.kind() != CLAIM_KIND {
            continue;
        }

        let owner = mint.owner.as_str().to_owned();
        let witnessed = carried.contains(&(owner.clone(), mint.label.clone()));
        let area = mint.label.area().to_owned();

        summary.intents += 1;
        summary.by_owner.entry(owner.clone()).or_default().intents += 1;
        summary.by_area.entry(area.clone()).or_default().intents += 1;

        if !witnessed {
            summary.unwitnessed += 1;
            summary
                .by_owner
                .entry(owner.clone())
                .or_default()
                .unwitnessed += 1;
            summary.by_area.entry(area).or_default().unwitnessed += 1;
        }

        summary.intent_sites.push(IntentSite {
            owner,
            label: mint.label.clone(),
            location: mint.location.clone(),
            witnessed,
        });
    }

    summary
        .intent_sites
        .sort_by(|left, right| (&left.owner, &left.label).cmp(&(&right.owner, &right.label)));
}
