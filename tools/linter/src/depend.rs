// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Dependency closure: whether an activated pair's prerequisites are activated too.
//!
//! Activating a policy for an owner activates a claim about what that verdict
//! rests on. The prerequisites are the running binary's, held in its catalog
//! beside each policy's recognizer and row codec, and they are not configurable —
//! a repository ruling selects which policies apply, and the linter defines what
//! a policy means and therefore what it presupposes
//! (´req:commandcontract:dependency-contract´).
//!
//! Checking each declared pair against its immediate edges is checking the
//! closure. A required pair that is present is itself a declared pair, so its own
//! edges are checked when the walk reaches it; a required pair that is absent is
//! named, and naming the missing pair rather than the symptom is what the
//! requirement asks for. So a pair set closed under the immediate edges is closed
//! under their transitive closure, and the two readings agree on the verdict
//! while this one reports each defect once instead of once per path that reaches
//! it.
//!
//! Satisfaction is presence and nothing more. A prerequisite whose own list is
//! nonempty still satisfies, because a tolerated defect is a debt the corpus has
//! ruled on rather than a reason to stop judging what depends on it. Absence is
//! correspondingly not a waiver: the dependent pair is what makes the
//! prerequisite applicable, so a missing one fails configuration instead of
//! quietly excusing itself.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`a_closed_pair_set_reports_nothing`] | depend | A pair set closed under the catalog's edges reports nothing, however many pairs it carries and however much its lists tolerate. Closure is a property of the declaration alone. |
//! | [`a_missing_same_owner_prerequisite_names_itself`] | depend | A same-owner prerequisite that is not activated is named as itself, of the owner whose pair required it, in the exact message the contract fixes. |
//! | [`a_missing_fixed_owner_prerequisite_names_the_root_pair`] | depend | A fixed-owner prerequisite names the root owner's pair wherever it is required from, because the reconciliation it rests on is one repository-wide artifact that no member can repair alone. |
//! | [`a_partition_naming_no_root_owner_wants_no_fixed_owner_pair`] | depend | A partition naming no root owner instantiates no fixed-owner edge at all, so the same pair set that named the root pair above names nothing. The defect to repair is a partition nobody can read a root owner out of, and reporting a missing pair of an owner that does not exist would send the reader to write a row nobody could file. |
//! | [`a_missing_cited_owner_prerequisite_carries_its_first_citation`] | depend | A cited-owner prerequisite is instantiated per registered owner the requiring owner's sources actually cite into, and the finding carries the first citation that required it, in source order. |
//! | [`several_citations_wanting_one_pair_produce_one_finding`] | depend | Several citations wanting one missing pair produce one finding rather than a drift of duplicates, and the location it carries is the earliest of them. |
//! | [`an_uncited_owner_instantiates_no_edge`] | depend | An owner the requiring owner never cites into instantiates no edge, so a permitted reach that nothing uses obliges no pair. Prospective closure over the whole permitted graph would oblige pairs no source needs. |
//! | [`presence_satisfies_and_absence_does_not_waive`] | depend | A prerequisite is satisfied by presence and nothing more, and absence of the dependent pair is not a waiver: a policy nobody activated for an owner requires nothing of that owner, while activating it requires the whole of what it rests on. |
//! | [`findings_are_ordered_by_their_identifiers`] | depend | Findings are ordered by the requiring owner, the requiring policy, the scope and the required pair, before any location — so two runs over one snapshot report the same list in the same order. |

#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};

use crate::adoption::Adoption;
#[cfg(test)]
use crate::catalogue::{Scope, catalogued};
use crate::engine::Analysis;
#[cfg(test)]
use crate::finding::Finding;
use crate::finding::Location;
#[cfg(test)]
use crate::snapshot::Pair;

/// One citation reaching from an owner's sources into another owner's corpus.
///
/// A citation naming a prefix nobody registered instantiates no requirement at
/// all: it is a defect of the citation, and inventing an owner to require
/// something of would report the wrong fault twice. Such citations are therefore
/// absent from this relation rather than carried with an empty target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedEdge {
    /// The owner whose source stands the citation.
    pub owner: String,
    /// The registered owner the citation names.
    pub target: String,
    /// Where the citation stands.
    pub location: Location,
}

/// Harvest the cited-owner relation from a completed analysis.
///
/// Only the citation's form, target prefix and location are wanted, and no
/// resolution verdict is issued from them: the harvest runs to instantiate
/// cross-owner dependencies, and a citation that will turn out to resolve
/// nowhere still names the owner whose registry it was going to be resolved
/// against.
#[must_use]
pub fn cited_edges(adoption: &Adoption, analysis: &Analysis) -> Vec<CitedEdge> {
    let mut edges = Vec::new();

    for import in analysis.imports() {
        let Some(target) = adoption.owner_of_prefix(import.prefix()) else {
            continue;
        };

        let owner = import.citing().as_str();

        if owner == target.as_str() {
            continue;
        }

        edges.push(CitedEdge {
            owner: owner.to_owned(),
            target: target.as_str().to_owned(),
            location: import.location().clone(),
        });
    }

    edges
}

/// Check every activated pair's prerequisites against the activated set.
///
/// The result is sorted by the requiring owner, the requiring policy, the scope,
/// the required owner and the required policy, before any location — so two runs
/// over one snapshot report the same list in the same order however the citations
/// happened to be harvested.
///
/// The root owner is the partition's rather than this binary's, and it is passed
/// in because a fixed-owner edge names it. A partition naming none instantiates
/// no fixed-owner edge: a repository-wide verdict wanted of an owner nobody can
/// identify names a pair nobody can activate, and the defect to repair is the
/// partition rather than the pair it would have asked for.
#[must_use]
#[cfg(test)]
pub fn retiring_verify(
    pairs: &[Pair],
    citations: &[CitedEdge],
    root_owner: Option<&str>,
) -> Vec<Finding> {
    let declared: BTreeSet<&Pair> = pairs.iter().collect();
    let cited = first_citations(citations);
    let mut findings = Vec::new();

    for pair in pairs {
        let Some(policy) = catalogued(&pair.policy) else {
            continue;
        };

        for edge in policy.dependencies {
            match edge.scope {
                Scope::SameOwner => {
                    require(
                        &declared,
                        pair,
                        edge.scope,
                        &pair.owner,
                        edge.policy,
                        None,
                        &mut findings,
                    );
                }
                Scope::FixedOwner => {
                    if let Some(root) = root_owner {
                        require(
                            &declared,
                            pair,
                            edge.scope,
                            root,
                            edge.policy,
                            None,
                            &mut findings,
                        );
                    }
                }
                Scope::CitedOwner => {
                    for ((owner, target), location) in &cited {
                        if owner != &pair.owner {
                            continue;
                        }

                        require(
                            &declared,
                            pair,
                            edge.scope,
                            target,
                            edge.policy,
                            Some(location),
                            &mut findings,
                        );
                    }
                }
            }
        }
    }

    findings.sort_by(|left, right| sort_key(left).cmp(&sort_key(right)));
    findings
}

/// The first citation, in byte-path and source order, for each owner-and-target pair.
#[cfg(test)]
fn first_citations(citations: &[CitedEdge]) -> BTreeMap<(String, String), Location> {
    let mut first: BTreeMap<(String, String), Location> = BTreeMap::new();

    for edge in citations {
        let key = (edge.owner.clone(), edge.target.clone());

        first
            .entry(key)
            .and_modify(|held| {
                if edge.location < *held {
                    *held = edge.location.clone();
                }
            })
            .or_insert_with(|| edge.location.clone());
    }

    first
}

/// Record a finding when one required pair is not activated.
#[cfg(test)]
fn require(
    declared: &BTreeSet<&Pair>,
    pair: &Pair,
    scope: Scope,
    required_owner: &str,
    required_policy: &str,
    location: Option<&Location>,
    findings: &mut Vec<Finding>,
) {
    let required = Pair::singleton(required_owner, required_policy);

    if declared.contains(&required) {
        return;
    }

    findings.push(Finding::MissingPolicyDependency {
        owner: pair.owner.clone(),
        policy: pair.policy.clone(),
        scope: scope.as_str(),
        required_owner: required.owner,
        required_policy: required.policy,
        location: location.cloned(),
    });
}

/// The five identifiers a dependency finding is ordered by.
#[cfg(test)]
fn sort_key(finding: &Finding) -> (&str, &str, &str, &str, &str) {
    match finding {
        Finding::MissingPolicyDependency {
            owner,
            policy,
            scope,
            required_owner,
            required_policy,
            ..
        } => (owner, policy, scope, required_owner, required_policy),
        _ => ("", "", "", "", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::{CitedEdge, retiring_verify as verify};
    use crate::finding::{Finding, Location};
    use crate::snapshot::Pair;

    /// The root owner an invented partition names, standing for the one a
    /// declaration derives.
    const ROOT: &str = "INDEX";

    /// One activated pair.
    fn pair(owner: &str, policy: &str) -> Pair {
        Pair::singleton(owner, policy)
    }

    /// One citation from an owner into another owner's corpus.
    fn citation(owner: &str, target: &str, path: &str, text: &str, offset: usize) -> CitedEdge {
        CitedEdge {
            owner: owner.to_owned(),
            target: target.to_owned(),
            location: Location::new(path, text, offset),
        }
    }

    /// The missing pairs a verdict names, as the message renders them.
    fn messages(findings: &[Finding]) -> Vec<String> {
        findings.iter().map(ToString::to_string).collect()
    }

    /// A pair set closed under the catalog's edges reports nothing, however many
    /// pairs it carries and however much its lists tolerate. Closure is a
    /// property of the declaration alone.
    ///
    /// ´claim:depend:a-closed-pair-set-reports-nothing´
    /// ´test:unit:a-closed-pair-set-reports-nothing´
    #[test]
    fn a_closed_pair_set_reports_nothing() {
        let pairs = vec![
            pair("INDEX", "labels.mints-well-formed"),
            pair("INDEX", "labels.mints-unique"),
            pair("INDEX", "labels.citations-local-resolve"),
        ];

        assert_eq!(verify(&pairs, &[], Some(ROOT)), Vec::new());
    }

    /// A same-owner prerequisite that is not activated is named as itself, of
    /// the owner whose pair required it, in the exact message the contract
    /// fixes.
    ///
    /// ´claim:depend:a-missing-same-owner-prerequisite-names-itself´
    /// ´test:unit:a-missing-same-owner-prerequisite-names-itself´
    #[test]
    fn a_missing_same_owner_prerequisite_names_itself() {
        let pairs = vec![pair("ASSAYER", "labels.mints-unique")];

        assert_eq!(
            messages(&verify(&pairs, &[], Some(ROOT))),
            vec![
                "policy dependency: ASSAYER : labels.mints-unique: missing same-owner pair ASSAYER : labels.mints-well-formed"
            ]
        );
    }

    /// A fixed-owner prerequisite names the root owner's pair wherever it is
    /// required from, because the reconciliation it rests on is one
    /// repository-wide artifact that no member can repair alone.
    ///
    /// ´claim:depend:a-missing-fixed-owner-prerequisite-names-the-root-pair´
    /// ´test:unit:a-missing-fixed-owner-prerequisite-names-the-root-pair´
    #[test]
    fn a_missing_fixed_owner_prerequisite_names_the_root_pair() {
        let pairs = vec![
            pair("ASSAYER", "labels.citations-layer-conform"),
            pair("ASSAYER", "labels.citations-imported-resolve"),
        ];

        let found = messages(&verify(&pairs, &[], Some(ROOT)));

        assert!(
            found.contains(
                &String::from(
                    "policy dependency: ASSAYER : labels.citations-layer-conform: missing fixed-owner pair INDEX : owners.reach-conform"
                )
            ),
            "{found:?}"
        );
    }

    /// A partition naming no root owner instantiates no fixed-owner edge at
    /// all, so the same pair set that named the root pair above names nothing.
    /// The defect to repair is a partition nobody can read a root owner out of,
    /// and reporting a missing pair of an owner that does not exist would send
    /// the reader to write a row nobody could file.
    ///
    /// ´claim:depend:a-partition-with-no-root-owner-instantiates-no-fixed-owner-edge´
    /// ´test:unit:a-partition-naming-no-root-owner-wants-no-fixed-owner-pair´
    #[test]
    fn a_partition_naming_no_root_owner_wants_no_fixed_owner_pair() {
        let pairs = vec![
            pair("ASSAYER", "labels.citations-layer-conform"),
            pair("ASSAYER", "labels.citations-imported-resolve"),
        ];

        let found = messages(&verify(&pairs, &[], None));

        assert!(
            !found.iter().any(|message| message.contains("fixed-owner")),
            "no owner is the repository's, so nothing is wanted of one: {found:?}"
        );
        assert!(
            found.iter().any(|message| message.contains("same-owner")),
            "and every other edge is instantiated exactly as it was: {found:?}"
        );
    }

    /// A cited-owner prerequisite is instantiated per registered owner the
    /// requiring owner's sources actually cite into, and the finding carries the
    /// first citation that required it, in source order.
    ///
    /// ´claim:depend:a-missing-cited-owner-prerequisite-carries-its-first-citation´
    /// ´test:unit:a-missing-cited-owner-prerequisite-carries-its-first-citation´
    #[test]
    fn a_missing_cited_owner_prerequisite_carries_its_first_citation() {
        let pairs = vec![
            pair("ASSAYER", "labels.citations-imported-resolve"),
            pair("ASSAYER", "labels.citations-import-form"),
            pair("ASSAYER", "labels.mints-well-formed"),
        ];

        let citations = vec![citation(
            "ASSAYER",
            "MUDLARK",
            "packages/assayer/a.md",
            "line one\nline two\n",
            9,
        )];

        assert_eq!(
            messages(&verify(&pairs, &citations, Some(ROOT))),
            vec![
                "policy dependency: ASSAYER : labels.citations-imported-resolve: missing cited-owner pair \
                 MUDLARK : labels.mints-unique; first required by packages/assayer/a.md:2:1"
            ]
        );
    }

    /// Several citations wanting one missing pair produce one finding rather
    /// than a drift of duplicates, and the location it carries is the earliest
    /// of them.
    ///
    /// ´claim:depend:several-citations-wanting-one-pair-produce-one-finding´
    /// ´test:unit:several-citations-wanting-one-pair-produce-one-finding´
    #[test]
    fn several_citations_wanting_one_pair_produce_one_finding() {
        let pairs = vec![
            pair("ASSAYER", "labels.citations-imported-resolve"),
            pair("ASSAYER", "labels.citations-import-form"),
            pair("ASSAYER", "labels.mints-well-formed"),
        ];

        let text = "one\ntwo\nthree\n";
        let citations = vec![
            citation("ASSAYER", "MUDLARK", "packages/assayer/b.md", text, 8),
            citation("ASSAYER", "MUDLARK", "packages/assayer/a.md", text, 4),
            citation("ASSAYER", "MUDLARK", "packages/assayer/b.md", text, 0),
        ];

        let found = verify(&pairs, &citations, Some(ROOT));

        assert_eq!(found.len(), 1);
        assert!(
            messages(&found)[0].ends_with("first required by packages/assayer/a.md:2:1"),
            "{found:?}"
        );
    }

    /// An owner the requiring owner never cites into instantiates no edge, so a
    /// permitted reach that nothing uses obliges no pair. Prospective closure
    /// over the whole permitted graph would oblige pairs no source needs.
    ///
    /// ´claim:depend:an-uncited-owner-instantiates-no-edge´
    /// ´test:unit:an-uncited-owner-instantiates-no-edge´
    #[test]
    fn an_uncited_owner_instantiates_no_edge() {
        let pairs = vec![
            pair("ASSAYER", "labels.citations-imported-resolve"),
            pair("ASSAYER", "labels.citations-import-form"),
            pair("ASSAYER", "labels.mints-well-formed"),
        ];

        assert_eq!(verify(&pairs, &[], Some(ROOT)), Vec::new());

        let citations = vec![citation(
            "MUDLARK",
            "SENTINEL",
            "packages/mudlark/a.md",
            "x",
            0,
        )];

        assert_eq!(verify(&pairs, &citations, Some(ROOT)), Vec::new());
    }

    /// A prerequisite is satisfied by presence and nothing more, and absence of
    /// the dependent pair is not a waiver: a policy nobody activated for an
    /// owner requires nothing of that owner, while activating it requires the
    /// whole of what it rests on.
    ///
    /// ´claim:depend:presence-satisfies-and-absence-does-not-waive´
    /// ´test:unit:presence-satisfies-and-absence-does-not-waive´
    #[test]
    fn presence_satisfies_and_absence_does_not_waive() {
        let unactivated = vec![
            pair("MUDLARK", "profile.tests-conform"),
            pair("MUDLARK", "labels.mints-well-formed"),
        ];
        assert_eq!(verify(&unactivated, &[], Some(ROOT)), Vec::new());

        let activated = vec![pair("MUDLARK", "projection.test-indexes-current")];
        assert_eq!(
            messages(&verify(&activated, &[], Some(ROOT))),
            vec![
                "policy dependency: MUDLARK : projection.test-indexes-current: missing same-owner pair MUDLARK : profile.tests-conform"
            ]
        );

        let both = vec![
            pair("MUDLARK", "projection.test-indexes-current"),
            pair("MUDLARK", "profile.tests-conform"),
            pair("MUDLARK", "labels.mints-well-formed"),
        ];
        assert_eq!(verify(&both, &[], Some(ROOT)), Vec::new());
    }

    /// Findings are ordered by the requiring owner, the requiring policy, the
    /// scope and the required pair, before any location — so two runs over one
    /// snapshot report the same list in the same order.
    ///
    /// ´claim:depend:findings-are-ordered-by-their-identifiers´
    /// ´test:unit:findings-are-ordered-by-their-identifiers´
    #[test]
    fn findings_are_ordered_by_their_identifiers() {
        let pairs = vec![
            pair("MUDLARK", "labels.heads-conform"),
            pair("ASSAYER", "labels.mints-unique"),
            pair("ASSAYER", "labels.heads-conform"),
        ];

        let found = messages(&verify(&pairs, &[], Some(ROOT)));

        assert_eq!(
            found,
            vec![
                "policy dependency: ASSAYER : labels.heads-conform: missing same-owner pair ASSAYER : labels.mints-kind-conform",
                "policy dependency: ASSAYER : labels.mints-unique: missing same-owner pair ASSAYER : labels.mints-well-formed",
                "policy dependency: MUDLARK : labels.heads-conform: missing same-owner pair MUDLARK : labels.mints-kind-conform",
            ]
        );
    }
}
