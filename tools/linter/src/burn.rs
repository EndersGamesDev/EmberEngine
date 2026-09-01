// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Burn lists: a legacy reference family, censused, and a ratchet that may only
//! shrink.
//!
//! The burn discipline (ADR-T-020, The migration disciplines) requires that
//! every legacy reference family is enumerated exactly and that the enumeration
//! is a ratchet: a new occurrence anywhere is a failure, and a vanished
//! occurrence must leave the enumeration in the same commit. This module is that
//! verification.
//!
//! # Where the enumeration stands
//!
//! It stands in the canonical list document, keyed by owner and program like
//! every other tolerated debt. It used to stand in a generated register document
//! per family, and those registers retired: content that was an argument became a
//! record, content that was a ratchet became these rows, and no declaration
//! points at a file (´conv:isolation:registers´). What the gate compares did not
//! change with them — it is the register-equals-census relation over the same
//! numbers, asked of the list.
//!
//! A family declared over the corpus root is keyed to the root owner rather than
//! to the owner of each file it counts, because the ratchet such a census earns is
//! one repository-wide artifact that no member can repair alone
//! (ADR-T-019, The layer owner graph).
//!
//! # Why an enumeration rather than a count
//!
//! A gate could hold the corpus to a total and be simpler. It would also be
//! nearly useless, because a total lets one file lose a reference while another
//! gains one and calls the trade clean — which is exactly the motion a migration
//! must not make. The ratchet is per file, so it holds file by file, and a commit
//! that migrates one document cannot pay for a regression in another.
//!
//! # Why per file and not per occurrence
//!
//! The alternative is a row per occurrence, naming it by line. It is exact, and it
//! churns: an unrelated edit at the top of a document moves every line number
//! below it, and the diff then hides the one change that matters among hundreds
//! that do not. A per-file count is invariant under every edit that does not
//! change what the file references, which is the property a reviewer needs to read
//! the diff at all, and it is still exact enough to ratchet — a file may only be
//! listed at its count or lower, and a file not listed may hold none.
//!
//! Locations are not lost by this: the census knows them, and reports them in the
//! finding that names a violation.
//!
//! # Both directions, and why the stale half matters
//!
//! Growth is the obvious failure. Shrinkage without a list edit is the subtle
//! one, and it is a failure here for the reason outline tracking checks its
//! relation both ways: a ratchet that is allowed to overstate quietly becomes a
//! declaration about a corpus that no longer exists, and the ratchet it is
//! supposed to be turns into a ceiling nobody has measured. So a file whose
//! occurrences have fallen is a failure until the list falls with it, in the same
//! commit that removed them.
//!
//! # The recognizers are the lint's own
//!
//! The shapes a burn list counts are the shapes the migration lint reports, read
//! by the same code. Prose is read by the lint's Markdown reader, so a form in
//! code font or a fenced block is displayed rather than referenced, exactly as
//! the lint has it. Rust sources are read for their comments alone, and the
//! comments are read by the recognizer the lint uses on running text. A census
//! that recognized the forms its own way would eventually disagree with the gate
//! about what a reference is, and a ratchet cannot survive that.
//!
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::catalogue::{CATALOG, Observer, catalogued};
use crate::finding::{Finding, Location};
use crate::legacy::{LegacyRule, scan_legacy, scan_regions};
use crate::legacy_profile::{LegacySite, is_production_source, scan_legacy_sites};
use crate::plan::CorpusPlan;
use crate::program::{LiteralSet, MarkNumbered, PrefixNumbers};
use crate::residual::{
    Residual, scan_residual_comments, scan_residual_markdown, scan_residual_script,
};
use crate::retired::{Retired, RetiredFamily, scan_retired_markdown, scan_retired_text};
use crate::snapshot::{DIRECTORY, LISTS_FILE, Rows, Snapshot};
use crate::todo::{TodoNotice, scan_todos};
use crate::token::{Region, markdown_regions, rust_regions};

/// The owner this binary is, whose share no census of this binary's own walks.
///
/// It is the one owner identifier compiled here, and it is compiled because it is
/// self-knowledge rather than a fact about the repository: a checker knows which
/// owner it is the same way it knows its own name, and asking a declaration to
/// tell it would be asking the corpus to say who is judging it. Where that share
/// stands is the corpus's to state and is read from the declared partition at
/// runtime, so nothing here knows a directory (´reg:isolation:owner-questions´).
///
/// ´const:indexlinter:self-owner´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:indexlinter:self-owner-text-xc0fc6733´
const SELF_OWNER: &str = "LINTER";

/// The run a repository-scoped family's policy carries beside its per-owner sibling.
///
/// Two policies of one program stand in the catalog wherever a family is censused
/// both ways: one per-owner, activated by the owner whose debt it is, and one over
/// the repository, activated by the owner whose share the repository is. The
/// second's identifier is the first's with this run appended, and that is the
/// whole of what tells them apart here — a compiled fact about this binary's own
/// vocabulary, spelling no repository fact at all.
///
/// The run is what makes a corpus-wide census tellable from a per-owner one
/// without asking a declaration which it is, and the ratchet such a census earns
/// belongs to the root owner for exactly that reason
/// (´[ORCHESTRATION-conv:layers:verdict-location]´).
///
/// ´const:indexlinter:repository-policy-run´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:indexlinter:repository-policy-run-text-x8f7cd1cc´
const REPOSITORY_RUN: &str = "-repository";

/// The burn lists this repository adopted.
///
/// Three open the discipline, one per legacy family the Assayer's campaign
/// retires — section references, retired record numbers and superseded tag
/// forms — each enumerated in a register the linter verifies exactly, a ratchet
/// that may only shrink (´[ORCHESTRATION-req:migration:burn-ratchet]´). A fourth reads
/// the record shape the other three cannot: a number written with no series at
/// all, which is ambiguous between the corpus that keeps its numbers and the
/// local numbering that retired. It is declared beside the lettered family
/// rather than folded into it, because a family is a bounded shape and
/// broadening one to admit a second shape leaves neither bounded
/// (´[ORCHESTRATION-conv:migration:burn-family]´) — and because the two are swept by
/// different judgments, the lettered form by rewriting a citation and the bare
/// one by first finding which record was meant. A fifth counts
/// an inventory profile's remainder rather than a reference family, under the
/// policy ADR-T-016 records
/// (´[ORCHESTRATION-conv:migration:burn-inventory-remainder]´). The remaining two are
/// the families the scenario matrix took with it when it retired, each
/// declaring its own recogniser and surface beside its register — the scenario
/// numbers bounded by range and the division names by enumeration, which are
/// the two bounding devices the family convention licenses
/// (´[ORCHESTRATION-conv:migration:burn-family]´).
///
/// Each declares what it counts and nothing about where. Where a family's census
/// walks is the corpus's to state, and the corpus states it in the document of
/// the policy that activates the census — because if a policy needs to know what
/// a source is and what a bench is, that knowledge belongs to the domain of that
/// policy. So a family is joined to its policy through the catalog and asks that
/// policy's own document for its surface; nothing here knows a directory, a reach
/// name or an owner's geography (´[ORCHESTRATION-conv:migration:burn-surfaces]´).
///
/// ´const:indexlinter:adopted-burn-lists´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:adopted-burn-lists-form-x34de554d´
const BURN_LISTS: &[Declaration] = &[
    Declaration {
        family: "section-sign references",
        shape: Shape::Legacy(&[LegacyRule::SectionNumber]),
    },
    Declaration {
        family: "retired record numbers",
        shape: Shape::Legacy(&[LegacyRule::RecordNumber]),
    },
    Declaration {
        family: "ambiguous unprefixed record numbers",
        shape: Shape::Legacy(&[LegacyRule::UnprefixedRecordNumber]),
    },
    Declaration {
        family: "superseded tag forms",
        shape: Shape::Legacy(&[LegacyRule::TagForm]),
    },
    Declaration {
        family: "retired scenario numbers",
        shape: Shape::Retired(RetiredFamily::ScenarioNumber),
    },
    Declaration {
        family: "retired division names",
        shape: Shape::Retired(RetiredFamily::DivisionName),
    },
    Declaration {
        family: "residual litter",
        shape: Shape::ResidualLitter,
    },
    Declaration {
        family: "unlabelled to-do notices",
        shape: Shape::UnlabelledTodo,
    },
    Declaration {
        family: "unlabelled legacy implementations",
        shape: Shape::UnlabelledLegacy,
    },
    Declaration {
        family: "section-sign references (repository)",
        shape: Shape::Legacy(&[LegacyRule::SectionNumber]),
    },
    Declaration {
        family: "retired record numbers (repository)",
        shape: Shape::RepositoryRecordNumber,
    },
];

/// What a burn list counts, and therefore which recognizer reads its surfaces.
///
/// The families the migration retires are shapes of the migration lint, read by
/// the lint's own code. The to-do family is not a reference form at all — it is
/// the remainder of an inventory profile, the notices that have not yet been
/// labelled — so it is read by that profile's own census, for the reason given
/// above about recognizers: the register and the gate must count one thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// The reference forms of the migration lint, held to the rules given.
    Legacy(&'static [LegacyRule]),
    /// A reference naming one of the repository's T-series records by number.
    ///
    /// This is track-only: the recognizer records the shape without making the
    /// later adoption decision about whether root prose replaces it with the
    /// record's label-headed identity.
    RepositoryRecordNumber,
    /// The covered notices of ADR-T-016's profile that carry no label.
    UnlabelledTodo,
    /// The marked legacy implementations that carry no derived label.
    UnlabelledLegacy,
    /// The three shapes an earlier sweep, or a retired notation, left standing.
    ///
    /// Read by its own recognizer for the reason the retired families are, and
    /// for one more of its own: two of its three shapes are degraded spellings
    /// of rules the shared migration lint already carries, and widening that
    /// lint to reach them is reserved to the record that retired those forms.
    /// Until that record is amended the family counts and judges nothing, which
    /// is the order the fourth family landed in.
    ResidualLitter,
    /// One of the two identity schemes the retired scenario matrix defined.
    ///
    /// These are read by their own recognizer rather than by the migration
    /// lint's, and the reason is the one the module head gives: no lint rule
    /// judges either shape, so the register is the only counter and cannot come
    /// to disagree with a second one.
    Retired(RetiredFamily),
}

impl Shape {
    /// The lint rules this shape reads with, which is none outside the lint.
    #[must_use]
    pub const fn rules(&self) -> &'static [LegacyRule] {
        match self {
            Self::Legacy(rules) => rules,
            Self::RepositoryRecordNumber
            | Self::UnlabelledTodo
            | Self::UnlabelledLegacy
            | Self::Retired(_)
            | Self::ResidualLitter => &[],
        }
    }

    /// The file extensions this shape's recognizer can read a code tree for.
    ///
    /// Which kinds of source a shape reads belongs with the shape rather than
    /// with the declaration, for the reason the recognizer does: a family is
    /// read by one reader, and a reader knows the languages it can lex. Every
    /// family but one reads Rust alone. The residual family reads shell scripts
    /// beside it, because one of its occurrences stands in a script's provenance
    /// comment and a census that could not see it would close with that debt
    /// standing.
    #[must_use]
    pub const fn code_extensions(&self) -> &'static [&'static str] {
        match self {
            Self::ResidualLitter => &["rs", "sh"],
            Self::Legacy(_)
            | Self::RepositoryRecordNumber
            | Self::UnlabelledTodo
            | Self::UnlabelledLegacy
            | Self::Retired(_) => &["rs"],
        }
    }

    /// The declared domain a family's surface stands in, where it is not its policy's own.
    ///
    /// A domain is what a document is about, and three of these families are
    /// about something a document already declares: the shapes their recognizers
    /// read are the marked ordinals, the verbatim sentences and the prefix-number
    /// schemes the corpus writes down, and a census of a shape belongs beside the
    /// declaration of that shape rather than in a second document about the same
    /// subject. Which declared payload a recognizer reads is the shape's own
    /// property for the reason the extensions above are — a family is read by one
    /// reader, and a reader knows what it reads for.
    ///
    /// Every other family is about nothing but itself, so its surface stands in
    /// the document its policy identifier names and this answers `None`.
    #[must_use]
    pub const fn domain(&self) -> Option<&'static str> {
        match self {
            Self::Retired(RetiredFamily::ScenarioNumber) => Some("references.scenarios"),
            Self::Retired(RetiredFamily::DivisionName) => Some("references.divisions"),
            Self::ResidualLitter => Some("references.prefix-numbers"),
            Self::Legacy(_)
            | Self::RepositoryRecordNumber
            | Self::UnlabelledTodo
            | Self::UnlabelledLegacy => None,
        }
    }
}

/// One burn list, as the adoption data declare it.
///
/// Two fields and no third. Which shape a family counts and how its recognizer
/// reads a source are this binary's to know — they are the program — while where
/// it looks is the corpus's to state, and the corpus states it in the document of
/// the policy this family is censused under. The join to that policy is the
/// catalog's, so the family name carries it and nothing is written twice.
struct Declaration {
    family: &'static str,
    shape: Shape,
}

/// One policy's declared burn surface: whose geography it stands in, and where its census walks.
///
/// The trees are places rather than names, because a policy document has no owner
/// root to resolve a name against and no second document to resolve it in: the
/// corpus writes out what its census walks, in the domain of the policy that
/// activates it. The owner rides along because a surface is written inside some
/// share and a report saying whose is saying something a reader can check against
/// the partition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Surface {
    owner: String,
    prose: Vec<PathBuf>,
    code: Vec<PathBuf>,
}

impl Surface {
    /// Declare a surface from the owner that wrote it and the trees it walks.
    #[must_use]
    pub fn new(owner: impl Into<String>, prose: Vec<PathBuf>, code: Vec<PathBuf>) -> Self {
        Self {
            owner: owner.into(),
            prose,
            code,
        }
    }

    /// The owner whose share this surface was written in.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The prose trees the census walks.
    #[must_use]
    pub fn prose(&self) -> &[PathBuf] {
        &self.prose
    }

    /// The code trees the census walks.
    #[must_use]
    pub fn code(&self) -> &[PathBuf] {
        &self.code
    }
}

/// The declared values the retiring families' recognizers read for.
///
/// Three payloads rather than one per family, because what a recognizer needs is
/// decided by its shape rather than by its name: the two retired families take a
/// marked ordinal and a literal set, and the residual family takes the whole run
/// of prefix-number schemes at once. Each is the program value the census
/// consumes, built by the loader from the document that declares it, so nothing
/// here knows a mark, a sentence or a prefix.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recognizer {
    marks: Vec<MarkNumbered>,
    literals: Option<LiteralSet>,
    prefix_numbers: Vec<PrefixNumbers>,
}

impl Recognizer {
    /// Read every payload the retiring families need out of one snapshot.
    #[must_use]
    pub fn declared(snapshot: &Snapshot) -> Self {
        Self {
            marks: snapshot
                .numbered_marks()
                .into_iter()
                .map(|(_name, mark)| mark)
                .collect(),
            literals: snapshot.declared_literals(),
            prefix_numbers: snapshot
                .declared_prefix_numbers()
                .into_iter()
                .map(|(_name, numbers)| numbers)
                .collect(),
        }
    }

    /// Declare a payload directly, for a caller standing outside a snapshot.
    #[must_use]
    pub fn new(
        marks: impl IntoIterator<Item = MarkNumbered>,
        literals: Option<LiteralSet>,
        prefix_numbers: impl IntoIterator<Item = PrefixNumbers>,
    ) -> Self {
        Self {
            marks: marks.into_iter().collect(),
            literals,
            prefix_numbers: prefix_numbers.into_iter().collect(),
        }
    }

    /// The marked ordinals the scenario family is read with.
    #[must_use]
    pub fn marks(&self) -> &[MarkNumbered] {
        &self.marks
    }

    /// The literal set the division family is read with.
    #[must_use]
    pub const fn literals(&self) -> Option<&LiteralSet> {
        self.literals.as_ref()
    }

    /// The prefix-number schemes the residual family is read with.
    #[must_use]
    pub fn prefix_numbers(&self) -> &[PrefixNumbers] {
        &self.prefix_numbers
    }
}

/// One burn list: a family, the surfaces it is counted over, and its register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurnList {
    family: String,
    shape: Shape,
    prose: Vec<PathBuf>,
    code: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    recognizer: Recognizer,
}

impl BurnList {
    /// Declare a burn list over a family and the surfaces it is counted over.
    ///
    /// A list starts excluding nothing. What a census never reads is a statement
    /// the corpus makes about its own trees, so a list built outside a
    /// declaration has no exclusions to inherit and takes them from
    /// [`BurnList::with_excluded`] where it has any.
    #[must_use]
    pub fn new(
        family: impl Into<String>,
        shape: Shape,
        prose: Vec<PathBuf>,
        code: Vec<PathBuf>,
    ) -> Self {
        Self {
            family: family.into(),
            shape,
            prose,
            code,
            excluded: Vec::new(),
            recognizer: Recognizer::default(),
        }
    }

    /// Give the list the trees the corpus says its census never reads.
    #[must_use]
    pub fn with_excluded(mut self, excluded: impl IntoIterator<Item = PathBuf>) -> Self {
        self.excluded = excluded.into_iter().collect();

        self
    }

    /// Hand the list the declared values its recognizer reads for.
    #[must_use]
    pub fn reading(mut self, recognizer: Recognizer) -> Self {
        self.recognizer = recognizer;

        self
    }

    /// The declared values this list's recognizer reads for.
    #[must_use]
    pub const fn recognizer(&self) -> &Recognizer {
        &self.recognizer
    }

    /// What this list counts, and therefore which recognizer reads it.
    #[must_use]
    pub const fn shape(&self) -> &Shape {
        &self.shape
    }

    /// The family this list counts, as a register names it.
    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    /// The lint rules whose shapes make up the family, where the lint reads it.
    #[must_use]
    pub const fn rules(&self) -> &[LegacyRule] {
        self.shape.rules()
    }

    /// The prose trees the family is counted over.
    #[must_use]
    pub fn prose(&self) -> &[PathBuf] {
        &self.prose
    }

    /// The Rust trees whose comments the family is counted over.
    #[must_use]
    pub fn code(&self) -> &[PathBuf] {
        &self.code
    }

    /// The trees the census never reads, whatever surfaces were declared.
    #[must_use]
    pub fn excluded(&self) -> &[PathBuf] {
        &self.excluded
    }

    /// Whether a path falls in a tree no census reads.
    fn is_excluded(&self, path: &Path) -> bool {
        self.excluded.iter().any(|tree| path.starts_with(tree))
    }

    /// Whether this family's census is one repository-wide artifact.
    ///
    /// The answer is read off the declared surfaces rather than declared beside
    /// them: a surface naming the corpus root reaches every owner's share, so the
    /// census it takes is nobody's share in particular. Such a census is one
    /// repository-wide artifact that no member can repair alone, which is exactly
    /// the shape whose verdict belongs to the root owner rather than to the owner
    /// of each file it counts (ADR-T-019, The layer owner graph).
    #[must_use]
    pub fn spans_corpus(&self) -> bool {
        self.prose
            .iter()
            .chain(self.code.iter())
            .any(|root| root == Path::new("."))
    }
}

/// Whether the family of this name censuses the corpus rather than one share.
///
/// The lookup stands beside [`BurnList::spans_corpus`] so that a caller holding a
/// family name rather than a censused list can ask the same question, and the two
/// answers cannot drift apart. This one is asked without a tree and therefore
/// without a declaration, so it reads the identifier the catalog gives the
/// family: a repository-scoped policy is the corpus-wide half of a program whose
/// other half is per-owner, and its identifier says so
/// (ADR-T-019, The layer owner graph).
#[must_use]
pub fn family_spans_corpus(family: &str) -> bool {
    policy_of(family).is_some_and(|policy| policy.ends_with(REPOSITORY_RUN))
}

/// The policy a family's census is activated under, as this binary catalogues it.
///
/// One join, asked here and nowhere else. A family is a shape this binary reads
/// and a policy is the vocabulary a declaration activates, and the catalog is the
/// one place the two are tied together — so a family's document, its ratchet and
/// its derived exclusions are all found from the same tie rather than from three
/// tables that could come to disagree.
fn policy_of(family: &str) -> Option<&'static str> {
    CATALOG
        .iter()
        .find(|policy| matches!(policy.observer, Observer::BurnFamily(named) if named == family))
        .map(|policy| policy.name)
}

/// The domain whose document declares one family's surface.
///
/// It is the family's policy identifier wherever the policy has a document of its
/// own, and the shape's declared domain where the family's subject already has
/// one. Either way it is a name this binary knows about itself, resolved against
/// the corpus's documents rather than against its directories.
fn domain_of(declaration: &Declaration) -> Option<&'static str> {
    declaration
        .shape
        .domain()
        .or_else(|| policy_of(declaration.family))
}

/// The owner a family's ratchet is declared at, where the snapshot activates one.
fn activating_owner(snapshot: &Snapshot, family: &str) -> Option<String> {
    let policy = policy_of(family)?;

    snapshot
        .policies()
        .iter()
        .find(|pair| snapshot.program(pair) == Some(policy))
        .map(|pair| pair.owner.clone())
}

/// The shares no census of this binary's reads, derived rather than declared.
///
/// Two rules, and both are policy read against declared data rather than a
/// repository fact written down twice.
///
/// The first is self-exemption. A census never walks the share of the owner that
/// is the checker itself: the linter's own sources carry every shape it retires,
/// as fixtures and as the prose arguing about them, and a census counting those
/// would be a register that grows every time it records a shrinkage. Which owner
/// this binary is, is self-knowledge; where that owner's share stands is looked up
/// in the declared partition at runtime.
///
/// The second is sibling-activation exclusion, and it reaches only a
/// repository-scoped family. Such a family is the corpus-wide half of a program
/// whose other half is activated per owner, and an owner that has taken the
/// per-owner half has taken its own debt: counting that share twice would put one
/// occurrence in two ratchets and let a repair in one be paid for by the other.
/// So the corpus-wide census cuts out exactly the shares whose owners activate the
/// sibling, read from the activation document at runtime.
///
/// A share the declaration does not partition is cut out by neither rule, because
/// a share nobody has placed is not a place.
fn derived_exclusions(snapshot: &Snapshot, policy: Option<&str>) -> Vec<PathBuf> {
    let mut cut: Vec<PathBuf> = Vec::new();

    if let Some(sibling) = policy
        .and_then(|policy| policy.strip_suffix(REPOSITORY_RUN))
        .filter(|sibling| catalogued(sibling).is_some())
    {
        let owners: BTreeSet<&str> = snapshot
            .policies()
            .iter()
            .filter(|pair| pair.policy == sibling && pair.family.is_none())
            .map(|pair| pair.owner.as_str())
            .collect();

        cut.extend(
            owners
                .into_iter()
                .filter_map(|owner| snapshot.share_root(owner)),
        );
    }

    if let Some(own) = snapshot.share_root(SELF_OWNER)
        && !cut.contains(&own)
    {
        cut.push(own);
    }

    cut
}

/// The burn lists this repository adopted, each carrying its declared payload.
///
/// What a family counts is compiled meaning and where it counts it is not, so the
/// two meet here: the family names a shape and the reaches it is counted over,
/// and the corpus's own documents say where those reaches are and what to look
/// for inside them.
///
#[must_use]
pub fn index_burn_lists(declared: &Snapshot) -> Vec<BurnList> {
    let recognizer = Recognizer::declared(declared);

    BURN_LISTS
        .iter()
        .map(|declaration| {
            let surface =
                domain_of(declaration).and_then(|domain| declared.declared_surface(domain));

            let excluded = match surface {
                // A census that walks nothing cuts nothing out of it, so the
                // rules derive against a surface or not at all.
                Some(_) => derived_exclusions(declared, policy_of(declaration.family)),
                None => Vec::new(),
            };

            BurnList::new(
                declaration.family,
                declaration.shape.clone(),
                surface.map_or_else(Vec::new, |surface| surface.prose().to_vec()),
                surface.map_or_else(Vec::new, |surface| surface.code().to_vec()),
            )
            .with_excluded(excluded)
            .reading(recognizer.clone())
        })
        .collect()
}

/// The families a corpus has activated and given no surface to walk.
///
/// An activated census with no declared program surface is a finding rather than a quiet
/// zero. The reason is the reason a stale register row is a failure: a census that
/// reads nothing reports nothing, a ratchet compared against nothing passes, and
/// the corpus is then told its debt is discharged by a walk that never happened.
/// A snapshot that activates no policy is a different case and is not this one:
/// that corpus has asked for no census, and the silence is its own answer.
pub fn undeclared_surfaces(snapshot: &Snapshot) -> Vec<Finding> {
    BURN_LISTS
        .iter()
        .filter_map(|declaration| {
            let policy = policy_of(declaration.family)?;
            let domain = domain_of(declaration)?;

            activating_owner(snapshot, declaration.family)?;

            snapshot
                .declared_surface(domain)
                .is_none()
                .then(|| Finding::UndeclaredBurnSurface {
                    family: declaration.family.to_owned(),
                    policy: policy.to_owned(),
                    document: domain.to_owned(),
                })
        })
        .collect()
}

/// One occurrence of a family, wherever the census found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurnOccurrence {
    text: String,
    location: Location,
}

impl BurnOccurrence {
    /// The occurrence as written.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where the occurrence stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// One file's line of a register: how many occurrences it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BurnRow {
    /// The file, relative to the repository root.
    pub path: String,
    /// How many occurrences of the family stand in it.
    pub count: usize,
}

/// What one burn list's census found.
#[derive(Debug, Clone, Default)]
pub struct BurnCensus {
    occurrences: Vec<BurnOccurrence>,
    rows: Vec<BurnRow>,
    files_scanned: usize,
}

impl BurnCensus {
    /// Every occurrence, ordered by file and position.
    #[must_use]
    pub fn occurrences(&self) -> &[BurnOccurrence] {
        &self.occurrences
    }

    /// One row per file holding at least one occurrence, in path order.
    #[must_use]
    pub fn rows(&self) -> &[BurnRow] {
        &self.rows
    }

    /// How many files the census read.
    #[must_use]
    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }

    /// Whether this census found nothing at all to register.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// How many occurrences stand over every surface.
    #[must_use]
    pub fn total(&self) -> usize {
        self.rows.iter().map(|row| row.count).sum()
    }
}

/// One row of a committed register, and where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRow {
    path: String,
    count: usize,
    location: Location,
}

impl RegisterRow {
    /// Declare a ratchet row from the file it bounds and the ceiling it sets.
    ///
    /// The location is where a reader would go to move the ceiling, which under
    /// the ratified grammar is the canonical list rather than a generated view.
    #[must_use]
    pub fn new(path: impl Into<String>, count: usize, location: Location) -> Self {
        Self {
            path: path.into(),
            count,
            location,
        }
    }

    /// The file the row registers.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The count the row registers.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Where the row stands in the register.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// Take one burn list's census over a repository root.
#[must_use]
pub fn census(root: &Path, list: &BurnList, corpus: &CorpusPlan) -> (BurnCensus, Vec<Finding>) {
    let mut census = BurnCensus::default();
    let mut findings = Vec::new();

    for tree in list.prose() {
        for path in sources(corpus, tree, "md") {
            if list.is_excluded(&path) {
                continue;
            }

            let Some(text) = read(root, &path, &mut findings) else {
                continue;
            };

            census.files_scanned += 1;

            match list.shape() {
                Shape::Retired(family) => {
                    census
                        .occurrences
                        .extend(retired(&path, &text, *family, list.recognizer()));
                }
                Shape::ResidualLitter => census.occurrences.extend(residual(
                    &path,
                    &text,
                    list.recognizer(),
                    scan_residual_markdown,
                )),
                Shape::RepositoryRecordNumber => {
                    census.occurrences.extend(repository_record_numbers(
                        &path,
                        &text,
                        &markdown_regions(&text),
                    ));
                }
                Shape::Legacy(_) | Shape::UnlabelledTodo => {
                    census
                        .occurrences
                        .extend(occurrences(scan_legacy(&path, &text, list.rules())));
                }
                Shape::UnlabelledLegacy => {}
            }
        }
    }

    for tree in list.code() {
        for extension in list.shape().code_extensions() {
            for path in sources(corpus, tree, extension) {
                if list.is_excluded(&path) {
                    continue;
                }

                let Some(text) = read(root, &path, &mut findings) else {
                    continue;
                };

                census.files_scanned += 1;

                match list.shape() {
                    Shape::Legacy(rules) => census
                        .occurrences
                        .extend(occurrences(scan_comments(&path, &text, rules))),
                    Shape::RepositoryRecordNumber => {
                        census.occurrences.extend(repository_record_numbers(
                            &path,
                            &text,
                            &rust_regions(&text),
                        ));
                    }
                    Shape::UnlabelledTodo => {
                        census.occurrences.extend(unlabelled_todos(&path, &text));
                    }
                    Shape::UnlabelledLegacy => {
                        census.occurrences.extend(unlabelled_legacy(&path, &text));
                    }
                    Shape::Retired(family) => census.occurrences.extend(retired_comments(
                        &path,
                        &text,
                        *family,
                        list.recognizer(),
                    )),
                    Shape::ResidualLitter if *extension == "sh" => {
                        census.occurrences.extend(residual(
                            &path,
                            &text,
                            list.recognizer(),
                            scan_residual_script,
                        ));
                    }
                    Shape::ResidualLitter => census.occurrences.extend(residual(
                        &path,
                        &text,
                        list.recognizer(),
                        scan_residual_comments,
                    )),
                }
            }
        }
    }

    census.occurrences.sort_by(|left, right| {
        left.location
            .path()
            .cmp(right.location.path())
            .then_with(|| left.location.offset().cmp(&right.location.offset()))
    });
    census.rows = tally(&census.occurrences);

    (census, findings)
}

/// Read repository record numbers from one surface's referring regions.
///
/// The repository's record series remains canonical. This family observes its
/// number-citation shape without deciding whether the root corpus later retires
/// it, and reads that shape with the same digit-run bound as the Assayer's
/// record family. The regions decide where referring text stands, so code spans,
/// fenced examples and string literals remain displays or data rather than
/// citations.
fn repository_record_numbers(path: &Path, source: &str, regions: &[Region]) -> Vec<BurnOccurrence> {
    let mut found = Vec::new();

    for region in regions {
        for (offset, prefix) in region.text().match_indices("ADR-T-") {
            let digits = region.text()[offset + prefix.len()..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();

            if digits == 0 {
                continue;
            }

            let end = offset + prefix.len() + digits;

            found.push(BurnOccurrence {
                text: region.text()[offset..end].to_owned(),
                location: Location::new(path, source, region.source_offset(offset)),
            });
        }
    }

    found.sort_by_key(|occurrence| occurrence.location().offset());
    found
}

/// Read one Markdown document for a retired identity family.
fn retired(
    path: &Path,
    source: &str,
    family: RetiredFamily,
    declared: &Recognizer,
) -> Vec<BurnOccurrence> {
    scan_retired_markdown(source, family, declared)
        .into_iter()
        .map(|(text, offset)| BurnOccurrence {
            text,
            location: Location::new(path, source, offset),
        })
        .collect()
}

/// Read one Rust source's comments for a retired identity family.
///
/// Only the comments, for the reason the legacy census reads only comments: a
/// scenario number inside an identifier or a string literal is a name the program
/// carries rather than a reference the corpus makes, and a register counting those
/// could only reach zero by renaming the code.
///
/// The regions come from the tokenization layer, exactly as the legacy census
/// and the residual census take theirs. Reading one comment at a time was a
/// defect of the reading rather than a narrowness of the family: adjacent comment
/// lines are one region there, so a retired identity the corpus wrapped across a
/// line boundary is one occurrence here too, and a census that could not see the
/// corpus's own wrapped spelling would close with that debt still standing
/// (ADR-T-020, The migration disciplines). Offsets are mapped back
/// through the region that produced them, so an occurrence is still reported
/// where its first character really stands.
fn retired_comments(
    path: &Path,
    source: &str,
    family: RetiredFamily,
    declared: &Recognizer,
) -> Vec<BurnOccurrence> {
    let mut found: Vec<Retired> = rust_regions(source)
        .iter()
        .flat_map(|region| {
            scan_retired_text(0, region.text(), family, declared)
                .into_iter()
                .map(|(text, offset)| (text, region.source_offset(offset)))
        })
        .collect();

    found.sort_by_key(|(_text, offset)| *offset);

    found
        .into_iter()
        .map(|(text, offset)| BurnOccurrence {
            text,
            location: Location::new(path, source, offset),
        })
        .collect()
}

/// Read one source for the residual family, by whichever reader suits it.
///
/// The reader is handed in rather than chosen here because the three surfaces
/// the family declares — Markdown prose, Rust commentary and shell commentary —
/// differ only in how the running text is found, and not at all in what is
/// looked for once it is.
fn residual(
    path: &Path,
    source: &str,
    declared: &Recognizer,
    reader: fn(&str, &Recognizer) -> Vec<Residual>,
) -> Vec<BurnOccurrence> {
    reader(source, declared)
        .into_iter()
        .map(|(text, offset)| BurnOccurrence {
            text,
            location: Location::new(path, source, offset),
        })
        .collect()
}

/// Read one Rust source's comments for a family's shapes.
///
/// Only the comments are read. Program text may carry a shape — an identifier
/// holding a record's number, a string literal holding a section locator — and
/// neither is a reference made in the sense the migration retires; a burn list
/// that counted them would never reach zero, because reaching zero would mean
/// renaming the code.
///
/// Adjacent comment lines are one region, so a reference the retired convention
/// wrapped across a line boundary is one reference here too. Reading each
/// comment alone made a census that could not see the corpus's own wrapped
/// spelling, which is a defect of the reading rather than a narrowness of the
/// rules (ADR-T-020, The migration disciplines).
fn scan_comments(path: &Path, source: &str, rules: &[LegacyRule]) -> Vec<Finding> {
    scan_regions(path, source, &rust_regions(source), rules)
}

/// Read one Rust source for the covered notices that carry no label.
///
/// The census is the profile's own, so the register counts exactly what the
/// check would hold to the standard place once the notice is labelled. The text
/// recorded for a growth finding is the marker and the summary, which is what
/// sends a reader to the notice that broke the ratchet.
///
/// The package is not known here and is not needed: whether a notice carries a
/// label is a fact of its own line, and the classification that does need a
/// package decides only which area the label names.
fn unlabelled_todos(path: &Path, source: &str) -> Vec<BurnOccurrence> {
    let (notices, _orphans) = scan_todos("", path, source);

    notices
        .into_iter()
        .filter(TodoNotice::is_unlabelled)
        .map(|notice| BurnOccurrence {
            text: format!("{}: {}", notice.marker(), notice.summary()),
            location: notice.location().clone(),
        })
        .collect()
}

/// Read one production Rust source for marked legacy sites carrying no label.
fn unlabelled_legacy(path: &Path, source: &str) -> Vec<BurnOccurrence> {
    if !is_production_source(path) {
        return Vec::new();
    }

    let (sites, _orphans) = scan_legacy_sites("", path, source);

    sites
        .into_iter()
        .filter(LegacySite::is_unlabelled)
        .map(|site| BurnOccurrence {
            text: format!("LEGACY: {}", site.summary()),
            location: site.location().clone(),
        })
        .collect()
}

/// Reduce the lint's findings to the occurrences a census counts.
fn occurrences(findings: Vec<Finding>) -> Vec<BurnOccurrence> {
    findings
        .into_iter()
        .filter_map(|finding| match finding {
            Finding::LegacySectionReference { text, location }
            | Finding::LegacyTagReference { text, location }
            | Finding::LegacyRecordReference { text, location }
            | Finding::LegacyUnprefixedRecordReference { text, location } => {
                Some(BurnOccurrence { text, location })
            }
            _other => None,
        })
        .collect()
}

/// Count the occurrences of each file, in path order.
fn tally(occurrences: &[BurnOccurrence]) -> Vec<BurnRow> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for occurrence in occurrences {
        *counts
            .entry(occurrence.location.path().to_string_lossy().into_owned())
            .or_default() += 1;
    }

    counts
        .into_iter()
        .map(|(path, count)| BurnRow { path, count })
        .collect()
}

/// Read a file, reporting rather than raising what could not be read.
fn read(root: &Path, relative: &Path, findings: &mut Vec<Finding>) -> Option<String> {
    match fs::read_to_string(root.join(relative)) {
        Ok(text) => Some(text),
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: relative.to_string_lossy().into_owned(),
                message: error.to_string(),
            });

            None
        }
    }
}

/// Every source of one extension below a tree, in path order.
///
/// A tree that is not there contributes nothing and raises nothing: a surface may
/// name a target directory a package does not carry yet, and a census that
/// complained would make declaring the surface a commitment to create it.
fn sources(corpus: &CorpusPlan, tree: &Path, extension: &str) -> Vec<PathBuf> {
    // A declaration names the repository root as `.`, but register rows and
    // exclusions are root-relative paths without that spelling. Resolve the
    // root marker before descending so both are compared in one path space.
    let tree = tree.strip_prefix(".").unwrap_or(tree);

    corpus
        .native_paths()
        .iter()
        .filter(|path| path.starts_with(tree))
        .filter(|path| {
            path.extension()
                .is_some_and(|found| found.eq_ignore_ascii_case(extension))
        })
        .cloned()
        .collect()
}

/// Verify one burn list's ratchet against its census, in both directions.
///
/// The ratchet is the canonical list, which is what the retired registers left
/// behind them: their content was either an argument, which became a record, or a
/// ratchet, which became these rows (´conv:isolation:registers´). The relation is
/// the one the register-equals-census gate held, over the same numbers.
///
/// Growth is attributed positionally: counts alone cannot say which of a file's
/// occurrences is the new one, so the occurrence named is the first standing
/// beyond what the list accounts for. That is the right occurrence whenever the
/// addition was at or after it, and a nearby one otherwise; either way the reader
/// is sent to the file whose census grew, with the arithmetic beside it.
#[must_use]
pub fn verify(list: &BurnList, census: &BurnCensus, declared: &[RegisterRow]) -> Vec<Finding> {
    let mut findings = Vec::new();

    for row in census.rows() {
        let entry = declared.iter().find(|entry| entry.path == row.path);
        let count = entry.map_or(0, |entry| entry.count);

        if row.count > count {
            findings.push(growth(list, row, count, census));
        }
    }

    for entry in declared {
        let found = census
            .rows()
            .iter()
            .find(|row| row.path == entry.path)
            .map_or(0, |row| row.count);

        if found < entry.count {
            findings.push(Finding::StaleBurnEntry {
                family: list.family().to_owned(),
                path: entry.path.clone(),
                registered: entry.count,
                found,
                location: entry.location.clone(),
            });
        }
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    findings
}

/// The finding one file's growth raises, naming the occurrence beyond the list.
fn growth(list: &BurnList, row: &BurnRow, registered: usize, census: &BurnCensus) -> Finding {
    let first_new = census
        .occurrences()
        .iter()
        .filter(|occurrence| occurrence.location.path().to_string_lossy() == row.path)
        .nth(registered);

    Finding::BurnListGrowth {
        family: list.family().to_owned(),
        path: row.path.clone(),
        registered,
        found: row.count,
        text: first_new.map_or_else(String::new, |occurrence| occurrence.text.clone()),
        location: first_new.map(|occurrence| occurrence.location.clone()),
    }
}

/// The ratchet rows a snapshot declares for one family, gathered across its keys.
///
/// A family reaching one owner's share has one key, and a family declared over
/// the corpus root has one too — the root owner's, because the ratchet such a
/// census earns is one repository-wide artifact no member can repair alone
/// (ADR-T-019, The layer owner graph). The rows are gathered across every
/// key naming the family's program, so neither shape has to be told apart here.
#[must_use]
pub fn declared_rows(snapshot: &Snapshot, family: &str) -> Vec<RegisterRow> {
    let Some(policy) = CATALOG
        .iter()
        .find(|policy| matches!(policy.observer, Observer::BurnFamily(named) if named == family))
    else {
        return Vec::new();
    };

    let mut rows = Vec::new();

    for (pair, held) in snapshot.lists() {
        if snapshot.program(pair) != Some(policy.name) {
            continue;
        }

        if let Rows::PathCounts(counts) = held {
            rows.extend(counts.iter().map(|count| RegisterRow {
                path: count.path.display(),
                count: usize::try_from(count.maximum).unwrap_or(usize::MAX),
                location: Location::new(format!("{DIRECTORY}/{LISTS_FILE}"), "", 0),
            }));
        }
    }

    rows
}

/// Verify every adopted burn list against the ratchet the snapshot declares.
///
/// A declared surface is held to its ratchets entire, and a family the surface
/// never activates is held to the empty ratchet — which is how a family stays at
/// zero.
#[must_use]
#[cfg(test)]
pub fn verify_all(
    root: &Path,
    snapshot: &Snapshot,
    corpus: &CorpusPlan,
) -> (Vec<(BurnList, BurnCensus)>, Vec<Finding>) {
    let mut taken = Vec::new();
    let mut findings = undeclared_surfaces(snapshot);

    for list in index_burn_lists(snapshot) {
        let (census, census_findings) = census(root, &list, corpus);
        findings.extend(census_findings);

        findings.extend(verify(
            &list,
            &census,
            &declared_rows(snapshot, list.family()),
        ));

        taken.push((list, census));
    }

    (taken, findings)
}
