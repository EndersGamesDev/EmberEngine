// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The claim profile of ADR-T-017: the statement a test establishes, said once,
//! where the test is.
//!
//! The kind this module reads is `claim`, and the registry of ADR-T-011
//! catalogues it under the Assertion and Claim rows of the results convention.
//! The kind is therefore outside the reserved set, so a claim label stands on an
//! authorship warrant and on nothing else: the author of a test chooses what the
//! test establishes and what to call that statement, and no rule computes it.
//! That is the exact opposite of the ruling for the derived test label beside it,
//! and the two do not conflict — the derived label names the function, the
//! authored label names the statement the function establishes, and one asset
//! carries one facet of each.
//!
//! # The two labels, in one fixed order
//!
//! A covered test's documentation comment carries the gloss, then the claim, then
//! the derived test label, and the last of those is the final line exactly as
//! ADR-T-015 already fixes. The claim goes above it and never after it, so a
//! claim written last is a defect of placement reported as one.
//!
//! A claim occurrence is one of two forms. A mint stands bare in the code syntax
//! of the calculus and names a statement this test is the first to establish. A
//! citation stands parenthesised and names a statement a sibling test already
//! minted, which is what turns shared coverage into a mechanical fact: a scenario
//! covered by five tests is one mint and four citations, and the count is a query
//! rather than a reading of five files.
//!
//! # What is staged and what is not
//!
//! The scale of the corpus is the reason the claims are staged and the reason the
//! staging is a coverage figure rather than a new register. A covered test with no
//! claim line is counted here and reported nowhere, until the commit that closes
//! its package's authoring wave activates that package's owner for the claims
//! profile; from that commit the package is held to the whole requirement.
//!
//! A claim that is written is held to everything from its first commit. The
//! staging concerns the claims not yet written and nothing else, so a citation
//! resolving nowhere, a collision, a malformed line, or a label out of place is a
//! failure whether it was written during the staging or after it.
//!
//! # Areas start free
//!
//! A claim's area is thematic: it names what the statement is about, not where the
//! test lives, the structural home being carried already by the derived label's
//! own area. The vocabulary starts free and this module censuses it, which is the
//! measurement the area register is curated from. Fixing the set before the
//! statements are written would fix it from a taxonomy that has met no sentence.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_a_mint_above_the_test_label`] | claim | A claim standing bare on the line above the derived test label is a mint, and the prose above it is read as the statement it names. The three parts — gloss, claim, derived label — are recovered from one documentation comment in the order the convention fixes them. |
//! | [`reads_a_citation_above_the_test_label`] | claim | The same line parenthesised is a citation instead, and a citing test needs no gloss of its own: the statement is said where it was minted, which is the whole point of citing rather than restating it. |
//! | [`joins_a_wrapped_gloss_into_one_line`] | claim | A gloss wrapped across several documentation lines joins into one line for projection, because a documentation comment wraps where its author wrapped it and a table cell cannot. The author writes prose; the generator gets a sentence. |
//! | [`counts_a_test_with_no_claim_rather_than_reporting_it`] | claim | A covered test carrying no claim is counted and reported nowhere, and counted against its own package. That is the whole of the staging: the scale of the corpus is met with a coverage figure rather than with thousands of findings nobody can act on at once. |
//! | [`reports_a_claim_written_last`] | claim | A claim written below the derived test label is a placement defect: the derived label is the final line, and a claim after it is reported rather than read. |
//! | [`reports_a_claim_above_the_gloss`] | claim | Any other placement is a defect too: a claim standing above its own gloss is misplaced, because the standard place is the line directly above the derived label and nowhere else. |
//! | [`reports_a_claim_sharing_its_line`] | claim | A claim sharing its line with other words is a defect: the standard place holds the occurrence and nothing else, so a label mentioned in passing inside a sentence is never mistaken for the test's own claim. |
//! | [`reports_two_claims_in_one_comment`] | claim | One test carries one claim: a comment holding two is reported as repeated rather than having one of them chosen. A test establishing two statements is a test to split, not a comment to stack labels in. |
//! | [`resolves_a_citation_of_a_sibling_mint`] | claim | Shared coverage becomes a mechanical fact: a statement two tests establish is one mint and one citation, both resolving cleanly. A scenario covered many times over is therefore a query rather than a reading of many files. |
//! | [`resolves_a_citation_of_a_mint_standing_below_it`] | claim | Resolution is staged, so the order a corpus is read in decides nothing: every mint is entered before any citation is asked to resolve, and a test may cite a sibling standing below it in the same file. |
//! | [`fails_a_citation_of_a_claim_nobody_minted`] | claim | Resolution is total: a citation of a statement nobody minted is a failure naming the label and the test that wrote it, so a claim cannot be referred to into existence. |
//! | [`fails_two_mints_of_one_claim_with_both_locations`] | claim | A statement is minted once within an owner: minting it twice is a failure carrying both locations, so the author is shown the pair to choose between rather than told a name is taken. |
//! | [`leaves_one_claim_minted_in_two_owners_alone`] | claim | Uniqueness is per owner, so two packages may mint the same claim name without colliding: ownership disambiguates, and neither package needs to know what the other has named its statements. |
//! | [`censuses_the_area_vocabulary`] | claim | The area vocabulary is censused rather than fixed: how many areas are in use and how many claims stand in each is measured from the claims actually written. Fixing the set first would fix it from a taxonomy that had met no sentence. |
//! | [`holds_a_closed_package_to_the_whole_requirement`] | claim | The staging is the set of packages whose waves have closed and nothing more: one claimless test is counted either way, reported nowhere while its package's wave is open, and held to the whole requirement from the moment that package joins the set. The number held is exactly the size of the set the caller passed, so the pass carries no roster of its own to disagree with the corpus. |
//! | [`closes_the_waves_the_retired_roster_named`] | claim | The set the check closes is exactly the fixture roster: its member names derive the owners activated for the claims profile in both directions, unrelated activations do not join the set, and the derivation carries every member back into the closed set. |
//! | [`leaves_a_claimless_corpus_clean`] | claim | A corpus in which nothing is claimed yet is clean: its tests are counted, no area is in use, and nothing is reported. The machinery can therefore land before a single claim is written. |

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::census::DocComment;
use crate::finding::Finding;
use crate::label::Label;
use crate::occurrence::Syntax;
use crate::profile::{ACUTE, CodeSpan, CoveredAsset, code_spans};
use crate::roster::OwnerNames;
use crate::snapshot::Pair;
use crate::workspace::Package;

/// The kind token this profile reads.
///
/// The record fixes the form a claim label takes, and this is its kind: the
/// claim label stands below the gloss, in the code syntax of the calculus, of a
/// form whose first token is this one (´[EMBER-conv:testdocs:two-labels]´).
///
/// ´const:emberlinter:claim-kind-token´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:claim-kind-token-word-claim´
pub const CLAIM_KIND: &str = "claim";

/// The activation an owner's closed wave is stated by.
const CLAIMS_POLICY: &str = "profile.claims-conform";

/// Which form a claim occurrence took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimForm {
    /// A bare occurrence: this test is the first to establish the statement.
    Mint,
    /// A parenthesised occurrence of a statement a sibling test minted.
    Citation,
}

/// The claim line one covered test carries, with the gloss standing above it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimLine {
    label: Label,
    form: ClaimForm,
    gloss: String,
}

impl ClaimLine {
    /// The claim label, minted or cited.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// Which form the occurrence took.
    #[must_use]
    pub const fn form(&self) -> ClaimForm {
        self.form
    }

    /// Whether this test minted the statement rather than citing it.
    #[must_use]
    pub fn is_mint(&self) -> bool {
        self.form == ClaimForm::Mint
    }

    /// The statement the test establishes, as one line.
    ///
    /// The gloss is the prose standing above the claim in the same documentation
    /// comment, joined into one line: a documentation comment wraps where its
    /// author wrapped it, and a projection into a table cell cannot.
    #[must_use]
    pub fn gloss(&self) -> &str {
        &self.gloss
    }
}

/// Why a documentation comment's claim line is not one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimDefect {
    /// The claim stands somewhere other than directly above the test label.
    Misplaced,
    /// The claim shares its line with other words.
    NotAloneOnItsLine,
    /// The documentation comment carries more than one claim label.
    Repeated,
}

impl std::fmt::Display for ClaimDefect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Misplaced => "it does not stand directly above the derived test label",
            Self::NotAloneOnItsLine => "it shares its line with other words",
            Self::Repeated => "the documentation comment carries more than one claim label",
        })
    }
}

/// What one covered test's documentation comment says about its claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// No claim label stands in the comment at all.
    Unclaimed,
    /// Exactly one claim label stands at the standard place.
    Claimed(ClaimLine),
    /// A claim label stands, and is not the convention's.
    Defective(ClaimDefect),
}

/// Read one covered test's claim standing from its documentation comment.
///
/// The standard place is the line directly above the derived test label, which is
/// itself the final line. A comment too short to hold both is a comment holding no
/// claim, whatever else may be wrong with it: the missing test label is the test
/// profile's finding and restating it here would report one defect twice.
#[must_use]
pub fn read_claim(asset: &CoveredAsset) -> Standing {
    let Some(doc) = asset.test().doc() else {
        return Standing::Unclaimed;
    };

    let occurrences = claim_occurrences(doc);

    let [(line, label, parenthesized)] = occurrences.as_slice() else {
        return if occurrences.is_empty() {
            Standing::Unclaimed
        } else {
            Standing::Defective(ClaimDefect::Repeated)
        };
    };

    let lines = doc.lines();

    // The claim stands directly above the final line, and the final line is the
    // derived test label's own. A claim on the final line is the placement defect
    // the record names in as many words: a claim written last.
    let Some(standard_place) = lines.len().checked_sub(2) else {
        return Standing::Defective(ClaimDefect::Misplaced);
    };

    if *line != standard_place {
        return Standing::Defective(ClaimDefect::Misplaced);
    }

    let text = lines[*line].as_str();
    let form = if *parenthesized {
        ClaimForm::Citation
    } else {
        ClaimForm::Mint
    };

    if !is_sole_occurrence(text, label, *parenthesized) {
        return Standing::Defective(ClaimDefect::NotAloneOnItsLine);
    }

    Standing::Claimed(ClaimLine {
        label: label.clone(),
        form,
        gloss: gloss_above(lines, *line),
    })
}

/// Every claim-kind occurrence of a documentation comment, in order.
fn claim_occurrences(doc: &DocComment) -> Vec<(usize, Label, bool)> {
    doc.lines()
        .iter()
        .enumerate()
        .flat_map(|(index, line)| {
            code_spans(line)
                .into_iter()
                .filter_map(move |span: CodeSpan<'_>| {
                    Label::parse(span.interior)
                        .filter(|label| label.kind() == CLAIM_KIND)
                        .map(|label| (index, label, span.parenthesized))
                })
                .collect::<Vec<(usize, Label, bool)>>()
        })
        .collect()
}

/// Whether a documentation line's sole occurrence is claim-kind, either form.
///
/// The one recognizer every consumer of the standard place shares
/// (ADR-T-020, The migration disciplines): the code lint deciding which
/// lines a profile owns, and the fix mode deciding where the paragraph break
/// belongs, both ask this rather than each judging a claim line for itself. A
/// writer carrying a private copy of this judgment is exactly how the fix mode
/// once drifted from the reader beside it, and one predicate is what keeps the
/// two from drifting again.
#[must_use]
pub fn line_holds_claim(line: &str) -> bool {
    let spans = code_spans(line);

    let [span] = spans.as_slice() else {
        return false;
    };

    Label::parse(span.interior).is_some_and(|label| label.kind() == CLAIM_KIND)
}

/// Whether a line is exactly the claim occurrence and nothing else.
fn is_sole_occurrence(line: &str, label: &Label, parenthesized: bool) -> bool {
    let acute = crate::profile::ACUTE;
    let bare = format!("{acute}{label}{acute}");

    if parenthesized {
        line == format!("({bare})")
    } else {
        line == bare
    }
}

/// The gloss standing above a claim line: the prose, joined into one line.
///
/// Blank documentation lines separate the gloss from the claim below it and are
/// dropped; a blank line inside the prose joins its two halves with one space,
/// because a table cell has one line to say the statement in.
fn gloss_above(lines: &[String], claim: usize) -> String {
    lines[..claim]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

/// The placeholder a projection writes where a test has no claim yet.
///
/// An em dash rather than an empty cell: a reader scanning the index sees that
/// the statement is unwritten rather than that the generator lost it, and the
/// coverage figure the staging turns on is the same fact counted.
///
/// TODO ´todo:code:the-record-fixes-that-an-unclaimed´: the record fixes that an unclaimed cell carries a placeholder, but no
/// record fixes which character it is. ADR-T-012 marks an absent entry of its
/// own class column with an em dash, which is a precedent rather than a rule
/// reaching this table; promoting it to one is a record edit, not a code edit.
///
/// ´const:emberlinter:unclaimed-cell-placeholder´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:unclaimed-cell-placeholder-text-x8850a3f1´
pub const NO_CLAIM_YET: &str = "\u{2014}";

/// The header row of either generated table, recognised by exact equality.
///
/// The record displays both projections under the same three cells, and it is
/// one shape rather than two that happen to agree: the Test cell names the test,
/// and the Area and Claim cells are the ones this module projects for either
/// surface (´[EMBER-conv:testdocs:file-index]´). What differs between the
/// surfaces is the concrete syntax a Test cell is written in, which the row
/// heading it does not state. Recognition by exact equality is what lets an
/// author write a paragraph above the table without the generator losing track
/// of its own region.
///
/// ´const:emberlinter:test-table-header-row´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:test-table-header-row-text-x0c2d3caa´
pub const TABLE_HEADER: &str = "| Test | Area | Claim |";

/// The delimiter row the generator writes below the header.
///
/// It stands under the header in the table the record displays, and its dashes
/// are laid to that display's own widths: the region is compared byte for byte,
/// so a row laid to other widths reads as staleness rather than as a variation
/// (´[EMBER-conv:testdocs:folder-matrix]´). One header admits one delimiter, so
/// the two rows are declared and moved together.
///
/// ´const:emberlinter:test-table-delimiter-row´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:test-table-delimiter-row-text-x470227c8´
pub const TABLE_DELIMITER: &str = "|------|------|-------|";

/// The Area and Claim cells one covered test projects into either table.
///
/// Both generated projections carry the same two cells and differ only in how
/// they name the test, so the cells are computed once here: a reader comparing a
/// file's index with its folder's matrix must never find them saying different
/// things about one test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedCells {
    /// The claim's area, or empty where no claim stands yet.
    pub area: String,
    /// The gloss, the citation, or the placeholder.
    pub claim: String,
}

/// Project one covered test's Area and Claim cells, for the given surface.
///
/// A citing test renders its Claim cell as a real citation of the statement, so
/// a reader sees at once that the statement is established elsewhere and where
/// — and the checker holds the reference to the corpus as it actually is. The
/// generator writes the form it means, in the concrete syntax of the surface
/// the cell will stand on: the generated-compliance invariant
/// (ADR-T-014, A calculus of documentation and source labels) makes a generated citation
/// resolve like any other, and the assets caveat (ADR-T-014, A calculus of documentation and source labels)
/// says a register's rows are citations like any others. The word naming the
/// relation stays outside the citation's parenthesis, which hugs the label
/// alone. The display form survives only for tokens genuinely shown rather than
/// meant, and this cell means its label.
#[must_use]
pub fn project_cells(asset: &CoveredAsset, syntax: Syntax) -> ProjectedCells {
    match read_claim(asset) {
        Standing::Claimed(claim) if claim.is_mint() => ProjectedCells {
            area: claim.label.area().to_owned(),
            claim: if claim.gloss.is_empty() {
                NO_CLAIM_YET.to_owned()
            } else {
                escape_cell(&claim.gloss)
            },
        },
        Standing::Claimed(claim) => ProjectedCells {
            area: claim.label.area().to_owned(),
            claim: match syntax {
                Syntax::Prose => format!("cites (`{}`)", claim.label),
                Syntax::Code => format!("cites ({ACUTE}{}{ACUTE})", claim.label),
            },
        },
        Standing::Unclaimed | Standing::Defective(_) => ProjectedCells {
            area: String::new(),
            claim: NO_CLAIM_YET.to_owned(),
        },
    }
}

/// Make one line of prose safe to stand in a table cell.
///
/// A gloss may say anything its author needs it to, mathematics included, and a
/// pipe in it would otherwise end the cell early. Escaping is the only change a
/// projection makes to an author's words.
fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// One covered test's claim, as the census holds it.
#[derive(Debug, Clone)]
pub struct CoveredClaim {
    asset: CoveredAsset,
    claim: ClaimLine,
}

impl CoveredClaim {
    /// The covered test this claim stands on.
    #[must_use]
    pub const fn asset(&self) -> &CoveredAsset {
        &self.asset
    }

    /// The claim itself.
    #[must_use]
    pub const fn claim(&self) -> &ClaimLine {
        &self.claim
    }
}

/// What the claim pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaimAnalysis {
    /// How many covered tests the pass considered.
    pub covered: usize,
    /// How many carry a claim label at the standard place.
    pub claimed: usize,
    /// How many carry none, which the staging counts rather than reports.
    pub unclaimed: usize,
    /// How many of the claims mint a statement.
    pub mints: usize,
    /// How many cite a statement a sibling test minted.
    pub citations: usize,
    /// How many distinct areas the claims are written in.
    pub areas: usize,
    /// How many claims stand in each area, which the area register is curated from.
    pub by_area: BTreeMap<String, usize>,
    /// How many covered tests of each package carry no claim yet.
    ///
    /// The coverage figure the staging is instrumented by, per owner rather than
    /// per area: an unclaimed test has no claim area to be counted under, and
    /// the wave that will write its claim is a package's.
    pub unclaimed_by_package: BTreeMap<String, usize>,
    /// How many packages the staging holds to the whole requirement.
    pub packages_closed: usize,
    /// How many claim lines are defective in placement or shape.
    pub defective: usize,
    /// How many citations resolve to no mint of the census.
    pub unresolved: usize,
    /// How many statements are minted twice in one owner.
    pub collisions: usize,
    /// How many mints carry no gloss for a projection to render.
    pub glossless: usize,
}

/// The packages whose authoring wave has closed, as the corpus declares it.
///
/// This is the staging instrument of ADR-T-017 and the whole of it. A package in
/// the returned set is held to the requirement that every covered test carries a
/// claim; a package outside it has its claimless tests counted in the coverage
/// figures and reported nowhere.
///
/// The membership was never a choice made here: the record fixes the criterion,
/// and the set is the packages meeting it. What states it is the activation. An
/// owner holding the claims pair is an owner whose wave has closed, and the crate
/// names come back through the same derivation the roster reconciles by — so a
/// package joins in the commit that activates its pair, and the set moves only as
/// the campaign does (ADR-T-017, The test documentation policy).
///
/// The direction matters. A spelling is not uniquely reversible to a crate name,
/// so the set is taken by asking each discovered member which owner it derives
/// and whether that owner activates, rather than by asking each activation which
/// crate it meant. A corpus with no declaration closes no wave, which is the
/// answer absence gives everywhere else: a tree that has not written the
/// activations has not said which waves it closed.
#[must_use]
pub fn closed_waves(
    packages: &[Package],
    names: Option<&OwnerNames>,
    pairs: &[Pair],
) -> Vec<String> {
    let Some(names) = names else {
        return Vec::new();
    };

    let closed: BTreeSet<&str> = pairs
        .iter()
        .filter(|pair| pair.policy == CLAIMS_POLICY)
        .map(|pair| pair.owner.as_str())
        .collect();

    packages
        .iter()
        .filter(|package| {
            names
                .derive(package.name())
                .is_some_and(|owner| closed.contains(owner.as_str()))
        })
        .map(|package| package.name().to_owned())
        .collect()
}

/// Run the claim pass over the covered assets of a workspace.
///
/// Resolution is staged exactly as the calculus stages it generally: every mint
/// of every owner is entered first, and only then is any citation asked to
/// resolve, so the order the corpus is read in decides nothing and a test may
/// cite a sibling that stands below it in the same file.
///
/// The closed set is passed in rather than known here, because which waves have
/// closed is the corpus's statement and this pass is any workspace's.
#[must_use]
pub fn analyze_claims(
    assets: &[CoveredAsset],
    closed: &[String],
) -> (ClaimAnalysis, Vec<CoveredClaim>, Vec<Finding>) {
    let mut analysis = ClaimAnalysis::default();
    let mut findings = Vec::new();
    let mut claims: Vec<CoveredClaim> = Vec::new();
    let closed: BTreeSet<&str> = closed.iter().map(String::as_str).collect();

    analysis.packages_closed = closed.len();

    for asset in assets {
        analysis.covered += 1;

        match read_claim(asset) {
            Standing::Claimed(claim) => {
                analysis.claimed += 1;
                claims.push(CoveredClaim {
                    asset: asset.clone(),
                    claim,
                });
            }
            Standing::Unclaimed => {
                analysis.unclaimed += 1;
                *analysis
                    .unclaimed_by_package
                    .entry(asset.test().package().to_owned())
                    .or_default() += 1;

                if closed.contains(asset.test().package()) {
                    findings.push(Finding::MissingClaimLabel {
                        owner: asset.test().package().to_owned(),
                        asset: asset.test().function().to_owned(),
                        location: asset.test().location().clone(),
                    });
                }
            }
            Standing::Defective(defect) => {
                analysis.defective += 1;
                findings.push(Finding::DefectiveClaimLine {
                    owner: asset.test().package().to_owned(),
                    asset: asset.test().function().to_owned(),
                    defect,
                    location: asset.test().location().clone(),
                });
            }
        }
    }

    registries(&claims, &mut analysis, &mut findings);

    for claim in &claims {
        *analysis
            .by_area
            .entry(claim.claim.label.area().to_owned())
            .or_default() += 1;
    }

    analysis.areas = analysis.by_area.len();
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, claims, findings)
}

/// Complete every owner's claim registry, then resolve every citation against it.
fn registries(claims: &[CoveredClaim], analysis: &mut ClaimAnalysis, findings: &mut Vec<Finding>) {
    let mut minted: BTreeMap<(&str, Label), &CoveredClaim> = BTreeMap::new();

    for claim in claims {
        if !claim.claim.is_mint() {
            continue;
        }

        analysis.mints += 1;

        if claim.claim.gloss.is_empty() {
            analysis.glossless += 1;
        }

        let owner = claim.asset.test().package();
        let key = (owner, claim.claim.label.clone());

        match minted.get(&key) {
            Some(first) => {
                analysis.collisions += 1;
                findings.push(Finding::DuplicateClaimMint {
                    label: claim.claim.label.clone(),
                    owner: owner.to_owned(),
                    first: first.asset.test().location().clone(),
                    second: claim.asset.test().location().clone(),
                });
            }
            None => {
                let _replaced = minted.insert(key, claim);
            }
        }
    }

    for claim in claims {
        if claim.claim.is_mint() {
            continue;
        }

        analysis.citations += 1;

        let owner = claim.asset.test().package();

        if !minted.contains_key(&(owner, claim.claim.label.clone())) {
            analysis.unresolved += 1;
            findings.push(Finding::UnresolvedClaimCitation {
                label: claim.claim.label.clone(),
                owner: owner.to_owned(),
                asset: claim.asset.test().function().to_owned(),
                location: claim.asset.test().location().clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::{ClaimAnalysis, ClaimDefect, ClaimForm, Standing, analyze_claims, read_claim};
    use crate::census::{Census, scan_source};
    use crate::finding::Finding;
    use crate::profile::{CoveredAsset, cover};
    use crate::roster::{OwnerNames, derive_owner};
    use crate::snapshot::Pair;
    use crate::workspace::Package;

    /// The acute the code syntax delimits an occurrence with.
    const ACUTE: char = '\u{b4}';

    /// A fictional activation relation owned by this test.
    const POLICIES_DOCUMENT: &str = include_str!("../tests/fixtures/claims-policies.toml");

    /// The project namespace this repository's own crate names carry.
    const NAMESPACE: &str = "ember-";

    /// The fictional members whose claims waves the fixture closes.
    const FIXTURE_MEMBERS: &[&str] = &["ember-river", "ember-valley"];

    /// Cover one package's sources the way the check covers them.
    fn assets_of(sources: &[(&str, &str)]) -> Vec<CoveredAsset> {
        let packages = vec![Package::new("ember-demo", "packages/demo")];
        let mut tests = Vec::new();

        for (path, text) in sources {
            tests
                .extend(scan_source("ember-demo", Path::new(path), text).expect("a Rust source"));
        }

        let (assets, findings) = cover(&packages, &Census::from_tests(tests, sources.len()));

        assert!(
            findings.is_empty(),
            "the fixture covers cleanly: {findings:?}"
        );

        assets
    }

    fn analyze(sources: &[(&str, &str)]) -> (ClaimAnalysis, Vec<Finding>) {
        let (analysis, _claims, findings) = analyze_claims(&assets_of(sources), &[]);

        (analysis, findings)
    }

    fn sole_standing(text: &str) -> Standing {
        let assets = assets_of(&[("packages/demo/src/tests/demo.rs", text)]);
        let [asset] = assets.as_slice() else {
            panic!("expected one covered test, got {assets:?}");
        };

        read_claim(asset)
    }

    /// A claim standing bare on the line above the derived test label is a
    /// mint, and the prose above it is read as the statement it names. The
    /// three parts — gloss, claim, derived label — are recovered from one
    /// documentation comment in the order the convention fixes them.
    ///
    /// ´claim:claim:a-bare-claim-above-the-test-label-mints-with-the-gloss-above-it´
    /// ´test:unit:reads-a-mint-above-the-test-label´
    #[test]
    fn reads_a_mint_above_the_test_label() {
        let standing = sole_standing(&format!(
            "/// The widths are identical across the sweep.\n\
             ///\n\
             /// {ACUTE}claim:resonance:crossover-widths-are-p-hat-independent{ACUTE}\n\
             /// {ACUTE}test:crate:widths-are-p-hat-independent{ACUTE}\n\
             #[test]\nfn widths_are_p_hat_independent() {{}}\n"
        ));

        let Standing::Claimed(claim) = standing else {
            panic!("expected a claim, got {standing:?}");
        };

        assert_eq!(claim.form(), ClaimForm::Mint);
        assert_eq!(
            claim.label().to_string(),
            "claim:resonance:crossover-widths-are-p-hat-independent"
        );
        assert_eq!(claim.gloss(), "The widths are identical across the sweep.");
    }

    /// The same line parenthesised is a citation instead, and a citing test
    /// needs no gloss of its own: the statement is said where it was minted,
    /// which is the whole point of citing rather than restating it.
    ///
    /// ´claim:claim:a-parenthesised-claim-cites-and-needs-no-gloss-of-its-own´
    /// ´test:unit:reads-a-citation-above-the-test-label´
    #[test]
    fn reads_a_citation_above_the_test_label() {
        let standing = sole_standing(&format!(
            "/// ({ACUTE}claim:resonance:widths-are-p-hat-independent{ACUTE})\n\
             /// {ACUTE}test:crate:widths-survive-the-public-surface{ACUTE}\n\
             #[test]\nfn widths_survive_the_public_surface() {{}}\n"
        ));

        let Standing::Claimed(claim) = standing else {
            panic!("expected a claim, got {standing:?}");
        };

        assert_eq!(claim.form(), ClaimForm::Citation);
        assert_eq!(claim.gloss(), "", "a citing test needs no gloss of its own");
    }

    /// A gloss wrapped across several documentation lines joins into one line
    /// for projection, because a documentation comment wraps where its author
    /// wrapped it and a table cell cannot. The author writes prose; the
    /// generator gets a sentence.
    ///
    /// ´claim:claim:a-wrapped-gloss-joins-into-one-line-for-projection´
    /// ´test:unit:joins-a-wrapped-gloss-into-one-line´
    #[test]
    fn joins_a_wrapped_gloss_into_one_line() {
        let standing = sole_standing(&format!(
            "/// The crossover landscape translates rigidly with p-hat: widths\n\
             /// are identical across the whole sweep.\n\
             ///\n\
             /// {ACUTE}claim:resonance:crossover{ACUTE}\n\
             /// {ACUTE}test:crate:covered{ACUTE}\n\
             #[test]\nfn covered() {{}}\n"
        ));

        let Standing::Claimed(claim) = standing else {
            panic!("expected a claim, got {standing:?}");
        };

        assert_eq!(
            claim.gloss(),
            "The crossover landscape translates rigidly with p-hat: widths are identical across the whole sweep."
        );
    }

    /// A covered test carrying no claim is counted and reported nowhere, and
    /// counted against its own package. That is the whole of the staging: the
    /// scale of the corpus is met with a coverage figure rather than with
    /// thousands of findings nobody can act on at once.
    ///
    /// ´claim:claim:an-unclaimed-test-is-counted-against-its-package-and-not-reported´
    /// ´test:unit:counts-a-test-with-no-claim-rather-than-reporting-it´
    #[test]
    fn counts_a_test_with_no_claim_rather_than_reporting_it() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!("/// {ACUTE}test:crate:covered{ACUTE}\n#[test]\nfn covered() {{}}\n"),
        )]);

        assert_eq!(analysis.covered, 1);
        assert_eq!(analysis.unclaimed, 1);
        assert_eq!(analysis.claimed, 0);
        assert_eq!(analysis.unclaimed_by_package["ember-demo"], 1);
        assert!(
            findings.is_empty(),
            "the staging counts rather than reports: {findings:?}"
        );
    }

    /// A claim written below the derived test label is a placement defect: the
    /// derived label is the final line, and a claim after it is reported rather
    /// than read.
    ///
    /// ´claim:claim:a-claim-written-after-the-test-label-is-misplaced´
    /// ´test:unit:reports-a-claim-written-last´
    #[test]
    fn reports_a_claim_written_last() {
        let standing = sole_standing(&format!(
            "/// {ACUTE}test:crate:covered{ACUTE}\n\
             /// {ACUTE}claim:resonance:written-last{ACUTE}\n\
             #[test]\nfn covered() {{}}\n"
        ));

        assert_eq!(standing, Standing::Defective(ClaimDefect::Misplaced));
    }

    /// Any other placement is a defect too: a claim standing above its own
    /// gloss is misplaced, because the standard place is the line directly
    /// above the derived label and nowhere else.
    ///
    /// ´claim:claim:a-claim-anywhere-but-directly-above-the-test-label-is-misplaced´
    /// ´test:unit:reports-a-claim-above-the-gloss´
    #[test]
    fn reports_a_claim_above_the_gloss() {
        let standing = sole_standing(&format!(
            "/// {ACUTE}claim:resonance:too-high{ACUTE}\n\
             /// The statement.\n\
             /// {ACUTE}test:crate:covered{ACUTE}\n\
             #[test]\nfn covered() {{}}\n"
        ));

        assert_eq!(standing, Standing::Defective(ClaimDefect::Misplaced));
    }

    /// A claim sharing its line with other words is a defect: the standard
    /// place holds the occurrence and nothing else, so a label mentioned in
    /// passing inside a sentence is never mistaken for the test's own claim.
    ///
    /// ´claim:claim:a-claim-sharing-its-line-with-other-words-is-defective´
    /// ´test:unit:reports-a-claim-sharing-its-line´
    #[test]
    fn reports_a_claim_sharing_its_line() {
        let standing = sole_standing(&format!(
            "/// see {ACUTE}claim:resonance:shared{ACUTE} for the statement\n\
             /// {ACUTE}test:crate:covered{ACUTE}\n\
             #[test]\nfn covered() {{}}\n"
        ));

        assert_eq!(
            standing,
            Standing::Defective(ClaimDefect::NotAloneOnItsLine)
        );
    }

    /// One test carries one claim: a comment holding two is reported as
    /// repeated rather than having one of them chosen. A test establishing two
    /// statements is a test to split, not a comment to stack labels in.
    ///
    /// ´claim:claim:a-comment-carrying-two-claims-is-defective´
    /// ´test:unit:reports-two-claims-in-one-comment´
    #[test]
    fn reports_two_claims_in_one_comment() {
        let standing = sole_standing(&format!(
            "/// {ACUTE}claim:resonance:first{ACUTE}\n\
             /// {ACUTE}claim:resonance:second{ACUTE}\n\
             /// {ACUTE}test:crate:covered{ACUTE}\n\
             #[test]\nfn covered() {{}}\n"
        ));

        assert_eq!(standing, Standing::Defective(ClaimDefect::Repeated));
    }

    /// Shared coverage becomes a mechanical fact: a statement two tests
    /// establish is one mint and one citation, both resolving cleanly. A
    /// scenario covered many times over is therefore a query rather than a
    /// reading of many files.
    ///
    /// ´claim:claim:shared-coverage-is-one-mint-and-the-citations-of-it´
    /// ´test:unit:resolves-a-citation-of-a-sibling-mint´
    #[test]
    fn resolves_a_citation_of_a_sibling_mint() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!(
                "/// The statement.\n\
                 ///\n\
                 /// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
                 /// {ACUTE}test:crate:mints{ACUTE}\n\
                 #[test]\nfn mints() {{}}\n\n\
                 /// ({ACUTE}claim:resonance:the-statement{ACUTE})\n\
                 /// {ACUTE}test:crate:cites{ACUTE}\n\
                 #[test]\nfn cites() {{}}\n"
            ),
        )]);

        assert_eq!(analysis.mints, 1);
        assert_eq!(analysis.citations, 1);
        assert_eq!(analysis.unresolved, 0);
        assert!(
            findings.is_empty(),
            "shared coverage is clean: {findings:?}"
        );
    }

    /// Resolution is staged, so the order a corpus is read in decides nothing:
    /// every mint is entered before any citation is asked to resolve, and a
    /// test may cite a sibling standing below it in the same file.
    ///
    /// ´claim:claim:a-citation-resolves-regardless-of-where-its-mint-stands´
    /// ´test:unit:resolves-a-citation-of-a-mint-standing-below-it´
    #[test]
    fn resolves_a_citation_of_a_mint_standing_below_it() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!(
                "/// ({ACUTE}claim:resonance:the-statement{ACUTE})\n\
                 /// {ACUTE}test:crate:cites{ACUTE}\n\
                 #[test]\nfn cites() {{}}\n\n\
                 /// The statement.\n\
                 ///\n\
                 /// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
                 /// {ACUTE}test:crate:mints{ACUTE}\n\
                 #[test]\nfn mints() {{}}\n"
            ),
        )]);

        assert_eq!(
            analysis.unresolved, 0,
            "every mint is entered before anything resolves"
        );
        assert!(findings.is_empty(), "findings: {findings:?}");
    }

    /// Resolution is total: a citation of a statement nobody minted is a
    /// failure naming the label and the test that wrote it, so a claim cannot
    /// be referred to into existence.
    ///
    /// ´claim:claim:a-citation-of-an-unminted-claim-is-a-failure´
    /// ´test:unit:fails-a-citation-of-a-claim-nobody-minted´
    #[test]
    fn fails_a_citation_of_a_claim_nobody_minted() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!(
                "/// ({ACUTE}claim:resonance:nobody-minted-this{ACUTE})\n\
                 /// {ACUTE}test:crate:cites{ACUTE}\n\
                 #[test]\nfn cites() {{}}\n"
            ),
        )]);

        assert_eq!(analysis.unresolved, 1);

        let [Finding::UnresolvedClaimCitation { label, asset, .. }] = findings.as_slice() else {
            panic!("expected one unresolved citation, got {findings:?}");
        };

        assert_eq!(label.to_string(), "claim:resonance:nobody-minted-this");
        assert_eq!(asset, "cites");
    }

    /// A statement is minted once within an owner: minting it twice is a
    /// failure carrying both locations, so the author is shown the pair to
    /// choose between rather than told a name is taken.
    ///
    /// ´claim:claim:minting-one-claim-twice-in-an-owner-fails-with-both-locations´
    /// ´test:unit:fails-two-mints-of-one-claim-with-both-locations´
    #[test]
    fn fails_two_mints_of_one_claim_with_both_locations() {
        let (analysis, findings) = analyze(&[
            (
                "packages/demo/src/tests/one.rs",
                &format!(
                    "/// The statement.\n///\n/// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
                     /// {ACUTE}test:crate:first{ACUTE}\n#[test]\nfn first() {{}}\n"
                ),
            ),
            (
                "packages/demo/src/tests/two.rs",
                &format!(
                    "/// The statement again.\n///\n/// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
                     /// {ACUTE}test:crate:second{ACUTE}\n#[test]\nfn second() {{}}\n"
                ),
            ),
        ]);

        assert_eq!(analysis.collisions, 1);

        let [Finding::DuplicateClaimMint { first, second, .. }] = findings.as_slice() else {
            panic!("expected one collision, got {findings:?}");
        };

        assert_eq!(first.path(), Path::new("packages/demo/src/tests/one.rs"));
        assert_eq!(second.path(), Path::new("packages/demo/src/tests/two.rs"));
    }

    /// Uniqueness is per owner, so two packages may mint the same claim name
    /// without colliding: ownership disambiguates, and neither package needs to
    /// know what the other has named its statements.
    ///
    /// ´claim:claim:claim-uniqueness-is-per-owner´
    /// ´test:unit:leaves-one-claim-minted-in-two-owners-alone´
    #[test]
    fn leaves_one_claim_minted_in_two_owners_alone() {
        let packages = vec![
            Package::new("ember-one", "packages/one"),
            Package::new("ember-two", "packages/two"),
        ];
        let body = |name: &str| {
            format!(
                "/// The statement.\n///\n/// {ACUTE}claim:resonance:the-statement{ACUTE}\n\
                 /// {ACUTE}test:crate:{name}{ACUTE}\n#[test]\nfn {name}() {{}}\n"
            )
            .replace('_', "-")
        };

        let mut tests = scan_source(
            "ember-one",
            Path::new("packages/one/src/tests/a.rs"),
            &body("first").replace("fn first-", "fn first"),
        )
        .expect("a Rust source");
        tests.extend(
            scan_source(
                "ember-two",
                Path::new("packages/two/src/tests/a.rs"),
                &body("second").replace("fn second-", "fn second"),
            )
            .expect("a Rust source"),
        );

        let (assets, _findings) = cover(&packages, &Census::from_tests(tests, 2));
        let (analysis, _claims, findings) = analyze_claims(&assets, &[]);

        assert_eq!(analysis.mints, 2);
        assert_eq!(analysis.collisions, 0, "ownership disambiguates");
        assert!(findings.is_empty(), "and nothing is reported: {findings:?}");
    }

    /// The area vocabulary is censused rather than fixed: how many areas are in
    /// use and how many claims stand in each is measured from the claims
    /// actually written. Fixing the set first would fix it from a taxonomy that
    /// had met no sentence.
    ///
    /// ´claim:claim:the-area-vocabulary-is-measured-from-the-claims-written´
    /// ´test:unit:censuses-the-area-vocabulary´
    #[test]
    fn censuses_the_area_vocabulary() {
        let (analysis, _findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!(
                "/// One.\n///\n/// {ACUTE}claim:resonance:one{ACUTE}\n/// {ACUTE}test:crate:one{ACUTE}\n\
                 #[test]\nfn one() {{}}\n\n\
                 /// Two.\n///\n/// {ACUTE}claim:resonance:two{ACUTE}\n/// {ACUTE}test:crate:two{ACUTE}\n\
                 #[test]\nfn two() {{}}\n\n\
                 /// Three.\n///\n/// {ACUTE}claim:decay:three{ACUTE}\n/// {ACUTE}test:crate:three{ACUTE}\n\
                 #[test]\nfn three() {{}}\n"
            ),
        )]);

        assert_eq!(analysis.areas, 2);
        assert_eq!(analysis.by_area["resonance"], 2);
        assert_eq!(analysis.by_area["decay"], 1);
    }

    /// The staging is the set of packages whose waves have closed and nothing
    /// more: one claimless test is counted either way, reported nowhere while
    /// its package's wave is open, and held to the whole requirement from the
    /// moment that package joins the set. The number held is exactly the size
    /// of the set the caller passed, so the pass carries no roster of its own to
    /// disagree with the corpus.
    ///
    /// ´claim:claim:the-staging-is-the-set-of-closed-packages-and-nothing-else´
    /// ´test:unit:holds-a-closed-package-to-the-whole-requirement´
    #[test]
    fn holds_a_closed_package_to_the_whole_requirement() {
        let assets = assets_of(&[(
            "packages/demo/src/tests/demo.rs",
            &format!("/// {ACUTE}test:crate:covered{ACUTE}\n#[test]\nfn covered() {{}}\n"),
        )]);

        let (open, _claims, unreported) = analyze_claims(&assets, &[]);

        assert_eq!(open.unclaimed, 1);
        assert_eq!(open.packages_closed, 0);
        assert!(
            unreported.is_empty(),
            "an open wave reports nothing: {unreported:?}"
        );

        let (closed, _claims, reported) = analyze_claims(&assets, &["ember-demo".to_owned()]);

        assert_eq!(
            closed.unclaimed, 1,
            "the coverage figure is taken either way"
        );
        assert_eq!(closed.packages_closed, 1);
        assert!(
            matches!(
                reported.as_slice(),
                [Finding::MissingClaimLabel { owner, .. }] if owner == "ember-demo"
            ),
            "a closed wave holds the same test to the requirement: {reported:?}"
        );
    }

    /// The set the check closes is exactly the fixture roster: its member names
    /// derive the owners activated for the claims profile in both directions,
    /// unrelated activations do not join the set, and the derivation carries
    /// every member back into the closed set.
    ///
    /// ´claim:claim:the-declared-activations-close-the-waves-the-roster-named´
    /// ´test:unit:closes-the-waves-the-retired-roster-named´
    #[test]
    fn closes_the_waves_the_retired_roster_named() {
        let activated: BTreeSet<String> = toml::from_str::<toml::Table>(POLICIES_DOCUMENT)
            .expect("the declaration parses")
            .get("policies")
            .and_then(toml::Value::as_array)
            .expect("the activation relation")
            .iter()
            .filter(|row| {
                row.get("policy").and_then(toml::Value::as_str) == Some(super::CLAIMS_POLICY)
            })
            .map(|row| {
                row.get("owner")
                    .and_then(toml::Value::as_str)
                    .expect("an owner on every row")
                    .to_owned()
            })
            .collect();

        let derived: BTreeSet<String> = FIXTURE_MEMBERS
            .iter()
            .map(|crate_name| {
                derive_owner(NAMESPACE, crate_name)
                    .expect("a well-formed spelling")
                    .as_str()
                    .to_owned()
            })
            .collect();

        assert_eq!(
            derived, activated,
            "the roster and the activations name one set"
        );

        let members: Vec<Package> = FIXTURE_MEMBERS
            .iter()
            .map(|crate_name| Package::new(*crate_name, "."))
            .collect();
        let pairs: Vec<Pair> = activated
            .iter()
            .map(|owner| Pair {
                owner: owner.clone(),
                policy: super::CLAIMS_POLICY.to_owned(),
                family: None,
            })
            .collect();

        assert_eq!(
            super::closed_waves(&members, Some(&OwnerNames::new(NAMESPACE, [])), &pairs),
            FIXTURE_MEMBERS,
            "and every fixture member is carried back into the closed set"
        );
    }

    /// A corpus in which nothing is claimed yet is clean: its tests are
    /// counted, no area is in use, and nothing is reported. The machinery can
    /// therefore land before a single claim is written.
    ///
    /// ´claim:claim:a-corpus-with-no-claims-yet-is-clean´
    /// ´test:unit:leaves-a-claimless-corpus-clean´
    #[test]
    fn leaves_a_claimless_corpus_clean() {
        let (analysis, findings) = analyze(&[(
            "packages/demo/src/tests/demo.rs",
            &format!(
                "/// {ACUTE}test:crate:one{ACUTE}\n#[test]\nfn one() {{}}\n\n\
                 /// {ACUTE}test:crate:two{ACUTE}\n#[test]\nfn two() {{}}\n"
            ),
        )]);

        assert_eq!(analysis.covered, 2);
        assert_eq!(analysis.unclaimed, 2);
        assert_eq!(analysis.areas, 0);
        assert!(
            findings.is_empty(),
            "a claimless corpus is clean: {findings:?}"
        );
    }
}
