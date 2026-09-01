// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Part-file assembly: many working parts, one committed publication.
//!
//! A document too large to rewrite in one pass is rewritten in parts, each part
//! a file small enough to hold in the head and lint on its own, and the document
//! the corpus publishes is the concatenation of them. The concatenation is
//! generated, so it is a committed generated publication in the sense of
//! ADR-L-012, and that record decides how it is checked: exact expected-byte
//! comparison, and no digest anywhere. This module is that comparison, and the
//! generator beside it.
//!
//! # The order is declared, never inferred
//!
//! Assembly needs an order, and there are two ways to get one. A naming
//! convention — parts sorted by a numeric filename prefix — infers the order
//! from the names, and renumbering a corpus of parts to insert one in the middle
//! is a rename of every file after it. A manifest declares the order, and the
//! campaign has already settled this question once: outline tracking declares its
//! relation rather than inferring it, because an inferred relation is exactly
//! what a document like this exists to remove. Assembly follows that ruling.
//!
//! The manifest is one document in the parts directory, and it carries a table in
//! the idiom this corpus already reads adoption data in:
//!
//! ```text
//! **Convention (Assembly)** · `conv:spec:assembly`
//!
//! | Part |
//! | --- |
//! | ``010-introduction.md`` |
//! | ``020-syntax.md`` |
//! ```
//!
//! One column, and a table is an assembly manifest exactly when its single
//! header cell is that word. The rows are the parts, in the order they are
//! written, and that order is the assembled document's order. A part cell names a
//! file by its name alone, relative to the parts directory: a part of a document
//! stands beside its siblings, and a manifest that could reach out of its own
//! directory would let one document's parts be assembled out of another's.
//!
//! Membership is checked both ways, for the reason outline tracking checks its
//! relation both ways. A manifest row naming a file that is not there cannot be
//! assembled, and a file standing in the parts directory that no row names would
//! be silently dropped from the publication — which is the failure mode a
//! part-wise rewrite is most likely to hit, because writing a new part and
//! forgetting to list it looks exactly like having finished.
//!
//! # The parts participate and the publication does not
//!
//! Both cannot participate. Every head of every part mints, the publication is
//! those same heads concatenated, and the unique-mint invariant
//! (ADR-L-014, A calculus of documentation and source labels) would read the second copy of each as a
//! duplicate of the first. So one of the two is authored and the other is
//! derived, and the choice is forced by what the rewrite needs: the parts are
//! the working files and must be independently lintable, so the parts are
//! authored and the publication participates in nothing — exactly as the
//! participation judgment (ADR-L-014, A calculus of documentation and source labels) already treats
//! a generated region. A citation of the document reaches the mint in the part
//! that carries it, which is the same label under the same owner, so nothing
//! outside the parts directory has to know the document was assembled at all.
//!
//! # Uniqueness across parts
//!
//! The entry this module answers asks for label uniqueness across parts before
//! assembly. Under the paragraph above, the check already has it: the parts are
//! ordinary carrier sources of one owner, so the unique-mint invariant holds them
//! to it without a second rule, and adding one would report a single duplicate
//! twice. The assemble command is also run on its own, before a whole-corpus
//! check has necessarily passed, so it takes the harvest over its own parts and
//! reports the duplicates it finds under the same code the check would use. The
//! check does not, because there the engine has already done it.
//!
//! # Dormancy, draft, and live
//!
//! An adopted pair passes through three states over its lifetime, and only one
//! of them is unmarked.
//!
//! An adopted pair whose parts directory does not exist is dormant: a document
//! whose part-wise rewrite has not started. That is not a defect and raises
//! nothing; the machinery wakes up when the first part is written, and until
//! then the publication is an ordinary authored document.
//!
//! Once the parts directory exists, the manifest decides whether the rewrite is
//! finished. A manifest opening with the paragraph `**Draft.**`, standing on
//! its own before the table, declares its parts still under authorship: they
//! are checked for membership like any other assembly, both ways, but no
//! freshness verdict is formed over them, because the committed publication is
//! deliberately still the old, unrelated document the campaign has not reached
//! yet — comparing the two would report a difference the rewrite has not
//! promised to have closed. Writing the assembly is refused in this state for
//! the same reason: the parts are not yet the publication's source of truth, so
//! overwriting the standing document with a partial assembly would destroy it
//! rather than update it.
//!
//! Once the marker is removed, the assembly is live: freshness binds again,
//! exactly as it did before draft mode existed, and `--write` is honoured.
//! Nothing about removing the marker is itself checked; a manifest's absence of
//! the paragraph is read the same way its absence of a parts directory is — as
//! the ordinary case.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_an_assembly_manifest`] | assembly | A manifest declares which parts a document is assembled from and in what order, and the order read back is the manifest's own rather than the directory's. The sequence of a published document is stated rather than inferred from file names. |
//! | [`reads_no_manifest_from_an_ordinary_table`] | assembly | A table is a manifest only under its one header name, so a document may tabulate other things without declaring an assembly. |
//! | [`reports_a_row_that_is_not_a_part`] | assembly | A part is one document beside the manifest, named once: a row with two cells, one naming a nested path, one naming a file of another kind, one naming the manifest itself, and a name repeated are each reported, and only the well-formed row is read. |
//! | [`assembles_the_parts_in_the_declared_order`] | assembly | The parts assemble in the order the manifest declares rather than the order their names would sort in, under a header saying where the document came from, and ending in exactly one line feed. |
//! | [`assembles_the_same_bytes_twice`] | assembly | Assembly is byte-stable: the same parts assemble to the same bytes on every run, so a publication can be compared against its parts and any difference means the parts changed rather than the assembler wandered. |
//! | [`reassembles_its_own_output_unchanged`] | assembly | A publication written from its parts is then fresh: reassembling produces exactly the committed bytes and nothing is reported, so the write and the check agree on one recipe. |
//! | [`reports_an_edited_part_as_a_stale_publication`] | assembly | Editing a part without republishing makes the publication stale, and the report names the document and the line at which it first differs from its parts, so the divergence is located rather than merely announced. |
//! | [`reports_an_absent_publication_as_stale`] | assembly | A publication that was never written is stale in the strongest sense, and says so: parts that assemble to a document nobody has published are a defect rather than a state to be tolerated. |
//! | [`reports_a_part_no_manifest_row_names`] | assembly | A document sitting among the parts that no manifest row names is reported as unassembled, while the assembly proceeds from the parts that are declared. A file cannot be silently dropped from a publication by being left out of the manifest. |
//! | [`reports_a_manifest_row_no_part_answers`] | assembly | A row naming a part that is not there is reported, and no document is assembled at all: an assembly missing a part is no document, so no freshness verdict is formed about one. |
//! | [`reports_a_parts_directory_without_a_manifest`] | assembly | A parts directory with no manifest in it is reported: once the parts exist the rewrite has started, and an order that nobody has declared is a gap rather than a default. |
//! | [`stays_dormant_without_a_parts_directory`] | assembly | A declared assembly whose parts directory does not exist stays dormant and reports nothing: an ordinary authored document that has not yet been broken into parts is not a defect, and the declaration may be written before the rewrite begins. |
//! | [`reports_a_label_two_parts_both_mint`] | assembly | The parts of one document may not mint one label twice: the collision is reported against the parts themselves, naming both, so it is found where it can be fixed rather than in the assembled publication. |
//! | [`writes_the_publication_the_check_then_accepts`] | assembly | What the writer commits is exactly what the checker then accepts: the bytes on disk equal the assembled text and the verification that follows finds nothing. Publishing and checking cannot disagree. |
//! | [`is_draft_reads_the_marker`] | assembly | A manifest may declare itself a draft by carrying the marker, which is how an assembly under active rewriting says so. |
//! | [`is_draft_reads_no_marker_when_absent`] | assembly | A manifest without the marker is live, so the ordinary state of an assembly is the one held to freshness. |
//! | [`is_draft_reads_no_marker_from_a_malformed_bold_line`] | assembly | The marker's grammar is exact, and a near miss is simply not the marker: the word without its stop, the wrong casing, extra words after it, and the word unemphasised each leave the manifest live. Suspending a check takes writing the marker and cannot happen by accident of prose. |
//! | [`a_draft_manifest_forms_no_freshness_verdict`] | assembly | A draft manifest still assembles its parts but no freshness verdict is formed about the publication, so a rewrite in progress is not required to republish after every edit. |
//! | [`draft_manifest_still_checks_membership`] | assembly | Draft mode suspends the freshness verdict and nothing else: a part no row names is still reported while the manifest is a draft, so the membership relation is checked throughout a rewrite. |
//! | [`removing_the_marker_restores_freshness`] | assembly | Freshness binds again the moment the marker is removed: one and the same manifest reports nothing as a draft and reports staleness once live, so a rewrite is finished by deleting a line. |
//! | [`reads_a_publication_row_as_owner_parts_and_target`] | assembly | A publication row carries its owner, its parts and its target together, so every consumer of the row is attributing the parts and the document the same way. Recovering the owner from the parts path at each use was the alternative, and two derivations of one relation are two chances to disagree about it. |
//! | [`derives_the_generated_documents_from_the_rows`] | assembly | The generated-document set is the targets of the rows, derived rather than declared beside them. A generated document mints nothing of its own — its labels are its parts', already read where they are maintained — so a second list of generated documents would be a second source for one truth, free to fall out of step with the rows. |
//! | [`derives_the_generator_and_containment_defects_from_the_rows`] | assembly | Well-formed rows fail nothing, and the two ways they can fail are derived from the rows alone. Two sets of parts claiming one target make the published bytes depend on which ran last, so the freshness comparison has no answer rather than a wrong one; and a target inside its own parts directory would be a part of itself, so writing it would change the input it was assembled from and no fixed point exists. |
//! | [`the_publications_program_fingerprints_its_defects`] | assembly | The publications program declares the fingerprint codec: a defect is a relation between an owner, a parts directory and a target rather than an occurrence inside a file, so two defects over one document stay two identities instead of collapsing into one tolerated row. |

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::catalogue::Codec;
use crate::finding::{Finding, Location};
use crate::label::Label;
use crate::prose::scan_markdown;
#[cfg(test)]
use crate::snapshot::Snapshot;

/// The single header cell that marks a table as an assembly manifest.
///
/// TODO ´todo:code:no-record-fixes-this-cell-membership´: no record fixes this cell. Membership being checked both ways is
/// recorded, but the spelling that makes a table the manifest rather than an
/// ordinary table of parts is this module's own recognition contract, and a
/// contract nothing states cannot be confirmed against anything.
///
/// ´const:emberlinter:manifest-recognition-header´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:manifest-recognition-header-form-x7fede830´
const MANIFEST_HEADER: [&str; 1] = ["Part"];

/// The manifest document's name, inside every parts directory.
///
/// TODO ´todo:code:no-record-fixes-this-filename-the´: no record fixes this filename. The rewrite's own entry names a
/// manifest and says the parts stand under it, but leaves what the file is
/// called to whoever wrote the tool.
///
/// ´const:emberlinter:manifest-filename´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:manifest-filename-text-x72298b10´
pub const MANIFEST_FILE: &str = "assembly.md";

/// One assembled document: its parts, their manifest, and its publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembly {
    parts: PathBuf,
    manifest: PathBuf,
    target: PathBuf,
}

impl Assembly {
    /// Declare an assembly from a parts directory and the document it publishes.
    ///
    /// Both paths are relative to the repository root, as every path a finding
    /// prints is.
    #[must_use]
    pub fn new(parts: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        let parts = parts.into();
        let manifest = parts.join(MANIFEST_FILE);

        Self {
            parts,
            manifest,
            target: target.into(),
        }
    }

    /// The directory the working parts stand in.
    #[must_use]
    pub fn parts(&self) -> &Path {
        &self.parts
    }

    /// The manifest declaring which parts are assembled, and in which order.
    #[must_use]
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }

    /// The published document the parts assemble into.
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }
}

/// The assemblies a snapshot's publication rows declare.
#[must_use]
#[cfg(test)]
pub fn retiring_index_assemblies(declared: &Snapshot) -> Vec<Assembly> {
    retiring_index_publications(declared)
        .rows()
        .iter()
        .map(|publication| publication.assembly().clone())
        .collect()
}

/// The publication rows the assembly document declares.
///
/// The owner is no longer asked of the partition, because the document states it:
/// a publication row stands on the table of the owner it belongs to, so the
/// attribution the partition would have derived is written where the parts and
/// the target are.
#[must_use]
#[cfg(test)]
pub fn retiring_index_publications(declared: &Snapshot) -> Publications {
    Publications::new(declared.declared_publications())
}

/// One publication row: an owner, its parts, and the document they publish.
///
/// The owner rides on the row rather than being recovered from the parts path at
/// each use. Attribution is a relation the owner partition already decides, and
/// a second derivation of it at each consumer is a second chance to disagree
/// with the first — so it is decided once and carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publication {
    owner: String,
    assembly: Assembly,
}

impl Publication {
    /// Declare a publication from its owner, its parts, and its target.
    #[must_use]
    pub fn new(
        owner: impl Into<String>,
        parts: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
    ) -> Self {
        Self::attributed(owner, Assembly::new(parts, target))
    }

    /// Attribute an already-formed assembly to an owner.
    #[must_use]
    pub fn attributed(owner: impl Into<String>, assembly: Assembly) -> Self {
        Self {
            owner: owner.into(),
            assembly,
        }
    }

    /// The owner whose corpus both the parts and the publication belong to.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The assembly this row declares.
    #[must_use]
    pub const fn assembly(&self) -> &Assembly {
        &self.assembly
    }
}

/// What a set of publication rows failed to be.
///
/// Both are derived from the rows rather than declared beside them, which is the
/// point of deriving them: a document also stating "this target has one
/// generator" would state something its own rows already decide, and the two
/// could then disagree with nothing to say which was right.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublicationDefect {
    /// One target is generated by more than one set of parts.
    ///
    /// Two generators make the published bytes depend on which one ran last, so
    /// the freshness comparison has no answer rather than a wrong one.
    TwoGenerators {
        /// The contested target.
        target: PathBuf,
        /// The parts directories claiming it, ordered.
        parts: Vec<PathBuf>,
    },
    /// A target stands inside the parts directory that generates it.
    ///
    /// The publication would be a part of itself: writing it would change the
    /// input it was assembled from, so the assembly has no fixed point and the
    /// freshness comparison could never settle.
    TargetInsideParts {
        /// The self-containing target.
        target: PathBuf,
        /// The parts directory that would contain it.
        parts: PathBuf,
    },
}

/// The parameters of the current-publications program: the rows, and nothing.
///
/// Everything a caller might otherwise have declared is derived here instead.
/// The generated-document set is the targets; nonparticipation follows from
/// being a target; and generator uniqueness is a property the rows already
/// have. A declaration repeating any of the three would be a second source for
/// one truth, and the older arrangement — an adoption list of targets beside a
/// separate policy identity for the same publication — is exactly what that
/// costs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publications {
    rows: Vec<Publication>,
}

impl Publications {
    /// Instantiate the program with a set of publication rows.
    #[must_use]
    pub fn new(rows: impl IntoIterator<Item = Publication>) -> Self {
        Self {
            rows: rows.into_iter().collect(),
        }
    }

    /// The identity form this program's tolerated violations are written in.
    ///
    /// A publication defect is a relation between an owner, a parts directory
    /// and a target rather than an occurrence inside a file, so the identity is
    /// a digest of those fields and two defects over one document stay two.
    #[must_use]
    pub const fn codec() -> Codec {
        Codec::Fingerprint
    }

    /// The declared rows.
    #[must_use]
    pub fn rows(&self) -> &[Publication] {
        &self.rows
    }

    /// The documents these rows generate.
    ///
    /// A generated document mints nothing of its own: its labels are its parts',
    /// already read where they are maintained, so reading them again at the
    /// publication would report every one as a collision with itself.
    /// Nonparticipation is therefore derived from being a target, and is never
    /// declared beside the row that makes it one.
    #[must_use]
    pub fn generated_targets(&self) -> Vec<PathBuf> {
        let mut targets: Vec<PathBuf> = self
            .rows
            .iter()
            .map(|row| row.assembly().target().to_path_buf())
            .collect();

        targets.sort();
        targets.dedup();
        targets
    }

    /// What the rows fail to be, derived from the rows alone.
    #[must_use]
    pub fn defects(&self) -> Vec<PublicationDefect> {
        let mut generators: BTreeMap<&Path, Vec<PathBuf>> = BTreeMap::new();
        let mut defects = Vec::new();

        for row in &self.rows {
            let assembly = row.assembly();

            generators
                .entry(assembly.target())
                .or_default()
                .push(assembly.parts().to_path_buf());

            if assembly.target().starts_with(assembly.parts()) {
                defects.push(PublicationDefect::TargetInsideParts {
                    target: assembly.target().to_path_buf(),
                    parts: assembly.parts().to_path_buf(),
                });
            }
        }

        for (target, mut parts) in generators {
            parts.sort();
            parts.dedup();

            if parts.len() > 1 {
                defects.push(PublicationDefect::TwoGenerators {
                    target: target.to_path_buf(),
                    parts,
                });
            }
        }

        defects.sort();
        defects
    }
}

/// One row of an assembly manifest: a part, and where it was listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRow {
    name: String,
    location: Location,
}

impl PartRow {
    /// The part's file name, relative to the parts directory.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the listing row stands in the manifest.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// What one assembly is, once its manifest and directory have been read.
#[derive(Debug, Clone)]
pub struct Assembled {
    /// Whether the parts directory exists at all.
    pub dormant: bool,
    /// Whether the manifest carries the draft marker.
    ///
    /// A draft assembly forms no freshness verdict, so [`Finding::StaleAssembly`]
    /// never fires over it, and the caller that would otherwise write its
    /// publication refuses to.
    pub draft: bool,
    /// How many parts the manifest listed and the directory carried.
    pub parts: usize,
    /// The assembled bytes, when every listed part could be read.
    pub text: Option<String>,
}

/// Read one manifest's assembly table, and what its rows failed to be.
///
/// A document carrying no assembly table declares nothing; that is a defect of a
/// manifest but not of this reader, so the reader stays total and the caller
/// decides.
#[must_use]
pub fn read_manifest(path: &Path, source: &str) -> (Vec<PartRow>, Vec<Finding>) {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let mut reader = Reader::new(path, source);

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        reader.event(&event, range.start);
    }

    reader.finish()
}

/// The bold text of the draft marker, `**Draft.**`, once Markdown unwraps it.
///
/// TODO ´todo:code:the-record-fixes-that-a-draft´: the record fixes that a draft marker exists and what it does — it is
/// the one deliberate suspension, and it suspends freshness alone — and the
/// rewrite's entry says the parts stood under a Draft-marked manifest. Neither
/// fixes the spelling, so the capitalisation and the stop this module insists on
/// answer to nothing but this line.
///
/// ´const:emberlinter:manifest-draft-marker´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:manifest-draft-marker-text-xf10e4686´
const DRAFT_MARKER: &str = "Draft.";

/// Whether a manifest's rows are still under authorship rather than published.
///
/// The marker is one paragraph standing on its own before the table, and
/// nothing else: exactly `**Draft.**`, in that capitalisation and with that
/// stop. A bold run that says something close — `**Draft**` without the stop,
/// `**DRAFT.**` shouting it, or `**Draft.**` sharing its paragraph with other
/// text — is not the marker, and is not a defect either: a manifest is free to
/// open with whatever prose it likes, and this reads for the one exact
/// paragraph and nothing past the table, where the marker would no longer
/// govern anything.
#[must_use]
fn is_draft(source: &str) -> bool {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let mut in_table = false;
    let mut paragraph: Option<Vec<Event<'_>>> = None;
    let mut found = false;

    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(Tag::Table(_)) => in_table = true,
            Event::Start(Tag::Paragraph) if !in_table => paragraph = Some(Vec::new()),
            Event::End(TagEnd::Paragraph) if !in_table => {
                if let Some(events) = paragraph.take()
                    && is_draft_paragraph(&events)
                {
                    found = true;
                }
            }
            other => {
                if let Some(events) = paragraph.as_mut() {
                    events.push(other);
                }
            }
        }
    }

    found
}

/// Whether a paragraph's inner events are exactly the draft marker's bold run.
fn is_draft_paragraph(events: &[Event<'_>]) -> bool {
    matches!(
        events,
        [Event::Start(Tag::Strong), Event::Text(text), Event::End(TagEnd::Strong)]
        if text.as_ref() == DRAFT_MARKER
    )
}

/// Assemble one adopted pair, and report everything that stood in the way.
///
/// The returned text is the bytes the publication must carry. It is absent when
/// the parts could not all be read, which is exactly when no freshness judgment
/// can be formed — reporting the document stale because a part is missing would
/// name the wrong repair.
#[must_use]
pub fn verify_assembly(root: &Path, assembly: &Assembly) -> (Assembled, Vec<Finding>) {
    let mut findings = Vec::new();

    if !root.join(assembly.parts()).is_dir() {
        return (dormant(), findings);
    }

    let manifest = match fs::read_to_string(root.join(assembly.manifest())) {
        Ok(text) => text,
        Err(_error) => {
            findings.push(Finding::MissingAssemblyManifest {
                parts: display(assembly.parts()),
                manifest: display(assembly.manifest()),
            });

            return (unassembled(), findings);
        }
    };

    let draft = is_draft(&manifest);

    let (rows, manifest_findings) = read_manifest(assembly.manifest(), &manifest);
    findings.extend(manifest_findings);

    let present = present_parts(root, assembly, &mut findings);
    let listed = check_membership(assembly, &rows, &present, &mut findings);

    let Some(listed) = listed else {
        return (unassembled(), findings);
    };

    let mut texts = Vec::with_capacity(listed.len());

    for name in &listed {
        match fs::read_to_string(root.join(assembly.parts()).join(name)) {
            Ok(text) => texts.push(text),
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: display(&assembly.parts().join(name)),
                    message: error.to_string(),
                });

                return (unassembled(), findings);
            }
        }
    }

    let text = assemble_text(assembly, &texts);

    if !draft {
        findings.extend(freshness(root, assembly, &text));
    }

    (
        Assembled {
            dormant: false,
            draft,
            parts: listed.len(),
            text: Some(text),
        },
        findings,
    )
}

/// Report every label two of an assembly's parts both mint.
///
/// This is the harvest the check already takes over the parts as carrier sources,
/// repeated here so the assemble command can stand alone. The owner is the one
/// the corpus's partition assigns the parts directory; the finding is the same
/// finding, so a reader meets one code for one defect however it was found.
#[must_use]
pub fn part_duplicate_mints(root: &Path, assembly: &Assembly, owner: &str) -> Vec<Finding> {
    let mut first_seen: BTreeMap<Label, Location> = BTreeMap::new();
    let mut findings = Vec::new();

    let Ok(entries) = part_names(root, assembly) else {
        return findings;
    };

    for name in entries {
        let relative = assembly.parts().join(&name);

        let Ok(text) = fs::read_to_string(root.join(&relative)) else {
            continue;
        };

        let (occurrences, _blocks, _findings) = scan_markdown(&relative, &text).into_parts();

        for occurrence in occurrences.iter().filter(|occurrence| occurrence.is_mint()) {
            match first_seen.get(occurrence.label()) {
                Some(first) => findings.push(Finding::DuplicateMint {
                    label: occurrence.label().clone(),
                    owner: owner.to_owned(),
                    first: first.clone(),
                    second: occurrence.location().clone(),
                }),
                None => {
                    first_seen.insert(occurrence.label().clone(), occurrence.location().clone());
                }
            }
        }
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    findings
}

/// Write an assembly's publication, when the parts yielded one.
///
/// # Errors
///
/// Returns the underlying message when the publication cannot be written, which
/// the caller reports rather than treating as a fresh document.
pub fn write_assembly(root: &Path, assembly: &Assembly, text: &str) -> Result<(), String> {
    let path = root.join(assembly.target());

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(path, text).map_err(|error| error.to_string())
}

/// The bytes a set of part texts assembles into.
///
/// The recipe is fixed and normalising, which is what makes assembly byte-stable
/// across runs and idempotent under reassembly: a banner naming where the
/// document came from, then every part's text with its surrounding blank lines
/// removed, separated by exactly one blank line, and exactly one line feed at the
/// end. A part that gains or loses trailing blank lines therefore does not change
/// the publication, and a part that gains a word does.
#[must_use]
fn assemble_text(assembly: &Assembly, parts: &[String]) -> String {
    let mut assembled = banner(assembly);

    for part in parts {
        let body = part.trim();

        if body.is_empty() {
            continue;
        }

        assembled.push('\n');
        assembled.push_str(body);
        assembled.push('\n');
    }

    assembled
}

/// The line every assembled publication opens with.
///
/// It is an HTML comment, so it carries no span, mints nothing, and cites
/// nothing; it exists so that a reader who opens the publication rather than the
/// parts learns immediately that editing it is editing the wrong file.
fn banner(assembly: &Assembly) -> String {
    format!(
        "<!-- Assembled from {}/ under {}. Edit the parts, not this file. -->\n",
        display(assembly.parts()),
        display(assembly.manifest())
    )
}

/// The state of a pair whose parts directory is not there.
const fn dormant() -> Assembled {
    Assembled {
        dormant: true,
        draft: false,
        parts: 0,
        text: None,
    }
}

/// The state of a pair that was read but could not be assembled.
const fn unassembled() -> Assembled {
    Assembled {
        dormant: false,
        draft: false,
        parts: 0,
        text: None,
    }
}

/// Every Markdown file standing in a parts directory, manifest excepted.
fn present_parts(root: &Path, assembly: &Assembly, findings: &mut Vec<Finding>) -> Vec<String> {
    match part_names(root, assembly) {
        Ok(names) => names,
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: display(assembly.parts()),
                message: error,
            });

            Vec::new()
        }
    }
}

/// Every Markdown file standing in a parts directory, in name order.
fn part_names(root: &Path, assembly: &Assembly) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(root.join(assembly.parts())).map_err(|error| error.to_string())?;

    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != MANIFEST_FILE && name.to_lowercase().ends_with(".md"))
        .collect();

    names.sort();

    Ok(names)
}

/// Check the manifest and the directory against each other, both ways.
///
/// Returns the parts to assemble, in the manifest's order, or nothing when a
/// listed part is not there — an assembly missing one of its parts is not a
/// shorter document, it is no document.
fn check_membership(
    assembly: &Assembly,
    rows: &[PartRow],
    present: &[String],
    findings: &mut Vec<Finding>,
) -> Option<Vec<String>> {
    let mut listed = Vec::with_capacity(rows.len());
    let mut complete = true;

    for row in rows {
        if present.iter().any(|name| name == &row.name) {
            listed.push(row.name.clone());
        } else {
            complete = false;
            findings.push(Finding::AbsentPart {
                part: row.name.clone(),
                parts: display(assembly.parts()),
                location: row.location.clone(),
            });
        }
    }

    for name in present {
        if !rows.iter().any(|row| &row.name == name) {
            findings.push(Finding::UnassembledPart {
                part: name.clone(),
                parts: display(assembly.parts()),
                manifest: display(assembly.manifest()),
            });
        }
    }

    complete.then_some(listed)
}

/// Compare the committed publication against the bytes the parts assemble into.
fn freshness(root: &Path, assembly: &Assembly, text: &str) -> Vec<Finding> {
    let stale = |reason: String| {
        vec![Finding::StaleAssembly {
            target: display(assembly.target()),
            parts: display(assembly.parts()),
            reason,
        }]
    };

    match fs::read_to_string(root.join(assembly.target())) {
        Err(_error) => stale("the assembled document is not there".to_owned()),
        Ok(found) if found == text => Vec::new(),
        Ok(found) => stale(first_difference(text, &found)),
    }
}

/// Where the committed publication first stops being the assembly.
///
/// The comparison itself is over the whole byte strings and nothing else; this
/// only says where, so that a reader has somewhere to look. No digest is taken,
/// per the artifact case of ADR-L-012.
fn first_difference(expected: &str, found: &str) -> String {
    let mut expected_lines = expected.lines();
    let mut found_lines = found.lines();
    let mut line = 1;

    loop {
        match (expected_lines.next(), found_lines.next()) {
            (None, None) => {
                return format!(
                    "the assembled document differs from its parts after line {}",
                    line - 1
                );
            }
            (Some(left), Some(right)) if left == right => line += 1,
            (Some(_left), None) => {
                return format!("the assembled document stops short at line {line}");
            }
            (None, Some(_right)) => {
                return format!("the assembled document runs on past line {line}");
            }
            (Some(_left), Some(_right)) => {
                return format!("the assembled document differs from its parts at line {line}");
            }
        }
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The manifest reader, which is a small state machine over the event stream.
struct Reader<'a> {
    path: &'a Path,
    source: &'a str,
    header: Vec<String>,
    cells: Vec<String>,
    cell: String,
    in_cell: bool,
    row_start: usize,
    rows: Vec<PartRow>,
    findings: Vec<Finding>,
}

impl<'a> Reader<'a> {
    const fn new(path: &'a Path, source: &'a str) -> Self {
        Self {
            path,
            source,
            header: Vec::new(),
            cells: Vec::new(),
            cell: String::new(),
            in_cell: false,
            row_start: 0,
            rows: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn event(&mut self, event: &Event<'_>, start: usize) {
        match event {
            Event::Start(Tag::Table(_)) => self.header.clear(),
            Event::Start(Tag::TableHead) => self.cells.clear(),
            Event::Start(Tag::TableRow) => {
                self.cells.clear();
                self.row_start = start;
            }
            Event::End(TagEnd::TableHead) => self.header = std::mem::take(&mut self.cells),
            Event::End(TagEnd::TableRow) => self.row(),
            Event::Start(Tag::TableCell) => {
                self.in_cell = true;
                self.cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                self.in_cell = false;
                let cell = std::mem::take(&mut self.cell);
                self.cells.push(cell.trim().to_owned());
            }
            Event::Text(text) | Event::Code(text) if self.in_cell => self.cell.push_str(text),
            _ => {}
        }
    }

    fn is_manifest_table(&self) -> bool {
        self.header == MANIFEST_HEADER
    }

    /// Read one body row of an assembly manifest.
    fn row(&mut self) {
        if !self.is_manifest_table() {
            return;
        }

        let location = Location::new(self.path, self.source, self.row_start);

        let [name] = self.cells.as_slice() else {
            self.findings.push(Finding::MalformedManifestRow {
                text: self.cells.join(" | "),
                reason: format!("a manifest row has one cell, not {}", self.cells.len()),
                location,
            });
            return;
        };

        if let Some(reason) = malformed_part_name(name) {
            self.findings.push(Finding::MalformedManifestRow {
                text: name.clone(),
                reason,
                location,
            });
            return;
        }

        if self.rows.iter().any(|row| &row.name == name) {
            self.findings.push(Finding::MalformedManifestRow {
                text: name.clone(),
                reason: "a part is listed once, and this row lists one already listed".to_owned(),
                location,
            });
            return;
        }

        self.rows.push(PartRow {
            name: name.clone(),
            location,
        });
    }

    fn finish(self) -> (Vec<PartRow>, Vec<Finding>) {
        (self.rows, self.findings)
    }
}

/// Why a manifest cell is not a part name, when it is not one.
fn malformed_part_name(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("a manifest row names a part file".to_owned());
    }

    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Some("a part stands beside its siblings, so its name carries no path".to_owned());
    }

    if !name.to_lowercase().ends_with(".md") {
        return Some("a part is a Markdown document".to_owned());
    }

    if name == MANIFEST_FILE {
        return Some("the manifest is not one of the parts it lists".to_owned());
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        Assembly, MANIFEST_FILE, Publication, PublicationDefect, Publications, is_draft,
        part_duplicate_mints, read_manifest, verify_assembly, write_assembly,
    };
    use crate::catalogue::Codec;
    use crate::finding::Finding;

    const PARTS: &str = "docs/spec";
    const TARGET: &str = "docs/spec.md";

    fn manifest_source(rows: &str) -> String {
        format!(
            "# The manifest\n\n\
             **Convention (Assembly)** · `conv:fixture:assembly`\n\n\
             | Part |\n| --- |\n{rows}\n"
        )
    }

    /// The same manifest, carrying the draft marker before its table.
    fn draft_manifest_source(rows: &str) -> String {
        format!(
            "# The manifest\n\n\
             **Draft.**\n\n\
             **Convention (Assembly)** · `conv:fixture:assembly`\n\n\
             | Part |\n| --- |\n{rows}\n"
        )
    }

    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(Finding::code).collect()
    }

    /// A fixture root with two parts, listed second before first.
    fn fixture(root: &Path, rows: &str) {
        fs::create_dir_all(root.join(PARTS)).expect("create");
        fs::write(root.join(PARTS).join(MANIFEST_FILE), manifest_source(rows)).expect("write");
        fs::write(
            root.join(PARTS).join("alpha.md"),
            "## Alpha · `sec:fixture:alpha`\n\nThe first part.\n",
        )
        .expect("write");
        fs::write(
            root.join(PARTS).join("beta.md"),
            "## Beta · `sec:fixture:beta`\n\nThe second part.\n",
        )
        .expect("write");
    }

    fn both_rows() -> String {
        "| ``beta.md`` |\n| ``alpha.md`` |".to_owned()
    }

    fn assembly() -> Assembly {
        Assembly::new(PARTS, TARGET)
    }

    /// Assemble a fixture root and write the publication the parts want.
    fn settle(root: &Path) -> String {
        let (assembled, _findings) = verify_assembly(root, &assembly());
        let text = assembled.text.expect("the parts assemble");
        write_assembly(root, &assembly(), &text).expect("write the publication");

        text
    }

    /// A manifest declares which parts a document is assembled from and in what
    /// order, and the order read back is the manifest's own rather than the
    /// directory's. The sequence of a published document is stated rather than
    /// inferred from file names.
    ///
    /// ´claim:assembly:a-manifest-declares-the-parts-and-their-order´
    /// ´test:unit:reads-an-assembly-manifest´
    #[test]
    fn reads_an_assembly_manifest() {
        let (rows, findings) = read_manifest(
            Path::new("docs/spec/assembly.md"),
            &manifest_source(&both_rows()),
        );

        assert_eq!(findings, []);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].name(),
            "beta.md",
            "the manifest's order, not the directory's"
        );
        assert_eq!(rows[1].name(), "alpha.md");
    }

    /// A table is a manifest only under its one header name, so a document may
    /// tabulate other things without declaring an assembly.
    ///
    /// ´claim:assembly:only-a-table-under-the-manifest-header-declares-parts´
    /// ´test:unit:reads-no-manifest-from-an-ordinary-table´
    #[test]
    fn reads_no_manifest_from_an_ordinary_table() {
        let source = "| Document |\n| --- |\n| ``docs/spec.md`` |\n";
        let (rows, findings) = read_manifest(Path::new("docs/spec/assembly.md"), source);

        assert!(
            rows.is_empty(),
            "a table is a manifest only under its one header name"
        );
        assert_eq!(findings, []);
    }

    /// A part is one document beside the manifest, named once: a row with two
    /// cells, one naming a nested path, one naming a file of another kind, one
    /// naming the manifest itself, and a name repeated are each reported, and
    /// only the well-formed row is read.
    ///
    /// ´claim:assembly:every-manifest-row-names-one-sibling-document-once´
    /// ´test:unit:reports-a-row-that-is-not-a-part´
    #[test]
    fn reports_a_row_that_is_not_a_part() {
        let rows = concat!(
            "| ``alpha.md`` | ``beta.md`` |\n",
            "| ``nested/gamma.md`` |\n",
            "| ``delta.txt`` |\n",
            "| ``assembly.md`` |\n",
            "| ``alpha.md`` |\n",
            "| ``alpha.md`` |",
        );
        let (read, findings) =
            read_manifest(Path::new("docs/spec/assembly.md"), &manifest_source(rows));

        assert_eq!(read.len(), 1, "only the one well-formed row is read");
        assert_eq!(
            codes(&findings),
            [
                "malformed_manifest_row",
                "malformed_manifest_row",
                "malformed_manifest_row",
                "malformed_manifest_row",
                "malformed_manifest_row",
            ]
        );
    }

    /// The parts assemble in the order the manifest declares rather than the
    /// order their names would sort in, under a header saying where the
    /// document came from, and ending in exactly one line feed.
    ///
    /// ´claim:assembly:the-parts-assemble-in-the-declared-order-under-a-provenance-header´
    /// ´test:unit:assembles-the-parts-in-the-declared-order´
    #[test]
    fn assembles_the_parts_in_the_declared_order() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());

        let (assembled, findings) = verify_assembly(root.path(), &assembly());
        let text = assembled.text.expect("the parts assemble");

        assert_eq!(
            codes(&findings),
            ["stale_assembly"],
            "nothing is committed yet"
        );
        assert_eq!(assembled.parts, 2);
        assert!(text.starts_with("<!-- Assembled from docs/spec/ under docs/spec/assembly.md."));
        assert!(
            text.find("sec:fixture:beta") < text.find("sec:fixture:alpha"),
            "the manifest's order decides, not the file names: {text}"
        );
        assert!(
            text.ends_with("The first part.\n"),
            "one line feed at the end: {text:?}"
        );
    }

    /// Assembly is byte-stable: the same parts assemble to the same bytes on
    /// every run, so a publication can be compared against its parts and any
    /// difference means the parts changed rather than the assembler wandered.
    ///
    /// ´claim:assembly:assembling-the-same-parts-yields-the-same-bytes´
    /// ´test:unit:assembles-the-same-bytes-twice´
    #[test]
    fn assembles_the_same_bytes_twice() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());

        let (once, _findings) = verify_assembly(root.path(), &assembly());
        let (twice, _again) = verify_assembly(root.path(), &assembly());

        assert_eq!(once.text, twice.text, "assembly is byte-stable across runs");
    }

    /// A publication written from its parts is then fresh: reassembling
    /// produces exactly the committed bytes and nothing is reported, so the
    /// write and the check agree on one recipe.
    ///
    /// ´claim:assembly:a-published-document-reassembles-to-exactly-itself´
    /// ´test:unit:reassembles-its-own-output-unchanged´
    #[test]
    fn reassembles_its_own_output_unchanged() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());

        let written = settle(root.path());
        let (again, findings) = verify_assembly(root.path(), &assembly());

        assert_eq!(
            codes(&findings),
            Vec::<&str>::new(),
            "the committed bytes are fresh"
        );
        assert_eq!(again.text.as_deref(), Some(written.as_str()));
    }

    /// Editing a part without republishing makes the publication stale, and the
    /// report names the document and the line at which it first differs from
    /// its parts, so the divergence is located rather than merely announced.
    ///
    /// ´claim:assembly:an-edited-part-makes-the-publication-stale-at-a-named-line´
    /// ´test:unit:reports-an-edited-part-as-a-stale-publication´
    #[test]
    fn reports_an_edited_part_as_a_stale_publication() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());
        let _settled = settle(root.path());

        fs::write(
            root.path().join(PARTS).join("alpha.md"),
            "## Alpha · `sec:fixture:alpha`\n\nThe first part, rewritten.\n",
        )
        .expect("write");

        let (_assembled, findings) = verify_assembly(root.path(), &assembly());

        let [Finding::StaleAssembly { target, reason, .. }] = findings.as_slice() else {
            panic!("expected the publication to be stale, got {findings:?}");
        };

        assert_eq!(target, TARGET);
        assert!(
            reason.contains("differs from its parts at line"),
            "got {reason}"
        );
    }

    /// A publication that was never written is stale in the strongest sense,
    /// and says so: parts that assemble to a document nobody has published are
    /// a defect rather than a state to be tolerated.
    ///
    /// ´claim:assembly:an-unwritten-publication-is-stale´
    /// ´test:unit:reports-an-absent-publication-as-stale´
    #[test]
    fn reports_an_absent_publication_as_stale() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());

        let (_assembled, findings) = verify_assembly(root.path(), &assembly());

        assert!(
            matches!(findings.as_slice(), [Finding::StaleAssembly { reason, .. }] if reason.contains("not there")),
            "got {findings:?}"
        );
    }

    /// A document sitting among the parts that no manifest row names is
    /// reported as unassembled, while the assembly proceeds from the parts that
    /// are declared. A file cannot be silently dropped from a publication by
    /// being left out of the manifest.
    ///
    /// ´claim:assembly:a-part-no-row-names-is-reported-and-the-rest-still-assemble´
    /// ´test:unit:reports-a-part-no-manifest-row-names´
    #[test]
    fn reports_a_part_no_manifest_row_names() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), "| ``alpha.md`` |");

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, Finding::UnassembledPart { part, .. } if part == "beta.md")),
            "got {findings:?}"
        );
        assert_eq!(assembled.parts, 1, "and the assembly proceeds without it");
    }

    /// A row naming a part that is not there is reported, and no document is
    /// assembled at all: an assembly missing a part is no document, so no
    /// freshness verdict is formed about one.
    ///
    /// ´claim:assembly:a-row-with-no-part-behind-it-assembles-no-document´
    /// ´test:unit:reports-a-manifest-row-no-part-answers´
    #[test]
    fn reports_a_manifest_row_no_part_answers() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(
            root.path(),
            "| ``alpha.md`` |\n| ``gamma.md`` |\n| ``beta.md`` |",
        );

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert_eq!(codes(&findings), ["absent_part"], "got {findings:?}");
        assert!(
            assembled.text.is_none(),
            "an assembly missing a part is no document, so no freshness verdict is formed"
        );
    }

    /// A parts directory with no manifest in it is reported: once the parts
    /// exist the rewrite has started, and an order that nobody has declared is
    /// a gap rather than a default.
    ///
    /// ´claim:assembly:a-parts-directory-without-a-manifest-is-reported´
    /// ´test:unit:reports-a-parts-directory-without-a-manifest´
    #[test]
    fn reports_a_parts_directory_without_a_manifest() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join(PARTS)).expect("create");
        fs::write(
            root.path().join(PARTS).join("alpha.md"),
            "## Alpha · `sec:fixture:alpha`\n",
        )
        .expect("write");

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert_eq!(codes(&findings), ["missing_assembly_manifest"]);
        assert!(assembled.text.is_none());
        assert!(
            !assembled.dormant,
            "the directory is there, so the rewrite has started"
        );
    }

    /// A declared assembly whose parts directory does not exist stays dormant
    /// and reports nothing: an ordinary authored document that has not yet been
    /// broken into parts is not a defect, and the declaration may be written
    /// before the rewrite begins.
    ///
    /// ´claim:assembly:an-assembly-with-no-parts-directory-lies-dormant´
    /// ´test:unit:stays-dormant-without-a-parts-directory´
    #[test]
    fn stays_dormant_without_a_parts_directory() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join("docs")).expect("create");
        fs::write(
            root.path().join(TARGET),
            "# An ordinary authored document\n",
        )
        .expect("write");

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert_eq!(
            codes(&findings),
            Vec::<&str>::new(),
            "an unstarted rewrite is not a defect"
        );
        assert!(assembled.dormant);
    }

    /// The parts of one document may not mint one label twice: the collision is
    /// reported against the parts themselves, naming both, so it is found where
    /// it can be fixed rather than in the assembled publication.
    ///
    /// ´claim:assembly:two-parts-minting-one-label-collide-in-the-parts-themselves´
    /// ´test:unit:reports-a-label-two-parts-both-mint´
    #[test]
    fn reports_a_label_two_parts_both_mint() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());
        fs::write(
            root.path().join(PARTS).join("beta.md"),
            "## Beta · `sec:fixture:alpha`\n\nThe second part, minting the first's label.\n",
        )
        .expect("write");

        let findings = part_duplicate_mints(root.path(), &assembly(), "ember-fixture");

        let [
            Finding::DuplicateMint {
                label,
                first,
                second,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one duplicate mint, got {findings:?}");
        };

        assert_eq!(label.to_string(), "sec:fixture:alpha");
        assert_eq!(first.path(), Path::new("docs/spec/alpha.md"));
        assert_eq!(second.path(), Path::new("docs/spec/beta.md"));
    }

    /// What the writer commits is exactly what the checker then accepts: the
    /// bytes on disk equal the assembled text and the verification that follows
    /// finds nothing. Publishing and checking cannot disagree.
    ///
    /// ´claim:assembly:what-the-writer-commits-is-what-the-check-accepts´
    /// ´test:unit:writes-the-publication-the-check-then-accepts´
    #[test]
    fn writes_the_publication_the_check_then_accepts() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());

        let text = settle(root.path());
        let committed = fs::read_to_string(root.path().join(TARGET)).expect("read");

        assert_eq!(committed, text);

        let (_assembled, findings) = verify_assembly(root.path(), &assembly());

        assert_eq!(codes(&findings), Vec::<&str>::new());
    }

    /// A manifest may declare itself a draft by carrying the marker, which is
    /// how an assembly under active rewriting says so.
    ///
    /// ´claim:assembly:a-manifest-declares-itself-a-draft-by-its-marker´
    /// ´test:unit:is-draft-reads-the-marker´
    #[test]
    fn is_draft_reads_the_marker() {
        assert!(is_draft(&draft_manifest_source(&both_rows())));
    }

    /// A manifest without the marker is live, so the ordinary state of an
    /// assembly is the one held to freshness.
    ///
    /// ´claim:assembly:a-manifest-without-the-marker-is-live´
    /// ´test:unit:is-draft-reads-no-marker-when-absent´
    #[test]
    fn is_draft_reads_no_marker_when_absent() {
        assert!(!is_draft(&manifest_source(&both_rows())));
    }

    /// The marker's grammar is exact, and a near miss is simply not the marker:
    /// the word without its stop, the wrong casing, extra words after it, and
    /// the word unemphasised each leave the manifest live. Suspending a check
    /// takes writing the marker and cannot happen by accident of prose.
    ///
    /// ´claim:assembly:a-near-miss-of-the-draft-marker-leaves-the-manifest-live´
    /// ´test:unit:is-draft-reads-no-marker-from-a-malformed-bold-line´
    #[test]
    fn is_draft_reads_no_marker_from_a_malformed_bold_line() {
        let variants = [
            "# The manifest\n\n**Draft**\n\n| Part |\n| --- |\n",
            "# The manifest\n\n**DRAFT.**\n\n| Part |\n| --- |\n",
            "# The manifest\n\n**Draft.** and more\n\n| Part |\n| --- |\n",
            "# The manifest\n\nDraft.\n\n| Part |\n| --- |\n",
        ];

        for source in variants {
            assert!(!is_draft(source), "got a marker from {source:?}");
        }
    }

    /// A draft manifest still assembles its parts but no freshness verdict is
    /// formed about the publication, so a rewrite in progress is not required
    /// to republish after every edit.
    ///
    /// ´claim:assembly:a-draft-assembles-without-being-judged-for-freshness´
    /// ´test:unit:a-draft-manifest-forms-no-freshness-verdict´
    #[test]
    fn a_draft_manifest_forms_no_freshness_verdict() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());
        fs::write(
            root.path().join(PARTS).join(MANIFEST_FILE),
            draft_manifest_source(&both_rows()),
        )
        .expect("write");

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert!(assembled.draft, "the marker is present");
        assert!(
            assembled.text.is_some(),
            "membership still assembles the parts"
        );
        assert_eq!(
            codes(&findings),
            Vec::<&str>::new(),
            "no freshness verdict is formed in draft mode"
        );
    }

    /// Draft mode suspends the freshness verdict and nothing else: a part no
    /// row names is still reported while the manifest is a draft, so the
    /// membership relation is checked throughout a rewrite.
    ///
    /// ´claim:assembly:draft-mode-suspends-freshness-alone-and-membership-still-binds´
    /// ´test:unit:draft-manifest-still-checks-membership´
    #[test]
    fn draft_manifest_still_checks_membership() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());
        fs::write(
            root.path().join(PARTS).join(MANIFEST_FILE),
            draft_manifest_source("| ``alpha.md`` |"),
        )
        .expect("write");

        let (assembled, findings) = verify_assembly(root.path(), &assembly());

        assert!(assembled.draft);
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, Finding::UnassembledPart { part, .. } if part == "beta.md")),
            "membership still fires in draft mode: {findings:?}"
        );
    }

    /// Freshness binds again the moment the marker is removed: one and the same
    /// manifest reports nothing as a draft and reports staleness once live, so
    /// a rewrite is finished by deleting a line.
    ///
    /// ´claim:assembly:removing-the-draft-marker-restores-the-freshness-verdict´
    /// ´test:unit:removing-the-marker-restores-freshness´
    #[test]
    fn removing_the_marker_restores_freshness() {
        let root = tempfile::tempdir().expect("temporary directory");
        fixture(root.path(), &both_rows());
        fs::write(
            root.path().join(PARTS).join(MANIFEST_FILE),
            draft_manifest_source(&both_rows()),
        )
        .expect("write");

        let (draft_assembled, draft_findings) = verify_assembly(root.path(), &assembly());

        assert!(draft_assembled.draft);
        assert_eq!(codes(&draft_findings), Vec::<&str>::new());

        fs::write(
            root.path().join(PARTS).join(MANIFEST_FILE),
            manifest_source(&both_rows()),
        )
        .expect("write");

        let (live_assembled, live_findings) = verify_assembly(root.path(), &assembly());

        assert!(!live_assembled.draft, "the marker is gone");
        assert_eq!(
            codes(&live_findings),
            ["stale_assembly"],
            "freshness binds again"
        );
    }

    /// An invented publication set: two owners, two parts directories, two
    /// documents. Nothing here names a corpus this repository has.
    fn publications() -> Publications {
        Publications::new([
            Publication::new("ALPHA", "books/alpha/parts", "books/alpha.md"),
            Publication::new("BETA", "books/beta/parts", "books/beta.md"),
        ])
    }

    /// A publication row carries its owner, its parts and its target together,
    /// so every consumer of the row is attributing the parts and the document
    /// the same way. Recovering the owner from the parts path at each use was
    /// the alternative, and two derivations of one relation are two chances to
    /// disagree about it.
    ///
    /// ´claim:assembly:a-publication-row-carries-its-owner-with-its-parts-and-target´
    /// ´test:unit:reads-a-publication-row-as-owner-parts-and-target´
    #[test]
    fn reads_a_publication_row_as_owner_parts_and_target() {
        let rows = publications();
        let first = &rows.rows()[0];

        assert_eq!(first.owner(), "ALPHA");
        assert_eq!(first.assembly().parts(), Path::new("books/alpha/parts"));
        assert_eq!(first.assembly().target(), Path::new("books/alpha.md"));
        assert_eq!(
            first.assembly().manifest(),
            Path::new("books/alpha/parts").join(MANIFEST_FILE),
            "the manifest is derived from the parts rather than declared beside them"
        );
    }

    /// The generated-document set is the targets of the rows, derived rather
    /// than declared beside them. A generated document mints nothing of its
    /// own — its labels are its parts', already read where they are
    /// maintained — so a second list of generated documents would be a second
    /// source for one truth, free to fall out of step with the rows.
    ///
    /// ´claim:assembly:the-generated-documents-are-derived-from-the-publication-rows´
    /// ´test:unit:derives-the-generated-documents-from-the-rows´
    #[test]
    fn derives_the_generated_documents_from_the_rows() {
        assert_eq!(
            publications().generated_targets(),
            [Path::new("books/alpha.md"), Path::new("books/beta.md")]
        );
        assert_eq!(
            Publications::new([]).generated_targets(),
            Vec::<std::path::PathBuf>::new(),
            "and a corpus publishing nothing generates nothing"
        );
    }

    /// Well-formed rows fail nothing, and the two ways they can fail are derived
    /// from the rows alone. Two sets of parts claiming one target make the
    /// published bytes depend on which ran last, so the freshness comparison has
    /// no answer rather than a wrong one; and a target inside its own parts
    /// directory would be a part of itself, so writing it would change the input
    /// it was assembled from and no fixed point exists.
    ///
    /// ´claim:assembly:the-publication-invariants-are-derived-from-the-rows-alone´
    /// ´test:unit:derives-the-generator-and-containment-defects-from-the-rows´
    #[test]
    fn derives_the_generator_and_containment_defects_from_the_rows() {
        assert_eq!(
            publications().defects(),
            Vec::new(),
            "two owners, two targets, no defect"
        );

        let contested = Publications::new([
            Publication::new("ALPHA", "books/alpha/parts", "books/shared.md"),
            Publication::new("BETA", "books/beta/parts", "books/shared.md"),
        ]);

        assert_eq!(
            contested.defects(),
            vec![PublicationDefect::TwoGenerators {
                target: "books/shared.md".into(),
                parts: vec!["books/alpha/parts".into(), "books/beta/parts".into()],
            }]
        );

        let swallowed = Publications::new([Publication::new(
            "ALPHA",
            "books/alpha",
            "books/alpha/whole.md",
        )]);

        assert_eq!(
            swallowed.defects(),
            vec![PublicationDefect::TargetInsideParts {
                target: "books/alpha/whole.md".into(),
                parts: "books/alpha".into(),
            }]
        );
    }

    /// The publications program declares the fingerprint codec: a defect is a
    /// relation between an owner, a parts directory and a target rather than an
    /// occurrence inside a file, so two defects over one document stay two
    /// identities instead of collapsing into one tolerated row.
    ///
    /// ´claim:assembly:the-publications-program-identifies-a-violation-by-a-digest´
    /// ´test:unit:the-publications-program-fingerprints-its-defects´
    #[test]
    fn the_publications_program_fingerprints_its_defects() {
        assert_eq!(Publications::codec(), Codec::Fingerprint);
        assert_eq!(Publications::codec().field(), "allowances");
    }
}
