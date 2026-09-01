// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The SPDX policy's parameters: two named sets, and a section for each owner.
//!
//! Twenty-seven of the catalogued policies are fully determined by their
//! recognizer — what a unique mint is, or a resolvable citation, is not a
//! repository choice. This one is the first whose requirement *is* a repository
//! choice. A licence expression and a copyright line are content the owner rules
//! and the linter cannot derive, and they differ across a tree: a vendored
//! subtree does not carry the repository's copyright. So the fifth declared file
//! carries the fifth kind of declaration — policy parameters, the data a policy
//! needs to know **what** to require, as distinct from whether it applies and
//! what is tolerated.
//!
//! Naming the file for its policy rather than calling it `parameters.toml` keeps
//! each parameter schema closed and policy-owned, exactly as each list codec is
//! policy-owned. The surface then grows one file per parameterized policy family
//! and never grows a shared table whose shape no single policy owns.
//!
//! # Two sets, and two halves that select between them
//!
//! The file declares two name-to-text sets, one for the licence expression and
//! one for the copyright line, and a section for each owner holding the policy.
//! An owner's section has two halves, and the half is what selects the set: an
//! inclusion row in the identifier half names an entry of the identifier set,
//! one in the copyright half names an entry of the copyright set, and no row
//! needs a third field to say which set it meant.
//!
//! A set is a name-to-text function, so it is a table keyed by the name and a
//! repeated name is a lexical error caught before any name is looked at. A
//! per-owner list is a list of pairs in which one reference may appear several
//! times, so it is an array of rows. The difference in shape carries the
//! difference in meaning and is not a matter of taste.
//!
//! # A partition nested inside a partition, twice over
//!
//! The owner file divides the repository among owners: its exclusion rules
//! remove paths from the accounting universe first, and its inclusion rows then
//! partition what survives, totally and exclusively. Each half of an owner's
//! section divides that owner's share the same way, and the two halves do it
//! independently — so a file may be governed by one half and excused by the
//! other, which is the vendored case the two halves exist for.
//!
//! The repository level and the policy level never interact. The repository
//! exclusion decides what is accounted at all; a policy exclusion decides what a
//! licence header is required of. A path the repository excludes is not in the
//! universe, so no row here can reach it and none may name it — a row that
//! reaches only such paths is reported as reaching nothing rather than kept as a
//! dead row nobody notices.
//!
//! Totality is what makes an exclusion list mean anything. Without it, *not
//! included* and *excluded* are one state, and a reviewer cannot tell an
//! intentional omission from a forgotten one because both look like absence.
//! Under exclude-then-partition, every file of the owner stands, in each half,
//! in exactly one of three states, each of them written down by a human:
//! excluded, governed and conforming, or governed and burning.
//!
//! # What the two texts are held to
//!
//! The identifier is held to a closed licence-expression syntax and never to
//! membership in a published list. A list check would age between releases,
//! would make the linter carry and version an external registry, and would
//! reject the fictional expression the governing ruling was itself written with.
//! What an identifier *means* is the owner's ruling; that it is a well-formed
//! expression is this module's.
//!
//! The copyright text is a nonempty exact string with no edge whitespace and no
//! line break, compared byte for byte and never evaluated against the calendar.
//! A copyright line is prose with a year and a holder, and every attempt to
//! constrain it further constrains a jurisdiction rather than a syntax.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`an_inclusion_row_selects_and_is_no_gloss`] | spdx | The licence policy's inclusion rows are constitutive, and the ruling that made the interchange and file-path include records a diagnostic gloss does not reach them. Here an inclusion row still selects: narrowing one removes a file from the governed set rather than leaving it governed under a gloss that failed, a half declaring no inclusion row governs nothing at all, and a surviving path no row reaches is ungoverned rather than merely unnamed. The scope of what may carry a licence is much wider than one carrier catalog, so the inclusion rows are the only thing that says which files this policy is about, and a repository cannot drop them. |
//! | [`a_half_excludes_then_partitions_what_survives`] | spdx | Each half removes the union of its exclusion rules first and then partitions what survives, totally and exclusively. A surviving path no inclusion row reaches breaks totality by name, and one two rows reach breaks exclusivity even where both rows name the same set entry — so an exception is carved by writing disjoint rows and never by shadowing a broad row with a narrow one. |
//! | [`a_file_is_governed_by_one_half_and_excused_by_the_other`] | spdx | The two halves divide the owner's share independently, so a file may be governed by one and excused by the other. That is the vendored case the per-half exclusion lists were ruled for: a third-party source must declare the licence it arrived under, and this repository may not put its copyright on it, and no single shared exclusion list could say both. |
//! | [`a_row_is_only_offered_its_owners_share`] | spdx | A row is offered only paths attributed to its owner. A row whose pattern would match another owner's path is idle if it reaches none in its share: containment is impossible by construction, while dead rows remain named. |
//! | [`a_path_that_is_not_text_is_unnameable_by_any_row`] | spdx | A path whose bytes are not text is matched by no row, including a row spelled to match everything, so it is unnameable by any row the surface can write. The failure is therefore loud rather than silent: the path is accounted like any other, so matching no inclusion row breaks totality by name instead of quietly leaving the governed set. |
//! | [`an_entry_that_can_never_head_is_named_before_it_is_read`] | spdx | A governed entry that can never carry a header is named at configuration time rather than failed forever: a symbolic link has no content of its own to head, and a file of a type no comment leader is catalogued for has nowhere to put one. Nothing is removed implicitly, so the remedy in both cases is an exclusion row somebody writes. |
//! | [`a_header_is_read_at_the_front_of_the_file_and_nowhere_else`] | spdx | A header is read at the front of the file and nowhere else. The two header lines standing inside a string literal are outside the region, so the file that carries them as a fixture correctly lacks a header where a whole-file search would report one; and an interpreter line moves the region down one, so the corpus's headed script conforms. |
//! | [`the_identifier_is_exactly_one_and_the_copyright_at_least_one`] | spdx | The two fields are not symmetric and the asymmetry is deliberate. A file has one licence, so exactly one region line carries the identifier and two fail whether or not the texts agree. A file may have many copyright holders, so at least one line must carry the required text and further copyright lines are permitted and never examined — requiring a sole copyright line would forbid a contributor from adding their own. |
//! | [`a_listed_path_tolerates_a_wrong_header_as_an_absent_one`] | spdx | A listed path is silent however it fails. The ruling is uniform toleration: a row carries a file that fails, whether the required header is absent or wrong, so the list keeps the one meaning a reviewer reads it as and a repository that discovers a wrong header needs no second mechanism to hold it while it is corrected. The configuration findings are not tolerated, because a burn list cannot excuse a section that does not describe the repository. |
//! | [`the_half_selects_the_set_and_the_field`] | spdx | The half is what selects both the set a row's name resolves in and the field a governed file must carry, so a row needs no third component to say which requirement it states and the two halves can never be crossed. |
//! | [`an_identifier_is_held_to_a_syntax_and_not_to_a_list`] | spdx | The identifier text is held to the licence-expression syntax entire — disjunction, conjunction, the exception operator, parentheses and the later-version mark — and never to membership in a published list. A list check would reject the fictional expression the governing ruling was written with, and would oblige this binary to carry and version a registry that ages between releases. |
//! | [`an_identifier_outside_the_syntax_is_refused`] | spdx | A text the syntax does not admit is refused: an empty text, an operator that is not surrounded by exactly one space, an unclosed parenthesis, a token opening with a separator, and a token ending in one. The spacing is part of the syntax because a surface admitting two spellings of one expression is a surface on which the repository has not actually fixed its licence. |
//! | [`a_copyright_text_is_a_line_and_not_a_form`] | spdx | The copyright text is a nonempty string with no edge whitespace and no line break, and nothing further is required of it: a year and a holder are prose, and a rule that constrained them would constrain a jurisdiction rather than a syntax. |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::declaration::AbnfPattern;
use crate::finding::Finding;
use crate::leader;
use crate::pattern::{BytePath, reversible};
use crate::selection::{
    ConstitutiveDefect, ConstitutiveSection, List as SelectionList, Rule as SelectionRule,
    constitutive,
};

/// The field the identifier half requires a governed file to carry.
pub const IDENTIFIER_FIELD: &str = "SPDX-License-Identifier";

/// The field the copyright half requires a governed file to carry.
pub const COPYRIGHT_FIELD: &str = "SPDX-FileCopyrightText";

/// One half of an owner's section: which requirement its rows state.
///
/// The half selects the set a row's name resolves in and the field a governed
/// file must carry, which is why a row carries a name and a pattern and nothing
/// else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Half {
    /// The licence the file declares.
    Identifier,
    /// The copyright line the file carries.
    Copyright,
}

/// Both halves, in the order a section declares them.
pub const HALVES: [Half; 2] = [Half::Identifier, Half::Copyright];

impl Half {
    /// The half's name, as a declaration and a report spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Copyright => "copyright",
        }
    }

    /// The header field a governed file must carry for this half.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Identifier => IDENTIFIER_FIELD,
            Self::Copyright => COPYRIGHT_FIELD,
        }
    }
}

impl fmt::Display for Half {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Which of a half's two lists a row stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListKind {
    /// The rows that remove paths from the half before anything is required of them.
    Exclude,
    /// The rows that partition what the exclusion left.
    Partition,
}

/// Both lists, in the order a half declares them.
pub const LISTS: [ListKind; 2] = [ListKind::Exclude, ListKind::Partition];

impl ListKind {
    /// The list's name, as a declaration and a report spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exclude => "exclude",
            Self::Partition => "partitions",
        }
    }
}

impl fmt::Display for ListKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One row of one list: a name, the entry it carries where it carries one, and
/// the paths it reaches.
///
/// The name is the row's own in both lists and is never a reference: it names the
/// region the row is about, and it is minted where it stands, so a report can say
/// which rule excused a file and which row governs one rather than only that some
/// row did. A partition row carries a second declaration beside it — the entry of
/// the half's set that its region holds — while an exclusion row carries none,
/// having no licence text to hold over anything.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SectionRow {
    /// The name the row carries.
    pub name: String,
    /// The entry of the half's set the row's region carries.
    pub entry: Option<String>,
    /// The paths the row reaches.
    pub pattern: AbnfPattern,
}

impl SectionRow {
    /// An exclusion rule of that name over that pattern.
    #[must_use]
    pub const fn new(name: String, pattern: AbnfPattern) -> Self {
        Self {
            name,
            entry: None,
            pattern,
        }
    }

    /// A partition row of that name, carrying that entry over that pattern.
    #[must_use]
    pub const fn carrying(name: String, entry: String, pattern: AbnfPattern) -> Self {
        Self {
            name,
            entry: Some(entry),
            pattern,
        }
    }
}

impl fmt::Display for SectionRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} : {}", self.name, self.pattern)
    }
}

/// One half of one owner's section: its exclusion rules and its partition rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HalfSection {
    /// The rules that remove paths from this half of the owner's share.
    exclude: Vec<SectionRow>,
    /// The rows that partition what the exclusion left.
    partitions: Vec<SectionRow>,
}

impl HalfSection {
    /// A half carrying these two lists.
    #[must_use]
    pub const fn new(exclude: Vec<SectionRow>, partitions: Vec<SectionRow>) -> Self {
        Self {
            exclude,
            partitions,
        }
    }

    /// The exclusion rules.
    #[must_use]
    pub fn exclude(&self) -> &[SectionRow] {
        &self.exclude
    }

    /// The rows the half partitions its survivors with.
    #[must_use]
    pub fn partitions(&self) -> &[SectionRow] {
        &self.partitions
    }

    /// Whichever of the two lists is asked for.
    #[must_use]
    pub fn list(&self, kind: ListKind) -> &[SectionRow] {
        match kind {
            ListKind::Exclude => self.exclude(),
            ListKind::Partition => self.partitions(),
        }
    }
}

/// One owner's section: the two halves, shaped independently.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Section {
    /// The half stating which licence the owner's files declare.
    identifier: HalfSection,
    /// The half stating which copyright line they carry.
    copyright: HalfSection,
}

impl Section {
    /// A section carrying these two halves.
    #[must_use]
    pub const fn new(identifier: HalfSection, copyright: HalfSection) -> Self {
        Self {
            identifier,
            copyright,
        }
    }

    /// Whichever half is asked for.
    #[must_use]
    pub const fn half(&self, half: Half) -> &HalfSection {
        match half {
            Half::Identifier => &self.identifier,
            Half::Copyright => &self.copyright,
        }
    }
}

/// The whole of the fifth declaration: the two sets, and a section per owner.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Parameters {
    /// The identifier set: a name against a licence expression.
    identifiers: BTreeMap<String, String>,
    /// The copyright set: a name against a copyright line.
    copyrights: BTreeMap<String, String>,
    /// One section per owner the policy governs.
    sections: BTreeMap<String, Section>,
}

impl Parameters {
    /// The parameters these two sets and these sections come to.
    #[must_use]
    pub const fn new(
        identifiers: BTreeMap<String, String>,
        copyrights: BTreeMap<String, String>,
        sections: BTreeMap<String, Section>,
    ) -> Self {
        Self {
            identifiers,
            copyrights,
            sections,
        }
    }

    /// The set a half's names resolve in.
    #[must_use]
    pub const fn set(&self, half: Half) -> &BTreeMap<String, String> {
        match half {
            Half::Identifier => &self.identifiers,
            Half::Copyright => &self.copyrights,
        }
    }

    /// The text a name stands for in a half's set, when the set declares it.
    #[must_use]
    pub fn text(&self, half: Half, name: &str) -> Option<&str> {
        self.set(half).get(name).map(String::as_str)
    }

    /// The section declared for each owner.
    #[must_use]
    pub const fn sections(&self) -> &BTreeMap<String, Section> {
        &self.sections
    }
}

/// Whether a text is a well-formed licence expression.
///
/// The syntax is closed and the spacing is part of it: exactly one space on
/// either side of each operator, and no other whitespace anywhere. A surface
/// that admitted two spellings of one expression would be a surface on which the
/// repository has not actually fixed its licence, and comparison here is byte
/// equality precisely so that it cannot.
#[must_use]
pub fn is_licence_expression(text: &str) -> bool {
    let mut reader = Expression { text, at: 0 };

    reader.or_expression() && reader.at == text.len()
}

/// Whether a text is a well-formed copyright line.
///
/// It is a nonempty exact string with no leading or trailing whitespace and no
/// line break — the shape the environment relation already imposes on a name —
/// and nothing further. A copyright line is prose with a year and a holder, and
/// a rule constraining it further would constrain a jurisdiction rather than a
/// syntax.
#[must_use]
pub fn is_copyright_text(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && !text.contains(['\n', '\r'])
}

/// A cursor over a licence expression, reading the closed syntax left to right.
struct Expression<'a> {
    /// The text being read.
    text: &'a str,
    /// How much of it has been consumed.
    at: usize,
}

impl Expression<'_> {
    /// One or more conjunctions, separated by the disjunction operator.
    fn or_expression(&mut self) -> bool {
        if !self.and_expression() {
            return false;
        }

        while self.take(" OR ") {
            if !self.and_expression() {
                return false;
            }
        }

        true
    }

    /// One or more exception-bearing terms, separated by the conjunction operator.
    fn and_expression(&mut self) -> bool {
        if !self.with_expression() {
            return false;
        }

        while self.take(" AND ") {
            if !self.with_expression() {
                return false;
            }
        }

        true
    }

    /// One term, optionally carrying one exception.
    fn with_expression(&mut self) -> bool {
        if !self.atom() {
            return false;
        }

        if self.take(" WITH ") {
            return self.idstring();
        }

        true
    }

    /// A parenthesized expression, or a bare identifier with its optional mark.
    fn atom(&mut self) -> bool {
        if self.take("(") {
            return self.or_expression() && self.take(")");
        }

        self.idstring() && {
            let _later = self.take("+");
            true
        }
    }

    /// One identifier token: alphanumeric first, and never separator-last.
    fn idstring(&mut self) -> bool {
        let rest = &self.text[self.at..];
        let mut end = 0;

        for (offset, letter) in rest.char_indices() {
            if letter.is_ascii_alphanumeric() || letter == '-' || letter == '.' {
                end = offset + letter.len_utf8();
            } else {
                break;
            }
        }

        let Some(token) = rest.get(..end).filter(|token| !token.is_empty()) else {
            return false;
        };

        let bytes = token.as_bytes();

        if !bytes[0].is_ascii_alphanumeric()
            || bytes[bytes.len() - 1] == b'-'
            || bytes[bytes.len() - 1] == b'.'
        {
            return false;
        }

        self.at += end;

        true
    }

    /// Consume a literal where it stands next, and report whether it did.
    fn take(&mut self, literal: &str) -> bool {
        if self.text[self.at..].starts_with(literal) {
            self.at += literal.len();

            return true;
        }

        false
    }
}

/// One file that one half of one owner's section governs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Governed {
    /// The file the half requires a header of.
    pub path: BytePath,
    /// The owner whose section governs it.
    pub owner: String,
    /// The half that governs it.
    pub half: Half,
    /// The inclusion row that reaches it.
    pub row: SectionRow,
}

/// The plan-time result of the SPDX policy's constitutive selection.
#[derive(Debug, Clone, Default)]
pub struct SelectionPlan {
    governed: Vec<Governed>,
    findings: Vec<Finding>,
    exclusions: Vec<Exclusion>,
}

impl SelectionPlan {
    /// Files selected by exactly one constitutive inclusion row.
    #[must_use]
    pub fn governed(&self) -> &[Governed] {
        &self.governed
    }

    /// Stable policy findings mapped from the generic selection judgment.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Named exclusion matches retained for audit explanations.
    #[must_use]
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }
}

/// One path a named rule removes from one half of an owner's section.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Exclusion {
    /// The excluded path, in reversible display form.
    pub path: String,
    /// The owner whose section excludes it.
    pub owner: String,
    /// The half the rule removes it from.
    pub half: Half,
    /// The exclusion rule's name.
    pub name: String,
}

/// Compile the SPDX policy's typed constitutive selection.
#[must_use]
pub fn selection_plan(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> SelectionPlan {
    let mut sections = Vec::new();

    for (owner, section) in parameters.sections() {
        for half in HALVES {
            let side = section.half(half);
            let exclude = side
                .exclude()
                .iter()
                .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), ()))
                .collect();
            let include = side
                .partitions()
                .iter()
                .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), row.clone()))
                .collect();
            sections.push(ConstitutiveSection::new(
                owner.clone(),
                half,
                exclude,
                include,
            ));
        }
    }

    let selected = constitutive(attribution, &sections);
    let governed = selected
        .governed
        .into_iter()
        .map(|entry| Governed {
            path: entry.path,
            owner: entry.owner,
            half: entry.context,
            row: entry.payload,
        })
        .collect();
    let mut exclusions: Vec<_> = selected
        .excluded
        .into_iter()
        .map(|entry| Exclusion {
            path: entry.path.display(),
            owner: entry.owner,
            half: entry.context,
            name: entry.name,
        })
        .collect();
    exclusions.sort();
    let findings = selected
        .defects
        .into_iter()
        .map(|defect| match defect {
            ConstitutiveDefect::Uncovered {
                path,
                owner,
                context,
            } => Finding::SpdxUngovernedPath {
                path: path.display(),
                owner,
                half: context.as_str(),
            },
            ConstitutiveDefect::MultiplyIncluded {
                path,
                owner,
                context,
                matches,
            } => Finding::SpdxMultiplyIncluded {
                path: path.display(),
                owner,
                half: context.as_str(),
                count: matches.len(),
                matches,
            },
            ConstitutiveDefect::IdleRow {
                owner,
                context,
                list,
                name,
            } => Finding::SpdxIdleRow {
                owner,
                half: context.as_str(),
                list: match list {
                    SelectionList::Exclude => ListKind::Exclude.as_str(),
                    SelectionList::Include => ListKind::Partition.as_str(),
                },
                name,
            },
        })
        .collect();

    SelectionPlan {
        governed,
        findings,
        exclusions,
    }
}

/// Name every rule that excludes an accounted path from one owner's halves.
#[must_use]
#[cfg(test)]
pub fn retiring_exclusions(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
    owner: &str,
) -> Vec<Exclusion> {
    let Some(section) = parameters.sections().get(owner) else {
        return Vec::new();
    };
    let mut excluded = Vec::new();

    for (path, accounted) in attribution {
        if *accounted != owner {
            continue;
        }

        for half in HALVES {
            for row in section
                .half(half)
                .exclude()
                .iter()
                .filter(|row| row.pattern.admits_path(path))
            {
                excluded.push(Exclusion {
                    path: path.display(),
                    owner: owner.to_owned(),
                    half,
                    name: row.name.clone(),
                });
            }
        }
    }

    excluded.sort();
    excluded
}

/// Divide each owner's accounted share, in each half, into excluded and governed.
///
/// The two passes are separate because a path a half's exclusion rules removed is
/// never evaluated against that half's inclusion rows at all, so an overlap
/// between an exclusion rule and an inclusion row is legal and cannot be double
/// accounting — the same reading the repository partition already has.
///
/// Every row is offered only its owner's accounted share. Containment is
/// therefore impossible by construction rather than a further judgment over the
/// repository. Reach is tallied for both lists within that share, because a row
/// of either kind that reaches nothing is dead and is reported rather than kept.
#[must_use]
#[cfg(test)]
pub fn retiring_govern<'a>(
    parameters: &'a Parameters,
    attribution: &BTreeMap<&'a BytePath, &'a str>,
) -> (Vec<Governed>, Vec<Finding>) {
    let mut governed = Vec::new();
    let mut findings = Vec::new();

    for (owner, section) in parameters.sections() {
        for half in HALVES {
            let side = section.half(half);
            let mut reached: BTreeMap<(ListKind, &str), bool> = BTreeMap::new();

            for kind in LISTS {
                for row in side.list(kind) {
                    reached.insert((kind, row.name.as_str()), false);
                }
            }

            // A row is only ever offered the paths its own owner is accounted,
            // so reach is measured over the share and never over the repository.
            // This retires containment rather than answering it: a row cannot
            // reach another owner's file because it is never shown one.
            for (path, accounted) in attribution {
                if *accounted != owner.as_str() {
                    continue;
                }

                for kind in LISTS {
                    for row in side.list(kind) {
                        if row.pattern.admits_path(path) {
                            reached.insert((kind, row.name.as_str()), true);
                        }
                    }
                }
            }

            for (path, accounted) in attribution {
                if *accounted != owner.as_str() {
                    continue;
                }

                if side
                    .exclude()
                    .iter()
                    .any(|row| row.pattern.admits_path(path))
                {
                    continue;
                }

                let matched: Vec<&SectionRow> = side
                    .partitions()
                    .iter()
                    .filter(|row| row.pattern.admits_path(path))
                    .collect();

                match matched.as_slice() {
                    [row] => governed.push(Governed {
                        path: (*path).clone(),
                        owner: owner.clone(),
                        half,
                        row: (*row).clone(),
                    }),
                    [] => findings.push(Finding::SpdxUngovernedPath {
                        path: path.display(),
                        owner: owner.clone(),
                        half: half.as_str(),
                    }),
                    rows => {
                        let mut row_names: Vec<String> =
                            rows.iter().map(ToString::to_string).collect();
                        row_names.sort();

                        findings.push(Finding::SpdxMultiplyIncluded {
                            path: path.display(),
                            owner: owner.clone(),
                            half: half.as_str(),
                            count: rows.len(),
                            matches: row_names,
                        });
                    }
                }
            }

            for ((kind, name), found) in reached {
                if !found {
                    findings.push(Finding::SpdxIdleRow {
                        owner: owner.clone(),
                        half: half.as_str(),
                        list: kind.as_str(),
                        name: name.to_owned(),
                    });
                }
            }
        }
    }

    (governed, findings)
}

/// Divide each owner's share through the plan's constitutive selection type.
#[must_use]
#[cfg(test)]
pub fn govern(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> (Vec<Governed>, Vec<Finding>) {
    let selected = selection_plan(parameters, attribution);
    (selected.governed, selected.findings)
}

/// Name every governed entry that can never carry a header, whatever it holds.
///
/// Both questions are asked of every entry and each is answered on its own, so an
/// entry that is a link *and* of an uncatalogued type is named twice. Neither
/// statement is weaker for the other being true, and a precedence between them
/// would be a rule a reader has to know before they can read the report.
///
/// The remedy in both cases is a human writing an exclusion row. Nothing is
/// removed implicitly, for the same reason no ignore-file convention removes a
/// tracked entry implicitly.
#[must_use]
pub fn carriers(root: &Path, governed: &[Governed]) -> Vec<Finding> {
    let mut findings = Vec::new();

    for entry in governed {
        if is_link(root, &entry.path) {
            findings.push(Finding::SpdxLinkedPath {
                path: entry.path.display(),
                owner: entry.owner.clone(),
                half: entry.half.as_str(),
                name: entry.row.name.clone(),
            });
        }

        if leader::catalogued(&entry.path).is_none() {
            findings.push(Finding::SpdxUncataloguedType {
                path: entry.path.display(),
                owner: entry.owner.clone(),
                half: entry.half.as_str(),
                name: entry.row.name.clone(),
            });
        }
    }

    findings
}

/// Read each governed file's header region and judge it against the text its row names.
///
/// The two fields are not symmetric, and the asymmetry is deliberate. A file has
/// one licence, so exactly one line of the region carries the identifier and two
/// are an ambiguity that fails whether or not the texts agree. A file may have
/// many copyright holders, so at least one line must carry the required text and
/// further copyright lines are permitted and never examined — requiring a sole
/// copyright line would forbid a contributor from adding their own.
///
/// Neither field is constrained in its position within the region or in its order
/// relative to the other. The halves have independent lists, so a positional rule
/// would make one field's placement depend on a row that need not exist.
///
/// A file this pass cannot read is left to the pass that names it. An entry that
/// is a link or of an uncatalogued type has already been reported as one that can
/// never carry a header, and reading it here would report the same fact twice in
/// weaker words.
///
/// A tolerated path is silent however it fails. The ruling is uniform toleration:
/// a listed path tolerates a required header that is absent and equally one that
/// is wrong, so the list keeps one meaning and a repository that discovers a
/// wrong header needs no second mechanism to hold it while it is corrected. The
/// set is bare paths rather than owner-and-path pairs because owner-scope
/// containment already makes a listed path unambiguous: a row filed under an
/// owner the inclusion relation does not attribute its file to refuses the whole
/// snapshot before this pass runs.
///
/// The configuration findings are not tolerated and are not asked here. They are
/// asked of a parsed declaration against the tree and fail before any header is
/// read, so a burn list cannot excuse a section that does not describe the
/// repository.
#[must_use]
pub fn conform(
    root: &Path,
    parameters: &Parameters,
    governed: &[Governed],
    tolerated: &BTreeSet<&BytePath>,
) -> Vec<Finding> {
    header_failures(root, parameters, governed, tolerated)
        .into_iter()
        .map(|failure| failure.finding)
        .collect()
}

/// The governed paths that fail at least one half, counted once per file.
#[must_use]
pub fn violating_paths<'a>(
    root: &Path,
    parameters: &Parameters,
    governed: &'a [Governed],
) -> BTreeSet<&'a BytePath> {
    header_failures(root, parameters, governed, &BTreeSet::new())
        .into_iter()
        .map(|failure| failure.path)
        .collect()
}

/// One half-specific header failure, attached to its file-level identity.
struct HeaderFailure<'a> {
    /// The path whose file-level identity failed.
    path: &'a BytePath,
    /// The half-specific finding the ordinary verdict reports.
    finding: Finding,
}

/// Judge each governed header while preserving the file identity beside the finding.
fn header_failures<'a>(
    root: &Path,
    parameters: &Parameters,
    governed: &'a [Governed],
    tolerated: &BTreeSet<&BytePath>,
) -> Vec<HeaderFailure<'a>> {
    let mut failures = Vec::new();
    let mut by_path: BTreeMap<&'a BytePath, Vec<&Governed>> = BTreeMap::new();

    for entry in governed {
        by_path.entry(&entry.path).or_default().push(entry);
    }

    for (path, entries) in by_path {
        if tolerated.contains(path) {
            continue;
        }

        let Some(catalogued) = leader::catalogued(path) else {
            continue;
        };

        if is_link(root, path) {
            continue;
        }

        let Some(full) = under(root, path) else {
            continue;
        };

        let Ok(source) = std::fs::read(&full) else {
            continue;
        };

        let region = catalogued.region(&source);

        for entry in entries {
            // A governed file is reached by a partition row, and a partition row
            // carries the entry whose text it requires. The absent case is the
            // decoder's to refuse, not this pass's to guess at.
            let Some(carried) = entry.row.entry.as_deref() else {
                continue;
            };

            let Some(required) = parameters.text(entry.half, carried) else {
                continue;
            };

            let declared: Vec<&[u8]> = region
                .iter()
                .filter_map(|line| catalogued.declared(line, entry.half.field()))
                .collect();

            let missing = |unmatched: bool| HeaderFailure {
                path,
                finding: Finding::SpdxMissingHeader {
                    path: path.display(),
                    half: entry.half.as_str(),
                    unmatched,
                    owner: entry.owner.clone(),
                    name: entry.row.name.clone(),
                    required: required.to_owned(),
                },
            };

            match entry.half {
                Half::Identifier => match declared.as_slice() {
                    [] => failures.push(missing(false)),
                    [found] if *found != required.as_bytes() => failures.push(HeaderFailure {
                        path,
                        finding: Finding::SpdxWrongIdentifier {
                            path: path.display(),
                            owner: entry.owner.clone(),
                            name: entry.row.name.clone(),
                            required: required.to_owned(),
                            found: reversible(found),
                        },
                    }),
                    [_one] => {}
                    many => failures.push(HeaderFailure {
                        path,
                        finding: Finding::SpdxRepeatedIdentifier {
                            path: path.display(),
                            count: many.len(),
                        },
                    }),
                },
                Half::Copyright => {
                    if declared.is_empty() {
                        failures.push(missing(false));
                    } else if !declared.contains(&required.as_bytes()) {
                        failures.push(missing(true));
                    }
                }
            }
        }
    }

    failures
}

/// Whether a tracked entry is a symbolic link, which is never followed.
fn is_link(root: &Path, path: &BytePath) -> bool {
    under(root, path).is_some_and(|full| {
        full.symlink_metadata()
            .is_ok_and(|data| data.file_type().is_symlink())
    })
}

/// The filesystem path a tracked entry stands at, without decoding its bytes.
///
/// Both platform arms keep one signature because the text-only arm can reject
/// undecodable bytes; on Unix every byte path maps directly, so this arm always
/// returns `Some`.
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use tempfile::TempDir;

    use super::{
        COPYRIGHT_FIELD, HALVES, Half, HalfSection, IDENTIFIER_FIELD, LISTS, ListKind, Parameters,
        Section, SectionRow, carriers, conform, govern, is_copyright_text, is_licence_expression,
    };
    use crate::declaration::AbnfPattern;
    use crate::finding::Finding;
    use crate::pattern::BytePath;

    /// The identifier text every fixture section requires.
    const IDENTIFIER: &str = "AGPL-3.0-only";

    /// The copyright text every fixture section requires.
    const COPYRIGHT: &str = "2026 Wild Sky Maker";

    /// The owner every fixture section belongs to.
    const OWNER: &str = "INDEX";

    /// One exclusion rule of one list.
    fn row(name: &str, pattern: &str) -> SectionRow {
        SectionRow::new(
            String::from(name),
            AbnfPattern::parse(pattern).expect("a compilable pattern"),
        )
    }

    /// One partition row: the region it names, and the entry that region carries.
    fn carries(name: &str, entry: &str, pattern: &str) -> SectionRow {
        SectionRow::carrying(
            String::from(name),
            String::from(entry),
            AbnfPattern::parse(pattern).expect("a compilable pattern"),
        )
    }

    /// One owner's section, each half given as its exclusion rules and its inclusion rows.
    fn section(
        identifier: (Vec<SectionRow>, Vec<SectionRow>),
        copyright: (Vec<SectionRow>, Vec<SectionRow>),
    ) -> Parameters {
        let sets =
            |name: &str, text: &str| BTreeMap::from([(String::from(name), String::from(text))]);

        Parameters::new(
            sets("agpl3only", IDENTIFIER),
            sets("ember2026", COPYRIGHT),
            BTreeMap::from([(
                String::from(OWNER),
                Section::new(
                    HalfSection::new(identifier.0, identifier.1),
                    HalfSection::new(copyright.0, copyright.1),
                ),
            )]),
        )
    }

    /// The fixture section both halves of the census's own configuration have: Rust only.
    fn rust_only() -> Parameters {
        section(
            (
                vec![row(
                    "non-rust",
                    "0*2VCHAR / *VCHAR ( %x21-72 / %x74-7E ) / *VCHAR ( %x21-71 / %x73-7E ) %s\"s\" / *VCHAR ( %x21-2D / %x2F-7E ) %s\"rs\"",
                )],
                vec![carries("code", "agpl3only", "*VCHAR")],
            ),
            (
                vec![row(
                    "non-rust",
                    "0*2VCHAR / *VCHAR ( %x21-72 / %x74-7E ) / *VCHAR ( %x21-71 / %x73-7E ) %s\"s\" / *VCHAR ( %x21-2D / %x2F-7E ) %s\"rs\"",
                )],
                vec![carries("code", "ember2026", "*VCHAR")],
            ),
        )
    }

    /// Decode a display into the path it stands for.
    fn path(display: &str) -> BytePath {
        BytePath::decode(display).expect("a decodable path")
    }

    /// Attribute every one of these paths to the fixture owner.
    fn accounted(paths: &[BytePath]) -> BTreeMap<&BytePath, &str> {
        paths.iter().map(|path| (path, OWNER)).collect()
    }

    /// The codes a run of findings carries, in the order it reports them.
    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(Finding::code).collect()
    }

    /// Write a tree of files under a temporary root, creating each parent.
    fn tree(files: &[(&str, &[u8])]) -> TempDir {
        let root = TempDir::new().expect("a temporary root");

        for (name, bytes) in files {
            let full = root.path().join(name);

            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).expect("a parent directory");
            }

            std::fs::write(full, bytes).expect("a file");
        }

        root
    }

    /// The header a conforming Rust source of this corpus opens with.
    fn headed(body: &str) -> Vec<u8> {
        format!("// {IDENTIFIER_FIELD}: {IDENTIFIER}\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\n\n{body}")
            .into_bytes()
    }

    /// Judge one tree against one declaration, over the paths it accounts.
    fn judge(
        root: &Path,
        parameters: &Parameters,
        paths: &[BytePath],
    ) -> (Vec<Finding>, Vec<Finding>) {
        let attributed = accounted(paths);
        let (governed, section_findings) = govern(parameters, &attributed);

        let mut configuration = section_findings;
        configuration.extend(carriers(root, &governed));

        (
            configuration,
            conform(root, parameters, &governed, &BTreeSet::new()),
        )
    }

    /// The licence policy's inclusion rows are constitutive, and the ruling that
    /// made the interchange and file-path include records a diagnostic gloss does
    /// not reach them. Here an inclusion row still selects: narrowing one removes
    /// a file from the governed set rather than leaving it governed under a gloss
    /// that failed, a half declaring no inclusion row governs nothing at all, and
    /// a surviving path no row reaches is ungoverned rather than merely unnamed.
    /// The scope of what may carry a licence is much wider than one carrier
    /// catalog, so the inclusion rows are the only thing that says which files
    /// this policy is about, and a repository cannot drop them.
    ///
    /// ´claim:spdx:an-inclusion-row-selects-and-is-no-gloss´
    /// ´test:unit:an-inclusion-row-selects-and-is-no-gloss´
    #[test]
    fn an_inclusion_row_selects_and_is_no_gloss() {
        let paths = [path("src/one.rs"), path("src/two.rs")];
        let attributed = accounted(&paths);

        // A broad row in each half governs both files, once per half.
        let broad = section(
            (
                Vec::new(),
                vec![carries("code", "agpl3only", "%s\"src/\" *VCHAR %s\".rs\"")],
            ),
            (
                Vec::new(),
                vec![carries(
                    "code",
                    "ember2026",
                    "%s\"src/\" *VCHAR %s\".rs\"",
                )],
            ),
        );
        let (governed, findings) = govern(&broad, &attributed);

        assert_eq!(governed.len(), 4, "{governed:?}");
        assert!(findings.is_empty(), "{findings:?}");

        // Narrowing each row to one file removes the other from the governed set.
        // That is the whole difference from a gloss: under the gloss ruling the
        // file would stay governed and only the declaration would fail.
        let narrow = section(
            (
                Vec::new(),
                vec![carries("code", "agpl3only", "%s\"src/one.rs\"")],
            ),
            (
                Vec::new(),
                vec![carries("code", "ember2026", "%s\"src/one.rs\"")],
            ),
        );
        let (governed, findings) = govern(&narrow, &attributed);

        assert_eq!(governed.len(), 2, "{governed:?}");
        assert!(
            governed
                .iter()
                .all(|entry| entry.path.display() == "src/one.rs"),
            "the narrowed row governs one file and the other left the set: {governed:?}"
        );
        assert_eq!(
            codes(&findings),
            ["spdx_ungoverned_path", "spdx_ungoverned_path"]
        );

        // A half declaring no inclusion row governs nothing, and every accounted
        // path in the share is ungoverned by name. An empty inclusion list here
        // is not the absence of a claim, as it is on the two parameterless
        // policies; it is a section that selects nothing.
        let none = section((Vec::new(), Vec::new()), (Vec::new(), Vec::new()));
        let (governed, findings) = govern(&none, &attributed);

        assert!(governed.is_empty(), "{governed:?}");
        assert_eq!(
            codes(&findings),
            [
                "spdx_ungoverned_path",
                "spdx_ungoverned_path",
                "spdx_ungoverned_path",
                "spdx_ungoverned_path"
            ]
        );
    }

    /// Each half removes the union of its exclusion rules first and then
    /// partitions what survives, totally and exclusively. A surviving path no
    /// inclusion row reaches breaks totality by name, and one two rows reach
    /// breaks exclusivity even where both rows name the same set entry — so an
    /// exception is carved by writing disjoint rows and never by shadowing a
    /// broad row with a narrow one.
    ///
    /// ´claim:spdx:a-half-excludes-then-partitions-what-survives´
    /// ´test:unit:a-half-excludes-then-partitions-what-survives´
    #[test]
    fn a_half_excludes_then_partitions_what_survives() {
        let paths = [
            path("src/one.rs"),
            path("src/notes.md"),
            path("src/logo.png"),
        ];
        let attributed = accounted(&paths);
        let parameters = rust_only();
        let (governed, findings) = govern(&parameters, &attributed);

        // The Rust source is governed by both halves; the prose and image are
        // excluded by both. This is the live configuration's one-cell
        // partition, transcribed without an owner-directory enumeration.
        assert_eq!(governed.len(), 2, "{governed:?}");
        assert!(
            governed
                .iter()
                .all(|entry| entry.path.display() == "src/one.rs")
        );
        assert!(findings.is_empty(), "{findings:?}");

        // A surviving path no inclusion row reaches still breaks totality.
        let incomplete = section(
            (
                vec![row(
                    "non-rust",
                    "0*2VCHAR / *VCHAR ( %x21-72 / %x74-7E ) / *VCHAR ( %x21-71 / %x73-7E ) %s\"s\" / *VCHAR ( %x21-2D / %x2F-7E ) %s\"rs\"",
                )],
                Vec::new(),
            ),
            (
                vec![row(
                    "non-rust",
                    "0*2VCHAR / *VCHAR ( %x21-72 / %x74-7E ) / *VCHAR ( %x21-71 / %x73-7E ) %s\"s\" / *VCHAR ( %x21-2D / %x2F-7E ) %s\"rs\"",
                )],
                vec![carries("code", "ember2026", "*VCHAR")],
            ),
        );
        let paths = [path("src/one.rs"), path("src/notes.md")];
        let attributed = accounted(&paths);
        let (governed, findings) = govern(&incomplete, &attributed);

        assert_eq!(governed.len(), 1, "{governed:?}");
        assert_eq!(codes(&findings), ["spdx_ungoverned_path"]);
        assert_eq!(
            findings[0].to_string(),
            "spdx section: src/one.rs: accounted to INDEX, excluded by no identifier row \
             and matched by no identifier partitions row"
        );

        // Two rows reaching one path fail exclusivity even where both name one
        // set entry, and the overlap between an exclusion rule and an inclusion
        // row is legal because an excluded path is never evaluated against the
        // inclusion list at all.
        let shadowed = section(
            (
                vec![row("prose", "%s\"src\" [ \"/\" *VCHAR ] %s\".md\"")],
                vec![
                    carries("code", "agpl3only", "%s\"src\" [ \"/\" *VCHAR ] %s\".rs\""),
                    carries("one-source", "agpl3only", "%s\"src/one.rs\""),
                ],
            ),
            (
                vec![row("everything", "%s\"src\" [ \"/\" *VCHAR ]")],
                Vec::new(),
            ),
        );
        let paths = [path("src/one.rs")];
        let attributed = accounted(&paths);
        let (governed, findings) = govern(&shadowed, &attributed);

        assert!(governed.is_empty(), "{governed:?}");
        assert_eq!(
            codes(&findings),
            ["spdx_multiply_included", "spdx_idle_row"]
        );
        assert_eq!(
            findings[0].to_string(),
            "spdx section: src/one.rs: matched by 2 INDEX identifier partitions rows: \
             code : %s\"src\" [ \"/\" *VCHAR ] %s\".rs\", one-source : %s\"src/one.rs\""
        );
    }

    /// The two halves divide the owner's share independently, so a file may be
    /// governed by one and excused by the other. That is the vendored case the
    /// per-half exclusion lists were ruled for: a third-party source must declare
    /// the licence it arrived under, and this repository may not put its
    /// copyright on it, and no single shared exclusion list could say both.
    ///
    /// ´claim:spdx:a-file-is-governed-by-one-half-and-excused-by-the-other´
    /// ´test:unit:a-file-is-governed-by-one-half-and-excused-by-the-other´
    #[test]
    fn a_file_is_governed_by_one_half_and_excused_by_the_other() {
        let vendored = section(
            (
                Vec::new(),
                vec![carries(
                    "code",
                    "agpl3only",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".rs\"",
                )],
            ),
            (
                vec![row(
                    "vendored",
                    "%s\"src/third-party\" [ \"/\" *VCHAR ] %s\".rs\"",
                )],
                vec![carries(
                    "code",
                    "ember2026",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".rs\"",
                )],
            ),
        );

        let paths = [path("src/third-party/shim.rs")];
        let attributed = accounted(&paths);
        let (governed, findings) = govern(&vendored, &attributed);

        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(governed.len(), 1, "{governed:?}");
        assert_eq!(
            governed[0].half,
            Half::Identifier,
            "the identifier half governs it"
        );

        // And the file conforms by carrying that half's line and nothing else,
        // which is exactly what a shared exclusion list could not have expressed.
        let root = tree(&[(
            "src/third-party/shim.rs",
            format!("// {IDENTIFIER_FIELD}: {IDENTIFIER}\n").as_bytes(),
        )]);

        assert_eq!(
            conform(root.path(), &vendored, &governed, &BTreeSet::new()),
            Vec::<Finding>::new()
        );
    }

    /// A row is offered only paths attributed to its owner. A row whose pattern
    /// would match another owner's path is idle if it reaches none in its share:
    /// containment is impossible by construction, while dead rows remain named.
    ///
    /// ´claim:spdx:a-row-is-only-offered-its-owners-share´
    /// ´test:unit:a-row-is-only-offered-its-owners-share´
    #[test]
    fn a_row_is_only_offered_its_owners_share() {
        let reaching = section(
            (
                vec![row("elsewhere", "%s\"packages\" [ \"/\" *VCHAR ]")],
                vec![carries("code", "agpl3only", "*VCHAR")],
            ),
            (Vec::new(), vec![carries("code", "ember2026", "*VCHAR")]),
        );

        let ours = path("src/one.rs");
        let theirs = path("packages/assayer/src/lib.rs");
        let attributed = BTreeMap::from([(&ours, OWNER), (&theirs, "ASSAYER")]);

        let (governed, findings) = govern(&reaching, &attributed);

        assert_eq!(governed.len(), 2, "{governed:?}");
        assert!(governed.iter().all(|entry| entry.path == ours));
        assert_eq!(codes(&findings), ["spdx_idle_row"]);
        assert_eq!(
            findings[0].to_string(),
            "spdx section: INDEX identifier exclude row elsewhere: pattern matches no accounted path"
        );

        // A row reaching no accounted path at all is a dead row, and reporting it
        // is what keeps a section from quietly carrying a rule that does nothing.
        let idle = section(
            (
                Vec::new(),
                vec![carries(
                    "code",
                    "agpl3only",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".rs\"",
                )],
            ),
            (
                vec![row("nowhere", "%s\"gone\" [ \"/\" *VCHAR ]")],
                vec![carries(
                    "code",
                    "ember2026",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".rs\"",
                )],
            ),
        );
        let paths = [path("src/one.rs")];
        let attributed = accounted(&paths);
        let (_governed, findings) = govern(&idle, &attributed);

        assert_eq!(codes(&findings), ["spdx_idle_row"]);
        assert_eq!(
            findings[0].to_string(),
            "spdx section: INDEX copyright exclude row nowhere: pattern matches no accounted path"
        );
    }

    /// A path whose bytes are not text is matched by no row, including a row
    /// spelled to match everything, so it is unnameable by any row the surface
    /// can write. The failure is therefore loud rather than silent: the path is
    /// accounted like any other, so matching no inclusion row breaks totality by
    /// name instead of quietly leaving the governed set.
    ///
    /// ´claim:spdx:a-path-that-is-not-text-is-unnameable-by-any-row´
    /// ´test:unit:a-path-that-is-not-text-is-unnameable-by-any-row´
    #[test]
    fn a_path_that_is_not_text_is_unnameable_by_any_row() {
        let everything = section(
            (Vec::new(), vec![carries("code", "agpl3only", "*VCHAR")]),
            (vec![row("everything", "*VCHAR")], Vec::new()),
        );

        let awkward = BytePath::from_bytes(b"src/od\xffd.rs".to_vec()).expect("an awkward path");
        let attributed = BTreeMap::from([(&awkward, OWNER)]);

        let (governed, findings) = govern(&everything, &attributed);

        assert!(governed.is_empty(), "no row reaches it: {governed:?}");
        assert_eq!(
            codes(&findings),
            [
                "spdx_ungoverned_path",
                "spdx_idle_row",
                "spdx_ungoverned_path",
                "spdx_idle_row"
            ],
            "{findings:?}"
        );
        assert_eq!(
            findings[0].to_string(),
            "spdx section: src/od%FFd.rs: accounted to INDEX, excluded by no identifier row \
             and matched by no identifier partitions row",
            "the path is named in the reversible display rather than lost to a decoding"
        );
    }

    /// A governed entry that can never carry a header is named at configuration
    /// time rather than failed forever: a symbolic link has no content of its own
    /// to head, and a file of a type no comment leader is catalogued for has
    /// nowhere to put one. Nothing is removed implicitly, so the remedy in both
    /// cases is an exclusion row somebody writes.
    ///
    /// ´claim:spdx:an-entry-that-can-never-head-is-named-before-it-is-read´
    /// ´test:unit:an-entry-that-can-never-head-is-named-before-it-is-read´
    #[cfg(unix)]
    #[test]
    fn an_entry_that_can_never_head_is_named_before_it_is_read() {
        let everything = section(
            (
                Vec::new(),
                vec![carries("code", "agpl3only", "%s\"src\" [ \"/\" *VCHAR ]")],
            ),
            (
                vec![row("everything", "%s\"src\" [ \"/\" *VCHAR ]")],
                Vec::new(),
            ),
        );

        let root = tree(&[
            ("src/one.rs", b"code();\n"),
            ("src/logo.png", b"\x89PNG\r\n"),
        ]);
        std::os::unix::fs::symlink("one.rs", root.path().join("src/link.rs"))
            .expect("a symbolic link");

        let paths = [path("src/link.rs"), path("src/logo.png")];
        let (configuration, headers) = judge(root.path(), &everything, &paths);

        assert_eq!(
            codes(&configuration),
            ["spdx_linked_path", "spdx_uncatalogued_type"],
            "{configuration:?}"
        );
        assert_eq!(
            configuration[0].to_string(),
            "spdx section: src/link.rs: governed by INDEX identifier partitions row code; \
             a symbolic link carries no header"
        );
        assert_eq!(
            configuration[1].to_string(),
            "spdx section: src/logo.png: governed by INDEX identifier partitions row code; \
             no comment leader is catalogued for this file"
        );

        // Neither is read for a header, because the pass that named them has
        // already said they can never carry one.
        assert!(headers.is_empty(), "{headers:?}");
    }

    /// A header is read at the front of the file and nowhere else. The two header
    /// lines standing inside a string literal are outside the region, so the file
    /// that carries them as a fixture correctly lacks a header where a whole-file
    /// search would report one; and an interpreter line moves the region down
    /// one, so the corpus's headed script conforms.
    ///
    /// ´claim:spdx:a-header-is-read-at-the-front-of-the-file-and-nowhere-else´
    /// ´test:unit:a-header-is-read-at-the-front-of-the-file-and-nowhere-else´
    #[test]
    fn a_header_is_read_at_the_front_of_the_file_and_nowhere_else() {
        // The census's own overcounted file: this crate's index generator carries
        // both header lines inside a string literal as the fixture for its own
        // header detection, and a whole-file search reports a header it does not
        // carry.
        let literal = format!(
            "//! The generator's own documentation.\nfn header() -> &'static str {{\n    \
             \"// {IDENTIFIER_FIELD}: {IDENTIFIER}\\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\\n\"\n}}\n"
        );

        let root = tree(&[
            ("src/literal.rs", literal.as_bytes()),
            ("src/headed.rs", &headed("fn one() {}\n")),
        ]);

        // The prose path stands in the accounting so that the fixture section's
        // exclusion rule reaches something and is not reported as a dead row.
        let paths = [
            path("src/literal.rs"),
            path("src/headed.rs"),
            path("src/notes.md"),
        ];
        let (configuration, headers) = judge(root.path(), &rust_only(), &paths);

        assert!(configuration.is_empty(), "{configuration:?}");
        assert_eq!(
            codes(&headers),
            ["spdx_missing_header", "spdx_missing_header"],
            "the headed file is silent and the literal one is not: {headers:?}"
        );
        assert_eq!(
            headers[0].to_string(),
            format!(
                "spdx identifier: src/literal.rs: no {IDENTIFIER_FIELD} header; INDEX row code requires {IDENTIFIER}"
            )
        );
        assert_eq!(
            headers[1].to_string(),
            format!(
                "spdx copyright: src/literal.rs: no {COPYRIGHT_FIELD} header; INDEX row code requires {COPYRIGHT}"
            )
        );

        // And the shebang case, measured from the corpus's one headed script: the
        // two header lines stand at the second and third lines.
        let scripts = section(
            (
                Vec::new(),
                vec![carries(
                    "code",
                    "agpl3only",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".sh\"",
                )],
            ),
            (
                Vec::new(),
                vec![carries(
                    "code",
                    "ember2026",
                    "%s\"src\" [ \"/\" *VCHAR ] %s\".sh\"",
                )],
            ),
        );
        let script = format!(
            "#!/usr/bin/env bash\n# {IDENTIFIER_FIELD}: {IDENTIFIER}\n# {COPYRIGHT_FIELD}: {COPYRIGHT}\n\nset -e\n"
        );
        let root = tree(&[("src/one.sh", script.as_bytes())]);
        let paths = [path("src/one.sh")];
        let (configuration, headers) = judge(root.path(), &scripts, &paths);

        assert!(configuration.is_empty(), "{configuration:?}");
        assert!(
            headers.is_empty(),
            "the script conforms under its interpreter line: {headers:?}"
        );
    }

    /// The two fields are not symmetric and the asymmetry is deliberate. A file
    /// has one licence, so exactly one region line carries the identifier and two
    /// fail whether or not the texts agree. A file may have many copyright
    /// holders, so at least one line must carry the required text and further
    /// copyright lines are permitted and never examined — requiring a sole
    /// copyright line would forbid a contributor from adding their own.
    ///
    /// ´claim:spdx:the-identifier-is-exactly-one-and-the-copyright-at-least-one´
    /// ´test:unit:the-identifier-is-exactly-one-and-the-copyright-at-least-one´
    #[test]
    fn the_identifier_is_exactly_one_and_the_copyright_at_least_one() {
        let wrong = format!(
            "// {IDENTIFIER_FIELD}: MIT\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\n\nfn one() {{}}\n"
        );
        let twice = format!(
            "// {IDENTIFIER_FIELD}: {IDENTIFIER}\n// {IDENTIFIER_FIELD}: MIT\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\n"
        );
        let others = format!(
            "// {IDENTIFIER_FIELD}: {IDENTIFIER}\n// {COPYRIGHT_FIELD}: 1998 Somebody Else\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\n"
        );
        let unmatched = format!(
            "// {IDENTIFIER_FIELD}: {IDENTIFIER}\n// {COPYRIGHT_FIELD}: 1998 Somebody Else\n"
        );

        let root = tree(&[
            ("src/wrong.rs", wrong.as_bytes()),
            ("src/twice.rs", twice.as_bytes()),
            ("src/others.rs", others.as_bytes()),
            ("src/unmatched.rs", unmatched.as_bytes()),
        ]);

        let paths = [
            path("src/wrong.rs"),
            path("src/twice.rs"),
            path("src/others.rs"),
            path("src/unmatched.rs"),
            path("src/notes.md"),
        ];
        let (configuration, headers) = judge(root.path(), &rust_only(), &paths);

        assert!(configuration.is_empty(), "{configuration:?}");

        // The file carrying another holder's line beside the required one is
        // silent, which is the whole of the copyright asymmetry.
        assert_eq!(
            codes(&headers),
            [
                "spdx_repeated_identifier",
                "spdx_missing_header",
                "spdx_wrong_identifier"
            ],
            "{headers:?}"
        );
        assert_eq!(
            headers[0].to_string(),
            format!(
                "spdx identifier: src/twice.rs: 2 {IDENTIFIER_FIELD} headers; a file declares one licence"
            )
        );
        assert_eq!(
            headers[1].to_string(),
            format!(
                "spdx copyright: src/unmatched.rs: no {COPYRIGHT_FIELD} header matches; \
                 INDEX row code requires {COPYRIGHT}"
            )
        );
        assert_eq!(
            headers[2].to_string(),
            format!(
                "spdx identifier: src/wrong.rs: header declares MIT; INDEX row code requires {IDENTIFIER}"
            )
        );
    }

    /// A listed path is silent however it fails. The ruling is uniform
    /// toleration: a row carries a file that fails, whether the required header
    /// is absent or wrong, so the list keeps the one meaning a reviewer reads it
    /// as and a repository that discovers a wrong header needs no second
    /// mechanism to hold it while it is corrected. The configuration findings are
    /// not tolerated, because a burn list cannot excuse a section that does not
    /// describe the repository.
    ///
    /// ´claim:spdx:a-listed-path-tolerates-a-wrong-header-as-an-absent-one´
    /// ´test:unit:a-listed-path-tolerates-a-wrong-header-as-an-absent-one´
    #[test]
    fn a_listed_path_tolerates_a_wrong_header_as_an_absent_one() {
        let wrong = format!("// {IDENTIFIER_FIELD}: MIT\n// {COPYRIGHT_FIELD}: {COPYRIGHT}\n");
        let root = tree(&[
            ("src/absent.rs", b"fn one() {}\n"),
            ("src/wrong.rs", wrong.as_bytes()),
            ("src/notes.md", b"# Notes\n"),
        ]);

        let paths = [
            path("src/absent.rs"),
            path("src/wrong.rs"),
            path("src/notes.md"),
        ];
        let attributed = accounted(&paths);
        let parameters = rust_only();
        let (governed, findings) = govern(&parameters, &attributed);

        assert!(findings.is_empty(), "{findings:?}");

        // Unlisted, the two files fail in the two ways a row tolerates.
        let none = BTreeSet::new();
        assert_eq!(
            codes(&conform(root.path(), &parameters, &governed, &none)),
            [
                "spdx_missing_header",
                "spdx_missing_header",
                "spdx_wrong_identifier"
            ]
        );

        // Listed, both are silent, and the one that is wrong is as silent as the
        // one that is absent.
        let listed: BTreeSet<&BytePath> = paths.iter().take(2).collect();

        assert_eq!(
            conform(root.path(), &parameters, &governed, &listed),
            Vec::<Finding>::new()
        );
    }

    /// The half is what selects both the set a row's name resolves in and the
    /// field a governed file must carry, so a row needs no third component to
    /// say which requirement it states and the two halves can never be crossed.
    ///
    /// ´claim:spdx:the-half-selects-the-set-and-the-field´
    /// ´test:unit:the-half-selects-the-set-and-the-field´
    #[test]
    fn the_half_selects_the_set_and_the_field() {
        assert_eq!(Half::Identifier.field(), IDENTIFIER_FIELD);
        assert_eq!(Half::Copyright.field(), COPYRIGHT_FIELD);

        assert_eq!(Half::Identifier.to_string(), "identifier");
        assert_eq!(Half::Copyright.to_string(), "copyright");
        assert_eq!(ListKind::Exclude.to_string(), "exclude");
        assert_eq!(ListKind::Partition.to_string(), "partitions");

        // Both halves and both lists are enumerable, because every rule the
        // section states is stated of each in turn and a rule that reached only
        // one of them would be a rule with a silent exception.
        assert_eq!(HALVES.len(), 2);
        assert_eq!(LISTS.len(), 2);
    }

    /// The identifier text is held to the licence-expression syntax entire —
    /// disjunction, conjunction, the exception operator, parentheses and the
    /// later-version mark — and never to membership in a published list. A list
    /// check would reject the fictional expression the governing ruling was
    /// written with, and would oblige this binary to carry and version a
    /// registry that ages between releases.
    ///
    /// ´claim:spdx:an-identifier-is-held-to-a-syntax-and-not-to-a-list´
    /// ´test:unit:an-identifier-is-held-to-a-syntax-and-not-to-a-list´
    #[test]
    fn an_identifier_is_held_to_a_syntax_and_not_to_a_list() {
        for text in [
            "AGPL-3.0-only",
            "MIT",
            "MIT OR Apache-2.0",
            "GPL-2.0-only WITH Classpath-exception-2.0",
            "GPL-2.0-or-later AND (MIT OR Apache-2.0)",
            "(MIT OR Apache-2.0) AND ISC",
            "LicenseRef-Proprietary",
            "GPL-3.0+",
        ] {
            assert!(
                is_licence_expression(text),
                "{text} is a well-formed expression"
            );
        }

        // The decisive case: the ruling that commissioned this design was
        // written with a licence no published list holds, and a list check would
        // have rejected the very example it was ruled with.
        assert!(is_licence_expression("AGPL-5.0+"));
    }

    /// A text the syntax does not admit is refused: an empty text, an operator
    /// that is not surrounded by exactly one space, an unclosed parenthesis, a
    /// token opening with a separator, and a token ending in one. The spacing is
    /// part of the syntax because a surface admitting two spellings of one
    /// expression is a surface on which the repository has not actually fixed
    /// its licence.
    ///
    /// ´claim:spdx:an-identifier-outside-the-syntax-is-refused´
    /// ´test:unit:an-identifier-outside-the-syntax-is-refused´
    #[test]
    fn an_identifier_outside_the_syntax_is_refused() {
        for text in [
            "",
            " MIT",
            "MIT ",
            "MIT  OR  Apache-2.0",
            "MIT or Apache-2.0",
            "MIT OR",
            "(MIT OR Apache-2.0",
            "MIT OR Apache-2.0)",
            "-MIT",
            "MIT-",
            "MIT.",
            "MIT WITH",
            "MIT+ +",
            "Apache 2.0",
        ] {
            assert!(
                !is_licence_expression(text),
                "{text:?} is not a well-formed expression"
            );
        }
    }

    /// The copyright text is a nonempty string with no edge whitespace and no
    /// line break, and nothing further is required of it: a year and a holder
    /// are prose, and a rule that constrained them would constrain a
    /// jurisdiction rather than a syntax.
    ///
    /// ´claim:spdx:a-copyright-text-is-a-line-and-not-a-form´
    /// ´test:unit:a-copyright-text-is-a-line-and-not-a-form´
    #[test]
    fn a_copyright_text_is_a_line_and_not_a_form() {
        for text in [
            "2026 Wild Sky Maker",
            "Wild Sky Maker",
            "2120 Auto Dev Collective",
            "1998-2026 Somebody Else, and others",
        ] {
            assert!(
                is_copyright_text(text),
                "{text} is a well-formed copyright line"
            );
        }

        for text in [
            "",
            " 2026 Wild Sky Maker",
            "2026 Wild Sky Maker ",
            "2026\nWild Sky Maker",
            "2026 Wild Sky Maker\r",
        ] {
            assert!(
                !is_copyright_text(text),
                "{text:?} is not a well-formed copyright line"
            );
        }
    }
}
