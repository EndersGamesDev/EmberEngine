// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The compiled programme catalogue.
//!
//! A declaration chooses which programme governs an owner and may provide named
//! entries that specialize that programme. This module is the one compiled join
//! between those facts: policy identity, list codec, observation route,
//! prerequisites, adopted set kinds, selection admission, and naked owner data.
//! It answers which compiled programme a declared set activates without making
//! an uncatalogued set a refusal.

use std::slice;

use serde::Serialize;

use crate::declaration::{Admission, SetKind};

/// The identity form a programme's tolerated violations are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    /// A violation is identified by a digest of its stable structured fields.
    Fingerprint,
    /// A violation is identified by its file, and every occurrence in that file counts.
    PathCount,
    /// A violation is identified by its file, which holds at most one occurrence.
    PathSet,
}

impl Codec {
    /// The codec's identifier, as a request and a response spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fingerprint => "fingerprint",
            Self::PathCount => "path-count",
            Self::PathSet => "path-set",
        }
    }

    /// The TOML field a table of this codec carries.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Fingerprint => "allowances",
            Self::PathCount => "path_counts",
            Self::PathSet => "paths",
        }
    }
}

/// Which owner a dependency edge wants its prerequisite of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// The prerequisite is wanted of the owner whose pair required it.
    SameOwner,
    /// The prerequisite is wanted of every registered other owner this owner cites into.
    CitedOwner,
    /// The prerequisite is wanted of the root owner.
    FixedOwner,
}

impl Scope {
    /// The scope's identifier, as the human message spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SameOwner => "same-owner",
            Self::CitedOwner => "cited-owner",
            Self::FixedOwner => "fixed-owner",
        }
    }
}

/// One immediate prerequisite of a programme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dependency {
    /// Which owner the prerequisite is wanted of.
    pub scope: Scope,
    /// The prerequisite programme's identifier.
    pub policy: &'static str,
}

/// Where a programme's observed violations are counted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observer {
    /// The two-pass label graph over one plan-provided ownership space.
    LabelGraph,
    /// The test-function census and its derived inventory.
    TestProfile,
    /// The to-do census and its derived inventory.
    TodoProfile,
    /// The marked legacy-implementation census and its derived inventory.
    LegacyProfile,
    /// Authored claims attached to covered tests.
    ClaimProfile,
    /// The production-constant census and its pins.
    ConstantProfile,
    /// The in-source test index projection.
    TestIndexes,
    /// The per-folder test matrix projection.
    TestMatrices,
    /// The generated constant-pin projection.
    ConstantPins,
    /// Declared publication assemblies and their generated targets.
    AssemblyPublications,
    /// The declared one-hop owner reach relation.
    OwnerReach,
    /// Workspace crate names reconciled with declared owners.
    OwnerRoster,
    /// Parameterized migration recognizers compiled into burn runs.
    ReferenceMigration,
    /// The occurrences are censused by the named burn family's recognizer.
    BurnFamily(&'static str),
    /// The files failing at least one governing SPDX half.
    SpdxHeaders,
    /// The governed files whose interchange envelope is absent or malformed.
    InterchangeEnvelopes,
    /// The path citations the finite recognizer reads out of governed sources.
    FilePathCitations,
}

/// One catalogue policy: its identifier, codec, observation route, and prerequisites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// The identifier a declaration activates the programme by.
    pub name: &'static str,
    /// The identity form its tolerated violations are written in.
    pub codec: Codec,
    /// Where its observed violations are counted from.
    pub observer: Observer,
    /// Its immediate prerequisites.
    pub dependencies: &'static [Dependency],
}

/// One adopted declared kind and the selection word its sections use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredKind {
    name: &'static str,
    kind: SetKind,
    admission: Admission,
}

impl DeclaredKind {
    /// The declared set type's spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// The decoder's typed representation of the set.
    #[must_use]
    pub const fn kind(self) -> SetKind {
        self.kind
    }

    /// The selection word a section over this kind admits rows under.
    #[must_use]
    pub const fn admission(self) -> Admission {
        self.admission
    }
}

/// One compiled programme and the declaration shapes that specialize it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompiledProgram {
    policy: Policy,
    sets: &'static [DeclaredKind],
    datum: bool,
}

impl CompiledProgram {
    /// The programme's policy identity and execution metadata.
    #[must_use]
    pub const fn policy(&self) -> &Policy {
        &self.policy
    }

    /// The declared set kinds whose named entries this programme reads.
    #[must_use]
    pub const fn sets(&self) -> &'static [DeclaredKind] {
        self.sets
    }

    /// Whether the programme also reads data on a naked owner table.
    #[must_use]
    pub const fn reads_datum(&self) -> bool {
        self.datum
    }
}

/// The one ordered catalogue of compiled programmes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramCatalog {
    programmes: &'static [CompiledProgram],
}

impl ProgramCatalog {
    /// Construct a catalogue from its compiled rows.
    const fn new(programmes: &'static [CompiledProgram]) -> Self {
        Self { programmes }
    }

    /// The number of compiled programmes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.programmes.len()
    }

    /// Whether the binary compiles no programme.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.programmes.is_empty()
    }

    /// The programme metadata in established policy order.
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'static Policy> {
        self.programmes.iter().map(CompiledProgram::policy)
    }

    /// The compiled programme of that identifier.
    #[must_use]
    pub fn program(self, name: &str) -> Option<&'static CompiledProgram> {
        self.programmes
            .iter()
            .find(|program| program.policy.name == name)
    }

    /// The compiled programme that reads a declared set type.
    #[must_use]
    pub fn program_for_set(self, set: &str) -> Option<&'static CompiledProgram> {
        self.programmes
            .iter()
            .find(|program| program.sets.iter().any(|declared| declared.name == set))
    }

    /// The adopted kind of a declared set type.
    #[must_use]
    pub fn set_kind(self, set: &str) -> SetKind {
        self.program_for_set(set)
            .and_then(|program| program.sets.iter().find(|declared| declared.name == set))
            .map_or(SetKind::Unadopted, |declared| declared.kind)
    }

    /// The selection admission of an adopted declared kind.
    #[must_use]
    pub fn admission(self, kind: SetKind) -> Option<Admission> {
        self.programmes
            .iter()
            .flat_map(|program| program.sets)
            .find(|declared| declared.kind == kind)
            .map(|declared| declared.admission)
    }

    /// Every adopted declared kind, in programme and then declaration order.
    pub fn declared_kinds(self) -> impl Iterator<Item = &'static DeclaredKind> {
        self.programmes.iter().flat_map(|program| program.sets)
    }
}

impl IntoIterator for ProgramCatalog {
    type Item = &'static Policy;
    type IntoIter = std::iter::Map<
        slice::Iter<'static, CompiledProgram>,
        fn(&'static CompiledProgram) -> &'static Policy,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.programmes.iter().map(CompiledProgram::policy)
    }
}

/// A same-owner edge on the named policy.
const fn same(policy: &'static str) -> Dependency {
    Dependency {
        scope: Scope::SameOwner,
        policy,
    }
}

/// A cited-owner edge on the named policy.
const fn cited(policy: &'static str) -> Dependency {
    Dependency {
        scope: Scope::CitedOwner,
        policy,
    }
}

/// A fixed-owner edge on the named policy, which is always the root owner's pair.
const fn fixed(policy: &'static str) -> Dependency {
    Dependency {
        scope: Scope::FixedOwner,
        policy,
    }
}

/// A programme that reads no declared set or naked owner datum.
const fn programme(
    name: &'static str,
    codec: Codec,
    observer: Observer,
    dependencies: &'static [Dependency],
) -> CompiledProgram {
    CompiledProgram {
        policy: Policy {
            name,
            codec,
            observer,
            dependencies,
        },
        sets: &[],
        datum: false,
    }
}

/// A programme specialized by declared set entries or naked owner data.
const fn declared_programme(
    name: &'static str,
    codec: Codec,
    observer: Observer,
    dependencies: &'static [Dependency],
    sets: &'static [DeclaredKind],
    datum: bool,
) -> CompiledProgram {
    CompiledProgram {
        policy: Policy {
            name,
            codec,
            observer,
            dependencies,
        },
        sets,
        datum,
    }
}

/// One adopted declared kind.
const fn set(name: &'static str, kind: SetKind, admission: Admission) -> DeclaredKind {
    DeclaredKind {
        name,
        kind,
        admission,
    }
}

/// The whole vocabulary a declaration may activate.
///
/// The entries keep the live policy catalogue's established order. Declaration
/// bindings sit on their programme rather than in a second table, so lookup by
/// policy and lookup by set kind meet at the same compiled row.
pub const CATALOG: ProgramCatalog = ProgramCatalog::new(&[
    programme(
        "labels.mints-well-formed",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[],
    ),
    programme(
        "labels.mints-kind-conform",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "labels.mints-unique",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "labels.heads-conform",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.mints-kind-conform")],
    ),
    programme(
        "labels.citations-local-resolve",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.mints-unique")],
    ),
    programme(
        "labels.citations-imported-resolve",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[
            same("labels.citations-import-form"),
            cited("labels.mints-unique"),
        ],
    ),
    programme(
        "labels.citations-import-form",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "labels.citations-layer-conform",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[
            same("labels.citations-imported-resolve"),
            fixed("owners.reach-conform"),
        ],
    ),
    programme(
        "labels.generated-regions-conform",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[
            same("labels.citations-local-resolve"),
            same("labels.citations-imported-resolve"),
        ],
    ),
    programme(
        "labels.outlines-conform",
        Codec::Fingerprint,
        Observer::LabelGraph,
        &[same("labels.heads-conform")],
    ),
    programme(
        "profile.tests-conform",
        Codec::Fingerprint,
        Observer::TestProfile,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "profile.todos-conform",
        Codec::Fingerprint,
        Observer::TodoProfile,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "profile.legacy-conform",
        Codec::Fingerprint,
        Observer::LegacyProfile,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "profile.claims-conform",
        Codec::Fingerprint,
        Observer::ClaimProfile,
        &[
            same("labels.mints-well-formed"),
            same("profile.tests-conform"),
        ],
    ),
    programme(
        "profile.constants-conform",
        Codec::Fingerprint,
        Observer::ConstantProfile,
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "projection.test-indexes-current",
        Codec::Fingerprint,
        Observer::TestIndexes,
        &[same("profile.tests-conform")],
    ),
    programme(
        "projection.test-matrices-current",
        Codec::Fingerprint,
        Observer::TestMatrices,
        &[same("profile.tests-conform")],
    ),
    programme(
        "projection.constant-pins-current",
        Codec::Fingerprint,
        Observer::ConstantPins,
        &[same("profile.constants-conform")],
    ),
    programme(
        "assembly.assayer-spec-current",
        Codec::Fingerprint,
        Observer::AssemblyPublications,
        &[],
    ),
    programme(
        "owners.reach-conform",
        Codec::Fingerprint,
        Observer::OwnerReach,
        &[],
    ),
    declared_programme(
        "spdx.headers-conform",
        Codec::PathSet,
        Observer::SpdxHeaders,
        &[],
        &[
            set("identifier", SetKind::Identifier, Admission::Partition),
            set("copyright", SetKind::Copyright, Admission::Partition),
        ],
        false,
    ),
    programme(
        "interchange.envelope-conform",
        Codec::PathSet,
        Observer::InterchangeEnvelopes,
        &[],
    ),
    programme(
        "references.file-paths-absent",
        Codec::PathCount,
        Observer::FilePathCitations,
        &[],
    ),
    programme(
        "legacy.section-references",
        Codec::PathCount,
        Observer::BurnFamily("section-sign references"),
        &[],
    ),
    programme(
        "legacy.record-references",
        Codec::PathCount,
        Observer::BurnFamily("retired record numbers"),
        &[],
    ),
    programme(
        "legacy.section-references-repository",
        Codec::PathCount,
        Observer::BurnFamily("section-sign references (repository)"),
        &[],
    ),
    programme(
        "legacy.record-references-repository",
        Codec::PathCount,
        Observer::BurnFamily("retired record numbers (repository)"),
        &[],
    ),
    programme(
        "legacy.unprefixed-record-references",
        Codec::PathCount,
        Observer::BurnFamily("ambiguous unprefixed record numbers"),
        &[],
    ),
    programme(
        "legacy.tag-references",
        Codec::PathCount,
        Observer::BurnFamily("superseded tag forms"),
        &[],
    ),
    programme(
        "legacy.scenario-numbers",
        Codec::PathCount,
        Observer::BurnFamily("retired scenario numbers"),
        &[],
    ),
    programme(
        "legacy.division-names",
        Codec::PathCount,
        Observer::BurnFamily("retired division names"),
        &[],
    ),
    programme(
        "legacy.residual-litter",
        Codec::PathCount,
        Observer::BurnFamily("residual litter"),
        &[],
    ),
    programme(
        "legacy.todos",
        Codec::PathCount,
        Observer::BurnFamily("unlabelled to-do notices"),
        &[same("labels.mints-well-formed")],
    ),
    programme(
        "legacy.implementation",
        Codec::PathCount,
        Observer::BurnFamily("unlabelled legacy implementations"),
        &[same("labels.mints-well-formed")],
    ),
    declared_programme(
        "owners.crate-names-conform",
        Codec::Fingerprint,
        Observer::OwnerRoster,
        &[],
        &[
            set("name-key", SetKind::NameKey, Admission::Deployment),
            set(
                "name-prefix-ignore",
                SetKind::NamePrefixIgnore,
                Admission::Deployment,
            ),
        ],
        true,
    ),
    declared_programme(
        "assembly.publications-current",
        Codec::Fingerprint,
        Observer::AssemblyPublications,
        &[],
        &[],
        true,
    ),
    declared_programme(
        "references.mark-numbered-absent",
        Codec::PathCount,
        Observer::ReferenceMigration,
        &[],
        &[set(
            "numbered-marks",
            SetKind::NumberedMarks,
            Admission::Deployment,
        )],
        false,
    ),
    declared_programme(
        "references.literal-set-absent",
        Codec::PathCount,
        Observer::ReferenceMigration,
        &[],
        &[set("literals", SetKind::Literals, Admission::Deployment)],
        false,
    ),
    declared_programme(
        "references.prefix-numbers-absent",
        Codec::PathCount,
        Observer::ReferenceMigration,
        &[],
        &[set(
            "prefix-numbers",
            SetKind::PrefixNumbers,
            Admission::Deployment,
        )],
        false,
    ),
]);

/// The catalogued policy of that identifier.
#[must_use]
pub fn catalogued(name: &str) -> Option<&'static Policy> {
    CATALOG.program(name).map(CompiledProgram::policy)
}

/// The programme identifier that reads a declared set type.
#[must_use]
pub fn program_of(set: &str) -> Option<&'static str> {
    CATALOG
        .program_for_set(set)
        .map(|program| program.policy.name)
}

/// Whether a catalogued policy censuses the corpus rather than an owner's share.
#[must_use]
pub fn spans_corpus(name: &str) -> bool {
    match catalogued(name).map(|policy| policy.observer) {
        Some(Observer::BurnFamily(family)) => crate::burn::family_spans_corpus(family),
        _ => false,
    }
}

/// Whether text is a well-formed policy identifier.
#[must_use]
pub fn is_policy_identifier(text: &str) -> bool {
    let mut components = text.split('.');
    let mut count = 0;

    for component in components.by_ref() {
        if !is_component(component) {
            return false;
        }

        count += 1;
    }

    count >= 2
}

/// Whether text is a well-formed declared name.
#[must_use]
pub fn is_declared_name(text: &str) -> bool {
    is_component(text)
}

/// Whether one dot-separated component is well-formed.
fn is_component(component: &str) -> bool {
    if component.is_empty() {
        return false;
    }

    let mut previous_hyphen = true;

    for byte in component.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => previous_hyphen = false,
            b'-' if !previous_hyphen => previous_hyphen = true,
            _ => return false,
        }
    }

    !previous_hyphen
}
