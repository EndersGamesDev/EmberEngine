// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Owner-name reconciliation: the derivation rule, and the two directions it is
//! held in.
//!
//! A participating owner's spelling is the deterministic projection of its crate
//! name rather than an independently chosen identity, and the projection is one
//! rule: strip the leading project namespace where it stands, remove the
//! hyphens, uppercase the remainder. Writing the spelling by hand beside the
//! crate name was the alternative, and it is the arrangement in which a table and
//! a tree drift apart quietly — the table stays syntactically perfect while
//! naming a crate nobody builds.
//!
//! # The reconciliation runs both ways, and neither way alone is the verdict
//!
//! One direction catches the crate the roster forgot: a workspace member whose
//! derived spelling is nobody's registered participating owner is a corpus of
//! prose with no owner to attribute it to. The other catches the roster entry
//! nothing answers for: a participating registered owner that neither a
//! discovered member nor a declared unbuilt entry accounts for is a name the
//! repository has stopped being able to explain.
//!
//! Holding only the first would let the roster accumulate owners forever; holding
//! only the second would let a new package join the tree and own nothing. So both
//! run, and the finding says which direction failed, because the two have
//! different repairs — one adds a registration, the other removes one or admits
//! the crate is not built.
//!
//! # Manifest absence is declared, because discovery cannot tell it from decay
//!
//! A member with no manifest is indistinguishable, from the outside, from a
//! member whose manifest was deleted or moved by accident. Discovery therefore
//! cannot decide which it met, and the owner spelling is not uniquely reversible
//! to a crate name, so guessing the crate name back out of the owner is not
//! available either. The intentional case is declared with both fields, and an
//! entry leaves in the commit that gives its crate a manifest.
//!
//! That last sentence is a rule the program enforces rather than a hope: an entry
//! declaring a crate the workspace does in fact build is reported, because it is
//! a row that has outlived the absence it recorded.
//!
//! # The identity is structured, so the codec is a digest
//!
//! These defects are relationships between a crate, an owner, and a directory
//! rather than occurrences inside a path. Either path codec would alias two
//! distinct defects that happen to concern one file — a collision and a missing
//! registration reported against the same package would become one tolerated row
//! — so the identity is a digest of the defect's own fields
//! (ADR-L-020, The migration disciplines).
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`derives_an_owner_by_stripping_removing_and_uppercasing`] | roster | An owner's spelling is the projection of its crate name by one rule — strip the leading namespace where it stands, remove the hyphens, uppercase the rest — and a name that does not carry the namespace keeps the whole of itself. A name projecting onto nothing the calculus would accept derives nothing rather than a spelling that would be refused later. |
//! | [`reconciles_a_built_member_and_an_unbuilt_entry_cleanly`] | roster | A built member and a declared unbuilt entry each account for one participating owner, and a roster explained in both directions reports nothing. The two sources of a member are equal before the reconciliation: what differs is where the crate name came from, not what it owes. |
//! | [`reports_a_member_missing_its_participating_registration`] | roster | A member deriving a spelling no participating owner is registered under is reported: its prose has no owner to be attributed to, and the repair is a registration rather than a rename. A registered owner that activates no pair is outside the verdict entirely, so a partitioned nonparticipant neither explains a crate nor demands one. |
//! | [`reports_an_unexplained_participating_owner`] | roster | A participating owner that neither a discovered member nor a declared entry accounts for is reported: the repository has a name it can no longer explain, and holding only the other direction would let such names accumulate forever. |
//! | [`reports_a_collision_between_two_derived_spellings`] | roster | Two distinct crate names projecting onto one spelling collide, and the collision is reported against the names rather than against the roster: one spelling cannot attribute two corpora, so no registration repairs it and the reconciliation does not go on to ask whether that spelling is registered. Removing the hyphens is what makes the projection non-injective, so two names differing only in where they break collide. |
//! | [`reads_one_name_from_two_sources_as_one_claim`] | roster | One crate name reaching a spelling from both the workspace and the declared entries is not a collision with itself. The duplication is the outlived-entry defect and is reported once, there — a self-collision would name one crate twice and offer a rename nobody can perform. |
//! | [`reports_an_unbuilt_entry_the_workspace_builds`] | roster | A declared unbuilt entry whose crate the workspace does build is reported: the row has outlived the absence it recorded. Leaving it standing would let the manifest disappear again later with nothing to notice, since the entry would go on explaining the owner either way. |
//! | [`reports_an_underivable_crate_name`] | roster | A crate name projecting onto nothing well-formed is reported as itself rather than passed over, so a member can never go missing from the partition by being unnameable. An empty roster and an empty workspace reconcile cleanly, because absence of everything is a corpus with no owners rather than a defect. |
//! | [`the_owner_names_program_fingerprints_its_defects`] | roster | The reconciliation declares the fingerprint codec, because its defects are relationships between a crate, an owner and a directory rather than occurrences inside a path. Either path codec would alias two distinct defects concerning one package into one tolerated row. |

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::catalogue::Codec;
use crate::label::Prefix;
use crate::workspace::Package;

/// Derive an owner's registered spelling from a crate name.
///
/// The namespace is stripped only where it actually leads the name, so a crate
/// that does not carry it keeps its whole name — which is what lets one rule
/// serve a workspace whose members are not all of one project. A name yielding
/// no well-formed spelling — one that is entirely hyphens, or that begins with a
/// digit — derives nothing, and the caller decides what to make of that rather
/// than receiving a spelling the calculus would refuse anyway.
#[must_use]
pub fn derive_owner(namespace: &str, crate_name: &str) -> Option<Prefix> {
    let stem = crate_name.strip_prefix(namespace).unwrap_or(crate_name);
    let letters: String = stem
        .chars()
        .filter(|character| *character != '-')
        .collect::<String>()
        .to_ascii_uppercase();

    Prefix::parse(&letters)
}

/// One participating member the workspace does not build.
///
/// Both fields are needed and neither is derivable from the other: the crate
/// name is what the derivation rule consumes, and the directory is what the
/// partition attributes the member's prose by. A row carrying only the owner
/// would be a spelling nothing could check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnbuiltMember {
    crate_name: String,
    directory: PathBuf,
}

impl UnbuiltMember {
    /// Declare an unbuilt member by its crate name and its directory.
    #[must_use]
    pub fn new(crate_name: impl Into<String>, directory: impl Into<PathBuf>) -> Self {
        Self {
            crate_name: crate_name.into(),
            directory: directory.into(),
        }
    }

    /// The crate name the derivation rule reads.
    #[must_use]
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// The directory the member's prose stands under.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

/// What a reconciliation found the roster and the workspace failing to be.
///
/// Five defects rather than one, because the five have five different repairs
/// and a single "roster disagrees" finding would tell a reader nothing about
/// which to make.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RosterDefect {
    /// A crate name yields no well-formed owner spelling.
    UnderivableName {
        /// The crate name the rule could not project.
        crate_name: String,
    },
    /// Two crate names project onto one owner spelling.
    ///
    /// A collision is a defect of the names rather than of the roster: one
    /// spelling cannot attribute two corpora, and no registration repairs it.
    Collision {
        /// The spelling both names reach.
        owner: String,
        /// The colliding crate names, ordered.
        crate_names: Vec<String>,
    },
    /// A member derives a spelling no participating owner is registered under.
    MissingRegistration {
        /// The crate whose owner is unregistered or nonparticipating.
        crate_name: String,
        /// The spelling the rule derived for it.
        owner: String,
    },
    /// A participating owner no member and no declared entry accounts for.
    UnexplainedOwner {
        /// The registered spelling nothing explains.
        owner: String,
    },
    /// A declared unbuilt entry whose crate the workspace does build.
    ///
    /// The row has outlived the absence it recorded, and leaving it standing
    /// would let a real manifest disappear later without anything noticing.
    BuiltUnbuiltEntry {
        /// The crate declared unbuilt and discovered anyway.
        crate_name: String,
        /// The directory the entry declared for it.
        directory: PathBuf,
    },
}

/// The parameters of the owner-name reconciliation.
///
/// Only two values, and the second empties itself: the namespace the derivation
/// strips, and the members that participate without a manifest. Built members
/// contribute nothing here because their names and directories come from their
/// manifests, and a value that could be read from the tree is a value a
/// declaration must not also carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerNames {
    namespace: String,
    unbuilt: Vec<UnbuiltMember>,
}

impl OwnerNames {
    /// Instantiate the program with a namespace and the unbuilt entries.
    #[must_use]
    pub fn new(
        namespace: impl Into<String>,
        unbuilt: impl IntoIterator<Item = UnbuiltMember>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            unbuilt: unbuilt.into_iter().collect(),
        }
    }

    /// The identity form this program's tolerated violations are written in.
    #[must_use]
    pub const fn codec() -> Codec {
        Codec::Fingerprint
    }

    /// The owner spelling this instance derives for a crate name.
    #[must_use]
    pub fn derive(&self, crate_name: &str) -> Option<Prefix> {
        derive_owner(&self.namespace, crate_name)
    }

    /// The members this instance declares participating without a manifest.
    #[must_use]
    pub fn unbuilt(&self) -> &[UnbuiltMember] {
        &self.unbuilt
    }

    /// Reconcile discovered members and declared entries against the roster.
    ///
    /// `participating` is the set of registered owner spellings that activate at
    /// least one policy or profile pair. A registered owner with no activated
    /// pair makes no crate claim and is outside this verdict entirely — that is
    /// non-applicability rather than a silent exemption, and it is what lets an
    /// archived or vendored tree be partitioned as an owner without being
    /// answerable to a crate name it never had.
    ///
    /// Defects come back ordered, so two runs over one repository produce one
    /// report and a register written from either compares equal.
    #[must_use]
    pub fn reconcile(&self, discovered: &[Package], participating: &[String]) -> Vec<RosterDefect> {
        let mut defects = Vec::new();
        let mut derived: BTreeMap<String, Vec<String>> = BTreeMap::new();

        let claims = discovered
            .iter()
            .map(|package| (package.name(), package.directory()))
            .chain(
                self.unbuilt
                    .iter()
                    .map(|entry| (entry.crate_name(), entry.directory())),
            );

        for (crate_name, _directory) in claims {
            match self.derive(crate_name) {
                Some(owner) => derived
                    .entry(owner.as_str().to_owned())
                    .or_default()
                    .push(crate_name.to_owned()),
                None => defects.push(RosterDefect::UnderivableName {
                    crate_name: crate_name.to_owned(),
                }),
            }
        }

        for (owner, crate_names) in &derived {
            // A collision is two *distinct* names reaching one spelling. One
            // name reaching it from both the workspace and the declared entries
            // is the outlived-entry defect below, reported once and there.
            let mut distinct = crate_names.clone();
            distinct.sort();
            distinct.dedup();

            if distinct.len() > 1 {
                defects.push(RosterDefect::Collision {
                    owner: owner.clone(),
                    crate_names: distinct,
                });

                continue;
            }

            if !participating.iter().any(|registered| registered == owner) {
                defects.push(RosterDefect::MissingRegistration {
                    crate_name: crate_names[0].clone(),
                    owner: owner.clone(),
                });
            }
        }

        for owner in participating {
            if !derived.contains_key(owner) {
                defects.push(RosterDefect::UnexplainedOwner {
                    owner: owner.clone(),
                });
            }
        }

        for entry in &self.unbuilt {
            if discovered
                .iter()
                .any(|package| package.name() == entry.crate_name())
            {
                defects.push(RosterDefect::BuiltUnbuiltEntry {
                    crate_name: entry.crate_name().to_owned(),
                    directory: entry.directory().to_path_buf(),
                });
            }
        }

        defects.sort();
        defects
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnerNames, RosterDefect, UnbuiltMember, derive_owner};
    use crate::catalogue::Codec;
    use crate::workspace::Package;

    /// An invented project namespace, of a project that is not this one.
    const NAMESPACE: &str = "quarry-";

    /// An invented instance declaring one participating member with no manifest.
    fn instance() -> OwnerNames {
        OwnerNames::new(
            NAMESPACE,
            [UnbuiltMember::new("quarry-slate", "parts/slate")],
        )
    }

    fn owners(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    /// An owner's spelling is the projection of its crate name by one rule —
    /// strip the leading namespace where it stands, remove the hyphens,
    /// uppercase the rest — and a name that does not carry the namespace keeps
    /// the whole of itself. A name projecting onto nothing the calculus would
    /// accept derives nothing rather than a spelling that would be refused
    /// later.
    ///
    /// ´claim:roster:an-owner-spelling-is-the-projection-of-its-crate-name´
    /// ´test:unit:derives-an-owner-by-stripping-removing-and-uppercasing´
    #[test]
    fn derives_an_owner_by_stripping_removing_and_uppercasing() {
        for (crate_name, expected) in [
            ("quarry-slate", "SLATE"),
            ("quarry-slate-cutter", "SLATECUTTER"),
            ("quarry", "QUARRY"),
            ("outsider-tool", "OUTSIDERTOOL"),
        ] {
            let derived = derive_owner(NAMESPACE, crate_name).expect("a well-formed spelling");

            assert_eq!(derived.as_str(), expected, "spelling of `{crate_name}`");
        }

        for crate_name in ["-", "9-lives", ""] {
            assert!(
                derive_owner(NAMESPACE, crate_name).is_none(),
                "`{crate_name}` should derive nothing"
            );
        }
    }

    /// A built member and a declared unbuilt entry each account for one
    /// participating owner, and a roster explained in both directions reports
    /// nothing. The two sources of a member are equal before the reconciliation:
    /// what differs is where the crate name came from, not what it owes.
    ///
    /// ´claim:roster:a-built-member-and-a-declared-entry-account-for-an-owner-alike´
    /// ´test:unit:reconciles-a-built-member-and-an-unbuilt-entry-cleanly´
    #[test]
    fn reconciles_a_built_member_and_an_unbuilt_entry_cleanly() {
        let discovered = [Package::new("quarry-granite", "parts/granite")];

        assert_eq!(
            instance().reconcile(&discovered, &owners(&["GRANITE", "SLATE"])),
            Vec::new(),
            "the built member and the declared entry explain both owners"
        );
    }

    /// A member deriving a spelling no participating owner is registered under
    /// is reported: its prose has no owner to be attributed to, and the repair
    /// is a registration rather than a rename. A registered owner that
    /// activates no pair is outside the verdict entirely, so a partitioned
    /// nonparticipant neither explains a crate nor demands one.
    ///
    /// ´claim:roster:a-member-with-no-participating-registration-is-reported´
    /// ´test:unit:reports-a-member-missing-its-participating-registration´
    #[test]
    fn reports_a_member_missing_its_participating_registration() {
        let discovered = [Package::new("quarry-granite", "parts/granite")];

        assert_eq!(
            instance().reconcile(&discovered, &owners(&["SLATE"])),
            vec![RosterDefect::MissingRegistration {
                crate_name: "quarry-granite".to_owned(),
                owner: "GRANITE".to_owned(),
            }]
        );
    }

    /// A participating owner that neither a discovered member nor a declared
    /// entry accounts for is reported: the repository has a name it can no
    /// longer explain, and holding only the other direction would let such names
    /// accumulate forever.
    ///
    /// ´claim:roster:a-participating-owner-nothing-accounts-for-is-reported´
    /// ´test:unit:reports-an-unexplained-participating-owner´
    #[test]
    fn reports_an_unexplained_participating_owner() {
        let discovered = [Package::new("quarry-granite", "parts/granite")];

        assert_eq!(
            instance().reconcile(&discovered, &owners(&["GRANITE", "SLATE", "BASALT"])),
            vec![RosterDefect::UnexplainedOwner {
                owner: "BASALT".to_owned()
            }]
        );
    }

    /// Two distinct crate names projecting onto one spelling collide, and the
    /// collision is reported against the names rather than against the roster:
    /// one spelling cannot attribute two corpora, so no registration repairs it
    /// and the reconciliation does not go on to ask whether that spelling is
    /// registered. Removing the hyphens is what makes the projection
    /// non-injective, so two names differing only in where they break collide.
    ///
    /// ´claim:roster:two-crate-names-reaching-one-spelling-collide´
    /// ´test:unit:reports-a-collision-between-two-derived-spellings´
    #[test]
    fn reports_a_collision_between_two_derived_spellings() {
        let discovered = [
            Package::new("quarry-granite", "parts/granite"),
            Package::new("quarry-gran-ite", "parts/other"),
        ];

        assert_eq!(
            instance().reconcile(&discovered, &owners(&["GRANITE", "SLATE"])),
            vec![RosterDefect::Collision {
                owner: "GRANITE".to_owned(),
                crate_names: vec!["quarry-gran-ite".to_owned(), "quarry-granite".to_owned()],
            }]
        );
    }

    /// One crate name reaching a spelling from both the workspace and the
    /// declared entries is not a collision with itself. The duplication is the
    /// outlived-entry defect and is reported once, there — a self-collision
    /// would name one crate twice and offer a rename nobody can perform.
    ///
    /// ´claim:roster:a-name-reaching-one-spelling-twice-is-not-a-collision-with-itself´
    /// ´test:unit:reads-one-name-from-two-sources-as-one-claim´
    #[test]
    fn reads_one_name_from_two_sources_as_one_claim() {
        let discovered = [Package::new("quarry-slate", "parts/slate")];
        let defects = instance().reconcile(&discovered, &owners(&["SLATE"]));

        assert!(
            !defects
                .iter()
                .any(|defect| matches!(defect, RosterDefect::Collision { .. })),
            "one name is one claim however many sources carry it: {defects:?}"
        );
    }

    /// A declared unbuilt entry whose crate the workspace does build is
    /// reported: the row has outlived the absence it recorded. Leaving it
    /// standing would let the manifest disappear again later with nothing to
    /// notice, since the entry would go on explaining the owner either way.
    ///
    /// ´claim:roster:a-declared-entry-whose-manifest-arrived-is-reported´
    /// ´test:unit:reports-an-unbuilt-entry-the-workspace-builds´
    #[test]
    fn reports_an_unbuilt_entry_the_workspace_builds() {
        let discovered = [Package::new("quarry-slate", "parts/slate")];

        assert_eq!(
            instance().reconcile(&discovered, &owners(&["SLATE"])),
            vec![RosterDefect::BuiltUnbuiltEntry {
                crate_name: "quarry-slate".to_owned(),
                directory: "parts/slate".into(),
            }]
        );
    }

    /// A crate name projecting onto nothing well-formed is reported as itself
    /// rather than passed over, so a member can never go missing from the
    /// partition by being unnameable. An empty roster and an empty workspace
    /// reconcile cleanly, because absence of everything is a corpus with no
    /// owners rather than a defect.
    ///
    /// ´claim:roster:an-underivable-crate-name-is-reported-rather-than-passed-over´
    /// ´test:unit:reports-an-underivable-crate-name´
    #[test]
    fn reports_an_underivable_crate_name() {
        let discovered = [Package::new("9-lives", "parts/nine")];
        let empty = OwnerNames::new(NAMESPACE, []);

        assert_eq!(
            empty.reconcile(&discovered, &[]),
            vec![RosterDefect::UnderivableName {
                crate_name: "9-lives".to_owned()
            }]
        );
        assert_eq!(
            empty.reconcile(&[], &[]),
            Vec::new(),
            "and nothing reconciles with nothing"
        );
    }

    /// The reconciliation declares the fingerprint codec, because its defects
    /// are relationships between a crate, an owner and a directory rather than
    /// occurrences inside a path. Either path codec would alias two distinct
    /// defects concerning one package into one tolerated row.
    ///
    /// ´claim:roster:the-reconciliation-identifies-a-violation-by-a-digest´
    /// ´test:unit:the-owner-names-program-fingerprints-its-defects´
    #[test]
    fn the_owner_names_program_fingerprints_its_defects() {
        assert_eq!(OwnerNames::codec(), Codec::Fingerprint);
        assert_eq!(OwnerNames::codec().field(), "allowances");
    }
}
