// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Corpus geography: where a share begins, and where a declared surface walks.
//!
//! A census over a corpus has to say where it looks. It used to say so in
//! compiled tables of repository paths, and those tables were a description of
//! somebody else's tree kept inside this binary — which is the arrangement in
//! which a directory is renamed and a tool goes on counting a place that is not
//! there.
//!
//! What replaced them was an experiment: one owner-areas document naming reaches
//! owner-relatively, which every census then resolved against its owner's root.
//! The experiment retired on the ruling that a scan surface is the property of
//! the policy that scans, not of the owner scanned — a policy needing to know
//! what a source is and what a bench is holds that knowledge in its own domain.
//! So a surface now stands in the document of its own policy and spells its
//! places outright, and what is left here is the two questions that survive it.
//!
//! # The root is read off the partition rather than declared again
//!
//! Where a share begins is already written down: the owner document says which
//! paths each owner includes. So the root is the longest run of components every
//! path an owner includes stands under, read from those patterns' own openings.
//! Declaring it a second time would create the one thing a second copy always
//! creates, which is the pair that disagrees.
//!
//! An owner whose partition rows share nothing is rooted at the corpus root, which is
//! the true answer for the owner that holds the repository's own crate: its share
//! begins where the repository does.
//!
//! That root is what the derived exclusions of a census are made of. A rule
//! saying that no census walks the checker's own share, or that a
//! repository-scoped census leaves alone the shares whose owners hold its
//! sibling, names owners and never directories; the partition turns each named
//! owner into a place.
//!
//! # A surface resolves to places, and not to a promise
//!
//! A surface is where a walk starts, and it becomes the places its pattern opens
//! with — including places that are not there, because a surface may name a
//! target directory a package does not carry yet and a reach that quietly forgot
//! it would make the declaration a commitment to create it.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`roots_a_share_where_every_path_it_includes_stands_under`] | areas | An owner's root is the longest run of components every path its partition rows admit stands under, read from the patterns themselves. A share named by one subtree roots at that subtree, a share named by several files under one directory roots at the directory rather than at a file, and a share whose partition rows share no opening roots at the corpus root — which is the true answer for the owner holding the repository's own crate. |
//! | [`opens_a_surface_at_every_place_its_pattern_names`] | areas | A declared surface resolves to the places its pattern opens with, in the order the pattern writes its arms, and a pattern committing to no opening at all is the corpus root entire — which is how a census over the whole repository states its surface without naming one directory of it. The places are taken as written, because a surface stands in the document of the policy that walks it and there is no owner root left to resolve it against. |

use std::path::PathBuf;

use crate::declaration::AbnfPattern;

/// The corpus root, as a declaration and a register row both spell it.
pub const CORPUS_ROOT: &str = ".";

/// The places one declared surface opens with, as the walk reading it meets them.
///
/// A surface stands in the document of the policy whose census walks it, and a
/// policy document has no owner root to resolve against: the corpus writes the
/// places out, and a pattern committing to no opening at all is the corpus root
/// entire — which is how a census over the whole repository is stated without
/// naming one directory of it.
#[must_use]
pub fn places(pattern: &AbnfPattern) -> Vec<PathBuf> {
    pattern
        .branches()
        .iter()
        .map(|branch| match branch.text() {
            "" => PathBuf::from(CORPUS_ROOT),
            opening => PathBuf::from(opening),
        })
        .collect()
}

/// Where a share begins, read from the patterns that say what it includes.
///
/// The answer is the longest run of components every path the owner includes
/// stands under. A complete opening names a file rather than a place, so what it
/// contributes is the directory it stands in; a prefix opening is already the
/// place a reach begins and contributes itself.
#[must_use]
pub fn partition_root<'a>(patterns: impl IntoIterator<Item = &'a AbnfPattern>) -> PathBuf {
    let mut shared: Option<Vec<String>> = None;

    for pattern in patterns {
        for branch in pattern.branches() {
            let mut components: Vec<String> =
                branch.text().split('/').map(ToOwned::to_owned).collect();

            // A complete opening is a file's whole spelling, and a file is not
            // the place a share begins.
            if branch.is_complete() {
                components.pop();
            }

            components.retain(|component| !component.is_empty());

            shared = Some(match shared {
                None => components,
                Some(held) => held
                    .into_iter()
                    .zip(components)
                    .take_while(|(held, found)| held == found)
                    .map(|(held, _found)| held)
                    .collect(),
            });
        }
    }

    match shared {
        Some(components) if !components.is_empty() => PathBuf::from(components.join("/")),
        _ => PathBuf::from(CORPUS_ROOT),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{partition_root, places};
    use crate::declaration::AbnfPattern;

    /// A pattern the declared surface would carry, compiled.
    fn pattern(source: &str) -> AbnfPattern {
        AbnfPattern::parse(source).expect("a pattern of the form")
    }

    /// An owner's root is the longest run of components every path its partition
    /// rows admit stands under, read from the patterns themselves. A share named
    /// by one subtree roots at that subtree, a share named by several files under
    /// one directory roots at the directory rather than at a file, and a share
    /// whose partition rows share no opening roots at the corpus root —
    /// which is the true answer for the owner holding the repository's own
    /// crate.
    ///
    /// ´claim:areas:a-root-is-read-off-the-partition´
    /// ´test:unit:roots-a-share-where-every-path-it-includes-stands-under´
    #[test]
    fn roots_a_share_where_every_path_it_includes_stands_under() {
        let subtree = [pattern(r#"%s"packages/assayer" [ "/" *VCHAR ]"#)];

        assert_eq!(partition_root(&subtree), PathBuf::from("packages/assayer"));

        // Several files under one directory root at the directory: a complete
        // opening names a file, and a file is not where a share begins.
        let listed = [pattern(
            r#"%s"contrib/dev-tools/su-exec/" ( %s"LICENSE" / %s"Makefile" / %s"su-exec.c" )"#,
        )];

        assert_eq!(
            partition_root(&listed),
            PathBuf::from("contrib/dev-tools/su-exec")
        );

        // A share reaching several trees and several root files shares no
        // opening at all, so it begins where the repository does.
        let spread = [
            pattern(r#"%s"docs" [ "/" *VCHAR ]"#),
            pattern(r#"%s"src" [ "/" *VCHAR ]"#),
            pattern(r#"%s"AGENTS.md""#),
        ];

        assert_eq!(partition_root(&spread), PathBuf::from("."));

        // A root is a run of whole components, so two siblings sharing letters
        // root above both rather than inside a name neither carries.
        let siblings = [
            pattern(r#"%s"packages/index-config" [ "/" *VCHAR ]"#),
            pattern(r#"%s"packages/index-config-probe" [ "/" *VCHAR ]"#),
        ];

        assert_eq!(partition_root(&siblings), PathBuf::from("packages"));

        // An owner including nothing begins nowhere in particular.
        assert_eq!(partition_root([]), PathBuf::from("."));
    }

    /// A declared surface resolves to the places its pattern opens with, in the
    /// order the pattern writes its arms, and a pattern committing to no opening
    /// at all is the corpus root entire — which is how a census over the whole
    /// repository states its surface without naming one directory of it. The
    /// places are taken as written, because a surface stands in the document of
    /// the policy that walks it and there is no owner root left to resolve it
    /// against.
    ///
    /// ´claim:areas:a-surface-opens-at-the-places-its-pattern-names´
    /// ´test:unit:opens-a-surface-at-every-place-its-pattern-names´
    #[test]
    fn opens_a_surface_at_every_place_its_pattern_names() {
        let member = pattern(r#"%s"packages/assayer/" ( %s"docs" / %s"adr" ) [ "/" *VCHAR ]"#);

        assert_eq!(
            places(&member),
            vec![
                PathBuf::from("packages/assayer/docs"),
                PathBuf::from("packages/assayer/adr")
            ]
        );

        // One tree named alone is one place, spelled where the corpus put it.
        assert_eq!(
            places(&pattern(r#"%s"docs" [ "/" *VCHAR ]"#)),
            vec![PathBuf::from("docs")]
        );

        // A pattern naming no opening is the repository entire.
        assert_eq!(places(&pattern("*VCHAR")), vec![PathBuf::from(".")]);
    }
}
