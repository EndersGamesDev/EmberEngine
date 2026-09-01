// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Programme-catalogue totality and observable-boundary fixtures.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`the_programme_catalogue_keeps_policy_order`] | catalogue | Unification keeps the live policy catalogue's established order exactly, because consumers may present observations in that order. |
//! | [`every_label_dependent_template_names_the_label_root`] | catalogue | The complete profile family and the legacy to-do census each name the well-formed-mint policy as a same-owner prerequisite. Enumerating the family from the catalogue makes a newly added profile without the edge fail here rather than participate only through compiled geography. |
//! | [`every_adopted_set_kind_activates_exactly_one_programme`] | catalogue | Every adopted declared set kind maps to exactly one compiled programme, every programme-declared kind is adopted, and uncatalogued kinds remain preserved rather than refused. |
//! | [`singleton_and_family_lookups_keep_their_programmes`] | catalogue | A singleton is found by its policy identifier while every family entry is found by its declared set kind, with multi-kind programmes joining back to the same row. |
//! | [`catalogue_refusal_and_list_codecs_are_exact`] | catalogue | The unknown-policy refusal bytes and the three list-codec selections stay exact across catalogue unification. |

use std::collections::BTreeSet;

use crate::catalogue::{CATALOG, Codec, Dependency, Scope, catalogued, program_of};
use crate::declaration::SetKind;
use crate::snapshot::{POLICIES_FILE, Pair, Refusal};

/// The declared set kinds the running binary adopts.
const ADOPTED: [&str; 7] = [
    "numbered-marks",
    "prefix-numbers",
    "literals",
    "identifier",
    "copyright",
    "name-key",
    "name-prefix-ignore",
];

/// Unification keeps the live policy catalogue's established order exactly,
/// because consumers may present observations in that order.
///
/// ´claim:catalogue:programme-order-is-stable´
/// ´test:crate:the-programme-catalogue-keeps-policy-order´
#[test]
fn the_programme_catalogue_keeps_policy_order() {
    let names: Vec<_> = CATALOG.iter().map(|policy| policy.name).collect();

    assert_eq!(
        names,
        [
            "labels.mints-well-formed",
            "labels.mints-kind-conform",
            "labels.mints-unique",
            "labels.heads-conform",
            "labels.citations-local-resolve",
            "labels.citations-imported-resolve",
            "labels.citations-import-form",
            "labels.citations-layer-conform",
            "labels.generated-regions-conform",
            "labels.outlines-conform",
            "profile.tests-conform",
            "profile.todos-conform",
            "profile.legacy-conform",
            "profile.claims-conform",
            "profile.constants-conform",
            "projection.test-indexes-current",
            "projection.test-matrices-current",
            "projection.constant-pins-current",
            "assembly.assayer-spec-current",
            "owners.reach-conform",
            "spdx.headers-conform",
            "interchange.envelope-conform",
            "references.file-paths-absent",
            "legacy.section-references",
            "legacy.record-references",
            "legacy.section-references-repository",
            "legacy.record-references-repository",
            "legacy.unprefixed-record-references",
            "legacy.tag-references",
            "legacy.scenario-numbers",
            "legacy.division-names",
            "legacy.residual-litter",
            "legacy.todos",
            "legacy.implementation",
            "owners.crate-names-conform",
            "assembly.publications-current",
            "references.mark-numbered-absent",
            "references.literal-set-absent",
            "references.prefix-numbers-absent",
        ]
    );
}

/// The complete profile family and the legacy to-do census each name the
/// well-formed-mint policy as a same-owner prerequisite. Enumerating the family
/// from the catalogue makes a newly added profile without the edge fail here
/// rather than participate only through compiled geography.
///
/// ´claim:catalogue:every-label-dependent-template-names-the-label-root´
/// ´test:crate:every-label-dependent-template-names-the-label-root´
#[test]
fn every_label_dependent_template_names_the_label_root() {
    let label_root = Dependency {
        scope: Scope::SameOwner,
        policy: "labels.mints-well-formed",
    };
    let dependents: Vec<_> = CATALOG
        .iter()
        .filter(|policy| {
            policy.name.starts_with("profile.")
                || matches!(policy.name, "legacy.todos" | "legacy.implementation")
        })
        .collect();

    assert_eq!(
        dependents
            .iter()
            .map(|policy| policy.name)
            .collect::<Vec<_>>(),
        [
            "profile.tests-conform",
            "profile.todos-conform",
            "profile.legacy-conform",
            "profile.claims-conform",
            "profile.constants-conform",
            "legacy.todos",
            "legacy.implementation",
        ]
    );

    for policy in dependents {
        assert!(
            policy.dependencies.contains(&label_root),
            "{} lacks the label root",
            policy.name
        );
    }
}

/// Every adopted declared set kind maps to exactly one compiled programme,
/// every programme-declared kind is adopted, and uncatalogued kinds remain
/// preserved rather than refused.
///
/// ´claim:catalogue:every-adopted-kind-activates-one-programme´
/// ´test:crate:every-adopted-set-kind-activates-exactly-one-programme´
#[test]
fn every_adopted_set_kind_activates_exactly_one_programme() {
    let mut seen = BTreeSet::new();

    for declared in CATALOG.declared_kinds() {
        let set = declared.name();

        assert!(
            seen.insert(set),
            "`{set}` is read by more than one programme"
        );
        assert!(
            SetKind::of(set).is_adopted(),
            "`{set}` is catalogued but not adopted"
        );
        assert_eq!(
            CATALOG
                .program_for_set(set)
                .map(|program| program.policy().name),
            program_of(set)
        );
    }

    assert_eq!(seen, BTreeSet::from(ADOPTED));
    assert_eq!(program_of("uncatalogued-shape"), None);
    assert_eq!(SetKind::of("uncatalogued-shape"), SetKind::Unadopted);
}

/// A singleton is found by its policy identifier while every family entry is
/// found by its declared set kind, with multi-kind programmes joining back to
/// the same row.
///
/// ´claim:catalogue:singleton-and-family-lookups-are-distinct´
/// ´test:crate:singleton-and-family-lookups-keep-their-programmes´
#[test]
fn singleton_and_family_lookups_keep_their_programmes() {
    assert_eq!(
        catalogued("labels.mints-unique").map(|policy| policy.name),
        Some("labels.mints-unique")
    );
    assert_eq!(
        program_of("prefix-numbers"),
        Some("references.prefix-numbers-absent")
    );
    assert_eq!(program_of("identifier"), Some("spdx.headers-conform"));
    assert_eq!(program_of("copyright"), Some("spdx.headers-conform"));
    assert!(
        CATALOG
            .program("spdx.headers-conform")
            .is_some_and(|program| {
                program
                    .sets()
                    .iter()
                    .map(|declared| declared.name())
                    .collect::<Vec<_>>()
                    == ["identifier", "copyright"]
            })
    );
    assert!(
        CATALOG
            .program("labels.mints-unique")
            .is_some_and(|program| { program.sets().is_empty() && !program.reads_datum() })
    );
}

/// The unknown-policy refusal bytes and the three list-codec selections stay
/// exact across catalogue unification.
///
/// ´claim:catalogue:refusal-and-codec-fixtures-are-exact´
/// ´test:crate:catalogue-refusal-and-list-codecs-are-exact´
#[test]
fn catalogue_refusal_and_list_codecs_are_exact() {
    let refusal = Refusal::UnknownPolicy {
        file: POLICIES_FILE,
        policy: String::from("labels.mints-unheard-of"),
    };

    assert_eq!(
        refusal.to_string(),
        ".linter/policies.toml: labels.mints-unheard-of: this binary catalogues no such policy"
    );

    let codecs = [
        ("labels.mints-well-formed", Codec::Fingerprint, "allowances"),
        ("legacy.todos", Codec::PathCount, "path_counts"),
        ("legacy.implementation", Codec::PathCount, "path_counts"),
        ("spdx.headers-conform", Codec::PathSet, "paths"),
    ];

    for (name, codec, field) in codecs {
        let policy = catalogued(name).expect("the fixture names a catalogued programme");

        assert_eq!(policy.codec, codec);
        assert_eq!(policy.codec.field(), field);
    }

    let wrong = Refusal::WrongCodec {
        pair: Pair::singleton("INDEX", "labels.mints-well-formed"),
        expected: Codec::Fingerprint,
        found: String::from("path_counts"),
    };

    assert_eq!(
        wrong.to_string(),
        ".linter/lists.toml: INDEX : labels.mints-well-formed: the policy selects `allowances` and the table carries `path_counts`"
    );
}
