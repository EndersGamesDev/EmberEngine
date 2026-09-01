// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The code carrier: what a package's Rust commentary contributes to the one
//! resolution space.
//!
//! ADR-L-014 gives the calculus two concrete syntaxes and says nothing about
//! one of them resolving less than the other: the well-formed environment
//! (ADR-L-014, A calculus of documentation and source labels) reads a span the same way whichever
//! delimiters carry it, and the total-resolution invariant
//! (ADR-L-014, A calculus of documentation and source labels) is a statement about citations
//! rather than about documents. The owner's ruling of 2026-08-20 makes that
//! explicit — a label is citable from either surface, whatever kind it is — and
//! this module is the half of that ruling the prose carrier could not supply.
//!
//! # One space, two surfaces
//!
//! The engine next door already holds one registry per owner and resolves every
//! citation against it. What it lacked was reach: the registries were seeded from
//! the test census alone, and the only carrier it harvested was Markdown. So a
//! notice or a claim — both minted in code — could be cited by nothing, and a
//! comment could cite nothing at all. Both halves are widened here rather than
//! per kind, because the ruling is about the space and not about a kind: this
//! module hands the engine every mint that stands in code, whatever profile
//! derived or authored it, and every citation that stands in commentary,
//! whatever kind it names.
//!
//! # Commentary and the standard places
//!
//! A comment line carries two quite different things, and only one of them is
//! this carrier's. A profile's standard place is the asset's own attested
//! field, and the profile that defines it owns it whole: ADR-L-015 owns the
//! derived label's line and ADR-L-017 owns the claim's, both of which already
//! resolve what stands there and report what does not. A standard place is a
//! position, though, never a shape: the derived label's line is the final line
//! of a covered test's documentation comment and the claim's is the line
//! directly above it, and only those positions — read from the census — are
//! passed over. A label-alone line anywhere else is ordinary commentary, and a
//! citation standing in commentary is this carrier's to resolve. Reading the
//! rule that way is what keeps one defect to one report: measured 2026-08-20,
//! the shape-alone pass-over left a label-alone line in ordinary commentary
//! reported by nobody, which is the silence the position-aware rule closes.
//!
//! Commentary spans are classified, never dropped: in scanned code text an
//! opening acute declares intent to mint or cite, and its loss is an error —
//! the local-classification rule of the participation judgment
//! (ADR-L-014, A calculus of documentation and source labels). A hugging-parenthesised span is a
//! citation and resolves. A bare span is a mint nothing warrants — the profiles
//! census their standard places and nothing else, so no census reaches
//! commentary — and it is reported rather than passed over; the carrier itself
//! still never mints. A span standing in a parenthesis beside prose or a second
//! label is a malformed citation, reported the same way. What stays unscanned
//! is what the participation judgment excludes: the standard places a profile
//! actually owns — owned by position, read from the census, never by a line's
//! shape alone — fenced documentation examples, and string and character
//! literals. Bare spans of the to-do kind are left out of these reports: that
//! kind's bare spans are the todo profile's own census wherever they stand —
//! its notice heads and its orphan report — and one defect keeps one report.
//!
//! # The committed index participates
//!
//! The committed in-file index is a generated region, and a generated
//! occurrence is an occurrence in full — the generated-compliance invariant
//! (ADR-L-014, A calculus of documentation and source labels). Its citations resolve against
//! the same registries as everything else, marked as generated so a failure
//! blames the register or its generator rather than an author. A bare span in
//! the region is the generator minting without warrant, since no adopted
//! profile sets a standard place in a generated register, and the one exclusion
//! is self-presentation: nothing in the region enters any census, registry, or
//! count the region was generated from — the carrier reads citations out of it
//! and mints nothing, so no-self-support stays a theorem.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_a_parenthesized_span_of_commentary_as_a_citation`] | carrier | A parenthesised occurrence standing in ordinary commentary is a citation the carrier reads, so a comment may point at any statement the corpus has established. Code and prose refer to one corpus. |
//! | [`reads_no_citation_from_a_bare_span`] | carrier | The carrier never mints: a bare occurrence in commentary yields no mint and no citation from this scanner. What it yields is a report, because no census reaches commentary and a warrantless mint may not pass in silence. |
//! | [`reports_a_bare_span_in_commentary_as_an_unwarranted_mint`] | carrier | A bare span in commentary is reported as an unwarranted mint: the opening acute declared intent the line cannot discharge, since the profiles census their standard places and nothing else, so the span is a finding rather than a silence. |
//! | [`reports_a_parenthesis_carrying_prose_beside_a_span`] | carrier | A parenthesis carrying prose beside a label is a malformed citation and is reported: citation intent that missed the hugging form may not lapse into text, since the acute's declared intent would be lost with it. |
//! | [`reports_each_label_of_a_shared_parenthesis`] | carrier | cites (´claim:carrier:a-parenthesis-carrying-prose-beside-a-label-is-a-malformed-citation´) |
//! | [`reports_an_import_shaped_span_without_parentheses`] | occurrence | cites (´claim:occurrence:an-import-without-parentheses-is-a-failure-not-text´) |
//! | [`leaves_a_bare_todo_span_to_the_todo_profile`] | carrier | A bare span of the to-do kind is the todo profile's to report wherever it stands — its notice heads carry one by design and its orphan report owns the rest — so the carrier stays silent about the kind and one defect keeps one report. |
//! | [`leaves_a_fenced_example_unscanned`] | carrier | A fenced documentation example is not commentary: what stands between the fences is shown rather than meant, so no span in it is read as a citation or reported as a defect. |
//! | [`resets_the_fence_at_the_next_documents_boundary`] | carrier | An unclosed fence in one item's documentation does not silence a citation in another's: fence state resets at each comment region's own boundary, the way rustdoc treats each item's documentation as its own markdown document. The unclosed document still draws its own report. |
//! | [`reports_one_unclosed_fence_at_its_opening_delimiter`] | carrier | A comment region ending with its fence still open is a hard finding naming the opening delimiter: the content after it went unscanned in full, silently, and that silence is exactly what the finding reports. |
//! | [`reads_a_citation_out_of_the_index_region`] | carrier | The committed in-file index is a generated region participating in full: a citation standing in its rows resolves like any other, marked as generated so a failure blames the register rather than an author. |
//! | [`reports_a_bare_span_in_the_index_region_as_the_generators`] | carrier | A bare span in the committed index region is the generator minting without warrant: no adopted profile sets a standard place in a generated register, so the span is reported as the generator's defect rather than as an author's unwarranted mint, and a display form remains inert there. |
//! | [`passes_over_the_standard_places_of_a_covered_asset`] | carrier | The pass-over is position-aware: the standard places of a covered asset — the derived label's final line and the claim line above it — are their profiles' own positions, and the carrier stays silent about them so that the profile owning each remains its only reporter. One defect keeps one report. |
//! | [`reports_a_label_alone_line_in_ordinary_commentary`] | carrier | cites (´claim:carrier:only-a-standard-place-a-profile-owns-is-passed-over´) |
//! | [`leaves_a_misplaced_test_label_to_its_profile`] | carrier | cites (´claim:carrier:only-a-standard-place-a-profile-owns-is-passed-over´) |
//! | [`reads_an_imported_citation_out_of_commentary`] | carrier | A comment reaches across an ownership boundary by the ordinary import form, so no new mechanism is needed for code to cite another package's statement. |
//! | [`reads_no_citation_a_string_literal_carries`] | comment | cites (´claim:comment:a-string-literal-is-never-read-as-commentary´) |
//! | [`warns_on_a_label_shaped_backtick_span_in_commentary`] | carrier | A label-shaped span in single backticks on the code surface is the prose delimiters where the acute syntax was meant, and draws a warning naming that repair without becoming an occurrence — while the doubled backticks stay the display form and stay inert. |
//! | [`warns_on_a_near_miss_acute_span`] | occurrence | cites (´claim:occurrence:a-span-one-repair-from-a-label-is-a-named-near-miss´) |
//! | [`warns_on_an_undelimited_bracket_import_in_commentary`] | occurrence | cites (´claim:occurrence:an-undelimited-bracketed-import-in-parentheses-warns´) |

#[cfg(test)]
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::census::{CENSUSED_DIRECTORIES, collect_rust_sources};
use crate::claim::{CLAIM_KIND, CoveredClaim, line_holds_claim};
use crate::comment::{LEADERS, comment_regions};
use crate::constant::{CONST_KIND, CoveredConstant};
use crate::finding::{Finding, Location};
use crate::index::committed_index;
use crate::label::Label;
use crate::legacy_profile::{CoveredLegacySite, is_legacy_kind};
use crate::occurrence::{
    Form, Occurrence, Reading, Syntax, UNDELIMITED_IMPORT_REASON, read_span, undelimited_imports,
};
#[cfg(test)]
use crate::plan::CorpusPlan;
use crate::plan::ProfileSource;
use crate::profile::{ACUTE, CodeSpan, CoveredAsset, TEST_KIND, code_spans};
use crate::todo::{CoveredNotice, TODO_KIND};
#[cfg(test)]
use crate::workspace::Package;

/// One mint standing in code, offered to the engine's registries.
///
/// The two facts a mint carries here are independent and both are wanted. Its
/// warrant says whether a derivation produced it or an author wrote it, which is
/// what the reserved kinds are about. Its surface says where it stands, which is
/// what the carrier counts are about — and a claim is the case that separates
/// them, being authored by a person and standing in code all the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMint {
    path: PathBuf,
    label: Label,
    location: Location,
    derived: bool,
}

impl CodeMint {
    /// The source the mint stands in, which decides its owner.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The minted label.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// Where the minting thing stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// Whether a profile's derivation warrants this mint rather than an author.
    #[must_use]
    pub const fn is_derived(&self) -> bool {
        self.derived
    }
}

/// One citation standing in a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeCitation {
    path: PathBuf,
    occurrence: Occurrence,
    generated: bool,
}

impl CodeCitation {
    /// The source the citation stands in, which decides its owner.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The occurrence itself.
    #[must_use]
    pub const fn occurrence(&self) -> &Occurrence {
        &self.occurrence
    }

    /// Whether the citation stands in a generated region.
    ///
    /// A generated citation resolves exactly as an authored one does; what the
    /// flag changes is who a failure blames. A dangling authored citation is the
    /// author's to repair, while a dangling generated one is staleness of the
    /// register or a defect of its generator, and the engine reports it beside
    /// the exactness check rather than as an author's slip.
    #[must_use]
    pub const fn is_generated(&self) -> bool {
        self.generated
    }
}

/// Everything the code surface contributes to the one resolution space.
///
/// A caller with no code at all builds an empty one, and the engine then checks a
/// carrier of prose exactly as it did before any of this existed.
#[derive(Debug, Clone, Default)]
pub struct CodeSurface {
    mints: Vec<CodeMint>,
    citations: Vec<CodeCitation>,
}

impl CodeSurface {
    /// Every mint standing in code.
    #[must_use]
    pub fn mints(&self) -> &[CodeMint] {
        &self.mints
    }

    /// Every citation standing in commentary.
    #[must_use]
    pub fn citations(&self) -> &[CodeCitation] {
        &self.citations
    }

    /// Admit the test profile's derived labels.
    #[must_use]
    pub fn with_tests(mut self, assets: &[CoveredAsset]) -> Self {
        for asset in assets {
            self.mints.push(CodeMint {
                path: asset.test().location().path().to_path_buf(),
                label: asset.label().clone(),
                location: asset.test().location().clone(),
                derived: true,
            });
        }

        self
    }

    /// Admit the todo profile's derived labels.
    ///
    /// The mint stands where the notice stands, so a reverse lookup of a notice
    /// label lands on the deficiency rather than on a document mentioning it.
    #[must_use]
    pub fn with_notices(mut self, notices: &[CoveredNotice]) -> Self {
        for notice in notices {
            self.mints.push(CodeMint {
                path: notice.notice().location().path().to_path_buf(),
                label: notice.label().clone(),
                location: notice.notice().location().clone(),
                derived: true,
            });
        }

        self
    }

    /// Admit the legacy-implementation profile's derived labels.
    #[must_use]
    pub fn with_legacy(mut self, sites: &[CoveredLegacySite]) -> Self {
        for site in sites {
            self.mints.push(CodeMint {
                path: site.site().location().path().to_path_buf(),
                label: site.label().clone(),
                location: site.site().location().clone(),
                derived: true,
            });
        }

        self
    }

    /// Admit the constant profile's identities and the pins beneath them.
    ///
    /// Both stand in code and both are mints, and they differ in what warrants
    /// them: an identity is the author's naming of a concept, while a pin stands
    /// on the derivation ADR-L-018 fixes. A pin enters only where it is actually
    /// standing at its place and is actually the derivation — a stale one is
    /// reported by the profile and mints nothing, so every citation of the value
    /// it used to state dangles in the commit that moved it, which is the
    /// tripwire the record was built for.
    #[must_use]
    pub fn with_constants(mut self, constants: &[CoveredConstant]) -> Self {
        for constant in constants {
            self.mints.push(CodeMint {
                path: constant.path().to_path_buf(),
                label: constant.identity().clone(),
                location: constant.location().clone(),
                derived: false,
            });

            if !constant.is_pinned() {
                continue;
            }

            for pin in constant.pins() {
                self.mints.push(CodeMint {
                    path: constant.path().to_path_buf(),
                    label: pin.clone(),
                    location: constant.location().clone(),
                    derived: true,
                });
            }
        }

        self
    }

    /// Admit the claim profile's authored mints.
    ///
    /// A claim is authored rather than derived, and it stands in code all the
    /// same. Both facts travel, because the first decides whether a hand may
    /// write the kind and the second decides whether the mint is prose a reviewer
    /// is reading.
    #[must_use]
    pub fn with_claims(mut self, claims: &[CoveredClaim]) -> Self {
        for claim in claims {
            if !claim.claim().is_mint() {
                continue;
            }

            self.mints.push(CodeMint {
                path: claim.asset().test().location().path().to_path_buf(),
                label: claim.claim().label().clone(),
                location: claim.asset().test().location().clone(),
                derived: false,
            });
        }

        self
    }

    /// Admit the citations a scan of the commentary found.
    #[must_use]
    pub fn with_citations(mut self, citations: Vec<CodeCitation>) -> Self {
        self.citations.extend(citations);

        self
    }
}

/// Whether a line, once its own decoration is resolved away, is a standard place.
///
/// A standard place holds one occurrence and nothing else. The test is written
/// about the shape of the line rather than about the kind the occurrence names,
/// because that is what the profiles themselves require of their standard places
/// and because a rule that asked about kinds would have to be revised by every
/// profile added after it. Shape alone is not enough to be passed over, though:
/// the pass-over is position-aware, and a standard-place-shaped line away from
/// any position a profile owns is ordinary commentary (`OwnedPlaces`).
fn is_standard_place(line: &str) -> bool {
    let rest = line.trim_start_matches(LEADERS).trim();
    let spans = code_spans(rest);

    let [span] = spans.as_slice() else {
        return false;
    };

    if span.parenthesized {
        span.start == 1 && span.end + 1 == rest.len()
    } else {
        span.start == 0 && span.end == rest.len()
    }
}

/// The commentary positions of one source that inventory profiles own.
///
/// A profile's standard place is the asset's own attested field, and the profile
/// that defines it is the only reporter of what stands there. Which positions
/// those are is a fact about the census, not about the shape of a line: the
/// derived test label's line is the final line of a covered test's documentation
/// comment, and the claim's is the line directly above it. A label-alone line
/// anywhere else is ordinary commentary and classifies like any other span —
/// the pass-over is position-aware, so silence cannot be had by shape alone.
#[derive(Debug, Clone, Default)]
pub struct OwnedPlaces {
    /// Byte ranges of whole covered documentation comments.
    docs: Vec<(usize, usize)>,
    /// Byte ranges of the standard-place lines the profiles read.
    places: Vec<(usize, usize)>,
}

impl OwnedPlaces {
    /// The owned positions of one source, from the covered assets standing in it.
    ///
    /// The final documentation line is owned whole: the test profile reports
    /// whatever stands there, right label or wrong. The line above it is the
    /// claim profile's exactly when a claim-kind occurrence stands on it — any
    /// other label there is commentary wearing the claim's position. A
    /// documentation comment that is not line-oriented cannot be owned a line at
    /// a time, so its whole extent is taken as owned places.
    #[must_use]
    pub fn of(source: &str, assets: &[&CoveredAsset]) -> Self {
        let starts = line_starts(source);
        let mut owned = Self::default();

        for asset in assets {
            let Some(doc) = asset.test().doc() else {
                continue;
            };

            let Some(range) = line_range(&starts, source, doc.first_line(), doc.last_line()) else {
                continue;
            };

            owned.docs.push(range);

            if !doc.is_line_oriented() {
                owned.places.push(range);
                continue;
            }

            if let Some(place) = line_range(&starts, source, doc.last_line(), doc.last_line()) {
                owned.places.push(place);
            }

            let has_claim_line = doc.lines().len() >= 2
                && doc
                    .lines()
                    .get(doc.lines().len() - 2)
                    .is_some_and(|line| line_holds_claim(line));

            if has_claim_line
                && let Some(place) =
                    line_range(&starts, source, doc.last_line() - 1, doc.last_line() - 1)
            {
                owned.places.push(place);
            }
        }

        owned
    }

    /// Whether a profile owns the standard place standing at this offset.
    fn owns_place(&self, offset: usize) -> bool {
        self.places
            .iter()
            .any(|(start, end)| offset >= *start && offset < *end)
    }

    /// Whether the offset falls inside a covered documentation comment.
    fn in_doc(&self, offset: usize) -> bool {
        self.docs
            .iter()
            .any(|(start, end)| offset >= *start && offset < *end)
    }
}

/// The byte offsets at which each line of a source begins.
fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0_usize];
    starts.extend(
        source
            .bytes()
            .enumerate()
            .filter(|(_index, byte)| *byte == b'\n')
            .map(|(index, _byte)| index + 1),
    );

    starts
}

/// The byte range of a one-based inclusive line span, when the lines exist.
fn line_range(starts: &[usize], source: &str, first: usize, last: usize) -> Option<(usize, usize)> {
    let start = *starts.get(first.checked_sub(1)?)?;
    let end = starts.get(last).copied().unwrap_or(source.len());

    (start <= end).then_some((start, end))
}

/// The byte range of the committed in-file index, when the source carries one.
///
/// The recogniser is the projection's own — the same header-row identity the
/// verification pass reads through — so the region the scan takes as generated
/// is exactly the region the projection owns, and no second recognition can
/// drift from the first.
fn index_region_bytes(source: &str) -> Option<(usize, usize)> {
    let committed = committed_index(source)?;
    let starts = line_starts(source);

    let start = *starts.get(committed.first_line)?;
    let end = starts
        .get(committed.past_last_line)
        .copied()
        .unwrap_or(source.len());

    Some((start, end))
}

/// Whether a fence delimiter opens or closes on this line of commentary.
fn is_fence_delimiter(line: &str) -> bool {
    line.trim_start_matches(LEADERS)
        .trim_start()
        .starts_with("```")
}

/// Read one Rust source for the citations its commentary stands, and the spans
/// standing there that fail to be citations.
///
/// The covered assets standing in the source decide which positions the
/// profiles own (`OwnedPlaces`); a caller with none passes an empty slice and
/// every standard-place-shaped line then classifies as commentary.
///
/// Fence state resets at each comment region's own boundary — where the gap
/// since the previous region held anything but whitespace — so that an
/// unclosed fence in one item's documentation cannot silence a citation in
/// another's. This is rustdoc's own model: each item's documentation is its
/// own markdown document, and a document that never closed its fence still
/// ends where it ends, leaving what follows unread by that one document alone.
#[must_use]
pub fn scan_code_citations(
    path: &Path,
    source: &str,
    assets: &[&CoveredAsset],
) -> (Vec<CodeCitation>, Vec<Finding>) {
    let mut citations = Vec::new();
    let mut findings = Vec::new();
    let index_region = index_region_bytes(source);
    let owned = OwnedPlaces::of(source, assets);
    let mut fenced = false;
    let mut fence_opened_at = 0_usize;
    let mut previous_end: Option<usize> = None;

    for region in comment_regions(source) {
        if !continues_previous_region(source, previous_end, region.start()) {
            close_dangling_fence(path, source, fenced, fence_opened_at, &mut findings);
            fenced = false;
        }

        let text = region.text(source);
        let mut offset = region.start();

        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            let in_index = index_region.is_some_and(|(start, end)| offset >= start && offset < end);

            if is_fence_delimiter(trimmed) {
                fenced = !fenced;

                if fenced {
                    fence_opened_at = offset;
                }
            } else if !fenced {
                let context = if in_index {
                    Context::Generated
                } else if is_standard_place(trimmed) && owned.owns_place(offset) {
                    // A profile's standard place: the profile owning the
                    // position remains the only reporter of what stands there.
                    offset += line.len();
                    continue;
                } else if owned.in_doc(offset) {
                    Context::CoveredDoc
                } else {
                    Context::Commentary
                };

                read_line(
                    path,
                    source,
                    offset,
                    trimmed,
                    context,
                    &mut citations,
                    &mut findings,
                );
            }

            offset += line.len();
        }

        previous_end = Some(region.end());
    }

    close_dangling_fence(path, source, fenced, fence_opened_at, &mut findings);

    (citations, findings)
}

/// Whether a comment region beginning at `start` continues the document the
/// region ending at `previous_end` began, rather than starting a new one.
///
/// Two regions continue the same document exactly when nothing but
/// whitespace stands between them: consecutive lines of one doc comment, or
/// consecutive lines inside one block comment, read on as the single markdown
/// document rustdoc would build from them. Anything else between them — code,
/// a blank line that is not itself commentary, another item entirely — starts
/// a new document, the way a new item's own documentation would.
///
/// `start` is a region's interior, which always begins two bytes past its
/// comment's own opening marker (`//` or `/*`, both two ASCII bytes): the gap
/// is measured up to the marker, not past it, so the marker's own two bytes —
/// present between any two consecutive line comments and never whitespace —
/// are not mistaken for a break between documents.
fn continues_previous_region(source: &str, previous_end: Option<usize>, start: usize) -> bool {
    let marker_start = start.saturating_sub(2);

    previous_end.is_some_and(|end| {
        source
            .get(end..marker_start)
            .is_some_and(|gap| gap.chars().all(char::is_whitespace))
    })
}

/// Report a fence that was still open when the document containing it ended.
///
/// A fence a document never closed leaves everything after the opening
/// delimiter unscanned for the rest of that document — silently, since the
/// scan simply stopped reading spans rather than reporting anything — so the
/// defect is reported at the opening delimiter itself, the location that
/// names the fence responsible.
fn close_dangling_fence(
    path: &Path,
    source: &str,
    fenced: bool,
    opened_at: usize,
    findings: &mut Vec<Finding>,
) {
    if fenced {
        findings.push(Finding::UnclosedCommentaryFence {
            location: Location::new(path, source, opened_at),
        });
    }
}

/// Where a scanned line of commentary stands, which decides who reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    /// Ordinary commentary: every span classifies here.
    Commentary,
    /// A covered asset's documentation comment, away from its standard places.
    ///
    /// Bare spans of the profiled kinds are their profiles' reports here — the
    /// test profile's misplaced-label rule and the claim profile's placement
    /// rules already fail them — and one defect keeps one report.
    CoveredDoc,
    /// The committed in-file index: a generated region participating in full.
    ///
    /// Its citations resolve like any others, and a bare span in it is the
    /// generator minting without warrant — the generated-compliance invariant
    /// (ADR-L-014, A calculus of documentation and source labels) — since no adopted profile
    /// sets a standard place in a generated register.
    Generated,
}

/// Whether a span that is not a hugged citation still stands in a parenthesis.
///
/// This decides which report a failed span gets, never whether it gets one: a
/// parenthesis around or beside the span reads as citation intent that missed
/// the form, and anything else is a bare span standing free.
fn stands_in_parenthesis(line: &str, span: &CodeSpan<'_>) -> bool {
    let depth = line[..span.start]
        .chars()
        .fold(0_usize, |depth, character| match character {
            '(' => depth + 1,
            ')' => depth.saturating_sub(1),
            _ => depth,
        });

    depth > 0 || line[span.end..].starts_with(')')
}

/// Classify every span one line of commentary stands.
fn read_line(
    path: &Path,
    source: &str,
    base: usize,
    line: &str,
    context: Context,
    citations: &mut Vec<CodeCitation>,
    findings: &mut Vec<Finding>,
) {
    for span in code_spans(line) {
        let location = Location::new(path, source, base + span.start);

        match read_span(span.interior, span.parenthesized) {
            Reading::Occurrence {
                form: Form::Mint,
                label,
            } => {
                if label.kind() == TODO_KIND {
                    // The todo profile's bare spans are its own census
                    // wherever they stand, its orphan report included.
                    continue;
                }

                if is_legacy_kind(label.kind()) {
                    // The marked legacy profile owns its standard places and
                    // reports labels of its kind standing anywhere else.
                    continue;
                }

                if label.kind() == CONST_KIND {
                    // The constant profile's bare spans are its own census
                    // wherever they stand: its identity mints, the generated
                    // pins beneath them, and the orphans it reports itself. Its
                    // pin line is the corpus's first adopted standard place for
                    // a generated mint, so a bare span there is warranted where
                    // every other bare span in generated output is not.
                    continue;
                }

                if context == Context::CoveredDoc
                    && (label.kind() == TEST_KIND || label.kind() == CLAIM_KIND)
                {
                    // The test profile's misplaced-label rule and the claim
                    // profile's placement rules already fail these, and one
                    // defect keeps one report.
                    continue;
                }

                findings.push(if context == Context::Generated {
                    Finding::BareGeneratedOccurrence { label, location }
                } else if stands_in_parenthesis(line, &span) {
                    Finding::MalformedCommentaryCitation { label, location }
                } else {
                    Finding::UnwarrantedCommentaryMint { label, location }
                });
            }
            Reading::Occurrence { form, label } => citations.push(CodeCitation {
                path: path.to_path_buf(),
                occurrence: Occurrence::new(form, label, Syntax::Code, location),
                generated: context == Context::Generated,
            }),
            Reading::NonParenthesizedImport { prefix, label } => {
                findings.push(Finding::NonParenthesizedImport {
                    prefix,
                    label,
                    location,
                });
            }
            Reading::NearMiss { reason } => findings.push(Finding::NearMiss {
                text: span.interior.to_owned(),
                reason: (*reason).to_owned(),
                location,
            }),
            Reading::Text => {}
        }
    }

    warn_near_miss_forms(path, source, base, line, findings);
}

/// The reason a label-shaped backtick span in code draws.
///
/// The class is named in the record rather than invented here: the checker should
/// warn on near-miss spans, and in scanned code text on label-shaped backtick
/// spans where an acute was meant, without treating any of them as occurrences
/// (´[EMBER-inv:labels:total-resolution]´) — and the implementation gate repeats
/// the same duty in the same words (´[EMBER-gate:labels:implementation]´). So the
/// value says what the record says the warning is about, and the doubled display
/// form stays inert because a span shown is not a span offered.
///
/// ´const:emberlinter:prose-delimiter-near-miss´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:prose-delimiter-near-miss-text-x1940ae5c´
const BACKTICK_REASON: &str = "backtick delimiters where the acute syntax was meant";

/// Warn on the near-miss forms that stand outside acute spans.
///
/// Two classes, both warnings and neither an occurrence, per the
/// total-resolution invariant of ADR-L-014, A calculus of documentation and
/// source labels. A label-shaped span in single
/// backticks on the code surface used the prose delimiters where the acute was
/// meant — doubled backticks stay the display form and stay inert. A
/// parenthesis directly wrapping bracketed import-shaped text with no span
/// delimiters at all wrote the exact bytes a citation means, minus the marks
/// that would make it one, and is otherwise invisible because span reading
/// starts at a delimiter.
fn warn_near_miss_forms(
    path: &Path,
    source: &str,
    base: usize,
    line: &str,
    findings: &mut Vec<Finding>,
) {
    for span in backtick_spans(line) {
        if matches!(
            read_span(span.interior, span.parenthesized),
            Reading::Occurrence { .. } | Reading::NonParenthesizedImport { .. }
        ) {
            findings.push(Finding::NearMiss {
                text: span.interior.to_owned(),
                reason: BACKTICK_REASON.to_owned(),
                location: Location::new(path, source, base + span.start),
            });
        }
    }

    for (start, interior) in undelimited_imports(line) {
        findings.push(Finding::NearMiss {
            text: interior.to_owned(),
            reason: UNDELIMITED_IMPORT_REASON.to_owned(),
            location: Location::new(path, source, base + start),
        });
    }
}

/// A single-backtick span of a code comment line, read like a `CodeSpan`.
struct BacktickSpan<'a> {
    interior: &'a str,
    parenthesized: bool,
    start: usize,
}

/// Every single-backtick span of a line, paired left to right.
///
/// A doubled backtick run is the display form and yields no span; the acute
/// spans of the line are not excluded here because a backtick standing inside
/// one would already have made its interior no label.
fn backtick_spans(line: &str) -> Vec<BacktickSpan<'_>> {
    let mut spans = Vec::new();
    let mut open: Option<usize> = None;
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }

        let run = bytes[index..]
            .iter()
            .take_while(|byte| **byte == b'`')
            .count();

        if run != 1 {
            // A doubled delimiter is the display form: it opens no span, and
            // it closes none, so a pending single backtick stays pending.
            index += run;
            continue;
        }

        match open.take() {
            None => open = Some(index),
            Some(start) => {
                let interior = &line[start + 1..index];
                let before = line[..start].chars().next_back();
                let after = line[index + 1..].chars().next();

                spans.push(BacktickSpan {
                    interior,
                    parenthesized: before == Some('(') && after == Some(')'),
                    start,
                });
            }
        }

        index += 1;
    }

    spans
}

/// Take the citations every censused Rust source of a workspace stands.
///
/// The trees read are the ones the censuses already read, so the surface a
/// citation may stand on is the surface an asset may stand on, and no source is
/// checked for one purpose and passed over for the other. The covered assets
/// travel in because the scan's pass-over is position-aware: which lines are a
/// profile's standard places is a fact about the census, and only the census
/// can say it.
#[must_use]
#[cfg(test)]
pub fn take_code_citations(
    root: &Path,
    packages: &[Package],
    assets: &[CoveredAsset],
    corpus: &CorpusPlan,
) -> (Vec<CodeCitation>, Vec<Finding>) {
    let mut citations = Vec::new();
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();
    let by_source = crate::index::by_source(assets);

    for package in packages {
        let mut paths = Vec::new();

        for directory in CENSUSED_DIRECTORIES {
            collect_rust_sources(corpus, &package.directory().join(directory), &mut paths);
        }

        paths.sort();

        for path in paths {
            if !seen.insert(path.clone()) {
                continue;
            }

            match fs::read_to_string(root.join(&path)) {
                Ok(text) => {
                    let standing = by_source
                        .get(&path)
                        .map_or(&[] as &[&CoveredAsset], Vec::as_slice);
                    let (read, reported) = scan_code_citations(&path, &text, standing);
                    citations.extend(read);
                    findings.extend(reported);
                }
                Err(error) => findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                }),
            }
        }
    }

    (citations, findings)
}

/// Take commentary citations from the execution plan's finite source projection.
#[must_use]
pub fn take_planned_code_citations(
    root: &Path,
    sources: &[ProfileSource],
    assets: &[CoveredAsset],
) -> (Vec<CodeCitation>, Vec<Finding>) {
    let mut citations = Vec::new();
    let mut findings = Vec::new();
    let by_source = crate::index::by_source(assets);

    for source in sources {
        let path = source.path();

        match fs::read_to_string(root.join(path)) {
            Ok(text) => {
                let standing = by_source
                    .get(path)
                    .map_or(&[] as &[&CoveredAsset], Vec::as_slice);
                let (read, reported) = scan_code_citations(path, &text, standing);
                citations.extend(read);
                findings.extend(reported);
            }
            Err(error) => findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            }),
        }
    }

    (citations, findings)
}

/// A label as it stands in the code syntax, for a message or a fixture.
#[must_use]
pub fn code_syntax(label: &Label) -> String {
    format!("{ACUTE}{label}{ACUTE}")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::scan_code_citations;
    use crate::finding::Finding;
    use crate::occurrence::{Form, Syntax};

    const ACUTE: char = '\u{b4}';

    /// A parenthesised occurrence standing in ordinary commentary is a citation
    /// the carrier reads, so a comment may point at any statement the corpus has
    /// established. Code and prose refer to one corpus.
    ///
    /// ´claim:carrier:a-parenthesised-span-of-commentary-is-a-citation´
    /// ´test:unit:reads-a-parenthesized-span-of-commentary-as-a-citation´
    #[test]
    fn reads_a_parenthesized_span_of_commentary_as_a_citation() {
        let source =
            format!("// the deficiency is ({ACUTE}todo:code:read-the-flag{ACUTE}) below\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 1);
        assert_eq!(
            citations[0].occurrence().label().to_string(),
            "todo:code:read-the-flag"
        );
        assert_eq!(*citations[0].occurrence().form(), Form::SameOwnerCitation);
        assert_eq!(citations[0].occurrence().syntax(), Syntax::Code);
        assert_eq!(
            findings.len(),
            0,
            "a well-formed citation is no defect: {findings:?}"
        );
    }

    /// The carrier never mints: a bare occurrence in commentary yields no mint
    /// and no citation from this scanner. What it yields is a report, because
    /// no census reaches commentary and a warrantless mint may not pass in
    /// silence.
    ///
    /// ´claim:carrier:the-code-carrier-reads-citations-and-never-mints´
    /// ´test:unit:reads-no-citation-from-a-bare-span´
    #[test]
    fn reads_no_citation_from_a_bare_span() {
        let source = format!("// a comment naming {ACUTE}sec:demo:witnesses{ACUTE} bare\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(
            citations.len(),
            0,
            "a bare span is a mint, and mints are not the carrier's"
        );
        assert_eq!(
            findings.len(),
            1,
            "what the carrier does not mint it reports"
        );
    }

    /// A bare span in commentary is reported as an unwarranted mint: the
    /// opening acute declared intent the line cannot discharge, since the
    /// profiles census their standard places and nothing else, so the span is
    /// a finding rather than a silence.
    ///
    /// ´claim:carrier:a-bare-span-in-commentary-is-reported-as-an-unwarranted-mint´
    /// ´test:unit:reports-a-bare-span-in-commentary-as-an-unwarranted-mint´
    #[test]
    fn reports_a_bare_span_in_commentary_as_an_unwarranted_mint() {
        let source = format!("// a comment naming {ACUTE}sec:demo:witnesses{ACUTE} bare\n");
        let (_citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        let [Finding::UnwarrantedCommentaryMint { label, location }] = findings.as_slice() else {
            panic!("expected one unwarranted commentary mint, got {findings:?}");
        };

        assert_eq!(label.to_string(), "sec:demo:witnesses");
        assert_eq!(location.line(), 1);
    }

    /// A parenthesis carrying prose beside a label is a malformed citation and
    /// is reported: citation intent that missed the hugging form may not lapse
    /// into text, since the acute's declared intent would be lost with it.
    ///
    /// ´claim:carrier:a-parenthesis-carrying-prose-beside-a-label-is-a-malformed-citation´
    /// ´test:unit:reports-a-parenthesis-carrying-prose-beside-a-span´
    #[test]
    fn reports_a_parenthesis_carrying_prose_beside_a_span() {
        let source = format!("// a comment reading (cites {ACUTE}sec:demo:witnesses{ACUTE})\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0, "a polluted parenthesis is no citation");

        let [Finding::MalformedCommentaryCitation { label, .. }] = findings.as_slice() else {
            panic!("expected one malformed commentary citation, got {findings:?}");
        };

        assert_eq!(label.to_string(), "sec:demo:witnesses");
    }

    /// Two labels sharing one parenthesis are two malformed citations: the
    /// grammar gives each citation its own parentheses, and each span whose
    /// hugging form the sharing breaks is reported on its own.
    ///
    /// (´claim:carrier:a-parenthesis-carrying-prose-beside-a-label-is-a-malformed-citation´)
    /// ´test:unit:reports-each-label-of-a-shared-parenthesis´
    #[test]
    fn reports_each_label_of_a_shared_parenthesis() {
        let source =
            format!("// see ({ACUTE}sec:demo:witnesses{ACUTE} {ACUTE}sec:demo:other{ACUTE})\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0, "a shared parenthesis hugs neither span");
        assert_eq!(
            findings.len(),
            2,
            "each broken span is its own report: {findings:?}"
        );
        assert!(
            findings
                .iter()
                .all(|finding| matches!(finding, Finding::MalformedCommentaryCitation { .. })),
            "both reports name the parenthesis: {findings:?}"
        );
    }

    /// An import-shaped span standing in commentary without its parentheses is
    /// the hard failure the grammar makes it everywhere else, and the carrier
    /// reports it rather than dropping it.
    ///
    /// (´claim:occurrence:an-import-without-parentheses-is-a-failure-not-text´)
    /// ´test:unit:reports-an-import-shaped-span-without-parentheses´
    #[test]
    fn reports_an_import_shaped_span_without_parentheses() {
        let source = format!("// see {ACUTE}[DEMO-sec:demo:witnesses]{ACUTE} for the argument\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::NonParenthesizedImport { .. }]
            ),
            "expected the non-parenthesised import report, got {findings:?}"
        );
    }

    /// A bare span of the to-do kind is the todo profile's to report wherever
    /// it stands — its notice heads carry one by design and its orphan report
    /// owns the rest — so the carrier stays silent about the kind and one
    /// defect keeps one report.
    ///
    /// ´claim:carrier:a-bare-todo-span-is-the-todo-profiles-report´
    /// ´test:unit:leaves-a-bare-todo-span-to-the-todo-profile´
    #[test]
    fn leaves_a_bare_todo_span_to_the_todo_profile() {
        let source = format!(
            "// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read it\n// mentions {ACUTE}todo:code:read-the-flag{ACUTE} in passing\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);
        assert_eq!(
            findings.len(),
            0,
            "the to-do kind is another profile's census: {findings:?}"
        );
    }

    /// A fenced documentation example is not commentary: what stands between
    /// the fences is shown rather than meant, so no span in it is read as a
    /// citation or reported as a defect.
    ///
    /// ´claim:carrier:a-fenced-documentation-example-is-not-commentary´
    /// ´test:unit:leaves-a-fenced-example-unscanned´
    #[test]
    fn leaves_a_fenced_example_unscanned() {
        let source = format!(
            "/// ```\n/// a bare {ACUTE}sec:demo:witnesses{ACUTE} span\n/// and (cites {ACUTE}sec:demo:other{ACUTE}) prose\n/// ```\n/// after ({ACUTE}sec:demo:witnesses{ACUTE}) cited\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(
            findings.len(),
            0,
            "a fenced span is shown, not meant: {findings:?}"
        );
        assert_eq!(citations.len(), 1, "scanning resumes when the fence closes");
        assert_eq!(
            citations[0].occurrence().label().to_string(),
            "sec:demo:witnesses"
        );
    }

    /// An unclosed fence in one item's documentation does not silence a
    /// citation in another's: fence state resets at each comment region's own
    /// boundary, the way rustdoc treats each item's documentation as its own
    /// markdown document. The unclosed document still draws its own report.
    ///
    /// ´claim:carrier:an-unclosed-fence-does-not-cross-a-documents-boundary´
    /// ´test:unit:resets-the-fence-at-the-next-documents-boundary´
    #[test]
    fn resets_the_fence_at_the_next_documents_boundary() {
        let source = format!(
            "/// ```\n/// unterminated example\nfn a() {{}}\n\n/// cites ({ACUTE}sec:demo:witnesses{ACUTE})\nfn b() {{}}\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(
            citations.len(),
            1,
            "b's document reads its citation although a's fence never closed: {citations:?}"
        );
        assert_eq!(
            citations[0].occurrence().label().to_string(),
            "sec:demo:witnesses"
        );

        let [Finding::UnclosedCommentaryFence { location }] = findings.as_slice() else {
            panic!("expected one unclosed-fence finding for a's document, got {findings:?}");
        };

        assert_eq!(
            location.line(),
            1,
            "the finding names the opening delimiter's own line"
        );
    }

    /// A comment region ending with its fence still open is a hard finding
    /// naming the opening delimiter: the content after it went unscanned in
    /// full, silently, and that silence is exactly what the finding reports.
    ///
    /// ´claim:carrier:a-fence-left-open-at-a-documents-end-is-reported-at-its-opening´
    /// ´test:unit:reports-one-unclosed-fence-at-its-opening-delimiter´
    #[test]
    fn reports_one_unclosed_fence_at_its_opening_delimiter() {
        let source = "/* one\n```\nnever closed */\n".to_owned();
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);

        let [Finding::UnclosedCommentaryFence { location }] = findings.as_slice() else {
            panic!("expected exactly one unclosed-fence finding, got {findings:?}");
        };

        assert_eq!(
            location.line(),
            2,
            "reported at the opening delimiter, not the region's end"
        );
    }

    /// The committed in-file index is a generated region participating in
    /// full: a citation standing in its rows resolves like any other, marked as
    /// generated so a failure blames the register rather than an author.
    ///
    /// ´claim:carrier:the-index-regions-citations-resolve-like-any-others´
    /// ´test:unit:reads-a-citation-out-of-the-index-region´
    #[test]
    fn reads_a_citation_out_of_the_index_region() {
        let source = format!(
            "//! # Test index\n//!\n//! | Test | Area | Claim |\n//! |------|------|-------|\n//! | [`a`] | demo | cites ({ACUTE}claim:demo:a-statement{ACUTE}) |\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(
            findings.len(),
            0,
            "a well-formed generated citation is no defect: {findings:?}"
        );
        assert_eq!(citations.len(), 1);
        assert_eq!(
            citations[0].occurrence().label().to_string(),
            "claim:demo:a-statement"
        );
        assert!(
            citations[0].is_generated(),
            "the citation knows it stands in generated output"
        );
    }

    /// A bare span in the committed index region is the generator minting
    /// without warrant: no adopted profile sets a standard place in a generated
    /// register, so the span is reported as the generator's defect rather than
    /// as an author's unwarranted mint, and a display form remains inert there.
    ///
    /// ´claim:carrier:a-bare-span-in-generated-output-is-the-generators-defect´
    /// ´test:unit:reports-a-bare-span-in-the-index-region-as-the-generators´
    #[test]
    fn reports_a_bare_span_in_the_index_region_as_the_generators() {
        let source = format!(
            "//! # Test index\n//!\n//! | Test | Area | Claim |\n//! |------|------|-------|\n//! | [`a`] | demo | naming {ACUTE}sec:demo:witnesses{ACUTE} and showing ``sec:demo:shown`` |\n\n// after, naming {ACUTE}sec:demo:witnesses{ACUTE} bare\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);

        let [
            Finding::BareGeneratedOccurrence { label, .. },
            Finding::UnwarrantedCommentaryMint { .. },
        ] = findings.as_slice()
        else {
            panic!("expected the generator's defect and the commentary mint, got {findings:?}");
        };

        assert_eq!(label.to_string(), "sec:demo:witnesses");
    }

    /// A covered test's documentation comment, and the covered assets that
    /// make its standard places owned positions.
    fn covered(source: &str) -> Vec<crate::profile::CoveredAsset> {
        let packages = vec![crate::workspace::Package::new(
            "ember-demo",
            "packages/demo",
        )];
        let tests = crate::census::scan_source(
            "ember-demo",
            Path::new("packages/demo/src/engine.rs"),
            source,
        )
        .expect("a Rust source");
        let (assets, findings) =
            crate::profile::cover(&packages, &crate::census::Census::from_tests(tests, 1));

        assert!(
            findings.is_empty(),
            "the fixture covers cleanly: {findings:?}"
        );

        assets
    }

    /// The pass-over is position-aware: the standard places of a covered asset
    /// — the derived label's final line and the claim line above it — are their
    /// profiles' own positions, and the carrier stays silent about them so
    /// that the profile owning each remains its only reporter. One defect
    /// keeps one report.
    ///
    /// ´claim:carrier:only-a-standard-place-a-profile-owns-is-passed-over´
    /// ´test:unit:passes-over-the-standard-places-of-a-covered-asset´
    #[test]
    fn passes_over_the_standard_places_of_a_covered_asset() {
        let source = format!(
            "/// Decodes a header.\n///\n/// {ACUTE}claim:demo:a-header-decodes{ACUTE}\n/// {ACUTE}test:unit:decodes-a-header{ACUTE}\n#[test]\nfn decodes_a_header() {{}}\n"
        );
        let assets = covered(&source);
        let borrowed: Vec<&crate::profile::CoveredAsset> = assets.iter().collect();
        let (citations, findings) =
            scan_code_citations(Path::new("packages/demo/src/engine.rs"), &source, &borrowed);

        assert_eq!(
            citations.len(),
            0,
            "both lines are owned standard places: {citations:?}"
        );
        assert_eq!(
            findings.len(),
            0,
            "an owned standard place is its profile's to report: {findings:?}"
        );
    }

    /// A label-alone line owes its silence to a profile owning the position,
    /// never to its shape: the same standard-place-shaped line standing in
    /// ordinary commentary is commentary, and its bare span is the unwarranted
    /// mint any bare span is. Silence cannot be had by shape alone.
    ///
    /// (´claim:carrier:only-a-standard-place-a-profile-owns-is-passed-over´)
    /// ´test:unit:reports-a-label-alone-line-in-ordinary-commentary´
    #[test]
    fn reports_a_label_alone_line_in_ordinary_commentary() {
        let source = format!("// {ACUTE}sec:demo:witnesses{ACUTE}\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::UnwarrantedCommentaryMint { .. }]
            ),
            "a label-alone line in commentary classifies like any bare span: {findings:?}"
        );
    }

    /// A bare span of a profiled kind inside a covered test's documentation is
    /// that profile's to report — the misplaced-label rule already fails it —
    /// so the carrier stays silent there and one defect keeps one report.
    ///
    /// (´claim:carrier:only-a-standard-place-a-profile-owns-is-passed-over´)
    /// ´test:unit:leaves-a-misplaced-test-label-to-its-profile´
    #[test]
    fn leaves_a_misplaced_test_label_to_its_profile() {
        let source = format!(
            "/// Mentions {ACUTE}test:unit:elsewhere{ACUTE} in its gloss.\n/// {ACUTE}test:unit:decodes-a-header{ACUTE}\n#[test]\nfn decodes_a_header() {{}}\n"
        );
        let assets = covered(&source);
        let borrowed: Vec<&crate::profile::CoveredAsset> = assets.iter().collect();
        let (citations, findings) =
            scan_code_citations(Path::new("packages/demo/src/engine.rs"), &source, &borrowed);

        assert_eq!(citations.len(), 0);
        assert_eq!(
            findings.len(),
            0,
            "the misplaced test label is the test profile's report: {findings:?}"
        );
    }

    /// A comment reaches across an ownership boundary by the ordinary import
    /// form, so no new mechanism is needed for code to cite another package's
    /// statement.
    ///
    /// ´claim:carrier:commentary-cites-another-owner-by-the-ordinary-import-form´
    /// ´test:unit:reads-an-imported-citation-out-of-commentary´
    #[test]
    fn reads_an_imported_citation_out_of_commentary() {
        let source = format!("// see ({ACUTE}[DEMO-sec:demo:witnesses]{ACUTE}) for the argument\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 1);
        assert!(
            matches!(
                citations[0].occurrence().form(),
                Form::ImportedCitation { .. }
            ),
            "expected an imported citation, got {:?}",
            citations[0].occurrence().form()
        );
        assert_eq!(
            findings.len(),
            0,
            "a well-formed import is no defect: {findings:?}"
        );
    }

    /// A citation a program carries as data is no citation: a span written
    /// inside a string literal, plain or raw, is a value rather than a reference
    /// to a statement.
    ///
    /// (´claim:comment:a-string-literal-is-never-read-as-commentary´)
    /// ´test:unit:reads-no-citation-a-string-literal-carries´
    #[test]
    fn reads_no_citation_a_string_literal_carries() {
        let source = format!(
            "let reject = \"({ACUTE}sec:demo:not-a-citation{ACUTE})\";\nlet bare = \"{ACUTE}sec:demo:witnesses{ACUTE}\";\n"
        );
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0, "a span a program carries is data");
        assert_eq!(
            findings.len(),
            0,
            "data is not commentary and reports nothing: {findings:?}"
        );
    }

    /// A label-shaped span in single backticks on the code surface is the
    /// prose delimiters where the acute syntax was meant, and draws a warning
    /// naming that repair without becoming an occurrence — while the doubled
    /// backticks stay the display form and stay inert.
    ///
    /// ´claim:carrier:a-label-shaped-backtick-span-in-code-warns´
    /// ´test:unit:warns-on-a-label-shaped-backtick-span-in-commentary´
    #[test]
    fn warns_on_a_label_shaped_backtick_span_in_commentary() {
        let source = "// per `sec:demo:witnesses`, shown as ``sec:demo:witnesses``\n";
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), source, &[]);

        assert_eq!(citations.len(), 0, "a near miss is never an occurrence");

        let [Finding::NearMiss { text, reason, .. }] = findings.as_slice() else {
            panic!("expected one warning for the single-backtick span alone, got {findings:?}");
        };

        assert_eq!(text, "sec:demo:witnesses");
        assert!(
            reason.contains("backtick"),
            "the warning names the repair: {reason}"
        );
    }

    /// The near-miss warnings of the prose surface reach the code surface too:
    /// an acute span one named repair from a label draws the warning naming
    /// that repair, and stays text rather than becoming an occurrence.
    ///
    /// (´claim:occurrence:a-span-one-repair-from-a-label-is-a-named-near-miss´)
    /// ´test:unit:warns-on-a-near-miss-acute-span´
    #[test]
    fn warns_on_a_near_miss_acute_span() {
        let source = format!("// nearly ({ACUTE}Sec:demo:witnesses{ACUTE}) cited\n");
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), &source, &[]);

        assert_eq!(citations.len(), 0);
        assert!(
            matches!(findings.as_slice(), [Finding::NearMiss { .. }]),
            "wrong casing warns on the code surface as it does in prose: {findings:?}"
        );
    }

    /// A parenthesis directly wrapping bracketed import-shaped text with no
    /// delimiters at all is the near-miss class no span reader can see, and
    /// the carrier warns on it rather than leaving the bytes silently inert.
    ///
    /// (´claim:occurrence:an-undelimited-bracketed-import-in-parentheses-warns´)
    /// ´test:unit:warns-on-an-undelimited-bracket-import-in-commentary´
    #[test]
    fn warns_on_an_undelimited_bracket_import_in_commentary() {
        let source = "// see ([DEMO-sec:demo:witnesses]) for the argument\n";
        let (citations, findings) = scan_code_citations(Path::new("a.rs"), source, &[]);

        assert_eq!(citations.len(), 0);

        let [Finding::NearMiss { text, reason, .. }] = findings.as_slice() else {
            panic!("expected one warning for the undelimited import, got {findings:?}");
        };

        assert_eq!(text, "[DEMO-sec:demo:witnesses]");
        assert!(
            reason.contains("delimiters"),
            "the warning names the repair: {reason}"
        );
    }
}
