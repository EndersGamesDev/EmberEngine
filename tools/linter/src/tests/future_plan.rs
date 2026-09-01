// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Test-only specifications salvaged for the future execution-plan bite.
//!
//! These helpers deliberately do not form a production resolver. They preserve
//! the useful union and topology algorithms while the dead resolver is retired,
//! so the later plan compiler has executable specifications without a shadow
//! production route.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`future_plan_unions_namespaces_before_programmes`] | catalogue | Namespace identity is established over the declaration union before any programme is selected, adopted kinds select programmes, and uncatalogued kinds remain inert. |
//! | [`future_plan_bounds_selection_by_owner_then_subtracts`] | catalogue | Topology first bounds a source to its owner's share, then admits by the entry pattern and subtracts the section-wide exclusion. |
//! | [`future_plan_refuses_a_non_text_path_once`] | catalogue | A path that cannot become pattern text is refused once in reversible display form rather than being silently absent from every programme. |
//! | [`future_plan_detects_overlap_only_on_shared_sources`] | catalogue | Two compiled prefix domains conflict only where their resolved source memberships intersect. |

use std::collections::{BTreeMap, BTreeSet};

use crate::catalogue::program_of;
use crate::declaration::{AbnfPattern, Declaration};
use crate::pattern::BytePath;
use crate::program::{PrefixBound, PrefixNumbers};

/// One path after the single bytes-to-pattern-text decision.
#[derive(Debug)]
struct Source {
    path: BytePath,
    text: String,
}

/// One test-only ownership bound.
struct Ownership {
    owner: &'static str,
    pattern: AbnfPattern,
}

/// Decode a relative display for an invented corpus.
fn path(display: &str) -> BytePath {
    BytePath::from_bytes(display.as_bytes()).expect("a relative fixture path")
}

/// Index declaration identities over the whole union.
fn namespaces(declarations: &[Declaration]) -> Result<BTreeMap<&str, &Declaration>, String> {
    let mut indexed = BTreeMap::new();

    for declaration in declarations {
        if indexed
            .insert(declaration.namespace(), declaration)
            .is_some()
        {
            return Err(format!(
                "{}: two documents stamp this identity",
                declaration.namespace()
            ));
        }
    }

    Ok(indexed)
}

/// The programmes a declaration's adopted kinds activate.
fn programmes(declaration: &Declaration) -> BTreeSet<&'static str> {
    declaration
        .sets()
        .keys()
        .filter_map(|set| program_of(set))
        .collect()
}

/// Make the corpus readable once before topology or selection.
fn readable(paths: &[BytePath]) -> Result<Vec<Source>, String> {
    paths
        .iter()
        .map(|path| {
            std::str::from_utf8(path.as_bytes())
                .map(|text| Source {
                    path: path.clone(),
                    text: text.to_owned(),
                })
                .map_err(|_| {
                    format!(
                        "{}: the path is not text, so no pattern decides it",
                        path.display()
                    )
                })
        })
        .collect()
}

/// Resolve the share of one owner from the compiled ownership rows.
fn share(owner: &str, ownership: &[Ownership], sources: &[Source]) -> BTreeSet<BytePath> {
    sources
        .iter()
        .filter(|source| {
            ownership
                .iter()
                .any(|row| row.owner == owner && row.pattern.admits(&source.text))
        })
        .map(|source| source.path.clone())
        .collect()
}

/// Reach one relation inside an already-resolved owner share.
fn reached(
    sources: &[Source],
    share: &BTreeSet<BytePath>,
    patterns: &[AbnfPattern],
) -> BTreeSet<BytePath> {
    sources
        .iter()
        .filter(|source| share.contains(&source.path))
        .filter(|source| patterns.iter().any(|pattern| pattern.admits(&source.text)))
        .map(|source| source.path.clone())
        .collect()
}

/// Admit by the entry and subtract the section's exclusion union.
fn selected(
    sources: &[Source],
    owner_share: &BTreeSet<BytePath>,
    include: &[AbnfPattern],
    exclude: &[AbnfPattern],
) -> BTreeSet<BytePath> {
    let admitted = reached(sources, owner_share, include);
    let removed = reached(sources, owner_share, exclude);

    admitted.difference(&removed).cloned().collect()
}

/// Whether two prefix-number runs could read one occurrence.
fn overlaps(
    one: &PrefixNumbers,
    one_sources: &BTreeSet<BytePath>,
    other: &PrefixNumbers,
    other_sources: &BTreeSet<BytePath>,
) -> bool {
    one.overlaps(other) && one_sources.intersection(other_sources).next().is_some()
}

/// Namespace identity is established over the declaration union before any
/// programme is selected, adopted kinds select programmes, and uncatalogued
/// kinds remain inert.
///
/// ´claim:catalogue:future-plan-namespace-union-precedes-programme-selection´
/// ´test:crate:future-plan-unions-namespaces-before-programmes´
#[test]
fn future_plan_unions_namespaces_before_programmes() {
    let adopted = Declaration::decode(
        r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"
"#,
    )
    .expect("an adopted fixture");
    let uncatalogued = Declaration::decode(
        r#"
namespace = "example.owner.extension"
version = [1, 0, 0]

[set.uncatalogued-shape]
kept = "not this binary's adoption decision"
"#,
    )
    .expect("an uncatalogued set remains a declaration");

    let declarations = [adopted.clone(), uncatalogued];
    let indexed = namespaces(&declarations).expect("distinct namespace identities");

    assert_eq!(indexed.len(), declarations.len());
    assert_eq!(
        programmes(&adopted),
        BTreeSet::from(["references.literal-set-absent"])
    );
    assert!(programmes(indexed["example.owner.extension"]).is_empty());

    assert_eq!(
        namespaces(&[adopted.clone(), adopted]).expect_err("one identity claimed twice"),
        "com.torrust.index.linter.policy.references.divisions: two documents stamp this identity"
    );
}

/// Topology first bounds a source to its owner's share, then admits by the
/// entry pattern and subtracts the section-wide exclusion.
///
/// ´claim:catalogue:future-plan-selection-is-owner-bounded-and-subtractive´
/// ´test:crate:future-plan-bounds-selection-by-owner-then-subtracts´
#[test]
fn future_plan_bounds_selection_by_owner_then_subtracts() {
    let paths = [
        path("quarry/docs/slate.md"),
        path("quarry/docs/notes/draft.md"),
        path("quarry/src/lode.rs"),
        path("spoil/docs/heap.md"),
    ];
    let sources = readable(&paths).expect("text fixture paths");
    let ownership = [
        Ownership {
            owner: "QUARRY",
            pattern: AbnfPattern::parse(r#"%s"quarry" [ "/" *VCHAR ]"#).expect("the quarry bound"),
        },
        Ownership {
            owner: "SPOIL",
            pattern: AbnfPattern::parse(r#"%s"spoil" [ "/" *VCHAR ]"#).expect("the spoil bound"),
        },
    ];
    let quarry = share("QUARRY", &ownership, &sources);
    let chosen = selected(
        &sources,
        &quarry,
        &[AbnfPattern::parse(r#"%s"quarry" [ "/" *VCHAR ]"#).expect("the inclusion")],
        &[AbnfPattern::parse(r#"%s"quarry/docs/notes" [ "/" *VCHAR ]"#).expect("the exclusion")],
    );
    let displays: Vec<_> = chosen.iter().map(BytePath::display).collect();

    assert_eq!(
        displays,
        [
            "quarry/docs/slate.md".to_owned(),
            "quarry/src/lode.rs".to_owned()
        ]
    );
}

/// A path that cannot become pattern text is refused once in reversible display
/// form rather than being silently absent from every programme.
///
/// ´claim:catalogue:future-plan-non-text-paths-refuse-once´
/// ´test:crate:future-plan-refuses-a-non-text-path-once´
#[test]
fn future_plan_refuses_a_non_text_path_once() {
    let opaque = BytePath::from_bytes(vec![
        b'q', b'u', b'a', b'r', b'r', b'y', b'/', 0xff, b'.', b'm', b'd',
    ])
    .expect("a relative byte path");

    assert_eq!(
        readable(&[opaque]).expect_err("a pattern cannot decide opaque bytes"),
        "quarry/%FF.md: the path is not text, so no pattern decides it"
    );
}

/// Two compiled prefix domains conflict only where their resolved source
/// memberships intersect.
///
/// ´claim:catalogue:future-plan-overlap-is-domain-and-membership-intersection´
/// ´test:crate:future-plan-detects-overlap-only-on-shared-sources´
#[test]
fn future_plan_detects_overlap_only_on_shared_sources() {
    let range = PrefixNumbers::new(
        "L-",
        PrefixBound::LeadingRange {
            minimum: 1,
            maximum: 30,
        },
        true,
    );
    let set = PrefixNumbers::new("L-", PrefixBound::LeadingSet(vec![4, 9]), true);
    let prose = BTreeSet::from([path("quarry/docs/slate.md")]);
    let same = BTreeSet::from([path("quarry/docs/slate.md")]);
    let code = BTreeSet::from([path("quarry/src/lode.rs")]);

    assert!(overlaps(&range, &prose, &set, &same));
    assert!(!overlaps(&range, &prose, &set, &code));
}
