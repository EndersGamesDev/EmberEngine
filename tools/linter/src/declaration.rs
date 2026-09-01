// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The ratified declaration grammar: one envelope, named sets, and three owner
//! selection shapes.
//!
//! An audit of the declared surface found five schema dialects living beside one
//! another, and the owner's review closed on one grammar for the whole
//! configuration surface (´gram:isolation:declaration´). This module reads one
//! instance document written in that grammar into a typed value. It decodes and
//! nothing else: no document reaches it from the loader, no verdict changes
//! because it exists, and the questions that are about several documents at once
//! are asked where the union is formed.
//!
//! # The namespace is the whole identity, which is why two keys die
//!
//! A document carries a dotted `namespace` and a `version`, and the namespace
//! alone says which instance it is. Two payloads merge into one document exactly
//! when one program acts on both, so the three prefix-number payloads become one
//! document of three named entries while the marked-ordinal and literal-set
//! payloads keep documents of their own. The `policy` and `family` keys have no
//! remaining work: the first restated what the namespace already fixes, and the
//! second discriminated instances that are now named entries of one set.
//!
//! # An entry is named, and a name is what a report can say
//!
//! Data stands in singular `[set.TYPE]` tables whose every entry is named, its
//! value a string where the entry is simple and an inline table where it is
//! parameterized. The name is not decoration. It is what an owner's selection
//! references, what a ratchet key ends in (´conv:isolation:registers´), and what
//! a report says when an entry is at fault — and a positional list has none of
//! the three, which is why the retiring documents could only say that some value
//! of some list was involved.
//!
//! A type this binary catalogues no program for still decodes: its entries are
//! kept by name and the set is marked unadopted. Refusing it here would make this
//! decoder the adoption authority, and adoption is an owner act recorded
//! elsewhere (ADR-T-023, Adopting the interchange conventions for first-party structured configuration).
//!
//! # Three owner shapes, told apart by the keys they carry
//!
//! Repository choices attach to owners in exactly three shapes: pattern rows
//! under `[owners.OWNER.TYPE]`, a singular `use` key naming one entry where
//! exactly one applies, and owner-bound data on the naked `[owners.OWNER]` table
//! for content with no set behind it. A reader tells a typed section from an
//! owner datum by the keys the table carries rather than by how it was written,
//! because a subtable and an inline table are one thing to a decoder: a table
//! carrying `use`, `partitions`, `include` or `exclude` is a selection, and any
//! other table is a datum. So the manifest key and a publication row sit directly
//! on the naked table, and both shapes stand for one owner at once.
//!
//! # A row names its own region, and spells a reference under the set's type
//!
//! Every row carries a `name`, and the name is the row's own: it says which
//! region of the owner's share the row is about, and it is what a report says
//! when the row is at fault. A row admitting sources carries a second
//! declaration beside it — which entry of the section's set that region carries —
//! and spells it under the name of the set the reference is into, so an admitted
//! row is `{ name, TYPE, pattern }` and an exclusion is `{ name, pattern }`. A
//! licence half writes `{ name = "code", identifier = "agpl3only", pattern = … }`
//! and the two names being different is the point rather than an oddity: `code`
//! is the region the half partitions and `agpl3only` is the entry it carries
//! there, and a grammar spelling both under one word could say only one of them.
//!
//! The key is the set's type rather than a fixed word because that is what says
//! which table the value is read against. A reader meeting `identifier =
//! "agpl3only"` knows where to look it up without first learning which relation
//! admitted the row, and the singular `use` key is the one reference that needs
//! no such help: it stands alone in its section and its section is already
//! typed.
//!
//! A reference resolves against that type's set and a dangling one is a defect,
//! while a name promises nothing about any set. That asymmetry is the reason an
//! exclude-only section over a type no set declares still decodes: an exclusion
//! never made a promise about a set to break.
//!
//! Names repeat where regions repeat. A partition's rows name its parts, so two
//! parts answering to one name is two answers to which part a report means and
//! refuses. A deployment's rows name regions that overlap by construction —
//! twelve division literals stand over one region of one package — so a name
//! standing twice there is the document saying one true thing twice, and
//! inventing a distinction to tell the rows apart would be a distinction the
//! document does not make. What is unique under either word is the reference:
//! where one entry reaches is one question, and two rows carrying it are two
//! answers.
//!
//! # Two words admit rows, and the entries decide which one is lawful
//!
//! A section writes its admitted rows under `partitions` where the entries
//! deployed divide the owner's share totally and exclusively, and under
//! `include` where they are deployed over reaches that may overlap. The two are
//! one relation in shape and two in meaning, and the meaning belongs to the set
//! type rather than to the document that writes it: each licence half partitions
//! what its exclusions leave, while twelve division literals deliberately share
//! one region. So the word is fixed by the type, and a section writing the other
//! one refuses here naming the word its entries call for — a selection that
//! decoded under either would carry the meaning of whichever the author happened
//! to reach for.
//!
//! A type no program of this binary reads takes either word. Fixing one would be
//! deciding what that type's entries do, which is the authority this decoder
//! already declines over adoption itself.
//!
//! # Every pattern is the augmented Backus-Naur form, and the value is one rule
//!
//! One pattern schema spans the surface and it is RFC 5234
//! (´lang:isolation:patterns´). A declared value is the right-hand side of a
//! single rule — an example row writes ``"handbook" [ "/" *VCHAR ]``
//! and nothing else — so the decoder names that rule [`PATTERN_RULE`], parses
//! the pair as a rule list over the core rules, and matches against that name. A
//! value wanting a helper rule writes it on a continuation line, which the
//! engine's own reading of an indented rule already admits, and no second
//! convention is needed for it. The form has no captures and the designs above
//! decompose rather than reach for one, so nothing here hands a fragment back.
//!
//! A pattern the engine declines carries the engine's defect inside the
//! declaration's, because the position the form stopped at is where the
//! declaration is repaired. Regular expressions are not a second accepted
//! dialect: a regular expression that happens to parse as the form means what
//! the form says it means, and one that does not is refused where it stands.
//!
//! # A retired key refuses by name rather than being ignored
//!
//! The keys the grammar retired are refused with what replaced them, because a
//! decoder that ignored them would let a pre-grammar document load while half of
//! what it declared went unread. That is the one failure this wave cannot have:
//! a `[scope]` block silently dropped is an instance censusing a corpus its
//! author bounded.
//!
//! # What this module is not yet
//!
//! One document at a time. Whether two documents stamp one namespace, whether an
//! owner is registered, whether a set type is adopted, and whether a selection's
//! program exists are questions about the union or the catalogue, and each is
//! asked where its authority stands (´proposal:isolation:one-plan´).
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`decodes_the_ratified_owner_name_document`] | declaration | The owner-name document the owner ratified entry by entry decodes whole: two sets of one named entry each, a naked owner table carrying the manifest the owner's names are read from, and one singular reference per set resolving to its entry. Both shapes stand for one owner at once, which is what the naked table exists to allow. |
//! | [`merges_one_program_s_payloads_into_one_document_of_named_entries`] | declaration | The three prefix-number payloads decode as three named entries of one set, each carrying the bound its own scheme had, and an owner's section selects among them by name. The merge is what the namespace ruling requires — one program acting on the sets is one document — and it repairs the collision of three files stamping one label between them. |
//! | [`decodes_a_parameterized_entry_into_the_program_s_own_value`] | declaration | A parameterized entry decodes into the value its program consumes rather than into a parallel shape holding the same numbers, and the compiled half of the program is not declarable beside it. A second shape carrying one instance's mark and bound would be a second chance to disagree about what the bound meant. |
//! | [`decodes_a_simple_entry_and_the_pattern_rows_that_deploy_it`] | declaration | A simple entry is a named string and an owner deploys it through pattern rows, the include row naming its own region and the entry that region carries, and the exclude row naming the exclusion itself and carrying nothing. The pattern is the form and not a regular expression, and it decides a path over the whole path rather than over a prefix of it. |
//! | [`decodes_the_two_licence_halves_as_named_entries_of_two_sets`] | declaration | The licence-header precedent decodes under the one grammar: two sets of named strings, and one typed section per half per owner. It is the shape every entry-across-files declaration takes, so the document that established it needs no dialect of its own. |
//! | [`decodes_publications_as_named_tables_on_the_naked_owner_table`] | declaration | A publication is a named inline table on the naked owner table, typed by the document's namespace and attributed by the table it stands on, so the retired `owner` field has no successor and no set stands behind it. |
//! | [`refuses_every_key_the_ratified_grammar_retired`] | declaration | Each retired key refuses by name and says what replaced it: the two envelope keys the namespace absorbed, the parameter table the named sets replaced, the scope and register blocks the owner sections and the register retirement left with no work, the program-owned list that was never repository data, the owner document's exclusion key the shape document already holds, and the plural spelling of the set table. Ignoring one would let a pre-grammar document load with half of what it declared unread. |
//! | [`refuses_a_reference_to_an_entry_no_set_declares`] | declaration | An include row and a singular reference both name an entry of their section's set, so a name resolving to nothing is a defect naming the owner, the type and the name. An exclude row is not held to it, because its name labels the exclusion rather than promising an entry. |
//! | [`refuses_the_row_spelling_the_ruling_replaced`] | declaration | A row carries two declarations the retiring spelling wrote as one, and each half missing refuses saying what is owed. A row with no name is a region no report can say; a row admitting sources with no reference under its set's own type is the retired spelling, where the reference stood as the name; and two rows carrying one entry are two answers to where that entry reaches. An author who wrote any of the three was writing what was lawful before the ruling, so what they are owed is the sentence saying which word carries the declaration now rather than a report that some key was unknown. |
//! | [`refuses_the_admitting_word_a_section_s_entries_do_not_call_for`] | declaration | A section admits its rows under the word its set type calls for: `partitions` where the entries divide what the exclusions left, `include` where they are deployed over reaches that may overlap. Each wrong direction refuses naming the word that was owed, because the two are one relation in shape and a decoder taking either would carry whichever promise the author happened to reach for. A type this binary reads no program for takes either word, and refuses only both at once. |
//! | [`refuses_a_singular_reference_standing_beside_pattern_rows`] | declaration | A section states one selection shape. A `use` key beside include rows is two answers to one question, and picking either would make an owner's reach depend on which the decoder happened to read first. An exclusion is not a second answer to it — it says what is cut out of whatever was selected — so it composes with either shape rather than conflicting with one. |
//! | [`decodes_a_section_naming_the_one_entry_it_deploys`] | declaration | A section deploys one entry by naming it, and the name promises an entry. One spelling and no second one: the list spelling retired with the owner-areas experiment that wanted it, so a document writing a list of names is refused rather than read as a deployment the grammar no longer has. A name resolving to no entry refuses where it is written, which is what keeps a misspelt entry name from becoming a selection of nothing. |
//! | [`refuses_a_pattern_the_form_does_not_read`] | declaration | A pattern the engine will not compile refuses where it is written and carries the engine's own defect inside, so the position the form stopped at reaches the reader. A reference to a rule nothing defines refuses the same way, which is what keeps a misspelt rule name from becoming a bound that accounts nothing. |
//! | [`keeps_a_set_no_program_claims_and_marks_it_unadopted`] | declaration | A set type this binary catalogues no program for decodes structurally with its entries kept by name and the set marked unadopted. Refusing it would make the decoder the adoption authority, and adoption is an owner act recorded outside configuration. |
//! | [`requires_the_envelope_that_identifies_a_document`] | declaration | A document identifies itself with a dotted namespace and a version, and a missing or malformed envelope refuses before anything reads what it declares. A key outside the grammar refuses too: an unread key is a declaration whose author believes something the checker never learnt. |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::abnf::{Branch, Grammar, GrammarDefect};
use crate::assembly::Publication;
use crate::catalogue::{CATALOG, is_declared_name};
use crate::pattern::BytePath;
use crate::program::{MarkNumbered, PrefixBound};

/// The rule a declared pattern's value is read as the right-hand side of.
///
/// The value is one rule rather than a rule list, so the decoder supplies the
/// name and the engine's whole-input anchoring then decides the path over the
/// path entire.
pub const PATTERN_RULE: &str = "pattern";

/// The table the data of a document stands in, in the ratified singular.
const SET_TABLE: &str = "set";

/// The table the owner sections stand in.
const OWNERS_TABLE: &str = "owners";

/// The key naming the one entry a singular reference selects.
const USE_KEY: &str = "use";

/// The relation admitting sources into a selection whose entries partition the share.
const PARTITIONS_KEY: &str = "partitions";

/// The relation admitting sources into a selection whose entries are deployed over it.
const INCLUDE_KEY: &str = "include";

/// The two words a section may admit its rows under, whichever its entries call for.
const ADMISSION_KEYS: [&str; 2] = [PARTITIONS_KEY, INCLUDE_KEY];

/// The relation removing them from it.
const EXCLUDE_KEY: &str = "exclude";

/// What a section's admitted entries do to the owner's share, and so which word admits them.
///
/// The distinction is not a spelling preference. A partition is answerable for
/// totality and exclusivity over what its exclusions left, and a deployment is
/// answerable for neither — two rows reaching one path is a defect in the first
/// and ordinary in the second. One word for both would leave a reader unable to
/// tell which promise a section was making, and would leave the checker's own
/// report unable to say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Admission {
    /// The entries divide what the exclusions left, totally and exclusively.
    Partition,
    /// The entries are deployed over reaches that may overlap.
    Deployment,
}

impl Admission {
    /// The key a section of this admission writes its rows under.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Partition => PARTITIONS_KEY,
            Self::Deployment => INCLUDE_KEY,
        }
    }

    /// The other admission's key, which a section of this one must not write.
    #[must_use]
    pub const fn refused(self) -> &'static str {
        match self {
            Self::Partition => INCLUDE_KEY,
            Self::Deployment => PARTITIONS_KEY,
        }
    }

    /// What the section's entries do, as a report says it.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Self::Partition => {
                "a section over this type divides what its exclusions left, totally and exclusively"
            }
            Self::Deployment => {
                "a section over this type deploys its entries over reaches that may overlap"
            }
        }
    }
}

impl fmt::Display for Admission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.key())
    }
}

/// A key the ratified grammar retired, and what took over its work.
///
/// Each stands here rather than among the unknown keys because a reader who
/// wrote one was not guessing: they were writing a dialect that was lawful
/// before the review, and what they are owed is the sentence saying where that
/// declaration lives now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetiredKey {
    /// The program identifier the namespace absorbed.
    Policy,
    /// The family key that discriminated instances now named as set entries.
    Family,
    /// The parameter table the named sets replaced.
    Parameters,
    /// The global source bound the owner sections replaced.
    Scope,
    /// The register destination that retired with the registers.
    Register,
    /// A program-owned list that was never repository data.
    Types,
    /// The owner document's exclusion key the shape document's ignore relation holds.
    Excluded,
    /// The plural spelling of the set table.
    PluralSets,
}

impl RetiredKey {
    /// The key as a retiring document spells it.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Family => "family",
            Self::Parameters => "parameters",
            Self::Scope => "scope",
            Self::Register => "register",
            Self::Types => "types",
            Self::Excluded => "excluded",
            Self::PluralSets => "sets",
        }
    }

    /// What carries this declaration under the ratified grammar.
    #[must_use]
    pub const fn replacement(self) -> &'static str {
        match self {
            Self::Policy => "the namespace is the document's whole identity",
            Self::Family => "one program acting on several sets is one document of named entries",
            Self::Parameters => "data stands in singular set tables of named entries",
            Self::Scope => "the owner sections say which paths an instance reaches",
            Self::Register => "the registers retire outright, and no declared key points at one",
            Self::Types => "a program-owned list is compiled meaning rather than repository data",
            Self::Excluded => "the shape document's ignore relation is that concept, declared once",
            Self::PluralSets => "the set table is singular",
        }
    }

    /// The retired key a top-level name is, where it is one.
    fn of(key: &str) -> Option<Self> {
        [
            Self::Policy,
            Self::Family,
            Self::Parameters,
            Self::Scope,
            Self::Register,
            Self::Types,
            Self::Excluded,
            Self::PluralSets,
        ]
        .into_iter()
        .find(|retired| retired.key() == key)
    }
}

impl fmt::Display for RetiredKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.key(), self.replacement())
    }
}

/// Why a document is not a declaration of the ratified grammar.
///
/// Every defect names where it stands in the document's own vocabulary — the set
/// and entry, or the owner, type and row — because a declaration is repaired
/// where it is written and a message saying only that something was wrong leaves
/// the reader searching a file for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "defect", rename_all = "snake_case")]
pub enum DeclarationDefect {
    /// The text is not a well-formed document at all.
    Malformed {
        /// What the parser said.
        message: String,
    },
    /// The envelope key that identifies a document is absent.
    AbsentEnvelope {
        /// The key nothing declared.
        key: &'static str,
    },
    /// An envelope key stands and is not what it must be.
    MalformedEnvelope {
        /// The key at fault.
        key: &'static str,
        /// What the grammar admits there.
        expected: &'static str,
    },
    /// A key the ratified grammar retired, reported with what replaced it.
    Retired {
        /// The retired key and its successor.
        retired: RetiredKey,
    },
    /// A key outside the grammar, which nothing would ever read.
    UnknownKey {
        /// The key as it stood.
        key: String,
    },
    /// A declared name outside the declared-name grammar.
    MalformedName {
        /// The relation it stands in, as a report names it.
        relation: String,
        /// The name as it stood.
        name: String,
    },
    /// Two rows of one relation answer to a single name.
    DuplicateName {
        /// The relation they stand in.
        relation: String,
        /// The name both rows claimed.
        name: String,
    },
    /// A row states no name for the region it claims.
    NamelessRow {
        /// The relation the row stands in.
        relation: String,
    },
    /// A row admitting sources states no reference to an entry of its section's set.
    AbsentReference {
        /// The relation the row stands in.
        relation: String,
        /// The type whose own name spells the reference.
        set: String,
    },
    /// Two rows of one relation carry a single entry.
    DuplicateReference {
        /// The relation they stand in.
        relation: String,
        /// The type the reference is into.
        set: String,
        /// The entry both rows carried.
        name: String,
    },
    /// A value that is not the shape its position calls for.
    MalformedValue {
        /// The relation it stands in, as a report names it.
        relation: String,
        /// What the grammar admits there.
        expected: String,
    },
    /// A pattern the augmented Backus-Naur form does not read.
    ///
    /// The engine's defect is carried whole rather than rendered away, because
    /// the position the form stopped at is where the declaration is repaired.
    MalformedPattern {
        /// The relation the row stands in.
        relation: String,
        /// The row's name.
        name: String,
        /// What the engine said.
        #[serde(rename = "grammar")]
        defect: GrammarDefect,
    },
    /// A section references entries of a type no set of this document declares.
    UnknownSetType {
        /// The owner whose section referenced it.
        owner: String,
        /// The type it named.
        set: String,
    },
    /// A reference names no entry of its section's set.
    DanglingReference {
        /// The owner whose section referenced it.
        owner: String,
        /// The set the reference is into.
        set: String,
        /// The name that resolves to nothing.
        name: String,
    },
    /// A section admits its rows under the word the other meaning is written with.
    WrongAdmission {
        /// The owner whose section writes it.
        owner: String,
        /// The type of the section.
        set: String,
        /// What this section's entries do, and so which word admits them.
        admission: Admission,
    },
    /// A section states two selection shapes at once.
    SelectionConflict {
        /// The owner whose section states them.
        owner: String,
        /// The type of the section.
        set: String,
    },
    /// Pattern rows stand on a naked owner table, which types nothing.
    UntypedRows {
        /// The owner whose table carries them.
        owner: String,
        /// The relation standing there.
        relation: String,
    },
}

impl fmt::Display for DeclarationDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { message } => formatter.write_str(message),
            Self::AbsentEnvelope { key } => write!(formatter, "{key}: the envelope declares it"),
            Self::MalformedEnvelope { key, expected } => write!(formatter, "{key}: {expected}"),
            Self::Retired { retired } => write!(formatter, "{retired}"),
            Self::UnknownKey { key } => {
                write!(formatter, "{key}: the grammar has no such declaration")
            }
            Self::MalformedName { relation, name } => {
                write!(
                    formatter,
                    "{relation}: {name}: not a well-formed declared name"
                )
            }
            Self::DuplicateName { relation, name } => write!(
                formatter,
                "{relation}: {name}: two rows answer to this name"
            ),
            Self::NamelessRow { relation } => write!(
                formatter,
                "{relation}: name: a row names the region it claims, and a row naming none is a region no report can say"
            ),
            Self::AbsentReference { relation, set } => write!(
                formatter,
                "{relation}: {set}: a row admitting sources references an entry of that set under the set's own type, name being the row's own region"
            ),
            Self::DuplicateReference {
                relation,
                set,
                name,
            } => write!(
                formatter,
                "{relation}: {set}: {name}: two rows carry this entry, and where one entry reaches is one question"
            ),
            Self::MalformedValue { relation, expected } => {
                write!(formatter, "{relation}: {expected}")
            }
            Self::MalformedPattern {
                relation,
                name,
                defect,
            } => write!(formatter, "{relation}: {name}: {defect}"),
            Self::UnknownSetType { owner, set } => write!(
                formatter,
                "{OWNERS_TABLE}.{owner}.{set}: no set of this document declares that type"
            ),
            Self::DanglingReference { owner, set, name } => write!(
                formatter,
                "{OWNERS_TABLE}.{owner}.{set}: {name}: no entry of that set answers to this name"
            ),
            Self::WrongAdmission {
                owner,
                set,
                admission,
            } => write!(
                formatter,
                "{OWNERS_TABLE}.{owner}.{set}: {}: {}, and {admission} is the word that admits them",
                admission.refused(),
                admission.because()
            ),
            Self::SelectionConflict { owner, set } => write!(
                formatter,
                "{OWNERS_TABLE}.{owner}.{set}: a section states one selection shape, and this states two"
            ),
            Self::UntypedRows { owner, relation } => write!(
                formatter,
                "{OWNERS_TABLE}.{owner}.{relation}: pattern rows stand in a typed section rather than on the owner"
            ),
        }
    }
}

/// One declared pattern: the source as written, and the grammar it compiles to.
///
/// The value a document writes is one rule's right-hand side, so the source is
/// kept beside the compiled form: a report naming a bound names what the author
/// wrote, and a document round-trips through the decoder unchanged.
#[derive(Debug, Clone)]
pub struct AbnfPattern {
    source: String,
    grammar: Grammar,
}

impl AbnfPattern {
    /// Compile a declared pattern value.
    ///
    /// # Errors
    ///
    /// Returns the engine's defect when the value is not the form, when it
    /// references a rule nothing defines, or when a rule of it can reference
    /// itself without consuming input.
    pub fn parse(source: impl Into<String>) -> Result<Self, GrammarDefect> {
        let source = source.into();
        let grammar = Grammar::parse(&format!("{PATTERN_RULE} = {source}"))?;

        Ok(Self { source, grammar })
    }

    /// The pattern as the declaration writes it.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether the text derives from the pattern, over the whole text.
    ///
    /// The question is asked of text rather than of a byte path, because how a
    /// path becomes the string a grammar decides is the resolver's to settle
    /// once for every relation rather than this decoder's to answer twice.
    #[must_use]
    pub fn admits(&self, text: &str) -> bool {
        self.grammar.matches(PATTERN_RULE, text).unwrap_or(false)
    }

    /// The literal openings every path the pattern admits begins with.
    ///
    /// The reach a pattern states is resolved to places through these: a caller
    /// walking what a pattern selects starts at each opening rather than asking
    /// the pattern about every path a corpus carries.
    #[must_use]
    pub fn branches(&self) -> Vec<Branch> {
        self.grammar
            .literal_branches(PATTERN_RULE)
            .unwrap_or_default()
    }

    /// Whether the pattern admits a repository-relative path entire.
    ///
    /// The path is read as text, and a path whose bytes are not text is admitted
    /// by no pattern, including one spelled to admit everything. The narrowing is
    /// the one the retiring dialect already carried and it is not silent: such a
    /// path stays in the accounting universe, so admitting no inclusion makes it
    /// fail totality by name rather than disappear.
    #[must_use]
    pub fn admits_path(&self, path: &BytePath) -> bool {
        std::str::from_utf8(path.as_bytes()).is_ok_and(|text| self.admits(text))
    }
}

impl PartialEq for AbnfPattern {
    /// Two patterns are one when their sources are, the grammar being a function
    /// of the source rather than a second thing the value carries.
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for AbnfPattern {}

impl PartialOrd for AbnfPattern {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AbnfPattern {
    /// Patterns order by what the document wrote, for the reason they compare by it.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.source.cmp(&other.source)
    }
}

impl std::hash::Hash for AbnfPattern {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl fmt::Display for AbnfPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source)
    }
}

impl Serialize for AbnfPattern {
    /// A pattern serializes as the text the document wrote, so a report naming a
    /// bound names the value its author can find in the file.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.source)
    }
}

/// A value kept by its form, where no program of this binary types it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Scalar {
    /// A declared string.
    Text(String),
    /// A declared whole number.
    Number(i64),
    /// A declared truth.
    Truth(bool),
}

/// The type of entry a set holds, where this binary has a program that reads it.
///
/// The vocabulary is closed at what the repository's documents declare, and a
/// type outside it is [`SetKind::Unadopted`] rather than a refusal: which types
/// are adopted is an owner act recorded outside configuration, so a decoder
/// deciding it here would be answering with authority it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetKind {
    /// A mark and the inclusive numeric bound its sequence issued.
    NumberedMarks,
    /// A prefix and the enumeration or range its scheme issued.
    PrefixNumbers,
    /// A verbatim string used as a locator.
    Literals,
    /// A licence expression.
    Identifier,
    /// A copyright text.
    Copyright,
    /// The manifest key a package name is read from.
    NameKey,
    /// A literal prefix stripped before an owner spelling is compared.
    NamePrefixIgnore,
    /// A type no program of this binary reads, kept by name and by form.
    Unadopted,
}

impl SetKind {
    /// The kind a declared type name is.
    #[must_use]
    pub fn of(set: &str) -> Self {
        CATALOG.set_kind(set)
    }

    /// Whether a program of this binary reads entries of this kind.
    #[must_use]
    pub const fn is_adopted(self) -> bool {
        !matches!(self, Self::Unadopted)
    }

    /// What a section over this kind does to the owner's share, where this binary reads the kind.
    ///
    /// The two licence halves partition: each removes the union of its exclusion
    /// rules and divides what survives, totally and exclusively, which is what
    /// makes an omission tellable from an exclusion there. Every other adopted
    /// kind deploys — a division literal, a numbered mark and a prefix scheme are
    /// each carried over whatever reach the owner writes for it, and two of them
    /// reaching one path is ordinary rather than a defect.
    ///
    /// An unadopted kind answers nothing, and takes either word. Fixing one would
    /// be this decoder deciding what a type it reads no program for does with its
    /// entries, which is the authority it already declines over adoption.
    #[must_use]
    pub fn admission(self) -> Option<Admission> {
        CATALOG.admission(self)
    }

    /// Whether an entry of this kind is a string rather than an inline table.
    const fn is_simple(self) -> bool {
        matches!(
            self,
            Self::Literals
                | Self::Identifier
                | Self::Copyright
                | Self::NameKey
                | Self::NamePrefixIgnore
        )
    }
}

/// One prefix-number entry: the prefix, and the bound its scheme had.
///
/// Shielding is absent on purpose. Whether an occurrence the section grammar has
/// already claimed belongs to that reference is a precedence configuration may
/// neither widen nor disable, so the compiled half of the program joins the
/// declared half where the run is assembled rather than being declarable here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixNumberEntry {
    prefix: String,
    bound: PrefixBound,
}

impl PrefixNumberEntry {
    /// Declare a prefix-number entry from its prefix and bound.
    #[must_use]
    pub fn new(prefix: impl Into<String>, bound: PrefixBound) -> Self {
        Self {
            prefix: prefix.into(),
            bound,
        }
    }

    /// The prefix that makes a number a reference to this scheme.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The bound the scheme issued within.
    #[must_use]
    pub const fn bound(&self) -> &PrefixBound {
        &self.bound
    }
}

/// One named entry of a set.
///
/// The parameterized variants hold the program's own values rather than copies
/// of them, so what a decoder produces is what a program consumes. The two
/// remaining variants are what an entry is before any program claims its type: a
/// string, or a table of values kept by form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetEntry {
    /// A mark and the inclusive bound its sequence issued.
    NumberedMark(MarkNumbered),
    /// A prefix and the bound its scheme issued.
    PrefixNumber(PrefixNumberEntry),
    /// A simple entry, which is its string entire.
    Text(String),
    /// A parameterized entry of an unadopted type, kept by form.
    Fields(BTreeMap<String, Scalar>),
}

/// One `[set.TYPE]` table: what type it holds, and its entries by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSet {
    kind: SetKind,
    entries: BTreeMap<String, SetEntry>,
}

impl DeclaredSet {
    /// Declare a set from its kind and its named entries.
    #[must_use]
    pub fn new(kind: SetKind, entries: impl IntoIterator<Item = (String, SetEntry)>) -> Self {
        Self {
            kind,
            entries: entries.into_iter().collect(),
        }
    }

    /// The kind of entry this set holds.
    #[must_use]
    pub const fn kind(&self) -> SetKind {
        self.kind
    }

    /// Whether a program of this binary reads this set.
    #[must_use]
    pub const fn is_adopted(&self) -> bool {
        self.kind.is_adopted()
    }

    /// The entries, by the names selections reference them under.
    #[must_use]
    pub const fn entries(&self) -> &BTreeMap<String, SetEntry> {
        &self.entries
    }

    /// The entry a name answers to, where one does.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&SetEntry> {
        self.entries.get(name)
    }
}

/// One row of an owner's pattern selection: the region it names, the entry it
/// carries where it carries one, and the pattern.
///
/// The name and the reference are two declarations rather than one spelling of
/// one. The name is the row's own and promises nothing about a set; the
/// reference names an entry of the section's type and is what the region carries
/// there. An exclusion has the first and never the second, which is why the
/// reference is an absence rather than an empty string: a row that carries no
/// entry has not named one badly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternRow {
    name: String,
    entry: Option<String>,
    pattern: AbnfPattern,
}

impl PatternRow {
    /// Declare a row that carries no entry, from its name and its compiled pattern.
    #[must_use]
    pub fn new(name: impl Into<String>, pattern: AbnfPattern) -> Self {
        Self {
            name: name.into(),
            entry: None,
            pattern,
        }
    }

    /// Declare a row admitting sources, which names the entry its region carries.
    #[must_use]
    pub fn carrying(
        name: impl Into<String>,
        entry: impl Into<String>,
        pattern: AbnfPattern,
    ) -> Self {
        Self {
            name: name.into(),
            entry: Some(entry.into()),
            pattern,
        }
    }

    /// The region this row claims, or the exclusion it labels.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The entry the region carries, which every admitted row declares and no exclusion does.
    #[must_use]
    pub fn entry(&self) -> Option<&str> {
        self.entry.as_deref()
    }

    /// The pattern the row reaches sources with.
    #[must_use]
    pub const fn pattern(&self) -> &AbnfPattern {
        &self.pattern
    }
}

/// How one owner selects the entries of one set type.
///
/// Two shapes rather than one, because the question *which entries apply here*
/// and the question *where does this one entry apply* are different questions
/// and a list of one is not the second one's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// The entries that apply across files, and where each of them does.
    Rows {
        /// The rows admitting sources, each naming its region and the entry that region carries.
        ///
        /// One field for the two admitting words, because `partitions` and
        /// `include` differ in what the rows promise rather than in what they
        /// are, and the promise is the set type's to make. A reader wanting to
        /// know which was written asks the type, which is where the answer was
        /// fixed.
        admitted: Vec<PatternRow>,
        /// The rows removing sources, each naming the exclusion itself.
        exclude: Vec<PatternRow>,
    },
    /// The one entry that applies to this owner, named without a list.
    Entry(String),
}

/// One datum bound to an owner rather than to a set.
///
/// The naked owner table is lawful exactly for content with no set behind it, so
/// what stands here is either a value the owner is asked for directly or a row
/// the document's namespace types entirely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerDatum {
    /// A simple value, such as the manifest an owner's names are read from.
    Text(String),
    /// A publication of parts into a target, attributed by the table it stands on.
    Publication(Publication),
    /// A parameterized datum no program of this binary types, kept by form.
    Fields(BTreeMap<String, Scalar>),
}

/// Everything one owner declares: its own data, and its typed selections.
///
/// Both halves stand for one owner at once, because the naked table and the
/// typed sections answer different questions and a document that had to choose
/// between them would need a second place to put whichever it lost.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnerDeclaration {
    data: BTreeMap<String, OwnerDatum>,
    selections: BTreeMap<String, Selection>,
}

impl OwnerDeclaration {
    /// Declare an owner from its own data and its typed selections.
    #[must_use]
    pub fn new(
        data: impl IntoIterator<Item = (String, OwnerDatum)>,
        selections: impl IntoIterator<Item = (String, Selection)>,
    ) -> Self {
        Self {
            data: data.into_iter().collect(),
            selections: selections.into_iter().collect(),
        }
    }

    /// The data standing on the naked owner table, by name.
    #[must_use]
    pub const fn data(&self) -> &BTreeMap<String, OwnerDatum> {
        &self.data
    }

    /// The datum a name answers to, where one does.
    #[must_use]
    pub fn datum(&self, name: &str) -> Option<&OwnerDatum> {
        self.data.get(name)
    }

    /// The selections, by the set type each is over.
    #[must_use]
    pub const fn selections(&self) -> &BTreeMap<String, Selection> {
        &self.selections
    }

    /// The selection over one set type, where the owner declares one.
    #[must_use]
    pub fn selection(&self, set: &str) -> Option<&Selection> {
        self.selections.get(set)
    }
}

/// One decoded instance document.
///
/// The namespace is the identity and the version is the schema's own; everything
/// else the document says is data in named sets and choices attached to owners.
/// There is no third kind of content, which is what keeps an instance document
/// from growing into the omnibus deployment record the outline attack rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    namespace: String,
    version: [u64; 3],
    sets: BTreeMap<String, DeclaredSet>,
    owners: BTreeMap<String, OwnerDeclaration>,
}

impl Declaration {
    /// Decode one instance document written in the ratified grammar.
    ///
    /// Every defect the document carries is returned rather than the first,
    /// because a document being converted to this grammar usually carries
    /// several and repairing them one round-trip at a time teaches the author
    /// nothing they could not have been told at once.
    ///
    /// # Errors
    ///
    /// Returns every defect found, in the document's own reading order.
    pub fn decode(text: &str) -> Result<Self, Vec<DeclarationDefect>> {
        let table: toml::Table = match toml::from_str(text) {
            Ok(table) => table,
            Err(error) => {
                return Err(vec![DeclarationDefect::Malformed {
                    message: error.to_string(),
                }]);
            }
        };

        Self::decode_table(&table)
    }

    /// Project one declaration from an already-parsed surface document.
    pub(crate) fn decode_table(table: &toml::Table) -> Result<Self, Vec<DeclarationDefect>> {
        let mut defects = Vec::new();

        let namespace = namespace(table, &mut defects);
        let version = version(table, &mut defects);

        surplus_keys(table, &mut defects);

        let sets = sets(table, &mut defects);
        let owners = owners(table, &sets, &mut defects);

        match (namespace, version) {
            (Some(namespace), Some(version)) if defects.is_empty() => Ok(Self {
                namespace,
                version,
                sets,
                owners,
            }),
            _ => Err(defects),
        }
    }

    /// The document's whole identity.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The version of the schema the namespace names.
    #[must_use]
    pub const fn version(&self) -> [u64; 3] {
        self.version
    }

    /// The declared sets, by the type each holds.
    #[must_use]
    pub const fn sets(&self) -> &BTreeMap<String, DeclaredSet> {
        &self.sets
    }

    /// The set of one type, where the document declares one.
    #[must_use]
    pub fn set(&self, set: &str) -> Option<&DeclaredSet> {
        self.sets.get(set)
    }

    /// The owner declarations, by owner.
    #[must_use]
    pub const fn owners(&self) -> &BTreeMap<String, OwnerDeclaration> {
        &self.owners
    }

    /// What one owner declares, where the document names it.
    #[must_use]
    pub fn owner(&self, owner: &str) -> Option<&OwnerDeclaration> {
        self.owners.get(owner)
    }
}

/// Read the dotted namespace that is the document's identity.
fn namespace(table: &toml::Table, defects: &mut Vec<DeclarationDefect>) -> Option<String> {
    let Some(value) = table.get("namespace") else {
        defects.push(DeclarationDefect::AbsentEnvelope { key: "namespace" });

        return None;
    };

    match value.as_str() {
        Some(text) if is_namespace(text) => Some(text.to_owned()),
        _ => {
            defects.push(DeclarationDefect::MalformedEnvelope {
                key: "namespace",
                expected: "a dotted identity of declared-name components",
            });

            None
        }
    }
}

/// Whether text is a dotted identity of declared-name components.
fn is_namespace(text: &str) -> bool {
    let mut components = 0;

    for component in text.split('.') {
        if !is_declared_name(component) {
            return false;
        }

        components += 1;
    }

    components >= 2
}

/// Read the three-component version the envelope carries.
fn version(table: &toml::Table, defects: &mut Vec<DeclarationDefect>) -> Option<[u64; 3]> {
    let Some(value) = table.get("version") else {
        defects.push(DeclarationDefect::AbsentEnvelope { key: "version" });

        return None;
    };

    let components: Option<Vec<u64>> = value.as_array().map(|values| {
        values
            .iter()
            .filter_map(|value| {
                value
                    .as_integer()
                    .and_then(|number| u64::try_from(number).ok())
            })
            .collect()
    });

    let Some(&[major, minor, patch]) = components.as_deref() else {
        defects.push(DeclarationDefect::MalformedEnvelope {
            key: "version",
            expected: "three whole numbers",
        });

        return None;
    };

    Some([major, minor, patch])
}

/// Refuse the keys the grammar retired, and the keys it never had.
fn surplus_keys(table: &toml::Table, defects: &mut Vec<DeclarationDefect>) {
    for key in table.keys() {
        if matches!(
            key.as_str(),
            "namespace" | "version" | SET_TABLE | OWNERS_TABLE
        ) {
            continue;
        }

        if let Some(retired) = RetiredKey::of(key) {
            defects.push(DeclarationDefect::Retired { retired });
        } else {
            defects.push(DeclarationDefect::UnknownKey { key: key.clone() });
        }
    }
}

/// Decode every `[set.TYPE]` table into its named entries.
fn sets(
    table: &toml::Table,
    defects: &mut Vec<DeclarationDefect>,
) -> BTreeMap<String, DeclaredSet> {
    let mut decoded = BTreeMap::new();

    let Some(value) = table.get(SET_TABLE) else {
        return decoded;
    };

    let Some(types) = value.as_table() else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: SET_TABLE.to_owned(),
            expected: "a table of named entries per type".to_owned(),
        });

        return decoded;
    };

    for (set, entries) in types {
        if !is_declared_name(set) {
            defects.push(DeclarationDefect::MalformedName {
                relation: SET_TABLE.to_owned(),
                name: set.clone(),
            });

            continue;
        }

        let relation = format!("{SET_TABLE}.{set}");

        let Some(entries) = entries.as_table() else {
            defects.push(DeclarationDefect::MalformedValue {
                relation,
                expected: "a table of named entries".to_owned(),
            });

            continue;
        };

        let kind = SetKind::of(set);
        let mut named = BTreeMap::new();

        for (name, value) in entries {
            if !is_declared_name(name) {
                defects.push(DeclarationDefect::MalformedName {
                    relation: relation.clone(),
                    name: name.clone(),
                });

                continue;
            }

            if let Some(entry) = set_entry(kind, &format!("{relation}.{name}"), value, defects) {
                named.insert(name.clone(), entry);
            }
        }

        decoded.insert(set.clone(), DeclaredSet::new(kind, named));
    }

    decoded
}

/// Decode one entry against the kind its set holds.
fn set_entry(
    kind: SetKind,
    relation: &str,
    value: &toml::Value,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<SetEntry> {
    if kind.is_simple() {
        let Some(text) = value.as_str() else {
            defects.push(DeclarationDefect::MalformedValue {
                relation: relation.to_owned(),
                expected: "a string, this entry being simple".to_owned(),
            });

            return None;
        };

        return Some(SetEntry::Text(text.to_owned()));
    }

    match kind {
        SetKind::NumberedMarks => numbered_mark(relation, value, defects),
        SetKind::PrefixNumbers => prefix_number(relation, value, defects),
        _ => match value {
            toml::Value::String(text) => Some(SetEntry::Text(text.clone())),
            toml::Value::Table(fields) => {
                fields_of(relation, fields, defects).map(SetEntry::Fields)
            }
            _ => {
                defects.push(DeclarationDefect::MalformedValue {
                    relation: relation.to_owned(),
                    expected: "a string or a table of values".to_owned(),
                });

                None
            }
        },
    }
}

/// Decode a reach stated relative to the root of whichever owner binds it.
/// Decode a mark and the inclusive bound its sequence issued.
fn numbered_mark(
    relation: &str,
    value: &toml::Value,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<SetEntry> {
    let fields = closed_table(relation, value, &["mark", "minimum", "maximum"], defects)?;

    let mark = text(relation, fields, "mark", defects);
    let minimum = number(relation, fields, "minimum", defects);
    let maximum = number(relation, fields, "maximum", defects);

    let (mark, minimum, maximum) = (mark?, minimum?, maximum?);
    let mut marks = mark.chars();

    let (Some(mark), None) = (marks.next(), marks.next()) else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: format!("{relation}.mark"),
            expected: "one character".to_owned(),
        });

        return None;
    };

    Some(SetEntry::NumberedMark(MarkNumbered::new(
        mark, minimum, maximum,
    )))
}

/// Decode a prefix and exactly one of the three bounds a scheme may have had.
fn prefix_number(
    relation: &str,
    value: &toml::Value,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<SetEntry> {
    let fields = closed_table(
        relation,
        value,
        &[
            "prefix",
            "exact",
            "leading",
            "leading-minimum",
            "leading-maximum",
        ],
        defects,
    )?;

    let prefix = text(relation, fields, "prefix", defects);

    let exact = fields.contains_key("exact");
    let leading = fields.contains_key("leading");
    let range = fields.contains_key("leading-minimum") || fields.contains_key("leading-maximum");

    if [exact, leading, range]
        .iter()
        .filter(|declared| **declared)
        .count()
        != 1
    {
        defects.push(DeclarationDefect::MalformedValue {
            relation: relation.to_owned(),
            expected: "exactly one of exact, leading and the leading range".to_owned(),
        });

        return None;
    }

    let bound = if exact {
        PrefixBound::Exact(strings(relation, fields, "exact", defects)?)
    } else if leading {
        PrefixBound::LeadingSet(numbers(relation, fields, "leading", defects)?)
    } else {
        let minimum = number(relation, fields, "leading-minimum", defects);
        let maximum = number(relation, fields, "leading-maximum", defects);

        PrefixBound::LeadingRange {
            minimum: minimum?,
            maximum: maximum?,
        }
    };

    Some(SetEntry::PrefixNumber(PrefixNumberEntry::new(
        prefix?, bound,
    )))
}

/// Read a table whose keys are held to what the entry's kind admits.
fn closed_table<'a>(
    relation: &str,
    value: &'a toml::Value,
    admitted: &[&str],
    defects: &mut Vec<DeclarationDefect>,
) -> Option<&'a toml::Table> {
    let Some(fields) = value.as_table() else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: relation.to_owned(),
            expected: "an inline table, this entry being parameterized".to_owned(),
        });

        return None;
    };

    let mut surplus = false;

    for key in fields.keys() {
        if !admitted.contains(&key.as_str()) {
            defects.push(DeclarationDefect::UnknownKey {
                key: format!("{relation}.{key}"),
            });

            surplus = true;
        }
    }

    (!surplus).then_some(fields)
}

/// Read one string field of a parameterized entry.
fn text(
    relation: &str,
    fields: &toml::Table,
    key: &str,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<String> {
    let Some(text) = fields.get(key).and_then(toml::Value::as_str) else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: format!("{relation}.{key}"),
            expected: "a string".to_owned(),
        });

        return None;
    };

    Some(text.to_owned())
}

/// Read one whole-number field of a parameterized entry.
fn number(
    relation: &str,
    fields: &toml::Table,
    key: &str,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<u32> {
    let declared = fields
        .get(key)
        .and_then(toml::Value::as_integer)
        .and_then(|number| u32::try_from(number).ok());

    if declared.is_none() {
        defects.push(DeclarationDefect::MalformedValue {
            relation: format!("{relation}.{key}"),
            expected: "a whole number".to_owned(),
        });
    }

    declared
}

/// Read one field enumerating strings.
fn strings(
    relation: &str,
    fields: &toml::Table,
    key: &str,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<Vec<String>> {
    let declared = fields.get(key).and_then(toml::Value::as_array);
    let values: Option<Vec<String>> = declared.map(|values| {
        values
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect()
    });

    match values {
        Some(values) if values.len() == declared.map_or(0, Vec::len) => Some(values),
        _ => {
            defects.push(DeclarationDefect::MalformedValue {
                relation: format!("{relation}.{key}"),
                expected: "an enumeration of strings".to_owned(),
            });

            None
        }
    }
}

/// Read one field enumerating whole numbers.
fn numbers(
    relation: &str,
    fields: &toml::Table,
    key: &str,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<Vec<u32>> {
    let declared = fields.get(key).and_then(toml::Value::as_array);
    let values: Option<Vec<u32>> = declared.map(|values| {
        values
            .iter()
            .filter_map(|value| {
                value
                    .as_integer()
                    .and_then(|number| u32::try_from(number).ok())
            })
            .collect()
    });

    match values {
        Some(values) if values.len() == declared.map_or(0, Vec::len) => Some(values),
        _ => {
            defects.push(DeclarationDefect::MalformedValue {
                relation: format!("{relation}.{key}"),
                expected: "an enumeration of whole numbers".to_owned(),
            });

            None
        }
    }
}

/// Read a table of values kept by form, for a type no program claims.
fn fields_of(
    relation: &str,
    fields: &toml::Table,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<BTreeMap<String, Scalar>> {
    let mut kept = BTreeMap::new();

    for (key, value) in fields {
        let scalar = match value {
            toml::Value::String(text) => Scalar::Text(text.clone()),
            toml::Value::Integer(number) => Scalar::Number(*number),
            toml::Value::Boolean(truth) => Scalar::Truth(*truth),
            _ => {
                defects.push(DeclarationDefect::MalformedValue {
                    relation: format!("{relation}.{key}"),
                    expected: "a string, a whole number or a truth".to_owned(),
                });

                return None;
            }
        };

        kept.insert(key.clone(), scalar);
    }

    Some(kept)
}

/// Decode every owner's data and typed selections.
fn owners(
    table: &toml::Table,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> BTreeMap<String, OwnerDeclaration> {
    let mut decoded = BTreeMap::new();

    let Some(value) = table.get(OWNERS_TABLE) else {
        return decoded;
    };

    let Some(rows) = value.as_table() else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: OWNERS_TABLE.to_owned(),
            expected: "a table of owner declarations".to_owned(),
        });

        return decoded;
    };

    for (owner, value) in rows {
        let relation = format!("{OWNERS_TABLE}.{owner}");

        let Some(keys) = value.as_table() else {
            defects.push(DeclarationDefect::MalformedValue {
                relation,
                expected: "a table of owner-bound data and typed sections".to_owned(),
            });

            continue;
        };

        decoded.insert(owner.clone(), owner_declaration(owner, keys, sets, defects));
    }

    decoded
}

/// Decode one owner's table, telling its two shapes apart by the keys they carry.
fn owner_declaration(
    owner: &str,
    keys: &toml::Table,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> OwnerDeclaration {
    let mut data = BTreeMap::new();
    let mut selections = BTreeMap::new();

    for (key, value) in keys {
        let relation = format!("{OWNERS_TABLE}.{owner}.{key}");

        if !is_declared_name(key) {
            defects.push(DeclarationDefect::MalformedName {
                relation: format!("{OWNERS_TABLE}.{owner}"),
                name: key.clone(),
            });

            continue;
        }

        match value {
            toml::Value::String(text) => {
                data.insert(key.clone(), OwnerDatum::Text(text.clone()));
            }
            toml::Value::Table(fields) if is_selection(fields) => {
                if let Some(selection) = selection(owner, key, fields, sets, defects) {
                    selections.insert(key.clone(), selection);
                }
            }
            toml::Value::Table(fields) => {
                if let Some(datum) = owner_datum(owner, &relation, fields, defects) {
                    data.insert(key.clone(), datum);
                }
            }
            toml::Value::Array(_) => defects.push(DeclarationDefect::UntypedRows {
                owner: owner.to_owned(),
                relation: key.clone(),
            }),
            _ => defects.push(DeclarationDefect::MalformedValue {
                relation,
                expected: "a string or a table".to_owned(),
            }),
        }
    }

    OwnerDeclaration::new(data, selections)
}

/// Whether a table under an owner is a selection rather than a datum.
///
/// The three keys of the two selection shapes are the discriminator, because a
/// subtable and an inline table are one thing here and what a table means is
/// therefore what it carries.
fn is_selection(fields: &toml::Table) -> bool {
    [USE_KEY, PARTITIONS_KEY, INCLUDE_KEY, EXCLUDE_KEY]
        .into_iter()
        .any(|key| fields.contains_key(key))
}

/// The word a section admits its rows under, where the section is lawful at all.
///
/// The set type decides it, so a section writing the other meaning's word refuses
/// here rather than decoding into rows that would then be read as the promise it
/// did not make. Where the kind is one this binary reads no program for, the
/// decoder holds no opinion and takes whichever word stands — but not both, since
/// two words for one relation is two answers to one question exactly as a `use`
/// key beside rows is.
fn admits(
    owner: &str,
    set: &str,
    fields: &toml::Table,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<&'static str> {
    let Some(admission) = SetKind::of(set).admission() else {
        if ADMISSION_KEYS
            .into_iter()
            .all(|key| fields.contains_key(key))
        {
            defects.push(DeclarationDefect::SelectionConflict {
                owner: owner.to_owned(),
                set: set.to_owned(),
            });

            return None;
        }

        return Some(
            ADMISSION_KEYS
                .into_iter()
                .find(|key| fields.contains_key(*key))
                .unwrap_or(INCLUDE_KEY),
        );
    };

    if fields.contains_key(admission.refused()) {
        defects.push(DeclarationDefect::WrongAdmission {
            owner: owner.to_owned(),
            set: set.to_owned(),
            admission,
        });

        return None;
    }

    Some(admission.key())
}

/// Decode one typed section into the selection shape it states.
fn selection(
    owner: &str,
    set: &str,
    fields: &toml::Table,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<Selection> {
    let admits = admits(owner, set, fields, defects)?;
    let singular = fields.contains_key(USE_KEY);
    let listed = fields.contains_key(admits);

    if singular && listed {
        defects.push(DeclarationDefect::SelectionConflict {
            owner: owner.to_owned(),
            set: set.to_owned(),
        });

        return None;
    }

    let relation = format!("{OWNERS_TABLE}.{owner}.{set}");

    for key in fields.keys() {
        if !matches!(key.as_str(), USE_KEY | EXCLUDE_KEY) && key != admits {
            defects.push(DeclarationDefect::UnknownKey {
                key: format!("{relation}.{key}"),
            });

            return None;
        }
    }

    if singular {
        return used(owner, set, &relation, fields, sets, defects).map(Selection::Entry);
    }

    let admitted = rows(owner, set, admits, fields, sets, defects);
    let exclude = rows(owner, set, EXCLUDE_KEY, fields, sets, defects);

    Some(Selection::Rows {
        admitted: admitted?,
        exclude: exclude?,
    })
}

/// Decode the one entry a section names without a list.
///
/// One spelling and no second one. The list spelling was the owner-areas
/// experiment's, and it retired with it: an entry carrying its own reach could be
/// deployed by naming it, so a section wanted several names and an exclude half
/// beside them. Nothing declares such a set now, and a grammar keeping a shape
/// nothing writes is a grammar with a second answer waiting for the question.
fn used(
    owner: &str,
    set: &str,
    relation: &str,
    fields: &toml::Table,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<String> {
    let Some(toml::Value::String(name)) = fields.get(USE_KEY) else {
        defects.push(DeclarationDefect::MalformedValue {
            relation: format!("{relation}.{USE_KEY}"),
            expected: "a name".to_owned(),
        });

        return None;
    };

    resolves(owner, set, name, sets, defects).then(|| name.clone())
}

/// Decode one relation of pattern rows, holding each name, reference and pattern to its form.
///
/// A row admitting sources carries a reference to an entry of the section's set
/// and spells it under that set's own type, while an exclusion carries none. The
/// test is against the one relation that references nothing rather than against a
/// list of the ones that do, so a word admitting rows carries the promise by
/// being an admitting word.
///
/// The two identities the rows carry are held to different rules, because they
/// mean different things. A partition's rows name its parts and two parts under
/// one name is two answers to which part a report means; a deployment's regions
/// overlap by construction, so a repeated name there is one true thing said
/// twice. A reference is unique under either word: where one entry reaches is one
/// question.
#[allow(
    clippy::too_many_lines,
    reason = "one ordered decoder accumulates row defects in source order, which the report exposes"
)]
fn rows(
    owner: &str,
    set: &str,
    key: &str,
    fields: &toml::Table,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<Vec<PatternRow>> {
    let relation = format!("{OWNERS_TABLE}.{owner}.{set}.{key}");
    let admitting = key != EXCLUDE_KEY;
    let named_apart = key != INCLUDE_KEY;

    let Some(value) = fields.get(key) else {
        return Some(Vec::new());
    };

    let Some(declared) = value.as_array() else {
        defects.push(DeclarationDefect::MalformedValue {
            relation,
            expected: "an enumeration of name-and-pattern rows".to_owned(),
        });

        return None;
    };

    let admits: Vec<&str> = if admitting {
        vec!["name", set, "pattern"]
    } else {
        vec!["name", "pattern"]
    };

    let mut named = BTreeSet::new();
    let mut carried = BTreeSet::new();
    let mut decoded = Vec::new();
    let mut sound = true;

    for value in declared {
        let Some(fields) = closed_table(&relation, value, &admits, defects) else {
            sound = false;

            continue;
        };

        if !fields.contains_key("name") {
            defects.push(DeclarationDefect::NamelessRow {
                relation: relation.clone(),
            });

            sound = false;

            continue;
        }

        if admitting && !fields.contains_key(set) {
            defects.push(DeclarationDefect::AbsentReference {
                relation: relation.clone(),
                set: set.to_owned(),
            });

            sound = false;

            continue;
        }

        let name = text(&relation, fields, "name", defects);
        let pattern = text(&relation, fields, "pattern", defects);
        let entry = admitting.then(|| text(&relation, fields, set, defects));

        let (Some(name), Some(pattern)) = (name, pattern) else {
            sound = false;

            continue;
        };

        let entry = match entry {
            Some(None) => {
                sound = false;

                continue;
            }
            Some(Some(entry)) => Some(entry),
            None => None,
        };

        let Some(name) = well_formed(&relation, name, defects) else {
            sound = false;

            continue;
        };

        if named_apart && !named.insert(name.clone()) {
            defects.push(DeclarationDefect::DuplicateName {
                relation: relation.clone(),
                name,
            });

            sound = false;

            continue;
        }

        if let Some(entry) = &entry {
            let Some(entry) = well_formed(&relation, entry.clone(), defects) else {
                sound = false;

                continue;
            };

            if !resolves(owner, set, &entry, sets, defects) {
                sound = false;

                continue;
            }

            if !carried.insert(entry.clone()) {
                defects.push(DeclarationDefect::DuplicateReference {
                    relation: relation.clone(),
                    set: set.to_owned(),
                    name: entry,
                });

                sound = false;

                continue;
            }
        }

        match AbnfPattern::parse(pattern) {
            Ok(pattern) => decoded.push(match entry {
                Some(entry) => PatternRow::carrying(name, entry, pattern),
                None => PatternRow::new(name, pattern),
            }),
            Err(defect) => {
                defects.push(DeclarationDefect::MalformedPattern {
                    relation: relation.clone(),
                    name,
                    defect,
                });

                sound = false;
            }
        }
    }

    sound.then_some(decoded)
}

/// A declared name of a row, or the defect saying it is not one.
fn well_formed(
    relation: &str,
    name: String,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<String> {
    if is_declared_name(&name) {
        return Some(name);
    }

    defects.push(DeclarationDefect::MalformedName {
        relation: relation.to_owned(),
        name,
    });

    None
}

/// Whether a reference names an entry of the set its section is over.
fn resolves(
    owner: &str,
    set: &str,
    name: &str,
    sets: &BTreeMap<String, DeclaredSet>,
    defects: &mut Vec<DeclarationDefect>,
) -> bool {
    let Some(declared) = sets.get(set) else {
        defects.push(DeclarationDefect::UnknownSetType {
            owner: owner.to_owned(),
            set: set.to_owned(),
        });

        return false;
    };

    if declared.entry(name).is_some() {
        return true;
    }

    defects.push(DeclarationDefect::DanglingReference {
        owner: owner.to_owned(),
        set: set.to_owned(),
        name: name.to_owned(),
    });

    false
}

/// Decode one parameterized datum on a naked owner table.
fn owner_datum(
    owner: &str,
    relation: &str,
    fields: &toml::Table,
    defects: &mut Vec<DeclarationDefect>,
) -> Option<OwnerDatum> {
    let publication =
        fields.len() == 2 && fields.contains_key("parts") && fields.contains_key("target");

    if publication {
        let parts = text(relation, fields, "parts", defects);
        let target = text(relation, fields, "target", defects);

        return Some(OwnerDatum::Publication(Publication::new(
            owner,
            PathBuf::from(parts?),
            PathBuf::from(target?),
        )));
    }

    fields_of(relation, fields, defects).map(OwnerDatum::Fields)
}

#[cfg(test)]
mod tests {
    use super::{
        Admission, Declaration, DeclarationDefect, DeclaredSet, OwnerDatum, PatternRow,
        PrefixNumberEntry, RetiredKey, Scalar, Selection, SetEntry, SetKind,
    };
    use crate::program::{MarkNumbered, PrefixBound};

    /// The document text of an invented corpus, decoded or refused.
    fn decode(text: &str) -> Result<Declaration, Vec<DeclarationDefect>> {
        Declaration::decode(text)
    }

    /// The defects a document carries, refused as a whole.
    fn defects(text: &str) -> Vec<DeclarationDefect> {
        decode(text).expect_err("a document the grammar refuses")
    }

    /// The renderings of a document's defects, for reading a refusal by what it says.
    fn rendered(text: &str) -> Vec<String> {
        defects(text).iter().map(ToString::to_string).collect()
    }

    /// Whether some defect's rendering carries the words.
    fn says(text: &str, words: &str) -> bool {
        rendered(text)
            .iter()
            .any(|rendering| rendering.contains(words))
    }

    /// The admitted rows of one owner's section over one set type.
    fn admitted<'a>(document: &'a Declaration, owner: &str, set: &str) -> &'a [PatternRow] {
        match document.owner(owner).and_then(|owner| owner.selection(set)) {
            Some(Selection::Rows { admitted, .. }) => admitted,
            _ => panic!("the section states pattern rows"),
        }
    }

    /// The owner-name document the owner ratified entry by entry decodes whole:
    /// two sets of one named entry each, a naked owner table carrying the
    /// manifest the owner's names are read from, and one singular reference per
    /// set resolving to its entry. Both shapes stand for one owner at once,
    /// which is what the naked table exists to allow.
    ///
    /// ´claim:declaration:the-ratified-owner-name-document-decodes-whole´
    /// ´test:unit:decodes-the-ratified-owner-name-document´
    #[test]
    fn decodes_the_ratified_owner_name_document() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[set.name-key]
cargo-package-name = "name"

[set.name-prefix-ignore]
torrust = "torrust-"

[owners.INDEX]
cargo-toml = "Cargo.toml"

[owners.INDEX.name-key]
use = "cargo-package-name"

[owners.INDEX.name-prefix-ignore]
use = "torrust"
"#,
        )
        .expect("the ratified document");

        assert_eq!(
            document.namespace(),
            "com.torrust.index.linter.policy.owner.names"
        );
        assert_eq!(document.version(), [1, 0, 0]);

        let keys = document.set("name-key").expect("the manifest-key set");
        let prefixes = document
            .set("name-prefix-ignore")
            .expect("the stripped-prefix set");

        assert_eq!(keys.kind(), SetKind::NameKey);
        assert_eq!(prefixes.kind(), SetKind::NamePrefixIgnore);
        assert_eq!(
            keys.entry("cargo-package-name"),
            Some(&SetEntry::Text("name".to_owned()))
        );
        assert_eq!(
            prefixes.entry("torrust"),
            Some(&SetEntry::Text("torrust-".to_owned()))
        );

        let index = document.owner("INDEX").expect("the crate owner");

        assert_eq!(
            index.datum("cargo-toml"),
            Some(&OwnerDatum::Text("Cargo.toml".to_owned())),
            "the manifest stands directly on the naked owner table"
        );
        assert_eq!(
            index.selection("name-key"),
            Some(&Selection::Entry("cargo-package-name".to_owned())),
            "where exactly one entry applies, no list is written"
        );
        assert_eq!(
            index.selection("name-prefix-ignore"),
            Some(&Selection::Entry("torrust".to_owned()))
        );
        assert_eq!(
            index.data().len(),
            1,
            "the naked table and the typed sections are distinct"
        );
        assert_eq!(index.selections().len(), 2);
    }

    /// The three prefix-number payloads decode as three named entries of one
    /// set, each carrying the bound its own scheme had, and an owner's section
    /// selects among them by name. The merge is what the namespace ruling
    /// requires — one program acting on the sets is one document — and it
    /// repairs the collision of three files stamping one label between them.
    ///
    /// ´claim:declaration:one-program-s-payloads-merge-into-one-document-of-named-entries´
    /// ´test:unit:merges-one-program-s-payloads-into-one-document-of-named-entries´
    #[test]
    fn merges_one_program_s_payloads_into_one_document_of_named_entries() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.references.prefixed"
version = [1, 0, 0]

[set.prefix-numbers]
work-packages = { prefix = "WP-", exact = ["1", "2.1"] }
chapters = { prefix = "L-", leading-minimum = 1, leading-maximum = 30 }
records = { prefix = "L-", leading = [4, 9] }

[owners.QUARRY.prefix-numbers]
include = [
  { name = "quarry-prose", prefix-numbers = "work-packages", pattern = "%s\"quarry/docs\" [ \"/\" *VCHAR ]" },
  { name = "quarry-prose", prefix-numbers = "chapters", pattern = "%s\"quarry/docs\" [ \"/\" *VCHAR ]" },
]
exclude = [
  { name = "linter-package", pattern = "%s\"packages/linter\" [ \"/\" *VCHAR ]" },
]
"#,
        )
        .expect("the merged document");

        let set = document.set("prefix-numbers").expect("the merged set");

        assert_eq!(set.kind(), SetKind::PrefixNumbers);
        assert_eq!(set.entries().len(), 3, "one program's payloads are one set");

        assert_eq!(
            set.entry("work-packages"),
            Some(&SetEntry::PrefixNumber(PrefixNumberEntry::new(
                "WP-",
                PrefixBound::Exact(vec!["1".to_owned(), "2.1".to_owned()])
            )))
        );
        assert_eq!(
            set.entry("chapters"),
            Some(&SetEntry::PrefixNumber(PrefixNumberEntry::new(
                "L-",
                PrefixBound::LeadingRange {
                    minimum: 1,
                    maximum: 30
                }
            )))
        );
        assert_eq!(
            set.entry("records"),
            Some(&SetEntry::PrefixNumber(PrefixNumberEntry::new(
                "L-",
                PrefixBound::LeadingSet(vec![4, 9])
            )))
        );

        let rows = admitted(&document, "QUARRY", "prefix-numbers");

        assert_eq!(
            rows.iter().map(PatternRow::entry).collect::<Vec<_>>(),
            vec![Some("work-packages"), Some("chapters")],
            "two entries of one set are carried over one owner, each referenced under the set's own type"
        );
        assert_eq!(
            rows.iter().map(PatternRow::name).collect::<Vec<_>>(),
            vec!["quarry-prose", "quarry-prose"],
            "and the one region carrying both is named once per row rather than split in two"
        );

        match document
            .owner("QUARRY")
            .and_then(|owner| owner.selection("prefix-numbers"))
        {
            Some(Selection::Rows { exclude, .. }) => {
                assert_eq!(exclude.len(), 1);
                assert!(
                    exclude[0]
                        .pattern()
                        .admits("packages/linter/src/declaration.rs"),
                    "the exclusion reaches the subtree"
                );
            }
            _ => panic!("the section states pattern rows"),
        }
    }

    /// A parameterized entry decodes into the value its program consumes rather
    /// than into a parallel shape holding the same numbers, and the compiled half
    /// of the program is not declarable beside it. A second shape carrying one
    /// instance's mark and bound would be a second chance to disagree about what
    /// the bound meant.
    ///
    /// ´claim:declaration:a-parameterized-entry-decodes-into-the-program-s-own-value´
    /// ´test:unit:decodes-a-parameterized-entry-into-the-program-s-own-value´
    #[test]
    fn decodes_a_parameterized_entry_into_the_program_s_own_value() {
        let document = decode(
            r##"
namespace = "com.torrust.index.linter.policy.references.scenarios"
version = [1, 0, 0]

[set.numbered-marks]
hash-one-to-91 = { mark = "#", minimum = 1, maximum = 91 }
"##,
        )
        .expect("a marked-ordinal document");

        let set = document
            .set("numbered-marks")
            .expect("the marked-ordinal set");

        assert_eq!(set.kind(), SetKind::NumberedMarks);
        assert!(set.is_adopted(), "a program of this binary reads the type");
        assert_eq!(
            set.entry("hash-one-to-91"),
            Some(&SetEntry::NumberedMark(MarkNumbered::new('#', 1, 91))),
            "the decoded entry is the program's own value"
        );

        assert!(
            says(
                r#"
namespace = "com.torrust.index.linter.policy.references.scenarios"
version = [1, 0, 0]

[set.numbered-marks]
doubled = { mark = "@@", minimum = 1, maximum = 4 }
"#,
                "one character"
            ),
            "a mark of two characters is no mark"
        );

        assert!(
            says(
                r#"
namespace = "com.torrust.index.linter.policy.references.prefixed"
version = [1, 0, 0]

[set.prefix-numbers]
both = { prefix = "L-", leading = [1], leading-minimum = 1, leading-maximum = 3 }
"#,
                "exactly one of exact, leading and the leading range"
            ),
            "a scheme had one bound, and an entry declaring two says nothing"
        );

        assert!(
            says(
                r##"
namespace = "com.torrust.index.linter.policy.references.scenarios"
version = [1, 0, 0]

[set.numbered-marks]
shielded = { mark = "#", minimum = 1, maximum = 4, shielded = true }
"##,
                "shielded"
            ),
            "the compiled half of a program is not declarable beside the declared half"
        );
    }

    /// A simple entry is a named string and an owner deploys it through pattern
    /// rows, the include row naming its own region and the entry that region
    /// carries, and the exclude row naming the exclusion itself and carrying
    /// nothing. The pattern is the form and not a regular expression, and it
    /// decides a path over the whole path rather than over a prefix of it.
    ///
    /// ´claim:declaration:a-simple-entry-is-a-named-string-deployed-by-pattern-rows´
    /// ´test:unit:decodes-a-simple-entry-and-the-pattern-rows-that-deploy-it´
    #[test]
    fn decodes_a_simple_entry_and_the_pattern_rows_that_deploy_it() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"
recovery = "Recovery preserves state and failures stay explicit"

[owners.QUARRY.literals]
include = [
  { name = "quarry-prose", literals = "information", pattern = "%s\"quarry/docs\" [ \"/\" *VCHAR ]" },
  { name = "quarry-prose", literals = "recovery", pattern = "%s\"quarry/docs\" [ \"/\" *VCHAR ]" },
]
exclude = [
  { name = "readmes", pattern = "*( segment \"/\" ) %s\"README.md\"\n  segment = 1*( %x21-2E / %x30-7E )" },
]
"#,
        )
        .expect("a literal-set document");

        let set = document.set("literals").expect("the literal set");

        assert_eq!(
            set.entry("information"),
            Some(&SetEntry::Text(
                "Information flows strictly forward".to_owned()
            ))
        );

        let rows = admitted(&document, "QUARRY", "literals");
        let bound = rows[0].pattern();

        assert!(
            bound.admits("quarry/docs"),
            "the bound reaches the directory"
        );
        assert!(
            bound.admits("quarry/docs/plans/one.md"),
            "and everything beneath it"
        );
        assert!(
            !bound.admits("quarry/docsy/one.md"),
            "and stops at the separator"
        );
        assert!(
            !bound.admits("quarry"),
            "and does not reach a prefix of itself"
        );
        assert_eq!(bound.source(), "%s\"quarry/docs\" [ \"/\" *VCHAR ]");

        match document
            .owner("QUARRY")
            .and_then(|owner| owner.selection("literals"))
        {
            Some(Selection::Rows { exclude, .. }) => {
                let readmes = exclude[0].pattern();

                assert_eq!(
                    exclude[0].name(),
                    "readmes",
                    "an exclude row labels the exclusion"
                );
                assert_eq!(
                    exclude[0].entry(),
                    None,
                    "and carries no entry to label it with"
                );
                assert!(
                    readmes.admits("README.md"),
                    "a helper rule on a continuation line reads"
                );
                assert!(readmes.admits("quarry/docs/README.md"));
                assert!(!readmes.admits("quarry/docs/README.md.bak"));
            }
            _ => panic!("the section states pattern rows"),
        }
    }

    /// The licence-header precedent decodes under the one grammar: two sets of
    /// named strings, and one typed section per half per owner. It is the shape
    /// every entry-across-files declaration takes, so the document that
    /// established it needs no dialect of its own.
    ///
    /// ´claim:declaration:the-licence-header-precedent-is-the-grammar-s-own-shape´
    /// ´test:unit:decodes-the-two-licence-halves-as-named-entries-of-two-sets´
    #[test]
    fn decodes_the_two_licence_halves_as_named_entries_of_two_sets() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.licence.headers"
version = [1, 0, 0]

[set.identifier]
quarry-licence = "AGPL-3.0-only"

[set.copyright]
quarry-2026 = "2026 Quarry corpus contributors"

[owners.QUARRY.identifier]
partitions = [
  { name = "code", identifier = "quarry-licence", pattern = "*( segment \"/\" ) 1*segment %s\".rs\"\n  segment = 1*( %x21-2D / %x30-7E )" },
]
exclude = [
  { name = "generated", pattern = "%s\"quarry/src/generated\" [ \"/\" *VCHAR ]" },
]

[owners.QUARRY.copyright]
partitions = [
  { name = "code", copyright = "quarry-2026", pattern = "*( segment \"/\" ) 1*segment %s\".rs\"\n  segment = 1*( %x21-2D / %x30-7E )" },
]
"#,
        )
        .expect("a licence-header document");

        assert_eq!(
            document.set("identifier").map(DeclaredSet::kind),
            Some(SetKind::Identifier)
        );
        assert_eq!(
            document.set("copyright").map(DeclaredSet::kind),
            Some(SetKind::Copyright)
        );

        let sources = admitted(&document, "QUARRY", "identifier")[0].pattern();

        assert!(
            sources.admits("quarry/src/mine.rs"),
            "the half reaches a source at depth"
        );
        assert!(sources.admits("mine.rs"), "and one at the root");
        assert!(
            !sources.admits("quarry/docs/mine.md"),
            "and no document beside it"
        );

        assert_eq!(
            admitted(&document, "QUARRY", "copyright")[0].entry(),
            Some("quarry-2026"),
            "each half is its own set and its own section"
        );
        assert_eq!(
            admitted(&document, "QUARRY", "copyright")[0].name(),
            "code",
            "and the region both halves partition is the row's own name in each"
        );
    }

    /// A publication is a named inline table on the naked owner table, typed by
    /// the document's namespace and attributed by the table it stands on, so the
    /// retired `owner` field has no successor and no set stands behind it.
    ///
    /// ´claim:declaration:a-publication-is-a-named-table-on-the-naked-owner-table´
    /// ´test:unit:decodes-publications-as-named-tables-on-the-naked-owner-table´
    #[test]
    fn decodes_publications_as_named_tables_on_the_naked_owner_table() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.assembly.publications"
version = [1, 0, 0]

[owners.QUARRY]
spec = { parts = "quarry/docs/spec", target = "quarry/docs/spec.md" }
manual = { parts = "quarry/docs/manual", target = "quarry/docs/manual.md" }
"#,
        )
        .expect("a publications document");

        assert!(
            document.sets().is_empty(),
            "owner-bound data needs no set behind it"
        );

        let quarry = document.owner("QUARRY").expect("the publishing owner");
        let Some(OwnerDatum::Publication(publication)) = quarry.datum("spec") else {
            panic!("a publication row")
        };

        assert_eq!(
            publication.owner(),
            "QUARRY",
            "the table it stands on attributes it"
        );
        assert_eq!(
            publication.assembly().target().display().to_string(),
            "quarry/docs/spec.md"
        );
        assert_eq!(
            quarry.data().len(),
            2,
            "each publication is one named entry"
        );
        assert!(quarry.selections().is_empty());
    }

    /// Each retired key refuses by name and says what replaced it: the two
    /// envelope keys the namespace absorbed, the parameter table the named sets
    /// replaced, the scope and register blocks the owner sections and the
    /// register retirement left with no work, the program-owned list that was
    /// never repository data, the owner document's exclusion key the shape
    /// document already holds, and the plural spelling of the set table.
    /// Ignoring one would let a pre-grammar document load with half of what it
    /// declared unread.
    ///
    /// ´claim:declaration:every-retired-key-refuses-by-name-with-its-successor´
    /// ´test:unit:refuses-every-key-the-ratified-grammar-retired´
    #[test]
    fn refuses_every_key_the_ratified_grammar_retired() {
        let document = r##"
namespace = "com.torrust.index.linter.policy.references.scenarios"
version = [1, 0, 0]

policy = "references.mark-numbered-absent"
family = "scenarios"
types = ["json", "toml"]
excluded = []

[parameters]
mark = "#"

[scope]
include = []

[register]
destination = "quarry/docs/plans/burn/scenario-numbers.md"

[sets.numbered-marks]
hash = { mark = "#", minimum = 1, maximum = 4 }
"##;

        let refused = defects(document);

        for retired in [
            RetiredKey::Policy,
            RetiredKey::Family,
            RetiredKey::Parameters,
            RetiredKey::Scope,
            RetiredKey::Register,
            RetiredKey::Types,
            RetiredKey::Excluded,
            RetiredKey::PluralSets,
        ] {
            assert!(
                refused.contains(&DeclarationDefect::Retired { retired }),
                "the retired key `{}` refuses by name",
                retired.key()
            );
        }

        assert!(says(
            document,
            "the namespace is the document's whole identity"
        ));
        assert!(says(
            document,
            "one program acting on several sets is one document"
        ));
        assert!(says(
            document,
            "data stands in singular set tables of named entries"
        ));
        assert!(says(
            document,
            "the owner sections say which paths an instance reaches"
        ));
        assert!(says(document, "the registers retire outright"));
        assert!(says(
            document,
            "compiled meaning rather than repository data"
        ));
        assert!(says(document, "the shape document's ignore relation"));
        assert!(says(document, "the set table is singular"));
    }

    /// An include row and a singular reference both name an entry of their
    /// section's set, so a name resolving to nothing is a defect naming the
    /// owner, the type and the name. An exclude row is not held to it, because
    /// its name labels the exclusion rather than promising an entry.
    ///
    /// ´claim:declaration:a-reference-into-a-set-resolves-or-refuses´
    /// ´test:unit:refuses-a-reference-to-an-entry-no-set-declares´
    #[test]
    fn refuses_a_reference_to_an_entry_no_set_declares() {
        let dangling = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
include = [
  { name = "quarry-tree", literals = "recovery", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(dangling).contains(&DeclarationDefect::DanglingReference {
                owner: "QUARRY".to_owned(),
                set: "literals".to_owned(),
                name: "recovery".to_owned(),
            }),
            "an include row names an entry of its set"
        );

        let singular = r#"
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[set.name-key]
cargo-package-name = "name"

[owners.QUARRY.name-key]
use = "package-name"
"#;

        assert!(says(singular, "no entry of that set answers to this name"));

        let untyped = r#"
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[owners.QUARRY.name-key]
use = "cargo-package-name"
"#;

        assert!(
            defects(untyped).contains(&DeclarationDefect::UnknownSetType {
                owner: "QUARRY".to_owned(),
                set: "name-key".to_owned(),
            }),
            "a section referencing entries names a set the document declares"
        );

        let labels = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
exclude = [
  { name = "readmes", pattern = "*VCHAR" },
]
"#;

        assert!(
            decode(labels).is_ok(),
            "an exclude row's name labels the exclusion and promises no entry"
        );
    }

    /// A row carries two declarations the retiring spelling wrote as one, and
    /// each half missing refuses saying what is owed. A row with no name is a
    /// region no report can say; a row admitting sources with no reference under
    /// its set's own type is the retired spelling, where the reference stood as
    /// the name; and two rows carrying one entry are two answers to where that
    /// entry reaches. An author who wrote any of the three was writing what was
    /// lawful before the ruling, so what they are owed is the sentence saying
    /// which word carries the declaration now rather than a report that some key
    /// was unknown.
    ///
    /// ´claim:declaration:the-retired-row-spelling-refuses-naming-what-it-owes´
    /// ´test:unit:refuses-the-row-spelling-the-ruling-replaced´
    #[test]
    fn refuses_the_row_spelling_the_ruling_replaced() {
        let ratified = r#"
namespace = "com.torrust.index.linter.policy.spdx"
version = [1, 0, 0]

[set.identifier]
agpl3only = "AGPL-3.0-only"
mit = "MIT"

[owners.QUARRY.identifier]
partitions = [
  { name = "code", identifier = "agpl3only", pattern = '%s"quarry/src" [ "/" *VCHAR ]' },
]
"#;

        assert!(decode(ratified).is_ok(), "the ratified spelling stands");

        // The retired spelling: the reference written as the row's name, and so
        // no name for the region left over.
        let retired = ratified.replace(
            r#"name = "code", identifier = "agpl3only""#,
            r#"name = "agpl3only""#,
        );

        assert!(
            defects(&retired).contains(&DeclarationDefect::AbsentReference {
                relation: String::from("owners.QUARRY.identifier.partitions"),
                set: String::from("identifier"),
            }),
            "{:?}",
            defects(&retired)
        );
        assert!(
            says(
                &retired,
                "under the set's own type, name being the row's own region"
            ),
            "and the refusal names the word that carries the reference now"
        );

        // The other half missing: a reference and no region to carry it over.
        let nameless = ratified.replace(
            r#"name = "code", identifier = "agpl3only""#,
            r#"identifier = "agpl3only""#,
        );

        assert!(
            defects(&nameless).contains(&DeclarationDefect::NamelessRow {
                relation: String::from("owners.QUARRY.identifier.partitions"),
            }),
            "{:?}",
            defects(&nameless)
        );
        assert!(says(&nameless, "a region no report can say"));

        // An exclusion carries the name alone, so the nameless refusal reaches it
        // too and the reference refusal never does.
        let unlabelled = ratified.replace(
            r#"  { name = "code", identifier = "agpl3only", pattern = '%s"quarry/src" [ "/" *VCHAR ]' },"#,
            r#"  { name = "code", identifier = "agpl3only", pattern = '%s"quarry/src" [ "/" *VCHAR ]' },
]
exclude = [
  { pattern = '%s"quarry/src/generated" [ "/" *VCHAR ]' },"#,
        );

        assert!(
            defects(&unlabelled).contains(&DeclarationDefect::NamelessRow {
                relation: String::from("owners.QUARRY.identifier.exclude"),
            }),
            "{:?}",
            defects(&unlabelled)
        );

        // Two rows named apart still answer one question twice where they carry
        // one entry, and the refusal names the entry rather than either row.
        let twice = ratified.replace(
            r#"  { name = "code", identifier = "agpl3only", pattern = '%s"quarry/src" [ "/" *VCHAR ]' },"#,
            r#"  { name = "code", identifier = "agpl3only", pattern = '%s"quarry/src" [ "/" *VCHAR ]' },
  { name = "prose", identifier = "agpl3only", pattern = '%s"quarry/docs" [ "/" *VCHAR ]' },"#,
        );

        assert!(
            defects(&twice).contains(&DeclarationDefect::DuplicateReference {
                relation: String::from("owners.QUARRY.identifier.partitions"),
                set: String::from("identifier"),
                name: String::from("agpl3only"),
            }),
            "{:?}",
            defects(&twice)
        );

        // Two rows carrying different entries over regions named apart is the
        // ordinary case the rule leaves alone.
        let apart = twice.replace(
            r#"name = "prose", identifier = "agpl3only""#,
            r#"name = "prose", identifier = "mit""#,
        );

        assert!(decode(&apart).is_ok(), "{:?}", defects(&apart));
    }

    /// A section admits its rows under the word its set type calls for:
    /// `partitions` where the entries divide what the exclusions left, `include`
    /// where they are deployed over reaches that may overlap. Each wrong
    /// direction refuses naming the word that was owed, because the two are one
    /// relation in shape and a decoder taking either would carry whichever
    /// promise the author happened to reach for. A type this binary reads no
    /// program for takes either word, and refuses only both at once.
    ///
    /// ´claim:declaration:the-set-type-fixes-the-word-that-admits-its-rows´
    /// ´test:unit:refuses-the-admitting-word-a-section-s-entries-do-not-call-for´
    #[test]
    fn refuses_the_admitting_word_a_section_s_entries_do_not_call_for() {
        let deploying_word = r#"
namespace = "com.torrust.index.linter.policy.spdx"
version = [1, 0, 0]

[set.identifier]
agpl3only = "AGPL-3.0-only"

[owners.QUARRY.identifier]
include = [
  { name = "code", identifier = "agpl3only", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(deploying_word).contains(&DeclarationDefect::WrongAdmission {
                owner: "QUARRY".to_owned(),
                set: "identifier".to_owned(),
                admission: Admission::Partition,
            }),
            "a licence half divides its survivors and is admitted by partitions"
        );

        assert!(
            says(deploying_word, "partitions is the word that admits them"),
            "and the refusal names the word that was owed"
        );

        let partitioning_word = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
partitions = [
  { name = "quarry-tree", literals = "information", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(partitioning_word).contains(&DeclarationDefect::WrongAdmission {
                owner: "QUARRY".to_owned(),
                set: "literals".to_owned(),
                admission: Admission::Deployment,
            }),
            "twelve literals deliberately share one region, and a deployment is admitted by include"
        );

        assert!(
            says(partitioning_word, "include is the word that admits them"),
            "and that refusal names its own owed word"
        );

        // A type no program of this binary reads is not held to either word,
        // because fixing one would decide what its entries do. Both at once is
        // still two answers to one question.
        let unadopted = r#"
namespace = "com.torrust.index.linter.policy.quarry.seams"
version = [1, 0, 0]

[set.seams]
adit = "adit"

[owners.QUARRY.seams]
partitions = [
  { name = "quarry-tree", seams = "adit", pattern = "*VCHAR" },
]
"#;

        assert!(
            decode(unadopted).is_ok(),
            "an unadopted type takes either word"
        );

        assert!(
            decode(&unadopted.replace("partitions = [", "include = [")).is_ok(),
            "and takes the other one just as readily"
        );

        let both = r#"
namespace = "com.torrust.index.linter.policy.quarry.seams"
version = [1, 0, 0]

[set.seams]
adit = "adit"

[owners.QUARRY.seams]
partitions = [
  { name = "quarry-tree", seams = "adit", pattern = "*VCHAR" },
]
include = [
  { name = "quarry-tree", seams = "adit", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(both).contains(&DeclarationDefect::SelectionConflict {
                owner: "QUARRY".to_owned(),
                set: "seams".to_owned(),
            }),
            "two words for one relation is two answers to one question"
        );
    }

    /// A section states one selection shape. A `use` key beside include rows is
    /// two answers to one question, and picking either would make an owner's
    /// reach depend on which the decoder happened to read first. An exclusion is
    /// not a second answer to it — it says what is cut out of whatever was
    /// selected — so it composes with either shape rather than conflicting with
    /// one.
    ///
    /// ´claim:declaration:a-section-states-one-selection-shape´
    /// ´test:unit:refuses-a-singular-reference-standing-beside-pattern-rows´
    #[test]
    fn refuses_a_singular_reference_standing_beside_pattern_rows() {
        let conflicted = r#"
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[set.name-key]
cargo-package-name = "name"

[owners.QUARRY.name-key]
use = "cargo-package-name"
include = [
  { name = "quarry-manifest", name-key = "cargo-package-name", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(conflicted).contains(&DeclarationDefect::SelectionConflict {
                owner: "QUARRY".to_owned(),
                set: "name-key".to_owned(),
            }),
            "two shapes in one section is two answers to one question"
        );
        assert!(says(
            conflicted,
            "a section states one selection shape, and this states two"
        ));

        let rowless = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[owners.QUARRY]
exclude = [
  { name = "readmes", pattern = "*VCHAR" },
]
"#;

        assert!(
            defects(rowless).contains(&DeclarationDefect::UntypedRows {
                owner: "QUARRY".to_owned(),
                relation: "exclude".to_owned(),
            }),
            "pattern rows stand in a typed section rather than on the naked owner table"
        );
    }

    /// A section deploys one entry by naming it, and the name promises an entry.
    /// One spelling and no second one: the list spelling retired with the
    /// owner-areas experiment that wanted it, so a document writing a list of
    /// names is refused rather than read as a deployment the grammar no longer
    /// has. A name resolving to no entry refuses where it is written, which is
    /// what keeps a misspelt entry name from becoming a selection of nothing.
    ///
    /// ´claim:declaration:a-section-names-the-one-entry-it-deploys´
    /// ´test:unit:decodes-a-section-naming-the-one-entry-it-deploys´
    #[test]
    fn decodes_a_section_naming_the_one_entry_it_deploys() {
        let singular = r#"
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[set.name-key]
cargo-package-name = "name"

[owners.QUARRY.name-key]
use = "cargo-package-name"
"#;

        assert_eq!(
            decode(singular)
                .expect("the singular shape")
                .owner("QUARRY")
                .and_then(|owner| owner.selection("name-key")),
            Some(&Selection::Entry("cargo-package-name".to_owned()))
        );

        let dangling = singular.replace(r#"use = "cargo-package-name""#, r#"use = "package-name""#);

        assert!(
            defects(&dangling).contains(&DeclarationDefect::DanglingReference {
                owner: "QUARRY".to_owned(),
                set: "name-key".to_owned(),
                name: "package-name".to_owned(),
            }),
            "a name promises an entry of the set it stands over"
        );

        // The list spelling is gone rather than tolerated, because a grammar
        // keeping a shape nothing writes keeps a second answer waiting for the
        // question.
        let listed = singular.replace(
            r#"use = "cargo-package-name""#,
            r#"use = ["cargo-package-name"]"#,
        );

        assert!(
            says(&listed, "a name"),
            "a section names one entry, and a list is not a name"
        );
    }

    /// A pattern the engine will not compile refuses where it is written and
    /// carries the engine's own defect inside, so the position the form stopped
    /// at reaches the reader. A reference to a rule nothing defines refuses the
    /// same way, which is what keeps a misspelt rule name from becoming a bound
    /// that accounts nothing.
    ///
    /// ´claim:declaration:a-pattern-the-form-declines-refuses-carrying-the-engine-s-defect´
    /// ´test:unit:refuses-a-pattern-the-form-does-not-read´
    #[test]
    fn refuses_a_pattern_the_form_does_not_read() {
        let unreadable = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
include = [
  { name = "quarry-tree", literals = "information", pattern = "%s\"quarry/docs" },
]
"#;

        let refused = defects(unreadable);

        assert!(
            matches!(
                refused.as_slice(),
                [DeclarationDefect::MalformedPattern { name, defect, .. }]
                    if name == "quarry-tree" && defect.position().is_some()
            ),
            "the engine's own defect is carried inside, position and all"
        );

        let undefined = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
include = [
  { name = "quarry-tree", literals = "information", pattern = "1*VISIBLE" },
]
"#;

        assert!(
            says(undefined, "names no rule"),
            "a reference to nothing is a defect"
        );

        let regular = r#"
namespace = "com.torrust.index.linter.policy.references.divisions"
version = [1, 0, 0]

[set.literals]
information = "Information flows strictly forward"

[owners.QUARRY.literals]
include = [
  { name = "quarry-tree", literals = "information", pattern = "^quarry/docs(?:/.*)?$" },
]
"#;

        assert!(
            decode(regular).is_err(),
            "a regular expression is not a second accepted dialect"
        );
    }

    /// A set type this binary catalogues no program for decodes structurally
    /// with its entries kept by name and the set marked unadopted. Refusing it
    /// would make the decoder the adoption authority, and adoption is an owner
    /// act recorded outside configuration.
    ///
    /// ´claim:declaration:an-unadopted-set-type-decodes-structurally-and-is-marked´
    /// ´test:unit:keeps-a-set-no-program-claims-and-marks-it-unadopted´
    #[test]
    fn keeps_a_set_no_program_claims_and_marks_it_unadopted() {
        let document = decode(
            r#"
namespace = "com.torrust.index.linter.policy.quarry.seams"
version = [1, 0, 0]

[set.seam-widths]
narrow = { millimetres = 4, dressed = true }
wide = "as-quarried"

[owners.QUARRY.seam-widths]
use = "narrow"
"#,
        )
        .expect("an unadopted set decodes");

        let set = document.set("seam-widths").expect("the unadopted set");

        assert_eq!(set.kind(), SetKind::Unadopted);
        assert!(
            !set.is_adopted(),
            "the catalogue decides adoption elsewhere"
        );
        assert_eq!(set.entries().len(), 2, "the entries are kept by name");
        assert_eq!(
            set.entry("wide"),
            Some(&SetEntry::Text("as-quarried".to_owned()))
        );

        let Some(SetEntry::Fields(fields)) = set.entry("narrow") else {
            panic!("a parameterized entry kept by form")
        };

        assert_eq!(fields.get("millimetres"), Some(&Scalar::Number(4)));
        assert_eq!(fields.get("dressed"), Some(&Scalar::Truth(true)));

        assert_eq!(
            document
                .owner("QUARRY")
                .and_then(|owner| owner.selection("seam-widths")),
            Some(&Selection::Entry("narrow".to_owned())),
            "a reference into an unadopted set resolves like any other"
        );
    }

    /// A document identifies itself with a dotted namespace and a version, and a
    /// missing or malformed envelope refuses before anything reads what it
    /// declares. A key outside the grammar refuses too: an unread key is a
    /// declaration whose author believes something the checker never learnt.
    ///
    /// ´claim:declaration:a-document-identifies-itself-before-it-declares-anything´
    /// ´test:unit:requires-the-envelope-that-identifies-a-document´
    #[test]
    fn requires_the_envelope_that_identifies_a_document() {
        let refused =
            defects("[set.literals]\ninformation = \"Information flows strictly forward\"\n");

        assert!(refused.contains(&DeclarationDefect::AbsentEnvelope { key: "namespace" }));
        assert!(refused.contains(&DeclarationDefect::AbsentEnvelope { key: "version" }));

        assert!(
            says(
                "namespace = \"quarry\"\nversion = [1, 0, 0]\n",
                "a dotted identity of declared-name components"
            ),
            "an undotted namespace names no schema"
        );
        assert!(
            says(
                "namespace = \"com.torrust.index.linter.policy.quarry\"\nversion = \"1.0.0\"\n",
                "three whole numbers"
            ),
            "a version is three numbers rather than a rendering of them"
        );

        assert!(
            says(
                r#"
namespace = "com.torrust.index.linter.policy.quarry"
version = [1, 0, 0]
universe = "git-tracked"
"#,
                "the grammar has no such declaration"
            ),
            "a key the grammar never had would be read by nothing"
        );

        assert!(
            decode("namespace = \"com.torrust.index.linter.policy.quarry\"\nversion = [1, 0, 0]\n")
                .is_ok(),
            "an envelope alone is a document declaring nothing yet"
        );
        assert!(
            !defects("namespace = ").is_empty(),
            "text that is no document at all refuses as one defect"
        );
    }
}
