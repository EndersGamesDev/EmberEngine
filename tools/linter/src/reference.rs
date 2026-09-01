// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The file-path recognizer: a finite lexicon, a small grammar, and the word *another*.
//!
//! The ruling this module serves is one sentence long: an authored document or
//! comment refers to another tracked file through its label, and writing that
//! file's path or filename instead is a violation. A label carries the target's
//! identity, its owner and its reach rules; a path carries none of those and goes
//! stale the moment the target moves. That difference is the whole of the rule,
//! and everything here exists to decide one question exactly — is this run of
//! bytes a locator for another file in this repository?
//!
//! # Neither shape nor membership alone
//!
//! Two stages answer it, and either one on its own gets it wrong. A byte grammar
//! finds a candidate; a membership test then asks whether that spelling can name
//! a file the repository actually tracks. Shape without membership turns every
//! hypothetical extension-bearing word into a file. Membership without shape
//! turns an ordinary lower-case word into a filename because some tracked entry
//! happens to share it — and this corpus really does contain such a word, which
//! is why a bare extensionless name is outside the grammar and an extensionless
//! name is recognized only where it stands in a segmented path.
//!
//! The lexicon is finite and comes from the same Git-index universe every other
//! judgment in this crate is taken over. Every full repository-relative path
//! contributes itself and each of its component-aligned suffixes; every basename
//! the bare rule admits contributes itself. So a repository-rooted spelling and a
//! local one are one policy rather than two: the full path is one member of the
//! suffix lexicon and the local path is another, and the recognizer never has to
//! infer which root an author had in mind in order to know that the author wrote
//! a locator.
//!
//! A directory is not a member merely because tracked files stand below it, and
//! a spelling that is a suffix of several tracked paths still contributes one
//! occurrence and names no target. Ambiguity about which file was meant is
//! evidence against the spelling rather than a reason to accept it, and a
//! recognizer that guessed would put a wrong path in a finding.
//!
//! # A maximal run, and the two tokens inside it
//!
//! The *run* is maximal between prose delimiters. A colon introducing a source
//! coordinate and a hash introducing a link fragment are locators *on* the
//! candidate rather than part of its file spelling, and both fall out for free
//! because neither byte is in the run alphabet. A slash or a path-unit byte
//! beside the run continues it rather than bounding it, so an external address
//! and a path under some other tree never resolve to the tracked suffix buried
//! in their tail.
//!
//! A run is not always one span, though, because the full stop is a path unit
//! and the end of an English sentence at once. So a mini-tokenizer reads each
//! run as exactly two tokens — every byte of the run belongs to one of them —
//! and membership rather than typography decides where the boundary falls:
//!
//! ```abnf
//! run       = candidate tail
//! tail      = *full-stop
//! full-stop = %s"."
//! ```
//!
//! The candidate token is the longest prefix of the run that satisfies the
//! grammar and is a member of the lexicon or a self reference; the tail is
//! whatever it leaves. A run whose remainder is not a tail is refused entire,
//! and so is a run no prefix of which is a member: there is no half-match, and a
//! name with an untracked suffix hung on it contributes nothing rather than
//! contributing the member its head happens to spell.
//!
//! Four things follow. The candidate is a prefix and never an interior span, so
//! maximality survives where it was doing its work. The tail is not measured —
//! one stop closes a sentence, three trail off, and any run of stops is a tail,
//! because admitting one and refusing three would legislate English typography
//! from inside a path grammar. Every other mark prose closes on bounds the run
//! before segmentation begins, since no such byte is in the run alphabet at all.
//! And segment alignment is a consequence rather than a rule: a tail is stops
//! alone, so no cut falls inside a component or across a separator, and a stop
//! standing inside a name is consumed by the longest member match.
//!
//! When two segmentations of one run both accept, the run is one occurrence
//! naming no target, exactly as an ambiguous suffix is. It carries the run as
//! the author wrote it, because choosing between the files it could mean would
//! put a spelling nobody wrote into a finding, and *another* removes it only
//! when every accepting segmentation is self-referential.
//!
//! # Escapes are the declared display, and *another* is lexical
//!
//! A path byte outside the literal alphabet enters prose through the reversible
//! byte-path display rather than through a lossy conversion, so a percent escape
//! is read as that display reads it: two uppercase hexadecimal digits, and never
//! a byte the grammar could have spelled outright. A non-canonical escape refuses
//! the whole candidate rather than decoding to something the display would never
//! have produced.
//!
//! The self test runs after membership and is lexical rather than a guess about
//! authorial intent. A spelling equal to the source's own path, to any
//! component-aligned suffix of it, or to its own basename is the source naming
//! itself, and the word *another* excludes it — including a module documentation
//! header naming the source it heads. The occurrence is removed even when a
//! different tracked file shares that basename, because the source-local reading
//! is the one *itself* makes available and recovering intent from the
//! surrounding sentence would make the verdict non-deterministic. A different
//! source writing the same spelling is governed in the ordinary way.
//!
//! # Participation is a carrier classification
//!
//! The recognizer does not choose where it runs. The shadow router below asks
//! the total carrier catalog for a reader, asks that reader for its regions and
//! hands only the citation-bearing roles here. An opaque but catalogued kind is
//! a classified carrier with no citations; an uncatalogued kind is a distinct
//! classification that a later policy wave can turn into its ruled finding.
//! Nothing in this module emits one.
//!
//! Structural data reaches that router as exact source bounds. A generator and
//! a registered table schema already know where their values stand, so the
//! router restates every region wholly inside such a bound as path-valued. It
//! does not infer data from a column heading or from a filename's shape, and it
//! keeps authored prose beside the bound citation-bearing.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`recognizes_a_rooted_a_relative_and_a_bare_spelling`] | reference | A repository-rooted spelling, a relative one and a bare dotted filename are one policy rather than three: each is a member of the same finite lexicon, and the relative prefix is removed before membership rather than resolved against a guessed root. Two spellings of one file are two occurrences, which is what the count codec is counting. |
//! | [`recognizes_a_hidden_filename_and_a_segmented_extensionless_one`] | reference | A hidden filename is recognized bare and segmented alike, and an extensionless name is recognized where it stands in a segmented path. Both are the bare rule doing its job: a leading dot is a filename and an interior separator makes the surrounding word one. |
//! | [`refuses_a_bare_extensionless_name_and_an_untracked_dotted_word`] | reference | Shape and membership each refuse what the other would admit. A bare extensionless name is outside the grammar however many tracked files carry it, because a corpus holding an ordinary word as a basename would otherwise grow a false family. A dotted word no tracked entry names is inside the grammar and outside the lexicon, and contributes nothing. |
//! | [`counts_an_ambiguous_suffix_once_and_names_no_target`] | reference | A spelling that is a suffix of several tracked paths still contributes one occurrence and names no target. Ambiguity about which file was meant is evidence against the spelling, and a recognizer that resolved it would put a path nobody wrote into a finding. |
//! | [`removes_a_self_path_a_self_suffix_and_a_self_basename`] | reference | The word *another* is lexical and source-local. A source naming its own path, its own suffix or its own basename is not citing another file, and the occurrence is removed even though a different tracked file carries that same basename. A different source writing those same spellings is governed in the ordinary way. |
//! | [`reads_past_a_coordinate_and_a_fragment_and_never_into_a_neighbour`] | reference | A source coordinate and a link fragment are locators on a candidate rather than part of its spelling, and neither byte is in the run alphabet, so both fall away without a rule of their own. The run itself stays maximal at its head: a longer path under some other tree and an external address are refused entire, because a candidate is a prefix of a run and never an interior span of one. Only a punctuation tail is shed, so the full stop closing a sentence leaves the name it follows intact. |
//! | [`recognizes_a_filename_that_closes_its_sentence`] | reference | A filename standing last in a sentence is recognized, because the stop that closes the sentence is the run's tail token rather than a byte of the name. A bare spelling, a segmented one and a source spelling all read alike there, and a name standing mid-sentence reads exactly as it did: the tokenizer changes where a run ends, never what a name is. |
//! | [`reads_an_ellipsis_as_a_tail_and_refuses_a_remainder_that_is_not_one`] | reference | The tail is a run of full stops and is not measured: one closes a sentence, three trail off, and both leave the name they follow intact. A remainder that is not a tail refuses the whole run instead, so a name with an untracked suffix hung on it contributes nothing rather than contributing the member its head spells. A stop standing inside a name is neither: the longest member match consumes it. |
//! | [`counts_two_segmentations_as_one_occurrence_naming_no_target`] | reference | When two segmentations of one run both yield a member, the run is one occurrence naming no target, exactly as an ambiguous suffix is. It carries what the author wrote rather than either file it could mean, because a recognizer that chose would put a spelling nobody wrote into a finding. A corpus holding only one of the two reads the same run unambiguously, which is what makes the refusal the ambiguity's doing. |
//! | [`carries_a_path_that_is_not_text_through_the_reversible_display`] | reference | A path whose bytes are not text reaches prose through the reversible display and is recognized through it, so a corner of the tree no conversion could carry is inside the policy rather than outside it. The display admits one spelling per value: a lower-case escape is not a digit, and an escape standing for a byte the grammar spells outright is not canonical. Both refuse the candidate rather than decoding to something no declaration could have written. |
//! | [`refuses_a_directory_and_reads_a_region_at_its_source_offsets`] | reference | A directory is not a member merely because tracked files stand below it, and a region the tokenization layer marked path-valued contributes nothing at all. Everything else a region can be is read alike, at offsets mapped back into the source rather than into the joined text. |
//! | [`derives_the_lexicon_from_the_tracked_universe_alone`] | reference | The lexicon is derived from the tracked universe and from nothing else: a path contributes itself and each of its component-aligned suffixes, and no proper prefix. That is what makes membership a finite question rather than a judgment about how file-like a word looks. |
//! | [`routes_every_authored_markdown_region_through_one_recognizer`] | reference | Running prose, headings, list and ordinary table text, link destinations and naked single-backtick filenames all reach the same recognizer. Their carrier syntax changes the region role and not the path judgment. |
//! | [`routes_each_language_comment_and_never_its_quoted_data`] | reference | Every comment reader the total catalog names reaches the same recognizer, while program text, strings and character literals contribute nothing. The path policy reads comments through language lexers rather than searching for leader bytes. |
//! | [`omits_display_and_complete_inline_data_but_keeps_the_prose_beside_them`] | reference | Fenced and indented blocks, double-backtick exhibits and complete inline glob, regular-expression and configuration values contribute nothing. A naked single-backtick filename, an incomplete value and authored prose surrounding the displays remain citation-bearing. |
//! | [`omits_generated_and_registered_path_values_but_keeps_authored_table_prose`] | reference | Exact bounds supplied by a generator and a registered table schema restate their contained regions as path-valued data. An authored table cell and prose beside a generated projection remain citations, proving both directions at each structural boundary. |
//! | [`classifies_an_uncatalogued_carrier_without_emitting_a_finding`] | reference | A catalogued opaque kind is a classified carrier with no citations, while a kind the total catalog has not learned receives the distinct uncatalogued classification. The classification is shadow machinery and is wired to no public finding. |
//! | [`removes_every_readme_and_nothing_that_merely_resembles_one`] | reference | The `readmes` row every section carries reaches a README at the repository root and at every depth, and reaches nothing whose basename merely resembles one — a different case, a longer name, a second suffix. The row is suffix-shaped and a section is only ever offered its own owner's share, so the identical row in every section is one uniform rule rather than thirteen that could drift. A removed source is never read, so it earns the audit line naming the rule rather than any finding. |
//! | [`an_owner_section_carries_named_exclusions_and_no_inclusion`] | reference | A section of the seventh file is an exclusion list and nothing else. Its rows name that owner's own exceptions and subtract as an orderless union, and an owner exception stands beside the uniform row rather than replacing it. There is no inclusion list to carry, because this policy assigns no parameter per cell and the program defines what is read. |
//! | [`the_include_records_are_an_optional_checked_gloss`] | reference | The `include` records are absent by default and owe nothing where they are absent. Where an owner writes them they must satisfy the checked partition of the sources this policy reads, but they supply diagnostic gloss only: they neither admit a carrier the exclusions removed nor narrow the total reach the record fixes. So the governed set is the same with a gloss, without one, and with a gloss the check finds false — an incomplete gloss, an overlapping one, and one padded with a row reaching only excluded sources or another owner's share are each a finding against the declaration and never a change to what is read. |
//! | [`governs_each_owner_share_and_names_every_readme_out`] | reference | Governance names every README out and offers a section only its own owner's share. The uniform row removes a README at the repository root and at every depth, and an excluded source is never read at all — so it earns the audit line naming the rule that excused it rather than any finding. Two sections carrying identical bytes cannot disagree, because neither is ever asked about the other's file. |
//! | [`counts_each_source_under_its_own_owner_and_omits_the_clean_ones`] | reference | A census row belongs to the owner of the source that wrote the citation and never to the owner of the file it names. One owner's document citing the other owner's file is a row in the citing owner's table, which is what makes the thirteen tables a division of the corpus rather than of the targets. A source holding no citation contributes no row at all, because the codec's identity is a source with a positive count. |
//! | [`reports_a_governed_source_whose_kind_the_catalog_has_not_learned`] | reference | A governed source whose kind the total carrier catalog has not learned is reported rather than skipped, because the reach the owner ruled cannot be met by a program that declines to ask whether a new syntax has comments. The finding names the source and its owner, and it is the only thing an unlearned kind produces: no citation is invented out of bytes no reader was willing to lex. |
//! | [`judges_every_occurrence_beyond_the_declared_maximum`] | reference | The declared maximum is a ceiling and the verdict is what stands beyond it. A source at its maximum is tolerated debt and raises nothing; a source above it raises exactly the excess, in the order the occurrences stand; and a source no row declares raises every one of them. Shrink is not judged here, because a source that has repaired a citation is making the progress the count codec exists to record. |
//!
//! The index is a generated projection and stands empty until the projection
//! writer fills it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::commentary::catalogued;
use crate::declaration::AbnfPattern;
use crate::finding::{Finding, Location};
use crate::pattern::BytePath;
use crate::selection::{
    GlossDefect, GlossSection, List as SelectionList, Rule as SelectionRule, diagnostic_gloss,
};
use crate::token::{Region, Role, regions};

/// Whether a byte may stand in a path component as prose spells it.
///
/// The alphabet is deliberately small: the bytes a filename in this repository
/// actually uses, plus the three punctuation bytes a package or a version
/// commonly carries. Everything outside it reaches prose as a percent escape.
const fn is_path_unit(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'@' | b'~')
}

/// Whether a byte may stand anywhere in a candidate run.
///
/// The separator joins components and the percent opens an escape, so both
/// continue a run even though neither is a path unit.
const fn is_run_byte(byte: u8) -> bool {
    is_path_unit(byte) || matches!(byte, b'/' | b'%')
}

/// One uppercase hexadecimal digit's value, or `None` when it is not one.
///
/// Lower case is not a digit here. The display admits one spelling per value,
/// and a recognizer accepting a second spelling would read as a member something
/// no declaration could have written.
const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Whether a filename satisfies the bare rule: a non-terminal dot, or a leading one.
///
/// The bare production reaches extension-bearing and hidden filenames and
/// nothing else. An extensionless name is recognized where it stands in a
/// segmented path and not where it stands alone, because the live corpus holds
/// an ordinary lower-case word that is also a tracked extensionless basename and
/// membership alone would make a false family out of it.
fn is_bare_filename(name: &[u8]) -> bool {
    let dotted = name
        .iter()
        .enumerate()
        .any(|(at, byte)| *byte == b'.' && at + 1 < name.len());
    let hidden = name.first() == Some(&b'.') && name.len() > 1;

    dotted || hidden
}

/// The bytes after the last separator, which is the whole path when it has none.
fn final_component(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|byte| *byte == b'/')
        .map_or(path, |offset| &path[offset + 1..])
}

/// The finite set of spellings that can name a tracked file.
///
/// Both halves are derived from the tracked universe and neither is a heuristic.
/// The suffix set answers a segmented candidate and the basename set answers a
/// bare one, and a candidate is asked of exactly one of them: the grammar has
/// already decided which kind it is.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Lexicon {
    /// Every full tracked path and every component-aligned suffix of one.
    suffixes: BTreeSet<Vec<u8>>,
    /// Every tracked basename the bare rule admits.
    basenames: BTreeSet<Vec<u8>>,
}

impl Lexicon {
    /// Derive the lexicon from the tracked universe.
    ///
    /// A path contributes itself and every suffix beginning at a component
    /// boundary. It contributes no proper prefix, which is what keeps a
    /// directory from becoming a member merely because tracked files stand
    /// below it.
    #[must_use]
    pub fn from_tracked(paths: &[BytePath]) -> Self {
        let mut suffixes = BTreeSet::new();
        let mut basenames = BTreeSet::new();

        for path in paths {
            let bytes = path.as_bytes();

            suffixes.insert(bytes.to_vec());

            for (at, _separator) in bytes
                .iter()
                .enumerate()
                .filter(|(_at, byte)| **byte == b'/')
            {
                suffixes.insert(bytes[at + 1..].to_vec());
            }

            let basename = final_component(bytes);

            if is_bare_filename(basename) {
                basenames.insert(basename.to_vec());
            }
        }

        Self {
            suffixes,
            basenames,
        }
    }

    /// Whether a candidate's spelling can name a tracked file.
    ///
    /// Comparison is case-sensitive byte equality, over the bytes the candidate
    /// decodes to rather than over the display it was written in.
    #[must_use]
    pub fn holds(&self, candidate: &Candidate) -> bool {
        if candidate.is_segmented() {
            self.suffixes.contains(candidate.path())
        } else {
            self.basenames.contains(candidate.path())
        }
    }

    /// How many spellings the lexicon holds, over both halves.
    #[must_use]
    pub fn len(&self) -> usize {
        self.suffixes.len() + self.basenames.len()
    }

    /// Whether the lexicon holds no spelling at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.suffixes.is_empty() && self.basenames.is_empty()
    }
}

/// One run of text the grammar reads as a file locator.
///
/// The spelling is what the author wrote, relative prefix included, and it is
/// what a finding quotes back. The path is what those bytes decode to, and it is
/// what membership and the self test are decided over. Keeping both is what lets
/// a finding name the spelling while the judgment runs on the bytes. Neither is
/// ever a resolved target: an ambiguous suffix and an ambiguous segmentation
/// alike carry the spelling their author wrote and name no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    spelling: String,
    offset: usize,
    path: Vec<u8>,
    segmented: bool,
    role: Role,
}

impl Candidate {
    /// The candidate as written, including any relative prefix.
    #[must_use]
    pub fn spelling(&self) -> &str {
        &self.spelling
    }

    /// Where the candidate opens, in whatever text it was read from.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The path bytes the spelling decodes to, with any relative prefix removed.
    #[must_use]
    pub fn path(&self) -> &[u8] {
        &self.path
    }

    /// Whether the candidate carries a separator, and is therefore asked of the suffix lexicon.
    #[must_use]
    pub const fn is_segmented(&self) -> bool {
        self.segmented
    }

    /// Which kind of region the candidate was read in.
    ///
    /// The role is the region's rather than the candidate's, so it is set where
    /// a region is read ([`cited_in`]) and carries the default where a caller
    /// hands the recognizer a bare run of text — which is that text read as
    /// running prose, the reading such a call is making.
    #[must_use]
    pub const fn region_kind(&self) -> Role {
        self.role
    }
}

/// Whether a byte can stand in a run's tail token.
///
/// The tail is punctuation the run alphabet happens to contain, and the full
/// stop is the only such byte. Every other mark prose closes a filename on — a
/// bracket, a quotation mark, a comma, a semicolon — bounds the run before the
/// tokenizer ever sees it.
const fn is_tail_mark(byte: u8) -> bool {
    byte == b'.'
}

/// One run's reading: the candidate token, and whether it is the source naming itself.
///
/// The self answer is settled here rather than by re-asking the candidate later,
/// because an ambiguous run's spelling is the run and not any one of the
/// segmentations that accepted it.
struct Reading {
    candidate: Candidate,
    self_reference: bool,
}

/// Every candidate the tokenizer reads out of a run of text, in the order they stand.
///
/// Membership gates every reading, so this is not shape alone: the lexicon is
/// what decides where a run's candidate token ends. What it is *not* is the word
/// *another* — a source naming itself is a reading here, and [`cited`] is the
/// same pass with that reading removed.
#[must_use]
pub fn candidates(text: &str, lexicon: &Lexicon, source: &BytePath) -> Vec<Candidate> {
    read(text, lexicon, source)
        .into_iter()
        .map(|reading| reading.candidate)
        .collect()
}

/// Tokenize every maximal run in a text, keeping the runs that yield a reading.
///
/// A run is maximal between prose delimiters, and the scan never starts one in
/// the middle of another: that is what keeps a longer path under some other tree
/// from resolving to the tracked suffix at its tail.
fn read(text: &str, lexicon: &Lexicon, source: &BytePath) -> Vec<Reading> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;

    while at < bytes.len() {
        if !is_run_byte(bytes[at]) {
            at += 1;
            continue;
        }

        let start = at;

        while at < bytes.len() && is_run_byte(bytes[at]) {
            at += 1;
        }

        if let Some(reading) = tokenize(&text[start..at], start, lexicon, source) {
            found.push(reading);
        }
    }

    found
}

/// Separate one run into a candidate token and a tail token, or refuse it.
///
/// The candidate is the longest prefix the grammar admits and the lexicon or the
/// self test accepts; the tail is what it leaves, and a tail is a run of full
/// stops or nothing. Two accepting segmentations are no segmentation: the run
/// becomes one occurrence carrying what the author wrote, and the word *another*
/// reaches it only when every reading was self-referential.
///
/// The refusal at the head is a refusal of the run rather than a short circuit.
/// A prefix considered here differs from the whole run only by trailing full
/// stops, and no shape condition can hold of such a prefix and fail of the run
/// it opens: a component that is neither empty nor dot-navigation stays so when
/// stops are appended, the bare rule's non-terminal dot is untouched by them,
/// and the escape reading never reaches them. A run the grammar refuses whole
/// therefore has no prefix the grammar admits.
fn tokenize(run: &str, offset: usize, lexicon: &Lexicon, source: &BytePath) -> Option<Reading> {
    let whole = shaped(run, offset)?;
    let bytes = run.as_bytes();
    let marks = bytes
        .iter()
        .rev()
        .take_while(|byte| is_tail_mark(**byte))
        .count();
    let mut readings = Vec::new();

    for shed in 0..=marks {
        let end = bytes.len() - shed;

        if end == 0 {
            break;
        }

        let Some(candidate) = shaped(&run[..end], offset) else {
            continue;
        };

        let self_reference = is_self_reference(source, candidate.path());

        if lexicon.holds(&candidate) || self_reference {
            readings.push(Reading {
                candidate,
                self_reference,
            });
        }
    }

    match readings.len() {
        0 => None,
        1 => readings.pop(),
        _ => Some(Reading {
            self_reference: readings.iter().all(|reading| reading.self_reference),
            candidate: whole,
        }),
    }
}

/// Read one span as a candidate the grammar admits, or refuse it.
///
/// This is shape alone, asked of a prefix the tokenizer proposes rather than of
/// a whole run. A spelling that reaches here is one that *could* name a file;
/// whether one does is the membership stage's question.
fn shaped(run: &str, offset: usize) -> Option<Candidate> {
    let path = decode(strip_relative_prefix(run))?;
    let components: Vec<&[u8]> = path.split(|byte| *byte == b'/').collect();

    if components
        .iter()
        .any(|component| component.is_empty() || *component == b"." || *component == b"..")
    {
        return None;
    }

    let segmented = components.len() > 1;

    if !segmented && !is_bare_filename(components[0]) {
        return None;
    }

    Some(Candidate {
        spelling: run.to_owned(),
        offset,
        path,
        segmented,
        role: Role::default(),
    })
}

/// Remove the navigation prefix a relative spelling opens with.
///
/// The navigation components occur here and nowhere else. After this, a dot-only
/// component is a malformed candidate rather than a step up the tree.
fn strip_relative_prefix(run: &str) -> &str {
    let mut rest = run;

    loop {
        if let Some(tail) = rest.strip_prefix("../") {
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("./") {
            rest = tail;
        } else {
            return rest;
        }
    }
}

/// Decode a spelling into the path bytes it stands for, or refuse it.
///
/// An escape is refused when it is malformed and when it is not canonical — that
/// is, when the byte it stands for is one the grammar admits outright. The
/// display has one spelling per value, and admitting a second here would make a
/// member out of a display no declaration could have written.
fn decode(name: &str) -> Option<Vec<u8>> {
    let source = name.as_bytes();
    let mut bytes = Vec::with_capacity(source.len());
    let mut at = 0;

    while at < source.len() {
        if source[at] == b'%' {
            let high = source.get(at + 1).copied().and_then(hex_value)?;
            let low = source.get(at + 2).copied().and_then(hex_value)?;
            let decoded = (high << 4) | low;

            if is_run_byte(decoded) {
                return None;
            }

            bytes.push(decoded);
            at += 3;
        } else {
            bytes.push(source[at]);
            at += 1;
        }
    }

    Some(bytes)
}

/// Whether a spelling is the source naming itself rather than another file.
///
/// The test is the source's own path, any component-aligned suffix of it, and
/// therefore its basename. It is lexical: a spelling that is self-referential
/// here is removed whatever the sentence around it says, and a different source
/// writing the same spelling is governed in the ordinary way.
#[must_use]
pub fn is_self_reference(source: &BytePath, path: &[u8]) -> bool {
    let bytes = source.as_bytes();

    bytes == path
        || bytes
            .iter()
            .enumerate()
            .filter(|(_at, byte)| **byte == b'/')
            .any(|(at, _separator)| &bytes[at + 1..] == path)
}

/// Every citation of another tracked file in a run of text, in the order they stand.
///
/// This is the whole recognizer: the tokenizer, which is shape and membership
/// together, and then *another*. The offsets are into the text as given, so a
/// caller reading a joined region maps them back itself.
#[must_use]
pub fn cited(text: &str, lexicon: &Lexicon, source: &BytePath) -> Vec<Candidate> {
    read(text, lexicon, source)
        .into_iter()
        .filter(|reading| !reading.self_reference)
        .map(|reading| reading.candidate)
        .collect()
}

/// Every citation of another tracked file in one region, at its offsets in the source.
///
/// A region whose role is not citation-bearing contributes nothing, because a
/// path a generator or a table schema presented is data the machine wrote rather
/// than a reference its author made. Every other role is read alike: the policy
/// has one verdict over prose and comments, so the recognizer has one entry
/// point for both.
#[must_use]
pub fn cited_in(region: &Region, lexicon: &Lexicon, source: &BytePath) -> Vec<Candidate> {
    if !region.role().is_citation_bearing() {
        return Vec::new();
    }

    cited(region.text(), lexicon, source)
        .into_iter()
        .map(|candidate| Candidate {
            offset: region.source_offset(candidate.offset),
            role: region.role(),
            ..candidate
        })
        .collect()
}

/// One exact source range a generator or registered schema establishes as path data.
///
/// The range is half-open and is never inferred from its contents. The future
/// policy integration will obtain these bounds from the structural owners that
/// already know them; the shadow router merely consumes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Justified: bite 2 lands this shadow input before policy integration.
struct PathValueRegion {
    start: usize,
    end: usize,
}

#[allow(dead_code)] // Justified: bite 2 lands this shadow input before policy integration.
impl PathValueRegion {
    /// Establish one half-open path-value range.
    #[must_use]
    const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Whether the whole tokenized region stands inside this structural value.
    fn contains(self, region: &Region) -> bool {
        region.stands_within(self.start, self.end)
    }
}

/// The shadow result of asking the carrier catalog where the path policy participates.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Justified: bite 2 preserves the future finding outcome in shadow.
enum CarrierClassification {
    /// The carrier has a declared reader; these are all citations that reader found.
    Catalogued(Vec<Candidate>),
    /// The carrier catalog has no row for this kind.
    Uncatalogued,
}

/// The audit line one excluded source earns, which is never a finding.
///
/// A source an exclusion row removes is never read, so it has no verdict to
/// fail. What a reader wants of it is which rule excused it, which is why every
/// exclusion row mints a name and why this line carries that name rather than
/// only reporting that some row matched.
pub fn exclusion_line(path: &impl fmt::Display, rule: &str) -> String {
    format!("file-path section: {path}: excluded by rule {rule}")
}

/// One row of an owner's exclusion list: a name minted where it stands, and its pattern.
///
/// The row mints its name, because it refers to no text the repository chose:
/// it answers *why is this source not read*. The name is what the audit line
/// reports, so a reader learns which rule excused a source rather than only
/// that some rule did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionRow {
    /// The row's name, unique within the list.
    pub(super) name: String,
    /// The paths it reaches, in the repository coordinate system.
    pub(super) pattern: AbnfPattern,
}

impl std::fmt::Display for SectionRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} : {}", self.name, self.pattern.source())
    }
}

/// One owner's section of the seventh file: an exclusion list and an optional gloss.
///
/// The section is evaluated only over its owner's share of the declared
/// partition and is never offered another owner's file
/// (´dec:rows:owner-input´), so a row written here is that owner's exception and
/// reaches no further. The rows subtract as an orderless union, and subtraction
/// is what a policy file of this shape does
/// (´dec:rows:subtract-then-partition´).
///
/// The inclusion records are absent by default and owe nothing where they are
/// absent (´dec:references:declarations´). They are not an instrument: the
/// sources this policy reads are the owner's share minus its exclusions, and
/// which kinds are read at all is fixed by this program and its code-owned
/// carrier catalog. Where an owner writes the key anyway, the rows must satisfy
/// the checked partition, but they supply diagnostic gloss only — they neither
/// admit carriers nor narrow total reach. A gloss row cannot bring a source into
/// the governed set that the exclusions removed, and cannot hold one out that
/// they did not. A decoder that still refused the key predates the ruling and
/// does not define it, which is why the key is read here and judged rather than
/// turned away at the door.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    /// The rows that remove sources from the owner's share.
    pub(super) exclude: Vec<SectionRow>,
    /// The optional gloss: a declared partition of the computed governed set.
    pub(super) include: Option<Vec<SectionRow>>,
}

impl Section {
    /// The declared gloss rows, which are none where no gloss is declared.
    ///
    /// Absent and empty are deliberately not the same declaration: an absent
    /// gloss claims nothing, while an empty one claims the governed set is empty
    /// and is judged like any other claim.
    #[cfg(test)]
    pub(super) fn gloss(&self) -> &[SectionRow] {
        self.include.as_deref().unwrap_or_default()
    }

    /// Whether the section declares a gloss at all.
    #[cfg(test)]
    pub(super) const fn glossed(&self) -> bool {
        self.include.is_some()
    }

    /// The name of the first exclusion row removing this path, if any removes it.
    ///
    /// The rows subtract as an orderless union, so *which* row is reported when
    /// several match is a reporting choice and never a semantic one
    /// (´dec:rows:subtract-then-partition´). The path is offered in the one
    /// repository coordinate system (´dec:rows:repository-coordinates´), and the
    /// caller has already restricted it to this owner's share
    /// (´dec:rows:owner-input´), so a row is never asked about a foreign file.
    #[cfg(test)]
    pub(super) fn excluded_by(&self, path: &BytePath) -> Option<&str> {
        self.exclude
            .iter()
            .find(|row| row.pattern.admits_path(path))
            .map(|row| row.name.as_str())
    }
}

/// The policy's whole parameter set: one section per owner holding the pair.
///
/// The file is the thinnest of the three, and what it does not carry is the
/// point. The requirement — that a document or comment cites another file by
/// label — is fixed by the record, and the sources it is asked of are fixed by
/// the program. What a repository may still say is which of its own sources are
/// exceptions, and that is the whole of what a section says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameters {
    /// Each owner's section, by owner.
    pub(crate) sections: BTreeMap<String, Section>,
}

/// Route one carrier's citation-bearing regions through the finite recognizer.
///
/// This is deliberately not connected to the engine or [`crate::finding`]. It
/// proves the participation relation and preserves the uncatalogued outcome for
/// the later policy wave without changing any current verdict.
#[allow(dead_code)] // Justified: bite 2 proves routing before bite 3 wires the policy.
fn classify_carrier(
    source: &BytePath,
    text: &str,
    lexicon: &Lexicon,
    path_values: &[PathValueRegion],
) -> CarrierClassification {
    let Some(reader) = catalogued(source) else {
        return CarrierClassification::Uncatalogued;
    };

    let citations = regions(reader, text)
        .into_iter()
        .flat_map(|region| {
            let region = if path_values.iter().any(|value| value.contains(&region)) {
                region.in_role(Role::PathValue)
            } else {
                region
            };

            cited_in(&region, lexicon, source)
        })
        .collect();

    CarrierClassification::Catalogued(citations)
}

/// One source of an owner's share the policy reads.
///
/// The pair is the whole of what governance decides here, because this policy
/// assigns no parameter per cell: a source is read or it is excused, and there is
/// no third state and no row to carry beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Governed {
    /// The source, in the one repository coordinate system.
    pub path: BytePath,
    /// The owner whose section governs it.
    pub owner: String,
}

/// One path a named rule removes from an owner's section.
///
/// An excluded source is never read, so it has no verdict to fail and earns the
/// audit line naming its rule instead. That is why every exclusion row mints a
/// name: a reader learns which rule excused a source rather than only that some
/// rule did.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Exclusion {
    /// The excluded path, in reversible display form.
    pub path: String,
    /// The owner whose section excludes it.
    pub owner: String,
    /// The rule that removed it.
    pub name: String,
}

/// The plan-time result of the file-path policy's diagnostic-gloss selection.
#[derive(Debug, Clone, Default)]
pub struct SelectionPlan {
    governed: Vec<Governed>,
    findings: Vec<Finding>,
    exclusions: Vec<Exclusion>,
}

/// Compile the file-path policy's typed exclusion-and-gloss selection.
#[must_use]
pub fn selection_plan(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> SelectionPlan {
    let sections: Vec<_> = parameters
        .sections
        .iter()
        .map(|(owner, section)| {
            let exclude = section
                .exclude
                .iter()
                .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), ()))
                .collect();
            let gloss = section.include.as_ref().map(|rows| {
                rows.iter()
                    .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), ()))
                    .collect()
            });
            GlossSection::new(owner.clone(), exclude, gloss)
        })
        .collect();
    let selected = diagnostic_gloss(attribution, &sections);
    let mut governed: Vec<_> = selected
        .governed
        .into_iter()
        .map(|entry| Governed {
            path: entry.path,
            owner: entry.owner,
        })
        .collect();
    governed.sort_by(|left, right| left.path.cmp(&right.path));

    let mut seen = BTreeSet::new();
    let mut exclusions: Vec<_> = selected
        .excluded
        .into_iter()
        .filter(|entry| seen.insert((entry.owner.clone(), entry.path.clone())))
        .map(|entry| Exclusion {
            path: entry.path.display(),
            owner: entry.owner,
            name: entry.name,
        })
        .collect();
    exclusions.sort();

    let findings = selected
        .defects
        .into_iter()
        .filter_map(|defect| match defect {
            GlossDefect::Uncovered { path, owner } => Some(Finding::FilePathGlossUncovered {
                path: path.display(),
                owner,
            }),
            GlossDefect::MultiplyIncluded {
                path,
                owner,
                matches,
            } => Some(Finding::FilePathGlossMultiplyIncluded {
                path: path.display(),
                owner,
                count: matches.len(),
                matches,
            }),
            GlossDefect::IdleRow {
                owner,
                list: SelectionList::Include,
                name,
            } => Some(Finding::FilePathIdleGlossRow { owner, name }),
            GlossDefect::IdleRow {
                list: SelectionList::Exclude,
                ..
            } => None,
        })
        .collect();

    SelectionPlan {
        governed,
        findings,
        exclusions,
    }
}

impl SelectionPlan {
    /// Sources selected by exclusion alone.
    #[must_use]
    pub fn governed(&self) -> &[Governed] {
        &self.governed
    }

    /// Stable policy findings mapped from the optional gloss judgment.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Named first-match exclusions retained for audit explanations.
    #[must_use]
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }
}

/// Every path one owner's rows remove from its own share.
#[must_use]
#[cfg(test)]
pub fn exclusions(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
    owner: &str,
) -> Vec<Exclusion> {
    selection_plan(parameters, attribution)
        .exclusions
        .into_iter()
        .filter(|excluded| excluded.owner == owner)
        .collect()
}

#[cfg(test)]
pub fn retiring_exclusions(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
    owner: &str,
) -> Vec<Exclusion> {
    let Some(section) = parameters.sections.get(owner) else {
        return Vec::new();
    };

    let mut excluded = Vec::new();

    for (path, accounted) in attribution {
        if *accounted != owner {
            continue;
        }

        if let Some(name) = section.excluded_by(path) {
            excluded.push(Exclusion {
                path: path.display(),
                owner: owner.to_owned(),
                name: name.to_owned(),
            });
        }
    }

    excluded.sort();
    excluded
}

/// Which sources of the declared partition this policy reads.
///
/// A section is offered only its own owner's share (´dec:rows:owner-input´), and
/// its rows subtract as an orderless union before anything else is asked of them
/// (´dec:rows:subtract-then-partition´). What survives is read; nothing else is
/// opened at all. The result is ordered by path so that a census over it, and
/// every finding the census raises, arrive in the order a reader reads the tree.
#[must_use]
#[cfg(test)]
pub fn retiring_govern(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> Vec<Governed> {
    let mut governed = Vec::new();

    for (owner, section) in &parameters.sections {
        for (path, accounted) in attribution {
            if *accounted != owner.as_str() || section.excluded_by(path).is_some() {
                continue;
            }

            governed.push(Governed {
                path: (*path).clone(),
                owner: (*accounted).to_owned(),
            });
        }
    }

    governed.sort_by(|left, right| left.path.cmp(&right.path));
    governed
}

/// Select each owner's sources through exclusion alone.
#[must_use]
#[cfg(test)]
pub fn govern(parameters: &Parameters, attribution: &BTreeMap<&BytePath, &str>) -> Vec<Governed> {
    selection_plan(parameters, attribution).governed
}

/// Judge every declared gloss against the governed set it claims to partition.
///
/// The `include` records are absent by default; when present they must satisfy
/// the checked partition, but they supply diagnostic gloss only — they neither
/// admit carriers nor narrow total reach (´dec:references:declarations´). So this
/// pass reads [`govern`]'s answer rather than helping to form it, and every
/// verdict it reaches leaves the governed set exactly as it found it.
///
/// The judgment is the ordinary one (´dec:rows:subtract-then-partition´), taken
/// over the governed set and nothing wider: a governed source no gloss row names
/// breaks completeness, one that two rows name breaks disjointness, and a row
/// reaching nothing in the governed set is idle — which is what a row naming only
/// excluded sources, or only sources of another owner's share, comes to. A
/// section declaring no gloss is asked nothing at all, and an empty gloss is a
/// claim that the governed set is empty rather than the absence of a claim.
#[must_use]
#[cfg(test)]
pub fn retiring_gloss(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let governed = govern(parameters, attribution);

    for (owner, section) in &parameters.sections {
        if !section.glossed() {
            continue;
        }

        let mut reached: BTreeMap<&str, bool> = section
            .gloss()
            .iter()
            .map(|row| (row.name.as_str(), false))
            .collect();

        for entry in &governed {
            if entry.owner != owner.as_str() {
                continue;
            }

            let matched: Vec<&SectionRow> = section
                .gloss()
                .iter()
                .filter(|row| row.pattern.admits_path(&entry.path))
                .collect();

            for row in &matched {
                reached.insert(row.name.as_str(), true);
            }

            match matched.as_slice() {
                [_] => {}
                [] => findings.push(Finding::FilePathGlossUncovered {
                    path: entry.path.display(),
                    owner: owner.clone(),
                }),
                rows => {
                    let mut names: Vec<String> = rows.iter().map(ToString::to_string).collect();
                    names.sort();

                    findings.push(Finding::FilePathGlossMultiplyIncluded {
                        path: entry.path.display(),
                        owner: owner.clone(),
                        count: rows.len(),
                        matches: names,
                    });
                }
            }
        }

        for (name, found) in reached {
            if !found {
                findings.push(Finding::FilePathIdleGlossRow {
                    owner: owner.clone(),
                    name: name.to_owned(),
                });
            }
        }
    }

    findings
}

/// Judge the optional inclusion records without moving the governed set.
#[must_use]
#[cfg(test)]
pub fn gloss(parameters: &Parameters, attribution: &BTreeMap<&BytePath, &str>) -> Vec<Finding> {
    selection_plan(parameters, attribution).findings
}

/// One citation a source makes, with the place it stands.
///
/// The location is taken during the census, where the source text is at hand,
/// rather than kept as an offset a later pass would have to re-read the file to
/// resolve. That keeps the whole corpus out of memory while still letting a
/// finding send a reader to the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The candidate as its author wrote it.
    pub candidate: Candidate,
    /// Where it stands in the source.
    pub location: Location,
}

/// What one governed source holds, and whether its kind had a reader at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Censused<'a> {
    /// The source, in the one repository coordinate system.
    pub path: &'a BytePath,
    /// The owner whose section governs it.
    pub owner: &'a str,
    /// Every citation of another tracked file the source's author made.
    pub citations: Vec<Citation>,
    /// Whether the total carrier catalog has no row for this source's kind.
    pub uncatalogued: bool,
}

/// The structural bounds a generator or a registered table schema establishes in one source.
///
/// The two cases the reach rule names (´rule:references:total-reach´) are the
/// folder-projection region and the burn register's file column, and neither
/// reaches a governed source at this boundary. A projection stands only in a
/// folder README, and every README is removed by the named row before a source
/// is opened. A register's file column is written by the generator as a
/// double-backtick exhibit, so the tokenization layer has already classified
/// every one of its cells as display rather than as a region this policy would
/// read.
///
/// So the honest answer today is *none*, measured rather than assumed: the census
/// over the whole tracked corpus reads the register documents and finds their
/// tables contribute nothing. [`PathValueRegion`] stays the shape this returns
/// when a structural owner first establishes a bound outside a README and outside
/// an exhibit, and the router already consumes it.
const fn path_values(_source: &BytePath, _text: &str) -> Vec<PathValueRegion> {
    Vec::new()
}

/// Census every governed source through the finite recognizer.
///
/// This is the recognizer census over the resolved regions and nothing beside
/// it: the router asks the total carrier catalog for a reader, asks that reader
/// for its regions, and hands the citation-bearing roles to one recognizer. A
/// source whose bytes are not text carries no region any reader would return, and
/// a catalogued opaque kind returns none by classification, so both contribute
/// nothing without a rule of their own.
#[must_use]
pub fn census<'a>(root: &Path, lexicon: &Lexicon, governed: &'a [Governed]) -> Vec<Censused<'a>> {
    governed
        .iter()
        .map(|entry| {
            let Some(full) = under(root, &entry.path) else {
                return Censused {
                    path: &entry.path,
                    owner: &entry.owner,
                    citations: Vec::new(),
                    uncatalogued: false,
                };
            };

            let text = std::fs::read_to_string(&full).unwrap_or_default();
            let display = entry.path.display();

            match classify_carrier(
                &entry.path,
                &text,
                lexicon,
                &path_values(&entry.path, &text),
            ) {
                CarrierClassification::Catalogued(found) => Censused {
                    path: &entry.path,
                    owner: &entry.owner,
                    citations: found
                        .into_iter()
                        .map(|candidate| Citation {
                            location: Location::new(display.clone(), &text, candidate.offset()),
                            candidate,
                        })
                        .collect(),
                    uncatalogued: false,
                },
                CarrierClassification::Uncatalogued => Censused {
                    path: &entry.path,
                    owner: &entry.owner,
                    citations: Vec::new(),
                    uncatalogued: true,
                },
            }
        })
        .collect()
}

/// How many citations each governed source holds, for the sources holding any.
///
/// This is the observation the declared path-count tables are compared against.
/// A source holding none contributes no row, because the codec's identity is a
/// source with a positive count and a row at zero is the stale state the writer
/// removes rather than a debt anything tolerates.
#[must_use]
pub fn counted<'a>(censused: &[Censused<'a>]) -> BTreeMap<&'a BytePath, u64> {
    censused
        .iter()
        .filter(|source| !source.citations.is_empty())
        .map(|source| (source.path, source.citations.len() as u64))
        .collect()
}

/// The filesystem path a tracked entry stands at, without decoding its bytes.
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn under(root: &Path, path: &BytePath) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    Some(root.join(std::ffi::OsStr::from_bytes(path.as_bytes())))
}

/// The filesystem path a tracked entry stands at, where the platform names files as text.
#[cfg(not(unix))]
fn under(root: &Path, path: &BytePath) -> Option<PathBuf> {
    std::str::from_utf8(path.as_bytes())
        .ok()
        .map(|text| root.join(text))
}

/// Report every governed source whose kind the total carrier catalog has not learned.
///
/// The catalog is total over tracked file kinds, and this is the finding that
/// keeps it so. A kind carrying no comments is catalogued as opaque rather than
/// omitted, so silence here means *classified as having no citations* and never
/// *not asked*. The reach the owner ruled cannot be met by a program that
/// declines to ask whether a new syntax has comments.
#[must_use]
pub fn carriers(censused: &[Censused<'_>]) -> Vec<Finding> {
    censused
        .iter()
        .filter(|source| source.uncatalogued)
        .map(|source| Finding::FilePathCarrier {
            path: source.path.display(),
            owner: source.owner.to_owned(),
        })
        .collect()
}

/// Judge each censused source against the maximum its declared row allows.
///
/// The codec is the existing per-source path count, and the ratchet it carries is
/// the one every count family already has: a source at or below its maximum is
/// tolerated debt and raises nothing, and every occurrence beyond that maximum is
/// a finding. Occurrences are taken in the order they stand, so the ones reported
/// are the ones past the ceiling rather than an arbitrary selection of the same
/// number — the same positional attribution the burn growth finding makes.
///
/// Shrink and staleness are not judged here. They are the writer's business
/// through the lowering door, and a source that has repaired one citation is
/// making the progress this codec exists to record rather than failing.
#[must_use]
pub fn conform(censused: &[Censused<'_>], tolerated: &BTreeMap<&BytePath, u64>) -> Vec<Finding> {
    let mut findings = Vec::new();

    for source in censused {
        let maximum = tolerated.get(source.path).copied().unwrap_or_default();

        for citation in source
            .citations
            .iter()
            .skip(usize::try_from(maximum).unwrap_or(usize::MAX))
        {
            findings.push(Finding::FilePathCitation {
                path: source.path.display(),
                owner: source.owner.to_owned(),
                spelling: citation.candidate.spelling().to_owned(),
                region_kind: citation.candidate.region_kind().as_str(),
                location: citation.location.clone(),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        Candidate, CarrierClassification, Exclusion, Lexicon, Parameters, PathValueRegion, Section,
        SectionRow, candidates, carriers, census, cited, cited_in, classify_carrier, conform,
        counted, exclusion_line, exclusions, gloss, govern, is_self_reference,
    };
    use crate::declaration::AbnfPattern;
    use crate::finding::Finding;
    use crate::pattern::BytePath;
    use crate::token::{Role, markdown_regions};

    /// A lexicon over an invented corpus, failing the test when a path will not decode.
    fn lexicon(paths: &[&str]) -> Lexicon {
        let tracked: Vec<BytePath> = paths
            .iter()
            .map(|path| BytePath::decode(path).expect("a decodable path"))
            .collect();

        Lexicon::from_tracked(&tracked)
    }

    /// The invented orchard every test here reads. Nothing in it is a path this
    /// repository holds, so the recognizer is exercised against a corpus rather
    /// than against the tree it happens to be running in.
    fn orchard() -> Lexicon {
        lexicon(&[
            "orchard/plum/basket.md",
            "orchard/quince/basket.md",
            "orchard/plum/harvest",
            "orchard/plum/gate.rs",
            "orchard/plum.old/gate.rs",
            "orchard/.thicket",
            "hedgerow.md",
        ])
    }

    /// A section as every one of the thirteen is written: the uniform row, and
    /// no inclusion list to carry. The bytes are the declaration's own, so the
    /// test reads what a repository writes rather than a shape invented for it.
    fn readmes_section() -> Section {
        Section {
            exclude: vec![SectionRow {
                name: String::from("readmes"),
                pattern: AbnfPattern::parse("[ *VCHAR \"/\" ] %s\"README.md\"")
                    .expect("a well-formed pattern"),
            }],
            include: None,
        }
    }

    /// A source path, failing the test when it will not decode.
    fn source(path: &str) -> BytePath {
        BytePath::decode(path).expect("a decodable path")
    }

    /// The spellings of a recognizer's answer, in the order they stand.
    fn spellings(found: &[Candidate]) -> Vec<&str> {
        found.iter().map(Candidate::spelling).collect()
    }

    // The citations in a catalogued carrier, failing when the fixture's kind
    // was deliberately left uncatalogued instead.
    fn classified(classification: CarrierClassification) -> Vec<Candidate> {
        let CarrierClassification::Catalogued(citations) = classification else {
            panic!("the invented carrier kind is catalogued");
        };

        citations
    }

    /// A repository-rooted spelling, a relative one and a bare dotted filename
    /// are one policy rather than three: each is a member of the same finite
    /// lexicon, and the relative prefix is removed before membership rather than
    /// resolved against a guessed root. Two spellings of one file are two
    /// occurrences, which is what the count codec is counting.
    ///
    /// ´claim:reference:a-rooted-a-relative-and-a-bare-spelling-are-one-policy´
    /// ´test:unit:recognizes-a-rooted-a-relative-and-a-bare-spelling´
    #[test]
    fn recognizes_a_rooted_a_relative_and_a_bare_spelling() {
        let found = cited(
            "See orchard/plum/gate.rs, ./orchard/plum/gate.rs and hedgerow.md here.",
            &orchard(),
            &source("orchard/notes.md"),
        );

        assert_eq!(
            spellings(&found),
            [
                "orchard/plum/gate.rs",
                "./orchard/plum/gate.rs",
                "hedgerow.md"
            ]
        );
        assert_eq!(
            found[1].path(),
            b"orchard/plum/gate.rs",
            "the relative prefix is removed before membership"
        );
        assert!(found[0].is_segmented() && !found[2].is_segmented());
    }

    /// A hidden filename is recognized bare and segmented alike, and an
    /// extensionless name is recognized where it stands in a segmented path.
    /// Both are the bare rule doing its job: a leading dot is a filename and an
    /// interior separator makes the surrounding word one.
    ///
    /// ´claim:reference:a-hidden-name-and-a-segmented-extensionless-one-are-members´
    /// ´test:unit:recognizes-a-hidden-filename-and-a-segmented-extensionless-one´
    #[test]
    fn recognizes_a_hidden_filename_and_a_segmented_extensionless_one() {
        let found = cited(
            "The orchard/.thicket list, the .thicket list, and orchard/plum/harvest beside them.",
            &orchard(),
            &source("hedgerow.md"),
        );

        assert_eq!(
            spellings(&found),
            ["orchard/.thicket", ".thicket", "orchard/plum/harvest"]
        );
        assert_eq!(
            spellings(&cited(
                "plum/harvest is a suffix too.",
                &orchard(),
                &source("hedgerow.md")
            )),
            ["plum/harvest"],
            "a component-aligned suffix is a member exactly as the full path is"
        );
    }

    /// Shape and membership each refuse what the other would admit. A bare
    /// extensionless name is outside the grammar however many tracked files
    /// carry it, because a corpus holding an ordinary word as a basename would
    /// otherwise grow a false family. A dotted word no tracked entry names is
    /// inside the grammar and outside the lexicon, and contributes nothing.
    ///
    /// ´claim:reference:shape-without-membership-and-membership-without-shape-both-refuse´
    /// ´test:unit:refuses-a-bare-extensionless-name-and-an-untracked-dotted-word´
    #[test]
    fn refuses_a_bare_extensionless_name_and_an_untracked_dotted_word() {
        let here = source("hedgerow.md");

        assert!(
            cited("the harvest was good", &lexicon(&["harvest"]), &here).is_empty(),
            "a bare extensionless name is outside the grammar however many tracked files carry it"
        );

        let hypothetical = "a quince.rs file, e.g. the one nobody wrote";

        assert!(
            cited(hypothetical, &orchard(), &here).is_empty(),
            "a dotted word no tracked entry names is outside the lexicon and contributes nothing"
        );
        assert_eq!(
            spellings(&cited(hypothetical, &lexicon(&["quince.rs"]), &here)),
            ["quince.rs"],
            "the same word is inside the grammar, so membership alone was refusing it"
        );
    }

    /// A spelling that is a suffix of several tracked paths still contributes
    /// one occurrence and names no target. Ambiguity about which file was meant
    /// is evidence against the spelling, and a recognizer that resolved it would
    /// put a path nobody wrote into a finding.
    ///
    /// ´claim:reference:an-ambiguous-suffix-is-one-occurrence-naming-no-target´
    /// ´test:unit:counts-an-ambiguous-suffix-once-and-names-no-target´
    #[test]
    fn counts_an_ambiguous_suffix_once_and_names_no_target() {
        let found = cited("the basket.md table", &orchard(), &source("hedgerow.md"));

        assert_eq!(spellings(&found), ["basket.md"]);
        assert_eq!(
            found[0].path(),
            b"basket.md",
            "the candidate carries the spelling and never a resolved target"
        );
    }

    /// The word *another* is lexical and source-local. A source naming its own
    /// path, its own suffix or its own basename is not citing another file, and
    /// the occurrence is removed even though a different tracked file carries
    /// that same basename. A different source writing those same spellings is
    /// governed in the ordinary way.
    ///
    /// ´claim:reference:a-source-naming-itself-is-not-citing-another-file´
    /// ´test:unit:removes-a-self-path-a-self-suffix-and-a-self-basename´
    #[test]
    fn removes_a_self_path_a_self_suffix_and_a_self_basename() {
        let text = "This basket.md documents orchard/plum/basket.md, plum/basket.md and orchard/quince/basket.md beside it.";

        assert_eq!(
            spellings(&cited(text, &orchard(), &source("orchard/plum/basket.md"))),
            ["orchard/quince/basket.md"],
            "every self spelling goes and the other file stays"
        );
        assert_eq!(
            spellings(&cited(text, &orchard(), &source("hedgerow.md"))).len(),
            4,
            "another source writing the same sentence cites all four"
        );
        assert!(
            is_self_reference(&source("orchard/plum/gate.rs"), b"gate.rs"),
            "a module header naming the source it heads is self-reference"
        );
        assert_eq!(
            spellings(&candidates(
                text,
                &orchard(),
                &source("orchard/plum/basket.md")
            ))
            .len(),
            4,
            "the tokenizer reads all four and the word *another* is what removes three"
        );
        assert_eq!(
            spellings(&candidates(
                "documented in gate.rs.",
                &lexicon(&["hedgerow.md"]),
                &source("orchard/plum/gate.rs")
            )),
            ["gate.rs"],
            "a self reference is a segmentation the tokenizer accepts even where the lexicon does not"
        );
    }

    /// A source coordinate and a link fragment are locators on a candidate
    /// rather than part of its spelling, and neither byte is in the run
    /// alphabet, so both fall away without a rule of their own. The run itself
    /// stays maximal at its head: a longer path under some other tree and an
    /// external address are refused entire, because a candidate is a prefix of a
    /// run and never an interior span of one. Only a punctuation tail is shed,
    /// so the full stop closing a sentence leaves the name it follows intact.
    ///
    /// ´claim:reference:a-run-is-maximal-at-its-head-and-its-locators-are-not-the-spelling´
    /// ´test:unit:reads-past-a-coordinate-and-a-fragment-and-never-into-a-neighbour´
    #[test]
    fn reads_past_a_coordinate_and_a_fragment_and_never_into_a_neighbour() {
        assert_eq!(
            spellings(&cited(
                "at orchard/plum/gate.rs:120:4 and orchard/plum/basket.md#the-harvest",
                &orchard(),
                &source("hedgerow.md")
            )),
            ["orchard/plum/gate.rs", "orchard/plum/basket.md"]
        );
        assert!(
            cited(
                "under notes/orchard/plum/gate.rs elsewhere",
                &orchard(),
                &source("hedgerow.md")
            )
            .is_empty(),
            "a longer run is refused entire rather than searched for a member inside it"
        );
        assert!(
            cited(
                "the address https://example.org/plum/basket.md",
                &orchard(),
                &source("hedgerow.md")
            )
            .is_empty(),
            "an external address is a longer run and is refused for the same reason"
        );
        assert_eq!(
            spellings(&cited(
                "the sentence ends with hedgerow.md.",
                &orchard(),
                &source("orchard/notes.md")
            )),
            ["hedgerow.md"],
            "the stop closing the sentence is the run's tail token rather than a byte of the name"
        );
    }

    /// A filename standing last in a sentence is recognized, because the stop
    /// that closes the sentence is the run's tail token rather than a byte of
    /// the name. A bare spelling, a segmented one and a source spelling all read
    /// alike there, and a name standing mid-sentence reads exactly as it did:
    /// the tokenizer changes where a run ends, never what a name is.
    ///
    /// ´claim:reference:a-filename-closing-its-sentence-is-recognized´
    /// ´test:unit:recognizes-a-filename-that-closes-its-sentence´
    #[test]
    fn recognizes_a_filename_that_closes_its_sentence() {
        let here = source("orchard/notes.md");

        assert_eq!(
            spellings(&cited(
                "The shape is documented in hedgerow.md.",
                &orchard(),
                &here
            )),
            ["hedgerow.md"],
            "a bare spelling closing a sentence is the name and a one-stop tail"
        );
        assert_eq!(
            spellings(&cited(
                "Take the lift from orchard/plum/basket.md.",
                &orchard(),
                &here
            )),
            ["orchard/plum/basket.md"],
            "a segmented spelling closes a sentence the same way"
        );
        assert_eq!(
            spellings(&cited("Survey orchard/plum/gate.rs.", &orchard(), &here)),
            ["orchard/plum/gate.rs"],
            "and so does a source spelling"
        );
        assert_eq!(
            spellings(&cited("See hedgerow.md here.", &orchard(), &here)),
            ["hedgerow.md"],
            "a name standing mid-sentence is untouched by any of this"
        );
    }

    /// The tail is a run of full stops and is not measured: one closes a
    /// sentence, three trail off, and both leave the name they follow intact. A
    /// remainder that is not a tail refuses the whole run instead, so a name
    /// with an untracked suffix hung on it contributes nothing rather than
    /// contributing the member its head spells. A stop standing inside a name is
    /// neither: the longest member match consumes it.
    ///
    /// ´claim:reference:a-tail-is-a-run-of-stops-and-any-other-remainder-refuses´
    /// ´test:unit:reads-an-ellipsis-as-a-tail-and-refuses-a-remainder-that-is-not-one´
    #[test]
    fn reads_an_ellipsis_as_a_tail_and_refuses_a_remainder_that_is_not_one() {
        let here = source("orchard/notes.md");

        assert_eq!(
            spellings(&cited(
                "The gate stands open in orchard/plum/gate.rs...",
                &orchard(),
                &here
            )),
            ["orchard/plum/gate.rs"],
            "an ellipsis is a tail: the grammar counts stops no more than it counts sentences"
        );
        assert!(
            cited(
                "the file hedgerow.md.backup nobody wrote",
                &orchard(),
                &here
            )
            .is_empty(),
            "a remainder that is not a tail refuses the run rather than half-matching its head"
        );
        assert_eq!(
            spellings(&cited(
                "built by orchard/plum.old/gate.rs here",
                &orchard(),
                &here
            )),
            ["orchard/plum.old/gate.rs"],
            "a stop inside a name is consumed by the longest member match"
        );
        assert_eq!(
            spellings(&cited(
                "built by orchard/plum.old/gate.rs.",
                &orchard(),
                &here
            )),
            ["orchard/plum.old/gate.rs"],
            "the interior stop is consumed and the final one is the tail, in one run"
        );
    }

    /// When two segmentations of one run both yield a member, the run is one
    /// occurrence naming no target, exactly as an ambiguous suffix is. It
    /// carries what the author wrote rather than either file it could mean,
    /// because a recognizer that chose would put a spelling nobody wrote into a
    /// finding. A corpus holding only one of the two reads the same run
    /// unambiguously, which is what makes the refusal the ambiguity's doing.
    ///
    /// ´claim:reference:two-segmentations-are-one-occurrence-naming-no-target´
    /// ´test:unit:counts-two-segmentations-as-one-occurrence-naming-no-target´
    #[test]
    fn counts_two_segmentations_as_one_occurrence_naming_no_target() {
        let here = source("hedgerow.md");
        let sentence = "kept in orchard/quince/relic.md..";
        let both = lexicon(&["orchard/quince/relic.md", "orchard/quince/relic.md."]);
        let found = cited(sentence, &both, &here);

        assert_eq!(
            spellings(&found),
            ["orchard/quince/relic.md.."],
            "two accepting segmentations are one occurrence carrying the run entire"
        );
        assert!(
            !both.holds(&found[0]),
            "and that occurrence names no target: its own spelling is no member"
        );
        assert_eq!(
            spellings(&cited(
                sentence,
                &lexicon(&["orchard/quince/relic.md"]),
                &here
            )),
            ["orchard/quince/relic.md"],
            "one member leaves one segmentation, and the two stops are the tail"
        );
    }

    /// A path whose bytes are not text reaches prose through the reversible
    /// display and is recognized through it, so a corner of the tree no
    /// conversion could carry is inside the policy rather than outside it. The
    /// display admits one spelling per value: a lower-case escape is not a
    /// digit, and an escape standing for a byte the grammar spells outright is
    /// not canonical. Both refuse the candidate rather than decoding to
    /// something no declaration could have written.
    ///
    /// ´claim:reference:a-path-that-is-not-text-is-recognized-through-its-display´
    /// ´test:unit:carries-a-path-that-is-not-text-through-the-reversible-display´
    #[test]
    fn carries_a_path_that_is_not_text_through_the_reversible_display() {
        let opaque = BytePath::from_bytes(b"orchard/pl\xffm.md".to_vec()).expect("a relative path");
        let held = Lexicon::from_tracked(std::slice::from_ref(&opaque));
        let here = source("hedgerow.md");

        assert_eq!(
            spellings(&cited("see orchard/pl%FFm.md there", &held, &here)),
            ["orchard/pl%FFm.md"]
        );
        assert!(
            cited("see orchard/pl%ffm.md there", &held, &here).is_empty(),
            "a lower-case escape is not a hexadecimal digit of this display"
        );
        assert!(
            cited("see orchard%2Fplum/basket.md there", &orchard(), &here).is_empty(),
            "an escape standing for a byte the grammar spells outright is not canonical, so the run never decodes to the member it imitates"
        );
        assert!(
            cited("see orchard/pl%FFm.md% there", &held, &here).is_empty(),
            "a percent that opens no escape refuses the run it stands in rather than falling back to a shorter reading"
        );
    }

    /// A directory is not a member merely because tracked files stand below it,
    /// and a region the tokenization layer marked path-valued contributes
    /// nothing at all. Everything else a region can be is read alike, at offsets
    /// mapped back into the source rather than into the joined text.
    ///
    /// ´claim:reference:a-directory-is-no-member-and-a-path-valued-region-contributes-nothing´
    /// ´test:unit:refuses-a-directory-and-reads-a-region-at-its-source-offsets´
    #[test]
    fn refuses_a_directory_and_reads_a_region_at_its_source_offsets() {
        let here = source("hedgerow.md");

        assert!(
            cited("under the orchard/plum tree", &orchard(), &here).is_empty(),
            "a directory is not a member of the lexicon"
        );

        let document = "A first line\nnaming orchard/plum/gate.rs here.\n";
        let region = markdown_regions(document)
            .pop()
            .expect("the paragraph is a region");
        let found = cited_in(&region, &orchard(), &here);

        assert_eq!(spellings(&found), ["orchard/plum/gate.rs"]);

        let at = found[0].offset();

        assert_eq!(
            &document[at..at + "orchard/plum/gate.rs".len()],
            "orchard/plum/gate.rs",
            "the offset lands on the spelling in the source rather than in the joined text"
        );
        assert!(
            cited_in(&region.in_role(Role::PathValue), &orchard(), &here).is_empty(),
            "a value a machine or a schema presented is not a citation anybody made"
        );
    }

    /// The lexicon is derived from the tracked universe and from nothing else: a
    /// path contributes itself and each of its component-aligned suffixes, and
    /// no proper prefix. That is what makes membership a finite question rather
    /// than a judgment about how file-like a word looks.
    ///
    /// ´claim:reference:the-lexicon-is-derived-from-the-tracked-universe-alone´
    /// ´test:unit:derives-the-lexicon-from-the-tracked-universe-alone´
    #[test]
    fn derives_the_lexicon_from_the_tracked_universe_alone() {
        let held = lexicon(&["orchard/plum/basket.md"]);

        assert!(!held.is_empty());
        assert_eq!(
            held.len(),
            4,
            "three suffixes and the one basename the bare rule admits"
        );
        assert!(
            Lexicon::default().is_empty(),
            "an empty universe holds no spelling"
        );
        assert!(
            cited(
                "orchard/plum/basket.md and basket.md",
                &held,
                &source("hedgerow.md")
            )
            .len()
                == 2,
            "the full path and its basename are both members"
        );
    }

    /// Running prose, headings, list and ordinary table text, link destinations
    /// and naked single-backtick filenames all reach the same recognizer. Their
    /// carrier syntax changes the region role and not the path judgment.
    ///
    /// ´claim:reference:every-authored-markdown-region-reaches-one-recognizer´
    /// ´test:unit:routes-every-authored-markdown-region-through-one-recognizer´
    #[test]
    fn routes_every_authored_markdown_region_through_one_recognizer() {
        let document = concat!(
            "# orchard/plum/gate.rs heading\n\n",
            "- orchard/plum/basket.md list\n\n",
            "| Note |\n",
            "| --- |\n",
            "| orchard/plum/gate.rs table |\n\n",
            "[gate](orchard/plum/gate.rs)\n\n",
            "`orchard/plum/gate.rs`\n",
        );
        let found = classified(classify_carrier(
            &source("orchard/notes.md"),
            document,
            &orchard(),
            &[],
        ));

        assert_eq!(
            spellings(&found),
            [
                "orchard/plum/gate.rs",
                "orchard/plum/basket.md",
                "orchard/plum/gate.rs",
                "orchard/plum/gate.rs",
                "orchard/plum/gate.rs",
            ],
            "all authored Markdown roles share the recognizer"
        );
    }

    /// Every comment reader the total catalog names reaches the same
    /// recognizer, while program text, strings and character literals contribute
    /// nothing. The path policy reads comments through language lexers rather
    /// than searching for leader bytes.
    ///
    /// ´claim:reference:every-language-comment-and-no-quoted-data-reaches-the-recognizer´
    /// ´test:unit:routes-each-language-comment-and-never-its-quoted-data´
    #[test]
    fn routes_each_language_comment_and_never_its_quoted_data() {
        let fixtures = [
            (
                "orchard/notes.rs",
                "const PATH: &str = \"orchard/plum/basket.md\";\nconst MARK: char = '/';\n// orchard/plum/gate.rs here\n",
            ),
            (
                "orchard/notes.sh",
                "printf '%s' 'orchard/plum/basket.md' # orchard/plum/gate.rs here\n",
            ),
            (
                "orchard/notes.sql",
                "SELECT 'orchard/plum/basket.md'; -- orchard/plum/gate.rs here\n",
            ),
            (
                "orchard/notes.html",
                "<p>orchard/plum/basket.md</p><!-- orchard/plum/gate.rs here -->\n",
            ),
        ];

        for (path, text) in fixtures {
            let found = classified(classify_carrier(&source(path), text, &orchard(), &[]));

            assert_eq!(
                spellings(&found),
                ["orchard/plum/gate.rs"],
                "{path} contributes its comment and no quoted or program data"
            );
        }
    }

    /// Fenced and indented blocks, double-backtick exhibits and complete inline
    /// glob, regular-expression and configuration values contribute nothing. A
    /// naked single-backtick filename, an incomplete value and authored prose
    /// surrounding the displays remain citation-bearing.
    ///
    /// ´claim:reference:display-and-complete-inline-data-are-not-citations´
    /// ´test:unit:omits-display-and-complete-inline-data-but-keeps-the-prose-beside-them´
    #[test]
    fn omits_display_and_complete_inline_data_but_keeps_the_prose_beside_them() {
        let document = concat!(
            "Before orchard/plum/gate.rs here\n\n",
            "`orchard/plum/basket.md`\n\n",
            "``orchard/plum/gate.rs``\n\n",
            "```text\norchard/plum/gate.rs\n```\n\n",
            "    orchard/plum/gate.rs\n\n",
            "`orchard/*.rs`\n",
            "`^orchard/.*[.]rs$`\n",
            "`path = \"orchard/plum/gate.rs\"`\n",
            "`path = orchard/plum/basket.md`\n\n",
            "After orchard/plum/gate.rs here\n",
        );
        let found = classified(classify_carrier(
            &source("orchard/notes.md"),
            document,
            &orchard(),
            &[],
        ));

        assert_eq!(
            spellings(&found),
            [
                "orchard/plum/gate.rs",
                "orchard/plum/basket.md",
                "orchard/plum/basket.md",
                "orchard/plum/gate.rs",
            ],
            "only authored prose, a naked span and an incomplete value participate"
        );
    }

    /// Exact bounds supplied by a generator and a registered table schema
    /// restate their contained regions as path-valued data. An authored table
    /// cell and prose beside a generated projection remain citations, proving
    /// both directions at each structural boundary.
    ///
    /// ´claim:reference:structural-path-values-and-authored-prose-have-opposite-participation´
    /// ´test:unit:omits-generated-and-registered-path-values-but-keeps-authored-table-prose´
    #[test]
    fn omits_generated_and_registered_path_values_but_keeps_authored_table_prose() {
        let document = concat!(
            "Generated orchard/plum/gate.rs value\n\n",
            "Authored orchard/plum/basket.md beside it\n\n",
            "| Path | Note |\n",
            "| --- | --- |\n",
            "| orchard/plum/gate.rs | orchard/plum/basket.md authored |\n",
        );
        let generated_end = document.find("\n\n").expect("the projection boundary");
        let path_start = document
            .rfind("orchard/plum/gate.rs")
            .expect("the registered path cell");
        let column_start = document[..path_start].rfind('|').expect("the cell opener") + 1;
        let column_end = document[path_start..]
            .find('|')
            .map_or(document.len(), |at| path_start + at);
        let path_values = [
            PathValueRegion::new(0, generated_end),
            PathValueRegion::new(column_start, column_end),
        ];
        let found = classified(classify_carrier(
            &source("orchard/notes.md"),
            document,
            &orchard(),
            &path_values,
        ));

        assert_eq!(
            spellings(&found),
            ["orchard/plum/basket.md", "orchard/plum/basket.md"],
            "the generated and registered values are data while authored prose beside each remains governed"
        );
    }

    /// A catalogued opaque kind is a classified carrier with no citations,
    /// while a kind the total catalog has not learned receives the distinct
    /// uncatalogued classification. The classification is shadow machinery and
    /// is wired to no public finding.
    ///
    /// ´claim:reference:an-uncatalogued-carrier-is-a-shadow-classification´
    /// ´test:unit:classifies-an-uncatalogued-carrier-without-emitting-a-finding´
    #[test]
    fn classifies_an_uncatalogued_carrier_without_emitting_a_finding() {
        assert_eq!(
            classify_carrier(
                &source("orchard/notes.json"),
                "orchard/plum/gate.rs",
                &orchard(),
                &[],
            ),
            CarrierClassification::Catalogued(Vec::new()),
            "an opaque kind is classified and carries no region"
        );
        assert_eq!(
            classify_carrier(
                &source("orchard/notes.quince"),
                "orchard/plum/gate.rs",
                &orchard(),
                &[],
            ),
            CarrierClassification::Uncatalogued,
            "an unknown kind is preserved for the future finding rather than silently classified as empty"
        );
    }

    /// The `readmes` row every section carries reaches a README at the
    /// repository root and at every depth, and reaches nothing whose basename
    /// merely resembles one — a different case, a longer name, a second suffix.
    /// The row is suffix-shaped and a section is only ever offered its own
    /// owner's share, so the identical row in every section is one uniform rule
    /// rather than thirteen that could drift. A removed source is never read, so
    /// it earns the audit line naming the rule rather than any finding.
    ///
    /// ´claim:reference:the-readme-row-is-one-uniform-rule-across-every-section´
    /// ´test:unit:removes-every-readme-and-nothing-that-merely-resembles-one´
    #[test]
    fn removes_every_readme_and_nothing_that_merely_resembles_one() {
        let section = readmes_section();

        for excluded in [
            "README.md",
            "orchard/README.md",
            "orchard/plum/README.md",
            "hedgerow/quince/deep/README.md",
        ] {
            assert_eq!(
                section.excluded_by(&source(excluded)),
                Some("readmes"),
                "expected `{excluded}` to be removed by the named row"
            );
        }

        for scanned in [
            "orchard/readme.md",
            "orchard/README.mdx",
            "orchard/README.md.bak",
            "orchard/READMEs.md",
            "orchard/plum/basket.md",
            "orchard/README-notes.md",
        ] {
            assert_eq!(
                section.excluded_by(&source(scanned)),
                None,
                "expected `{scanned}` to be scanned"
            );
        }

        // The row is written once and holds in every section, because the
        // pattern is anchored at the basename rather than at any owner's root.
        // Two owners' sections carrying the same bytes are one rule read twice,
        // and neither section is ever offered the other's files.
        assert_eq!(readmes_section().exclude, section.exclude);
        assert_eq!(
            exclusion_line(&source("orchard/plum/README.md"), "readmes"),
            "file-path section: orchard/plum/README.md: excluded by rule readmes",
            "a removed source earns the audit line naming the rule and never a finding"
        );
    }

    /// A section of the seventh file is an exclusion list and nothing else. Its
    /// rows name that owner's own exceptions and subtract as an orderless
    /// union, and an owner exception stands beside the uniform row rather than
    /// replacing it. There is no inclusion list to carry, because this policy
    /// assigns no parameter per cell and the program defines what is read.
    ///
    /// ´claim:reference:a-section-is-an-exclusion-list-and-carries-no-inclusion´
    /// ´test:unit:an-owner-section-carries-named-exclusions-and-no-inclusion´
    #[test]
    fn an_owner_section_carries_named_exclusions_and_no_inclusion() {
        let mut section = readmes_section();

        section.exclude.push(SectionRow {
            name: String::from("vendored-orchard"),
            pattern: AbnfPattern::parse("%s\"orchard/quince\" [ \"/\" *VCHAR ]")
                .expect("a well-formed pattern"),
        });

        assert_eq!(section.exclude.len(), 2);
        assert_eq!(
            section.excluded_by(&source("orchard/quince/basket.md")),
            Some("vendored-orchard")
        );
        assert_eq!(
            section.excluded_by(&source("orchard/plum/README.md")),
            Some("readmes")
        );
        assert_eq!(section.excluded_by(&source("orchard/plum/basket.md")), None);

        let parameters = Parameters {
            sections: BTreeMap::from([(String::from("ORCHARD"), section)]),
        };

        assert_eq!(parameters.sections.len(), 1);
        assert_eq!(Parameters::default().sections.len(), 0);
        assert_eq!(Section::default().exclude, [] as [SectionRow; 0]);
    }

    /// An invented two-owner partition, and the seventh file's uniform sections
    /// over it. Nothing in it is a path this repository holds.
    fn orchard_partition() -> (Vec<BytePath>, Parameters) {
        let paths = [
            "README.md",
            "orchard/README.md",
            "orchard/notes.md",
            "orchard/plum/basket.md",
            "orchard/plum/gate.rs",
            "hedgerow/README.md",
            "hedgerow/notes.md",
            "hedgerow/quince.rs",
        ];

        let parameters = Parameters {
            sections: BTreeMap::from([
                (String::from("ORCHARD"), readmes_section()),
                (String::from("HEDGEROW"), readmes_section()),
            ]),
        };

        (paths.iter().map(|path| source(path)).collect(), parameters)
    }

    /// Attribute the invented partition by its first component: the orchard tree
    /// and the root document to one owner, the hedgerow to the other.
    fn orchard_attribution(paths: &[BytePath]) -> BTreeMap<&BytePath, &'static str> {
        paths
            .iter()
            .map(|path| {
                let owner = if path.display().starts_with("hedgerow/") {
                    "HEDGEROW"
                } else {
                    "ORCHARD"
                };

                (path, owner)
            })
            .collect()
    }

    /// Write the invented corpus into a temporary tree and return its root.
    fn orchard_tree(sources: &[(&str, &str)]) -> tempfile::TempDir {
        let root = tempfile::TempDir::new().expect("a temporary root");

        for (path, text) in sources {
            let full = root.path().join(path);

            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).expect("a fixture directory");
            }

            std::fs::write(&full, text).expect("a fixture source");
        }

        root
    }

    /// One row of a gloss, minted where it stands.
    fn gloss_row(name: &str, pattern: &str) -> SectionRow {
        SectionRow {
            name: String::from(name),
            pattern: AbnfPattern::parse(pattern).expect("a compilable pattern"),
        }
    }

    /// The uniform section with a gloss written beside its exclusion row.
    fn glossed_section(rows: Vec<SectionRow>) -> Section {
        Section {
            include: Some(rows),
            ..readmes_section()
        }
    }

    /// The orchard partition with a gloss written into the orchard's section.
    fn orchard_gloss(rows: Vec<SectionRow>) -> Parameters {
        Parameters {
            sections: BTreeMap::from([
                (String::from("ORCHARD"), glossed_section(rows)),
                (String::from("HEDGEROW"), readmes_section()),
            ]),
        }
    }

    /// The codes a run of findings carries, in the order it reports them.
    fn gloss_codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(Finding::code).collect()
    }

    /// The `include` records are absent by default and owe nothing where they
    /// are absent. Where an owner writes them they must satisfy the checked
    /// partition of the sources this policy reads, but they supply diagnostic
    /// gloss only: they neither admit a carrier the exclusions removed nor narrow
    /// the total reach the record fixes. So the governed set is the same with a
    /// gloss, without one, and with a gloss the check finds false — an incomplete
    /// gloss, an overlapping one, and one padded with a row reaching only
    /// excluded sources or another owner's share are each a finding against the
    /// declaration and never a change to what is read.
    ///
    /// ´claim:reference:the-include-records-are-an-optional-checked-gloss´
    /// ´test:unit:the-include-records-are-an-optional-checked-gloss´
    #[test]
    fn the_include_records_are_an_optional_checked_gloss() {
        let (paths, bare) = orchard_partition();
        let attributed = orchard_attribution(&paths);
        let governed: Vec<String> = govern(&bare, &attributed)
            .iter()
            .map(|entry| entry.path.display())
            .collect();

        // No gloss anywhere, and nothing is owed for the absence.
        assert!(
            gloss(&bare, &attributed).is_empty(),
            "an absent gloss claims nothing"
        );

        // A correct gloss over one owner's share: the orchard's three sources
        // fall to two disjoint rows that cover them exactly.
        let correct = orchard_gloss(vec![
            gloss_row(
                "prose",
                "%s\"orchard/\" [ *VCHAR \"/\" ] 1*( %x21-2E / %x30-7E ) %s\".md\"",
            ),
            gloss_row(
                "code",
                "%s\"orchard/\" [ *VCHAR \"/\" ] 1*( %x21-2E / %x30-7E ) %s\".rs\"",
            ),
        ]);

        assert!(
            gloss(&correct, &attributed).is_empty(),
            "{:?}",
            gloss(&correct, &attributed)
        );
        assert_eq!(
            govern(&correct, &attributed)
                .iter()
                .map(|entry| entry.path.display())
                .collect::<Vec<String>>(),
            governed,
            "a correct gloss reads the same sources a bare section does"
        );

        // An incomplete gloss leaves a governed source unnamed. The source is
        // read all the same: the gloss admits nothing and holds nothing out.
        let incomplete = orchard_gloss(vec![gloss_row(
            "prose",
            "%s\"orchard/\" [ *VCHAR \"/\" ] 1*( %x21-2E / %x30-7E ) %s\".md\"",
        )]);
        let findings = gloss(&incomplete, &attributed);

        assert_eq!(gloss_codes(&findings), ["file_path_gloss_uncovered"]);
        assert_eq!(
            findings[0].to_string(),
            "file-path section: orchard/plum/gate.rs: governed under ORCHARD and named by no include row; \
             the declared include gloss does not cover the governed set"
        );
        assert_eq!(
            govern(&incomplete, &attributed)
                .iter()
                .map(|entry| entry.path.display())
                .collect::<Vec<String>>(),
            governed,
            "a false gloss narrows nothing"
        );

        // An overlapping gloss names one source twice, which is the other half of
        // the partition judgment.
        let overlapping = orchard_gloss(vec![
            gloss_row("everything", "%s\"orchard\" [ \"/\" *VCHAR ]"),
            gloss_row(
                "deep-prose",
                "%s\"orchard/plum/\" 1*( %x21-2E / %x30-7E ) %s\".md\"",
            ),
        ]);
        let findings = gloss(&overlapping, &attributed);

        assert_eq!(
            gloss_codes(&findings),
            ["file_path_gloss_multiply_included"]
        );
        assert_eq!(
            findings[0].to_string(),
            "file-path section: orchard/plum/basket.md: named by 2 ORCHARD include rows: \
             deep-prose : %s\"orchard/plum/\" 1*( %x21-2E / %x30-7E ) %s\".md\", everything : %s\"orchard\" [ \"/\" *VCHAR ]"
        );

        // A row naming only a source the exclusions removed reaches nothing it
        // could partition, and so does one naming only another owner's share.
        // Both are idle, which is the class a dead row has always earned.
        let padded = orchard_gloss(vec![
            gloss_row("everything", "%s\"orchard\" [ \"/\" *VCHAR ]"),
            gloss_row("readme-gloss", "%s\"orchard/README.md\""),
            gloss_row("foreign", "%s\"hedgerow\" [ \"/\" *VCHAR ]"),
        ]);
        let findings = gloss(&padded, &attributed);

        assert_eq!(
            gloss_codes(&findings),
            ["file_path_idle_gloss_row", "file_path_idle_gloss_row"]
        );
        assert_eq!(
            findings
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<String>>(),
            [
                "file-path section: ORCHARD include row foreign: pattern matches no governed path",
                "file-path section: ORCHARD include row readme-gloss: pattern matches no governed path",
            ]
        );

        // An empty gloss over a non-empty governed set is a claim that there was
        // nothing to read, and it is false of every source the section governs.
        let empty = orchard_gloss(Vec::new());

        assert_eq!(
            gloss_codes(&gloss(&empty, &attributed)),
            [
                "file_path_gloss_uncovered",
                "file_path_gloss_uncovered",
                "file_path_gloss_uncovered"
            ]
        );
    }

    /// Governance names every README out and offers a section only its own
    /// owner's share. The uniform row removes a README at the repository root
    /// and at every depth, and an excluded source is never read at all — so it
    /// earns the audit line naming the rule that excused it rather than any
    /// finding. Two sections carrying identical bytes cannot disagree, because
    /// neither is ever asked about the other's file.
    ///
    /// ´claim:reference:governance-names-every-readme-out-within-each-owner-share´
    /// ´test:unit:governs-each-owner-share-and-names-every-readme-out´
    #[test]
    fn governs_each_owner_share_and_names_every_readme_out() {
        let (paths, parameters) = orchard_partition();
        let attributed = orchard_attribution(&paths);
        let governed = govern(&parameters, &attributed);

        let read: Vec<(&str, String)> = governed
            .iter()
            .map(|entry| (entry.owner.as_str(), entry.path.display()))
            .collect();

        assert_eq!(
            read,
            vec![
                ("HEDGEROW", String::from("hedgerow/notes.md")),
                ("HEDGEROW", String::from("hedgerow/quince.rs")),
                ("ORCHARD", String::from("orchard/notes.md")),
                ("ORCHARD", String::from("orchard/plum/basket.md")),
                ("ORCHARD", String::from("orchard/plum/gate.rs")),
            ],
            "every README is named out, at the root and at depth, and every other source is read"
        );

        assert_eq!(
            exclusions(&parameters, &attributed, "ORCHARD"),
            vec![
                Exclusion {
                    path: String::from("README.md"),
                    owner: String::from("ORCHARD"),
                    name: String::from("readmes"),
                },
                Exclusion {
                    path: String::from("orchard/README.md"),
                    owner: String::from("ORCHARD"),
                    name: String::from("readmes"),
                },
            ],
            "an excluded source earns the audit line naming its rule and never a finding"
        );

        assert_eq!(
            exclusions(&parameters, &attributed, "HEDGEROW")
                .into_iter()
                .map(|excluded| excluded.path)
                .collect::<Vec<_>>(),
            vec![String::from("hedgerow/README.md")],
            "a section is offered only its own owner's share, so it excuses no foreign README"
        );
    }

    /// A census row belongs to the owner of the source that wrote the citation
    /// and never to the owner of the file it names. One owner's document citing
    /// the other owner's file is a row in the citing owner's table, which is what
    /// makes the thirteen tables a division of the corpus rather than of the
    /// targets. A source holding no citation contributes no row at all, because
    /// the codec's identity is a source with a positive count.
    ///
    /// ´claim:reference:a-census-row-belongs-to-the-owner-of-the-citing-source´
    /// ´test:unit:counts-each-source-under-its-own-owner-and-omits-the-clean-ones´
    #[test]
    fn counts_each_source_under_its_own_owner_and_omits_the_clean_ones() {
        let (paths, parameters) = orchard_partition();
        let attributed = orchard_attribution(&paths);
        let lexicon = Lexicon::from_tracked(&paths);

        // The hedgerow document cites the orchard's file, and the orchard
        // document cites the hedgerow's. Each row belongs to the source.
        let root = orchard_tree(&[
            (
                "README.md",
                "orchard/plum/gate.rs stands here and is never read",
            ),
            (
                "orchard/README.md",
                "orchard/plum/basket.md is never read either",
            ),
            (
                "orchard/notes.md",
                "See hedgerow/quince.rs and hedgerow/notes.md.",
            ),
            ("orchard/plum/basket.md", "Nothing here names a file."),
            (
                "orchard/plum/gate.rs",
                "// orchard/plum/basket.md is cited in a comment\nfn main() {}",
            ),
            ("hedgerow/notes.md", "The gate is `orchard/plum/gate.rs`."),
            (
                "hedgerow/quince.rs",
                "fn main() { let path = \"orchard/plum/gate.rs\"; }",
            ),
        ]);

        let governed = govern(&parameters, &attributed);
        let censused = census(root.path(), &lexicon, &governed);

        let rows: Vec<(&str, String, usize)> = censused
            .iter()
            .filter(|source| !source.citations.is_empty())
            .map(|source| (source.owner, source.path.display(), source.citations.len()))
            .collect();

        assert_eq!(
            rows,
            vec![
                ("HEDGEROW", String::from("hedgerow/notes.md"), 1),
                ("ORCHARD", String::from("orchard/notes.md"), 2),
                ("ORCHARD", String::from("orchard/plum/gate.rs"), 1),
            ],
            "a row stands under the owner of the citing source, never the owner of the target"
        );

        assert!(
            censused.iter().all(|source| !source.uncatalogued),
            "every fixture kind is one the total carrier catalog reads"
        );
        assert!(
            carriers(&censused).is_empty(),
            "a catalogued kind raises no carrier finding, whether it reads comments or is opaque"
        );

        assert_eq!(
            counted(&censused).len(),
            3,
            "a source holding no citation contributes no row, and neither does one never read"
        );

        // The hedgerow source's only path stands in a string literal, which is
        // program data rather than a citation its author made in commentary.
        assert!(
            counted(&censused)
                .keys()
                .all(|path| path.display() != "hedgerow/quince.rs"),
            "a path in a string literal is data and contributes nothing"
        );
    }

    /// A governed source whose kind the total carrier catalog has not learned is
    /// reported rather than skipped, because the reach the owner ruled cannot be
    /// met by a program that declines to ask whether a new syntax has comments.
    /// The finding names the source and its owner, and it is the only thing an
    /// unlearned kind produces: no citation is invented out of bytes no reader
    /// was willing to lex.
    ///
    /// ´claim:reference:an-unlearned-carrier-kind-is-reported-rather-than-skipped´
    /// ´test:unit:reports-a-governed-source-whose-kind-the-catalog-has-not-learned´
    #[test]
    fn reports_a_governed_source_whose_kind_the_catalog_has_not_learned() {
        let paths: Vec<BytePath> = [
            "orchard/notes.md",
            "orchard/plum/gate.rs",
            "orchard/thicket.quince",
        ]
        .iter()
        .map(|path| source(path))
        .collect();

        let parameters = Parameters {
            sections: BTreeMap::from([(String::from("ORCHARD"), readmes_section())]),
        };

        let attributed: BTreeMap<&BytePath, &str> =
            paths.iter().map(|path| (path, "ORCHARD")).collect();
        let lexicon = Lexicon::from_tracked(&paths);

        let root = orchard_tree(&[
            ("orchard/notes.md", "The gate is orchard/plum/gate.rs."),
            ("orchard/plum/gate.rs", "fn main() {}"),
            ("orchard/thicket.quince", "orchard/plum/gate.rs stands here"),
        ]);

        let governed = govern(&parameters, &attributed);
        let censused = census(root.path(), &lexicon, &governed);
        let reported = carriers(&censused);

        assert_eq!(
            reported.iter().map(ToString::to_string).collect::<Vec<_>>(),
            vec![String::from(
                "file-path carrier: orchard/thicket.quince: the tracked file kind has no declared region reader"
            )],
            "the unlearned kind is named, and the two catalogued sources beside it are not"
        );

        let unlearned = censused
            .iter()
            .find(|entry| entry.path.display() == "orchard/thicket.quince")
            .expect("the unlearned source is censused");

        assert!(unlearned.uncatalogued);
        assert!(
            unlearned.citations.is_empty(),
            "no citation is invented out of bytes no reader was willing to lex"
        );
        assert_eq!(
            counted(&censused).len(),
            1,
            "the unlearned source contributes no row, so its debt is never quietly tolerated"
        );
    }

    /// The declared maximum is a ceiling and the verdict is what stands beyond
    /// it. A source at its maximum is tolerated debt and raises nothing; a source
    /// above it raises exactly the excess, in the order the occurrences stand;
    /// and a source no row declares raises every one of them. Shrink is not
    /// judged here, because a source that has repaired a citation is making the
    /// progress the count codec exists to record.
    ///
    /// ´claim:reference:the-declared-maximum-is-a-ceiling-and-the-excess-is-the-verdict´
    /// ´test:unit:judges-every-occurrence-beyond-the-declared-maximum´
    #[test]
    fn judges_every_occurrence_beyond_the_declared_maximum() {
        let (paths, parameters) = orchard_partition();
        let attributed = orchard_attribution(&paths);
        let lexicon = Lexicon::from_tracked(&paths);

        let root = orchard_tree(&[
            (
                "orchard/notes.md",
                "See hedgerow/quince.rs and hedgerow/notes.md.",
            ),
            (
                "orchard/plum/basket.md",
                "The gate is orchard/plum/gate.rs.",
            ),
            (
                "orchard/plum/gate.rs",
                "// see orchard/plum/basket.md\nfn main() {}",
            ),
            ("hedgerow/notes.md", "Nothing here names a file."),
            ("hedgerow/quince.rs", "fn main() {}"),
        ]);

        let governed = govern(&parameters, &attributed);
        let censused = census(root.path(), &lexicon, &governed);

        let notes = source("orchard/notes.md");
        let basket = source("orchard/plum/basket.md");

        // Every source is declared at exactly what it holds: the tolerated state.
        let exact = BTreeMap::from([(&notes, 2), (&basket, 1)]);
        let declared = conform(&censused, &exact);
        let uncovered: Vec<&str> = declared
            .iter()
            .map(|finding| match finding {
                Finding::FilePathCitation { path, .. } => path.as_str(),
                _ => panic!("this pass raises no other finding"),
            })
            .collect();

        assert_eq!(
            uncovered,
            vec!["orchard/plum/gate.rs"],
            "a source at its maximum raises nothing, and one no row declares raises all of its citations"
        );

        // Lower one ceiling by one and exactly the last occurrence stands out.
        let lowered = BTreeMap::from([(&notes, 1), (&basket, 1)]);
        let excess = conform(&censused, &lowered);

        let Finding::FilePathCitation {
            path,
            owner,
            spelling,
            region_kind,
            location,
        } = &excess[0]
        else {
            panic!("this pass raises no other finding");
        };

        assert_eq!(
            excess.len(),
            2,
            "the excess is one occurrence beyond the lowered ceiling, and the undeclared source's own"
        );
        assert_eq!(path, "orchard/notes.md");
        assert_eq!(owner, "ORCHARD");
        assert_eq!(
            spelling, "hedgerow/notes.md",
            "the excess is the occurrence past the ceiling, not the first one"
        );
        assert_eq!(*region_kind, "prose");
        assert_eq!(
            location.line(),
            1,
            "the finding sends a reader to the line the spelling stands on"
        );

        assert_eq!(
            excess[0].to_string(),
            "file-path citation: orchard/notes.md: hedgerow/notes.md names another tracked file; cite its label",
            "the human finding names the target and asks for its label"
        );

        // A ceiling above the observation is shrink, and shrink is the writer's
        // business rather than a verdict this pass takes.
        let raised = BTreeMap::from([(&notes, 9), (&basket, 9)]);

        assert_eq!(
            conform(&censused, &raised).len(),
            1,
            "shrink raises nothing here; only the undeclared source's citation stands"
        );
    }
}
