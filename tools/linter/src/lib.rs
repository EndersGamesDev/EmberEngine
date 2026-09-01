// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Repository-wide label linter for the label calculus of ADR-T-014.
//!
//! The calculus governs a reference graph over the corpus: heads mint labels,
//! running text cites them, and a checker decides whether every mint is unique
//! and every citation resolves. This crate is that checker. Where these comments
//! name an environment of the governing document they cite it, in the code
//! surface's own delimiters and by the import form its owner registers, so that
//! a reference the reader can follow is one the checker resolves too. Until wave
//! N1 gave code a carrier they could only be named in ordinary prose, and a name
//! written that way resolves nothing: it read as a citation while reaching
//! nobody, which is the silence this crate exists to refuse. The linter's own
//! source still mints nothing while describing what it implements.
//!
//! The prose side covers the label language, Markdown scanning, the two-pass
//! engine, and the reference graph, over a carrier that reaches the repository's
//! own prose and every package's beside it, partitioned into the owners ADR-T-015
//! already gives the code. Above the spans stand the environment heads: a reader
//! that pairs each head with the mint naming its environment, and the kind
//! registry of ADR-T-011, read from that document's own tables, against which
//! every head is validated. Beside all of it stands the first inventory profile,
//! ADR-T-015's test profile: a census taken from the abstract syntax of every
//! workspace member, a classification, a derivation, validation of the standard
//! place, and a fix mode that writes the labels the check then enforces.
//!
//! Wave T2 adds the three capabilities a documentation migration needs beside
//! the check. Outline tracking makes an outline document and the document it
//! outlines answerable to each other, in both directions and by declaration
//! rather than by inference. The graph report answers the review questions the
//! check has no verdict for — what nothing cites, what everything cites, how the
//! corpus is spread — as a report and never as a gate. And the migration lint
//! reads the reference forms the calculus supersedes, held per document and rule
//! by rule, so a corpus part-way through a campaign can hold its migrated
//! documents to a standard its unmigrated ones do not yet meet.
//!
//! Wave T3 adds the two the rewrite itself needs. Part-file assembly lets a
//! document too large to rewrite in one pass be written in parts, each
//! independently lintable, and concatenated into one committed publication in an
//! order its authors declare — with the publication held to the exact-byte
//! freshness of ADR-T-012 rather than to anybody's memory. And the shape report
//! measures what "short and super sharp" has never had numbers for: heads per
//! document, words and citations per environment, against a band measured from
//! the records the campaign is content with rather than against a constant.
//!
//! Wave TP1 adds the second inventory profile, ADR-T-016's to-do profile, whose
//! assets are the deficiency notices a source carries in its comments. It brings
//! the local extension of the kind registry that its kind needed, and a fourth
//! burn list: the notices that carry no label yet are counted and ratcheted
//! rather than reported, so the corpus's standing debt is recorded while a notice
//! written unlabelled after the profile landed fails the check as growth.
//!
//! Wave S5 adds the second of the campaign's audit instruments, the claim
//! coverage report. It joins the two registries a claim can stand in — the census
//! over test comments, and the carrier over readme prose — and reports the two
//! facts neither holds alone: which statements no sibling test cites, and which
//! intents no test witnesses at all. That second fact is what lets a document of
//! ninety-one hand-tracked promises retire, because a promise that survives the
//! retirement as a claim is a promise a query can find.
//!
//! Wave N1 settles what the two carriers come to, on the owner's ruling of
//! 2026-08-20: there is one resolution space, and a label is citable from either
//! surface whatever kind it is and whichever surface minted it. The code carrier
//! is the half that was missing — it hands the engine every mint standing in
//! code, so a notice and a claim become citable at last, and it reads the
//! citations a package's commentary stands, so code may cite as prose does. A
//! profile's standard place stays that profile's own.
//!
//! Wave C1 adds the third inventory profile, ADR-T-018's constant profile, and
//! with it the calculus's first adopted standard place for a generated mint. Its
//! assets are the screaming constants a package's production sources declare, and
//! what it adds to the corpus is a value that answers for itself: a warrant citing
//! the record that fixed it, an identity naming the concept it fixes rather than
//! the identifier the code may refactor, a citation of the program that governs
//! its shape, and a generated pin that re-mints when the value moves — so every
//! document citing that value dangles in the commit that moved it. The nine
//! programs are documented algorithms minting beside their implementations, and
//! the sweep that writes the pins writes nothing else.
//!
//! Synthetic citations, anchor harvest, and generated registers are later waves.
//! The module seams are already in place.

mod abnf;
mod adoption;
mod application;
mod areas;
mod assembly;
mod burn;
mod carrier;
mod catalogue;
mod census;
mod claim;
mod code;
mod comment;
mod commentary;
mod constant;
mod control;
mod coverage;
mod declaration;
mod depend;
mod engine;
mod finding;
mod fix;
mod fmt;
mod graph;
mod head;
mod index;
mod interchange;
mod label;
mod layers;
mod leader;
mod legacy;
mod legacy_profile;
mod matrix;
mod occurrence;
mod outline;
mod partition;
mod pattern;
mod plan;
mod profile;
mod program;
mod prose;
mod reference;
mod registry;
mod report;
mod residual;
mod retired;
mod roster;
mod selection;
mod shape;
mod snapshot;
mod spdx;
mod subscribe;
mod surface;
mod todo;
mod token;
mod universe;
mod workspace;

#[cfg(test)]
pub(crate) mod test_support;

#[cfg(test)]
mod tests;

pub use abnf::{Grammar, GrammarDefect, UnknownRule};
pub use adoption::{Adoption, Owner, index_adoption};
pub use assembly::{
    Assembled, Assembly, MANIFEST_FILE, PartRow, Publication, PublicationDefect, Publications,
    part_duplicate_mints, read_manifest, verify_assembly, write_assembly,
};
pub use burn::{
    BurnCensus, BurnList, BurnOccurrence, BurnRow, RegisterRow, Shape, census, declared_rows,
    index_burn_lists, verify,
};
pub use carrier::Source;
#[cfg(test)]
pub use carrier::{Carrier, index_carrier};
pub use catalogue::{
    CATALOG, Codec, CompiledProgram, DeclaredKind, Dependency, Policy, ProgramCatalog, Scope,
    catalogued, is_policy_identifier, program_of,
};
pub use census::{
    Census, DocComment, TestFunction, scan_source, take_planned_census as take_census,
};
pub use claim::{
    CLAIM_KIND, ClaimAnalysis, ClaimDefect, ClaimForm, ClaimLine, CoveredClaim, Standing,
    TABLE_DELIMITER, TABLE_HEADER, analyze_claims, closed_waves, line_holds_claim, read_claim,
};
pub use code::{
    CodeCitation, CodeMint, CodeSurface, code_syntax, scan_code_citations,
    take_planned_code_citations as take_code_citations,
};
pub use comment::{CommentRegion, comment_regions};
pub use commentary::{Reader, catalogued as catalogued_reader};
pub use constant::{
    Binding, CONST_KIND, ConstantAnalysis, ConstantCensus, ConstantDeclaration, CoveredConstant,
    Program, analyze_constants, cover_constants, derive_pin, digest_word, natural_slug,
    scan_constants, take_planned_constant_census as take_constant_census, write_pins,
};
pub use control::{
    Anomaly, AppendChange, AppendRefusal, AppendResponse, AppendTarget, AuditResponse, AuditTarget,
    Authority, Comparison, Control, Identity, Operation, Row as ListRow, lower, lower_lists,
    lower_lists_with_plan, maintain, maintain_with_configuration, maintain_with_plan, render_lists,
};
pub use coverage::{
    ClaimCoverage, ClaimSite, CoverageCounts, CoverageSummary, DEFAULT_UNCITED, IntentSite,
    summarise_coverage,
};
pub use declaration::{
    AbnfPattern, Admission, Declaration, DeclarationDefect, DeclaredSet, OwnerDatum,
    OwnerDeclaration, PATTERN_RULE, PatternRow, PrefixNumberEntry, RetiredKey, Scalar, Selection,
    SetEntry, SetKind,
};
pub use depend::{CitedEdge, cited_edges};
pub use engine::{Analysis, CitationEdge, ImportSite, MintNode, ReferenceGraph, Surface, analyze};
pub use finding::{Finding, Location, MissingLabelReason, Severity, UnwarrantedPinReason};
pub use fix::{FixOutcome, fix_profile, fix_todos, is_tree_dirty};
pub use fmt::{FormatChange, FormatError, FormatReport, format_markdown, format_paths};
pub use graph::{
    CitationSite, Coverage, DEFAULT_HUBS, GraphSummary, MintSite, Reverse, dangling, summarise,
};
pub use head::{Head, HeadName, HeadStyle, read_heads, validate_heads};
pub use index::{
    CommittedIndex, INDEX_HEADING, IndexAnalysis, by_source, committed_index,
    region as index_region, verify_indexes, write_index,
};
pub use interchange::SelectionPlan as InterchangeSelectionPlan;
pub use label::{Label, Prefix};
pub use layers::{LayerAnalysis, Reach, derive_reach, verify_layers};
pub use leader::{CATALOG as LEADER_CATALOG, Leader, catalogued as catalogued_leader};
pub use legacy::{LegacyRule, Mark, SECTION_MARK, read_mark, scan_legacy, scan_regions, scan_text};
pub use matrix::{
    CommittedMatrix, MATRIX_KIND, MATRIX_SUFFIX, MatrixAnalysis, MatrixFolder, README,
    assets_by_folder, committed_matrices, derive_matrix, planned_folders as folders,
    region as matrix_region, verify_planned_matrices as verify_matrices, write_matrix,
};
pub use occurrence::{Form, Occurrence, Syntax};
pub use outline::{Outline, TrackingRow, read_outline, validate_tracking};
pub use partition::PartitionCounts;
pub use pattern::{BytePath, PathDefect};
pub use plan::{
    ActivationPlan, AssemblyRun, AssemblyWritePlan, BurnRun, BurnWritePlan, ContentPlan,
    ControlObservationPlan, ControlWritePlan, CorpusPlan, DependencyPlan, ExecutionPlan,
    FixWritePlan, LabelPlan, MigrationPlan, PartitionPlan, PathParticipation, PlanError,
    ProfilePlan, ProfileSource, ProjectionWritePlan, PublicationPlan, TopologyDefect, TopologyPlan,
    WorkspacePlan, WriteGuardPlan,
};
pub use profile::{
    Area, CoveredAsset, ProfileAnalysis, TEST_KIND, analyze_profile, classify, cover, derive,
    transform_name, validate,
};
pub use program::{LiteralDefect, LiteralSet, MarkNumbered, PrefixBound, PrefixNumbers, Sighting};
pub use prose::{BlockKind, ProseBlock, ProseScan, scan_markdown};
pub use reference::{
    Candidate, Lexicon, SelectionPlan as ReferenceSelectionPlan, candidates, cited, cited_in,
    is_self_reference,
};
pub use registry::{HeadDefect, KindRegistry, RUNG_KIND, Row, Status, fixture_kind_registry};
pub use report::{
    AssembleReport, AssembledDocument, AssemblyCounts, BurnCounts, BurnReport, BurnedFamily,
    CheckReport, ConfigurationCounts, CoverageReport, FixReport, GraphReport, Passes,
    ProjectReport, ProjectionOutcome, ReportedFinding, ShapeReport, assemble, assemble_with_plan,
    assemble_with_write_plan, burn, burn_with_plan, burn_with_write_plan, check, check_with,
    check_with_plan, coverage, coverage_with_plan, fix, fix_todo, fix_todo_with_plan,
    fix_todo_with_write_plan, fix_with_plan, fix_with_write_plan, harvest_citations,
    harvest_citations_with_plan, project, project_with_plan, project_with_write_plan, report,
    report_with_plan, shape, shape_with_plan, verify_configuration, verify_configuration_with_plan,
    verify_write_guard,
};
pub use residual::{
    Residual, scan_residual_comments, scan_residual_markdown, scan_residual_script,
    scan_residual_text,
};
pub use retired::{Retired, RetiredFamily, scan_retired_markdown, scan_retired_text};
pub use roster::{OwnerNames, RosterDefect, UnbuiltMember, derive_owner};
pub use shape::{
    Distribution, DocumentShape, EnvironmentShape, ShapeSummary, measure, summarise_shape,
};
pub use snapshot::{
    Allowance, Configuration, DIRECTORY, ENVIRONMENTS_FILE, EnvironmentRow, FILES, LISTS_FILE,
    OWNERS_FILE, OwnerRow, POLICIES_FILE, Pair, PathCount, ReachRow as DeclaredReachRow, Refusal,
    Rows, SHAPE_FILE, Snapshot, configuration,
};
pub use spdx::{
    COPYRIGHT_FIELD, HALVES, Half, HalfSection, IDENTIFIER_FIELD, LISTS, ListKind, Parameters,
    Section, SectionRow, SelectionPlan as SpdxSelectionPlan, is_copyright_text,
    is_licence_expression,
};
pub use subscribe::Subscription;
pub use todo::{
    CoveredNotice, NAME_WORDS, Placement, TODO_KIND, TODO_MARKERS, TodoAnalysis, TodoArea,
    TodoCensus, TodoNotice, analyze_todos, classify_todo, cover_todos, derive_todo, place_label,
    scan_todos, standard_place_text, take_planned_todo_census as take_todo_census,
    transform_summary,
};
pub use token::{
    CodeSpan, Region, Role, angle_regions, markdown_code_spans, markdown_destinations,
    markdown_regions, regions, rust_regions, script_regions, sql_regions,
};
pub use universe::{CorpusShape, IgnoreRow, UniverseKind};
pub use workspace::{Package, pending_packages, prefix_for_crate};
