// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The test profile of ADR-T-015: classification, derivation, and the standard
//! place.
//!
//! The profiles signature of ADR-T-014, A calculus of documentation and source labels, says
//! what a profile must fix: its kind token, its census, its classification
//! rule, its name transformation, and its standard place. The census lives next
//! door; this module is the other four, and the derivation judgment
//! (ADR-T-014, A calculus of documentation and source labels) binds them together — the kind is the
//! profile's, the area is the profile's classification of the asset, and the
//! name is the profile's transformation of the asset's bare identifier. The
//! derivation reads those facts and no others.
//!
//! # Classification
//!
//! ADR-T-015 assigns the area from the asset's structural home, and its table
//! is read here as a rule about the file, not about the module nesting inside
//! it: a test in a file under the crate's `src/tests/` directory is of the
//! crate area however deeply nested its module is, and only a file outside that
//! directory yields the unit area. Reading it the other way — letting an inline
//! module inside a `src/tests/` file yield the unit area — would make the area
//! depend on module structure, which the rejected Ansatz on path derivation
//! (ADR-T-014, A calculus of documentation and source labels) warns against and which the
//! table's own wording does not ask for.
//!
//! Two consequences are worth stating where a reader will meet them. A `tests`
//! directory nested deeper than the crate root, such as a `src/maths/tests/`,
//! is not the crate's `src/tests/` directory and so yields the unit area. And a
//! module file named `src/tests/mod.rs` is under that directory like any other
//! file there, so its own tests are of the crate area.
//!
//! # The standard place
//!
//! The standard place is the final line of the test function's documentation
//! comment, written in the code syntax of the calculus — the label between two
//! acute accents. The derivation-warrant inference rule
//! (ADR-T-014, A calculus of documentation and source labels) makes writing the label
//! attestation rather than naming, so a final line whose text differs from the
//! derivation warrants nothing and fails; and the warrant-totality invariant
//! (ADR-T-014, A calculus of documentation and source labels) makes a label of this kind standing
//! anywhere but a standard place a hard failure rather than a mint.
//!
//! This module scans the documentation of covered tests and nothing else. A
//! citation of the kind — the label parenthesised — is left alone here, because
//! resolving citations out of Rust comments is the code carrier's work, and it
//! now does it: a citation standing in commentary resolves against the one
//! registry like any other, while the standard place stays this profile's.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`classifies_the_three_areas`] | profile | A test's area is decided by where its file sits in the package: an ordinary source is a unit test, the package's crate-test directory holds crate tests, and its integration directory holds integration tests. A label's area therefore says which surface the test exercises. |
//! | [`classifies_a_module_file_under_the_crate_tests_directory`] | profile | cites (´claim:profile:a-tests-area-is-decided-by-where-its-file-sits´) |
//! | [`classifies_a_nested_module_by_its_file_not_its_nesting`] | profile | Nesting inside a file changes nothing: a test written in a module within a crate-test source is a crate test, because the area follows the file and not the module path above the function. |
//! | [`classifies_a_deeper_tests_directory_as_unit`] | profile | Only the package's own crate-test directory names crate tests: a directory of the same name nested deeper inside the sources holds unit tests, so a module may organise its tests into a folder without reclassifying them. |
//! | [`transforms_underscores_into_hyphens`] | profile | A label's name is the function's identifier with its word separators rewritten to the ones the grammar admits, and a single-word identifier passes through as it is. The transformation is that and nothing more. |
//! | [`derives_the_label_of_the_records_example`] | profile | A test's whole label is derived from its area and its identifier, kind included, so the label is a fact about the test rather than a name anybody chose for it. |
//! | [`refuses_an_identifier_that_is_no_label_name`] | profile | An identifier that does not transform into a well-formed name derives no label: capitals, a leading or trailing separator, and a doubled one are each refused rather than repaired into something the grammar would accept. |
//! | [`accepts_the_derived_label_at_the_standard_place`] | profile | A test carrying its derived label as the final line of its documentation is clean, and is counted as both covered and labelled. This is what a conforming test looks like. |
//! | [`reports_a_test_without_documentation`] | profile | A covered test not carrying its label at the standard place is reported with the label it should have had and with the reason it is missing, so the author is told both what to write and why it is wanted. Having no documentation at all is one such reason. |
//! | [`reports_documentation_whose_final_line_is_not_the_label`] | profile | cites (´claim:profile:a-missing-label-is-reported-with-its-derivation-and-its-reason´) |
//! | [`reports_a_label_with_extra_text_after_it`] | profile | cites (´claim:profile:a-missing-label-is-reported-with-its-derivation-and-its-reason´) |
//! | [`reports_a_label_that_is_not_the_derivation`] | profile | A label at the standard place attests the derivation rather than naming the test: one that is not what the test derives is reported showing both what stands there and what the test actually gives, so renaming a function and forgetting its label is caught. |
//! | [`reports_a_label_of_this_kind_away_from_the_standard_place`] | profile | A second label of the derived kind minted elsewhere in the same comment is reported as misplaced while the one at the standard place still counts, so a derived kind is minted at its own place and nowhere else. |
//! | [`leaves_a_citation_away_from_the_standard_place_alone`] | profile | Citing is not minting: a test's documentation may refer to another test by its label without that being a misplaced mint, so a comment can point at a sibling while carrying its own label below. |
//! | [`reports_a_collision_with_both_locations`] | profile | The derivation must be injective within an owner: two tests of one name in one package collide, and the report names the identifier and both files, so a citation of that label can never be ambiguous about which test it reached. |
//! | [`collides_on_the_identifier_even_when_the_areas_differ`] | profile | Collision is judged on the identifier rather than on the finished label: two tests sharing a name in different areas collide even though their labels differ, because a reader searching for the name would find two tests and no way to tell which a sentence meant. |
//! | [`leaves_one_identifier_in_two_owners_alone`] | profile | Injectivity is required within an owner and not across owners: two packages may each carry a test of the same name without colliding, so packages need not coordinate how they name their tests. |
//! | [`covers_only_packages_the_workspace_knows`] | profile | Only tests belonging to a package the workspace knows are covered: a test of an unregistered package contributes no covered asset and no finding, so pointing the checker at a partial tree neither invents coverage nor invents defects. |
//! | [`validates_an_empty_census_cleanly`] | profile | A corpus with no tests in it validates cleanly, reporting nothing covered and no findings, while still carrying every area in its breakdown at zero. An empty count is a measurement rather than a gap. |

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::census::{Census, TestFunction};
use crate::finding::{Finding, MissingLabelReason};
use crate::label::Label;
use crate::workspace::Package;

/// The kind token this profile governs.
///
/// The test profile opens by fixing it, before its census, its classification or
/// its standard place (´[ORCHESTRATION-conv:profiles:test-profile]´), and the calculus
/// makes that the first thing a profile owes: a profile fixes its kind token, and
/// the kind is what puts a label under the profile's derivation rather than under
/// authorship (´[ORCHESTRATION-sig:labels:profiles]´). So the value is not this module's
/// to choose — moving it would move every derived label of the corpus at once.
///
/// ´const:indexlinter:test-profile-token´ (´[ORCHESTRATION-alg:const:word]´)
/// ´const:indexlinter:test-profile-token-word-test´
pub const TEST_KIND: &str = "test";

/// The acute accent that delimits an occurrence in the code syntax.
///
/// The label language gives every occurrence two concrete syntaxes, one for prose
/// and one for code comments, and the code syntax is this character on both sides
/// of the span (´[ORCHESTRATION-lang:labels:label-language]´). Reading a doc comment's
/// ordinary code-font spans instead was the alternative, and it was rejected
/// because participation then wants a fragile label-like heuristic and ordinary
/// documentation makes false mints (´[ORCHESTRATION-ansatz:labels:code-font-delimiters]´).
/// The character is therefore load-bearing rather than decorative: it is what
/// separates a span offered to the calculus from a span merely set in code font.
///
/// ´const:indexlinter:code-syntax-delimiter´ (´[ORCHESTRATION-alg:const:codepoint]´)
/// ´const:indexlinter:code-syntax-delimiter-codepoint-ub4´
pub const ACUTE: char = '\u{b4}';

/// The area a covered test's structural home assigns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// A test written inline in a Rust code file.
    Unit,
    /// A test in a file under the crate's `src/tests/` directory.
    Crate,
    /// A test in a target under the package's top-level `tests/` directory.
    Integration,
}

impl Area {
    /// The area's word, as it stands in a label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Crate => "crate",
            Self::Integration => "integration",
        }
    }

    /// Every area, for reporting.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Unit, Self::Crate, Self::Integration]
    }
}

/// One covered asset with its classification and its derived label.
#[derive(Debug, Clone)]
pub struct CoveredAsset {
    test: TestFunction,
    area: Area,
    label: Label,
}

impl CoveredAsset {
    /// The test function this asset is.
    #[must_use]
    pub const fn test(&self) -> &TestFunction {
        &self.test
    }

    /// The area the classification assigns.
    #[must_use]
    pub const fn area(&self) -> Area {
        self.area
    }

    /// The label the derivation gives.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// The label as it stands at the standard place, in the code syntax.
    #[must_use]
    pub fn standard_place_text(&self) -> String {
        format!("{ACUTE}{}{ACUTE}", self.label)
    }
}

/// Classify a source file by its structural home within its package.
///
/// The package directory is relative to the workspace root, and empty for a
/// root package. Returns `None` for a file under neither censused root, which
/// the census does not produce.
#[must_use]
pub fn classify(package_directory: &Path, path: &Path) -> Option<Area> {
    let relative = path.strip_prefix(package_directory).ok()?;

    if relative.starts_with("tests") {
        return Some(Area::Integration);
    }

    if relative.starts_with("src/tests") {
        return Some(Area::Crate);
    }

    if relative.starts_with("src") {
        return Some(Area::Unit);
    }

    None
}

/// Transform a bare identifier into a label's name segment.
#[must_use]
pub fn transform_name(identifier: &str) -> String {
    identifier.replace('_', "-")
}

/// Derive the label of a covered asset from its area and bare identifier.
#[must_use]
pub fn derive(area: Area, identifier: &str) -> Option<Label> {
    Label::parse(&format!(
        "{TEST_KIND}:{}:{}",
        area.as_str(),
        transform_name(identifier)
    ))
}

/// Classify and derive every asset of a census.
///
/// An identifier that transforms into no well-formed name is reported and the
/// asset carries no label, because there is nothing for it to carry.
#[must_use]
pub fn cover(packages: &[Package], census: &Census) -> (Vec<CoveredAsset>, Vec<Finding>) {
    let directories: BTreeMap<&str, &Path> = packages
        .iter()
        .map(|package| (package.name(), package.directory()))
        .collect();

    let mut assets = Vec::new();
    let mut findings = Vec::new();

    for test in census.tests() {
        let Some(directory) = directories.get(test.package()) else {
            continue;
        };

        let Some(area) = classify(directory, test.path()) else {
            continue;
        };

        match derive(area, test.function()) {
            Some(label) => assets.push(CoveredAsset {
                test: test.clone(),
                area,
                label,
            }),
            None => findings.push(Finding::UnderivableAssetName {
                owner: test.package().to_owned(),
                asset: test.function().to_owned(),
                transformed: transform_name(test.function()),
                location: test.location().clone(),
            }),
        }
    }

    (assets, findings)
}

/// What the profile pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProfileAnalysis {
    /// How many Rust sources the census parsed.
    pub files_scanned: usize,
    /// How many assets the census covers.
    pub covered: usize,
    /// How many covered assets fall in each area.
    pub by_area: BTreeMap<String, usize>,
    /// How many covered assets carry their derived label at the standard place.
    pub labelled: usize,
    /// How many carry no label at the standard place.
    pub missing: usize,
    /// How many carry a label at the standard place that is not their own.
    pub wrong: usize,
    /// How many groups of assets of one owner share a bare identifier.
    pub collision_groups: usize,
    /// How many assets stand in those groups.
    pub colliding_assets: usize,
    /// How many of those groups also derive one label.
    ///
    /// Recorded beside the identifier collisions because ADR-T-015 states the
    /// injectivity requirement of this profile in terms of the identifier,
    /// while the inventory invariant (ADR-T-014, A calculus of documentation and source labels) states
    /// the general one in terms of the label. A group whose assets fall in two
    /// areas breaks the profile's requirement without breaking the general
    /// invariant, so both counts are worth having.
    pub label_collision_groups: usize,
    /// How many labels of this kind stand away from a standard place.
    pub misplaced: usize,
    /// How many identifiers transform into no well-formed name.
    pub underivable: usize,
}

/// Validate every covered asset against the profile's standard place.
#[must_use]
pub fn validate(assets: &[CoveredAsset]) -> (ProfileAnalysis, Vec<Finding>) {
    let mut analysis = ProfileAnalysis::default();
    let mut findings = Vec::new();

    for area in Area::all() {
        analysis.by_area.insert(area.as_str().to_owned(), 0);
    }

    for asset in assets {
        analysis.covered += 1;
        *analysis
            .by_area
            .entry(asset.area.as_str().to_owned())
            .or_default() += 1;

        match check_standard_place(asset) {
            Standing::Carried => analysis.labelled += 1,
            Standing::Missing(reason) => {
                analysis.missing += 1;
                findings.push(Finding::MissingInventoryLabel {
                    label: asset.label.clone(),
                    owner: asset.test.package().to_owned(),
                    asset: asset.test.function().to_owned(),
                    reason,
                    location: asset.test.location().clone(),
                });
            }
            Standing::Wrong(found) => {
                analysis.wrong += 1;
                findings.push(Finding::WrongInventoryLabel {
                    expected: asset.label.clone(),
                    found,
                    owner: asset.test.package().to_owned(),
                    asset: asset.test.function().to_owned(),
                    location: asset.test.location().clone(),
                });
            }
        }

        for label in misplaced_labels(asset) {
            analysis.misplaced += 1;
            findings.push(Finding::MisplacedInventoryLabel {
                label,
                owner: asset.test.package().to_owned(),
                asset: asset.test.function().to_owned(),
                location: asset.test.location().clone(),
            });
        }
    }

    collisions(assets, &mut analysis, &mut findings);

    (analysis, findings)
}

/// How a covered asset stands with respect to its standard place.
enum Standing {
    /// The final documentation line is exactly the derived label.
    Carried,
    /// The final documentation line carries no label of this profile.
    Missing(MissingLabelReason),
    /// The final documentation line carries some other label.
    Wrong(Label),
}

/// Read the standard place of one covered asset.
fn check_standard_place(asset: &CoveredAsset) -> Standing {
    let Some(doc) = asset.test.doc() else {
        return Standing::Missing(MissingLabelReason::NoDocComment);
    };

    let Some(final_line) = doc.final_line() else {
        return Standing::Missing(MissingLabelReason::NoDocComment);
    };

    if final_line == asset.standard_place_text() {
        return Standing::Carried;
    }

    if let Some(interior) = sole_code_span(final_line) {
        return Label::parse(interior).map_or(
            Standing::Missing(MissingLabelReason::NoLabelOnFinalLine),
            Standing::Wrong,
        );
    }

    let carries_a_label = code_spans(final_line)
        .iter()
        .any(|span| Label::parse(span.interior).is_some());

    if carries_a_label {
        Standing::Missing(MissingLabelReason::LabelWithExtraText)
    } else {
        Standing::Missing(MissingLabelReason::NoLabelOnFinalLine)
    }
}

/// Every label of this profile's kind minted away from the standard place.
fn misplaced_labels(asset: &CoveredAsset) -> Vec<Label> {
    let Some(doc) = asset.test.doc() else {
        return Vec::new();
    };

    let lines = doc.lines();
    let Some((_final_line, earlier)) = lines.split_last() else {
        return Vec::new();
    };

    earlier
        .iter()
        .flat_map(|line| code_spans(line))
        .filter(|span: &CodeSpan<'_>| !span.parenthesized)
        .filter_map(|span| Label::parse(span.interior))
        .filter(|label| label.kind() == TEST_KIND)
        .collect()
}

/// Report every group of assets of one owner that share a bare identifier.
fn collisions(
    assets: &[CoveredAsset],
    analysis: &mut ProfileAnalysis,
    findings: &mut Vec<Finding>,
) {
    let mut groups: BTreeMap<(&str, &str), Vec<&CoveredAsset>> = BTreeMap::new();

    for asset in assets {
        groups
            .entry((asset.test.package(), asset.test.function()))
            .or_default()
            .push(asset);
    }

    for ((owner, name), group) in groups {
        let Some((first, rest)) = group.split_first() else {
            continue;
        };

        if rest.is_empty() {
            continue;
        }

        analysis.collision_groups += 1;
        analysis.colliding_assets += group.len();

        if rest.iter().all(|other| other.label == first.label) {
            analysis.label_collision_groups += 1;
        }

        for other in rest {
            findings.push(Finding::CollidingDerivation {
                asset: name.to_owned(),
                owner: owner.to_owned(),
                first_label: first.label.clone(),
                second_label: other.label.clone(),
                first: first.test.location().clone(),
                second: other.test.location().clone(),
            });
        }
    }
}

/// One acute-delimited span of a documentation line.
pub struct CodeSpan<'a> {
    /// The text between the two acutes.
    pub interior: &'a str,
    /// Whether the span stands immediately inside a parenthesis pair.
    pub parenthesized: bool,
    /// The byte offset of the opening acute within the line.
    pub start: usize,
    /// The byte offset one past the closing acute within the line.
    pub end: usize,
}

/// Every acute-delimited span of a line, paired left to right.
///
/// An acute left open at the end of the line closes nothing and yields no span.
/// The hard failure that an unclosed acute is belongs to the code carrier's
/// scanner and is not written yet: the carrier reads what pairs and reports what
/// dangles, and an unpaired delimiter is a later bite, the prose side's
/// treatment of an unpaired backtick being the shape to follow.
pub fn code_spans(line: &str) -> Vec<CodeSpan<'_>> {
    let mut spans = Vec::new();
    let mut open = None;

    for (index, character) in line.char_indices() {
        if character != ACUTE {
            continue;
        }

        match open.take() {
            None => open = Some(index),
            Some(start) => {
                let interior = &line[start + ACUTE.len_utf8()..index];
                let before = line[..start].chars().next_back();
                let after = line[index + ACUTE.len_utf8()..].chars().next();

                spans.push(CodeSpan {
                    interior,
                    parenthesized: before == Some('(') && after == Some(')'),
                    start,
                    end: index + ACUTE.len_utf8(),
                });
            }
        }
    }

    spans
}

/// The interior of a line that is exactly one acute-delimited span.
fn sole_code_span(line: &str) -> Option<&str> {
    let mut characters = line.chars();

    if characters.next()? != ACUTE || characters.next_back()? != ACUTE {
        return None;
    }

    let interior = &line[ACUTE.len_utf8()..line.len() - ACUTE.len_utf8()];

    (!interior.contains(ACUTE)).then_some(interior)
}

/// Run the profile pass over a workspace.
#[must_use]
pub fn analyze_profile(
    packages: &[Package],
    census: &Census,
) -> (ProfileAnalysis, Vec<CoveredAsset>, Vec<Finding>) {
    let (assets, mut findings) = cover(packages, census);
    let (mut analysis, validation) = validate(&assets);

    analysis.files_scanned = census.files_scanned();
    analysis.underivable = findings.len();
    findings.extend(validation);
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, assets, findings)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        Area, ProfileAnalysis, TEST_KIND, classify, cover, derive, transform_name, validate,
    };
    use crate::census::{Census, scan_source};
    use crate::finding::{Finding, MissingLabelReason};
    use crate::workspace::Package;

    /// Build a one-package census from sources given as path and text.
    fn census_of(sources: &[(&str, &str)]) -> (Vec<Package>, Census) {
        let packages = vec![Package::new("torrust-demo", "packages/demo")];
        let mut tests = Vec::new();

        for (path, text) in sources {
            tests
                .extend(scan_source("torrust-demo", Path::new(path), text).expect("a Rust source"));
        }

        (packages, Census::from_tests(tests, sources.len()))
    }

    fn analyze(sources: &[(&str, &str)]) -> (ProfileAnalysis, Vec<Finding>) {
        let (packages, census) = census_of(sources);
        let (analysis, _assets, findings) = super::analyze_profile(&packages, &census);

        (analysis, findings)
    }

    /// A test's area is decided by where its file sits in the package: an
    /// ordinary source is a unit test, the package's crate-test directory holds
    /// crate tests, and its integration directory holds integration tests. A
    /// label's area therefore says which surface the test exercises.
    ///
    /// ´claim:profile:a-tests-area-is-decided-by-where-its-file-sits´
    /// ´test:unit:classifies-the-three-areas´
    #[test]
    fn classifies_the_three_areas() {
        let directory = PathBuf::from("packages/demo");

        assert_eq!(
            classify(&directory, Path::new("packages/demo/src/engine.rs")),
            Some(Area::Unit)
        );
        assert_eq!(
            classify(&directory, Path::new("packages/demo/src/tests/engine.rs")),
            Some(Area::Crate)
        );
        assert_eq!(
            classify(&directory, Path::new("packages/demo/tests/engine.rs")),
            Some(Area::Integration)
        );
    }

    /// A module file is classified by the directory holding it like any other
    /// source, so the file that declares a crate-test tree is itself a
    /// crate-test source rather than a special case.
    ///
    /// (´claim:profile:a-tests-area-is-decided-by-where-its-file-sits´)
    /// ´test:unit:classifies-a-module-file-under-the-crate-tests-directory´
    #[test]
    fn classifies_a_module_file_under_the_crate_tests_directory() {
        let directory = PathBuf::from("packages/demo");

        assert_eq!(
            classify(&directory, Path::new("packages/demo/src/tests/mod.rs")),
            Some(Area::Crate)
        );
    }

    /// Nesting inside a file changes nothing: a test written in a module within
    /// a crate-test source is a crate test, because the area follows the file
    /// and not the module path above the function.
    ///
    /// ´claim:profile:module-nesting-inside-a-file-does-not-change-the-area´
    /// ´test:unit:classifies-a-nested-module-by-its-file-not-its-nesting´
    #[test]
    fn classifies_a_nested_module_by_its_file_not_its_nesting() {
        let (analysis, _findings) = analyze(&[(
            "packages/demo/src/tests/engine.rs",
            "#[cfg(test)]\nmod inner {\n    #[test]\n    fn nested() {}\n}\n",
        )]);

        assert_eq!(analysis.by_area["crate"], 1);
        assert_eq!(analysis.by_area["unit"], 0);
    }

    /// Only the package's own crate-test directory names crate tests: a
    /// directory of the same name nested deeper inside the sources holds unit
    /// tests, so a module may organise its tests into a folder without
    /// reclassifying them.
    ///
    /// ´claim:profile:only-the-packages-own-test-directories-name-their-areas´
    /// ´test:unit:classifies-a-deeper-tests-directory-as-unit´
    #[test]
    fn classifies_a_deeper_tests_directory_as_unit() {
        let directory = PathBuf::from("packages/demo");

        assert_eq!(
            classify(
                &directory,
                Path::new("packages/demo/src/maths/tests/brand.rs")
            ),
            Some(Area::Unit)
        );
    }

    /// A label's name is the function's identifier with its word separators
    /// rewritten to the ones the grammar admits, and a single-word identifier
    /// passes through as it is. The transformation is that and nothing more.
    ///
    /// ´claim:profile:a-name-is-the-identifier-with-its-separators-rewritten´
    /// ´test:unit:transforms-underscores-into-hyphens´
    #[test]
    fn transforms_underscores_into_hyphens() {
        assert_eq!(
            transform_name("empty_chain_returns_zeros"),
            "empty-chain-returns-zeros"
        );
        assert_eq!(transform_name("single"), "single");
    }

    /// A test's whole label is derived from its area and its identifier, kind
    /// included, so the label is a fact about the test rather than a name
    /// anybody chose for it.
    ///
    /// ´claim:profile:a-tests-whole-label-is-derived-from-its-area-and-identifier´
    /// ´test:unit:derives-the-label-of-the-records-example´
    #[test]
    fn derives_the_label_of_the_records_example() {
        let label = derive(Area::Crate, "empty_chain_returns_zeros").expect("a well-formed label");

        assert_eq!(label.to_string(), "test:crate:empty-chain-returns-zeros");
        assert_eq!(label.kind(), TEST_KIND);
    }

    /// An identifier that does not transform into a well-formed name derives no
    /// label: capitals, a leading or trailing separator, and a doubled one are
    /// each refused rather than repaired into something the grammar would
    /// accept.
    ///
    /// ´claim:profile:an-identifier-that-is-no-well-formed-name-derives-nothing´
    /// ´test:unit:refuses-an-identifier-that-is-no-label-name´
    #[test]
    fn refuses_an_identifier_that_is_no_label_name() {
        assert!(derive(Area::Unit, "Capitalised").is_none());
        assert!(derive(Area::Unit, "_leading").is_none());
        assert!(derive(Area::Unit, "trailing_").is_none());
        assert!(derive(Area::Unit, "double__underscore").is_none());
    }

    /// A test carrying its derived label as the final line of its documentation
    /// is clean, and is counted as both covered and labelled. This is what a
    /// conforming test looks like.
    ///
    /// ´claim:profile:a-test-carrying-its-derived-label-last-is-clean´
    /// ´test:unit:accepts-the-derived-label-at-the-standard-place´
    #[test]
    fn accepts_the_derived_label_at_the_standard_place() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// Checks the thing.\n/// \u{b4}test:unit:checks-the-thing\u{b4}\n#[test]\nfn checks_the_thing() {}\n",
        )]);

        assert_eq!(findings.len(), 0, "a labelled test is clean: {findings:?}");
        assert_eq!(analysis.labelled, 1);
        assert_eq!(analysis.covered, 1);
    }

    /// A covered test not carrying its label at the standard place is reported
    /// with the label it should have had and with the reason it is missing, so
    /// the author is told both what to write and why it is wanted. Having no
    /// documentation at all is one such reason.
    ///
    /// ´claim:profile:a-missing-label-is-reported-with-its-derivation-and-its-reason´
    /// ´test:unit:reports-a-test-without-documentation´
    #[test]
    fn reports_a_test_without_documentation() {
        let (analysis, findings) =
            analyze(&[("packages/demo/src/engine.rs", "#[test]\nfn bare() {}\n")]);

        assert_eq!(analysis.missing, 1);
        let [Finding::MissingInventoryLabel { reason, label, .. }] = findings.as_slice() else {
            panic!("expected one missing label, got {findings:?}");
        };
        assert_eq!(*reason, MissingLabelReason::NoDocComment);
        assert_eq!(label.to_string(), "test:unit:bare");
    }

    /// Documentation that ends in prose rather than in the label is the second
    /// way of missing it, and is reported as that distinct reason.
    ///
    /// (´claim:profile:a-missing-label-is-reported-with-its-derivation-and-its-reason´)
    /// ´test:unit:reports-documentation-whose-final-line-is-not-the-label´
    #[test]
    fn reports_documentation_whose_final_line_is_not_the_label() {
        let (_analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// Checks the thing.\n#[test]\nfn bare() {}\n",
        )]);

        let [Finding::MissingInventoryLabel { reason, .. }] = findings.as_slice() else {
            panic!("expected one missing label, got {findings:?}");
        };
        assert_eq!(*reason, MissingLabelReason::NoLabelOnFinalLine);
    }

    /// A label sharing its final line with other words is the third way, named
    /// as its own reason: the standard place holds the label alone.
    ///
    /// (´claim:profile:a-missing-label-is-reported-with-its-derivation-and-its-reason´)
    /// ´test:unit:reports-a-label-with-extra-text-after-it´
    #[test]
    fn reports_a_label_with_extra_text_after_it() {
        let (_analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// \u{b4}test:unit:bare\u{b4} and more words\n#[test]\nfn bare() {}\n",
        )]);

        let [Finding::MissingInventoryLabel { reason, .. }] = findings.as_slice() else {
            panic!("expected one missing label, got {findings:?}");
        };
        assert_eq!(*reason, MissingLabelReason::LabelWithExtraText);
    }

    /// A label at the standard place attests the derivation rather than naming
    /// the test: one that is not what the test derives is reported showing both
    /// what stands there and what the test actually gives, so renaming a
    /// function and forgetting its label is caught.
    ///
    /// ´claim:profile:a-label-that-is-not-the-derivation-is-reported-with-both´
    /// ´test:unit:reports-a-label-that-is-not-the-derivation´
    #[test]
    fn reports_a_label_that_is_not_the_derivation() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// \u{b4}test:unit:something-else\u{b4}\n#[test]\nfn bare() {}\n",
        )]);

        assert_eq!(analysis.wrong, 1);
        let [
            Finding::WrongInventoryLabel {
                expected, found, ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one wrong label, got {findings:?}");
        };
        assert_eq!(expected.to_string(), "test:unit:bare");
        assert_eq!(found.to_string(), "test:unit:something-else");
    }

    /// A second label of the derived kind minted elsewhere in the same comment
    /// is reported as misplaced while the one at the standard place still
    /// counts, so a derived kind is minted at its own place and nowhere else.
    ///
    /// ´claim:profile:a-derived-label-minted-away-from-the-standard-place-is-misplaced´
    /// ´test:unit:reports-a-label-of-this-kind-away-from-the-standard-place´
    #[test]
    fn reports_a_label_of_this_kind_away_from_the_standard_place() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// \u{b4}test:unit:elsewhere\u{b4}\n/// \u{b4}test:unit:bare\u{b4}\n#[test]\nfn bare() {}\n",
        )]);

        assert_eq!(analysis.labelled, 1, "the standard place is still right");
        assert_eq!(analysis.misplaced, 1);
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, Finding::MisplacedInventoryLabel { .. })),
            "expected a misplaced label, got {findings:?}"
        );
    }

    /// Citing is not minting: a test's documentation may refer to another test
    /// by its label without that being a misplaced mint, so a comment can point
    /// at a sibling while carrying its own label below.
    ///
    /// ´claim:profile:citing-a-derived-label-in-a-comment-is-not-minting-one´
    /// ´test:unit:leaves-a-citation-away-from-the-standard-place-alone´
    #[test]
    fn leaves_a_citation_away_from_the_standard_place_alone() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/engine.rs",
            "/// See (\u{b4}test:unit:elsewhere\u{b4}).\n/// \u{b4}test:unit:bare\u{b4}\n#[test]\nfn bare() {}\n",
        )]);

        assert_eq!(analysis.misplaced, 0);
        assert_eq!(findings.len(), 0, "a citation is not a mint: {findings:?}");
    }

    /// The derivation must be injective within an owner: two tests of one name
    /// in one package collide, and the report names the identifier and both
    /// files, so a citation of that label can never be ambiguous about which
    /// test it reached.
    ///
    /// ´claim:profile:two-tests-deriving-one-label-collide-with-both-locations´
    /// ´test:unit:reports-a-collision-with-both-locations´
    #[test]
    fn reports_a_collision_with_both_locations() {
        let (analysis, findings) = analyze(&[
            (
                "packages/demo/src/one.rs",
                "/// \u{b4}test:unit:shared\u{b4}\n#[test]\nfn shared() {}\n",
            ),
            (
                "packages/demo/src/two.rs",
                "/// \u{b4}test:unit:shared\u{b4}\n#[test]\nfn shared() {}\n",
            ),
        ]);

        assert_eq!(analysis.collision_groups, 1);
        assert_eq!(analysis.colliding_assets, 2);
        assert_eq!(
            analysis.label_collision_groups, 1,
            "one area, so one label too"
        );

        let [
            Finding::CollidingDerivation {
                asset,
                first,
                second,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one collision, got {findings:?}");
        };

        assert_eq!(asset, "shared");
        assert_eq!(first.path(), Path::new("packages/demo/src/one.rs"));
        assert_eq!(second.path(), Path::new("packages/demo/src/two.rs"));
    }

    /// Collision is judged on the identifier rather than on the finished label:
    /// two tests sharing a name in different areas collide even though their
    /// labels differ, because a reader searching for the name would find two
    /// tests and no way to tell which a sentence meant.
    ///
    /// ´claim:profile:collision-is-judged-on-the-identifier-not-the-finished-label´
    /// ´test:unit:collides-on-the-identifier-even-when-the-areas-differ´
    #[test]
    fn collides_on_the_identifier_even_when_the_areas_differ() {
        let (analysis, findings) = analyze(&[
            (
                "packages/demo/src/one.rs",
                "/// \u{b4}test:unit:shared\u{b4}\n#[test]\nfn shared() {}\n",
            ),
            (
                "packages/demo/tests/two.rs",
                "/// \u{b4}test:integration:shared\u{b4}\n#[test]\nfn shared() {}\n",
            ),
        ]);

        assert_eq!(analysis.collision_groups, 1, "the identifiers are the same");
        assert_eq!(
            analysis.label_collision_groups, 0,
            "but the labels differ by area"
        );

        let [
            Finding::CollidingDerivation {
                first_label,
                second_label,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one collision, got {findings:?}");
        };

        assert_eq!(first_label.to_string(), "test:unit:shared");
        assert_eq!(second_label.to_string(), "test:integration:shared");
    }

    /// Injectivity is required within an owner and not across owners: two
    /// packages may each carry a test of the same name without colliding, so
    /// packages need not coordinate how they name their tests.
    ///
    /// ´claim:profile:test-derivations-collide-only-within-one-owner´
    /// ´test:unit:leaves-one-identifier-in-two-owners-alone´
    #[test]
    fn leaves_one_identifier_in_two_owners_alone() {
        let packages = vec![
            Package::new("torrust-one", "packages/one"),
            Package::new("torrust-two", "packages/two"),
        ];
        let mut tests = scan_source(
            "torrust-one",
            Path::new("packages/one/src/a.rs"),
            "/// \u{b4}test:unit:shared\u{b4}\n#[test]\nfn shared() {}\n",
        )
        .expect("a Rust source");
        tests.extend(
            scan_source(
                "torrust-two",
                Path::new("packages/two/src/a.rs"),
                "/// \u{b4}test:unit:shared\u{b4}\n#[test]\nfn shared() {}\n",
            )
            .expect("a Rust source"),
        );

        let census = Census::from_tests(tests, 2);
        let (analysis, _assets, findings) = super::analyze_profile(&packages, &census);

        assert_eq!(analysis.collision_groups, 0, "ownership disambiguates");
        assert_eq!(findings.len(), 0, "and nothing is reported: {findings:?}");
    }

    /// Only tests belonging to a package the workspace knows are covered: a
    /// test of an unregistered package contributes no covered asset and no
    /// finding, so pointing the checker at a partial tree neither invents
    /// coverage nor invents defects.
    ///
    /// ´claim:profile:only-tests-of-a-known-package-are-covered´
    /// ´test:unit:covers-only-packages-the-workspace-knows´
    #[test]
    fn covers_only_packages_the_workspace_knows() {
        let packages = vec![Package::new("torrust-other", "packages/other")];
        let tests = scan_source(
            "torrust-demo",
            Path::new("packages/demo/src/a.rs"),
            "#[test]\nfn a() {}\n",
        )
        .expect("a Rust source");
        let census = Census::from_tests(tests, 1);

        let (assets, findings) = cover(&packages, &census);

        assert!(
            assets.is_empty(),
            "an unknown package covers nothing: {assets:?}"
        );
        assert!(
            findings.is_empty(),
            "and reports nothing here: {findings:?}"
        );
    }

    /// A corpus with no tests in it validates cleanly, reporting nothing
    /// covered and no findings, while still carrying every area in its
    /// breakdown at zero. An empty count is a measurement rather than a gap.
    ///
    /// ´claim:profile:an-empty-census-validates-cleanly-with-every-area-at-zero´
    /// ´test:unit:validates-an-empty-census-cleanly´
    #[test]
    fn validates_an_empty_census_cleanly() {
        let (analysis, findings) = validate(&[]);

        assert_eq!(analysis.covered, 0);
        assert_eq!(findings.len(), 0);
        assert_eq!(analysis.by_area["unit"], 0);
    }
}
