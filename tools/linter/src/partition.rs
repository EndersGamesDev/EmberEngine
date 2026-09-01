// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The owner partition: every file the repository tracks accounted to exactly
//! one owner by exactly one row.
//!
//! Attribution used to be directory logic — package directories sorted longest
//! first, matched as prefixes, everything unmatched falling to the root owner.
//! Both halves of that are gone here. Longest match is a rule for resolving a
//! conflict, and a relation that can resolve conflicts is a relation whose
//! conflicts nobody has to notice; and the fallback meant a file nobody had
//! thought about acquired an owner by default rather than by a decision. What
//! replaces them is a declared relation that must partition the tree, with no
//! priority, no longest-match selection and no order dependence
//! (ADR-L-019, The layer owner graph).
//!
//! The accounting universe is the repository's tracked corpus: every path
//! `git ls-files` reports beneath the root, in the bytes git recorded for it.
//! What is partitioned is the repository's content, not the working copy that
//! happens to stand around it. A checkout carries material nobody committed —
//! build output, local configuration, editor droppings, a data directory the
//! program filled at runtime — and none of that is the repository. Reading the
//! universe from the physical tree made the verdict a property of whoever ran
//! the check rather than of the commit: one and the same tree partitioned
//! cleanly in a fresh clone and failed by two hundred thousand paths in a
//! working checkout, where a runtime data directory outnumbered the corpus by
//! two orders of magnitude and a directory the user could not read failed
//! traversal outright. The tracked set answers the same for one commit in a
//! clone, a worktree and a dirty checkout alike, and asks the filesystem for
//! nothing beyond the listing.
//!
//! A symbolic link is a tracked entry in its own right and is never followed —
//! git records the link, never what it points at. A directory is not an entry
//! and git records none, so an empty directory contributes nothing for the same
//! reason it always did. Nothing else leaves the universe implicitly: no ignore
//! file, generated-artifact or hidden-path convention removes a *tracked* path,
//! because an implicit exclusion is an ownership decision nobody ratified
//! (ADR-L-019, The layer owner graph). An entry leaves because a human wrote
//! a named rule saying so, and the excluded set is removed as a union — a union
//! has no order, so no exclusion rule can shadow another and overlapping distinct
//! rules are legal.
//!
//! The version-control store is the one path the tracked set never carries,
//! because git does not track its own store. The named rule that excused `.git`
//! stands and now reaches nothing, which is a thing an exclusion rule is
//! entitled to do: a rule matching no path is legal, and keeping it records
//! that the exclusion was decided rather than inherited from whatever mechanism
//! enumerates the tree. A later universe that did carry the store would find
//! the rule already waiting for it.
//!
//! `-z` is what makes the listing usable at this discipline. Without it git
//! renders an awkward path as a C-style quoted string, and `core.quotePath`
//! decides per repository and per user which bytes that reaches; with it git
//! writes the recorded bytes verbatim, separated by NUL, and no configuration
//! alters them. NUL is the one byte a path cannot contain, so the separation is
//! unambiguous and the bytes arrive exactly as the byte-path discipline needs
//! them — no decoding step, and nothing for a locale to have an opinion about.
//!
//! Taking the listing can still fail: git absent, the root not a repository,
//! the command refusing. That is the traversal failure the walk used to report
//! from a directory it could not open, and it is reported once against the root
//! with git's own account of it, rather than yielding a universe that quietly
//! shrank and let totality pass for the wrong reason.
//!
//! Each rule's own reach is tallied beside the union it contributes to. That is
//! what the rule's name is for: a report can say which rule excused a path rather
//! than only that some rule did. Because coverage may overlap, the tallies can
//! sum to more than the union removed, and the excess is the redundancy the
//! surface deliberately permits made visible rather than hidden.
//!
//! Totality is that every survivor matches at least one inclusion row.
//! Exclusivity is stated at the row: a path matching two rows fails even when
//! both rows name the same owner. Both are ordinary findings against a snapshot
//! that parsed, because the snapshot is well-formed and it is the description
//! that disagrees with the tree (´rule:commandcontract:configuration-verdicts´).
//!
use std::collections::BTreeMap;

#[cfg(test)]
use crate::finding::Finding;
#[cfg(test)]
use crate::pattern::BytePath;
#[cfg(test)]
use crate::snapshot::{OwnerRow, Snapshot};

/// What the partition was found to be, in counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct PartitionCounts {
    /// How many non-directory entries the walk reached.
    pub universe: usize,
    /// How many of them the excluded set removed.
    pub excluded: usize,
    /// How many each named exclusion rule reached, which may overlap.
    pub excluded_by: BTreeMap<String, usize>,
    /// How many survived the exclusion pre-pass.
    pub surviving: usize,
    /// How many survivors exactly one inclusion row accounted.
    pub accounted: usize,
    /// How many survivors no inclusion row accounted.
    pub unaccounted: usize,
    /// How many survivors more than one inclusion row accounted.
    pub multiply_accounted: usize,
}

/// Remove the union of every exclusion match, then account what survives.
///
/// The two passes are separate because a path removed by the exclusion relation
/// is never evaluated against the inclusion relation at all, so an overlap
/// between an exclusion rule and an inclusion row is legal and cannot be double
/// accounting.
#[must_use]
#[cfg(test)]
pub fn retiring_verify(
    snapshot: &Snapshot,
    universe: &[BytePath],
) -> (PartitionCounts, Vec<Finding>) {
    let mut counts = PartitionCounts {
        universe: universe.len(),
        ..PartitionCounts::default()
    };

    let mut findings = Vec::new();

    for path in universe {
        // Every matching rule is tallied rather than only the first, because the
        // relation is a union and the per-rule reach is what the names buy.
        let mut removed = false;

        for rule in snapshot.shape().ignore() {
            if rule.matches(path) {
                *counts
                    .excluded_by
                    .entry(rule.name().to_owned())
                    .or_default() += 1;
                removed = true;
            }
        }

        if removed {
            counts.excluded += 1;
            continue;
        }

        counts.surviving += 1;

        let accounting: Vec<&OwnerRow> = snapshot
            .partitions()
            .iter()
            .filter(|row| row.pattern.admits_path(path))
            .collect();

        match accounting.len() {
            1 => counts.accounted += 1,
            0 => {
                counts.unaccounted += 1;
                findings.push(Finding::UnaccountedPath {
                    path: path.display(),
                });
            }
            count => {
                counts.multiply_accounted += 1;

                let mut matches: Vec<String> = accounting.iter().map(ToString::to_string).collect();
                matches.sort();

                findings.push(Finding::MultiplyAccountedPath {
                    path: path.display(),
                    count,
                    matches,
                });
            }
        }
    }

    (counts, findings)
}

/// Attribute each surviving path to the owner that accounts it.
///
/// A path no row accounts, and a path more than one accounts, are absent from
/// the result rather than guessed at: the verdict has already named them, and an
/// attribution invented here would be the fallback this relation exists to
/// remove.
#[must_use]
#[cfg(test)]
pub fn retiring_attribution<'a>(
    snapshot: &'a Snapshot,
    universe: &'a [BytePath],
) -> BTreeMap<&'a BytePath, &'a str> {
    let mut attributed = BTreeMap::new();

    for path in universe {
        if snapshot
            .shape()
            .ignore()
            .iter()
            .any(|row| row.matches(path))
        {
            continue;
        }

        let mut matched = snapshot
            .partitions()
            .iter()
            .filter(|row| row.pattern.admits_path(path));

        if let (Some(row), None) = (matched.next(), matched.next()) {
            attributed.insert(path, row.owner.as_str());
        }
    }

    attributed
}
