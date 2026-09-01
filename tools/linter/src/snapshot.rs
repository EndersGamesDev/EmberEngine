// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The declared configuration snapshot: five files read as one, or not at all.
//!
//! The five declarations are one snapshot rather than five settings. Every
//! command reads them before it does anything else, requires exactly those
//! filenames, parses all five, cross-validates them against each other, and only
//! then runs a policy. There is no partial load and no per-file default: a
//! command that cannot read its own configuration has no standing to say anything
//! about the corpus (´spec:commandcontract:configuration´).
//!
//! Two questions are asked in order, and they get different answers. The first is
//! whether the snapshot is a snapshot at all — whether it parses, whether every
//! name it uses is a name this binary knows, whether no row is written twice, and
//! whether every activated pair has exactly one list in the codec its policy
//! selects, and whether every prerequisite whose owner the declaration fixes is
//! activated. A snapshot that fails any of those is a *refused precondition*:
//! the command exits with the shared failure class, stdout stays empty, no policy
//! runs and no writing mode changes a byte. The second question is whether the
//! snapshot is coherent as a description of this repository, and that one is
//! judged rather than refused — the answers are ordinary findings of failing
//! severity (´rule:commandcontract:configuration-verdicts´). This module owns
//! the first question entire; the second is the partition verifier's and the
//! cited-owner dependency validator's.
//!
//! Absence refuses the precondition. Without the shape declaration, neither
//! the repository universe nor the global-ignore relation can be resolved, so
//! no command may reconstruct a corpus from compiled declarations.
//!
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::areas::{self, CORPUS_ROOT, partition_root};
use crate::assembly::Publication;
use crate::burn::Surface as BurnSurface;
use crate::catalogue::{
    Codec, Scope, catalogued, is_declared_name, is_policy_identifier, program_of, spans_corpus,
};
use crate::declaration::{
    AbnfPattern, Declaration, OwnerDatum, OwnerDeclaration, PatternRow, Selection, SetEntry,
};
use crate::interchange::{
    Parameters as InterchangeParameters, Section as InterchangeSection,
    SectionRow as InterchangeRow, declared_refusal_from_parsed,
};
use crate::label::Prefix;
use crate::pattern::{BytePath, PathDefect};
use crate::program::{LiteralSet, MarkNumbered, PrefixNumbers};
use crate::reference::{
    Parameters as ReferenceParameters, Section as ReferenceSection, SectionRow as ReferenceRow,
};
use crate::registry::KindRegistry;
use crate::roster::{OwnerNames, UnbuiltMember};
use crate::spdx::{
    HALVES, Half, HalfSection, Parameters, Section, SectionRow, is_copyright_text,
    is_licence_expression,
};
use crate::surface::{DocumentDefect, SurfaceAst};
use crate::universe::{CorpusShape, decode_shape_table};

/// The policy whose parameters the fifth file carries.
const SPDX_POLICY: &str = "spdx.headers-conform";

/// The policy whose parameters the sixth file carries.
const INTERCHANGE_POLICY: &str = "interchange.envelope-conform";

/// The policy whose parameters the seventh file carries.
const REFERENCES_POLICY: &str = "references.file-paths-absent";

/// The directory the declared surface stands in, relative to the repository root.
pub const DIRECTORY: &str = ".linter";

/// The owner file: owner identity, the partition, the exclusion set and the may-cite relation.
pub const OWNERS_FILE: &str = "owners.toml";

/// The environment relation file.
pub const ENVIRONMENTS_FILE: &str = "environments.toml";

/// The owner-and-policy activation file.
pub const POLICIES_FILE: &str = "policies.toml";

/// The file carrying the list at each activated pair.
pub const LISTS_FILE: &str = "lists.toml";

/// The allocated namespace of the SPDX policy's instance document.
///
/// The document is found by the identity it stamps rather than by the name it
/// happens to stand under, because a namespace is a document's whole identity
/// under the ratified grammar and a filename is only where it was put
/// (´gram:isolation:declaration´).
pub const SPDX_NAMESPACE: &str = "com.torrust.index.linter.policy.spdx";

/// The allocated namespace of the interchange policy's instance document.
pub const INTERCHANGE_NAMESPACE: &str = "com.torrust.index.linter.policy.interchange";

/// The allocated namespace of the file-path policy's instance document.
pub const PATH_LINKING_NAMESPACE: &str = "com.torrust.index.linter.policy.references.path-linking";

/// The allocated namespace of the publication policy's instance document.
pub const ASSEMBLY_PUBLICATIONS_NAMESPACE: &str =
    "com.torrust.index.linter.policy.assembly-publications";

/// The allocated namespace of the owner-name policy's instance document.
pub const OWNER_NAMES_NAMESPACE: &str = "com.torrust.index.linter.policy.owner.names";

/// The allocated namespace of the scenario policy's instance document.
pub const SCENARIOS_NAMESPACE: &str = "com.torrust.index.linter.policy.references.scenarios";

/// The allocated namespace of the division policy's instance document.
pub const DIVISIONS_NAMESPACE: &str = "com.torrust.index.linter.policy.references.divisions";

/// The allocated namespace of the prefix-number policy's instance document.
pub const PREFIX_NUMBERS_NAMESPACE: &str =
    "com.torrust.index.linter.policy.references.prefix-numbers";

/// The run every instance document's allocated namespace opens with.
///
/// What follows it is the domain the document declares, which is a policy
/// identifier wherever a policy has a document of its own. A census asking where
/// it walks therefore asks for its own policy by name and no compiled table of
/// filenames stands between the question and the answer
/// (´gram:isolation:declaration´).
pub const POLICY_NAMESPACE: &str = "com.torrust.index.linter.policy.";

/// The datum a policy's document writes its prose surface under.
const PROSE_DATUM: &str = "prose";

/// The datum a policy's document writes its code surface under.
const CODE_DATUM: &str = "code";

/// The set type the SPDX document's identifier half stands over.
const IDENTIFIER_SET: &str = "identifier";

/// The set type the SPDX document's copyright half stands over.
const COPYRIGHT_SET: &str = "copyright";

/// The set type the interchange document's owner sections stand over.
const INTERCHANGE_SET: &str = "interchange-documents";

/// The set type the file-path document's owner sections stand over.
const PATH_LINKING_SET: &str = "path-references";

/// The set type the scenario document's marked ordinals stand in.
const NUMBERED_MARKS_SET: &str = "numbered-marks";

/// The set type the division document's verbatim sentences stand in.
const LITERALS_SET: &str = "literals";

/// The set type the prefix-number document's schemes stand in.
const PREFIX_NUMBERS_SET: &str = "prefix-numbers";

/// The set type the owner-name document's stripped prefixes stand in.
const NAME_PREFIX_IGNORE_SET: &str = "name-prefix-ignore";

/// The datum naming a registered crate the workspace does not yet build.
const CRATE_NAME_DATUM: &str = "crate-name";

/// The datum naming the directory an unbuilt registered crate's prose stands under.
const PACKAGE_DIRECTORY_DATUM: &str = "package-directory";

/// The file carrying the corpus universe answer and the global-ignore relation.
///
/// The loader reads it and is grounded below both declarations it supplies: the
/// directory is read physically and byte for byte, never consulting git and never
/// applying the ignore relation, including while reading this file itself
/// (´just:isolation:policy-data´).
pub const SHAPE_FILE: &str = "shape.toml";

/// The declarations a snapshot requires whatever a repository activates.
///
/// Five rather than seven. The three parameter documents this loader used to
/// require by name are found by their allocated namespaces instead, and the
/// shape document joins the core on the fixed core's own argument: a member the
/// set does not name is a refusal wherever it stands, and a conditional member
/// would buy two failure modes to save writing one small file
/// (´just:isolation:policy-data´).
///
/// Required is not the same set as recognized. Membership is physical: a present
/// document whose name identifies a policy declaration joins this core rather
/// than refusing beside it (´dec:snapshot:physical-membership´), which is what
/// lets a declared surface grow a parameter document without a compiled filename
/// list growing with it.
pub const FILES: [&str; 5] = [
    OWNERS_FILE,
    ENVIRONMENTS_FILE,
    POLICIES_FILE,
    LISTS_FILE,
    SHAPE_FILE,
];

/// The name every parameter document opens with.
const POLICY_PREFIX: &str = "policy-";

/// The carrier suffix the declared surface is written in.
const TOML_SUFFIX: &str = ".toml";

/// Whether a directory entry's name is a parameter document's.
///
/// The rule stands here rather than in each reader of the directory, because two
/// modules deciding separately what a policy declaration is called are two places
/// for the answer to drift.
#[must_use]
pub fn is_policy_document(name: &str) -> bool {
    name.strip_prefix(POLICY_PREFIX)
        .and_then(|rest| rest.strip_suffix(TOML_SUFFIX))
        .is_some_and(is_declared_name)
}

/// Whether a directory entry's name is a member of the declared surface at all.
///
/// Recognition is wider than requirement and narrower than anything: the required
/// core and any parameter document. An entry that is neither is a refusal wherever
/// it stands, so the directory remains a closed surface without being a closed
/// list of filenames.
#[must_use]
pub fn is_declared_member(name: &str) -> bool {
    FILES.contains(&name) || is_policy_document(name)
}

/// The prefix a well-formed fingerprint carries.
const FINGERPRINT_PREFIX: &str = "sha256:";

/// How many hexadecimal digits a fingerprint carries after its prefix.
const FINGERPRINT_DIGITS: usize = 64;

/// One owner-and-policy pair: the unit enforcement is declared at.
///
/// The policy half is the full declared key rather than the program identifier
/// alone. A program deployed once is named by its identifier and carries no
/// family; a program a corpus deploys several times is named by its identifier
/// and the family naming one deployment, and the two deployments are two pairs
/// that can neither satisfy nor pay for one another
/// (ADR-T-020, The migration disciplines). What grows is the pair's
/// second component and never its arity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Pair {
    /// The owner the policy governs.
    pub owner: String,
    /// The program governing it, or the policy namespace naming its document.
    ///
    /// Which of the two it is, is decided by the entry beside it and never
    /// guessed at: a key naming a set entry is a key into an instance document,
    /// so its middle component is that document's namespace relative to the
    /// corpus prefix, and a key naming none is a program's own.
    pub policy: String,
    /// The set entry naming one deployment, where the key names one.
    ///
    /// Absent is the ordinary state and is written as nothing rather than as an
    /// empty entry, so a report of a program's own pair reads exactly as it
    /// always did and no consumer has to learn a second spelling of *no entry*.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
}

impl Pair {
    /// The pair a singleton program is activated at.
    #[must_use]
    pub fn singleton(owner: impl Into<String>, policy: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            policy: policy.into(),
            family: None,
        }
    }
}

impl fmt::Display for Pair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.family {
            Some(family) => write!(formatter, "{} : {} ({family})", self.owner, self.policy),
            None => write!(formatter, "{} : {}", self.owner, self.policy),
        }
    }
}

/// One partition row: the region it names, the owner that region answers to, and
/// the paths it reaches.
///
/// The name is the row's own and references nothing: the owner is already the
/// type-named reference, and what the name adds is the word a report says when
/// this row is the one at fault. Two rows of one owner were otherwise tellable
/// apart only by their patterns, which is a report asking its reader to read a
/// grammar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OwnerRow {
    /// The region the row claims.
    pub name: String,
    /// The owner the row heads.
    pub owner: String,
    /// The paths the row names.
    pub pattern: AbnfPattern,
}

impl fmt::Display for OwnerRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} : {} : {}",
            self.name, self.owner, self.pattern
        )
    }
}

/// One row of the declared may-cite relation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ReachRow {
    /// The owner whose reach the row states.
    pub owner: String,
    /// The owner it reaches.
    pub target: String,
}

/// One row of the declared environment relation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct EnvironmentRow {
    /// The environment name a head is written with.
    pub environment: String,
    /// The kind token its mint carries.
    pub kind: String,
}

/// One tolerated violation identified by a digest of its own stable fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Allowance {
    /// The violation's fingerprint.
    pub fingerprint: String,
    /// How many occurrences of it are tolerated.
    pub maximum: u64,
}

/// One tolerated file, with the ceiling every occurrence in it counts against.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PathCount {
    /// The file the row names.
    pub path: BytePath,
    /// How many occurrences in it are tolerated.
    pub maximum: u64,
}

/// The rows carried at one pair, in the codec its policy selects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Rows {
    /// The fingerprint codec's rows.
    Allowances(Vec<Allowance>),
    /// The legacy path-count codec's rows.
    PathCounts(Vec<PathCount>),
    /// The path-set codec's rows, each a bare path.
    Paths(Vec<BytePath>),
}

impl Rows {
    /// The codec these rows are written in.
    #[must_use]
    pub const fn codec(&self) -> Codec {
        match self {
            Self::Allowances(_) => Codec::Fingerprint,
            Self::PathCounts(_) => Codec::PathCount,
            Self::Paths(_) => Codec::PathSet,
        }
    }

    /// How many rows the list carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        match self {
            Self::Allowances(rows) => rows.len(),
            Self::PathCounts(rows) => rows.len(),
            Self::Paths(rows) => rows.len(),
        }
    }

    /// Whether the list is retained and empty, which is a live statement that nothing is tolerated.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Why a snapshot is refused rather than judged.
///
/// Every variant is a defect of the snapshot as a snapshot, which is the class
/// that exits with the shared failure code and an empty stdout. A defect in how
/// the snapshot describes the repository is not here — that is a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    /// One required declaration file does not stand.
    MissingFile {
        /// The file that is not there.
        file: &'static str,
    },
    /// The declaration directory carries an entry that is none of the declared files.
    UnknownMember {
        /// The entry's name, in the reversible byte display.
        name: String,
    },
    /// A declared file could not be read.
    Unreadable {
        /// The file that could not be read.
        file: &'static str,
        /// What the filesystem said.
        error: String,
    },
    /// A declared file is not well-formed, which includes carrying a key the schema does not have.
    Malformed {
        /// The file that is not well-formed.
        file: &'static str,
        /// What the parser said.
        message: String,
    },
    /// A declared file carries a key the surface retired, reported with what replaced it.
    ///
    /// The key stands here rather than among the unknown ones because a reader who
    /// wrote it was not guessing: they were writing a spelling that was lawful
    /// before the ruling, and what they are owed is the sentence saying where that
    /// declaration lives now.
    Retired {
        /// The file the retired key stands in.
        file: &'static str,
        /// The key as a retiring document spells it.
        key: &'static str,
        /// What carries the declaration under the ruling.
        replacement: &'static str,
    },
    /// A declared file's envelope is absent or defective, so the file has made no claim to read.
    ///
    /// The envelope of a declared file is read before that file's content, and a
    /// defect refuses in the class a lexical error already occupies. A file whose
    /// envelope is malformed has not made a claim the loader can misread; it has
    /// made no claim, and there is nothing for satisfaction to hold of.
    Envelope {
        /// The refusal text, which the envelope machinery owns because the
        /// requirement is the record's rather than this module's.
        text: String,
    },
    /// A row names an owner the owner list does not register.
    UnknownOwner {
        /// The file the row stands in.
        file: &'static str,
        /// The owner the row named.
        owner: String,
    },
    /// A registered owner identifier is not a well-formed citation prefix.
    MalformedOwner {
        /// The identifier as it stood.
        owner: String,
    },
    /// A may-cite row states the reach every owner has to itself.
    ///
    /// Self-citation is structural rather than declared: an owner reaches its own
    /// corpus because it is that corpus, and no declaration could withhold the
    /// reach or grant it. A row spelling it is therefore not a permission but a
    /// restatement, and the surface refuses a restatement rather than carrying it
    /// — a reader meeting one would reasonably conclude that the reach depends on
    /// the row, and deleting the row would then look like a change of policy
    /// instead of the no-op it is.
    SelfReach {
        /// The owner the row names on both sides.
        owner: String,
    },
    /// A row states no name for the region it claims.
    NamelessRow {
        /// The file the row stands in.
        file: &'static str,
        /// The relation the row stands in.
        relation: &'static str,
        /// The row as it stood, by the declarations it did carry.
        row: String,
    },
    /// A row's name is outside the declared-name grammar.
    MalformedRowName {
        /// The file the row stands in.
        file: &'static str,
        /// The relation the row stands in.
        relation: &'static str,
        /// The name as it stood.
        name: String,
    },
    /// A set entry's text is outside the grammar its half holds a text to.
    MalformedText {
        /// The half the set belongs to.
        half: &'static str,
        /// The name the entry stands under.
        name: String,
        /// The text as it stood.
        text: String,
    },
    /// An owner holds the SPDX pair and no section, or a section and no pair.
    ///
    /// Pair-to-section equality is exact in both directions, exactly as
    /// pair-to-table equality already is. A section for an owner holding no pair
    /// declares a requirement nothing will ever read; a pair whose owner has no
    /// section activates a verdict with no parameters to form it from.
    UnpairedSection {
        /// The owner the mismatch stands at.
        owner: String,
        /// What stands, of the two.
        found: &'static str,
    },
    /// An owner holds the envelope pair and no section, or a section and no pair.
    ///
    /// The fifth data-shape relation, and exact in both directions like the
    /// fourth. A section for an owner holding no pair declares a governed set
    /// nothing will ever read; a pair whose owner has no section activates a
    /// verdict with no share to form it over.
    UnpairedEnvelopeSection {
        /// The owner the mismatch stands at.
        owner: String,
        /// What stands, of the two.
        found: &'static str,
    },
    /// An owner holds the file-path pair and no section, or a section and no pair.
    ///
    /// The seventh file's sections stand under the same exact relation the
    /// interchange file's do, and the owner's shape ruling is what puts them
    /// there: a policy document carries a per-owner section, so a section for an
    /// owner holding no pair declares exceptions nothing reads, and a pair whose
    /// owner has no section activates a verdict with no section to form it over.
    UnpairedReferenceSection {
        /// The owner the mismatch stands at.
        owner: String,
        /// What stands, of the two.
        found: &'static str,
    },
    /// A row names a policy this binary does not carry.
    UnknownPolicy {
        /// The file the row stands in.
        file: &'static str,
        /// The policy the row named.
        policy: String,
    },
    /// An activated policy lacks a prerequisite whose owner the declaration fixes.
    ///
    /// Cited-owner prerequisites are not in this class because the declaration
    /// does not name which owners the corpus actually cites. Same-owner and
    /// fixed-owner prerequisites are complete declaration facts, so letting the
    /// command run without either would run a policy from a question its owner
    /// did not fully declare.
    MissingPolicyDependency {
        /// The activated pair that requires another.
        requiring: Pair,
        /// How the prerequisite owner was obtained.
        scope: Scope,
        /// The pair the declaration omitted.
        required: Pair,
    },
    /// A policy identifier is outside the identifier grammar.
    MalformedPolicy {
        /// The identifier as it stood.
        file: &'static str,
        /// The identifier as it stood.
        policy: String,
    },
    /// A pattern is one the engine will not compile.
    MalformedPattern {
        /// The file the pattern stands in.
        file: &'static str,
        /// The pattern as it stood.
        pattern: String,
        /// What the engine said when it declined it.
        message: String,
    },
    /// An instance document said something its grammar does not admit.
    Declaration {
        /// The document the defect stands in, as the directory names it.
        document: String,
        /// The decoder's own account of the defect.
        message: String,
    },
    /// Two instance documents stamp one namespace, so neither is that identity.
    DuplicateNamespace {
        /// The namespace both claimed.
        namespace: String,
    },
    /// An owner answered a selection question with the other selection shape.
    MalformedSelection {
        /// The document the section stands in.
        file: &'static str,
        /// The owner whose section it is.
        owner: String,
        /// The set type the section stands over.
        set: String,
    },
    /// A declared path display does not decode canonically.
    MalformedPath {
        /// The display as it stood.
        path: String,
        /// Why it does not decode.
        defect: PathDefect,
    },
    /// A fingerprint is not this producer's canonical form.
    MalformedFingerprint {
        /// The fingerprint as it stood.
        fingerprint: String,
    },
    /// An environment name is empty or carries edge whitespace.
    MalformedEnvironment {
        /// The name as it stood.
        environment: String,
    },
    /// A kind token is outside the label grammar's kind production.
    MalformedKind {
        /// The token as it stood.
        kind: String,
    },
    /// A ceiling is not a positive integer, so the row tolerates nothing and says so at length.
    NonPositiveMaximum {
        /// The row the ceiling stands in.
        row: String,
    },
    /// A row stands twice, which adds nothing and hides which of the two a reader is looking at.
    DuplicateRow {
        /// The file the row stands in.
        file: &'static str,
        /// The row as it stood.
        row: String,
    },
    /// An activated pair carries no list.
    UnpairedPolicy {
        /// The pair with no list.
        pair: Pair,
    },
    /// A list stands at a pair nothing activates.
    OrphanList {
        /// The pair the list stands at.
        pair: Pair,
    },
    /// A list is written in a codec its policy does not select.
    WrongCodec {
        /// The pair the list stands at.
        pair: Pair,
        /// The codec the policy selects.
        expected: Codec,
        /// The field the table actually carried.
        found: String,
    },
    /// A path-count row names a file the inclusion relation does not attribute to the table's owner.
    OwnerPathMismatch {
        /// The pair the row stands at.
        pair: Pair,
        /// The path the row named.
        path: String,
    },
}

impl fmt::Display for Refusal {
    // One exhaustive arm per variant of the refusal taxonomy, each a single
    // write. Splitting the match would scatter the renderings across several
    // functions and make an unrendered variant easy to miss.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per variant of an exhaustive refusal taxonomy"
    )]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile { file } => write!(
                formatter,
                "{DIRECTORY}/{file}: the snapshot requires this file"
            ),
            Self::UnknownMember { name } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{name}: the declared surface is the required core, the shape document and enveloped policy documents, and this is none of them"
                )
            }
            Self::Unreadable { file, error } => write!(formatter, "{DIRECTORY}/{file}: {error}"),
            Self::Malformed { file, message } => write!(formatter, "{DIRECTORY}/{file}: {message}"),
            Self::Retired {
                file,
                key,
                replacement,
            } => {
                write!(formatter, "{DIRECTORY}/{file}: {key}: {replacement}")
            }
            Self::Envelope { text } => write!(formatter, "{text}"),
            Self::UnknownOwner { file, owner } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{file}: {owner}: no such registered owner"
                )
            }
            Self::MalformedOwner { owner } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{OWNERS_FILE}: {owner}: not a well-formed owner prefix"
                )
            }
            Self::SelfReach { owner } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{OWNERS_FILE}: may_cite: {owner} : {owner}: an owner reaches itself by being itself, and the row is not written"
                )
            }
            Self::NamelessRow {
                file,
                relation,
                row,
            } => write!(
                formatter,
                "{DIRECTORY}/{file}: {relation}: {row}: a row names the region it claims, and name is the word for it"
            ),
            Self::MalformedRowName {
                file,
                relation,
                name,
            } => write!(
                formatter,
                "{DIRECTORY}/{file}: {relation}: {name}: not a well-formed declared name"
            ),
            Self::MalformedText { half, name, text } => {
                write!(
                    formatter,
                    "{SPDX_NAMESPACE}: set.{half}: {name}: {text}: not a well-formed {half} text"
                )
            }
            Self::UnpairedSection { owner, found } => {
                write!(
                    formatter,
                    "{SPDX_NAMESPACE}: {owner}: {found} stands without the other"
                )
            }
            Self::UnpairedEnvelopeSection { owner, found } => {
                write!(
                    formatter,
                    "{INTERCHANGE_NAMESPACE}: {owner}: {found} stands without the other"
                )
            }
            Self::UnpairedReferenceSection { owner, found } => {
                write!(
                    formatter,
                    "{PATH_LINKING_NAMESPACE}: {owner}: {found} stands without the other"
                )
            }
            Self::Declaration { document, message } => {
                write!(formatter, "{DIRECTORY}/{document}: {message}")
            }
            Self::DuplicateNamespace { namespace } => {
                write!(
                    formatter,
                    "{DIRECTORY}: {namespace}: two documents stamp this namespace"
                )
            }
            Self::MalformedSelection { file, owner, set } => {
                write!(
                    formatter,
                    "{file}: {owner}: {set}: a section of rows is wanted here and a singular reference stands"
                )
            }
            Self::UnknownPolicy { file, policy } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{file}: {policy}: this binary catalogues no such policy"
                )
            }
            Self::MissingPolicyDependency {
                requiring,
                scope,
                required,
            } => write!(
                formatter,
                "policy dependency: {requiring}: missing {} pair {required}",
                scope.as_str()
            ),
            Self::MalformedPolicy { file, policy } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{file}: {policy}: not a well-formed policy identifier"
                )
            }
            Self::MalformedPattern {
                file,
                pattern,
                message,
            } => {
                write!(formatter, "{DIRECTORY}/{file}: {pattern}: {message}")
            }
            Self::MalformedPath { path, defect } => {
                write!(formatter, "{DIRECTORY}/{LISTS_FILE}: {path}: {defect}")
            }
            Self::MalformedFingerprint { fingerprint } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{LISTS_FILE}: {fingerprint}: not a canonical fingerprint"
                )
            }
            Self::MalformedEnvironment { environment } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{ENVIRONMENTS_FILE}: {environment}: not a well-formed environment name"
                )
            }
            Self::MalformedKind { kind } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{ENVIRONMENTS_FILE}: {kind}: not a well-formed kind token"
                )
            }
            Self::NonPositiveMaximum { row } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{LISTS_FILE}: {row}: a ceiling is a positive integer"
                )
            }
            Self::DuplicateRow { file, row } => {
                write!(formatter, "{DIRECTORY}/{file}: {row}: the row stands twice")
            }
            Self::UnpairedPolicy { pair } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{LISTS_FILE}: {pair}: the activated pair carries no list"
                )
            }
            Self::OrphanList { pair } => {
                write!(
                    formatter,
                    "{DIRECTORY}/{LISTS_FILE}: {pair}: a list stands at a pair nothing activates"
                )
            }
            Self::WrongCodec {
                pair,
                expected,
                found,
            } => write!(
                formatter,
                "{DIRECTORY}/{LISTS_FILE}: {pair}: the policy selects `{}` and the table carries `{found}`",
                expected.field()
            ),
            Self::OwnerPathMismatch { pair, path } => write!(
                formatter,
                "{DIRECTORY}/{LISTS_FILE}: {pair}: {path}: the inclusion relation attributes this path elsewhere"
            ),
        }
    }
}

/// The whole declared surface, parsed and cross-validated as one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    surface: Arc<SurfaceAst>,
    owners: Vec<String>,
    partitions: Vec<OwnerRow>,
    shape: CorpusShape,
    may_cite: Vec<ReachRow>,
    environments: Vec<EnvironmentRow>,
    environment_extensions: Vec<EnvironmentRow>,
    reserved_kinds: BTreeSet<String>,
    reserved_extensions: BTreeSet<String>,
    policies: Vec<Pair>,
    lists: BTreeMap<Pair, Rows>,
    deployments: BTreeMap<(String, String), String>,
    documents: BTreeMap<String, Declaration>,
    spdx: Parameters,
    interchange: InterchangeParameters,
    references: ReferenceParameters,
    surfaces: BTreeMap<String, BurnSurface>,
}

/// The declaration core shared by strict list compilation and permissive audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationCore {
    surface: Arc<SurfaceAst>,
    owners: Vec<String>,
    partitions: Vec<OwnerRow>,
    shape: CorpusShape,
    may_cite: Vec<ReachRow>,
    environments: Vec<EnvironmentRow>,
    environment_extensions: Vec<EnvironmentRow>,
    reserved_kinds: BTreeSet<String>,
    reserved_extensions: BTreeSet<String>,
    policies: Vec<Pair>,
    deployments: BTreeMap<(String, String), String>,
    documents: BTreeMap<String, Declaration>,
    spdx: Parameters,
    interchange: InterchangeParameters,
    references: ReferenceParameters,
    surfaces: BTreeMap<String, BurnSurface>,
}

impl DeclarationCore {
    /// The one syntax tree both validators consume.
    #[must_use]
    pub(crate) const fn surface(&self) -> &Arc<SurfaceAst> {
        &self.surface
    }

    /// The activated owner-and-policy pairs.
    #[must_use]
    pub(crate) fn policies(&self) -> &[Pair] {
        &self.policies
    }

    /// The partition relation used to attribute list rows.
    #[must_use]
    pub(crate) fn partitions(&self) -> &[OwnerRow] {
        &self.partitions
    }

    /// The program a ratchet key names, where this binary catalogues one.
    #[must_use]
    pub(crate) fn program(&self, pair: &Pair) -> Option<&str> {
        pair.family.as_ref().map_or_else(
            || catalogued(&pair.policy).map(|policy| policy.name),
            |entry| {
                self.deployments
                    .get(&(pair.policy.clone(), entry.clone()))
                    .map(String::as_str)
            },
        )
    }

    /// Complete the core with one validator's list projection.
    pub(crate) fn with_lists(self, lists: BTreeMap<Pair, Rows>) -> Snapshot {
        Snapshot {
            surface: self.surface,
            owners: self.owners,
            partitions: self.partitions,
            shape: self.shape,
            may_cite: self.may_cite,
            environments: self.environments,
            environment_extensions: self.environment_extensions,
            reserved_kinds: self.reserved_kinds,
            reserved_extensions: self.reserved_extensions,
            policies: self.policies,
            lists,
            deployments: self.deployments,
            documents: self.documents,
            spdx: self.spdx,
            interchange: self.interchange,
            references: self.references,
            surfaces: self.surfaces,
        }
    }
}

/// What the permissive consumer found where a declaration core would stand.
pub enum CoreConfiguration {
    Absent,
    Refused(Vec<Refusal>),
    Present(Box<DeclarationCore>),
}

impl Snapshot {
    /// The one syntax tree this compiled snapshot projects from.
    #[must_use]
    pub(crate) const fn surface(&self) -> &Arc<SurfaceAst> {
        &self.surface
    }

    /// The registered owners.
    #[must_use]
    pub fn owners(&self) -> &[String] {
        &self.owners
    }

    /// The interchange policy's parameters, empty while the sixth file is unwritten.
    #[must_use]
    pub const fn interchange(&self) -> &InterchangeParameters {
        &self.interchange
    }

    /// The file-path policy's parameters: one section per owner holding the pair.
    #[must_use]
    pub const fn references(&self) -> &ReferenceParameters {
        &self.references
    }

    /// The burn surface one policy's domain declares, where the corpus declared one.
    ///
    /// A domain that declared none is answered `None` rather than an empty
    /// surface, because a census walking no tree and a census nobody told where to
    /// walk are different states and only one of them is a corpus's decision.
    #[must_use]
    pub fn declared_surface(&self, domain: &str) -> Option<&BurnSurface> {
        self.surfaces.get(domain)
    }

    /// Where one owner's share begins, read from the relation that divides the universe.
    ///
    /// An owner the partition places nowhere has no root, which is not the corpus
    /// root: a rule cutting an unplaced share out of a walk would cut out the
    /// whole repository (´just:isolation:policy-data´).
    #[must_use]
    pub fn share_root(&self, owner: &str) -> Option<PathBuf> {
        let patterns: Vec<&AbnfPattern> = self
            .partitions
            .iter()
            .filter(|row| row.owner == owner)
            .map(|row| &row.pattern)
            .collect();

        (!patterns.is_empty()).then(|| partition_root(patterns))
    }

    /// The partition relation, which must divide the post-ignore set exactly once.
    #[must_use]
    pub fn partitions(&self) -> &[OwnerRow] {
        &self.partitions
    }

    /// The declared corpus universe answer and global-ignore relation.
    ///
    /// It stands here rather than beside the loader because everything that
    /// ranges over the corpus ranges over the post-ignore universe, and a caller
    /// holding the snapshot holds the one relation that decided which paths those
    /// are (´just:isolation:policy-data´).
    #[must_use]
    pub const fn shape(&self) -> &CorpusShape {
        &self.shape
    }

    /// The declared may-cite relation.
    #[must_use]
    pub fn may_cite(&self) -> &[ReachRow] {
        &self.may_cite
    }

    /// The declared environment relation.
    #[must_use]
    pub fn environments(&self) -> &[EnvironmentRow] {
        &self.environments
    }

    /// The repository-local rows extending the adopted environment relation.
    #[must_use]
    pub fn environment_extensions(&self) -> &[EnvironmentRow] {
        &self.environment_extensions
    }

    /// The kinds the adopted registry reserves for derivation.
    #[must_use]
    pub const fn reserved_kinds(&self) -> &BTreeSet<String> {
        &self.reserved_kinds
    }

    /// The kinds repository-local profiles additionally reserve for derivation.
    #[must_use]
    pub const fn reserved_extensions(&self) -> &BTreeSet<String> {
        &self.reserved_extensions
    }

    /// Build the effective kind registry from this snapshot's declarations.
    #[must_use]
    pub fn kind_registry(&self) -> KindRegistry {
        KindRegistry::from_declared(
            self.environments
                .iter()
                .map(|row| (row.environment.as_str(), row.kind.as_str())),
            self.reserved_kinds.iter().map(String::as_str),
        )
        .with_declared(
            self.environment_extensions
                .iter()
                .map(|row| (row.environment.as_str(), row.kind.as_str())),
        )
        .with_reserved(self.reserved_extensions.iter().map(String::as_str))
    }

    /// The activated owner-and-policy pairs.
    #[must_use]
    pub fn policies(&self) -> &[Pair] {
        &self.policies
    }

    /// The program a ratchet key names, where this binary catalogues one.
    ///
    /// A key naming no set entry names a program outright. A key naming one names
    /// an instance document and an entry of one of its sets, and the program is
    /// the one this binary catalogues for that set type — so a corpus may deploy a
    /// program several times without the binary being taught which deployments it
    /// has (´gram:isolation:declaration´).
    #[must_use]
    pub fn program(&self, pair: &Pair) -> Option<&str> {
        pair.family.as_ref().map_or_else(
            || catalogued(&pair.policy).map(|policy| policy.name),
            |entry| {
                self.deployments
                    .get(&(pair.policy.clone(), entry.clone()))
                    .map(String::as_str)
            },
        )
    }

    /// Every ratchet key the surface carries, in the order its activations stand.
    ///
    /// The activation document's order is the file's order, because a writer that
    /// sorted the keys would rewrite a hand-authored document on its first run and
    /// tell its reader nothing about the corpus. Within one activation the keys of
    /// its deployments follow their own order, which is the one thing about them
    /// the activation does not say.
    #[must_use]
    pub fn list_keys(&self) -> Vec<Pair> {
        self.ordered(&self.lists)
    }

    /// Order a set of ratchet keys as the activation document stands.
    ///
    /// A key no activation claims comes last rather than being dropped, because a
    /// writer that silently omitted a key it could not place would delete a debt
    /// the corpus had recorded. Where the keys are the snapshot's own there is no
    /// such key, and the permissive reading — which carries the activations and
    /// not the lists — is what the tail is for.
    #[must_use]
    pub fn ordered(&self, lists: &BTreeMap<Pair, Rows>) -> Vec<Pair> {
        let mut keys = Vec::with_capacity(lists.len());

        for activation in &self.policies {
            for pair in lists.keys() {
                if pair.owner == activation.owner
                    && self.program(pair) == Some(activation.policy.as_str())
                {
                    keys.push(pair.clone());
                }
            }
        }

        for pair in lists.keys() {
            if !keys.contains(pair) {
                keys.push(pair.clone());
            }
        }

        keys
    }

    /// The list carried at each activated pair.
    #[must_use]
    pub const fn lists(&self) -> &BTreeMap<Pair, Rows> {
        &self.lists
    }

    /// The SPDX policy's parameters: the two sets and the per-owner sections.
    ///
    /// A repository that activates no SPDX pair carries two empty sets and no
    /// section.
    #[must_use]
    pub const fn spdx(&self) -> &Parameters {
        &self.spdx
    }

    /// The instance document one allocated namespace stands for.
    ///
    /// The namespace is the identity a document is found by, so a reader wanting
    /// one asks for the identity rather than for the filename its author chose.
    #[must_use]
    pub fn document(&self, namespace: &str) -> Option<&Declaration> {
        self.documents.get(namespace)
    }

    /// The marked-ordinal instances the scenario document declares, by entry name.
    ///
    /// The mark and the two ends of the bound are the corpus's and the reading they
    /// parameterize is the program's, so what comes back is the program value the
    /// census consumes rather than the three scalars it was written from.
    #[must_use]
    pub fn numbered_marks(&self) -> Vec<(String, MarkNumbered)> {
        let Some(set) = self
            .documents
            .get(SCENARIOS_NAMESPACE)
            .and_then(|document| document.set(NUMBERED_MARKS_SET))
        else {
            return Vec::new();
        };

        set.entries()
            .iter()
            .filter_map(|(name, entry)| match entry {
                SetEntry::NumberedMark(mark) => Some((name.clone(), mark.clone())),
                _ => None,
            })
            .collect()
    }

    /// The literal-set instance the division document declares.
    ///
    /// The whole set is one instance rather than one instance per sentence,
    /// because the twelve sentences are one identity scheme and a census that read
    /// them separately would order its occurrences by which sentence was asked for
    /// rather than by where they stand.
    #[must_use]
    pub fn declared_literals(&self) -> Option<LiteralSet> {
        let set = self
            .documents
            .get(DIVISIONS_NAMESPACE)
            .and_then(|document| document.set(LITERALS_SET))?;

        let values: Vec<String> = set
            .entries()
            .values()
            .filter_map(|entry| match entry {
                SetEntry::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect();

        LiteralSet::new(values).ok()
    }

    /// The prefix-number instances the prefix-number document declares, by entry name.
    ///
    /// Shielding is joined here rather than read from the document, because whether
    /// an occurrence the section grammar has already claimed belongs to that
    /// reference is a precedence configuration may neither widen nor disable.
    #[must_use]
    pub fn declared_prefix_numbers(&self) -> Vec<(String, PrefixNumbers)> {
        let Some(set) = self
            .documents
            .get(PREFIX_NUMBERS_NAMESPACE)
            .and_then(|document| document.set(PREFIX_NUMBERS_SET))
        else {
            return Vec::new();
        };

        set.entries()
            .iter()
            .filter_map(|(name, entry)| match entry {
                SetEntry::PrefixNumber(declared) => Some((
                    name.clone(),
                    PrefixNumbers::new(declared.prefix(), declared.bound().clone(), true),
                )),
                _ => None,
            })
            .collect()
    }

    /// The publication rows the assembly document declares, attributed by their tables.
    ///
    /// The owner rides on the row because the table the row stands on is what
    /// attributes it, so nothing downstream has to recover an owner from a path.
    #[must_use]
    pub fn declared_publications(&self) -> Vec<Publication> {
        let Some(document) = self.documents.get(ASSEMBLY_PUBLICATIONS_NAMESPACE) else {
            return Vec::new();
        };

        document
            .owners()
            .values()
            .flat_map(|declared| {
                declared.data().values().filter_map(|datum| match datum {
                    OwnerDatum::Publication(publication) => Some(publication.clone()),
                    _ => None,
                })
            })
            .collect()
    }

    /// The owner whose share is the repository, where the partition names one.
    ///
    /// A fixed-owner prerequisite is wanted of the owner a repository-wide
    /// artifact belongs to, and which owner that is, is a fact about the
    /// partition rather than about this binary: the graph reconciliation is one
    /// artifact for the whole tree, so the pair carrying it is the root owner's
    /// wherever it is required from (ADR-T-019, The layer owner graph).
    ///
    /// The root owner is the one whose partition rows share no opening, so its share
    /// begins at the corpus root and every other share stands somewhere inside
    /// the tree it heads — which is the answer the partition already gives for
    /// the owner holding the repository's own crate
    /// (´claim:areas:a-root-is-read-off-the-partition´).
    /// An owner including nothing is passed over rather than rooted at the
    /// corpus root by vacuity, because a share admitting no path heads no tree.
    ///
    /// A partition rooting two owners at the corpus root names no root owner at
    /// all, and neither does one rooting none there. Both are answers rather than
    /// defaults: a repository-wide verdict wanted of an owner nobody can identify
    /// is a verdict nobody can repair, and choosing between two would attribute it
    /// by luck.
    #[must_use]
    pub fn root_owner(&self) -> Option<&str> {
        let mut found: Option<&str> = None;

        for owner in &self.owners {
            let mut rows = self
                .partitions
                .iter()
                .filter(|row| row.owner == *owner)
                .peekable();

            if rows.peek().is_none() {
                continue;
            }

            if partition_root(rows.map(|row| &row.pattern)) != Path::new(CORPUS_ROOT) {
                continue;
            }

            if found.is_some() {
                return None;
            }

            found = Some(owner.as_str());
        }

        found
    }

    /// The owner-name reconciliation the owner-name document declares.
    ///
    /// The stripped namespace is the sole entry of the prefix-ignore set, and the
    /// unbuilt members are the owners whose naked table names a crate and a
    /// directory without naming a manifest — which is the whole of what it is to be
    /// registered and not yet built.
    #[must_use]
    pub fn declared_owner_names(&self) -> Option<OwnerNames> {
        let document = self.documents.get(OWNER_NAMES_NAMESPACE)?;

        let mut prefixes = document
            .set(NAME_PREFIX_IGNORE_SET)?
            .entries()
            .values()
            .filter_map(|entry| match entry {
                SetEntry::Text(text) => Some(text.clone()),
                _ => None,
            });

        let namespace = prefixes.next()?;

        if prefixes.next().is_some() {
            return None;
        }

        let unbuilt = document.owners().values().filter_map(|declared| {
            let (Some(OwnerDatum::Text(name)), Some(OwnerDatum::Text(directory))) = (
                declared.datum(CRATE_NAME_DATUM),
                declared.datum(PACKAGE_DIRECTORY_DATUM),
            ) else {
                return None;
            };

            Some(UnbuiltMember::new(name.clone(), directory.clone()))
        });

        Some(OwnerNames::new(namespace, unbuilt))
    }
}

/// What a command found where its configuration would stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Configuration {
    /// A declaration stands and is not a snapshot, so the command refuses to start.
    Refused(Vec<Refusal>),
    /// A declaration stands and parses, so the command runs and judges it against the tree.
    Present(Box<Snapshot>),
}

impl Configuration {
    /// The refusals, when the configuration was refused.
    #[must_use]
    pub fn refusals(&self) -> &[Refusal] {
        match self {
            Self::Refused(refusals) => refusals,
            Self::Present(_) => &[],
        }
    }

    /// The snapshot, when one stands and parsed.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        match self {
            Self::Present(snapshot) => Some(snapshot),
            Self::Refused(_) => None,
        }
    }
}

/// Read the declared configuration standing at a repository root.
///
/// The five files are read as one. Every refusal the snapshot carries is
/// collected rather than only the first, because a caller repairing a
/// declaration wants the whole list and a second run to find out there is more
/// is a poor way to learn it.
#[must_use]
pub fn configuration(root: &Path) -> Configuration {
    let directory = root.join(DIRECTORY);

    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Configuration::Refused(vec![Refusal::MissingFile { file: SHAPE_FILE }]);
        }
        Err(error) => {
            return Configuration::Refused(vec![Refusal::Unreadable {
                file: "",
                error: error.to_string(),
            }]);
        }
    };

    let mut refusals = Vec::new();
    let mut present = BTreeSet::new();

    for entry in entries.flatten() {
        let name = entry.file_name();

        match name.to_str() {
            Some(name) if is_declared_member(name) => {
                present.insert(name.to_owned());
            }
            _ => refusals.push(Refusal::UnknownMember {
                name: display_name(&name),
            }),
        }
    }

    for file in FILES {
        if !present.contains(file) {
            refusals.push(Refusal::MissingFile { file });
        }
    }

    if !refusals.is_empty() {
        return Configuration::Refused(refusals);
    }

    match load(&directory, true) {
        Ok((core, lists)) => {
            let snapshot = core.with_lists(lists);
            let refusals = policy_dependency_refusals(&snapshot);

            if refusals.is_empty() {
                Configuration::Present(Box::new(snapshot))
            } else {
                Configuration::Refused(refusals)
            }
        }
        Err(refusals) => Configuration::Refused(refusals),
    }
}

/// Missing prerequisites whose owner is fixed by the declaration itself.
fn policy_dependency_refusals(snapshot: &Snapshot) -> Vec<Refusal> {
    let declared: BTreeSet<&Pair> = snapshot.policies.iter().collect();
    let mut refusals = Vec::new();

    for requiring in &snapshot.policies {
        let Some(policy) = catalogued(&requiring.policy) else {
            continue;
        };

        for dependency in policy.dependencies {
            let owner = match dependency.scope {
                Scope::SameOwner => requiring.owner.as_str(),
                Scope::FixedOwner => {
                    let Some(owner) = snapshot.root_owner() else {
                        continue;
                    };

                    owner
                }
                Scope::CitedOwner => continue,
            };
            let required = Pair::singleton(owner, dependency.policy);

            if !declared.contains(&required) {
                refusals.push(Refusal::MissingPolicyDependency {
                    requiring: requiring.clone(),
                    scope: dependency.scope,
                    required,
                });
            }
        }
    }

    refusals
}

/// Read the declaration core the permissive list validator consumes.
///
/// The audit mode must survive a configuration it would refuse to run policy
/// against, because finding out what the lists say is exactly what a caller runs
/// it for. So the three declarations the lists are checked against are loaded
/// under the ordinary rules — a defect in any of them still refuses, since there
/// would be nothing to check the lists against — while the list file's own
/// semantic defects are left for the audit to report as anomalies rather than
/// refused here. This is the declaration half of a snapshot, not a listless
/// snapshot manufactured to fit the strict configuration type.
#[must_use]
pub fn core_configuration(root: &Path) -> CoreConfiguration {
    let directory = root.join(DIRECTORY);

    if !directory.is_dir() {
        return CoreConfiguration::Absent;
    }

    match load(&directory, false) {
        Ok((core, _lists)) => CoreConfiguration::Present(Box::new(core)),
        Err(refusals) => CoreConfiguration::Refused(refusals),
    }
}

/// Every recognized document standing in a declaration directory, ordered by name.
///
/// The required core is named whether or not it stands, so a missing member still
/// reaches the reader that reports it missing rather than disappearing from the
/// list of things to look at.
fn declared_members(directory: &Path) -> BTreeSet<String> {
    let mut members: BTreeSet<String> = FILES.iter().map(|file| (*file).to_owned()).collect();

    let Ok(entries) = std::fs::read_dir(directory) else {
        return members;
    };

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str()
            && is_declared_member(name)
        {
            members.insert(name.to_owned());
        }
    }

    members
}

/// Render a directory entry's name in the reversible display.
fn display_name(name: &std::ffi::OsStr) -> String {
    BytePath::from_bytes(os_bytes(name)).map_or_else(|_| String::from("?"), |path| path.display())
}

/// A directory entry name's bytes, without a lossy conversion.
#[cfg(unix)]
fn os_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    name.as_bytes().to_vec()
}

/// A directory entry name's bytes, without a lossy conversion.
#[cfg(not(unix))]
fn os_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    name.to_str().unwrap_or_default().as_bytes().to_vec()
}

/// The owner file's shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOwners {
    /// The allocated schema label carried by the owner file's envelope.
    #[serde(default, rename = "namespace")]
    _namespace: String,
    /// The owner schema's version triple.
    #[serde(default, rename = "version")]
    _version: [u64; 3],
    owners: Vec<String>,
    /// The relation dividing the universe among owners, optional in the shape
    /// only so that its absence can be said in the surface's own words.
    #[serde(default)]
    partitions: Option<Vec<RawOwnerRow>>,
    /// The retired spelling of that relation, admitted here to refuse it by name.
    ///
    /// A document written before the ruling was not guessing, so what its author
    /// is owed is the sentence saying which word carries the declaration now.
    /// Leaving the key to the unknown-field refusal would name it and stop there.
    #[serde(default)]
    inclusions: Option<Vec<RawOwnerRow>>,
    may_cite: Vec<RawReachRow>,
}

/// The rows of the partition relation, under the one word that declares it.
///
/// The relation divides the universe among owners exactly once, and the word says
/// so. The retired spelling refuses with its successor rather than being read as a
/// synonym, because a surface carrying both words for one relation is a surface
/// where the meaning depends on which the reader learnt first — and writing both
/// is the retired one standing, so it refuses there too. Writing neither refuses
/// as well: a partition nobody wrote is not an empty partition, it is a universe
/// nobody divided.
fn declared_partition<'a>(raw: &'a RawOwners, refusals: &mut Vec<Refusal>) -> &'a [RawOwnerRow] {
    match (&raw.partitions, &raw.inclusions) {
        (_, Some(_)) => refusals.push(Refusal::Retired {
            file: OWNERS_FILE,
            key: "inclusions",
            replacement: "the relation divides the universe among owners exactly once, and partitions is the word for it",
        }),
        (Some(rows), None) => return rows,
        (None, None) => refusals.push(Refusal::Malformed {
            file: OWNERS_FILE,
            message: String::from("partitions: the owner file declares the relation that divides the universe"),
        }),
    }

    &[]
}

/// One raw partition row.
///
/// The name is optional in the shape only so that its absence can be refused in
/// the surface's own words. A row written before the ruling was not guessing,
/// and what its author is owed is the sentence saying that a partition row names
/// the region it claims — not the parser's account of a field it wanted.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOwnerRow {
    #[serde(default)]
    name: Option<String>,
    owner: String,
    pattern: String,
}

/// One raw may-cite row.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReachRow {
    owner: String,
    target: String,
}

/// The environment file's shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvironments {
    /// The allocated schema label carried by the environment file's envelope.
    #[serde(default, rename = "namespace")]
    _namespace: String,
    /// The environment schema's version triple.
    #[serde(default, rename = "version")]
    _version: [u64; 3],
    #[serde(default)]
    reserved_kinds: Vec<String>,
    #[serde(default)]
    reserved_extensions: Vec<String>,
    environments: Vec<RawEnvironmentRow>,
    #[serde(default)]
    extensions: Vec<RawEnvironmentRow>,
}

/// One raw environment row.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvironmentRow {
    environment: String,
    kind: String,
}

/// The activation file's shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicies {
    /// The allocated schema label carried by the activation file's envelope.
    #[serde(default, rename = "namespace")]
    _namespace: String,
    /// The activation schema's version triple.
    #[serde(default, rename = "version")]
    _version: [u64; 3],
    policies: Vec<RawPair>,
}

/// One raw activation row: an owner and the program it activates.
///
/// No third field. Under the ratified grammar the activation document says that a
/// program acts for an owner, and the instance document then says what that means
/// for that owner, so the family key the retiring dialect carried has nothing left
/// to discriminate (´gram:isolation:declaration´).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPair {
    owner: String,
    policy: String,
}

/// Project one core schema from the surface's single parsed document.
fn read_ast_file<T: serde::de::DeserializeOwned>(
    surface: &SurfaceAst,
    file: &'static str,
    refusals: &mut Vec<Refusal>,
) -> Option<T> {
    let Some(document) = surface.document(file) else {
        refusals.push(Refusal::Unreadable {
            file,
            error: String::from("No such file or directory"),
        });

        return None;
    };

    match document.deserialize() {
        Ok(value) => Some(value),
        Err(DocumentDefect::Unreadable(error)) => {
            refusals.push(Refusal::Unreadable { file, error });

            None
        }
        Err(DocumentDefect::Malformed(message)) => {
            refusals.push(Refusal::Malformed { file, message });

            None
        }
    }
}

/// Project the corpus shape from its already-parsed surface document.
fn read_shape(surface: &SurfaceAst, refusals: &mut Vec<Refusal>) -> Option<CorpusShape> {
    let document = surface.document(SHAPE_FILE)?;
    let text = match document.strict_text() {
        Ok(text) => text,
        Err(DocumentDefect::Unreadable(error)) => {
            refusals.push(Refusal::Unreadable {
                file: SHAPE_FILE,
                error,
            });

            return None;
        }
        Err(DocumentDefect::Malformed(_)) => return None,
    };

    let decoded = document.table().map_or_else(
        |message| {
            Err(vec![crate::universe::ShapeDefect::Malformed(
                message.to_owned(),
            )])
        },
        |table| decode_shape_table(table, text),
    );

    match decoded {
        Ok(shape) => Some(shape),
        Err(defects) => {
            for defect in defects {
                refusals.push(Refusal::Declaration {
                    document: SHAPE_FILE.to_owned(),
                    message: defect.to_string(),
                });
            }

            None
        }
    }
}

/// Parse and cross-validate the files standing in a declaration directory.
///
/// The list file is validated against the other three only under the strict
/// reading; the permissive one leaves its defects to the audit that reports
/// them.
///
/// The envelope of every declared file is read before any of them is
/// interpreted, and a defect refuses the command entire. The requirement is
/// unconditional and constitutional: it holds of the declared files because they
/// are the configuration, not because the configuration says so, so it is
/// checked before any activation is known and whether or not a single owner
/// holds the pair the policy is declared at.
fn load(
    directory: &Path,
    strict: bool,
) -> Result<(DeclarationCore, BTreeMap<Pair, Rows>), Vec<Refusal>> {
    // A file that is not a well-formed document is left to the parser's own
    // refusal, where it always stood. The envelope requirement joins the class a
    // lexical error already occupies rather than absorbing it: both refuse the
    // same way and neither reaches a verdict, and between two texts of one class
    // the parser's is the one that tells a reviewer what to repair. So the
    // envelope is asked of the declared files that are documents, which is every
    // file of which the question can be asked at all.
    // Membership is physical, so the envelope question is asked of every present
    // member rather than of a compiled filename list. A document this loader reads
    // nothing out of is still a member of the surface, and a member whose envelope
    // does not identify it has made no claim any reader could check.
    let members = declared_members(directory);
    let surface = Arc::new(SurfaceAst::read(directory, members.iter().cloned()));
    let envelopes: Vec<String> = surface
        .documents()
        .filter_map(|(file, document)| {
            document.table().ok()?;

            declared_refusal_from_parsed(file, document.text().ok()?)
        })
        .collect();

    if !envelopes.is_empty() {
        return Err(envelopes
            .into_iter()
            .map(|text| Refusal::Envelope { text })
            .collect());
    }

    let mut refusals = Vec::new();

    let raw_owners: Option<RawOwners> = read_ast_file(&surface, OWNERS_FILE, &mut refusals);
    let raw_environments: Option<RawEnvironments> =
        read_ast_file(&surface, ENVIRONMENTS_FILE, &mut refusals);
    let raw_policies: Option<RawPolicies> = read_ast_file(&surface, POLICIES_FILE, &mut refusals);

    let raw_lists: Option<crate::surface::ListDocument> = if strict {
        read_ast_file(&surface, LISTS_FILE, &mut refusals)
    } else {
        None
    };

    // The shape document is read through the one decoder that owns its schema,
    // and the loader reading it is grounded below both declarations it supplies:
    // the directory is read physically, never consulting git and never applying
    // the ignore relation, including here (´just:isolation:policy-data´).
    let shape = read_shape(&surface, &mut refusals);

    let (Some(raw_owners), Some(raw_environments), Some(raw_policies), Some(shape)) =
        (raw_owners, raw_environments, raw_policies, shape)
    else {
        return Err(refusals);
    };

    if strict && raw_lists.is_none() {
        return Err(refusals);
    }

    let registered = registered_owners(&raw_owners.owners, &mut refusals);

    let declared = declared_partition(&raw_owners, &mut refusals);
    let partitions = owner_rows(declared, "partitions", &registered, &mut refusals);
    let may_cite = reach_rows(&raw_owners.may_cite, &registered, &mut refusals);
    let environments = environment_rows(&raw_environments.environments, &mut refusals);
    let environment_extensions = environment_rows(&raw_environments.extensions, &mut refusals);
    let reserved_kinds = reserved_kind_set(
        &raw_environments.reserved_kinds,
        "reserved_kinds",
        &mut refusals,
    );
    let reserved_extensions = reserved_kind_set(
        &raw_environments.reserved_extensions,
        "reserved_extensions",
        &mut refusals,
    );
    let policies = activation_pairs(&raw_policies.policies, &registered, &mut refusals);

    // Every instance document is decoded before any of them is read, because the
    // relations between them are asked over the union rather than through a
    // privileged file, and a document found by its namespace has to be found
    // before it can be asked for (´proposal:isolation:one-plan´).
    let documents = declared_documents_from_ast(&surface, &members, &mut refusals);

    let deployments = deployments(
        &documents,
        corpus_prefix(&list_namespace_from_ast(&surface)),
    );

    let spdx = spdx_parameters(&documents, &registered, &policies, &mut refusals);
    let interchange = interchange_parameters(&documents, &registered, &policies, &mut refusals);
    let references = references_parameters(&documents, &registered, &policies, &mut refusals);
    let surfaces = surface_parameters(&documents, &registered, &mut refusals);

    let lists = match raw_lists {
        Some(raw_lists) if strict => list_tables(
            &raw_lists.tables,
            &registered,
            &policies,
            &deployments,
            &partitions,
            &mut refusals,
        ),
        Some(_) | None => BTreeMap::new(),
    };

    if !refusals.is_empty() {
        return Err(refusals);
    }

    let core = DeclarationCore {
        surface,
        owners: raw_owners.owners,
        partitions,
        shape,
        may_cite,
        environments,
        environment_extensions,
        reserved_kinds,
        reserved_extensions,
        policies,
        deployments,
        documents,
        spdx,
        interchange,
        references,
        surfaces,
    };

    Ok((core, lists))
}

/// Decode every instance document from the surface's single parsed tree.
fn declared_documents_from_ast(
    surface: &SurfaceAst,
    members: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
) -> BTreeMap<String, Declaration> {
    let mut documents: BTreeMap<String, Declaration> = BTreeMap::new();

    for name in members {
        if !is_policy_document(name) {
            continue;
        }

        let Some(document) = surface.document(name) else {
            continue;
        };

        let _text = match document.strict_text() {
            Ok(text) => text,
            Err(DocumentDefect::Unreadable(error)) => {
                refusals.push(Refusal::Unreadable {
                    file: "",
                    error: format!("{name}: {error}"),
                });

                continue;
            }
            Err(DocumentDefect::Malformed(_)) => continue,
        };

        let decoded = match document.table() {
            Ok(table) => Declaration::decode_table(table),
            Err(message) => Err(vec![crate::declaration::DeclarationDefect::Malformed {
                message: message.to_owned(),
            }]),
        };

        match decoded {
            Ok(declaration) => {
                if documents.contains_key(declaration.namespace()) {
                    refusals.push(Refusal::DuplicateNamespace {
                        namespace: declaration.namespace().to_owned(),
                    });

                    continue;
                }

                documents.insert(declaration.namespace().to_owned(), declaration);
            }
            Err(defects) => {
                for defect in defects {
                    refusals.push(Refusal::Declaration {
                        document: name.clone(),
                        message: defect.to_string(),
                    });
                }
            }
        }
    }

    documents
}

/// The owners whose activation of one program the activation document declares.
fn activating(policies: &[Pair], program: &str) -> BTreeSet<String> {
    policies
        .iter()
        .filter(|pair| pair.policy == program && pair.family.is_none())
        .map(|pair| pair.owner.clone())
        .collect()
}

/// Hold an instance document's owner sections and its program's activations equal.
///
/// Equality in both directions, and for the reason it always had: a section for an
/// owner holding no pair declares a requirement nothing reads, and a pair whose
/// owner has no section activates a verdict with no parameters to form it over.
/// Non-application for one owner is the omission of both, which is the ratified
/// grammar's only way of saying it (´gram:isolation:declaration´).
fn paired_sections(
    declared: &BTreeSet<String>,
    activated: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
    unpaired: impl Fn(String, &'static str) -> Refusal,
) {
    for owner in activated.difference(declared) {
        refusals.push(unpaired(owner.clone(), "the activated pair"));
    }

    for owner in declared.difference(activated) {
        refusals.push(unpaired(owner.clone(), "the section"));
    }
}

/// The pattern rows one owner declares over one set type, as a pair of lists.
///
/// A selection written as the singular reference is no selection over files at
/// all, so an owner declaring one where a section is wanted has answered a
/// different question and refuses rather than being read as an empty list.
fn selected_rows<'a>(
    document: &'static str,
    owner: &str,
    set: &str,
    declaration: &'a OwnerDeclaration,
    refusals: &mut Vec<Refusal>,
) -> Option<(&'a [PatternRow], &'a [PatternRow])> {
    match declaration.selection(set) {
        Some(Selection::Rows { admitted, exclude }) => Some((admitted, exclude)),
        Some(Selection::Entry(_)) => {
            refusals.push(Refusal::MalformedSelection {
                file: document,
                owner: owner.to_owned(),
                set: set.to_owned(),
            });

            None
        }
        None => Some((&[], &[])),
    }
}

/// Read the SPDX policy's instance document: its two sets and its owner sections.
///
/// The names are already held to the declared-name grammar and the include
/// references already resolved by the grammar's own decoder, so what remains here
/// is the half only this policy can judge — that an identifier entry is a licence
/// expression and a copyright entry is a copyright text — together with the
/// pairing every instance document is held to.
fn spdx_parameters(
    documents: &BTreeMap<String, Declaration>,
    registered: &BTreeSet<String>,
    policies: &[Pair],
    refusals: &mut Vec<Refusal>,
) -> Parameters {
    let activated = activating(policies, SPDX_POLICY);

    let Some(document) = documents.get(SPDX_NAMESPACE) else {
        paired_sections(&BTreeSet::new(), &activated, refusals, |owner, found| {
            Refusal::UnpairedSection { owner, found }
        });

        return Parameters::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    };

    let identifiers = spdx_set(document, Half::Identifier, IDENTIFIER_SET, refusals);
    let copyrights = spdx_set(document, Half::Copyright, COPYRIGHT_SET, refusals);

    let mut sections = BTreeMap::new();

    for (owner, declaration) in document.owners() {
        if !registered.contains(owner) {
            refusals.push(Refusal::UnknownOwner {
                file: SPDX_NAMESPACE,
                owner: owner.clone(),
            });

            continue;
        }

        let halves = HALVES.map(|half| {
            let set = match half {
                Half::Identifier => IDENTIFIER_SET,
                Half::Copyright => COPYRIGHT_SET,
            };

            let Some((admitted, exclude)) =
                selected_rows(SPDX_NAMESPACE, owner, set, declaration, refusals)
            else {
                return HalfSection::new(Vec::new(), Vec::new());
            };

            HalfSection::new(section_rows(exclude), partition_rows(admitted))
        });

        let [identifier, copyright] = halves;

        sections.insert(owner.clone(), Section::new(identifier, copyright));
    }

    let declared: BTreeSet<String> = sections.keys().cloned().collect();

    paired_sections(&declared, &activated, refusals, |owner, found| {
        Refusal::UnpairedSection { owner, found }
    });

    Parameters::new(identifiers, copyrights, sections)
}

/// Read one half's set, holding every entry to the text shape that half requires.
fn spdx_set(
    document: &Declaration,
    half: Half,
    set: &str,
    refusals: &mut Vec<Refusal>,
) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();

    let Some(declared) = document.set(set) else {
        return entries;
    };

    for (name, entry) in declared.entries() {
        let SetEntry::Text(text) = entry else {
            refusals.push(Refusal::MalformedText {
                half: half.as_str(),
                name: name.clone(),
                text: String::from("a parameterized entry where this half holds a text"),
            });

            continue;
        };

        let well_formed = match half {
            Half::Identifier => is_licence_expression(text),
            Half::Copyright => is_copyright_text(text),
        };

        if !well_formed {
            refusals.push(Refusal::MalformedText {
                half: half.as_str(),
                name: name.clone(),
                text: text.clone(),
            });

            continue;
        }

        entries.insert(name.clone(), text.clone());
    }

    entries
}

/// Carry decoded exclusion rows into the header policy's own row type.
fn section_rows(rows: &[PatternRow]) -> Vec<SectionRow> {
    rows.iter()
        .map(|row| SectionRow::new(row.name().to_owned(), row.pattern().clone()))
        .collect()
}

/// Carry decoded partition rows into the header policy's own row type.
///
/// A partition row carries the entry of its half's set, and the decoder refuses
/// one that does not, so a row arriving here without one is dropped rather than
/// given a reference this module would have to invent.
fn partition_rows(rows: &[PatternRow]) -> Vec<SectionRow> {
    rows.iter()
        .filter_map(|row| {
            row.entry().map(|entry| {
                SectionRow::carrying(
                    row.name().to_owned(),
                    entry.to_owned(),
                    row.pattern().clone(),
                )
            })
        })
        .collect()
}

/// Read the interchange policy's instance document: its owner sections.
///
/// The document carries no set, and that is the ratified answer rather than an
/// omission: which carrier types are in domain is compiled meaning, so a declared
/// list of them would be a second authority over one relation with nothing able to
/// say which of the two had drifted (´gram:isolation:declaration´).
fn interchange_parameters(
    documents: &BTreeMap<String, Declaration>,
    registered: &BTreeSet<String>,
    policies: &[Pair],
    refusals: &mut Vec<Refusal>,
) -> InterchangeParameters {
    let activated = activating(policies, INTERCHANGE_POLICY);

    let Some(document) = documents.get(INTERCHANGE_NAMESPACE) else {
        paired_sections(&BTreeSet::new(), &activated, refusals, |owner, found| {
            Refusal::UnpairedEnvelopeSection { owner, found }
        });

        return InterchangeParameters::default();
    };

    let mut sections = BTreeMap::new();

    for (owner, declaration) in document.owners() {
        if !registered.contains(owner) {
            refusals.push(Refusal::UnknownOwner {
                file: INTERCHANGE_NAMESPACE,
                owner: owner.clone(),
            });

            continue;
        }

        let Some((include, exclude)) = selected_rows(
            INTERCHANGE_NAMESPACE,
            owner,
            INTERCHANGE_SET,
            declaration,
            refusals,
        ) else {
            continue;
        };

        sections.insert(
            owner.clone(),
            InterchangeSection {
                exclude: interchange_rows(exclude),
                include: glossed(declaration, INTERCHANGE_SET).then(|| interchange_rows(include)),
            },
        );
    }

    let declared: BTreeSet<String> = sections.keys().cloned().collect();

    paired_sections(&declared, &activated, refusals, |owner, found| {
        Refusal::UnpairedEnvelopeSection { owner, found }
    });

    InterchangeParameters { sections }
}

/// Whether an owner's section over one set declares a gloss at all.
///
/// An absent gloss and an empty one are deliberately different: an absent gloss
/// owes nothing, while an empty one is the claim that the governed set is empty
/// and is judged like any other claim. The ratified grammar carries that
/// difference in the presence of the key, which the decoder preserves by giving an
/// undeclared relation no rows rather than an empty list.
fn glossed(declaration: &OwnerDeclaration, set: &str) -> bool {
    matches!(declaration.selection(set), Some(Selection::Rows { admitted, .. }) if !admitted.is_empty())
}

/// Carry decoded pattern rows into the envelope policy's own row type.
fn interchange_rows(rows: &[PatternRow]) -> Vec<InterchangeRow> {
    rows.iter()
        .map(|row| InterchangeRow {
            name: row.name().to_owned(),
            pattern: row.pattern().clone(),
        })
        .collect()
}

/// Read the file-path policy's instance document: its owner sections.
///
/// Its whole declaration is an exclude-only owner section over a type no set
/// declares, which decodes because an exclude row's name labels the exclusion
/// itself and promises nothing about a set. What the policy requires, and which
/// sources it reads at all, are the program's own (´proposal:isolation:staging´).
fn references_parameters(
    documents: &BTreeMap<String, Declaration>,
    registered: &BTreeSet<String>,
    policies: &[Pair],
    refusals: &mut Vec<Refusal>,
) -> ReferenceParameters {
    let activated = activating(policies, REFERENCES_POLICY);

    let Some(document) = documents.get(PATH_LINKING_NAMESPACE) else {
        paired_sections(&BTreeSet::new(), &activated, refusals, |owner, found| {
            Refusal::UnpairedReferenceSection { owner, found }
        });

        return ReferenceParameters::default();
    };

    let mut sections = BTreeMap::new();

    for (owner, declaration) in document.owners() {
        if !registered.contains(owner) {
            refusals.push(Refusal::UnknownOwner {
                file: PATH_LINKING_NAMESPACE,
                owner: owner.clone(),
            });

            continue;
        }

        let Some((include, exclude)) = selected_rows(
            PATH_LINKING_NAMESPACE,
            owner,
            PATH_LINKING_SET,
            declaration,
            refusals,
        ) else {
            continue;
        };

        sections.insert(
            owner.clone(),
            ReferenceSection {
                exclude: reference_rows(exclude),
                include: glossed(declaration, PATH_LINKING_SET).then(|| reference_rows(include)),
            },
        );
    }

    let declared: BTreeSet<String> = sections.keys().cloned().collect();

    paired_sections(&declared, &activated, refusals, |owner, found| {
        Refusal::UnpairedReferenceSection { owner, found }
    });

    ReferenceParameters { sections }
}

/// Read every policy document for the burn surface it declares, by its domain.
///
/// The surfaces are gathered over the union of the documents rather than out of a
/// privileged one, because which policies census a corpus is the corpus's to say
/// and a loader holding a list of them would be holding that answer twice. A
/// document declaring neither half is a document about something else and
/// contributes nothing here (´gram:isolation:declaration´).
fn surface_parameters(
    documents: &BTreeMap<String, Declaration>,
    registered: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
) -> BTreeMap<String, BurnSurface> {
    let mut surfaces = BTreeMap::new();

    for (namespace, declaration) in documents {
        let Some(domain) = namespace.strip_prefix(POLICY_NAMESPACE) else {
            continue;
        };

        for (owner, declared) in declaration.owners() {
            let prose = declared.datum(PROSE_DATUM);
            let code = declared.datum(CODE_DATUM);

            if prose.is_none() && code.is_none() {
                continue;
            }

            if !registered.contains(owner) {
                refusals.push(Refusal::Declaration {
                    document: namespace.clone(),
                    message: format!("{owner}: no such registered owner"),
                });

                continue;
            }

            let prose = surface_places(namespace, owner, PROSE_DATUM, prose, refusals);
            let code = surface_places(namespace, owner, CODE_DATUM, code, refusals);

            if surfaces
                .insert(
                    domain.to_owned(),
                    BurnSurface::new(owner.clone(), prose, code),
                )
                .is_some()
            {
                refusals.push(Refusal::Declaration {
                    document: namespace.clone(),
                    message: String::from("two owners declare this policy's surface"),
                });
            }
        }
    }

    surfaces
}

/// The places one half of a declared surface opens with.
///
/// A surface is written as a pattern for the reason every reach in this surface
/// is: a run of alternatives is one statement rather than a list a reader has to
/// keep aligned, and the engine that compiles it is the engine that compiles the
/// partition beside it.
fn surface_places(
    namespace: &str,
    owner: &str,
    key: &str,
    datum: Option<&OwnerDatum>,
    refusals: &mut Vec<Refusal>,
) -> Vec<PathBuf> {
    let Some(datum) = datum else {
        return Vec::new();
    };

    let OwnerDatum::Text(source) = datum else {
        refusals.push(Refusal::Declaration {
            document: namespace.to_owned(),
            message: format!("owners.{owner}.{key}: a surface is a pattern"),
        });

        return Vec::new();
    };

    match AbnfPattern::parse(source) {
        Ok(pattern) => areas::places(&pattern),
        Err(defect) => {
            refusals.push(Refusal::Declaration {
                document: namespace.to_owned(),
                message: format!("owners.{owner}.{key}: {source}: {defect}"),
            });

            Vec::new()
        }
    }
}

/// Carry decoded pattern rows into the file-path policy's own row type.
fn reference_rows(rows: &[PatternRow]) -> Vec<ReferenceRow> {
    rows.iter()
        .map(|row| ReferenceRow {
            name: row.name().to_owned(),
            pattern: row.pattern().clone(),
        })
        .collect()
}

/// Validate the owner list, returning the set every other row is checked against.
fn registered_owners(owners: &[String], refusals: &mut Vec<Refusal>) -> BTreeSet<String> {
    let mut registered = BTreeSet::new();

    for owner in owners {
        if Prefix::parse(owner).is_none() {
            refusals.push(Refusal::MalformedOwner {
                owner: owner.clone(),
            });
            continue;
        }

        if !registered.insert(owner.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: OWNERS_FILE,
                row: owner.clone(),
            });
        }
    }

    registered
}

/// Validate one named owner-and-pattern relation.
///
/// A partition's rows name its parts, so the name is required, held to the
/// declared-name grammar, and unique in the relation: two parts answering to one
/// name make the report that names the part unreadable, exactly as two identical
/// rows do.
fn owner_rows(
    rows: &[RawOwnerRow],
    relation: &'static str,
    registered: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
) -> Vec<OwnerRow> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();
    let mut named = BTreeSet::new();

    for row in rows {
        let Some(name) = row.name.clone() else {
            refusals.push(Refusal::NamelessRow {
                file: OWNERS_FILE,
                relation,
                row: format!("{} : {}", row.owner, row.pattern),
            });

            continue;
        };

        if !is_declared_name(&name) {
            refusals.push(Refusal::MalformedRowName {
                file: OWNERS_FILE,
                relation,
                name,
            });

            continue;
        }

        if !named.insert(name.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: OWNERS_FILE,
                row: format!("{relation}: {name}"),
            });

            continue;
        }

        if !registered.contains(&row.owner) {
            refusals.push(Refusal::UnknownOwner {
                file: OWNERS_FILE,
                owner: row.owner.clone(),
            });

            continue;
        }

        let pattern = match AbnfPattern::parse(&row.pattern) {
            Ok(pattern) => pattern,
            Err(defect) => {
                refusals.push(Refusal::MalformedPattern {
                    file: OWNERS_FILE,
                    pattern: row.pattern.clone(),
                    message: defect.to_string(),
                });

                continue;
            }
        };

        if !seen.insert((row.owner.clone(), pattern.source().to_owned())) {
            refusals.push(Refusal::DuplicateRow {
                file: OWNERS_FILE,
                row: format!("{relation}: {} : {}", row.owner, pattern),
            });

            continue;
        }

        parsed.push(OwnerRow {
            name,
            owner: row.owner.clone(),
            pattern,
        });
    }

    parsed
}

/// Validate the declared may-cite relation's shape.
///
/// The relation states the reach an owner has to *other* owners. Its reach to
/// itself is not among the things a declaration can say, because it does not
/// follow from any row: an owner is its own corpus, and there is no arrangement
/// of this file under which it stops being one. A row saying so is refused
/// rather than absorbed, so that what the file contains is the set of decisions
/// somebody actually made.
fn reach_rows(
    rows: &[RawReachRow],
    registered: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
) -> Vec<ReachRow> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        for owner in [&row.owner, &row.target] {
            if !registered.contains(owner) {
                refusals.push(Refusal::UnknownOwner {
                    file: OWNERS_FILE,
                    owner: owner.clone(),
                });
            }
        }

        if row.owner == row.target {
            refusals.push(Refusal::SelfReach {
                owner: row.owner.clone(),
            });

            continue;
        }

        if !seen.insert((row.owner.clone(), row.target.clone())) {
            refusals.push(Refusal::DuplicateRow {
                file: OWNERS_FILE,
                row: format!("may_cite: {} : {}", row.owner, row.target),
            });

            continue;
        }

        parsed.push(ReachRow {
            owner: row.owner.clone(),
            target: row.target.clone(),
        });
    }

    parsed
}

/// Validate the declared environment relation's shape.
fn environment_rows(
    rows: &[RawEnvironmentRow],
    refusals: &mut Vec<Refusal>,
) -> Vec<EnvironmentRow> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        if !is_environment_name(&row.environment) {
            refusals.push(Refusal::MalformedEnvironment {
                environment: row.environment.clone(),
            });

            continue;
        }

        if !is_kind_token(&row.kind) {
            refusals.push(Refusal::MalformedKind {
                kind: row.kind.clone(),
            });

            continue;
        }

        if !seen.insert((row.environment.clone(), row.kind.clone())) {
            refusals.push(Refusal::DuplicateRow {
                file: ENVIRONMENTS_FILE,
                row: format!("{} : {}", row.environment, row.kind),
            });

            continue;
        }

        parsed.push(EnvironmentRow {
            environment: row.environment.clone(),
            kind: row.kind.clone(),
        });
    }

    parsed
}

/// Validate one declared set of reserved kind tokens.
fn reserved_kind_set(
    kinds: &[String],
    relation: &str,
    refusals: &mut Vec<Refusal>,
) -> BTreeSet<String> {
    let mut parsed = BTreeSet::new();

    for kind in kinds {
        if !is_kind_token(kind) {
            refusals.push(Refusal::MalformedKind { kind: kind.clone() });

            continue;
        }

        if !parsed.insert(kind.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: ENVIRONMENTS_FILE,
                row: format!("{relation}: {kind}"),
            });
        }
    }

    parsed
}

/// Whether an environment name is a nonempty name with no edge whitespace or line break.
fn is_environment_name(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && !name.contains(['\n', '\r'])
}

/// Whether a kind token is a word of the label grammar.
fn is_kind_token(kind: &str) -> bool {
    !kind.is_empty()
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// Validate the activation pairs against the owner list and this binary's catalog.
fn activation_pairs(
    rows: &[RawPair],
    registered: &BTreeSet<String>,
    refusals: &mut Vec<Refusal>,
) -> Vec<Pair> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        if !registered.contains(&row.owner) {
            refusals.push(Refusal::UnknownOwner {
                file: POLICIES_FILE,
                owner: row.owner.clone(),
            });

            continue;
        }

        if !is_policy_identifier(&row.policy) {
            refusals.push(Refusal::MalformedPolicy {
                file: POLICIES_FILE,
                policy: row.policy.clone(),
            });

            continue;
        }

        if catalogued(&row.policy).is_none() {
            refusals.push(Refusal::UnknownPolicy {
                file: POLICIES_FILE,
                policy: row.policy.clone(),
            });

            continue;
        }

        let pair = Pair::singleton(row.owner.clone(), row.policy.clone());

        if !seen.insert(pair.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: POLICIES_FILE,
                row: pair.to_string(),
            });

            continue;
        }

        parsed.push(pair);
    }

    parsed
}

/// The corpus prefix a ratchet key's middle component is relative to.
///
/// It is the list document's own namespace less its final component, which is why
/// the shorter key form is generic rather than a convenience for one repository: a
/// corpus whose documents open with another prefix keys its lists the same way and
/// the reader needs no compiled answer (´reg:isolation:owner-questions´).
fn corpus_prefix(namespace: &str) -> Option<&str> {
    let (prefix, _own) = namespace.rsplit_once('.')?;

    (!prefix.is_empty()).then_some(prefix)
}

/// Read the list namespace from the already-parsed surface tree.
fn list_namespace_from_ast(surface: &SurfaceAst) -> String {
    surface
        .document(LISTS_FILE)
        .and_then(|document| document.table().ok())
        .and_then(|table| {
            table
                .get("namespace")
                .and_then(toml::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_default()
}

/// Every deployment the instance documents declare, by the key that names it.
///
/// A deployment is a set entry of a document whose set type this binary
/// catalogues a program for, keyed as a ratchet key spells it: the document's
/// namespace relative to the corpus prefix, then the entry. Which entries exist is
/// the corpus's to say, so nothing here is compiled but the map from set type to
/// program (´gram:isolation:declaration´).
fn deployments(
    documents: &BTreeMap<String, Declaration>,
    prefix: Option<&str>,
) -> BTreeMap<(String, String), String> {
    let mut deployed = BTreeMap::new();

    for (namespace, declaration) in documents {
        let relative = prefix
            .and_then(|prefix| namespace.strip_prefix(prefix))
            .map_or_else(
                || namespace.clone(),
                |rest| rest.trim_start_matches('.').to_owned(),
            );

        for (set, declared) in declaration.sets() {
            let Some(program) = program_of(set) else {
                continue;
            };

            for entry in declared.entries().keys() {
                deployed.insert((relative.clone(), entry.clone()), program.to_owned());
            }
        }
    }

    deployed
}

/// The program a ratchet key names, where the surface declares one.
///
/// A key naming no set entry names a program outright; a key naming one names a
/// deployment, and the deployments are what the instance documents declared.
fn keyed_program(pair: &Pair, deployed: &BTreeMap<(String, String), String>) -> Option<String> {
    pair.family.as_ref().map_or_else(
        || catalogued(&pair.policy).map(|policy| policy.name.to_owned()),
        |entry| deployed.get(&(pair.policy.clone(), entry.clone())).cloned(),
    )
}

/// Validate the list tables against the activations and the codecs their programs select.
///
/// Two directions, as they always were, and the relation between them has widened
/// by exactly what the ratified grammar widened: an activation is covered by at
/// least one key rather than by exactly one, because a program acting on a
/// declared set acts once per entry the corpus deployed and each of those
/// deployments earns a ratchet of its own (´conv:isolation:registers´).
fn list_tables(
    raw: &BTreeMap<String, BTreeMap<String, crate::surface::ListEntry>>,
    registered: &BTreeSet<String>,
    policies: &[Pair],
    deployed: &BTreeMap<(String, String), String>,
    partitions: &[OwnerRow],
    refusals: &mut Vec<Refusal>,
) -> BTreeMap<Pair, Rows> {
    let activated: BTreeSet<&Pair> = policies.iter().collect();
    let mut tables = BTreeMap::new();
    let mut covered: BTreeSet<Pair> = BTreeSet::new();

    for (owner, per_policy) in raw {
        if !registered.contains(owner) {
            refusals.push(Refusal::UnknownOwner {
                file: LISTS_FILE,
                owner: owner.clone(),
            });

            continue;
        }

        for (policy, entry) in per_policy {
            // One list stands at a program this corpus deploys once and one table
            // of lists stands at a document whose entries it deploys several
            // times, so both shapes reduce to the same question asked once per
            // key: the entry is part of the key rather than a column inside a
            // shared table.
            let lists: Vec<(Option<String>, &crate::surface::ListTable)> = match entry {
                crate::surface::ListEntry::Singleton(list) => vec![(None, list)],
                crate::surface::ListEntry::Instanced(entries) => entries
                    .iter()
                    .map(|(entry, list)| (Some(entry.clone()), list))
                    .collect(),
            };

            for (family, list) in lists {
                let pair = Pair {
                    owner: owner.clone(),
                    policy: policy.clone(),
                    family,
                };

                let Some(program) = keyed_program(&pair, deployed) else {
                    refusals.push(if is_policy_identifier(policy) || pair.family.is_some() {
                        Refusal::UnknownPolicy {
                            file: LISTS_FILE,
                            policy: pair.to_string(),
                        }
                    } else {
                        Refusal::MalformedPolicy {
                            file: LISTS_FILE,
                            policy: policy.clone(),
                        }
                    });

                    continue;
                };

                let activation = Pair::singleton(owner.clone(), program.clone());

                if !activated.contains(&activation) {
                    refusals.push(Refusal::OrphanList { pair });

                    continue;
                }

                let Some(catalogued) = catalogued(&program) else {
                    continue;
                };

                let Some(rows) = codec_rows(
                    &pair,
                    &program,
                    catalogued.codec,
                    list,
                    partitions,
                    refusals,
                ) else {
                    continue;
                };

                covered.insert(activation);
                tables.insert(pair, rows);
            }
        }
    }

    for pair in policies {
        if !covered.contains(pair) {
            refusals.push(Refusal::UnpairedPolicy { pair: pair.clone() });
        }
    }

    tables
}

/// Read one list table in the codec its policy selects, refusing every other shape.
fn codec_rows(
    pair: &Pair,
    program: &str,
    codec: Codec,
    list: &crate::surface::ListTable,
    partitions: &[OwnerRow],
    refusals: &mut Vec<Refusal>,
) -> Option<Rows> {
    let written: Vec<&'static str> = [
        (list.allowances.is_some(), Codec::Fingerprint),
        (list.path_counts.is_some(), Codec::PathCount),
        (list.paths.is_some(), Codec::PathSet),
    ]
    .into_iter()
    .filter(|(present, _codec)| *present)
    .map(|(_present, codec)| codec.field())
    .collect();

    let found = match written.as_slice() {
        [field] => *field,
        [] => "nothing",
        _ => "several",
    };

    match (codec, &list.allowances, &list.path_counts, &list.paths) {
        (Codec::Fingerprint, Some(rows), None, None) => {
            Some(Rows::Allowances(allowance_rows(pair, rows, refusals)))
        }
        (Codec::PathCount, None, Some(rows), None) => Some(Rows::PathCounts(path_count_rows(
            pair, program, rows, partitions, refusals,
        ))),
        (Codec::PathSet, None, None, Some(rows)) => Some(Rows::Paths(path_rows(
            pair, program, rows, partitions, refusals,
        ))),
        _ => {
            refusals.push(Refusal::WrongCodec {
                pair: pair.clone(),
                expected: codec,
                found: found.to_owned(),
            });

            None
        }
    }
}

/// Validate one fingerprint list.
fn allowance_rows(
    pair: &Pair,
    rows: &[crate::surface::AllowanceRow],
    refusals: &mut Vec<Refusal>,
) -> Vec<Allowance> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        if !is_fingerprint(&row.fingerprint) {
            refusals.push(Refusal::MalformedFingerprint {
                fingerprint: row.fingerprint.clone(),
            });

            continue;
        }

        let Some(maximum) = positive(row.maximum) else {
            refusals.push(Refusal::NonPositiveMaximum {
                row: format!("{pair}: {}", row.fingerprint),
            });

            continue;
        };

        if !seen.insert(row.fingerprint.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: LISTS_FILE,
                row: format!("{pair}: {}", row.fingerprint),
            });

            continue;
        }

        parsed.push(Allowance {
            fingerprint: row.fingerprint.clone(),
            maximum,
        });
    }

    parsed.sort();
    parsed
}

/// Validate one path-count list.
///
/// A row is held to owner containment exactly when its policy divides the owner
/// partition. A census declared over the corpus root does not: it reaches every
/// share at once, so its ratchet is one repository-wide artifact filed under the
/// owner who activated it, and holding each of its rows to that owner's
/// attribution would refuse the very table the census earned
/// (ADR-T-019, The layer owner graph).
fn path_count_rows(
    pair: &Pair,
    program: &str,
    rows: &[crate::surface::PathCountRow],
    partitions: &[OwnerRow],
    refusals: &mut Vec<Refusal>,
) -> Vec<PathCount> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        let path = match BytePath::decode(&row.path) {
            Ok(path) => path,
            Err(defect) => {
                refusals.push(Refusal::MalformedPath {
                    path: row.path.clone(),
                    defect,
                });

                continue;
            }
        };

        let Some(maximum) = positive(row.maximum) else {
            refusals.push(Refusal::NonPositiveMaximum {
                row: format!("{pair}: {}", path.display()),
            });

            continue;
        };

        if !seen.insert(path.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: LISTS_FILE,
                row: format!("{pair}: {}", path.display()),
            });

            continue;
        }

        if !spans_corpus(program) && !attributed_to(&path, &pair.owner, partitions) {
            refusals.push(Refusal::OwnerPathMismatch {
                pair: pair.clone(),
                path: path.display(),
            });

            continue;
        }

        parsed.push(PathCount { path, maximum });
    }

    parsed.sort();
    parsed
}

/// Validate one path-set list.
///
/// A row is a bare path and carries no ceiling, because the identity holds at
/// most one violation and a ceiling that could only ever be one would encode
/// nothing. The path is held to the same two rules a path-count row's is: it
/// decodes canonically, and it is filed under the owner the inclusion relation
/// attributes it to.
fn path_rows(
    pair: &Pair,
    program: &str,
    rows: &[String],
    partitions: &[OwnerRow],
    refusals: &mut Vec<Refusal>,
) -> Vec<BytePath> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        let path = match BytePath::decode(row) {
            Ok(path) => path,
            Err(defect) => {
                refusals.push(Refusal::MalformedPath {
                    path: row.clone(),
                    defect,
                });

                continue;
            }
        };

        if !seen.insert(path.clone()) {
            refusals.push(Refusal::DuplicateRow {
                file: LISTS_FILE,
                row: format!("{pair}: {}", path.display()),
            });

            continue;
        }

        if !spans_corpus(program) && !attributed_to(&path, &pair.owner, partitions) {
            refusals.push(Refusal::OwnerPathMismatch {
                pair: pair.clone(),
                path: path.display(),
            });

            continue;
        }

        parsed.push(path);
    }

    parsed.sort();
    parsed
}

/// Whether the inclusion relation attributes a declared path to exactly this owner.
///
/// The question is asked of the declaration alone and never of the tree, so a row
/// naming a file that has since been deleted is still attributed. What it catches
/// is a row filed under the wrong owner, which is a defect of the snapshot rather
/// than a disagreement with the repository.
fn attributed_to(path: &BytePath, owner: &str, partitions: &[OwnerRow]) -> bool {
    let mut matched = partitions
        .iter()
        .filter(|row| row.pattern.admits_path(path));

    match (matched.next(), matched.next()) {
        (Some(row), None) => row.owner == owner,
        _ => false,
    }
}

/// A ceiling, when it is the positive integer a row requires.
const fn positive(maximum: i64) -> Option<u64> {
    if maximum >= 1 {
        Some(maximum.cast_unsigned())
    } else {
        None
    }
}

/// Whether text is this producer's canonical fingerprint form.
fn is_fingerprint(text: &str) -> bool {
    let Some(digits) = text.strip_prefix(FINGERPRINT_PREFIX) else {
        return false;
    };

    digits.len() == FINGERPRINT_DIGITS
        && digits
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
