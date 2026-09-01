// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Reports over the reference graph.
//!
//! The check decides whether the corpus is in good standing, and says nothing
//! else. Review wants the other questions: which environments nothing cites,
//! which are cited by everything, how the corpus is spread across its owners and
//! areas, and who cites a given label. None of those is a rule, and none of them
//! can be, which is why they are a report rather than more findings.
//!
//! # Orphans are advisory, and must be
//!
//! A mint that nothing cites is worth seeing during an outline review and is not
//! a defect. Whole genres of environment are minted precisely so that something
//! else can be said about them later — a backlog entry is minted to be flipped in
//! place and cited only when another entry depends on it, and a register item is
//! minted to be pinned. A corpus that failed on uncited mints would either be
//! wrong or would push authors into writing citations that carry no meaning. The
//! report therefore lists orphans and forms no judgment about them, and the
//! command's exit status does not move when the list is long.
//!
//! # Dangling citations are the check's business
//!
//! Citations that resolve nowhere are listed here too, because a reviewer wants
//! them in the same place as everything else, but they are listed as what the
//! check already found rather than re-derived. The invariant named
//! (ADR-T-014, A calculus of documentation and source labels) is a rule of the check, and having
//! the report restate it as a second gate would give one rule two verdicts.
//!
//! # The code surface is not listed
//!
//! The registries carry every mint standing in code beside the ones the
//! documents mint, so that prose can cite a test, a notice, or a claim rather
//! than remember it. Those nodes are deliberately absent from every listing and
//! every count here, and the test is the surface rather than the warrant: an
//! authored claim standing where its test is would swamp this report exactly as
//! a derived label would.
//!
//! The reason is arithmetic: the code surface outnumbers the prose several times
//! over, and almost none of it is cited, so a report that admitted it would be
//! an orphan listing thousands long with the handful a reviewer wanted buried
//! somewhere inside. What that surface does support is a coverage view of its
//! own — how many tests each environment cites, and which cite none — and that
//! is a view to be added, not a change to these.
//!
//! A reverse lookup is the exception, and is not an exception to the rule. It
//! reports the mints of one label the caller named, so nothing can be buried by
//! it; asking after a notice or a test label and being told it is minted nowhere
//! would simply be false.
//!
//! The graph is petgraph's, used directly: in-degree is the citation count of a
//! mint, and the incoming edges of a node are the citations of it.
//!
use std::collections::BTreeMap;

use petgraph::Direction;
use petgraph::visit::EdgeRef;
use serde::Serialize;

use crate::engine::{Analysis, ReferenceGraph, Surface};
use crate::finding::{Finding, Location};
use crate::label::Label;
use crate::occurrence::Form;

/// The finding codes that are a citation failing to reach a mint.
///
/// The invariant of total resolution enumerates the ways a participating
/// citation fails to be the conclusion of exactly one rule — unknown owners,
/// unresolved citations, non-parenthesized imports, and bracket-free cross-owner
/// tokens (´[ORCHESTRATION-inv:labels:total-resolution]´) — and the import rule adds the
/// fifth by a side condition, a self-qualified import being underivable
/// (´[ORCHESTRATION-inf:labels:imported-citation]´). The value is that enumeration under
/// the codes this checker reports it by, which is why it is a list of the check's
/// own verdicts rather than a second derivation: the report restates what the
/// check found, so that one rule keeps one verdict.
///
/// ´const:indexlinter:resolution-failure-codes´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:resolution-failure-codes-form-x44e307eb´
const DANGLING_CODES: &[&str] = &[
    "unresolved_citation",
    "unresolved_citation_wanting_import",
    "unregistered_prefix",
    "self_qualified_import",
    "non_parenthesized_import",
];

/// How many of the most-cited mints the report lists by default.
///
/// The listing is advisory and forms no judgment, so nothing about the corpus
/// fixes where it should stop: the bound is a reading length for an outline
/// review, and a reviewer wanting more passes a larger one. No record states
/// this length, and the honest reading is that one is owed rather than absent.
///
/// TODO ´todo:code:record-the-reading-length-the-default´: record the reading length the default hub listing is cut to.
///
/// ´const:indexlinter:hub-listing-bound´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:indexlinter:hub-listing-bound-count-10´
pub const DEFAULT_HUBS: usize = 10;

/// One mint, as the report names it.
#[derive(Debug, Clone, Serialize)]
pub struct MintSite {
    /// The owner whose registry holds the mint.
    pub owner: String,
    /// The minted label.
    pub label: Label,
    /// Where the minting occurrence stands.
    pub location: Location,
    /// How many resolved citations reach it.
    pub citations: usize,
}

/// One citation of a label, as a reverse lookup reports it.
#[derive(Debug, Clone, Serialize)]
pub struct CitationSite {
    /// The owner the citing environment belongs to.
    pub owner: String,
    /// The environment the citation stands in.
    pub from: Label,
    /// Whether the citation crossed an ownership boundary.
    pub imported: bool,
    /// Where the citation stands.
    pub location: Location,
}

/// The counts for one cell of a partition of the corpus.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Coverage {
    /// How many mints fall in this cell.
    pub mints: usize,
    /// How many of them at least one citation reaches.
    pub cited: usize,
    /// How many of them no citation reaches.
    pub orphans: usize,
    /// How many resolved citations stand inside this cell's environments.
    pub citations: usize,
}

impl Coverage {
    /// Record one mint and the citations reaching it.
    const fn admit(&mut self, incoming: usize, outgoing: usize) {
        self.mints += 1;
        self.citations += outgoing;

        if incoming == 0 {
            self.orphans += 1;
        } else {
            self.cited += 1;
        }
    }
}

/// What a reverse lookup found.
#[derive(Debug, Clone, Serialize)]
pub struct Reverse {
    /// The label asked about.
    pub label: String,
    /// Every mint of that label, one per owner that mints it.
    pub mints: Vec<MintSite>,
    /// Every resolved citation reaching those mints.
    pub citers: Vec<CitationSite>,
}

/// What the report says about the reference graph.
#[derive(Debug, Clone, Serialize)]
pub struct GraphSummary {
    /// How many mints stand in the carrier.
    pub mints: usize,
    /// How many resolved citations the graph carries as edges.
    pub citations: usize,
    /// Mints no citation reaches, in owner and label order.
    pub orphans: Vec<MintSite>,
    /// The most-cited mints, most cited first.
    pub hubs: Vec<MintSite>,
    /// The counts, per owner.
    pub by_owner: BTreeMap<String, Coverage>,
    /// The counts, per area.
    pub by_area: BTreeMap<String, Coverage>,
    /// The reverse lookup, when one was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse: Option<Reverse>,
}

/// Summarise the reference graph of one analysis.
///
/// The label, when given, is the one a reverse lookup is taken over; the number
/// of hubs bounds the most-cited listing, so a caller that wants none asks for
/// zero rather than for a flag.
#[must_use]
pub fn summarise(analysis: &Analysis, label: Option<&Label>, hubs: usize) -> GraphSummary {
    let graph = analysis.graph();
    let sites = mint_sites(graph);
    let carried: Vec<&MintSite> = graph
        .node_indices()
        .zip(&sites)
        .filter(|(node, _site)| graph[*node].surface == Surface::Document)
        .map(|(_node, site)| site)
        .collect();

    let mut by_owner: BTreeMap<String, Coverage> = BTreeMap::new();
    let mut by_area: BTreeMap<String, Coverage> = BTreeMap::new();

    for (node, site) in graph.node_indices().zip(&sites) {
        if graph[node].surface != Surface::Document {
            continue;
        }

        let outgoing = graph.edges_directed(node, Direction::Outgoing).count();

        by_owner
            .entry(site.owner.clone())
            .or_default()
            .admit(site.citations, outgoing);
        by_area
            .entry(site.label.area().to_owned())
            .or_default()
            .admit(site.citations, outgoing);
    }

    let mut orphans: Vec<MintSite> = carried
        .iter()
        .filter(|site| site.citations == 0)
        .map(|site| (*site).clone())
        .collect();
    orphans.sort_by(|left, right| (&left.owner, &left.label).cmp(&(&right.owner, &right.label)));

    let mut ranked: Vec<MintSite> = carried
        .iter()
        .filter(|site| site.citations > 0)
        .map(|site| (*site).clone())
        .collect();
    ranked.sort_by(|left, right| {
        right
            .citations
            .cmp(&left.citations)
            .then_with(|| (&left.owner, &left.label).cmp(&(&right.owner, &right.label)))
    });
    ranked.truncate(hubs);

    GraphSummary {
        mints: carried.len(),
        citations: graph.edge_count(),
        orphans,
        hubs: ranked,
        by_owner,
        by_area,
        reverse: label.map(|label| reverse_lookup(graph, &sites, label)),
    }
}

/// Every finding of an analysis that is a citation reaching no mint.
#[must_use]
pub fn dangling(analysis: &Analysis) -> Vec<Finding> {
    analysis
        .findings()
        .iter()
        .filter(|finding| DANGLING_CODES.contains(&finding.code()))
        .cloned()
        .collect()
}

/// Read every node of the graph as a mint site, in node order.
fn mint_sites(graph: &ReferenceGraph) -> Vec<MintSite> {
    graph
        .node_indices()
        .map(|node| {
            let mint = &graph[node];

            MintSite {
                owner: mint.owner.as_str().to_owned(),
                label: mint.label.clone(),
                location: mint.location.clone(),
                citations: graph.edges_directed(node, Direction::Incoming).count(),
            }
        })
        .collect()
}

/// Take a reverse lookup over every mint of one label.
///
/// A label may be minted once per owner, so the lookup is over a set of mints
/// rather than a single one; reporting them together is what lets a reader see
/// that two owners mint the name before concluding anything about who cites it.
fn reverse_lookup(graph: &ReferenceGraph, sites: &[MintSite], label: &Label) -> Reverse {
    let mut mints = Vec::new();
    let mut citers = Vec::new();

    for (node, site) in graph.node_indices().zip(sites) {
        if &site.label != label {
            continue;
        }

        mints.push(site.clone());

        for edge in graph.edges_directed(node, Direction::Incoming) {
            let from = &graph[edge.source()];

            citers.push(CitationSite {
                owner: from.owner.as_str().to_owned(),
                from: from.label.clone(),
                imported: matches!(edge.weight().form, Form::ImportedCitation { .. }),
                location: edge.weight().location.clone(),
            });
        }
    }

    citers.sort_by(|left, right| left.location.cmp(&right.location));

    Reverse {
        label: label.to_string(),
        mints,
        citers,
    }
}
