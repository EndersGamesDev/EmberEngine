// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Residual litter: the three shapes an earlier sweep left standing.
//!
//! The reference-burn campaign retired several identity schemes, and each
//! retirement left a remainder that no recognizer selects. The three shapes here
//! are one family rather than three because they share a cause: each is what a
//! sweep, or a retired convention's own notation, left behind after the part a
//! recognizer could see had gone. A work-package number outlived the plan that
//! numbered it. A record locator outlived the prefix that made it findable. A
//! locator sign outlived the number that made it point somewhere.
//!
//! What they have in common is worth stating plainly, because it is the reason
//! the family exists: every one of them keeps its full referential force for a
//! human reader while being invisible to the gate built to see it. That is
//! strictly worse than an unmigrated reference, which at least fails loudly.
//!
//! # This recognizer is the register's own, and stops short of the lint
//!
//! Two of the three shapes are degraded spellings of forms the migration lint
//! already reads, and a recognizer that joined the lint would therefore widen
//! what the shared lint reads rather than declare a private scheme beside a
//! register. Widening the shared lint is reserved to the record that retired
//! those forms, and that record has not been amended. So the family counts here
//! and judges nowhere: the register may carry occurrences while the retirement
//! is unwritten, exactly as the unprefixed-record family did before it, and the
//! rules join the lint's policy only after the sweep has emptied the register
//! and the retirement has been recorded.
//!
//! The consequence for this module is a boundary rather than a compromise. The
//! shapes are read with the lint's own locator reader wherever the two touch —
//! the word-shaped mark is defined as the complement of the section rule over
//! the same mark, computed by calling that rule rather than by restating it —
//! so the two readings cannot drift apart into counting different things.
//!
//! # Why every bound is an enumeration
//!
//! Each shape is bounded, and none of the bounds is fastidiousness. An
//! unbounded work-package rule would count every capital-letter pair followed by
//! a number in the corpus. An unbounded locator rule would count every
//! hyphenated letter-and-number, so a corpus that still numbers its own
//! divisions would find its numbering counted as another's debt. A register that
//! counted either could never reach zero, because reaching zero would mean
//! rewriting prose that was never about the retired documents. So the work
//! packages are the eight the retired plan actually numbered, and the locators
//! are the numbers the two retired documents actually carried: the old
//! specification's chapters and the lettered record series.
//!
//! The mark is the exception that proves the rule, and it is bounded a different
//! way: numbering carries no identity anywhere in this corpus, so a mark
//! surviving in the counted surfaces is debt whatever follows it. The bound
//! there is the surface rather than the value.
//!
//! # What the mark rule reaches, and what the section rule takes back
//!
//! The rule as ruled is the mark, optionally one space, then a run the locator
//! reader rejects **whose first character is an ASCII letter**. That clause was
//! written from the three spellings the corpus then held — a document token, an
//! ordinal placeholder, and a named division introduced by a space —
//! and it was ruled together with a claim that the two readings of the mark
//! exhaust it, leaving no occurrence in neither. For a time the corpus falsified
//! the second half of that pairing, and two spellings the retired convention
//! itself defined stood in neither reading: a named division written with its
//! heading in quotation marks rather than after a space, and the first mark of
//! the doubled pair that introduces a list or a range.
//!
//! Neither was a gap in this rule. The owner ruled both a defect of the
//! tokenization layer rather than of the policy, and the mark's reader now
//! parses every occurrence to exactly one of four readings
//! (ADR-T-020, The migration disciplines). A quoted heading is the same
//! named-division reference the spaced spelling makes, so it is the section
//! family's, by the degraded-spelling reading rather than by a new family. A
//! doubled mark opens one reference whose locator its companion carries, which
//! is the decision the old line-at-a-time reading could not make and the run
//! reading makes without being asked: the pair is one reference, so there is one
//! count and one replacement owed.
//!
//! This family's clause is therefore untouched and its reach is unchanged. What
//! moved is where the two recovered spellings land — with the section register,
//! which is where the form they degrade from is counted.
//!
//! # One reference wrapped across two lines is one reference
//!
//! A retired convention wrote ranges and lists, and wrapped them across comment
//! lines. A mark and a document token can therefore stand at the end of one
//! comment line with the locator on the next. A recognizer reading each line
//! alone would report a word-shaped mark on the first and a bare locator on the
//! second — counting one reference twice, and offering two replacements where
//! one is owed. So comment leaders are resolved away and a run of adjacent
//! comment lines is read as one region, which is what the surface reading
//! already requires of anything reading commentary
//! (ADR-T-020, The migration disciplines). Prose is joined across a
//! soft line break for the same reason and by the same rule.
//!
//! That joining used to live here, and it does not any more. It was this
//! module's answer to a question every recognizer in the crate asks, and the two
//! rules that answered it differently were not narrower rules but blind ones. It
//! is declared once in the tokenization layer now, and this module consumes it
//! like every other reader: what changed is the layer, not this family's shapes.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_the_work_package_numbers_the_retired_plan_carried`] | residual | A work-package number is the prefix opening a token and one of the eight dotted numbers the retired plan actually issued. The enumeration is the bound: a number the plan never carried belongs to some other scheme, and a rule reading every capital pair before a number could never reach zero. |
//! | [`reads_a_bare_locator_within_the_two_enumerations`] | residual | A bare locator is the letter and hyphen opening a token, then a dot-joined number whose head is one the two retired documents issued — a chapter of the old specification or a number of the lettered record series. A head outside both enumerations is another corpus's numbering rather than this one's debt. |
//! | [`leaves_a_locator_the_section_rule_has_already_read_alone`] | residual | A locator standing inside a reference the section rule already read is not a second occurrence. Where a mark, or a mark and a document token, precedes the locator, the whole is one reference belonging to the section family, and counting the locator again would register one debt twice. |
//! | [`leaves_a_lettered_record_reference_alone`] | residual | A lettered record reference is not a bare locator: its own prefix stands before it, so the locator does not open a token there. The record family reads that form, and this one reads only the spelling with the prefix gone. |
//! | [`reads_a_word_shaped_mark_as_the_complement_of_the_section_rule`] | residual | A word-shaped mark is exactly what the section rule refuses over the same mark: the rule is called rather than restated, so a mark carrying a number or a Roman run stays in the section family while a mark carrying a document token, an ordinal placeholder or a named division falls here. The two readings cannot drift into counting different things. |
//! | [`joins_a_reference_wrapped_across_two_comment_lines`] | residual | A mark and document token ending one comment line with the locator opening the next is one reference, not two. Reading the adjacent comment lines as one region with their leaders resolved away hands the whole to the section rule, so neither half is registered as litter. |
//! | [`leaves_displayed_residual_forms_alone`] | residual | A form shown in code font or inside a fenced block is exhibited rather than referred to, in this family as in every other, which is what lets the register's own preamble and the campaign's own report name the shapes they count. |
//! | [`reads_a_shell_script_for_its_comments_alone`] | residual | A shell script is read for its comments alone, as a Rust source is: the mark opens a comment only where a word does not continue and only outside quotes, so a form standing in a quoted string is data the script carries rather than a reference it makes. |
//! | [`leaves_the_two_recovered_spellings_to_the_section_family`] | residual | The two spellings that once stood in neither reading are the section family's, and this register counts neither of them. A quoted heading is a named division and a doubled mark opens one reference; both are read where the form they degrade from is read, so this family's clause holds exactly where it was ruled and the mark is exhausted between the two readings rather than by widening one. |
//! | [`keeps_the_three_prefix_number_domains_disjoint`] | residual | The three prefix-number instances this module still carries the payloads of read disjoint domains, so splitting one combined locator reading into two independent instances counts each occurrence exactly once. The work packages differ by prefix; the chapters and the records share one prefix and are kept apart by their bounds alone, which is the relation the resolver will validate over declared documents and which holds here of the compiled values. |
//! | [`reports_residual_offsets_into_the_source_not_the_run`] | residual | Offsets are into the source rather than into the joined region, so a caller reading one comment out of a file still reports where in the file the reference stands. |

use crate::burn::Recognizer;
use crate::legacy::{Mark, SECTION_MARK, read_mark};
use crate::token::{Region, markdown_regions, rust_regions, script_regions};

/// One occurrence: the form as written, and where it stands in the source.
pub type Residual = (String, usize);

/// Read one run of text, already joined and with its leaders resolved away.
///
/// The three shapes are read in one pass over the same run because two of them
/// interact: a locator standing inside a reference the section rule has already
/// read is that reference's, not this family's, so the spans the section rule
/// covers must be known before the locators are gathered.
#[must_use]
pub fn scan_residual_text(base: usize, text: &str, declared: &Recognizer) -> Vec<Residual> {
    let mut found = Vec::new();

    for scheme in declared.prefix_numbers() {
        found.extend(scheme.scan_text(base, text));
    }

    word_shaped_marks(base, text, &mut found);

    found.sort_by_key(|(_text, offset)| *offset);
    found
}

/// Gather every word-shaped mark standing in one run of text.
///
/// The shape is the exact complement of the section rule over the same mark: a
/// mark the rule reads is that rule's, and a mark it refuses is this family's
/// where a letter follows. The letter is what the corpus's occurrences actually
/// carry — a document token, an ordinal placeholder, or a named division — and a
/// mark followed by anything else is left for the census to report rather than
/// claimed by a rule nobody wrote.
fn word_shaped_marks(base: usize, text: &str, found: &mut Vec<Residual>) {
    for (offset, _mark) in text.match_indices(SECTION_MARK) {
        if let Mark::WordShaped { text, .. } = read_mark(text, offset) {
            found.push((text, base + offset));
        }
    }
}

/// Read every region of one surface for the three shapes.
///
/// The regions come from the tokenization layer, which is the whole of what
/// this module knows about where referring text is and how much of it is one
/// run. Offsets are mapped back through the region, so a caller reading one
/// comment out of a file still learns where in the file the reference stands.
fn scan_regions(regions: &[Region], declared: &Recognizer) -> Vec<Residual> {
    let mut found: Vec<Residual> = regions
        .iter()
        .flat_map(|region| {
            scan_residual_text(0, region.text(), declared)
                .into_iter()
                .map(|(text, offset)| (text, region.source_offset(offset)))
        })
        .collect();

    found.sort_by_key(|(_text, offset)| *offset);
    found
}

/// Read one Markdown document for the three shapes.
///
/// Only running text is read, and it is joined across soft line breaks so that a
/// reference wrapped mid-sentence is one reference. A form shown in code font or
/// inside a fenced block is exhibited rather than referred to, which is the same
/// boundary every other family keeps.
#[must_use]
pub fn scan_residual_markdown(source: &str, declared: &Recognizer) -> Vec<Residual> {
    scan_regions(&markdown_regions(source), declared)
}

/// Read one Rust source's comments for the three shapes.
///
/// Adjacent comment lines are read as one region with their leaders resolved
/// away, for the reason the module head gives. Only the comments are read: a
/// form in a string literal is data the program carries rather than a reference
/// the corpus makes, and one in an identifier is a name only renaming could
/// retire.
#[must_use]
pub fn scan_residual_comments(source: &str, declared: &Recognizer) -> Vec<Residual> {
    scan_regions(&rust_regions(source), declared)
}

/// Read one shell script's comments for the three shapes.
///
/// A script is read exactly as a Rust source is, for the same reason: a
/// provenance comment is a reference the corpus makes, and a form inside a
/// quoted word is data the script carries.
#[must_use]
pub fn scan_residual_script(source: &str, declared: &Recognizer) -> Vec<Residual> {
    scan_regions(&script_regions(source), declared)
}

#[cfg(test)]
mod tests {
    use super::{
        scan_residual_comments, scan_residual_markdown, scan_residual_script, scan_residual_text,
    };
    use crate::burn::Recognizer;
    use crate::program::{PrefixBound, PrefixNumbers};

    /// The work-package scheme, as the prefix-number document declares it.
    fn work_packages() -> PrefixNumbers {
        PrefixNumbers::new(
            "WP-",
            PrefixBound::Exact(
                ["2", "8", "4.0", "4.1", "4.2", "4.3", "4.4", "4.5"]
                    .iter()
                    .map(|number| (*number).to_owned())
                    .collect(),
            ),
            true,
        )
    }

    /// The chapter scheme, bounded by the range the old specification numbered.
    fn chapters() -> PrefixNumbers {
        PrefixNumbers::new(
            "L-",
            PrefixBound::LeadingRange {
                minimum: 1,
                maximum: 30,
            },
            true,
        )
    }

    /// The record scheme, bounded by the bands the lettered series carried.
    fn records() -> PrefixNumbers {
        PrefixNumbers::new(
            "L-",
            PrefixBound::LeadingSet(vec![
                110, 120, 130, 140, 150, 160, 170, 210, 220, 230, 240, 250, 260, 270, 310, 320,
                330, 340, 350, 360, 370, 410, 420, 430, 440, 450, 460, 510, 520, 530, 540, 550,
                560, 570, 580,
            ]),
            true,
        )
    }

    /// The payload the prefix-number document declares, as the census reads it.
    ///
    /// Written here rather than read from the corpus for the reason the retired
    /// family's fixture gives: these tests hold the reading to its rules, and that
    /// the document declares exactly this payload is a separate claim held where
    /// the document is loaded.
    fn declared() -> Recognizer {
        Recognizer::new([], None, [work_packages(), chapters(), records()])
    }

    fn texts(found: &[(String, usize)]) -> Vec<String> {
        found.iter().map(|(text, _offset)| text.clone()).collect()
    }

    fn reads(text: &str) -> Vec<String> {
        texts(&scan_residual_text(0, text, &declared()))
    }

    /// A work-package number is the prefix opening a token and one of the eight
    /// dotted numbers the retired plan actually issued. The enumeration is the
    /// bound: a number the plan never carried belongs to some other scheme, and
    /// a rule reading every capital pair before a number could never reach
    /// zero.
    ///
    /// ´claim:residual:a-work-package-number-is-one-of-the-eight-the-plan-issued´
    /// ´test:unit:reads-the-work-package-numbers-the-retired-plan-carried´
    #[test]
    fn reads_the_work_package_numbers_the_retired_plan_carried() {
        assert_eq!(reads("Built under WP-4.1 and WP-8.\n"), ["WP-4.1", "WP-8"]);
        assert_eq!(reads("Under WP-2, before the plan retired.\n"), ["WP-2"]);

        let quiet = [
            "A plan numbered WP-9 elsewhere.",
            "Nor WP-4.6, one past the last.",
            "Nor WP-40 either.",
            "An identifier a-WP-2 continues a word.",
            "And SWP-2 is one word.",
            "A trailing letter makes WP-2x another token.",
        ];

        for source in quiet {
            assert_eq!(reads(source), Vec::<String>::new(), "on: {source}");
        }
    }

    /// A bare locator is the letter and hyphen opening a token, then a
    /// dot-joined number whose head is one the two retired documents issued — a
    /// chapter of the old specification or a number of the lettered record
    /// series. A head outside both enumerations is another corpus's numbering
    /// rather than this one's debt.
    ///
    /// ´claim:residual:a-bare-locator-carries-a-number-one-of-the-two-documents-issued´
    /// ´test:unit:reads-a-bare-locator-within-the-two-enumerations´
    #[test]
    fn reads_a_bare_locator_within_the_two_enumerations() {
        assert_eq!(reads("The L-16.4.2 formula binds.\n"), ["L-16.4.2"]);
        assert_eq!(reads("As L-160 requires, and L-6 too.\n"), ["L-160", "L-6"]);
        assert_eq!(reads("Standing last in the sentence, L-12.\n"), ["L-12"]);

        let quiet = [
            "A chapter L-31 the specification never had.",
            "A record L-190 no series carried.",
            "A locator L-1000 of some other corpus.",
            "An identifier L-6x is one word.",
            "And CELL-6 continues a word.",
            "A snake_case L_6 is not this form.",
        ];

        for source in quiet {
            assert_eq!(reads(source), Vec::<String>::new(), "on: {source}");
        }
    }

    /// A locator standing inside a reference the section rule already read is
    /// not a second occurrence. Where a mark, or a mark and a document token,
    /// precedes the locator, the whole is one reference belonging to the
    /// section family, and counting the locator again would register one debt
    /// twice.
    ///
    /// ´claim:residual:a-locator-the-section-rule-already-read-is-not-counted-again´
    /// ´test:unit:leaves-a-locator-the-section-rule-has-already-read-alone´
    #[test]
    fn leaves_a_locator_the_section_rule_has_already_read_alone() {
        assert_eq!(
            reads("As \u{a7}L-16.4.2 requires.\n"),
            Vec::<String>::new(),
            "a mark before the locator makes the whole one section reference"
        );
        assert_eq!(
            reads("As \u{a7}SPEC L-16.4.2 requires.\n"),
            Vec::<String>::new(),
            "a mark and a document token do the same across the space"
        );
        assert_eq!(
            reads("As \u{a7}SPEC L-16.4.2 requires, and L-9 besides.\n"),
            ["L-9"],
            "and the sibling standing outside that reference is still counted"
        );
    }

    /// A lettered record reference is not a bare locator: its own prefix stands
    /// before it, so the locator does not open a token there. The record family
    /// reads that form, and this one reads only the spelling with the prefix
    /// gone.
    ///
    /// ´claim:residual:a-lettered-record-reference-is-the-record-familys-and-not-this-one´
    /// ´test:unit:leaves-a-lettered-record-reference-alone´
    #[test]
    fn leaves_a_lettered_record_reference_alone() {
        assert_eq!(reads("As ADR-L-160 requires.\n"), Vec::<String>::new());
        assert_eq!(
            reads("As ADR-L-160 requires, and L-160 alone besides.\n"),
            ["L-160"],
            "the spelling with the prefix gone is this family's"
        );
    }

    /// A word-shaped mark is exactly what the section rule refuses over the
    /// same mark: the rule is called rather than restated, so a mark carrying a
    /// number or a Roman run stays in the section family while a mark carrying
    /// a document token, an ordinal placeholder or a named division falls here.
    /// The two readings cannot drift into counting different things.
    ///
    /// ´claim:residual:a-word-shaped-mark-is-the-complement-of-the-section-rule´
    /// ´test:unit:reads-a-word-shaped-mark-as-the-complement-of-the-section-rule´
    #[test]
    fn reads_a_word_shaped_mark_as_the_complement_of_the_section_rule() {
        assert_eq!(reads("Named in the \u{a7}SPEC\n"), ["\u{a7}SPEC"]);
        assert_eq!(
            reads("The meta-notation \u{a7}N stands for a number.\n"),
            ["\u{a7}N"],
            "an ordinal placeholder locates nothing"
        );
        assert_eq!(
            reads("Under \u{a7} Purpose and Principles, the rule holds.\n"),
            ["\u{a7} Purpose"],
            "a named division carries the mark and one space"
        );

        let section = [
            "As \u{a7}10.3 requires.",
            "As \u{a7} 10.3 requires.",
            "As \u{a7}VII requires.",
            "As \u{a7}SPEC L-16.4.2 requires.",
        ];

        for source in section {
            assert_eq!(
                reads(source),
                Vec::<String>::new(),
                "the section rule reads this one: {source}"
            );
        }
    }

    /// A mark and document token ending one comment line with the locator
    /// opening the next is one reference, not two. Reading the adjacent comment
    /// lines as one region with their leaders resolved away hands the whole to
    /// the section rule, so neither half is registered as litter.
    ///
    /// ´claim:residual:a-reference-wrapped-across-two-comment-lines-is-one-reference´
    /// ´test:unit:joins-a-reference-wrapped-across-two-comment-lines´
    #[test]
    fn joins_a_reference_wrapped_across_two_comment_lines() {
        let wrapped = "/// Interior crossovers are not asserted: the \u{a7}SPEC\n\
                       /// L-16.4.2 formula binds at the outermost pair.\n";

        assert_eq!(
            texts(&scan_residual_comments(wrapped, &declared())),
            Vec::<String>::new(),
            "the joined reading hands the whole to the section rule"
        );

        let apart = "/// Interior crossovers are not asserted: the \u{a7}SPEC\n\
                     let binding = 1;\n\
                     /// L-16.4.2 formula binds at the outermost pair.\n";

        assert_eq!(
            texts(&scan_residual_comments(apart, &declared())),
            ["\u{a7}SPEC", "L-16.4.2"],
            "program text between two comments ends the run, so each half stands alone"
        );
    }

    /// A form shown in code font or inside a fenced block is exhibited rather
    /// than referred to, in this family as in every other, which is what lets
    /// the register's own preamble and the campaign's own report name the
    /// shapes they count.
    ///
    /// ´claim:residual:a-residual-form-shown-rather-than-written-is-left-alone´
    /// ´test:unit:leaves-displayed-residual-forms-alone´
    #[test]
    fn leaves_displayed_residual_forms_alone() {
        let displayed =
            "A shape such as `WP-8` is named here.\n\n```text\nL-160 and \u{a7}SPEC\n```\n";

        assert_eq!(
            texts(&scan_residual_markdown(displayed, &declared())),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan_residual_markdown(
                "Built under WP-8, per L-160.\n",
                &declared()
            )),
            ["WP-8", "L-160"],
            "and running text is read as it always was"
        );
    }

    /// A shell script is read for its comments alone, as a Rust source is: the
    /// mark opens a comment only where a word does not continue and only
    /// outside quotes, so a form standing in a quoted string is data the script
    /// carries rather than a reference it makes.
    ///
    /// ´claim:residual:a-script-is-read-for-its-comments-alone´
    /// ´test:unit:reads-a-shell-script-for-its-comments-alone´
    #[test]
    fn reads_a_shell_script_for_its_comments_alone() {
        let script =
            "#!/bin/sh\n# Feed-forward lint for the crate (WP-8).\necho \"nothing of WP-2 here\"\n";

        assert_eq!(texts(&scan_residual_script(script, &declared())), ["WP-8"]);

        let quoted = "echo 'a # mark inside quotes opens nothing, WP-2'\necho \"nor WP-4.1\"\n";

        assert_eq!(
            texts(&scan_residual_script(quoted, &declared())),
            Vec::<String>::new()
        );
    }

    /// The two spellings that once stood in neither reading are the section
    /// family's, and this register counts neither of them. A quoted heading is
    /// a named division and a doubled mark opens one reference; both are read
    /// where the form they degrade from is read, so this family's clause holds
    /// exactly where it was ruled and the mark is exhausted between the two
    /// readings rather than by widening one.
    ///
    /// ´claim:residual:the-two-recovered-spellings-belong-to-the-section-family´
    /// ´test:unit:leaves-the-two-recovered-spellings-to-the-section-family´
    #[test]
    fn leaves_the_two_recovered_spellings_to_the_section_family() {
        assert_eq!(
            reads("Per Spike 2 \u{a7}\"Extraction Path\" the mapping holds.\n"),
            Vec::<String>::new(),
            "a quoted heading is a named division and belongs to the section register"
        );
        assert_eq!(
            reads("Classification tags (\u{a7}\u{a7}SPEC L-16.6, L-16.8).\n"),
            ["L-16.8".to_owned()],
            "the doubled mark opens one section reference covering its locator, \
             and only the list sibling standing outside it is litter"
        );
        assert_eq!(
            reads("The \u{a7}\u{a7}SPEC pair carrying no locator at all.\n"),
            ["\u{a7}\u{a7}SPEC".to_owned()],
            "a doubled mark the section rule still refuses is one word-shaped mark, not two"
        );
    }

    /// The three prefix-number instances this module still carries the payloads
    /// of read disjoint domains, so splitting one combined locator reading into
    /// two independent instances counts each occurrence exactly once. The work
    /// packages differ by prefix; the chapters and the records share one prefix
    /// and are kept apart by their bounds alone, which is the relation the
    /// resolver will validate over declared documents and which holds here of
    /// the compiled values.
    ///
    /// ´claim:residual:the-three-prefix-number-instances-read-disjoint-domains´
    /// ´test:unit:keeps-the-three-prefix-number-domains-disjoint´
    #[test]
    fn keeps_the_three_prefix_number_domains_disjoint() {
        let work = work_packages();
        let chapter_numbers = chapters();
        let record_numbers = records();

        assert!(
            !chapter_numbers.overlaps(&record_numbers),
            "one prefix, and no leading value in both bounds"
        );
        assert!(
            !work.overlaps(&chapter_numbers),
            "and a different prefix reads a disjoint span"
        );
        assert!(!work.overlaps(&record_numbers));
    }

    /// Offsets are into the source rather than into the joined region, so a
    /// caller reading one comment out of a file still reports where in the file
    /// the reference stands.
    ///
    /// ´claim:residual:offsets-are-into-the-source-rather-than-into-the-joined-run´
    /// ´test:unit:reports-residual-offsets-into-the-source-not-the-run´
    #[test]
    fn reports_residual_offsets_into_the_source_not_the_run() {
        assert_eq!(
            scan_residual_text(1000, "see WP-8 there", &declared()),
            [("WP-8".to_owned(), 1004)]
        );

        let source = "// a first line\n// and then WP-8 here\n";
        let found = scan_residual_comments(source, &declared());

        assert_eq!(found.len(), 1);
        assert_eq!(
            &source[found[0].1..found[0].1 + 4],
            "WP-8",
            "the offset lands on the form"
        );
    }
}
