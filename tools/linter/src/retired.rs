// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The two identity families the scenario matrix takes with it when it retires.
//!
//! ADR-T-017 retires a document that gave ninety-one promises numbers and sorted
//! them into twelve named divisions. Both identity schemes were the document's
//! own: nothing outside it defines what the ninetieth scenario is, and nothing
//! outside it says which promises "None of the above breaks under weird inputs"
//! covers. When the document goes, a reference to either names nothing at all.
//!
//! That is what makes them burn families rather than a style preference. The
//! burn discipline (ADR-T-020, The migration disciplines) requires that a legacy
//! reference family is enumerated in a register the linter verifies exactly and
//! that the enumeration only shrinks, and these two qualify on its
//! own terms: the corpus carries hundreds of such references, most of them in
//! the document that is going, and the rest scattered through test comments and
//! planning prose where they will quietly outlive their referent unless
//! something counts them.
//!
//! # These recognizers are the register's own, and may be
//!
//! The burn lists ordinarily read with the migration lint's recognizers, because
//! a register that counted one thing while a gate judged another could not
//! ratchet. That reasoning does not reach here, and its absence is the licence:
//! no gate judges these two shapes. There is no lint rule for a scenario number
//! and none for a division name, so the register is the only counter and cannot
//! disagree with a second one. The precedent is the unlabelled-notice family
//! beside it, which is read by its own profile's census for exactly this reason.
//!
//! Adding them to the migration lint instead was the alternative, and it was
//! rejected because the lint is held per document and rule by rule: every rule
//! added to it widens what fifteen already-migrated documents are held to, in the
//! commit that adds it, whether or not those documents ever carried the shape.
//!
//! # What each family is, exactly
//!
//! A scenario number is the hash mark and one or two digits naming a scenario of
//! the retired matrix, bounded to the range the matrix actually numbered. The
//! bound is not fastidiousness: an unbounded rule would count every hash and
//! number in the corpus — a percentage in a comment, an ordinal in a list — and a
//! register that counted those could never reach zero, because reaching zero
//! would mean rewriting prose that was never about the matrix.
//!
//! A division name is one of twelve sentences, matched entire. The names are
//! indicative sentences rather than nouns, which is what makes matching them
//! literally safe: "Stale state dissolves on a bounded schedule" is a thing
//! somebody wrote about this matrix, not a phrase that recurs by accident.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_the_scenario_numbers_of_a_sentence`] | retired | A scenario of the retired matrix is recognised by the mark and the one or two digits after it, wherever in a sentence it stands, and the whole run of them in a sentence is found rather than the first. A range counts as the endpoint that carries the mark and no more, because the mark is what makes a number a reference — so a corpus writing a span of scenarios owes the register one occurrence for it rather than as many as the span covers. |
//! | [`leaves_numbers_outside_the_matrix_range_alone`] | retired | The range the matrix numbered is the whole of the family: a number past its end is some other corpus's, and counting those would give the register a floor it could never reach, since reaching it would mean rewriting prose that was never about the matrix. |
//! | [`leaves_marks_that_open_no_token_alone`] | retired | The mark must open a token. A hash inside a word belongs to that word, a doubled hash opens no number, and a mark with no digits after it locates nothing — so an attribute, a heading, and an ordinary identifier all stay out of the census. |
//! | [`reads_a_division_name_written_into_a_sentence`] | retired | A division of the retired matrix is recognised by the sentence that headed it, matched entire. The names are indicative sentences rather than nouns, which is what makes a literal match safe: each is a thing somebody wrote about this matrix rather than a phrase that recurs by accident. |
//! | [`leaves_displayed_retired_forms_alone`] | retired | A form shown rather than written is left alone in either family: code font and fenced blocks exhibit a shape without referring to it, which is the boundary that lets a register's own preamble name what it counts without entering the census it describes. |
//! | [`reports_offsets_into_the_source_not_the_run`] | retired | Offsets are into the source rather than into the run, so a caller reading one comment out of a file still reports where in the file the reference stands. Without that a register would name every occurrence at the top of the source it was found in. |

use crate::burn::Recognizer;

/// One of the two identity schemes the retired matrix defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetiredFamily {
    /// A scenario of the matrix, named by its number.
    ScenarioNumber,
    /// A division of the matrix, named by the sentence that headed it.
    DivisionName,
}

/// One occurrence: the form as written, and where it stands in the source.
pub type Retired = (String, usize);

/// Read one run of plain text for a retired family's shape.
///
/// The text is a run standing at `base` in the source, and the offsets returned
/// are into the source, so a caller reading a fragment — one comment out of a
/// Rust file, say — still reports where the fragment stands.
#[must_use]
pub fn scan_retired_text(
    base: usize,
    text: &str,
    family: RetiredFamily,
    declared: &Recognizer,
) -> Vec<Retired> {
    match family {
        RetiredFamily::ScenarioNumber => {
            gathered(declared.marks(), |mark| mark.scan_text(base, text))
        }
        RetiredFamily::DivisionName => declared
            .literals()
            .map(|literals| literals.scan_text(base, text))
            .unwrap_or_default(),
    }
}

/// Gather what every declared instance of one family reads, in offset order.
///
/// A family declaring several instances is read by all of them, and the run they
/// find together is ordered by where the occurrences stand rather than by which
/// instance found them — because a register naming an occurrence at the top of a
/// file it does not stand in is a register nobody can repair from.
fn gathered<T>(instances: &[T], scan: impl Fn(&T) -> Vec<Retired>) -> Vec<Retired> {
    let mut found: Vec<Retired> = instances.iter().flat_map(scan).collect();

    found.sort_by_key(|(_text, offset)| *offset);
    found
}

/// Read one Markdown document for a retired family's shape.
///
/// Only running text is read. A form shown in code font or inside a fenced block
/// is exhibited rather than referred to, which is the same boundary the migration
/// lint draws and the same one that lets a register's own preamble name the shapes
/// it counts without entering its own census.
#[must_use]
pub fn scan_retired_markdown(
    source: &str,
    family: RetiredFamily,
    declared: &Recognizer,
) -> Vec<Retired> {
    match family {
        RetiredFamily::ScenarioNumber => {
            gathered(declared.marks(), |mark| mark.scan_markdown(source))
        }
        RetiredFamily::DivisionName => declared
            .literals()
            .map(|literals| literals.scan_markdown(source))
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{RetiredFamily, scan_retired_markdown, scan_retired_text};
    use crate::burn::Recognizer;
    use crate::program::{LiteralSet, MarkNumbered};

    /// The payload the retiring matrix's own documents declare.
    ///
    /// Written here rather than read from the corpus because these tests hold the
    /// *reading* to its rules — that a mark must open a token, that a range counts
    /// once — and a reading is tested against a payload rather than against a
    /// repository. That the documents declare exactly this payload is a different
    /// claim, and [`crate::snapshot`] holds it where the documents are loaded.
    fn declared() -> Recognizer {
        Recognizer::new(
            [MarkNumbered::new('#', 1, 91)],
            Some(
                LiteralSet::new([
                    "Information flows strictly forward",
                    "The core model and the decision layer are independent",
                    "Structure can change without destroying what was learned",
                    "Stale state dissolves on a bounded schedule",
                    "Learning converges despite censoring and class imbalance",
                    "The anchor provides coverage when the sister can't",
                    "The decision landscape faithfully reflects model belief",
                    "Calibration and companion trackers stay current",
                    "The fast path stays fast",
                    "The system knows when it's struggling",
                    "None of the above breaks under weird inputs",
                    "Recovery preserves state and failures stay explicit",
                ])
                .expect("the twelve division names are distinct, nonempty and unbroken"),
            ),
            [],
        )
    }

    fn texts(found: &[(String, usize)]) -> Vec<String> {
        found.iter().map(|(text, _offset)| text.clone()).collect()
    }

    fn numbers(text: &str) -> Vec<String> {
        texts(&scan_retired_text(
            0,
            text,
            RetiredFamily::ScenarioNumber,
            &declared(),
        ))
    }

    fn divisions(text: &str) -> Vec<String> {
        texts(&scan_retired_text(
            0,
            text,
            RetiredFamily::DivisionName,
            &declared(),
        ))
    }

    /// A scenario of the retired matrix is recognised by the mark and the one or
    /// two digits after it, wherever in a sentence it stands, and the whole run
    /// of them in a sentence is found rather than the first. A range counts as
    /// the endpoint that carries the mark and no more, because the mark is what
    /// makes a number a reference — so a corpus writing a span of scenarios owes
    /// the register one occurrence for it rather than as many as the span covers.
    ///
    /// ´claim:retired:a-scenario-number-is-the-mark-and-the-digits-after-it´
    /// ´test:unit:reads-the-scenario-numbers-of-a-sentence´
    #[test]
    fn reads_the_scenario_numbers_of_a_sentence() {
        assert_eq!(
            numbers("Covered by #59's witnesses, and by #71 and #9.\n"),
            ["#59", "#71", "#9"]
        );
        assert_eq!(numbers("#1"), ["#1"]);
        assert_eq!(
            numbers("The pair #26-27 splits either way.\n"),
            ["#26"],
            "the unmarked endpoint of a range is a number, not a reference"
        );
    }

    /// The range the matrix numbered is the whole of the family: a number past
    /// its end is some other corpus's, and counting those would give the
    /// register a floor it could never reach, since reaching it would mean
    /// rewriting prose that was never about the matrix.
    ///
    /// ´claim:retired:only-the-numbers-the-matrix-used-are-of-the-family´
    /// ´test:unit:leaves-numbers-outside-the-matrix-range-alone´
    #[test]
    fn leaves_numbers_outside_the_matrix_range_alone() {
        assert_eq!(
            numbers("Issue #92 and #99 are not scenarios.\n"),
            Vec::<String>::new()
        );
        assert_eq!(numbers("Neither is #0.\n"), Vec::<String>::new());
        assert_eq!(
            numbers("Nor pull request #857.\n"),
            Vec::<String>::new(),
            "a longer number is not read as a shorter one with a tail"
        );
    }

    /// The mark must open a token. A hash inside a word belongs to that word, a
    /// doubled hash opens no number, and a mark with no digits after it locates
    /// nothing — so an attribute, a heading, and an ordinary identifier all stay
    /// out of the census.
    ///
    /// ´claim:retired:the-mark-must-open-a-token-to-name-a-scenario´
    /// ´test:unit:leaves-marks-that-open-no-token-alone´
    #[test]
    fn leaves_marks_that_open_no_token_alone() {
        let quiet = [
            "The repository torrust-index#59 is another corpus.",
            "An attribute #[test] carries no number.",
            "A heading # 59 is spaced apart.",
            "A doubled ##59 opens nothing.",
            "An identifier a#59 is one word.",
        ];

        for source in quiet {
            assert_eq!(numbers(source), Vec::<String>::new(), "on: {source}");
        }
    }

    /// A division of the retired matrix is recognised by the sentence that
    /// headed it, matched entire. The names are indicative sentences rather than
    /// nouns, which is what makes a literal match safe: each is a thing somebody
    /// wrote about this matrix rather than a phrase that recurs by accident.
    ///
    /// ´claim:retired:a-division-name-is-its-heading-sentence-matched-entire´
    /// ´test:unit:reads-a-division-name-written-into-a-sentence´
    #[test]
    fn reads_a_division_name_written_into_a_sentence() {
        assert_eq!(
            divisions("The promise that Stale state dissolves on a bounded schedule.\n"),
            ["Stale state dissolves on a bounded schedule"]
        );
        assert_eq!(
            divisions("Under The fast path stays fast, latency is budgeted.\n"),
            ["The fast path stays fast"]
        );
        assert_eq!(
            divisions("Neither decay nor lifecycle is named here.\n"),
            Vec::<String>::new()
        );
    }

    /// A form shown rather than written is left alone in either family: code
    /// font and fenced blocks exhibit a shape without referring to it, which is
    /// the boundary that lets a register's own preamble name what it counts
    /// without entering the census it describes.
    ///
    /// ´claim:retired:a-retired-form-shown-rather-than-written-is-left-alone´
    /// ´test:unit:leaves-displayed-retired-forms-alone´
    #[test]
    fn leaves_displayed_retired_forms_alone() {
        let displayed = "A shape such as `#59` is named here.\n\n```text\n#71 and The fast path stays fast\n```\n";

        assert_eq!(
            texts(&scan_retired_markdown(
                displayed,
                RetiredFamily::ScenarioNumber,
                &declared()
            )),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan_retired_markdown(
                displayed,
                RetiredFamily::DivisionName,
                &declared()
            )),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan_retired_markdown(
                "Covered by #59 and #71.\n",
                RetiredFamily::ScenarioNumber,
                &declared()
            )),
            ["#59", "#71"],
            "and running text is read as it always was"
        );
    }

    /// Offsets are into the source rather than into the run, so a caller reading
    /// one comment out of a file still reports where in the file the reference
    /// stands. Without that a register would name every occurrence at the top of
    /// the source it was found in.
    ///
    /// ´claim:retired:offsets-are-into-the-source-rather-than-into-the-run´
    /// ´test:unit:reports-offsets-into-the-source-not-the-run´
    #[test]
    fn reports_offsets_into_the_source_not_the_run() {
        let found = scan_retired_text(
            1000,
            "see #59 there",
            RetiredFamily::ScenarioNumber,
            &declared(),
        );

        assert_eq!(found, [("#59".to_owned(), 1004)]);
    }
}
