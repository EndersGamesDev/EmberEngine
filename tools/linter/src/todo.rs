// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The todo profile of ADR-T-016: the census of deficiency notices, their
//! classification, their derivation, and the standard place.
//!
//! The profiles signature of ADR-T-014, A calculus of documentation and source labels, says
//! what a profile must fix, and the test profile next door fixes those five
//! things for tests. This module fixes them for the notices a source carries in
//! its comments, and the two profiles differ in one way worth stating before
//! any code is read: a test has an identifier the language gives it, and a
//! notice has not. What a notice has is the sentence its author wrote, so that
//! sentence is the identifier, and the name transformation reads its opening
//! words.
//!
//! That choice is what makes the derivation a derivation. The alternative — a
//! slug the author invents beside the label — would put the name in the notice
//! only so that the label could copy it, which is authorship wearing a
//! derivation's clothes and is refused by the rejected Ansatz on authored asset
//! labels (ADR-T-014, A calculus of documentation and source labels). Reading the words
//! that are already there keeps the label evidence of the source rather than a
//! second opinion about it, and it is what lets a sweep write every label in
//! the corpus mechanically.
//!
//! # What is read, and what is deliberately not
//!
//! Only comments are read, through the lexer next door, for the reason that
//! module exists: a marker word inside a string literal is data a program
//! carries, and counting it would make a census that could never reach zero
//! without renaming the code.
//!
//! Only the marker's own line feeds the name. A notice wrapping over several
//! comment lines is one notice, and its continuation is prose the label never
//! reads. Two properties follow, and both were wanted. A reader can confirm a
//! label against the line it stands on, without holding a paragraph in mind. And
//! re-wrapping the body of a long notice does not re-derive its label, which
//! reading the whole notice would have made it do.
//!
//! # The two censuses are one recognizer
//!
//! The check validates the labels that are written; the burn register counts the
//! notices that carry none. Both read a source through [`scan_todos`] here, for
//! the reason the burn module gives about its own families: a register counting
//! one thing while a gate judged another is a ratchet that cannot hold.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`classifies_the_two_areas`] | todo | A notice's area is decided by where it stands: sources under a package's test trees are test notices and the rest of its sources are code notices, while a file belonging to neither — a build script beside the manifest — carries no notice area at all. |
//! | [`reads_a_marker_only_as_a_whole_word`] | todo | The marker heads a notice only as a whole word: a longer word that merely contains the spelling heads nothing, while the marker itself opens a notice and the words after it are its summary. |
//! | [`reads_a_marker_only_at_the_start_of_a_comment_line`] | todo | A notice is headed only where the marker opens its comment line, so prose that mentions notices in passing is prose rather than a backlog item — including the prose that documents this very rule. |
//! | [`reads_no_marker_a_string_literal_carries`] | comment | cites (´claim:comment:a-string-literal-is-never-read-as-commentary´) |
//! | [`takes_the_first_six_words_of_the_summary`] | todo | A notice's name is its opening words, bounded so a long summary yields a short label and a short one yields all of it. Punctuation separates words rather than entering them, so a summary naming a function in code font still derives a well-formed name. |
//! | [`keeps_the_qualifier_out_of_the_derivation`] | todo | A qualifier in parentheses after the marker — a record reference, a date — is not part of the summary and so not part of the derived name, so a notice can be annotated without changing what it is called. |
//! | [`derives_the_records_worked_example`] | todo | A notice's label is derived rather than chosen: the area it stands in and the words of its summary determine the whole label, kind included, so nobody names a notice by hand. |
//! | [`refuses_a_summary_that_carries_no_word`] | todo | A summary carrying no word at all derives nothing: an empty one and one of bare punctuation both refuse, because a label needs a name and no name can be made from nothing. |
//! | [`accepts_a_notices_derived_label_at_the_standard_place`] | todo | A notice carrying its own derived label at the standard place is clean, counted as covered and labelled and tallied under its area. This is what a migrated notice looks like. |
//! | [`counts_an_unlabelled_notice_without_reporting_it`] | todo | An unlabelled notice is counted and not reported: the register measures the backlog while the check declines to fail on it, so the size of the work is visible without the work blocking every run. |
//! | [`reports_a_notice_label_that_is_not_the_derivation`] | todo | A label on a notice attests the derivation rather than choosing a name: one that is not what the summary derives is reported, showing both what was written and what the summary actually gives. |
//! | [`reports_a_todo_label_no_marker_heads`] | todo | A label of the notice kind minted where no marker heads anything is reported as orphaned: a derived kind may only be minted by the derivation that warrants it, never written into ordinary commentary. |
//! | [`leaves_a_citation_of_the_kind_alone`] | todo | Citing a notice is not minting one, so a comment referring to a deficiency by its label raises nothing. Code may point at the backlog without joining it. |
//! | [`reports_a_notice_collision_with_both_locations`] | todo | The derivation must be injective within an owner: two notices deriving one label are a collision reported with both locations, so two distinct deficiencies can never hide behind a single name. |
//! | [`leaves_one_summary_in_two_owners_alone`] | todo | That injectivity is required within an owner and not across owners: two packages may each carry a notice with the same summary without colliding, so packages need not coordinate their backlogs. |
//! | [`reads_a_block_comments_continuation_leaders`] | todo | A block comment's decorative continuation leaders are resolved away before a line is read, so a notice written in the middle of a formatted block is found and its summary does not begin with the decoration. |

use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(test)]
use crate::census::{CENSUSED_DIRECTORIES, collect_rust_sources};
use crate::comment::{LEADERS, comment_regions};
use crate::finding::{Finding, Location};
use crate::label::Label;
#[cfg(test)]
use crate::plan::CorpusPlan;
use crate::plan::ProfileSource;
use crate::profile::{ACUTE, code_spans};
use crate::workspace::Package;

/// The kind token this profile governs.
///
/// The record catalogues the To-do environment against this spelling in its own
/// local extension row, the registry of ADR-T-011 naming no environment for a
/// deficiency notice: a notice is neither a remark on the work nor an
/// annotation decoding displayed material (´[ORCHESTRATION-sig:todos:kind-extension]´).
///
/// ´const:indexlinter:notice-kind-token´ (´[ORCHESTRATION-alg:const:word]´)
/// ´const:indexlinter:notice-kind-token-word-todo´
pub const TODO_KIND: &str = "todo";

/// The marker spellings the census recognizes.
///
/// ADR-T-016 censuses all three so that no synonym escapes the policy by being
/// spelled differently, and lets none of them reach the label: the corpus writes
/// the first, and grading deficiencies by their marker word is a taxonomy nobody
/// maintains (´[ORCHESTRATION-conv:todos:census]´).
///
/// ´const:indexlinter:notice-marker-spellings´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:notice-marker-spellings-form-xf1db7b8e´
pub const TODO_MARKERS: &[&str] = &["TODO", "FIXME", "XXX"];

/// How many words of a summary the name transformation takes.
///
/// The record fixes the figure outright, and what it buys is a name a reader can
/// confirm against the line it stands on: the transformation reads the words the
/// author wrote after the marker and takes the first six, lowercased and joined
/// by hyphens (´[ORCHESTRATION-conv:todos:profile]´). Only the marker's own line feeds
/// the name, so the span is bounded by a line rather than by a notice, and a
/// continuation the label never reads cannot re-derive it.
///
/// ´const:indexlinter:notice-name-summary-span´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:indexlinter:notice-name-summary-span-count-6´
pub const NAME_WORDS: usize = 6;

/// The area a covered notice's structural home assigns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoArea {
    /// A notice under the package's `src/`, outside `src/tests/`.
    Code,
    /// A notice under `src/tests/`, or under the package's own `tests/`.
    Test,
}

impl TodoArea {
    /// The area's word, as it stands in a label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Test => "test",
        }
    }

    /// Every area, for reporting.
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::Code, Self::Test]
    }
}

/// One deficiency notice, as the census reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoNotice {
    package: String,
    path: PathBuf,
    marker: String,
    summary: String,
    carried: Option<Label>,
    location: Location,
}

impl TodoNotice {
    /// The crate name of the package that owns this notice.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The source file, relative to the workspace root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The marker spelling that heads the notice.
    #[must_use]
    pub fn marker(&self) -> &str {
        &self.marker
    }

    /// The notice's summary: the marker's line, which the name transformation
    /// reads.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// The label standing at the notice's standard place, when one does.
    #[must_use]
    pub const fn carried(&self) -> Option<&Label> {
        self.carried.as_ref()
    }

    /// Whether the notice carries no label at its standard place.
    #[must_use]
    pub const fn is_unlabelled(&self) -> bool {
        self.carried.is_none()
    }

    /// Where the marker stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// Transform a notice's summary into a label's name segment.
///
/// The summary is lowercased, every character outside the label alphabet is read
/// as a separator, and the first [`NAME_WORDS`] words are joined by hyphens. A
/// summary yielding no word yields no name, which is the underivable case: a
/// deficiency that says nothing cannot be tracked.
#[must_use]
pub fn transform_summary(summary: &str) -> Option<String> {
    let lowered = summary.to_lowercase();
    let words: Vec<&str> = lowered
        .split(|character: char| !(character.is_ascii_lowercase() || character.is_ascii_digit()))
        .filter(|word| !word.is_empty())
        .take(NAME_WORDS)
        .collect();

    (!words.is_empty()).then(|| words.join("-"))
}

/// Classify a source file by its structural home within its package.
///
/// The package directory is relative to the workspace root, and empty for a root
/// package. Returns `None` for a file under neither censused root, which the
/// census does not produce.
#[must_use]
pub fn classify_todo(package_directory: &Path, path: &Path) -> Option<TodoArea> {
    let relative = path.strip_prefix(package_directory).ok()?;

    if relative.starts_with("tests") || relative.starts_with("src/tests") {
        return Some(TodoArea::Test);
    }

    relative.starts_with("src").then_some(TodoArea::Code)
}

/// Derive the label of a covered notice from its area and its summary.
#[must_use]
pub fn derive_todo(area: TodoArea, summary: &str) -> Option<Label> {
    let name = transform_summary(summary)?;

    Label::parse(&format!("{TODO_KIND}:{}:{name}", area.as_str()))
}

/// The label as it stands at a standard place, in the code syntax.
#[must_use]
pub fn standard_place_text(label: &Label) -> String {
    format!("{ACUTE}{label}{ACUTE}")
}

/// Whether a line, once its leaders are resolved away, is headed by a marker.
///
/// Returns the marker and the text following it. The marker must stand whole: a
/// longer word carrying one of the spellings heads no notice.
fn marker_of(line: &str) -> Option<(&'static str, &str)> {
    let stripped = line.trim_start_matches(LEADERS);

    TODO_MARKERS.iter().find_map(|marker| {
        let rest = stripped.strip_prefix(*marker)?;
        let continues = rest
            .chars()
            .next()
            .is_some_and(|character| character.is_alphanumeric() || character == '_');

        (!continues).then_some((*marker, rest))
    })
}

/// Read a marker's line into its qualifier, its standard place, and its summary.
///
/// The line is read as the marker, then an optional parenthesized qualifier,
/// then the standard place, then an optional colon, then the summary. The
/// qualifier is the shape this corpus already writes where a notice names the
/// record whose work it awaits; ADR-T-016 lets it stand and keeps it out of
/// every derivation. It is handed back here — without its parentheses — only so
/// that a sweep rewriting the line can put it back where it stood, which is the
/// whole of what "neither requires it nor retires it" costs in code.
fn read_notice(rest: &str) -> (Option<&str>, Option<Label>, String) {
    let mut tail = rest.trim_start();
    let mut qualifier = None;

    if let Some(after) = tail.strip_prefix('(')
        && let Some(close) = after.find(')')
    {
        qualifier = Some(&after[..close]);
        tail = after[close + 1..].trim_start();
    }

    let carried = match leading_label(tail) {
        Some((label, width)) => {
            tail = tail[width..].trim_start();

            Some(label)
        }
        None => None,
    };

    let summary = tail.strip_prefix(':').unwrap_or(tail).trim();

    (qualifier, carried, summary.to_owned())
}

/// The label opening a text as a bare acute span, with the bytes it occupies.
fn leading_label(tail: &str) -> Option<(Label, usize)> {
    let span = code_spans(tail).into_iter().next()?;

    if span.start != 0 || span.parenthesized {
        return None;
    }

    let label = Label::parse(span.interior)?;

    (label.kind() == TODO_KIND).then_some((label, span.end))
}

/// Read one Rust source for the notices its comments carry, and the labels of
/// this kind standing where no notice heads a line.
#[must_use]
pub fn scan_todos(package: &str, path: &Path, source: &str) -> (Vec<TodoNotice>, Vec<Finding>) {
    let mut notices = Vec::new();
    let mut orphans = Vec::new();

    for region in comment_regions(source) {
        let text = region.text(source);
        let mut offset = region.start();

        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);

            match marker_of(trimmed) {
                Some((marker, rest)) => {
                    let (_qualifier, carried, summary) = read_notice(rest);
                    let marker_offset = offset + (trimmed.len() - rest.len() - marker.len());

                    notices.push(TodoNotice {
                        package: package.to_owned(),
                        path: path.to_path_buf(),
                        marker: marker.to_owned(),
                        summary,
                        carried,
                        location: Location::new(path, source, marker_offset),
                    });
                }
                None => orphans.extend(orphan_labels(path, source, offset, trimmed)),
            }

            offset += line.len();
        }
    }

    (notices, orphans)
}

/// Every label of this kind minted on a line no marker heads.
///
/// A citation of the kind — the label parenthesised — is left alone, because a
/// citation consumes a mint and never claims to be one.
fn orphan_labels(path: &Path, source: &str, base: usize, line: &str) -> Vec<Finding> {
    code_spans(line)
        .into_iter()
        .filter(|span| !span.parenthesized)
        .filter_map(|span| {
            let label = Label::parse(span.interior)?;

            (label.kind() == TODO_KIND).then(|| Finding::OrphanTodoLabel {
                label,
                location: Location::new(path, source, base + span.start),
            })
        })
        .collect()
}

/// What a marker's line needs, read against the label its notice derives.
///
/// The three cases are the three a sweep must count apart, and they are the same
/// three the test profile's sweep counts: the place is already right, the place
/// is empty, or the place holds something the writer would not have written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The line already reads exactly as the writer would have written it.
    Kept,
    /// The line carried no label, and reads so with one written in.
    Written(String),
    /// The line carried a label the writer would not have written — another
    /// name, or the derivation in a form that is not the standard one — and
    /// reads so with the derivation standing as the writer renders it.
    Rewritten(String),
}

/// Rewrite a marker's line so the notice's label stands at its standard place.
///
/// The line is rebuilt rather than patched: everything before the marker is kept
/// byte for byte — a comment's leaders, its indentation, and any code standing
/// before a trailing comment — and everything after it is re-emitted in the
/// order (ADR-T-016, The TODO label profile) fixes, which is marker, qualifier, label, colon,
/// summary. Rebuilding is what makes the sweep idempotent by construction: the
/// line it writes is the line its own reader parses back to the same three
/// parts, so a second sweep reaches [`Placement::Kept`] and stops.
///
/// Only the marker's own line is touched. A notice wrapping over several comment
/// lines keeps its continuation exactly as it stands, because the continuation is
/// prose the label never reads.
///
/// # One renderer decides what standing right means
///
/// [`Placement::Kept`] is decided by rendering the line and comparing, never by
/// asking whether the carried label happens to equal the derivation. The two are
/// not the same question: a line can carry exactly the right label in a form this
/// writer would never emit — no colon after the label, spacing loose around it —
/// and answering the second question keeps such a line for ever, so it is
/// reported unchanged by every sweep and converges on nothing. Rendering first
/// leaves one definition of the standard place in the code, the writer's own, and
/// the sweep repairs everything that differs from it — the order
/// (ADR-T-016, The TODO label profile) fixes, and nothing beside it. The test profile
/// repairs its same class, a right label pressed against the prose above it, and
/// the profiles agree.
///
/// # Errors
///
/// Returns a reason when the recorded offset does not open with the recorded
/// marker — a source edited under the census — or when the notice's summary is
/// empty, which derives no name and is a defect of the notice rather than of its
/// label.
pub fn place_label(
    source: &str,
    offset: usize,
    marker: &str,
    label: &Label,
) -> Result<Placement, String> {
    let tail = source
        .get(offset..)
        .ok_or_else(|| "the notice's recorded offset falls outside its source".to_owned())?;
    let line_tail = tail
        .split('\n')
        .next()
        .unwrap_or(tail)
        .trim_end_matches('\r');
    let after_marker = line_tail
        .strip_prefix(marker)
        .ok_or_else(|| "no marker stands where the census recorded one".to_owned())?;

    let line_start = source
        .get(..offset)
        .ok_or_else(|| "the notice's recorded offset falls outside its source".to_owned())?
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = &source[line_start..offset];

    let (qualifier, carried, summary) = read_notice(after_marker);

    if summary.is_empty() {
        return Err("its summary is empty, so no name derives from it".to_owned());
    }

    let qualifier = qualifier.map_or_else(String::new, |text| format!("({text})"));
    let rendered = format!(
        "{marker}{qualifier} {}: {summary}",
        standard_place_text(label)
    );

    if rendered == line_tail {
        return Ok(Placement::Kept);
    }

    let line = format!("{prefix}{rendered}");

    Ok(if carried.is_some() {
        Placement::Rewritten(line)
    } else {
        Placement::Written(line)
    })
}

/// One covered notice, paired with the label its summary derives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoveredNotice {
    notice: TodoNotice,
    area: TodoArea,
    label: Label,
}

impl CoveredNotice {
    /// The notice this covers.
    #[must_use]
    pub const fn notice(&self) -> &TodoNotice {
        &self.notice
    }

    /// The area its structural home assigns it.
    #[must_use]
    pub const fn area(&self) -> TodoArea {
        self.area
    }

    /// The label its summary derives.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }
}

/// Pair every covered notice of a census with the label it derives.
///
/// A notice whose summary derives no name is reported rather than covered: there
/// is no label for a sweep to write, and the repair is to say something.
#[must_use]
pub fn cover_todos(
    packages: &[Package],
    census: &TodoCensus,
) -> (Vec<CoveredNotice>, Vec<Finding>) {
    let directories: BTreeMap<&str, &Path> = packages
        .iter()
        .map(|package| (package.name(), package.directory()))
        .collect();
    let mut covered = Vec::new();
    let mut findings = Vec::new();

    for notice in census.notices() {
        let Some(directory) = directories.get(notice.package()) else {
            continue;
        };

        let Some(area) = classify_todo(directory, notice.path()) else {
            continue;
        };

        match derive_todo(area, notice.summary()) {
            Some(label) => covered.push(CoveredNotice {
                notice: notice.clone(),
                area,
                label,
            }),
            None => findings.push(Finding::UnderivableAssetName {
                owner: notice.package().to_owned(),
                asset: notice.summary().to_owned(),
                transformed: String::new(),
                location: notice.location().clone(),
            }),
        }
    }

    (covered, findings)
}

/// The census over a workspace, together with what it took to read it.
#[derive(Debug, Clone, Default)]
pub struct TodoCensus {
    notices: Vec<TodoNotice>,
    files_scanned: usize,
}

impl TodoCensus {
    /// Build a census from notices already scanned.
    ///
    /// Exposed for tests that assemble a census from source text rather than from
    /// a tree on disk.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_notices(notices: Vec<TodoNotice>, files_scanned: usize) -> Self {
        Self {
            notices,
            files_scanned,
        }
    }

    /// Every covered notice, ordered by source path and position.
    #[must_use]
    pub fn notices(&self) -> &[TodoNotice] {
        &self.notices
    }

    /// How many Rust sources the census read.
    #[must_use]
    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }
}

/// Take the todo census of every package of a workspace.
#[must_use]
#[cfg(test)]
pub fn take_todo_census(
    root: &Path,
    packages: &[Package],
    corpus: &CorpusPlan,
) -> (TodoCensus, Vec<Finding>) {
    let mut census = TodoCensus::default();
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();

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

            let text = match fs::read_to_string(root.join(&path)) {
                Ok(text) => text,
                Err(error) => {
                    findings.push(Finding::TraversalFailure {
                        path: path.to_string_lossy().into_owned(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };

            census.files_scanned += 1;

            let (notices, orphans) = scan_todos(package.name(), &path, &text);

            census.notices.extend(notices);
            findings.extend(orphans);
        }
    }

    (census, findings)
}

/// Take the to-do census from the execution plan's finite source projection.
#[must_use]
pub fn take_planned_todo_census(
    root: &Path,
    sources: &[ProfileSource],
) -> (TodoCensus, Vec<Finding>) {
    let mut census = TodoCensus::default();
    let mut findings = Vec::new();

    for source in sources {
        let path = source.path();
        let text = match fs::read_to_string(root.join(path)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        census.files_scanned += 1;
        let (notices, orphans) = scan_todos(source.package(), path, &text);
        census.notices.extend(notices);
        findings.extend(orphans);
    }

    (census, findings)
}

/// What the todo pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TodoAnalysis {
    /// How many Rust sources the census read.
    pub files_scanned: usize,
    /// How many notices the census covers.
    pub covered: usize,
    /// How many covered notices fall in each area.
    pub by_area: BTreeMap<String, usize>,
    /// How many covered notices carry their derived label at the standard place.
    pub labelled: usize,
    /// How many carry no label at the standard place.
    ///
    /// These are the register's family rather than the check's findings, under
    /// the staged adoption ADR-T-016 records: the debt standing at adoption is
    /// counted and ratcheted, and growth fails through the burn list.
    pub unlabelled: usize,
    /// How many carry a label at the standard place that is not their own.
    pub wrong: usize,
    /// How many groups of labelled notices of one owner derive one label.
    pub collision_groups: usize,
    /// How many labelled notices stand in those groups.
    pub colliding_notices: usize,
    /// How many summaries transform into no well-formed name.
    pub underivable: usize,
}

/// Validate every covered notice of a census against the profile.
#[must_use]
pub fn analyze_todos(packages: &[Package], census: &TodoCensus) -> (TodoAnalysis, Vec<Finding>) {
    let directories: BTreeMap<&str, &Path> = packages
        .iter()
        .map(|package| (package.name(), package.directory()))
        .collect();

    let mut analysis = TodoAnalysis {
        files_scanned: census.files_scanned(),
        ..TodoAnalysis::default()
    };
    let mut findings = Vec::new();
    let mut labelled: Vec<(&TodoNotice, Label)> = Vec::new();

    for area in TodoArea::all() {
        analysis.by_area.insert(area.as_str().to_owned(), 0);
    }

    for notice in census.notices() {
        let Some(directory) = directories.get(notice.package()) else {
            continue;
        };

        let Some(area) = classify_todo(directory, notice.path()) else {
            continue;
        };

        analysis.covered += 1;
        *analysis
            .by_area
            .entry(area.as_str().to_owned())
            .or_default() += 1;

        let Some(carried) = notice.carried() else {
            analysis.unlabelled += 1;
            continue;
        };

        let Some(derived) = derive_todo(area, notice.summary()) else {
            analysis.underivable += 1;
            findings.push(Finding::UnderivableAssetName {
                owner: notice.package().to_owned(),
                asset: notice.summary().to_owned(),
                transformed: String::new(),
                location: notice.location().clone(),
            });
            continue;
        };

        if *carried == derived {
            analysis.labelled += 1;
            labelled.push((notice, derived));
        } else {
            analysis.wrong += 1;
            findings.push(Finding::WrongInventoryLabel {
                expected: derived,
                found: carried.clone(),
                owner: notice.package().to_owned(),
                asset: notice.summary().to_owned(),
                location: notice.location().clone(),
            });
        }
    }

    collisions(&labelled, &mut analysis, &mut findings);
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, findings)
}

/// Report every group of labelled notices of one owner deriving one label.
///
/// The inventory invariant (ADR-T-014, A calculus of documentation and source labels) makes a collision a
/// naming defect of the assets, repaired by rewording one of them. Only
/// labelled notices are grouped: an unlabelled one has attested nothing yet,
/// and reporting it here would report the marking wave's backlog as a defect of
/// the corpus.
fn collisions(
    labelled: &[(&TodoNotice, Label)],
    analysis: &mut TodoAnalysis,
    findings: &mut Vec<Finding>,
) {
    let mut groups: BTreeMap<(&str, &Label), Vec<&TodoNotice>> = BTreeMap::new();

    for (notice, label) in labelled {
        groups
            .entry((notice.package(), label))
            .or_default()
            .push(notice);
    }

    for ((owner, label), group) in groups {
        let Some((first, rest)) = group.split_first() else {
            continue;
        };

        if rest.is_empty() {
            continue;
        }

        analysis.collision_groups += 1;
        analysis.colliding_notices += group.len();

        for other in rest {
            findings.push(Finding::CollidingDerivation {
                asset: first.summary().to_owned(),
                owner: owner.to_owned(),
                first_label: label.clone(),
                second_label: label.clone(),
                first: first.location().clone(),
                second: other.location().clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        TodoArea, TodoCensus, analyze_todos, classify_todo, derive_todo, scan_todos,
        transform_summary,
    };
    use crate::finding::Finding;
    use crate::workspace::Package;

    const ACUTE: char = '\u{b4}';

    fn census_of(sources: &[(&str, &str)]) -> (Vec<Package>, TodoCensus, Vec<Finding>) {
        let packages = vec![Package::new("torrust-demo", "packages/demo")];
        let mut notices = Vec::new();
        let mut findings = Vec::new();

        for (path, text) in sources {
            let (found, orphans) = scan_todos("torrust-demo", Path::new(path), text);

            notices.extend(found);
            findings.extend(orphans);
        }

        (
            packages,
            TodoCensus::from_notices(notices, sources.len()),
            findings,
        )
    }

    fn analyze(sources: &[(&str, &str)]) -> (super::TodoAnalysis, Vec<Finding>) {
        let (packages, census, mut findings) = census_of(sources);
        let (analysis, profile_findings) = analyze_todos(&packages, &census);

        findings.extend(profile_findings);

        (analysis, findings)
    }

    /// A notice's area is decided by where it stands: sources under a package's
    /// test trees are test notices and the rest of its sources are code
    /// notices, while a file belonging to neither — a build script beside the
    /// manifest — carries no notice area at all.
    ///
    /// ´claim:todo:a-notices-area-is-decided-by-where-the-source-stands´
    /// ´test:unit:classifies-the-two-areas´
    #[test]
    fn classifies_the_two_areas() {
        let directory = PathBuf::from("packages/demo");

        assert_eq!(
            classify_todo(&directory, Path::new("packages/demo/src/engine.rs")),
            Some(TodoArea::Code)
        );
        assert_eq!(
            classify_todo(&directory, Path::new("packages/demo/src/tests/engine.rs")),
            Some(TodoArea::Test)
        );
        assert_eq!(
            classify_todo(&directory, Path::new("packages/demo/tests/engine.rs")),
            Some(TodoArea::Test)
        );
        assert_eq!(
            classify_todo(&directory, Path::new("packages/demo/build.rs")),
            None
        );
    }

    /// The marker heads a notice only as a whole word: a longer word that
    /// merely contains the spelling heads nothing, while the marker itself
    /// opens a notice and the words after it are its summary.
    ///
    /// ´claim:todo:the-marker-heads-a-notice-only-as-a-whole-word´
    /// ´test:unit:reads-a-marker-only-as-a-whole-word´
    #[test]
    fn reads_a_marker_only_as_a_whole_word() {
        let (notices, _orphans) = scan_todos(
            "torrust-demo",
            Path::new("a.rs"),
            "// TODOS are not notices\n",
        );

        assert_eq!(
            notices.len(),
            0,
            "a longer word carrying the spelling heads nothing"
        );

        let (found, _orphans) =
            scan_todos("torrust-demo", Path::new("a.rs"), "// TODO: a notice\n");

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].summary(), "a notice");
    }

    /// A notice is headed only where the marker opens its comment line, so
    /// prose that mentions notices in passing is prose rather than a backlog
    /// item — including the prose that documents this very rule.
    ///
    /// ´claim:todo:only-a-marker-opening-its-comment-line-heads-a-notice´
    /// ´test:unit:reads-a-marker-only-at-the-start-of-a-comment-line´
    #[test]
    fn reads_a_marker_only_at_the_start_of_a_comment_line() {
        let (notices, _orphans) = scan_todos(
            "torrust-demo",
            Path::new("a.rs"),
            "// a sentence mentioning TODO in passing\n",
        );

        assert_eq!(notices.len(), 0, "prose about notices is not a notice");
    }

    /// A marker a program carries as data heads no notice: a notice written
    /// inside a string literal, plain or raw, is a value and not a backlog
    /// item.
    ///
    /// (´claim:comment:a-string-literal-is-never-read-as-commentary´)
    /// ´test:unit:reads-no-marker-a-string-literal-carries´
    #[test]
    fn reads_no_marker_a_string_literal_carries() {
        let (notices, _orphans) = scan_todos(
            "torrust-demo",
            Path::new("a.rs"),
            "let reject = \"// TODO: not a notice\";\nlet raw = r#\"// TODO: nor this\"#;\n",
        );

        assert_eq!(notices.len(), 0, "a marker a program carries is data");
    }

    /// A notice's name is its opening words, bounded so a long summary yields a
    /// short label and a short one yields all of it. Punctuation separates
    /// words rather than entering them, so a summary naming a function in code
    /// font still derives a well-formed name.
    ///
    /// ´claim:todo:a-name-is-the-summarys-opening-words-with-punctuation-as-separators´
    /// ´test:unit:takes-the-first-six-words-of-the-summary´
    #[test]
    fn takes_the_first_six_words_of_the_summary() {
        assert_eq!(
            transform_summary("Read the policy flag before deciding, and then some more"),
            Some("read-the-policy-flag-before-deciding".to_owned())
        );
        assert_eq!(
            transform_summary("Remove this rate."),
            Some("remove-this-rate".to_owned())
        );
        assert_eq!(
            transform_summary("Use `Journal::open_after` here"),
            Some("use-journal-open-after-here".to_owned()),
            "punctuation separates words rather than entering them"
        );
    }

    /// A qualifier in parentheses after the marker — a record reference, a date
    /// — is not part of the summary and so not part of the derived name, so a
    /// notice can be annotated without changing what it is called.
    ///
    /// ´claim:todo:a-qualifier-after-the-marker-is-no-part-of-the-derivation´
    /// ´test:unit:keeps-the-qualifier-out-of-the-derivation´
    #[test]
    fn keeps_the_qualifier_out_of_the_derivation() {
        let (notices, _orphans) = scan_todos(
            "torrust-demo",
            Path::new("packages/demo/src/a.rs"),
            "// TODO(RECORD-1 2026-05-17): Accept the policy\n",
        );

        assert_eq!(notices[0].summary(), "Accept the policy");
        assert_eq!(
            derive_todo(TodoArea::Code, notices[0].summary())
                .expect("a well-formed label")
                .to_string(),
            "todo:code:accept-the-policy"
        );
    }

    /// A notice's label is derived rather than chosen: the area it stands in
    /// and the words of its summary determine the whole label, kind included,
    /// so nobody names a notice by hand.
    ///
    /// ´claim:todo:a-notices-whole-label-is-derived-from-its-area-and-summary´
    /// ´test:unit:derives-the-records-worked-example´
    #[test]
    fn derives_the_records_worked_example() {
        let label = derive_todo(TodoArea::Code, "read the policy flag before deciding")
            .expect("a well-formed label");

        assert_eq!(
            label.to_string(),
            "todo:code:read-the-policy-flag-before-deciding"
        );
        assert_eq!(label.kind(), super::TODO_KIND);
    }

    /// A summary carrying no word at all derives nothing: an empty one and one
    /// of bare punctuation both refuse, because a label needs a name and no
    /// name can be made from nothing.
    ///
    /// ´claim:todo:a-summary-with-no-word-in-it-derives-no-label´
    /// ´test:unit:refuses-a-summary-that-carries-no-word´
    #[test]
    fn refuses_a_summary_that_carries_no_word() {
        assert_eq!(transform_summary(""), None);
        assert_eq!(transform_summary("   ?!  "), None);
        assert!(derive_todo(TodoArea::Code, "").is_none());
    }

    /// A notice carrying its own derived label at the standard place is clean,
    /// counted as covered and labelled and tallied under its area. This is what
    /// a migrated notice looks like.
    ///
    /// ´claim:todo:a-notice-carrying-its-derived-label-is-clean-and-counted´
    /// ´test:unit:accepts-a-notices-derived-label-at-the-standard-place´
    #[test]
    fn accepts_a_notices_derived_label_at_the_standard_place() {
        let source = format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n");
        let (analysis, findings) = analyze(&[("packages/demo/src/a.rs", &source)]);

        assert_eq!(
            findings.len(),
            0,
            "a labelled notice is clean: {findings:?}"
        );
        assert_eq!(analysis.covered, 1);
        assert_eq!(analysis.labelled, 1);
        assert_eq!(analysis.by_area["code"], 1);
    }

    /// An unlabelled notice is counted and not reported: the register measures
    /// the backlog while the check declines to fail on it, so the size of the
    /// work is visible without the work blocking every run.
    ///
    /// ´claim:todo:an-unlabelled-notice-is-counted-and-not-reported´
    /// ´test:unit:counts-an-unlabelled-notice-without-reporting-it´
    #[test]
    fn counts_an_unlabelled_notice_without_reporting_it() {
        let (analysis, findings) =
            analyze(&[("packages/demo/src/a.rs", "// TODO: read the flag\n")]);

        assert_eq!(analysis.unlabelled, 1);
        assert_eq!(analysis.labelled, 0);
        assert_eq!(
            findings.len(),
            0,
            "the register counts the backlog; the check does not report it: {findings:?}"
        );
    }

    /// A label on a notice attests the derivation rather than choosing a name:
    /// one that is not what the summary derives is reported, showing both what
    /// was written and what the summary actually gives.
    ///
    /// ´claim:todo:a-label-that-is-not-the-derivation-is-reported-with-both´
    /// ´test:unit:reports-a-notice-label-that-is-not-the-derivation´
    #[test]
    fn reports_a_notice_label_that_is_not_the_derivation() {
        let source = format!("// TODO {ACUTE}todo:code:something-else{ACUTE}: read the flag\n");
        let (analysis, findings) = analyze(&[("packages/demo/src/a.rs", &source)]);

        assert_eq!(analysis.wrong, 1);
        let [
            Finding::WrongInventoryLabel {
                expected, found, ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one wrong label, got {findings:?}");
        };
        assert_eq!(expected.to_string(), "todo:code:read-the-flag");
        assert_eq!(found.to_string(), "todo:code:something-else");
    }

    /// A label of the notice kind minted where no marker heads anything is
    /// reported as orphaned: a derived kind may only be minted by the
    /// derivation that warrants it, never written into ordinary commentary.
    ///
    /// ´claim:todo:a-notice-label-no-marker-heads-is-orphaned´
    /// ´test:unit:reports-a-todo-label-no-marker-heads´
    #[test]
    fn reports_a_todo_label_no_marker_heads() {
        let source = format!("// a plain comment carrying {ACUTE}todo:code:read-the-flag{ACUTE}\n");
        let (_analysis, findings) = analyze(&[("packages/demo/src/a.rs", &source)]);

        assert!(
            matches!(findings.as_slice(), [Finding::OrphanTodoLabel { .. }]),
            "expected an orphan label, got {findings:?}"
        );
    }

    /// Citing a notice is not minting one, so a comment referring to a
    /// deficiency by its label raises nothing. Code may point at the backlog
    /// without joining it.
    ///
    /// ´claim:todo:citing-a-notice-is-not-minting-one´
    /// ´test:unit:leaves-a-citation-of-the-kind-alone´
    #[test]
    fn leaves_a_citation_of_the_kind_alone() {
        let source = format!("// see ({ACUTE}todo:code:read-the-flag{ACUTE}) for the deficiency\n");
        let (_analysis, findings) = analyze(&[("packages/demo/src/a.rs", &source)]);

        assert_eq!(findings.len(), 0, "a citation is not a mint: {findings:?}");
    }

    /// The derivation must be injective within an owner: two notices deriving
    /// one label are a collision reported with both locations, so two distinct
    /// deficiencies can never hide behind a single name.
    ///
    /// ´claim:todo:two-notices-deriving-one-label-collide-with-both-locations´
    /// ´test:unit:reports-a-notice-collision-with-both-locations´
    #[test]
    fn reports_a_notice_collision_with_both_locations() {
        let source = format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n");
        let (analysis, findings) = analyze(&[
            ("packages/demo/src/one.rs", &source),
            ("packages/demo/src/two.rs", &source),
        ]);

        assert_eq!(analysis.collision_groups, 1);
        assert_eq!(analysis.colliding_notices, 2);

        let [Finding::CollidingDerivation { first, second, .. }] = findings.as_slice() else {
            panic!("expected one collision, got {findings:?}");
        };
        assert_eq!(first.path(), Path::new("packages/demo/src/one.rs"));
        assert_eq!(second.path(), Path::new("packages/demo/src/two.rs"));
    }

    /// That injectivity is required within an owner and not across owners: two
    /// packages may each carry a notice with the same summary without
    /// colliding, so packages need not coordinate their backlogs.
    ///
    /// ´claim:todo:notice-derivations-collide-only-within-one-owner´
    /// ´test:unit:leaves-one-summary-in-two-owners-alone´
    #[test]
    fn leaves_one_summary_in_two_owners_alone() {
        let packages = vec![
            Package::new("torrust-one", "packages/one"),
            Package::new("torrust-two", "packages/two"),
        ];
        let source = format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n");

        let (mut notices, _orphans) =
            scan_todos("torrust-one", Path::new("packages/one/src/a.rs"), &source);
        let (other, _more) = scan_todos("torrust-two", Path::new("packages/two/src/a.rs"), &source);

        notices.extend(other);

        let census = TodoCensus::from_notices(notices, 2);
        let (analysis, findings) = analyze_todos(&packages, &census);

        assert_eq!(analysis.collision_groups, 0, "ownership disambiguates");
        assert_eq!(findings.len(), 0, "and nothing is reported: {findings:?}");
    }

    /// A block comment's decorative continuation leaders are resolved away
    /// before a line is read, so a notice written in the middle of a formatted
    /// block is found and its summary does not begin with the decoration.
    ///
    /// ´claim:todo:block-comment-continuation-leaders-are-resolved-away´
    /// ´test:unit:reads-a-block-comments-continuation-leaders´
    #[test]
    fn reads_a_block_comments_continuation_leaders() {
        let (notices, _orphans) = scan_todos(
            "torrust-demo",
            Path::new("packages/demo/src/a.rs"),
            "/*\n * TODO: read the flag\n */\n",
        );

        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].summary(), "read the flag");
    }
}
