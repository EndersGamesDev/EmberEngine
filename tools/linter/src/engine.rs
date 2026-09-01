// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The two-pass engine and the reference graph.
//!
//! The two-pass invariant of ADR-T-014, A calculus of documentation and source labels, stages
//! derivation. The adoption data are loaded first; then every carrier source is
//! harvested and the minting registries of all owners are completed, duplicates
//! failing by the unique-mint invariant (ADR-T-014, A calculus of documentation and source labels);
//! only then is any resolution judgment derived, against the completed
//! registries. That staging is what buys the order-independence meta-theorem
//! (ADR-T-014, A calculus of documentation and source labels), so this module keeps the two
//! passes strictly separate rather than resolving as it scans.
//!
//! The total-resolution invariant (ADR-T-014, A calculus of documentation and source labels) makes
//! resolution total: a parenthesised span whose interior is label-shaped but
//! resolves nowhere fails and never lapses into text, while a span that parses
//! as no form was already text before the engine saw it.
//!
//! The graph is petgraph's directed graph used directly, with mints for nodes
//! and resolved citations for edges. An edge runs from the mint of the
//! environment a citation stands in to the mint the citation resolves to. Wave
//! L1 reads the containing environment as the nearest preceding mint in the same
//! source, which is exactly right for documents whose every head mints; a later
//! wave computes environment extents properly and can then also attach the
//! citations that precede their source's first mint.
//!
//! # The code surface enters before pass one
//!
//! Every mint standing in code — a test label or a notice label a profile
//! derived, a claim an author wrote where the test is — is a mint like any
//! other, owned by the owner its package names and citable by the ordinary
//! forms. They enter here rather than in their profiles because resolution is
//! this module's business: the seeding completes before the harvest, so the
//! staging the two-pass invariant fixes is untouched and the order the corpus is
//! traversed in still decides nothing.
//!
//! Three properties follow from putting them in early, and are worth stating
//! where a reader will look for them. A citation of a renamed or deleted asset
//! fails as an unresolved citation, which is the whole point of citing an
//! inventory rather than remembering it. Nothing can be authored into a reserved
//! space by hand, because a bare occurrence of a reserved kind is refused before
//! the registry is ever consulted — the derivation-warrant rule survives seeding
//! exactly as it stood. And a mint standing in code is not a mint of the prose:
//! the count this module reports, and the listings the graph report makes, are
//! of the document surface, so admitting an inventory does not restate it as
//! prose.
//!
//! # One space, whatever the kind and whatever the surface
//!
//! The owner's ruling of 2026-08-20 settles what the two carriers come to: a
//! label is citable from either surface, and it makes no difference what kind it
//! is or which surface minted it. So the citations the code carrier harvests
//! join the ones the prose carrier harvests, in one held list resolved against
//! one set of registries. There is no per-kind clause anywhere below, and that
//! is the point — a rule that named kinds would have to be revised by every
//! profile added after it.
//!
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use petgraph::graph::{DiGraph, NodeIndex};
use serde::Serialize;

use crate::adoption::{Adoption, Owner};
use crate::carrier::Source;
use crate::code::CodeSurface;
use crate::finding::{Finding, Location};
use crate::head::{Head, read_heads, validate_heads};
use crate::label::{Label, Prefix};
use crate::matrix::{README, committed_matrices};
use crate::occurrence::{Form, Occurrence, Syntax};
use crate::outline::{Outline, read_outline, validate_tracking};
use crate::profile::ACUTE;
use crate::prose::{ProseBlock, scan_markdown};

/// The reference graph: mints for nodes, resolved citations for edges.
pub type ReferenceGraph = DiGraph<MintNode, CitationEdge>;

/// Which surface a mint stands on.
///
/// This is independent of the warrant a mint stands on, and the claim profile is
/// the case that shows why both are needed: a claim is authored by a person, so
/// its kind is unreserved, and it stands in code all the same. Counting the
/// corpus a reviewer reads is a question about the surface; asking whether a
/// hand may write a kind is a question about the warrant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    /// The mint stands at a head of a participating document.
    Document,
    /// The mint stands in a Rust source: at an asset, or at a standard place.
    Code,
}

/// A node of the reference graph: one mint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MintNode {
    /// The owner whose registry holds this mint.
    pub owner: Owner,
    /// The minted label.
    pub label: Label,
    /// Where the minting occurrence stands.
    ///
    /// For a derived mint this is the asset itself — the file and line of the
    /// covered test — so a reverse lookup of the label lands where the thing is
    /// rather than where a document mentions it.
    pub location: Location,
    /// Whether a profile's derivation warrants this mint rather than an author.
    ///
    /// The distinction is not decorative: it is what the reserved kinds are
    /// about, and a hand may not write a kind a derivation fills.
    pub derived: bool,
    /// Which surface the mint stands on.
    ///
    /// This is what the carrier's mint count and the graph report's listings are
    /// of. A mint standing in code resolves citations like any other and is no
    /// part of the corpus a reviewer is reading, or an inventory of thousands
    /// would bury the prose it serves.
    pub surface: Surface,
}

/// An edge of the reference graph: one resolved citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CitationEdge {
    /// The citation form that resolved.
    pub form: Form,
    /// Where the citation stands.
    pub location: Location,
}

/// One participating imported citation, held for the layer pass.
///
/// The layer law of ADR-T-019 judges admissibility rather than resolution, so
/// it needs the imports this engine already recognized rather than a second
/// recognizer of its own (ADR-T-020, The migration disciplines). What
/// it needs of each is the citing corpus, the cited prefix, the surface, and
/// the location — never the cited label, whose kind, area and mint site are the
/// cited corpus's business (ADR-T-019, The layer owner graph).
///
/// Only participating occurrences are collected, which is what makes the law
/// blind to a displayed import (ADR-T-019, The layer owner graph).
/// Generated imports participate in full: their derivation exclusion only
/// keeps a register row from feeding the set it presents
/// (ADR-T-014, A calculus of documentation and source labels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSite {
    citing: Owner,
    prefix: Prefix,
    syntax: Syntax,
    location: Location,
}

impl ImportSite {
    /// The corpus the import stands in.
    #[must_use]
    pub const fn citing(&self) -> &Owner {
        &self.citing
    }

    /// The prefix the import named.
    #[must_use]
    pub const fn prefix(&self) -> &Prefix {
        &self.prefix
    }

    /// The concrete syntax the import was written in.
    #[must_use]
    pub const fn syntax(&self) -> Syntax {
        self.syntax
    }

    /// Where the import stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// The byte ranges of a Markdown source's generated regions.
///
/// The recogniser is the projection's own — a folder matrix is found by its
/// labelled head — so the region the engine treats as generated is exactly the
/// region the projection owns, and no second recognition can drift from the
/// first. The head itself stays authored: it stands on the boundary line above
/// the region, and its mint is the author's attestation, not the generator's.
///
/// Only a folder readme carries the projection: the same head-and-region shape
/// standing in any other document — a policy record's worked example, say — is
/// authored prose about the projection, and its occurrences participate as
/// authored ones.
fn generated_regions(path: &Path, text: &str) -> Vec<(usize, usize)> {
    if path.file_name().is_none_or(|name| name != README) {
        return Vec::new();
    }

    let mut starts = vec![0_usize];
    starts.extend(
        text.bytes()
            .enumerate()
            .filter(|(_index, byte)| *byte == b'\n')
            .map(|(index, _byte)| index + 1),
    );

    committed_matrices(text)
        .iter()
        .filter_map(|matrix| {
            let start = *starts.get(matrix.head_line + 1)?;
            let end = starts
                .get(matrix.past_last_line)
                .copied()
                .unwrap_or(text.len());

            Some((start, end))
        })
        .collect()
}

/// Whether an offset falls inside any of the ranges.
fn falls_in(regions: &[(usize, usize)], offset: usize) -> bool {
    regions
        .iter()
        .any(|(start, end)| offset >= *start && offset < *end)
}

/// Render the import form an unresolved citation's repair would take, in the
/// concrete syntax of the surface the citation stands on.
///
/// The suggestion is the exact text to write, delimiters included: a rendering
/// without them would teach the one form the grammar reads as nothing.
fn render_import(syntax: Syntax, prefix: &Prefix, label: &Label) -> String {
    match syntax {
        Syntax::Prose => format!("(`[{prefix}-{label}]`)"),
        Syntax::Code => format!("({ACUTE}[{prefix}-{label}]{ACUTE})"),
    }
}

/// The result of checking a carrier.
#[derive(Debug)]
pub struct Analysis {
    graph: ReferenceGraph,
    findings: Vec<Finding>,
    imports: Vec<ImportSite>,
    sources_scanned: usize,
    citations_resolved: usize,
    heads_validated: usize,
    derived_mints: usize,
    document_mints: usize,
}

impl Analysis {
    /// The reference graph over the whole carrier.
    #[must_use]
    pub const fn graph(&self) -> &ReferenceGraph {
        &self.graph
    }

    /// Every finding raised, ordered by source and position.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Every participating imported citation the harvest met.
    ///
    /// These are the occurrences the layer law of ADR-T-019 ranges over. They
    /// are reported here rather than judged here, because whether an import is
    /// admissible is a question about the workspace's manifests and not about
    /// the reference graph.
    #[must_use]
    pub fn imports(&self) -> &[ImportSite] {
        &self.imports
    }

    /// How many carrier sources were scanned.
    #[must_use]
    pub const fn sources_scanned(&self) -> usize {
        self.sources_scanned
    }

    /// How many mints stand at heads of the carrier's documents.
    ///
    /// The code surface is not carrier prose: those mints stand where the code
    /// is, and are counted on their own below. Reading them into this number
    /// would restate a census as prose and move a figure the corpus is held to
    /// whenever a test is written.
    #[must_use]
    pub const fn mints(&self) -> usize {
        self.document_mints
    }

    /// How many mints a profile's derivation seeded into the registries.
    #[must_use]
    pub const fn derived_mints(&self) -> usize {
        self.derived_mints
    }

    /// How many mints stand on the code surface, derived and authored together.
    #[must_use]
    pub fn code_mints(&self) -> usize {
        self.graph.node_count() - self.document_mints
    }

    /// How many citations resolved to a mint.
    #[must_use]
    pub const fn citations_resolved(&self) -> usize {
        self.citations_resolved
    }

    /// How many environment heads were paired with their mint and validated.
    #[must_use]
    pub const fn heads_validated(&self) -> usize {
        self.heads_validated
    }

    /// Whether the carrier is in good standing.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        !self.findings.iter().any(Finding::is_failure)
    }
}

/// Check a carrier and a code surface against adoption data, as one space.
///
/// The code surface carries what the censuses of the Rust sources found: every
/// mint standing in code and every citation standing in commentary. A caller
/// with none passes an empty one, and a carrier of prose alone is then checked
/// exactly as it was before any profile existed.
#[must_use]
pub fn analyze(adoption: &Adoption, sources: &[Source], code: &CodeSurface) -> Analysis {
    let mut engine = Engine::new(adoption);

    engine.seed(code);

    let scanned = engine.harvest(sources);

    engine.hold_code_citations(code);
    engine.resolve();
    engine.track();
    engine.finish(scanned)
}

/// A citation held between the two passes.
struct HeldCitation {
    owner: Owner,
    source: PathBuf,
    occurrence: Occurrence,
    /// Whether the citation stands in a generated region.
    ///
    /// Resolution is identical either way; what the flag decides is whom a
    /// failure blames. An authored citation that dangles is the author's, and
    /// the report may suggest a repair to write. A generated one that dangles
    /// is staleness of the register or a defect of its generator — the
    /// generated-compliance invariant
    /// (ADR-T-014, A calculus of documentation and source labels) — surfaced beside the
    /// exactness check rather than as an author's slip.
    generated: bool,
}

struct Engine<'a> {
    adoption: &'a Adoption,
    graph: ReferenceGraph,
    registries: BTreeMap<Owner, BTreeMap<Label, NodeIndex>>,
    mints_by_source: BTreeMap<PathBuf, Vec<(usize, NodeIndex)>>,
    citations: Vec<HeldCitation>,
    imports: Vec<ImportSite>,
    findings: Vec<Finding>,
    citations_resolved: usize,
    heads: usize,
    heads_by_source: BTreeMap<PathBuf, Vec<Head>>,
    minted_by_source: BTreeMap<PathBuf, BTreeSet<Label>>,
    outlines: Vec<Outline>,
    derived_mints: usize,
    document_mints: usize,
}

impl<'a> Engine<'a> {
    fn new(adoption: &'a Adoption) -> Self {
        Self {
            adoption,
            graph: ReferenceGraph::new(),
            registries: BTreeMap::new(),
            mints_by_source: BTreeMap::new(),
            citations: Vec::new(),
            imports: Vec::new(),
            findings: Vec::new(),
            citations_resolved: 0,
            heads: 0,
            heads_by_source: BTreeMap::new(),
            minted_by_source: BTreeMap::new(),
            outlines: Vec::new(),
            derived_mints: 0,
            document_mints: 0,
        }
    }

    /// Pass zero: enter every mint standing in code into its owner's registry.
    ///
    /// The owner is read from the mint's path through the same partition the
    /// carrier's prose is read through, which is why a package's tests and a
    /// package's documents land in one registry and the root package's tests
    /// land in the repository's: the partition already agrees with itself, and
    /// asking it once rather than re-deriving the rule here keeps it that way.
    ///
    /// A label already seeded is passed over rather than reported. Uniqueness of
    /// a code-side mint is the owning profile's own judgment and each already
    /// raises a finding for every collision it finds — a colliding derivation
    /// for the derived kinds, a duplicate claim mint for the authored one.
    /// Failing here as well would report one defect twice, under a code that
    /// means an author wrote a name twice in the prose.
    fn seed(&mut self, code: &CodeSurface) {
        for mint in code.mints() {
            let owner = self.adoption.owner_of(mint.path()).clone();
            let label = mint.label().clone();

            if self
                .registries
                .get(&owner)
                .is_some_and(|registry| registry.contains_key(&label))
            {
                continue;
            }

            let node = self.graph.add_node(MintNode {
                owner: owner.clone(),
                label: label.clone(),
                location: mint.location().clone(),
                derived: mint.is_derived(),
                surface: Surface::Code,
            });

            self.registries
                .entry(owner)
                .or_default()
                .insert(label, node);

            if mint.is_derived() {
                self.derived_mints += 1;
            }
        }
    }

    /// Hold every citation the commentary stands, for the resolution pass.
    ///
    /// They join the prose carrier's citations in one list rather than being
    /// resolved apart, because there is one resolution space and a citation's
    /// surface decides nothing about what it may reach.
    fn hold_code_citations(&mut self, code: &CodeSurface) {
        for citation in code.citations() {
            self.citations.push(HeldCitation {
                owner: self.adoption.owner_of(citation.path()).clone(),
                source: citation.path().to_path_buf(),
                occurrence: citation.occurrence().clone(),
                generated: citation.is_generated(),
            });
        }
    }

    /// Pass one: complete every owner's minting registry.
    ///
    /// Returns how many sources were scanned: a carried source that participates
    /// in nothing is read past rather than counted.
    fn harvest(&mut self, sources: &[Source]) -> usize {
        let mut scanned = 0;

        for source in sources {
            if !self.adoption.participates(source.path()) {
                continue;
            }

            scanned += 1;

            let owner = self.adoption.owner_of(source.path()).clone();
            let (occurrences, blocks, findings) =
                scan_markdown(source.path(), source.text()).into_parts();

            self.findings.extend(findings);

            let (outline, outline_findings) = read_outline(source.path(), source.text());

            self.findings.extend(outline_findings);

            if !outline.is_empty() {
                self.outlines.push(outline);
            }

            let generated = generated_regions(source.path(), source.text());

            // The head discipline reads authored occurrences only: a span in a
            // generated region heads nothing and pairs with nothing, and its
            // defects are reported under the generated-compliance rules below —
            // one defect, one report.
            let authored: Vec<Occurrence> = occurrences
                .iter()
                .filter(|occurrence| !falls_in(&generated, occurrence.location().offset()))
                .cloned()
                .collect();

            let heads = self.discipline(source, &blocks, &authored);

            self.heads_by_source
                .insert(source.path().to_path_buf(), heads);
            self.minted_by_source.insert(
                source.path().to_path_buf(),
                authored
                    .iter()
                    .filter(|occurrence| occurrence.is_mint())
                    .map(|occurrence| occurrence.label().clone())
                    .collect(),
            );

            for occurrence in occurrences {
                let in_generated = falls_in(&generated, occurrence.location().offset());

                if occurrence.is_mint() {
                    if in_generated {
                        // A generated occurrence is a mint only where a profile
                        // sets its standard place in the register, and no
                        // adopted profile does: the generator minted without
                        // warrant, and nothing enters the registry for it.
                        self.findings.push(Finding::BareGeneratedOccurrence {
                            label: occurrence.label().clone(),
                            location: occurrence.location().clone(),
                        });
                    } else {
                        self.mint(&owner, source.path(), &occurrence);
                    }
                } else {
                    self.citations.push(HeldCitation {
                        owner: owner.clone(),
                        source: source.path().to_path_buf(),
                        occurrence,
                        generated: in_generated,
                    });
                }
            }
        }

        scanned
    }

    /// Hold one source to the head discipline, when the source practises it.
    ///
    /// A document that mints nothing is not writing under the calculus yet: its
    /// headings head divisions of a document the migration has not reached, and
    /// asking them for mints would report the migration's backlog as the
    /// document's defects. A document that mints at all has begun, and every head
    /// of it is then answerable — which is the discipline in one sentence, and the
    /// reason the campaign's rule is tools over care.
    ///
    /// The authored heads are returned as well as counted: outline tracking asks
    /// which heads a document carries, and the answer is exactly this one.
    fn discipline(
        &mut self,
        source: &Source,
        blocks: &[ProseBlock],
        occurrences: &[Occurrence],
    ) -> Vec<Head> {
        if !occurrences.iter().any(Occurrence::is_mint) {
            return Vec::new();
        }

        let (heads, findings) = read_heads(source.path(), source.text(), blocks, occurrences);

        // A reserved-kind token at a head is no authored head: the invariant
        // named inv:labels:warrant-totality already fails it as an occurrence
        // awaiting its derivation, and asking the registry to classify it as
        // well would report one defect twice.
        let authored: Vec<Head> = heads
            .into_iter()
            .filter(|head| !self.adoption.is_reserved_kind(head.label().kind()))
            .collect();

        self.findings.extend(findings);
        self.findings
            .extend(validate_heads(self.adoption.registry(), &authored));
        self.heads += authored.len();

        authored
    }

    /// Admit one bare occurrence into an owner's registry, or fail it.
    fn mint(&mut self, owner: &Owner, source: &Path, occurrence: &Occurrence) {
        let label = occurrence.label();

        if self.adoption.is_reserved_kind(label.kind()) {
            self.findings.push(Finding::UnwarrantedReservedKind {
                label: label.clone(),
                location: occurrence.location().clone(),
            });
            return;
        }

        if let Some(existing) = self
            .registries
            .get(owner)
            .and_then(|registry| registry.get(label))
        {
            let first = self.graph[*existing].location.clone();

            self.findings.push(Finding::DuplicateMint {
                label: label.clone(),
                owner: owner.as_str().to_owned(),
                first,
                second: occurrence.location().clone(),
            });
            return;
        }

        let node = self.graph.add_node(MintNode {
            owner: owner.clone(),
            label: label.clone(),
            location: occurrence.location().clone(),
            derived: false,
            surface: Surface::Document,
        });

        self.document_mints += 1;

        self.registries
            .entry(owner.clone())
            .or_default()
            .insert(label.clone(), node);

        self.mints_by_source
            .entry(source.to_path_buf())
            .or_default()
            .push((occurrence.location().offset(), node));
    }

    /// Pass two: resolve every citation against the completed registries.
    fn resolve(&mut self) {
        for held in std::mem::take(&mut self.citations) {
            match held.occurrence.form() {
                Form::Mint => {}
                Form::SameOwnerCitation => self.resolve_same_owner(&held),
                Form::ImportedCitation { prefix } => {
                    let prefix = prefix.clone();

                    self.imports.push(ImportSite {
                        citing: held.owner.clone(),
                        prefix: prefix.clone(),
                        syntax: held.occurrence.syntax(),
                        location: held.occurrence.location().clone(),
                    });

                    self.resolve_import(&held, &prefix);
                }
            }
        }
    }

    fn resolve_same_owner(&mut self, held: &HeldCitation) {
        let label = held.occurrence.label();

        if let Some(target) = self.lookup(&held.owner, label) {
            self.connect(held, target);
            return;
        }

        if held.generated {
            // A dangling generated citation is never the author's slip: the
            // register is stale or its generator wrote a citation of nothing,
            // so the report sends the reader to the regeneration rather than
            // suggesting an edit inside a region no hand may write.
            self.findings.push(Finding::DanglingGeneratedCitation {
                label: label.clone(),
                location: held.occurrence.location().clone(),
            });
            return;
        }

        let elsewhere = self
            .registries
            .iter()
            .find(|(owner, registry)| **owner != held.owner && registry.contains_key(label))
            .map(|(owner, _registry)| owner.clone());

        let finding = elsewhere.map_or_else(
            || Finding::UnresolvedCitation {
                label: label.clone(),
                location: held.occurrence.location().clone(),
            },
            |minting_owner| {
                let suggestion = self.adoption.prefix_of_owner(&minting_owner).map_or_else(
                    || format!("an imported citation of owner {minting_owner}, which registers no prefix"),
                    |prefix| render_import(held.occurrence.syntax(), prefix, label),
                );

                Finding::UnresolvedCitationWantingImport {
                    label: label.clone(),
                    location: held.occurrence.location().clone(),
                    minting_owner: minting_owner.as_str().to_owned(),
                    suggestion,
                }
            },
        );

        self.findings.push(finding);
    }

    fn resolve_import(&mut self, held: &HeldCitation, prefix: &Prefix) {
        let label = held.occurrence.label();
        let location = held.occurrence.location().clone();

        let Some(cited) = self.adoption.owner_of_prefix(prefix).cloned() else {
            self.findings.push(Finding::UnregisteredPrefix {
                prefix: prefix.clone(),
                label: label.clone(),
                location,
            });
            return;
        };

        if cited == held.owner {
            self.findings.push(Finding::SelfQualifiedImport {
                prefix: prefix.clone(),
                label: label.clone(),
                location,
            });
            return;
        }

        if let Some(target) = self.lookup(&cited, label) {
            self.connect(held, target);
        } else if held.generated {
            self.findings.push(Finding::DanglingGeneratedCitation {
                label: label.clone(),
                location,
            });
        } else {
            self.findings.push(Finding::UnresolvedCitation {
                label: label.clone(),
                location,
            });
        }
    }

    /// Pass three: check every declared tracking against the harvested heads.
    ///
    /// Tracking runs after the harvest for the same reason resolution does: an
    /// outline may stand before or after the document it tracks, and a check that
    /// depended on which would decide different things about one corpus read in
    /// two orders.
    fn track(&mut self) {
        let findings = validate_tracking(
            &self.outlines,
            &self.heads_by_source,
            &self.minted_by_source,
        );

        self.findings.extend(findings);
    }

    fn lookup(&self, owner: &Owner, label: &Label) -> Option<NodeIndex> {
        self.registries.get(owner)?.get(label).copied()
    }

    /// Record a resolved citation, edging it from its containing environment.
    fn connect(&mut self, held: &HeldCitation, target: NodeIndex) {
        self.citations_resolved += 1;

        if let Some(source) = self.enclosing_mint(&held.source, held.occurrence.location().offset())
        {
            self.graph.add_edge(
                source,
                target,
                CitationEdge {
                    form: held.occurrence.form().clone(),
                    location: held.occurrence.location().clone(),
                },
            );
        }
    }

    /// The nearest mint preceding an offset in the same source.
    fn enclosing_mint(&self, source: &Path, offset: usize) -> Option<NodeIndex> {
        let mints = self.mints_by_source.get(source)?;
        let following = mints.partition_point(|(mint_offset, _node)| *mint_offset < offset);

        following.checked_sub(1).map(|index| mints[index].1)
    }

    fn finish(mut self, sources_scanned: usize) -> Analysis {
        self.findings
            .sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

        Analysis {
            graph: self.graph,
            findings: self.findings,
            imports: self.imports,
            sources_scanned,
            citations_resolved: self.citations_resolved,
            heads_validated: self.heads,
            derived_mints: self.derived_mints,
            document_mints: self.document_mints,
        }
    }
}
