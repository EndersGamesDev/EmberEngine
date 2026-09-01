// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The migration lint: reference forms the calculus supersedes.
//!
//! The calculus supersedes a two-level tag system in favour of labels minted at
//! heads, and retires section-number and record-by-number identities with it
//! (ADR-T-020, The migration disciplines). No retirement
//! is worth anything while the forms can still be written, so this module reads
//! a document for the four shapes the campaign leaves behind: a section-number
//! reference, a tag of the superseded system, a decision record named by its
//! number, and a record number carrying no series at all.
//!
//! The fourth shape is the one that names nothing decidable. A record number
//! written with no series letter is ambiguous between a corpus that keeps its
//! numbers and the local numbering that retired, and inferring which was meant
//! is the one thing a recognizer must not do — which is why it is a rule of its
//! own rather than a widening of the third: that rule reads the series letter,
//! and a version of it that read a number carrying none would flag the
//! canonical form the corpus keeps. For as long as no record had retired the
//! shape, this module read it so that a register could count it while the
//! policy stayed at three, because a retirement is a recorded choice rather
//! than a recognizer's opinion about prose. The record now states it, so the
//! rule is in the policy and the register stands empty behind it.
//!
//! What makes the lint usable mid-migration is that it is held per document. A
//! corpus part-way through a campaign carries both worlds at once, and a lint
//! that reported every unmigrated document would drown the migrated ones in the
//! work not yet done. The adoption datum therefore names the documents the
//! migration has reached, and names them rule by rule: a document that has
//! migrated its identities but legitimately quotes another corpus's section
//! numbers is held to the three rules it can meet and exempted from the one it
//! cannot, with the reason recorded beside the datum rather than remembered.
//!
//! Where the forms may stand is as much of the rule as what they look like.
//! Running text is where a reference is made, so that is where the section and
//! record forms are read; a form standing in code font is a token shown, not a
//! reference made, which is exactly how the campaign's own backlog names the
//! forms it bans. A tag is the other way round: the superseded system's own
//! citation syntax put the tag in a single-backtick span, so that is where a
//! tag citation stands, while a displayed double-backtick span shows one
//! without citing it. Both readings follow the participation judgment
//! (ADR-T-014, A calculus of documentation and source labels) rather than inventing a second notion
//! of display.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reports_section_numbers_in_running_text`] | legacy | A section-number reference made in running text is reported as the superseded form it is, at any depth of numbering, and the reported text is normalised so that a space after the mark does not disguise it. |
//! | [`reads_a_foreign_corpus_locator_as_one_reference`] | legacy | A locator naming the corpus it points into as well as the section within it is read as a single reference rather than as a mark and some unrelated words, so the whole locator is what the author is shown. |
//! | [`leaves_section_marks_without_a_number_alone`] | legacy | The section mark by itself locates nothing, and neither does one followed by an ordinary word, so prose may discuss sections without every mention becoming a superseded reference. |
//! | [`reports_roman_numeral_section_locators`] | legacy | A locator numbered in roman numerals is a section reference too, and is reported beside the decimal ones. A qualifier trailing after it is no part of the locator, and a run of letters outside the roman numerals locates nothing at all. |
//! | [`leaves_displayed_and_code_font_forms_alone`] | legacy | A superseded form shown in code font, in a displayed span, or inside a fenced block is a token exhibited rather than a reference made, and is left alone. This is what lets the campaign's own documents name the forms they ban without failing their own lint. |
//! | [`reports_retired_record_series_only`] | legacy | Only the retired record series are reported by number. This repository's own records keep their numbered identity, so a reference to one is not swept up by a campaign that was never about them. |
//! | [`reads_a_record_reference_standing_last_in_its_sentence`] | legacy | A record reference is read wherever it legitimately ends — closing a sentence, or standing inside parentheses — so punctuation after it does not hide it. A series named with no number after it names nothing. |
//! | [`reports_unprefixed_record_numbers_within_their_bound`] | legacy | A record named by a bare number is reported by the rule that reads no series, and by that rule alone. The bound is what the family rests on: the prefix must open a token, so one inside a longer word is part of that word; the number is exactly three digits, so a longer one is a different token rather than one of these with a tail, and a shorter one is not a record number at all; and a series letter is not a digit, so every lettered form — the canonical series included — is outside this family by construction rather than by exempting the files it stands in. |
//! | [`keeps_the_two_record_families_apart`] | legacy | The two record rules are two families, and neither reads the other's shape. A document held to one is held to it alone, and a document held to the whole policy is read by each rule for its own shape and by neither for the other's — which is what lets two registers count two shapes without either counting twice. |
//! | [`leaves_displayed_unprefixed_record_numbers_alone`] | legacy | A bare number shown rather than written is left alone by the same display boundary the other rules keep, so this register's own preamble and the campaign's own report may name the shape they count. |
//! | [`reports_tag_forms_in_participating_spans`] | legacy | A tag of the superseded system is reported where that system cited them: in a participating span. Both the plain tag and the one carrying a step number are read, because both were citations under the old rules. |
//! | [`leaves_labels_and_ordinary_code_font_alone`] | legacy | The tag rule leaves everything else in code font alone: a well-formed label, a command line, a file name, a language path, a time of day, a slug hyphenated wrongly, and a step with no number are none of them tags. The rule is narrow enough to live beside ordinary technical prose. |
//! | [`holds_a_document_to_the_rules_it_is_given`] | legacy | The lint is applied per document and rule by rule: given no rules a document is held to nothing, and given some it is held to exactly those. A document that has migrated its identities but legitimately quotes another corpus's section numbers can therefore meet the rules it can meet while a campaign is still under way. |
//! | [`reads_a_reference_wrapped_across_a_comment_boundary`] | legacy | A reference the retired convention wrapped across a comment line boundary is one reference to the migration lint, at two lines or three, because the rules are handed a joined region rather than a line. Reading each line alone saw two halves of nothing, which was a defect of the reading and not a narrowness of the rules. |
//! | [`reads_a_reference_wrapped_across_a_soft_line_break`] | legacy | Prose wraps too, and a reference broken over a soft line break in Markdown is one reference for the same reason and by the same layer. |
//! | [`leaves_a_form_in_a_string_literal_invisible`] | legacy | Widening what a rule is handed does not widen where a reference may stand. A form in a string literal, in code font or inside a fenced block is still a token shown rather than a reference made, and the joined reading never reaches one. |
//! | [`reads_a_code_span_wrapped_across_a_line_as_one_span`] | legacy | An inline code span broken across a line is one span, not two, so a tag standing whole inside a wrapped paragraph is read where it stands. A tag broken mid-token does not survive the break: the span's line becomes a space and the superseded syntax wrote a tag as one token, so what is left is no tag rather than a tag reported twice. |
//! | [`reads_the_two_degraded_spellings_of_the_mark`] | legacy | The mark's two remaining spellings are the section family's. A heading quoted after the mark is the same named-division reference the spaced spelling makes, and a doubled mark opens one reference rather than two — the pair's first mark is that reference's own opening, so the second is no second count. Both hold across a comment line boundary. |
//! | [`parses_every_mark_to_exactly_one_reading`] | legacy | Every occurrence of the mark parses to exactly one of four readings — a companion of the reference before it, a section reference, a word-shaped mark, or nothing — so the mark is exhausted by construction. The table is the spellings the corpus and the retired convention between them actually write. |

use std::path::Path;

use crate::finding::{Finding, Location};
use crate::token::{Region, markdown_code_spans, markdown_regions};

/// The record series whose by-number references the campaign retires.
///
/// Names move to the calculus, and a record named by a series letter and a
/// number does not survive a rewrite into it
/// (´[ORCHESTRATION-dec:migration:superseded-forms]´); a family's reach is its
/// register's to name, and it names exactly the series some corpus retired —
/// the four the Assayer's campaign retired, with this repository's own records
/// deliberately absent, because they keep their numbers and a census counting
/// them would count a naming scheme nobody has retired
/// (´[ORCHESTRATION-conv:migration:burn-family]´). So the four letters here are a
/// reach rather than a taste, and a fifth would need a fifth retirement.
///
/// ´const:indexlinter:superseded-record-lettering´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:superseded-record-lettering-form-xa7ed5995´
const RETIRED_RECORD_SERIES: &[char] = &['L', 'M', 'R', 'S'];

/// How many digits a record number written without its series letter carries.
///
/// The width is the bound that makes the unprefixed family countable at all. A
/// family is bounded so that zero is reachable, and a range bound follows the
/// mark's own shape: a longer number is never read as a shorter one carrying a
/// tail (´[ORCHESTRATION-conv:migration:burn-family]´). Three is the width every record
/// number in the campaign's corpus was written at, so a run of four digits is a
/// different token rather than one of these with something after it, and the
/// recognizer below reads the digit run entire and then measures it rather than
/// taking the first three of however many stand there.
///
/// ´const:indexlinter:unprefixed-record-number-width´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:indexlinter:unprefixed-record-number-width-count-3´
const UNPREFIXED_RECORD_WIDTH: usize = 3;

/// The mark introducing a section-number reference.
///
/// The register counting this family is named by the sign itself: the family is
/// the locator sign introducing a division's number, and the register is
/// generated by this recognizer rather than by one of its own
/// (´[ORCHESTRATION-conv:migration:burn-one-recognizer]´), under the
/// decision that retires section-number identities and leaves numbering carrying
/// no identity anywhere (´[ORCHESTRATION-dec:migration:superseded-forms]´). The mark is what
/// makes a number a pointer, which is why the recognizer reads it rather than the
/// document it points into — a locator naming another corpus's document is
/// counted like any other, and what to do with such a row is a judgment about the
/// row.
///
/// ´const:indexlinter:division-locator-sign´ (´[ORCHESTRATION-alg:const:codepoint]´)
/// ´const:indexlinter:division-locator-sign-codepoint-ua7´
pub const SECTION_MARK: char = '§';

/// One rule of the migration lint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LegacyRule {
    /// A reference to a division by its number.
    SectionNumber,
    /// A citation written as a tag of the superseded two-level system.
    TagForm,
    /// A decision record named by its series and number.
    RecordNumber,
    /// A decision record named by a bare number, with no series letter at all.
    ///
    /// This rule is the fourth retirement, and it is in [`LegacyRule::ALL`]
    /// because a record put it there. It reads the one shape that names no
    /// series, which is exactly the shape that cannot be resolved by reading
    /// it: the number is ambiguous between a corpus that keeps its numbers and
    /// the local numbering that retired. A register enumerated the ambiguity
    /// rather than arguing about it while the retirement was unwritten; the
    /// register reached zero, the record was written, and the rule joined the
    /// policy in that order.
    UnprefixedRecordNumber,
}

impl LegacyRule {
    /// Every rule of the lint: the policy a fully migrated document meets.
    ///
    /// The four rules are four retirements and no more: the two-level tag
    /// system, superseded by labels minted at heads; the informal record
    /// naming; the section-number identities, numbering carrying no identity
    /// anywhere afterwards; and the record number carrying no series at all
    /// (´[ORCHESTRATION-dec:migration:superseded-forms]´). A fifth rule would be a
    /// fifth retirement, which is why the value grows only by decision — and
    /// why a document is held rule by rule rather than all at once: the datum
    /// naming what the migration has reached exempts a document from the rule
    /// it cannot meet without exempting it from the three it can.
    ///
    /// The fourth rule stood in the enum and outside this value for as long as
    /// the retirement was unrecorded, which was the decision rule working
    /// rather than an omission. Recording it was one edit to one record, and
    /// the order the discipline demands is the order it happened in: the
    /// register [`LegacyRule::UnprefixedRecordNumber`] fed
    /// (´[ORCHESTRATION-conv:migration:burn-family]´) reached zero, the retirement was
    /// written down, and only then did the rule join the policy. Adding it
    /// here first would have held the corpus to a rule no record stated;
    /// adding it here while occurrences stood would have failed the documents
    /// carrying them for a rule that arrived after they were written.
    ///
    /// ´const:indexlinter:migration-lint-policy´ (´[ORCHESTRATION-alg:const:form]´)
    /// ´const:indexlinter:migration-lint-policy-form-x7cadc3ef´
    pub const ALL: &'static [Self] = &[
        Self::SectionNumber,
        Self::TagForm,
        Self::RecordNumber,
        Self::UnprefixedRecordNumber,
    ];
}

/// One legacy form as read from a run of text.
///
/// The rule that read it, the form as written, and where it opens in the run
/// handed over. The offset is into that run and not into any file, because a run
/// is the tokenization layer's fiction and only its owner knows where its pieces
/// really stand.
type Read = (LegacyRule, String, usize);

/// Read one run of plain text for the reference forms the campaign retires.
///
/// This is the recognizer proper, held apart from the readers above it so that
/// every surface can be read with it rather than beside it. The burn lists
/// census the same three shapes over Rust comments, where there is no Markdown
/// at all, and a census that recognized the forms its own way would eventually
/// disagree with the lint about what a reference is — the one defect a ratchet
/// cannot survive, because the gate and the register would then be counting
/// different things.
///
/// The tag rule is deliberately absent: a tag citation is a backtick span, which
/// is a fact about a document's markup rather than about text, so it is
/// recognized only by the reader that knows where the spans are.
fn read_forms(text: &str, rules: &[LegacyRule]) -> Vec<Read> {
    let mut found = Vec::new();

    if rules.contains(&LegacyRule::SectionNumber) {
        collect_section_references(text, &mut found);
    }

    if rules.contains(&LegacyRule::RecordNumber) {
        collect_record_references(text, &mut found);
    }

    if rules.contains(&LegacyRule::UnprefixedRecordNumber) {
        collect_unprefixed_record_references(text, &mut found);
    }

    found
}

/// Build the finding one read form is reported as.
fn finding_for(path: &Path, source: &str, (rule, text, offset): Read) -> Finding {
    let location = Location::new(path, source, offset);

    match rule {
        LegacyRule::SectionNumber => Finding::LegacySectionReference { text, location },
        LegacyRule::RecordNumber => Finding::LegacyRecordReference { text, location },
        LegacyRule::UnprefixedRecordNumber => {
            Finding::LegacyUnprefixedRecordReference { text, location }
        }
        // The tag rule reads spans rather than text, so nothing this function is
        // handed can carry it. The arm stands so that a rule added to the enum
        // and forgotten here is a compiler error rather than a silent
        // reclassification.
        LegacyRule::TagForm => Finding::LegacyTagReference { text, location },
    }
}

/// Read one run of plain text standing at a known offset in its source.
///
/// The text is a run standing at `base` in `source`, and the locations returned
/// are into `source`, so a caller reading a fragment still reports where the
/// fragment stands. A caller whose run was joined from several pieces has no
/// single base and reads regions instead.
#[must_use]
pub fn scan_text(
    path: &Path,
    source: &str,
    base: usize,
    text: &str,
    rules: &[LegacyRule],
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = read_forms(text, rules)
        .into_iter()
        .map(|(rule, form, offset)| finding_for(path, source, (rule, form, base + offset)))
        .collect();

    findings.sort_by_key(|finding| finding.primary_location().map_or(0, Location::offset));

    findings
}

/// Read regions of running text for the forms the campaign retires.
///
/// This is what every surface hands the rules. The regions come from the
/// tokenization layer, which decides where referring text is and how much of it
/// is one run; the rules decide only what a reference looks like. Offsets are
/// mapped back through the region that produced them, so a reference wrapped
/// across a comment line boundary is one reference reported where its first
/// character really stands.
#[must_use]
pub fn scan_regions(
    path: &Path,
    source: &str,
    regions: &[Region],
    rules: &[LegacyRule],
) -> Vec<Finding> {
    let mut findings: Vec<Finding> = regions
        .iter()
        .flat_map(|region| {
            read_forms(region.text(), rules)
                .into_iter()
                .map(|(rule, form, offset)| {
                    finding_for(path, source, (rule, form, region.source_offset(offset)))
                })
        })
        .collect();

    findings.sort_by_key(|finding| finding.primary_location().map_or(0, Location::offset));

    findings
}

/// Read one document for the reference forms the campaign retires.
///
/// The rules are the policy the adoption data hold this document to; a document
/// held to none is read for nothing, which is how an unmigrated document stays
/// out of the report.
///
/// Running text and code spans are two readings of one document because they are
/// two surfaces: the three reference rules are made in running text, and a tag
/// citation is a single-backtick span because that is where the superseded
/// system put one. Both come from the tokenization layer, which is also what
/// decides that a displayed span shows its interior rather than citing it.
#[must_use]
pub fn scan_legacy(path: &Path, source: &str, rules: &[LegacyRule]) -> Vec<Finding> {
    if rules.is_empty() {
        return Vec::new();
    }

    let mut findings = scan_regions(path, source, &markdown_regions(source), rules);

    if rules.contains(&LegacyRule::TagForm) {
        for span in markdown_code_spans(source) {
            if !span.displayed() && is_tag_form(span.interior()) {
                findings.push(Finding::LegacyTagReference {
                    text: span.interior().to_owned(),
                    location: Location::new(path, source, span.offset()),
                });
            }
        }
    }

    findings.sort_by_key(|finding| finding.primary_location().map_or(0, Location::offset));

    findings
}

/// Gather every section-number reference standing in one run of running text.
fn collect_section_references(raw: &str, found: &mut Vec<Read>) {
    for (offset, _mark) in raw.match_indices(SECTION_MARK) {
        if let Mark::Section { text, .. } = read_mark(raw, offset) {
            found.push((LegacyRule::SectionNumber, text, offset));
        }
    }
}

/// The quotation mark a named division's heading is written between.
///
/// The retired convention wrote a division either by its number or by its name,
/// and it wrote the name two ways: after one space, and between quotation marks.
/// The quoted spelling is the same reference as the spaced one, so the mark's
/// reader admits it, and the character it admits stands here rather than in the
/// reader because it is the form's own punctuation
/// (´[ORCHESTRATION-conv:migration:burn-family]´). The typewriter quote is the whole of
/// it: the corpus writes headings with that character, and admitting the
/// typographic pair would be widening the form on a guess rather than on an
/// occurrence.
///
/// ´const:indexlinter:division-title-quote´ (´[ORCHESTRATION-alg:const:codepoint]´)
/// ´const:indexlinter:division-title-quote-codepoint-u22´
const TITLE_QUOTE: char = '"';

/// What the section mark standing at an offset opens.
///
/// Four arms and no fifth, which is what it means for the mark to be exhausted:
/// every occurrence of the mark in a run of text is exactly one of these, and
/// the exhaustion is a property of this type rather than a claim about a corpus
/// that a later corpus could falsify. The two families that read the mark read
/// it through here — the section family takes [`Mark::Section`] and the residual
/// register takes [`Mark::WordShaped`] — so neither can drift into counting what
/// the other counts (ADR-T-020, The migration disciplines).
///
/// The companion arm is the decision the old line-at-a-time reading could not
/// make. The retired convention introduced a range or a list with a doubled
/// mark, and a rule that read each mark alone had to either report the pair
/// twice or report neither. Reading the run of marks as the opening of one
/// reference makes the answer fall out: the second mark is the same reference's,
/// so there is one reference and one replacement owed for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// A mark continuing the run of marks that opened a reference before it.
    Companion,
    /// A section reference: the form as written, and how far it runs.
    Section {
        /// The reference as written, mark run included.
        text: String,
        /// How many bytes the whole reference occupies from the mark onwards.
        extent: usize,
    },
    /// A mark carrying a word where a locator would stand: the residual family's.
    WordShaped {
        /// The form as written, mark run included.
        text: String,
        /// How many bytes the whole form occupies from the mark onwards.
        extent: usize,
    },
    /// A mark carrying nothing either reading reaches.
    Bare,
}

/// Read the mark standing at an offset in a run of text, exhaustively.
///
/// The grammar is one rule read in one order. The mark opens a run of marks, of
/// which the corpus writes one and the retired convention's ranges write two;
/// the whole run opens one reference. After it stands optionally one space and
/// then the body, and the body decides the family: a quoted heading and a
/// locator are both the section rule's, a word is the residual register's, and
/// anything else locates nothing.
///
/// The quoted heading is read before the locator because a quote is not a
/// locator character, so the two readings cannot both match; the order is stated
/// rather than relied upon.
#[must_use]
pub fn read_mark(raw: &str, offset: usize) -> Mark {
    if raw[..offset].ends_with(SECTION_MARK) {
        return Mark::Companion;
    }

    let run = raw[offset..]
        .chars()
        .take_while(|character| *character == SECTION_MARK)
        .count();
    let opening = run * SECTION_MARK.len_utf8();
    let marks = SECTION_MARK.to_string().repeat(run);
    let body = &raw[offset + opening..];

    if let Some((title, extent)) = read_quoted_title(body) {
        return Mark::Section {
            text: format!("{marks}{TITLE_QUOTE}{title}{TITLE_QUOTE}"),
            extent: opening + extent,
        };
    }

    if let Some((locator, extent)) = read_locator(body) {
        return Mark::Section {
            text: format!("{marks}{locator}"),
            extent: opening + extent,
        };
    }

    if let Some((word, spaced, extent)) = read_word(body) {
        let separator = if spaced { " " } else { "" };

        return Mark::WordShaped {
            text: format!("{marks}{separator}{word}"),
            extent: opening + extent,
        };
    }

    Mark::Bare
}

/// Read the quoted heading a section mark introduces, when it introduces one.
///
/// The heading is whatever stands between the two quotation marks, taken whole:
/// a division's name is prose and no token rule applies to it. An unclosed quote
/// is not a heading, and neither is an empty one — both name nothing, and a
/// reader that admitted them would report a mark and two characters as a
/// reference to a division that does not exist.
fn read_quoted_title(rest: &str) -> Option<(String, usize)> {
    let unspaced = rest.strip_prefix(' ').unwrap_or(rest);
    let space = rest.len() - unspaced.len();
    let inside = unspaced.strip_prefix(TITLE_QUOTE)?;
    let close = inside.find(TITLE_QUOTE)?;
    let title = &inside[..close];

    (!title.is_empty()).then(|| {
        (
            title.to_owned(),
            space + TITLE_QUOTE.len_utf8() + close + TITLE_QUOTE.len_utf8(),
        )
    })
}

/// Read the word a section mark introduces, when it introduces one.
///
/// This is the shape the residual register counts, and it is read here rather
/// than beside that register so that the mark has one reader. The word is a
/// locator token whose first character is an ASCII letter — a document token, an
/// ordinal placeholder, or a named division introduced by a space — which is
/// what the corpus's occurrences of the mark without a number actually carry.
fn read_word(rest: &str) -> Option<(String, bool, usize)> {
    let unspaced = rest.strip_prefix(' ').unwrap_or(rest);
    let space = rest.len() - unspaced.len();

    if !unspaced.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return None;
    }

    let token = locator_token(unspaced);

    Some((token.to_owned(), space == 1, space + token.len()))
}

/// Read the locator a section mark introduces, when it introduces one.
///
/// A locator is one token, or a document's short name and then one token, and it
/// is a locator at all only if a digit stands in it somewhere, or the token is a
/// bare run of Roman-numeral letters: the mark also introduces the word
/// "section" in ordinary prose, and a run carrying neither locates nothing. One
/// leading space is admitted because the corpus writes the mark both ways. A
/// trailing qualifier such as " #59" is not part of the locator either way — the
/// token boundary is the space before it, and the qualifier is simply left for
/// the surrounding text to carry.
pub fn read_locator(rest: &str) -> Option<(String, usize)> {
    let unspaced = rest.strip_prefix(' ').unwrap_or(rest);
    let space = rest.len() - unspaced.len();
    let first = locator_token(unspaced);

    if first.is_empty() {
        return None;
    }

    let trimmed_first = first.trim_end_matches(['.', '-']);

    let (locator, extent) =
        if first.bytes().any(|byte| byte.is_ascii_digit()) || is_roman_locator(trimmed_first) {
            (first.to_owned(), space + first.len())
        } else {
            let second = locator_token(unspaced[first.len()..].strip_prefix(' ')?);

            if second.is_empty() {
                return None;
            }

            (
                format!("{first} {second}"),
                space + first.len() + 1 + second.len(),
            )
        };

    let trimmed = locator.trim_end_matches(['.', '-']);

    if !trimmed.bytes().any(|byte| byte.is_ascii_digit()) && !is_roman_locator(trimmed) {
        return None;
    }

    Some((trimmed.to_owned(), extent))
}

/// The letters a Roman-numeral locator may be spelled with.
///
/// The policy's numeral clause is what puts them in the lint: a Roman-numeral
/// locator is a section reference like any other, read by the census and by the
/// lint alike, and replaced with a label like any other
/// (´[ORCHESTRATION-dec:migration:superseded-forms]´). The value is the numeral alphabet
/// entire, deliberately and not by omission — see the reader below, which
/// enforces no ordering and no repetition rule, because the clause records
/// the corpus's actual usage rather than an idealised subset of it and a stricter
/// reader would silently drop the forms that do not fit its grammar.
///
/// ´const:indexlinter:locator-numeral-alphabet´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:locator-numeral-alphabet-form-x2e7a533a´
const ROMAN_NUMERALS: &[char] = &['I', 'V', 'X', 'L', 'C', 'D', 'M'];

/// Whether a token is a bare run of Roman-numeral letters.
///
/// This is not Roman-numeral grammar: no ordering or repetition rule is
/// enforced, so a run of the seven admitted letters counts regardless of
/// whether it spells a numeral a scribe would recognize. The campaign's own
/// amendment records the corpus's actual usage of `§II`, `§IV`, `§XI` and kin
/// rather than an idealised subset of it, and a stricter reader would silently
/// drop the forms that do not fit its grammar.
fn is_roman_locator(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| ROMAN_NUMERALS.contains(&character))
}

/// The leading run of characters a locator token may be spelled with.
pub fn locator_token(text: &str) -> &str {
    let end = text
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || character == '.' || character == '-')
        })
        .unwrap_or(text.len());

    &text[..end]
}

/// Gather every record-by-number reference standing in one run of running text.
fn collect_record_references(raw: &str, found: &mut Vec<Read>) {
    for (offset, _prefix) in raw.match_indices("ADR-") {
        let rest = &raw[offset + "ADR-".len()..];
        let mut characters = rest.chars();

        let Some(series) = characters
            .next()
            .filter(|series| RETIRED_RECORD_SERIES.contains(series))
        else {
            continue;
        };

        if characters.next() != Some('-') {
            continue;
        }

        // Only the digit run belongs to the reference. Reading further would
        // swallow the sentence's full stop and then reject the reference for
        // carrying one, which silently exempts every reference written last in
        // its sentence — the commonest place a cross-reference stands.
        let digits = leading_digits(&rest[series.len_utf8() + 1..]);

        if digits.is_empty() {
            continue;
        }

        found.push((
            LegacyRule::RecordNumber,
            format!("ADR-{series}-{digits}"),
            offset,
        ));
    }
}

/// Gather every record named by a bare number in one run of running text.
///
/// The shape is the record prefix opening a token, then a digit run of exactly
/// the declared width. Both halves of that are the family's bound rather than
/// taste (ADR-T-020, The migration disciplines), and both are what keep the
/// canonical series out of the family by construction rather than by exempting
/// the files they stand in: a series letter is not a digit, so no lettered form
/// can be read as a bare one, and a prefix standing inside a longer word
/// belongs to that word.
///
/// The width is measured over the digit run entire rather than by reading three
/// digits and looking at what follows. The two are the same rule stated twice,
/// and stating it once is what makes a four-digit number a different token
/// instead of one of these with a tail — which is the failure a range bound
/// exists to prevent.
fn collect_unprefixed_record_references(raw: &str, found: &mut Vec<Read>) {
    for (offset, prefix) in raw.match_indices("ADR-") {
        if !opens_a_token(raw, offset) {
            continue;
        }

        let digits = leading_digits(&raw[offset + prefix.len()..]);

        if digits.len() != UNPREFIXED_RECORD_WIDTH {
            continue;
        }

        found.push((
            LegacyRule::UnprefixedRecordNumber,
            format!("ADR-{digits}"),
            offset,
        ));
    }
}

/// Whether the character before an offset lets a token begin there.
///
/// A token begins where a word does not continue, so a letter, a digit, an
/// underscore or a hyphen before the prefix means the prefix stands inside a
/// longer name rather than opening one. The hyphen is in that list because these
/// references are themselves hyphenated: a hyphen is a token's own punctuation
/// here, so one standing before the prefix joins it to whatever preceded it
/// rather than separating the two.
///
/// A run beginning at the very start of what was handed here opens a token. The
/// text is a fragment of a document rather than the document, and a fragment's
/// first character is preceded by a boundary of the reader's making — an inline
/// span ending, or a comment's leader — never by a word this reference could be
/// part of.
pub fn opens_a_token(raw: &str, offset: usize) -> bool {
    raw[..offset].chars().next_back().is_none_or(|character| {
        !(character.is_ascii_alphanumeric() || character == '_' || character == '-')
    })
}

/// The leading run of digits, which is all a record's number may be.
pub fn leading_digits(text: &str) -> &str {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());

    &text[..end]
}

/// Whether a span's interior is a tag of the superseded two-level system.
///
/// The superseded grammar is a namespace and a slug joined by one colon, each of
/// them lowercase kebab-case, optionally carrying a step ordinal. A segment of
/// digits alone is no tag — the system forbade ordinal slugs — and requiring a
/// letter in each segment is also what keeps ordinary code font, where a colon
/// separates a time or a numeric pair, out of the lint. A three-segment token is
/// not a tag either: it is a label, and the calculus reads it as one.
fn is_tag_form(interior: &str) -> bool {
    let Some((namespace, rest)) = interior.split_once(':') else {
        return false;
    };

    let slug = match rest.split_once('#') {
        Some((slug, ordinal))
            if !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            slug
        }
        Some(_step) => return false,
        None => rest,
    };

    is_segment(namespace) && is_segment(slug)
}

/// Whether one segment is a lowercase kebab-case word carrying a letter.
fn is_segment(text: &str) -> bool {
    if text.is_empty() || text.starts_with('-') || text.ends_with('-') {
        return false;
    }

    let admitted = text
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');

    admitted && !text.contains("--") && text.bytes().any(|byte| byte.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{LegacyRule, Mark, SECTION_MARK, read_mark, scan_legacy, scan_regions};
    use crate::finding::Finding;
    use crate::token::rust_regions;

    fn scan(source: &str) -> Vec<Finding> {
        scan_legacy(Path::new("doc.md"), source, LegacyRule::ALL)
    }

    fn texts(findings: &[Finding]) -> Vec<String> {
        findings
            .iter()
            .map(|finding| match finding {
                Finding::LegacySectionReference { text, .. }
                | Finding::LegacyTagReference { text, .. }
                | Finding::LegacyRecordReference { text, .. }
                | Finding::LegacyUnprefixedRecordReference { text, .. } => text.clone(),
                other => panic!("unexpected finding {other:?}"),
            })
            .collect()
    }

    /// A section-number reference made in running text is reported as the
    /// superseded form it is, at any depth of numbering, and the reported text
    /// is normalised so that a space after the mark does not disguise it.
    ///
    /// ´claim:legacy:a-section-number-reference-in-running-text-is-reported´
    /// ´test:unit:reports-section-numbers-in-running-text´
    #[test]
    fn reports_section_numbers_in_running_text() {
        assert_eq!(
            texts(&scan("As §10.3 requires, and §6.1.1 too.\n")),
            ["§10.3", "§6.1.1"]
        );
        assert_eq!(texts(&scan("Stated in § 4.2 of that record.\n")), ["§4.2"]);
        assert_eq!(texts(&scan("The rule of §12.\n")), ["§12"]);
    }

    /// A locator naming the corpus it points into as well as the section within
    /// it is read as a single reference rather than as a mark and some
    /// unrelated words, so the whole locator is what the author is shown.
    ///
    /// ´claim:legacy:a-locator-naming-a-foreign-corpus-reads-as-one-reference´
    /// ´test:unit:reads-a-foreign-corpus-locator-as-one-reference´
    #[test]
    fn reads_a_foreign_corpus_locator_as_one_reference() {
        assert_eq!(
            texts(&scan("Compare §SPEC L-27.2 for the older statement.\n")),
            ["§SPEC L-27.2"]
        );
    }

    /// The section mark by itself locates nothing, and neither does one
    /// followed by an ordinary word, so prose may discuss sections without
    /// every mention becoming a superseded reference.
    ///
    /// ´claim:legacy:a-section-mark-without-a-locator-is-left-alone´
    /// ´test:unit:leaves-section-marks-without-a-number-alone´
    #[test]
    fn leaves_section_marks_without_a_number_alone() {
        assert_eq!(
            texts(&scan("The § mark alone locates nothing.\n")),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan("Neither does §Appendix here.\n")),
            Vec::<String>::new()
        );
    }

    /// A locator numbered in roman numerals is a section reference too, and is
    /// reported beside the decimal ones. A qualifier trailing after it is no
    /// part of the locator, and a run of letters outside the roman numerals
    /// locates nothing at all.
    ///
    /// ´claim:legacy:a-roman-numeral-locator-is-a-section-reference´
    /// ´test:unit:reports-roman-numeral-section-locators´
    #[test]
    fn reports_roman_numeral_section_locators() {
        assert_eq!(
            texts(&scan("As §II states, and §XIV too.\n")),
            ["§II", "§XIV"]
        );
        assert_eq!(texts(&scan("The rule of §IV.\n")), ["§IV"]);
        assert_eq!(
            texts(&scan(
                "See §IV #25–27 and §VII #16, #17 for the invariants.\n"
            )),
            ["§IV", "§VII"],
            "a trailing qualifier is not part of the locator"
        );
        assert_eq!(
            texts(&scan("As §10.3 requires.\n")),
            ["§10.3"],
            "the digit locator is unaffected"
        );
        assert_eq!(
            texts(&scan("Neither does §SPEC here.\n")),
            Vec::<String>::new(),
            "a run outside I V X L C D M still locates nothing"
        );
    }

    /// A superseded form shown in code font, in a displayed span, or inside a
    /// fenced block is a token exhibited rather than a reference made, and is
    /// left alone. This is what lets the campaign's own documents name the
    /// forms they ban without failing their own lint.
    ///
    /// ´claim:legacy:a-form-shown-rather-than-written-is-left-alone´
    /// ´test:unit:leaves-displayed-and-code-font-forms-alone´
    #[test]
    fn leaves_displayed_and_code_font_forms_alone() {
        assert_eq!(
            texts(&scan(
                "The lint covers `§10.3` and kin, and `ADR-L-550` by number.\n"
            )),
            Vec::<String>::new(),
            "a form in code font is named, not written"
        );
        assert_eq!(
            texts(&scan("Shown: ``land:rigid`` stays displayed.\n")),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan("Text.\n\n```text\n§10.3 and `land:rigid`\n```\n")),
            Vec::<String>::new()
        );
    }

    /// Only the retired record series are reported by number. This
    /// repository's own records keep their numbered identity, so a reference to
    /// one is not swept up by a campaign that was never about them.
    ///
    /// ´claim:legacy:only-the-retired-record-series-are-reported-by-number´
    /// ´test:unit:reports-retired-record-series-only´
    #[test]
    fn reports_retired_record_series_only() {
        assert_eq!(
            texts(&scan(
                "See ADR-L-550 and ADR-M-032 and ADR-S-013 and ADR-R-001.\n"
            )),
            ["ADR-L-550", "ADR-M-032", "ADR-S-013", "ADR-R-001"]
        );
        assert_eq!(
            texts(&scan(
                "But [ADR-T-014](../adr/014-label-calculus.md) keeps its number.\n"
            )),
            Vec::<String>::new(),
            "the repository's own records are not this campaign's corpus"
        );
    }

    /// A record reference is read wherever it legitimately ends — closing a
    /// sentence, or standing inside parentheses — so punctuation after it does
    /// not hide it. A series named with no number after it names nothing.
    ///
    /// ´claim:legacy:a-record-reference-is-read-through-the-punctuation-that-ends-it´
    /// ´test:unit:reads-a-record-reference-standing-last-in-its-sentence´
    #[test]
    fn reads_a_record_reference_standing_last_in_its_sentence() {
        assert_eq!(texts(&scan("The decision is ADR-L-550.\n")), ["ADR-L-550"]);
        assert_eq!(
            texts(&scan("As recorded (ADR-M-032), it holds.\n")),
            ["ADR-M-032"]
        );
        assert_eq!(
            texts(&scan("A bare series ADR-L- names nothing.\n")),
            Vec::<String>::new()
        );
    }

    /// A record named by a bare number is reported by the rule that reads no
    /// series, and by that rule alone. The bound is what the family rests on:
    /// the prefix must open a token, so one inside a longer word is part of
    /// that word; the number is exactly three digits, so a longer one is a
    /// different token rather than one of these with a tail, and a shorter one
    /// is not a record number at all; and a series letter is not a digit, so
    /// every lettered form — the canonical series included — is outside this
    /// family by construction rather than by exempting the files it stands in.
    ///
    /// ´claim:legacy:a-record-named-by-a-bare-number-is-reported-and-bounded´
    /// ´test:unit:reports-unprefixed-record-numbers-within-their-bound´
    #[test]
    fn reports_unprefixed_record_numbers_within_their_bound() {
        let path = Path::new("doc.md");
        let bare = &[LegacyRule::UnprefixedRecordNumber];
        let read = |source: &str| texts(&scan_legacy(path, source, bare));

        assert_eq!(
            read("The number ADR-123 names nothing decidable.\n"),
            ["ADR-123"]
        );
        assert_eq!(
            read("Recorded (ADR-008), and again in ADR-009.\n"),
            ["ADR-008", "ADR-009"],
            "a reference is read inside parentheses and last in its sentence alike"
        );

        let quiet = [
            "The canonical ADR-T-123 keeps its series.",
            "So does the retired ADR-L-123 and ADR-M-123 and ADR-S-123 and ADR-R-123.",
            "A longer number ADR-1234 is a different token.",
            "A shorter one ADR-12 is no record number.",
            "A prefix inside a word XADR-123 belongs to the word.",
            "So does a hyphenated one, PRE-ADR-123.",
            "A bare prefix ADR- names nothing.",
        ];

        for source in quiet {
            assert_eq!(
                read(&format!("{source}\n")),
                Vec::<String>::new(),
                "on: {source}"
            );
        }
    }

    /// The two record rules are two families, and neither reads the other's
    /// shape. A document held to one is held to it alone, and a document held
    /// to the whole policy is read by each rule for its own shape and by
    /// neither for the other's — which is what lets two registers count two
    /// shapes without either counting twice.
    ///
    /// ´claim:legacy:the-two-record-rules-do-not-read-each-others-shape´
    /// ´test:unit:keeps-the-two-record-families-apart´
    #[test]
    fn keeps_the_two_record_families_apart() {
        let path = Path::new("doc.md");
        let source = "Both ADR-L-550 and ADR-008 stand here.\n";

        assert_eq!(
            texts(&scan_legacy(path, source, &[LegacyRule::RecordNumber])),
            ["ADR-L-550"]
        );
        assert_eq!(
            texts(&scan_legacy(
                path,
                source,
                &[LegacyRule::UnprefixedRecordNumber]
            )),
            ["ADR-008"]
        );
        assert_eq!(
            texts(&scan_legacy(path, source, LegacyRule::ALL)),
            ["ADR-L-550", "ADR-008"],
            "the policy carrying both rules reads each shape exactly once, neither rule reaching the other's"
        );
    }

    /// A bare number shown rather than written is left alone by the same
    /// display boundary the other rules keep, so this register's own preamble
    /// and the campaign's own report may name the shape they count.
    ///
    /// ´claim:legacy:a-bare-number-shown-rather-than-written-is-left-alone´
    /// ´test:unit:leaves-displayed-unprefixed-record-numbers-alone´
    #[test]
    fn leaves_displayed_unprefixed_record_numbers_alone() {
        let path = Path::new("doc.md");
        let bare = &[LegacyRule::UnprefixedRecordNumber];

        assert_eq!(
            texts(&scan_legacy(
                path,
                "The family counts `ADR-008` and kin.\n",
                bare
            )),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan_legacy(
                path,
                "Shown: ``ADR-008`` stays displayed.\n",
                bare
            )),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&scan_legacy(path, "Text.\n\n```text\nADR-008\n```\n", bare)),
            Vec::<String>::new()
        );
    }

    /// A tag of the superseded system is reported where that system cited them:
    /// in a participating span. Both the plain tag and the one carrying a step
    /// number are read, because both were citations under the old rules.
    ///
    /// ´claim:legacy:a-tag-in-a-participating-span-is-reported´
    /// ´test:unit:reports-tag-forms-in-participating-spans´
    #[test]
    fn reports_tag_forms_in_participating_spans() {
        assert_eq!(
            texts(&scan(
                "Widths carry zero variance (`land:rigid`), see also `run:update-path#13`.\n"
            )),
            ["land:rigid", "run:update-path#13"]
        );
    }

    /// The tag rule leaves everything else in code font alone: a well-formed
    /// label, a command line, a file name, a language path, a time of day, a
    /// slug hyphenated wrongly, and a step with no number are none of them
    /// tags. The rule is narrow enough to live beside ordinary technical prose.
    ///
    /// ´claim:legacy:the-tag-rule-leaves-labels-and-ordinary-code-font-alone´
    /// ´test:unit:leaves-labels-and-ordinary-code-font-alone´
    #[test]
    fn leaves_labels_and_ordinary_code_font_alone() {
        let quiet = [
            "A label `sec:labels:syntax` is three segments.",
            "Run `cargo test --all-features` beside `Cargo.toml`.",
            "The path `std::fmt` and the pair `10:30` are not tags.",
            "Neither is `-lead:ing` nor `trail:ing-` nor `two--dashes:slug`.",
            "Nor an unnumbered step `run:update-path#x`.",
        ];

        for source in quiet {
            assert_eq!(texts(&scan(source)), Vec::<String>::new(), "on: {source}");
        }
    }

    /// The lint is applied per document and rule by rule: given no rules a
    /// document is held to nothing, and given some it is held to exactly those.
    /// A document that has migrated its identities but legitimately quotes
    /// another corpus's section numbers can therefore meet the rules it can
    /// meet while a campaign is still under way.
    ///
    /// ´claim:legacy:a-document-is-held-only-to-the-rules-it-is-given´
    /// ´test:unit:holds-a-document-to-the-rules-it-is-given´
    #[test]
    fn holds_a_document_to_the_rules_it_is_given() {
        let source = "Quotes §4.2 of RFC 8949, cites `land:rigid`, and names ADR-L-550.\n";
        let path = Path::new("doc.md");

        assert_eq!(texts(&scan_legacy(path, source, &[])), Vec::<String>::new());
        assert_eq!(
            texts(&scan_legacy(path, source, &[LegacyRule::SectionNumber])),
            ["§4.2"]
        );
        assert_eq!(
            texts(&scan_legacy(
                path,
                source,
                &[LegacyRule::TagForm, LegacyRule::RecordNumber]
            )),
            ["land:rigid", "ADR-L-550"],
            "a document quoting a foreign corpus keeps the rules it can meet"
        );
    }

    /// A reference the retired convention wrapped across a comment line
    /// boundary is one reference to the migration lint, at two lines or three,
    /// because the rules are handed a joined region rather than a line. Reading
    /// each line alone saw two halves of nothing, which was a defect of the
    /// reading and not a narrowness of the rules.
    ///
    /// ´claim:legacy:a-reference-wrapped-across-a-comment-boundary-is-one-reference´
    /// ´test:unit:reads-a-reference-wrapped-across-a-comment-boundary´
    #[test]
    fn reads_a_reference_wrapped_across_a_comment_boundary() {
        let path = Path::new("source.rs");
        let read = |source: &str| {
            texts(&scan_regions(
                path,
                source,
                &rust_regions(source),
                LegacyRule::ALL,
            ))
        };

        let wrapped = "/// Interior crossovers are not asserted: the \u{a7}SPEC\n\
                       /// L-16.4.2 formula binds at the outermost pair.\n";

        assert_eq!(read(wrapped), ["\u{a7}SPEC L-16.4.2"]);

        let thrice = "/// the formula named by\n\
                      /// \u{a7}SPEC\n\
                      /// L-16.4.2 binds here.\n";

        assert_eq!(
            read(thrice),
            ["\u{a7}SPEC L-16.4.2"],
            "three lines join as readily as two: the run is the unit, not the pair"
        );

        let apart = "/// the \u{a7}SPEC\n\
                     let binding = 1;\n\
                     /// L-16.4.2 formula binds.\n";

        assert_eq!(
            read(apart),
            Vec::<String>::new(),
            "program text between the two comments ends the region, so neither half is a reference"
        );
    }

    /// Prose wraps too, and a reference broken over a soft line break in
    /// Markdown is one reference for the same reason and by the same layer.
    ///
    /// ´claim:legacy:a-reference-wrapped-across-a-soft-line-break-is-one-reference´
    /// ´test:unit:reads-a-reference-wrapped-across-a-soft-line-break´
    #[test]
    fn reads_a_reference_wrapped_across_a_soft_line_break() {
        assert_eq!(
            texts(&scan(
                "Compare \u{a7}SPEC\nL-27.2 for the older statement.\n"
            )),
            ["\u{a7}SPEC L-27.2"]
        );
        assert_eq!(
            texts(&scan(
                "The decision is recorded in\nADR-L-550 and stands.\n"
            )),
            ["ADR-L-550"],
            "a record reference opening a wrapped line is read where it stands"
        );
    }

    /// Widening what a rule is handed does not widen where a reference may
    /// stand. A form in a string literal, in code font or inside a fenced block
    /// is still a token shown rather than a reference made, and the joined
    /// reading never reaches one.
    ///
    /// ´claim:legacy:widening-the-reading-does-not-widen-participation´
    /// ´test:unit:leaves-a-form-in-a-string-literal-invisible´
    #[test]
    fn leaves_a_form_in_a_string_literal_invisible() {
        let path = Path::new("source.rs");
        let literal = "/// A comment naming nothing.\nlet shown = \"\u{a7}10.3 and ADR-L-550\";\n";

        assert_eq!(
            texts(&scan_regions(
                path,
                literal,
                &rust_regions(literal),
                LegacyRule::ALL
            )),
            Vec::<String>::new(),
            "a string literal is data the program carries, whatever the joined reading can see"
        );

        let split =
            "/// naming the \u{a7}SPEC\nlet shown = \"L-16.4.2\";\n/// and nothing further.\n";

        assert_eq!(
            texts(&scan_regions(
                path,
                split,
                &rust_regions(split),
                LegacyRule::ALL
            )),
            Vec::<String>::new(),
            "and a literal standing between two comments joins neither of them"
        );

        assert_eq!(
            texts(&scan(
                "Shown: `\u{a7}10.3` and ``ADR-008``.\n\n```text\n\u{a7}4.2\n```\n"
            )),
            Vec::<String>::new(),
            "the display boundaries of the prose surface are exactly what they were"
        );
    }

    /// An inline code span broken across a line is one span, not two, so a tag
    /// standing whole inside a wrapped paragraph is read where it stands. A tag
    /// broken mid-token does not survive the break: the span's line becomes a
    /// space and the superseded syntax wrote a tag as one token, so what is left
    /// is no tag rather than a tag reported twice.
    ///
    /// ´claim:legacy:a-code-span-wrapped-across-a-line-is-one-span´
    /// ´test:unit:reads-a-code-span-wrapped-across-a-line-as-one-span´
    #[test]
    fn reads_a_code_span_wrapped_across_a_line_as_one_span() {
        assert_eq!(
            texts(&scan(
                "Widths carry zero variance\n(`land:rigid`), and the rule holds.\n"
            )),
            ["land:rigid"],
            "a tag standing whole in a wrapped paragraph is read where it stands"
        );
        assert_eq!(
            texts(&scan("Cites `land:\nrigid` across the break.\n")),
            Vec::<String>::new(),
            "one span, whose interior now carries a space: no tag, rather than a tag counted twice"
        );
        assert_eq!(
            texts(&scan("Shows ``land:rigid`` displayed.\n")),
            Vec::<String>::new(),
            "a displayed span shows its interior rather than citing it"
        );
    }

    /// The mark's two remaining spellings are the section family's. A heading
    /// quoted after the mark is the same named-division reference the spaced
    /// spelling makes, and a doubled mark opens one reference rather than two —
    /// the pair's first mark is that reference's own opening, so the second is
    /// no second count. Both hold across a comment line boundary.
    ///
    /// ´claim:legacy:the-marks-two-degraded-spellings-are-the-section-familys´
    /// ´test:unit:reads-the-two-degraded-spellings-of-the-mark´
    #[test]
    fn reads_the_two_degraded_spellings_of_the_mark() {
        assert_eq!(
            texts(&scan(
                "Per ADR-S-017 \u{a7}\"No Slot Reservation\" the cells warm.\n"
            )),
            ["ADR-S-017", "\u{a7}\"No Slot Reservation\""],
            "a quoted heading is the named division the spaced spelling names"
        );
        assert_eq!(
            texts(&scan(
                "Classification tags (\u{a7}\u{a7}SPEC L-16.6, L-16.8).\n"
            )),
            ["\u{a7}\u{a7}SPEC L-16.6"],
            "the doubled mark opens one reference, and the pair is not two counts"
        );
        assert_eq!(
            texts(&scan("Ranges are written \u{a7}\u{a7}12.3 and up.\n")),
            ["\u{a7}\u{a7}12.3"],
            "a doubled mark before a bare number is one reference too"
        );

        let path = Path::new("source.rs");
        let read = |source: &str| {
            texts(&scan_regions(
                path,
                source,
                &rust_regions(source),
                LegacyRule::ALL,
            ))
        };

        let split_pair = "/// Classification tags are set by \u{a7}\u{a7}SPEC\n\
                          /// L-16.6 and its siblings.\n";

        assert_eq!(
            read(split_pair),
            ["\u{a7}\u{a7}SPEC L-16.6"],
            "a doubled pair broken across a comment boundary is still one reference"
        );

        let split_title = "/// Recorded under \u{a7}\"No Slot\n\
                           /// Reservation\" in the record.\n";

        assert_eq!(
            read(split_title),
            ["\u{a7}\"No Slot Reservation\""],
            "and so is a heading whose quotation spans the boundary"
        );
    }

    /// Every occurrence of the mark parses to exactly one of four readings — a
    /// companion of the reference before it, a section reference, a word-shaped
    /// mark, or nothing — so the mark is exhausted by construction. The table is
    /// the spellings the corpus and the retired convention between them actually
    /// write.
    ///
    /// ´claim:legacy:every-mark-parses-to-exactly-one-reading´
    /// ´test:unit:parses-every-mark-to-exactly-one-reading´
    #[test]
    fn parses_every_mark_to_exactly_one_reading() {
        let reading = |text: &str| {
            text.match_indices(SECTION_MARK)
                .map(|(offset, _mark)| match read_mark(text, offset) {
                    Mark::Companion => "companion".to_owned(),
                    Mark::Section { text, .. } => format!("section {text}"),
                    Mark::WordShaped { text, .. } => format!("word {text}"),
                    Mark::Bare => "bare".to_owned(),
                })
                .collect::<Vec<_>>()
        };

        let table = [
            ("a \u{a7}10.3 number", vec!["section \u{a7}10.3"]),
            ("a \u{a7} 4.2 spaced number", vec!["section \u{a7}4.2"]),
            ("a \u{a7}VII numeral", vec!["section \u{a7}VII"]),
            (
                "a \u{a7}SPEC L-16.4.2 locator",
                vec!["section \u{a7}SPEC L-16.4.2"],
            ),
            (
                "a \u{a7}\"Named Division\" heading",
                vec!["section \u{a7}\"Named Division\""],
            ),
            (
                "a \u{a7} \"Named Division\" spaced heading",
                vec!["section \u{a7}\"Named Division\""],
            ),
            (
                "a \u{a7}\u{a7}IDEA M-1 range",
                vec!["section \u{a7}\u{a7}IDEA M-1", "companion"],
            ),
            (
                "a \u{a7}\u{a7}12.3 range",
                vec!["section \u{a7}\u{a7}12.3", "companion"],
            ),
            ("a \u{a7}SPEC document token", vec!["word \u{a7}SPEC"]),
            ("a \u{a7}N ordinal placeholder", vec!["word \u{a7}N"]),
            (
                "a \u{a7} Purpose named division",
                vec!["word \u{a7} Purpose"],
            ),
            (
                "a \u{a7}\u{a7}SPEC pair carrying no locator",
                vec!["word \u{a7}\u{a7}SPEC", "companion"],
            ),
            ("a \u{a7}- dash", vec!["bare"]),
            ("a \u{a7}\u{2019} typographic quote", vec!["bare"]),
            ("a \u{a7}\"\" empty heading", vec!["bare"]),
            ("a \u{a7}\"unclosed heading", vec!["bare"]),
            ("a lone \u{a7}", vec!["bare"]),
            ("a \u{a7}  doubled space", vec!["bare"]),
        ];

        for (source, expected) in table {
            assert_eq!(reading(source), expected, "on: {source}");
        }
    }
}
