// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Subscription routing for the projection family: which owners' shares a
//! projection policy reaches.
//!
//! Activation is a repository ruling and lives in the declared surface. A
//! projection policy an owner has activated governs that owner's share; an
//! owner that has not activated it is neither walked, written, nor censused by
//! it — non-applicability rather than a silent exemption. The walk over
//! workspace members is how a projection finds candidate artifacts, and
//! membership alone was never the ruling: the declaration is.
//!
//! Attribution flows from the declared partition and never from compiled path
//! knowledge. The row that accounts a path names the owner whose subscription
//! is consulted, exactly as the partition verdict attributes the path. A path
//! no row accounts, and a path more than one accounts, is not governed here:
//! the partition verdict has already named that defect, and an attribution
//! invented beside it would be the fallback the declared relation exists to
//! remove.
//!
//! A surface that activates no pair for a policy has ruled that policy inactive
//! everywhere, which is the same ruling an owner's missing row states for its
//! own share. No routing view exists without a declared partition.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`a_subscribed_owners_share_is_governed`] | subscribe | A path the partition attributes to an owner activating the policy is governed, so a subscription reaches exactly the shares whose owners asked for the projection. |
//! | [`an_unsubscribed_owners_share_is_not_governed`] | subscribe | A path the partition attributes to an owner without the activation is not governed, so an owner that did not subscribe is neither walked, written, nor censused by the projection. |
//! | [`an_unaccounted_path_is_not_governed`] | subscribe | A path no partition row accounts is not governed, because attribution is the declared relation's to state and the partition verdict already reports the defect. |
//! | [`a_multiply_accounted_path_is_not_governed`] | subscribe | A path more than one row accounts is not governed, because choosing between the rows would be the priority rule the declared partition deliberately does not have. |
//! | [`an_instanced_pair_does_not_subscribe`] | subscribe | A pair carrying a set entry does not activate a singleton projection policy, so a family deployment cannot stand in for the policy's own activation. |
//! | [`another_policys_activation_does_not_subscribe`] | subscribe | An owner's activation of one projection policy does not subscribe it to another, so the three family members are consulted one by one. |
//! | [`planned_activation_matches_the_current_constructor`] | plan | The compiled activation projection gives the same routing answers as the constructor it replaces, including paths absent from the materialized corpus and paths the declared partition cannot attribute. |

use std::collections::BTreeSet;
use std::path::Path;

use crate::pattern::BytePath;
use crate::snapshot::OwnerRow;

/// The activation the in-file test index projection is consulted at.
pub const TEST_INDEXES_POLICY: &str = "projection.test-indexes-current";

/// The activation the per-folder test matrix projection is consulted at.
pub const TEST_MATRICES_POLICY: &str = "projection.test-matrices-current";

/// The activation the constant pin projection is consulted at.
pub const CONSTANT_PINS_POLICY: &str = "projection.constant-pins-current";

/// Which owners' shares one projection policy reaches.
///
/// Built once per policy per run, and consulted per candidate artifact path.
/// The three projection policies are consulted one by one, because the
/// catalogue permits an owner to activate them separately even where a corpus
/// grants all three together.
#[derive(Debug)]
pub struct Subscription<'a> {
    /// The declared routing.
    routed: Routed<'a>,
}

/// The declared surface a subscription consults: the partition that attributes
/// a path, and the owners whose activations carry the policy.
#[derive(Debug)]
struct Routed<'a> {
    /// The declared partition rows, exactly as the snapshot carries them.
    partitions: &'a [OwnerRow],
    /// The owners activating the policy through a singleton pair.
    owners: Option<&'a BTreeSet<String>>,
}

impl<'a> Subscription<'a> {
    pub(crate) const fn planned(
        partitions: &'a [OwnerRow],
        owners: Option<&'a BTreeSet<String>>,
    ) -> Self {
        Self {
            routed: Routed { partitions, owners },
        }
    }

    /// An explicit fictional declaration governing every test-fixture path.
    ///
    /// # Panics
    ///
    /// Panics only if the hard-coded wildcard fails to parse, which the test
    /// fixture cannot arrange.
    #[cfg(test)]
    #[must_use]
    pub fn fictional_all() -> Subscription<'static> {
        use std::sync::OnceLock;

        static PARTITIONS: OnceLock<Vec<OwnerRow>> = OnceLock::new();
        static OWNERS: OnceLock<BTreeSet<String>> = OnceLock::new();

        let partitions = PARTITIONS.get_or_init(|| {
            vec![OwnerRow {
                name: String::from("fictional-root"),
                owner: String::from("INDEX"),
                pattern: crate::declaration::AbnfPattern::parse("*VCHAR")
                    .expect("the wildcard pattern parses"),
            }]
        });
        let owners = OWNERS.get_or_init(|| BTreeSet::from([String::from("INDEX")]));

        Subscription::planned(partitions, Some(owners))
    }

    /// Whether the policy governs an artifact standing at a repository-relative
    /// path.
    ///
    /// The path is attributed by the declared partition: exactly one row must
    /// account it, and the subscription consulted is that row's owner's. A
    /// path the partition cannot attribute — none of the rows account it, or
    /// more than one does — is not governed, because the partition verdict has
    /// already named the defect and no fallback attribution is invented here.
    #[must_use]
    pub fn governs(&self, path: &Path) -> bool {
        let routed = &self.routed;

        let Ok(candidate) = BytePath::from_bytes(path.as_os_str().as_encoded_bytes().to_vec())
        else {
            return false;
        };

        let mut rows = routed
            .partitions
            .iter()
            .filter(|row| row.pattern.admits_path(&candidate));

        match (rows.next(), rows.next()) {
            (Some(row), None) => routed
                .owners
                .is_some_and(|owners| owners.contains(row.owner.as_str())),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use crate::declaration::AbnfPattern;
    use crate::pattern::BytePath;
    use crate::plan::ActivationPlan;
    use crate::snapshot::{OwnerRow, Pair};

    /// The projection policy the fixtures subscribe owners to.
    const POLICY: &str = "projection.test-matrices-current";

    /// A partition row accounting one package directory to an owner.
    fn row(name: &str, owner: &str, directory: &str) -> OwnerRow {
        OwnerRow {
            name: name.to_owned(),
            owner: owner.to_owned(),
            pattern: AbnfPattern::parse(format!("%s\"{directory}\" [ \"/\" *VCHAR ]"))
                .expect("a well-formed pattern"),
        }
    }

    /// The two-owner fixture partition: one subscribed owner, one not.
    fn partitions() -> Vec<OwnerRow> {
        vec![
            row("orchard-package", "ORCHARD", "packages/orchard"),
            row("quince-package", "QUINCE", "packages/quince"),
        ]
    }

    /// The fixture activations: ORCHARD subscribes, QUINCE does not.
    fn pairs() -> Vec<Pair> {
        vec![
            Pair::singleton("ORCHARD", POLICY),
            Pair::singleton("QUINCE", "legacy.todos"),
        ]
    }

    /// A path the partition attributes to an owner activating the policy is
    /// governed, so a subscription reaches exactly the shares whose owners
    /// asked for the projection.
    ///
    /// ´claim:subscribe:a-subscribed-owners-share-is-governed´
    /// ´test:unit:a-subscribed-owners-share-is-governed´
    #[test]
    fn a_subscribed_owners_share_is_governed() {
        let (partitions, pairs) = (partitions(), pairs());
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription(POLICY);

        assert!(subscription.governs(Path::new("packages/orchard/src/README.md")));
        assert!(subscription.governs(Path::new("packages/orchard/tests/walk.rs")));
    }

    /// A path the partition attributes to an owner without the activation is
    /// not governed, so an owner that did not subscribe is neither walked,
    /// written, nor censused by the projection.
    ///
    /// ´claim:subscribe:an-unsubscribed-owners-share-is-not-governed´
    /// ´test:unit:an-unsubscribed-owners-share-is-not-governed´
    #[test]
    fn an_unsubscribed_owners_share_is_not_governed() {
        let (partitions, pairs) = (partitions(), pairs());
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription(POLICY);

        assert!(!subscription.governs(Path::new("packages/quince/src/README.md")));
        assert!(!subscription.governs(Path::new("packages/quince/tests/walk.rs")));
    }

    /// A path no partition row accounts is not governed, because attribution
    /// is the declared relation's to state and the partition verdict already
    /// reports the defect.
    ///
    /// ´claim:subscribe:an-unaccounted-path-is-not-governed´
    /// ´test:unit:an-unaccounted-path-is-not-governed´
    #[test]
    fn an_unaccounted_path_is_not_governed() {
        let (partitions, pairs) = (partitions(), pairs());
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription(POLICY);

        assert!(!subscription.governs(Path::new("packages/medlar/src/README.md")));
    }

    /// A path more than one row accounts is not governed, because choosing
    /// between the rows would be the priority rule the declared partition
    /// deliberately does not have.
    ///
    /// ´claim:subscribe:a-multiply-accounted-path-is-not-governed´
    /// ´test:unit:a-multiply-accounted-path-is-not-governed´
    #[test]
    fn a_multiply_accounted_path_is_not_governed() {
        let mut partitions = partitions();
        partitions.push(row("orchard-again", "ORCHARD", "packages/orchard"));
        let pairs = pairs();
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription(POLICY);

        assert!(!subscription.governs(Path::new("packages/orchard/src/README.md")));
    }

    /// A pair carrying a set entry does not activate a singleton projection
    /// policy, so a family deployment cannot stand in for the policy's own
    /// activation.
    ///
    /// ´claim:subscribe:an-instanced-pair-does-not-subscribe´
    /// ´test:unit:an-instanced-pair-does-not-subscribe´
    #[test]
    fn an_instanced_pair_does_not_subscribe() {
        let partitions = partitions();
        let pairs = vec![Pair {
            owner: "QUINCE".to_owned(),
            policy: POLICY.to_owned(),
            family: Some("orchard-run".to_owned()),
        }];
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription(POLICY);

        assert!(!subscription.governs(Path::new("packages/quince/src/README.md")));
    }

    /// An owner's activation of one projection policy does not subscribe it to
    /// another, so the three family members are consulted one by one.
    ///
    /// ´claim:subscribe:another-policys-activation-does-not-subscribe´
    /// ´test:unit:another-policys-activation-does-not-subscribe´
    #[test]
    fn another_policys_activation_does_not_subscribe() {
        let (partitions, pairs) = (partitions(), pairs());
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let subscription = activations.subscription("projection.test-indexes-current");

        assert!(!subscription.governs(Path::new("packages/orchard/src/lib.rs")));
    }

    fn retiring_governs(
        partitions: &[OwnerRow],
        pairs: &[Pair],
        policy: &str,
        path: &Path,
    ) -> bool {
        let Ok(candidate) = BytePath::from_bytes(path.as_os_str().as_encoded_bytes().to_vec())
        else {
            return false;
        };
        let owners: BTreeSet<&str> = pairs
            .iter()
            .filter(|pair| pair.policy == policy && pair.family.is_none())
            .map(|pair| pair.owner.as_str())
            .collect();
        let mut rows = partitions
            .iter()
            .filter(|row| row.pattern.admits_path(&candidate));

        matches!((rows.next(), rows.next()), (Some(row), None) if owners.contains(row.owner.as_str()))
    }

    /// The compiled activation projection gives the same routing answers as
    /// the constructor it replaces, including paths absent from the materialized
    /// corpus and paths the declared partition cannot attribute.
    ///
    /// ´claim:plan:activation-agrees-with-retiring-constructor´
    /// ´test:unit:planned-activation-matches-the-current-constructor´
    #[test]
    fn planned_activation_matches_the_current_constructor() {
        let mut partitions = partitions();
        partitions.push(row("orchard-again", "ORCHARD", "packages/orchard/overlap"));
        let pairs = pairs();
        let activations = ActivationPlan::from_parts(&partitions, &pairs);
        let planned = activations.subscription(POLICY);
        let candidates = [
            Path::new("packages/orchard/src/README.md"),
            Path::new("packages/quince/src/README.md"),
            Path::new("packages/medlar/src/README.md"),
            Path::new("packages/orchard/overlap/README.md"),
        ];

        for candidate in candidates {
            assert_eq!(
                planned.governs(candidate),
                retiring_governs(&partitions, &pairs, POLICY, candidate),
                "routing of {candidate:?}"
            );
        }
    }
}
