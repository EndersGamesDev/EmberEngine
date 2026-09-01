// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The generic reference programs, and the typed parameters that instantiate
//! them.
//!
//! A recognizer that carries its own payload is two things wearing one name. The
//! grammar it reads — where a token may open, how far a decimal run goes, which
//! spans are exhibited rather than written — is this binary's meaning, fixed
//! wherever the binary runs. The mark, the bound, and the enumeration it compares
//! against are what one repository happens to hold, and they are the reason a
//! recognizer written that way can only be exercised against the corpus that
//! seeded it. This module is the first half alone: programs over parameters, with
//! nothing in them that a second corpus would have to be talked out of.
//!
//! # The parameters are a type rather than a tuple
//!
//! A mark and a bound have no natural order, and a positional pair of them
//! transposes silently — a caller handing the maximum first gets an empty census
//! rather than a compiler error, and an empty census is exactly what a healthy
//! corpus also produces. So each program takes one named struct, constructed
//! through one constructor, and reads its fields by name. That is also what lets
//! a later decoder hand a program declared values without this module gaining a
//! second entry point for them: the decoder builds the same struct.
//!
//! # The payloads are not here, and the wrappers that still hold them say so
//!
//! Nothing in this module names a corpus. The current marks, bounds, and
//! enumerations stay for now in the modules that always held them, which
//! construct these parameters and call in — a temporary arrangement that the
//! declared-data migration removes by moving each payload into the parameter
//! document of the policy that consumes it. Until then the wrapper is where a
//! reader finds today's values, and this module is where a reader finds what the
//! binary does with any values at all.
//!
//! # Display exclusion belongs to the program, not to its caller
//!
//! A form shown in code font or inside a fenced block is exhibited rather than
//! referred to, and every family in the corpus keeps that boundary. Leaving it to
//! callers was the alternative and it is the arrangement that produced two
//! readings of one surface: a register's own preamble entered the census it
//! described, because the caller that scanned it had not been told. So each
//! program offers the Markdown door beside the plain-text one and draws the
//! boundary itself, and a caller that wants the plain-text door is asking for a
//! run whose participation somebody else has already decided.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_the_marked_numbers_of_a_sentence`] | program | A marked number is the parameterized mark and the decimal run after it, wherever in a sentence it stands, and every one of them in the run is found rather than the first. The mark is what makes a number a reference, so an unmarked endpoint of a range is a number and not an occurrence. |
//! | [`bounds_marked_numbers_by_the_declared_inclusive_range`] | program | The declared bound is inclusive at both ends and is the whole of the family: the endpoints are occurrences and a value one past either end is some other scheme's. A program whose bound came from the corpus it reads could not be exercised against an invented one. |
//! | [`reads_the_whole_decimal_run_rather_than_a_prefix_of_it`] | program | The entire decimal run is read and then bounded, so a longer number is refused rather than accepted as a shorter one wearing a tail. Reading a prefix would let an unrelated number inside the bound by truncation, which is the failure the bound exists to prevent. |
//! | [`leaves_a_mark_that_opens_no_token_alone`] | program | The mark must open a token: a mark inside a word belongs to that word, a doubled mark opens no number, and a mark with no digits after it locates nothing. The rule is drawn against the parameterized mark itself rather than against one spelling of it. |
//! | [`excludes_a_marked_number_shown_rather_than_written`] | program | A marked number in code font or inside a fenced block is exhibited rather than referred to, so it enters no census. The program draws that boundary itself, which is what lets a register's own preamble name what it counts without joining it. |
//! | [`maps_marked_number_offsets_back_into_the_source`] | program | Offsets are into the source rather than into the run handed in, so a caller reading one fragment out of a file still reports where in the file the reference stands. Without that a census would name every occurrence at the top of the source it was found in. |
//! | [`the_mark_numbered_program_counts_paths`] | program | The program declares the path-count codec, because one source may hold many marked numbers and repairing one while leaving another is real progress a per-file ceiling records and a per-file set could not. |
//! | [`reads_a_literal_value_written_into_a_sentence`] | program | A literal is recognised by the whole of the value, matched entire and wherever it stands, and every match in the run is found rather than the first. Matching entire is what makes a set of sentences safe to count: a shorter rule would read ordinary prose on the same subject as a reference. |
//! | [`orders_literal_occurrences_by_where_they_stand`] | program | Occurrences are ordered by where they stand rather than by which value found them, and a value standing inside another is its own occurrence rather than being swallowed by it. Where two of them open at one offset the tie falls to the earlier-declared value, which is arbitrary and is therefore written down: the alternative is an order that varies with how a set happens to be stored. |
//! | [`refuses_an_empty_repeated_or_broken_literal`] | program | An empty value, a repeated one, and one carrying a line break are refused when the instance is built rather than dropped from it. A value silently ignored would let an instance believe it had declared a locator it had not, and its census would be quietly narrower than the policy it was activated under. |
//! | [`excludes_a_literal_shown_rather_than_written`] | program | A literal shown in code font or inside a fenced block is exhibited rather than referred to, so it enters no census — the same boundary the marked ordinal keeps, drawn by the program rather than by its caller. |
//! | [`maps_literal_offsets_back_into_the_source`] | program | Offsets are into the source rather than into the run handed in, so a caller reading one fragment out of a file still reports where in the file the locator stands. |
//! | [`the_literal_set_program_counts_paths`] | program | The literal-set program declares the path-count codec, for the marked ordinal's reason: one source may hold many of these locators, and repairing one while another stands is progress a count records. |
//! | [`reads_the_prefixed_numbers_an_exact_bound_admits`] | program | A prefixed number is the prefix opening a token and one complete dot-joined decimal token the bound admits. The exact bound compares the whole token, so a dotted item stays distinct from the item whose number it extends, and an alphanumeric tail makes the whole a longer name rather than one of these wearing a suffix. |
//! | [`bounds_a_prefixed_number_by_its_leading_component`] | program | A leading bound reads the complete dotted token and then compares its first component alone, so a suffix belongs to the one occurrence rather than opening a second. A range and an enumerated set are both leading bounds and differ only in what they admit: a scheme that numbered in bands with gaps needs the set, because a range spanning its bands would admit numbers no item ever bore. |
//! | [`shields_a_token_the_section_rule_has_already_read`] | program | A shielded instance leaves alone a token standing inside a reference the generic section rule has already read whole: the whole is one reference of the section family, and counting the token again would register one debt twice. An unshielded instance is one whose spelling that grammar cannot reach, so nothing is taken from it. |
//! | [`detects_an_overlap_between_two_prefix_number_domains`] | program | Two instances over one prefix whose bounds meet would each count the same token, so one debt would stand in two registers and one repair would have to be made twice. Instances over different prefixes read disjoint spans whatever their bounds, so the prefix is compared before the bound is. |
//! | [`excludes_a_displayed_prefixed_number_and_maps_offsets_back`] | program | A prefixed number shown in code font or inside a fenced block is exhibited rather than referred to, and offsets are into the source rather than into the run handed in — the two boundaries every program in this module keeps. |
//! | [`the_prefix_number_program_counts_paths`] | program | The prefix-number program declares the path-count codec, which is what lets three instances sharing one program keep three independent registers: the identity is the file and the count, and the full policy key is what tells one instance's row from another's. |

use crate::catalogue::Codec;
use crate::legacy::{Mark, SECTION_MARK, opens_a_token, read_mark};

/// One occurrence a reference program reads: the form as written, and where it
/// stands in the source.
///
/// The pair is the whole of what a census needs and deliberately no more. A
/// richer observation would have to name the instance that found it, and an
/// occurrence that carried its own provenance would let two instances disagree
/// about one span while both looked well-formed.
pub type Sighting = (String, usize);

/// The parameters of one marked-ordinal instance.
///
/// A marked ordinal is navigation within a mutable numbered sequence rather than
/// a stable identity, and the three values here are the whole of what separates
/// one such sequence from another: the mark that makes a number a reference, and
/// the two ends of the run the sequence actually issued. The bound is not
/// fastidiousness. An unbounded rule would count every mark and number in a
/// corpus — a percentage in a comment, an ordinal in a list — and a census over
/// those could never reach zero, because reaching zero would mean rewriting prose
/// that was never about the sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkNumbered {
    mark: char,
    minimum: u32,
    maximum: u32,
}

impl MarkNumbered {
    /// Instantiate the program with a mark and an inclusive numeric bound.
    ///
    /// Both ends are inclusive because both ends are things a sequence issued: a
    /// half-open bound would make the last item the corpus actually numbered
    /// unreachable, and a reader checking the parameters against the retiring
    /// document would have to know to add one.
    #[must_use]
    pub const fn new(mark: char, minimum: u32, maximum: u32) -> Self {
        Self {
            mark,
            minimum,
            maximum,
        }
    }

    /// The identity form this program's tolerated violations are written in.
    ///
    /// One source may hold many marked numbers, and replacing one while leaving
    /// another is a real partial repair. A per-file set could not record that
    /// progress and a digest would give one file as many identities as it has
    /// occurrences, so the count is the codec that says what a repair is here.
    #[must_use]
    pub const fn codec() -> Codec {
        Codec::PathCount
    }

    /// Read one run of plain text for this instance's marked numbers.
    ///
    /// The text is a run standing at `base` in the source, and the offsets
    /// returned are into the source, so a caller reading a fragment — one comment
    /// out of a Rust file, say — still reports where the fragment stands.
    #[must_use]
    pub fn scan_text(&self, base: usize, text: &str) -> Vec<Sighting> {
        let mut found = Vec::new();

        for (offset, _mark) in text.match_indices(self.mark) {
            if !self.opens_a_token(text, offset) {
                continue;
            }

            let rest = &text[offset + self.mark.len_utf8()..];
            let digits = decimal_run(rest);

            if digits.is_empty() {
                continue;
            }

            // The whole run is read before it is bounded, so a longer number is
            // refused rather than admitted as a shorter one wearing a tail.
            let Ok(number) = digits.parse::<u32>() else {
                continue;
            };

            if number < self.minimum || number > self.maximum {
                continue;
            }

            found.push((format!("{}{digits}", self.mark), base + offset));
        }

        found
    }

    /// Read one Markdown document for this instance's marked numbers.
    ///
    /// Only running text is read: a form shown in code font or inside a fenced
    /// block is exhibited rather than referred to.
    #[must_use]
    pub fn scan_markdown(&self, source: &str) -> Vec<Sighting> {
        scan_running_text(source, |base, text| self.scan_text(base, text))
    }

    /// Whether the mark at `offset` opens a token rather than continuing one.
    ///
    /// The boundary is drawn against the parameterized mark itself: a mark inside
    /// a word belongs to that word, and a mark directly after another mark opens
    /// no number, whatever character the instance chose. Restating the rule per
    /// spelling was the alternative, and it is how one family came to have two
    /// readings of the same surface.
    fn opens_a_token(&self, text: &str, offset: usize) -> bool {
        text[..offset].chars().next_back().is_none_or(|character| {
            !(character.is_ascii_alphanumeric() || character == '_' || character == self.mark)
        })
    }
}

/// What one literal-set instance's declared values failed to be.
///
/// The three are refused rather than tolerated because each would make the
/// census say something untrue rather than merely something odd. Reporting them
/// as findings against the corpus was the alternative and it is the wrong
/// address: the corpus did not write these values, the instance that declares
/// the policy did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralDefect {
    /// A value carrying nothing, which stands at every offset of every source.
    Empty,
    /// A value declared twice, which would count one occurrence once per copy.
    Duplicate(String),
    /// A value carrying a line break, which no participating run holds entire.
    ///
    /// Running text reaches a reader joined across a soft break, so a value
    /// written with the break in it can never match what the reader saw. A
    /// value that cannot match is not a strict policy; it is a policy with a
    /// typo, and it is refused where the typo is.
    LineBreak(String),
}

/// The parameters of one literal-set instance.
///
/// A selected heading sentence used verbatim as a locator is an implicit
/// identity system, and the verbatim strings are the whole of what separates one
/// such system from another. Matching them entire is what makes the family safe
/// to count: a name written as an indicative sentence is a thing somebody wrote
/// about one document rather than a phrase that recurs by accident, and no rule
/// shorter than the sentence tells the two apart.
///
/// The values keep the order they were declared in, which decides nothing about
/// which occurrences are found and one thing about ties: where two values stand
/// at one offset, the earlier-declared one is reported first. That is arbitrary
/// and it is written down, because the alternative is an order that varies with
/// the hashing of a set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralSet {
    values: Vec<String>,
}

impl LiteralSet {
    /// Instantiate the program with a set of verbatim values.
    ///
    /// An empty value, a repeated one, and one carrying a line break are refused
    /// rather than dropped: silently ignoring a value would let an instance
    /// believe it had declared a locator it had not, and the census would then
    /// be quietly narrower than the policy it was activated under.
    ///
    /// # Errors
    ///
    /// Returns the first defect found among the values, in declaration order.
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Result<Self, LiteralDefect> {
        let mut kept: Vec<String> = Vec::new();

        for value in values {
            let value = value.into();

            if value.is_empty() {
                return Err(LiteralDefect::Empty);
            }

            if value.contains('\n') || value.contains('\r') {
                return Err(LiteralDefect::LineBreak(value));
            }

            if kept.contains(&value) {
                return Err(LiteralDefect::Duplicate(value));
            }

            kept.push(value);
        }

        Ok(Self { values: kept })
    }

    /// The identity form this program's tolerated violations are written in.
    ///
    /// One source may hold many of these locators, and the reasoning is the
    /// marked ordinal's exactly: a count is what records the repair of one while
    /// another still stands.
    #[must_use]
    pub const fn codec() -> Codec {
        Codec::PathCount
    }

    /// Read one run of plain text for this instance's literals.
    ///
    /// Every match is found rather than the first, and the occurrences are
    /// ordered by where they stand rather than by which value found them, so a
    /// census does not depend on the order the values were declared in.
    #[must_use]
    pub fn scan_text(&self, base: usize, text: &str) -> Vec<Sighting> {
        let mut found = Vec::new();

        for value in &self.values {
            for (offset, matched) in text.match_indices(value.as_str()) {
                found.push((matched.to_owned(), base + offset));
            }
        }

        found.sort_by_key(|(_text, offset)| *offset);
        found
    }

    /// Read one Markdown document for this instance's literals.
    ///
    /// Only running text is read, on the same boundary every other program
    /// keeps.
    #[must_use]
    pub fn scan_markdown(&self, source: &str) -> Vec<Sighting> {
        scan_running_text(source, |base, text| self.scan_text(base, text))
    }
}

/// How one prefix-number instance bounds the token it reads.
///
/// Three shapes rather than one, because the three retiring schemes bounded
/// themselves in three ways and flattening them would lie about two of them. An
/// enumeration written as a range would admit numbers the scheme never issued; a
/// range written as an enumeration would have to be regenerated whenever the
/// scheme's end moved. The bound a scheme actually had is the bound its instance
/// declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixBound {
    /// The whole dotted token is one of an enumerated set.
    ///
    /// The comparison is against the complete token, so a dotted work-item
    /// number stays distinct from the number of its parent.
    Exact(Vec<String>),
    /// The token's leading decimal component lies within an inclusive range.
    ///
    /// A suffix belongs to the one occurrence rather than opening a second, so a
    /// chapter locator carrying a subsection is one reference to that chapter.
    LeadingRange {
        /// The lowest leading component the scheme issued.
        minimum: u32,
        /// The highest leading component the scheme issued.
        maximum: u32,
    },
    /// The token's leading decimal component is one of an enumerated set.
    ///
    /// A scheme that numbered in bands with gaps between them needs this rather
    /// than a range: a range spanning the bands would admit numbers no item of
    /// the scheme ever bore.
    LeadingSet(Vec<u32>),
}

impl PrefixBound {
    /// Whether this bound admits one complete dotted token.
    fn admits(&self, token: &str) -> bool {
        match self {
            Self::Exact(values) => values.iter().any(|value| value == token),
            Self::LeadingRange { minimum, maximum } => {
                leading_component(token).is_some_and(|head| head >= *minimum && head <= *maximum)
            }
            Self::LeadingSet(values) => {
                leading_component(token).is_some_and(|head| values.contains(&head))
            }
        }
    }

    /// Whether some token this bound admits is one the other admits too.
    ///
    /// Enumerated tokens are tested against the other bound directly, because a
    /// finite set is its own witness. Two leading bounds are compared on their
    /// leading components alone, which is exactly what both of them read.
    fn meets(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Exact(values), bound) => values.iter().any(|value| bound.admits(value)),
            (bound, Self::Exact(values)) => values.iter().any(|value| bound.admits(value)),
            (
                Self::LeadingRange { minimum, maximum },
                Self::LeadingRange {
                    minimum: other_minimum,
                    maximum: other_maximum,
                },
            ) => minimum <= other_maximum && other_minimum <= maximum,
            (Self::LeadingRange { minimum, maximum }, Self::LeadingSet(values))
            | (Self::LeadingSet(values), Self::LeadingRange { minimum, maximum }) => {
                values.iter().any(|head| head >= minimum && head <= maximum)
            }
            (Self::LeadingSet(values), Self::LeadingSet(others)) => {
                values.iter().any(|head| others.contains(head))
            }
        }
    }
}

/// The parameters of one prefix-number instance.
///
/// A work item, chapter, or record named only by a plan-local prefixed number
/// loses its referent when the plan changes or retires. What separates one such
/// scheme from another is its prefix and the bound on what may follow, and both
/// stand here rather than in the reading — which is prefix token-opening, one
/// complete dot-joined decimal token, rejection of an alphanumeric tail, the
/// display boundary, offsets, and ordering.
///
/// # Shielding is the instance's shape rather than a preference
///
/// A locator-shaped instance shares its spelling with the second half of the
/// generic section-reference grammar, so an occurrence the section rule has
/// already read whole is that reference's rather than a second one. Counting it
/// again would register one debt twice and offer two repairs where one is owed.
/// The precedence itself is not configurable and is computed by calling the
/// section rule rather than by restating it, so the two readings cannot drift
/// into counting different things; the field here says only whether this
/// instance is one of the shapes that grammar can reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixNumbers {
    prefix: String,
    bound: PrefixBound,
    shielded: bool,
}

impl PrefixNumbers {
    /// Instantiate the program with a prefix, a bound, and its shielding.
    #[must_use]
    pub fn new(prefix: impl Into<String>, bound: PrefixBound, shielded: bool) -> Self {
        Self {
            prefix: prefix.into(),
            bound,
            shielded,
        }
    }

    /// The identity form this program's tolerated violations are written in.
    #[must_use]
    pub const fn codec() -> Codec {
        Codec::PathCount
    }

    /// Whether this instance and another can read one occurrence between them.
    ///
    /// Two instances over one prefix whose bounds meet would each count the same
    /// token, so one repair would have to be made twice and one debt would stand
    /// in two registers. Instances over different prefixes read disjoint spans
    /// whatever their bounds, so the prefixes are compared first.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.prefix == other.prefix && self.bound.meets(&other.bound)
    }

    /// Read one run of plain text for this instance's prefixed numbers.
    ///
    /// The text is a run standing at `base` in the source, already joined and
    /// with any comment leaders resolved away, so a reference wrapped across two
    /// comment lines is one reference rather than two halves.
    #[must_use]
    pub fn scan_text(&self, base: usize, text: &str) -> Vec<Sighting> {
        let covered = if self.shielded {
            section_spans(text)
        } else {
            Vec::new()
        };
        let mut found = Vec::new();

        for (offset, prefix) in text.match_indices(self.prefix.as_str()) {
            if !opens_a_token(text, offset) {
                continue;
            }

            // A token inside a reference the section rule already read is that
            // reference's second half rather than an occurrence of its own.
            if covered
                .iter()
                .any(|(start, end)| offset >= *start && offset < *end)
            {
                continue;
            }

            let rest = &text[offset + prefix.len()..];
            let Some(token) = dotted_number(rest) else {
                continue;
            };

            if !self.bound.admits(token) {
                continue;
            }

            found.push((format!("{}{token}", self.prefix), base + offset));
        }

        found
    }

    /// Read one Markdown document for this instance's prefixed numbers.
    #[must_use]
    pub fn scan_markdown(&self, source: &str) -> Vec<Sighting> {
        scan_running_text(source, |base, text| self.scan_text(base, text))
    }
}

/// The byte ranges of `text` the generic section rule reads as references.
///
/// The rule is called rather than restated, so a shielded instance and the
/// section family cannot come to disagree about what a reference is
/// (ADR-T-020, The migration disciplines).
fn section_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();

    for (offset, _mark) in text.match_indices(SECTION_MARK) {
        if let Mark::Section { extent, .. } = read_mark(text, offset) {
            spans.push((offset, offset + extent));
        }
    }

    spans
}

/// The leading decimal component of a dotted token.
fn leading_component(token: &str) -> Option<u32> {
    token.split('.').next().unwrap_or(token).parse::<u32>().ok()
}

/// The dot-joined digit runs opening a token, when the token is one.
///
/// The run ends at the first character that is neither a digit nor a full stop
/// joining two digit runs, so a trailing full stop belongs to the sentence
/// rather than to the token. A letter, a digit or an underscore standing after
/// the run means the whole is a longer name rather than one of these with a
/// tail, and nothing is read.
fn dotted_number(rest: &str) -> Option<&str> {
    let mut end = decimal_run(rest).len();

    if end == 0 {
        return None;
    }

    loop {
        let tail = &rest[end..];

        let Some(after_dot) = tail.strip_prefix('.') else {
            break;
        };

        let run = decimal_run(after_dot).len();

        if run == 0 {
            break;
        }

        end += 1 + run;
    }

    let continues = rest[end..]
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');

    (!continues).then(|| &rest[..end])
}

/// The leading run of decimal digits, however long it runs.
///
/// The run is not truncated, because truncating it is what would let an
/// unrelated longer number in by its prefix. A caller bounds the value the run
/// parses to instead, which refuses the longer number as a whole.
fn decimal_run(text: &str) -> &str {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());

    &text[..end]
}

/// Read every participating run of one Markdown document with the given reader.
///
/// Fenced and indented blocks are display and are skipped entire; the reader is
/// handed each remaining text run together with where that run stands in the
/// source, and the occurrences it returns are ordered by that offset. Ordering
/// here rather than in each program is deliberate: a census whose order depended
/// on which program produced it could not be compared against a register written
/// by another.
fn scan_running_text(
    source: &str,
    mut reader: impl FnMut(usize, &str) -> Vec<Sighting>,
) -> Vec<Sighting> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let mut found = Vec::new();
    let mut depth = 0_usize;

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                depth += 1;
            }
            Event::End(TagEnd::CodeBlock) => depth = depth.saturating_sub(1),
            Event::Text(_) if depth == 0 => {
                let start = range.start;

                found.extend(reader(start, &source[range]));
            }
            _ => {}
        }
    }

    found.sort_by_key(|(_text, offset)| *offset);
    found
}

#[cfg(test)]
mod tests {
    use super::{LiteralDefect, LiteralSet, MarkNumbered, PrefixBound, PrefixNumbers};
    use crate::catalogue::Codec;

    /// An invented marked-ordinal instance: a mark no corpus here writes, and a
    /// bound no document here issued.
    fn instance() -> MarkNumbered {
        MarkNumbered::new('@', 3, 40)
    }

    fn texts(found: &[(String, usize)]) -> Vec<String> {
        found.iter().map(|(text, _offset)| text.clone()).collect()
    }

    fn reads(text: &str) -> Vec<String> {
        texts(&instance().scan_text(0, text))
    }

    /// A marked number is the parameterized mark and the decimal run after it,
    /// wherever in a sentence it stands, and every one of them in the run is
    /// found rather than the first. The mark is what makes a number a
    /// reference, so an unmarked endpoint of a range is a number and not an
    /// occurrence.
    ///
    /// ´claim:program:a-marked-number-is-the-mark-and-the-decimal-run-after-it´
    /// ´test:unit:reads-the-marked-numbers-of-a-sentence´
    #[test]
    fn reads_the_marked_numbers_of_a_sentence() {
        assert_eq!(
            reads("Witnessed by @7, and by @31 and @19.\n"),
            ["@7", "@31", "@19"]
        );
        assert_eq!(reads("@3"), ["@3"]);
        assert_eq!(
            reads("The pair @12-13 splits either way.\n"),
            ["@12"],
            "the unmarked endpoint of a range is a number, not a reference"
        );
    }

    /// The declared bound is inclusive at both ends and is the whole of the
    /// family: the endpoints are occurrences and a value one past either end is
    /// some other scheme's. A program whose bound came from the corpus it reads
    /// could not be exercised against an invented one.
    ///
    /// ´claim:program:the-declared-bound-is-inclusive-and-is-the-whole-of-the-family´
    /// ´test:unit:bounds-marked-numbers-by-the-declared-inclusive-range´
    #[test]
    fn bounds_marked_numbers_by_the_declared_inclusive_range() {
        assert_eq!(reads("Both @3 and @40 stand at the ends.\n"), ["@3", "@40"]);

        let quiet = [
            "One short at @2.",
            "One past at @41.",
            "And @0 numbers nothing.",
        ];

        for source in quiet {
            assert_eq!(reads(source), Vec::<String>::new(), "on: {source}");
        }
    }

    /// The entire decimal run is read and then bounded, so a longer number is
    /// refused rather than accepted as a shorter one wearing a tail. Reading a
    /// prefix would let an unrelated number inside the bound by truncation,
    /// which is the failure the bound exists to prevent.
    ///
    /// ´claim:program:the-whole-decimal-run-is-read-before-it-is-bounded´
    /// ´test:unit:reads-the-whole-decimal-run-rather-than-a-prefix-of-it´
    #[test]
    fn reads_the_whole_decimal_run_rather_than_a_prefix_of_it() {
        assert_eq!(
            reads("An identifier @315 is refused whole.\n"),
            Vec::<String>::new(),
            "the run is read entire, so the longer number is not admitted by its prefix"
        );
        assert_eq!(
            reads("Nor is @3000000000000 within any bound.\n"),
            Vec::<String>::new(),
            "a run too long to be a number of this width is refused rather than truncated"
        );
    }

    /// The mark must open a token: a mark inside a word belongs to that word, a
    /// doubled mark opens no number, and a mark with no digits after it locates
    /// nothing. The rule is drawn against the parameterized mark itself rather
    /// than against one spelling of it.
    ///
    /// ´claim:program:the-mark-must-open-a-token-to-name-an-ordinal´
    /// ´test:unit:leaves-a-mark-that-opens-no-token-alone´
    #[test]
    fn leaves_a_mark_that_opens_no_token_alone() {
        let quiet = [
            "An address alpha@31 is one word.",
            "A doubled @@31 opens nothing.",
            "An underscore _@31 continues a name.",
            "A mark @ alone locates nothing.",
            "And @ 31 is spaced apart.",
        ];

        for source in quiet {
            assert_eq!(reads(source), Vec::<String>::new(), "on: {source}");
        }
    }

    /// A marked number in code font or inside a fenced block is exhibited rather
    /// than referred to, so it enters no census. The program draws that boundary
    /// itself, which is what lets a register's own preamble name what it counts
    /// without joining it.
    ///
    /// ´claim:program:a-marked-number-shown-rather-than-written-is-left-alone´
    /// ´test:unit:excludes-a-marked-number-shown-rather-than-written´
    #[test]
    fn excludes_a_marked_number_shown_rather_than_written() {
        let displayed = "A shape such as `@31` is named here.\n\n```text\n@19 and @7\n```\n";

        assert_eq!(
            texts(&instance().scan_markdown(displayed)),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&instance().scan_markdown("Witnessed by @31 and @7.\n")),
            ["@31", "@7"],
            "and running text is read as it always was"
        );
    }

    /// Offsets are into the source rather than into the run handed in, so a
    /// caller reading one fragment out of a file still reports where in the file
    /// the reference stands. Without that a census would name every occurrence
    /// at the top of the source it was found in.
    ///
    /// ´claim:program:offsets-are-into-the-source-rather-than-into-the-run´
    /// ´test:unit:maps-marked-number-offsets-back-into-the-source´
    #[test]
    fn maps_marked_number_offsets_back_into_the_source() {
        assert_eq!(
            instance().scan_text(1000, "see @31 there"),
            [("@31".to_owned(), 1004)]
        );
    }

    /// The program declares the path-count codec, because one source may hold
    /// many marked numbers and repairing one while leaving another is real
    /// progress a per-file ceiling records and a per-file set could not.
    ///
    /// ´claim:program:the-mark-numbered-program-identifies-a-violation-by-a-count´
    /// ´test:unit:the-mark-numbered-program-counts-paths´
    #[test]
    fn the_mark_numbered_program_counts_paths() {
        assert_eq!(MarkNumbered::codec(), Codec::PathCount);
        assert_eq!(MarkNumbered::codec().field(), "path_counts");
    }

    /// An invented literal-set instance: three sentences no document here heads.
    fn literals() -> LiteralSet {
        LiteralSet::new([
            "The tide answers the moon",
            "Salt keeps the harvest",
            "The tide",
        ])
        .expect("well-formed values")
    }

    fn literal_reads(text: &str) -> Vec<String> {
        texts(&literals().scan_text(0, text))
    }

    /// A literal is recognised by the whole of the value, matched entire and
    /// wherever it stands, and every match in the run is found rather than the
    /// first. Matching entire is what makes a set of sentences safe to count: a
    /// shorter rule would read ordinary prose on the same subject as a
    /// reference.
    ///
    /// ´claim:program:a-literal-is-the-whole-value-matched-entire´
    /// ´test:unit:reads-a-literal-value-written-into-a-sentence´
    #[test]
    fn reads_a_literal_value_written_into_a_sentence() {
        assert_eq!(
            literal_reads("Under Salt keeps the harvest, the year holds.\n"),
            ["Salt keeps the harvest"]
        );
        assert_eq!(
            literal_reads("Neither moon nor harvest is named here.\n"),
            Vec::<String>::new()
        );
    }

    /// Occurrences are ordered by where they stand rather than by which value
    /// found them, and a value standing inside another is its own occurrence
    /// rather than being swallowed by it. Where two of them open at one offset
    /// the tie falls to the earlier-declared value, which is arbitrary and is
    /// therefore written down: the alternative is an order that varies with how
    /// a set happens to be stored.
    ///
    /// ´claim:program:literal-occurrences-are-ordered-by-offset-and-ties-by-declaration´
    /// ´test:unit:orders-literal-occurrences-by-where-they-stand´
    #[test]
    fn orders_literal_occurrences_by_where_they_stand() {
        assert_eq!(
            literal_reads("Salt keeps the harvest, and The tide answers the moon.\n"),
            [
                "Salt keeps the harvest",
                "The tide answers the moon",
                "The tide"
            ],
            "the later-standing pair opens at one offset, so the earlier-declared value leads"
        );
        assert_eq!(
            literal_reads("The tide alone, after Salt keeps the harvest.\n"),
            ["The tide", "Salt keeps the harvest"],
            "and where the offsets differ, the offsets decide"
        );
    }

    /// An empty value, a repeated one, and one carrying a line break are refused
    /// when the instance is built rather than dropped from it. A value silently
    /// ignored would let an instance believe it had declared a locator it had
    /// not, and its census would be quietly narrower than the policy it was
    /// activated under.
    ///
    /// ´claim:program:a-malformed-literal-refuses-the-instance-rather-than-being-dropped´
    /// ´test:unit:refuses-an-empty-repeated-or-broken-literal´
    #[test]
    fn refuses_an_empty_repeated_or_broken_literal() {
        assert_eq!(
            LiteralSet::new(["a good one", ""]),
            Err(LiteralDefect::Empty)
        );
        assert_eq!(
            LiteralSet::new(["a good one", "a good one"]),
            Err(LiteralDefect::Duplicate("a good one".to_owned()))
        );
        assert_eq!(
            LiteralSet::new(["a good one", "broken\nacross"]),
            Err(LiteralDefect::LineBreak("broken\nacross".to_owned()))
        );
        assert!(
            LiteralSet::new(Vec::<String>::new()).is_ok(),
            "and no value at all is a set"
        );
    }

    /// A literal shown in code font or inside a fenced block is exhibited rather
    /// than referred to, so it enters no census — the same boundary the marked
    /// ordinal keeps, drawn by the program rather than by its caller.
    ///
    /// ´claim:program:a-literal-shown-rather-than-written-is-left-alone´
    /// ´test:unit:excludes-a-literal-shown-rather-than-written´
    #[test]
    fn excludes_a_literal_shown_rather_than_written() {
        let displayed = "A name such as `Salt keeps the harvest` is quoted here.\n\n```text\nThe tide answers the moon\n```\n";

        assert_eq!(
            texts(&literals().scan_markdown(displayed)),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&literals().scan_markdown("Under Salt keeps the harvest, the year holds.\n")),
            ["Salt keeps the harvest"],
            "and running text is read as it always was"
        );
    }

    /// Offsets are into the source rather than into the run handed in, so a
    /// caller reading one fragment out of a file still reports where in the file
    /// the locator stands.
    ///
    /// ´claim:program:literal-offsets-are-into-the-source-rather-than-into-the-run´
    /// ´test:unit:maps-literal-offsets-back-into-the-source´
    #[test]
    fn maps_literal_offsets_back_into_the_source() {
        assert_eq!(
            literals().scan_text(1000, "see Salt keeps the harvest there"),
            [("Salt keeps the harvest".to_owned(), 1004)]
        );
    }

    /// The literal-set program declares the path-count codec, for the marked
    /// ordinal's reason: one source may hold many of these locators, and
    /// repairing one while another stands is progress a count records.
    ///
    /// ´claim:program:the-literal-set-program-identifies-a-violation-by-a-count´
    /// ´test:unit:the-literal-set-program-counts-paths´
    #[test]
    fn the_literal_set_program_counts_paths() {
        assert_eq!(LiteralSet::codec(), Codec::PathCount);
        assert_eq!(LiteralSet::codec().field(), "path_counts");
    }

    /// An invented enumerated instance: a prefix and four issued tokens.
    fn issued() -> PrefixNumbers {
        PrefixNumbers::new(
            "TN-",
            PrefixBound::Exact(vec![
                "3".to_owned(),
                "9".to_owned(),
                "7.1".to_owned(),
                "7.2".to_owned(),
            ]),
            false,
        )
    }

    /// An invented ranged locator instance, shielded as a locator shape is.
    fn ranged() -> PrefixNumbers {
        PrefixNumbers::new(
            "K-",
            PrefixBound::LeadingRange {
                minimum: 1,
                maximum: 12,
            },
            true,
        )
    }

    /// An invented gapped locator instance over the same prefix as the ranged one.
    fn gapped() -> PrefixNumbers {
        PrefixNumbers::new("K-", PrefixBound::LeadingSet(vec![210, 240, 380]), true)
    }

    /// A prefixed number is the prefix opening a token and one complete
    /// dot-joined decimal token the bound admits. The exact bound compares the
    /// whole token, so a dotted item stays distinct from the item whose number
    /// it extends, and an alphanumeric tail makes the whole a longer name rather
    /// than one of these wearing a suffix.
    ///
    /// ´claim:program:a-prefixed-number-is-one-complete-dotted-token-the-bound-admits´
    /// ´test:unit:reads-the-prefixed-numbers-an-exact-bound-admits´
    #[test]
    fn reads_the_prefixed_numbers_an_exact_bound_admits() {
        assert_eq!(
            texts(&issued().scan_text(0, "Built under TN-7.1 and TN-9.\n")),
            ["TN-7.1", "TN-9"]
        );
        assert_eq!(
            texts(&issued().scan_text(0, "Standing last in the sentence, TN-3.\n")),
            ["TN-3"],
            "a trailing full stop belongs to the sentence rather than to the token"
        );

        let quiet = [
            "A scheme numbered TN-4 elsewhere.",
            "Nor TN-7.3, one past the last issued.",
            "Nor TN-7 alone, which is the parent and not an issued item.",
            "An identifier a-TN-3 continues a word.",
            "And XTN-3 is one word.",
            "A trailing letter makes TN-3x another token.",
        ];

        for source in quiet {
            assert_eq!(
                texts(&issued().scan_text(0, source)),
                Vec::<String>::new(),
                "on: {source}"
            );
        }
    }

    /// A leading bound reads the complete dotted token and then compares its
    /// first component alone, so a suffix belongs to the one occurrence rather
    /// than opening a second. A range and an enumerated set are both leading
    /// bounds and differ only in what they admit: a scheme that numbered in
    /// bands with gaps needs the set, because a range spanning its bands would
    /// admit numbers no item ever bore.
    ///
    /// ´claim:program:a-leading-bound-compares-the-first-component-of-the-whole-token´
    /// ´test:unit:bounds-a-prefixed-number-by-its-leading-component´
    #[test]
    fn bounds_a_prefixed_number_by_its_leading_component() {
        assert_eq!(
            texts(&ranged().scan_text(0, "The K-4.6.2 formula binds.\n")),
            ["K-4.6.2"],
            "the suffix belongs to the one occurrence"
        );
        assert_eq!(
            texts(&ranged().scan_text(0, "At K-1 and K-12, the ends.\n")),
            ["K-1", "K-12"]
        );
        assert_eq!(
            texts(&ranged().scan_text(0, "A chapter K-13 the document never had.\n")),
            Vec::<String>::new()
        );

        assert_eq!(
            texts(&gapped().scan_text(0, "As K-240 requires.\n")),
            ["K-240"]
        );
        assert_eq!(
            texts(&gapped().scan_text(0, "A record K-250 no band carried.\n")),
            Vec::<String>::new(),
            "the gap between bands is the point of enumerating rather than ranging"
        );
    }

    /// A shielded instance leaves alone a token standing inside a reference the
    /// generic section rule has already read whole: the whole is one reference
    /// of the section family, and counting the token again would register one
    /// debt twice. An unshielded instance is one whose spelling that grammar
    /// cannot reach, so nothing is taken from it.
    ///
    /// ´claim:program:a-token-the-section-rule-already-read-is-not-counted-again´
    /// ´test:unit:shields-a-token-the-section-rule-has-already-read´
    #[test]
    fn shields_a_token_the_section_rule_has_already_read() {
        assert_eq!(
            texts(&ranged().scan_text(0, "As \u{a7}K-4.6.2 requires.\n")),
            Vec::<String>::new(),
            "a mark before the token makes the whole one section reference"
        );
        assert_eq!(
            texts(&ranged().scan_text(0, "As \u{a7}SPEC K-4.6.2 requires, and K-9 besides.\n")),
            ["K-9"],
            "and the sibling standing outside that reference is still counted"
        );

        let unshielded = PrefixNumbers::new(
            "K-",
            PrefixBound::LeadingRange {
                minimum: 1,
                maximum: 12,
            },
            false,
        );

        assert_eq!(
            texts(&unshielded.scan_text(0, "As \u{a7}K-4.6.2 requires.\n")),
            ["K-4.6.2"],
            "an instance the grammar cannot reach yields nothing to it"
        );
    }

    /// Two instances over one prefix whose bounds meet would each count the same
    /// token, so one debt would stand in two registers and one repair would have
    /// to be made twice. Instances over different prefixes read disjoint spans
    /// whatever their bounds, so the prefix is compared before the bound is.
    ///
    /// ´claim:program:two-instances-overlap-when-one-prefix-and-two-bounds-meet´
    /// ´test:unit:detects-an-overlap-between-two-prefix-number-domains´
    #[test]
    fn detects_an_overlap_between_two_prefix_number_domains() {
        assert!(
            !ranged().overlaps(&gapped()),
            "a range below every band meets none of them"
        );
        assert!(
            !ranged().overlaps(&issued()),
            "and a different prefix never overlaps at all"
        );

        let widened = PrefixNumbers::new(
            "K-",
            PrefixBound::LeadingRange {
                minimum: 1,
                maximum: 300,
            },
            true,
        );

        assert!(
            widened.overlaps(&gapped()),
            "a range reaching into a band meets it"
        );
        assert!(gapped().overlaps(&widened), "and the relation is symmetric");

        let enumerated = PrefixNumbers::new("K-", PrefixBound::Exact(vec!["4.6".to_owned()]), true);

        assert!(
            enumerated.overlaps(&ranged()),
            "an enumerated token inside a leading range is a token both would count"
        );
        assert!(
            !enumerated.overlaps(&gapped()),
            "and one outside every band is a token only the enumeration reads"
        );
        assert!(
            ranged().overlaps(&ranged()),
            "an instance trivially meets itself"
        );
    }

    /// A prefixed number shown in code font or inside a fenced block is
    /// exhibited rather than referred to, and offsets are into the source rather
    /// than into the run handed in — the two boundaries every program in this
    /// module keeps.
    ///
    /// ´claim:program:a-prefixed-number-shown-rather-than-written-is-left-alone´
    /// ´test:unit:excludes-a-displayed-prefixed-number-and-maps-offsets-back´
    #[test]
    fn excludes_a_displayed_prefixed_number_and_maps_offsets_back() {
        let displayed = "A shape such as `TN-9` is named here.\n\n```text\nTN-3 and TN-7.2\n```\n";

        assert_eq!(
            texts(&issued().scan_markdown(displayed)),
            Vec::<String>::new()
        );
        assert_eq!(
            texts(&issued().scan_markdown("Built under TN-9, per TN-3.\n")),
            ["TN-9", "TN-3"],
            "and running text is read as it always was"
        );
        assert_eq!(
            issued().scan_text(1000, "see TN-9 there"),
            [("TN-9".to_owned(), 1004)]
        );
    }

    /// The prefix-number program declares the path-count codec, which is what
    /// lets three instances sharing one program keep three independent
    /// registers: the identity is the file and the count, and the full policy
    /// key is what tells one instance's row from another's.
    ///
    /// ´claim:program:the-prefix-number-program-identifies-a-violation-by-a-count´
    /// ´test:unit:the-prefix-number-program-counts-paths´
    #[test]
    fn the_prefix_number_program_counts_paths() {
        assert_eq!(PrefixNumbers::codec(), Codec::PathCount);
        assert_eq!(PrefixNumbers::codec().field(), "path_counts");
    }
}
