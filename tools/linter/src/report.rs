// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The check report, and the top-level check itself.
//!
//! The report is this command's stdout result data under ADR-T-010, so it is one
//! JSON object with no human-oriented text outside the `message` fields carried
//! inside the JSON.
//!
//! The object carries no version of its own. Report shape is not versioned here:
//! a reader takes the members it names and ignores the rest, so a member added
//! is a member a reader that did not ask for it never sees, and a member removed
//! is a reader's own dangling reference rather than a number to migrate against.
//! The command contract's register stands as the changelog of what this object
//! has promised, read by people rather than by programs.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`counts_failures_and_warnings_separately`] | report | Failures and warnings are counted apart from one another, and only a failure spoils the verdict: a corpus carrying one of each reports one of each and is not clean, on account of the failure alone. |
//! | [`serializes_findings_with_their_code`] | report | Every finding serialises with a stable machine-readable code beside its severity and the label it concerns, so a consumer can select the findings it cares about without parsing the human sentence. |

use std::path::Path;

use serde::Serialize;

use crate::burn::BurnRow;
use crate::claim::ClaimAnalysis;
use crate::constant::ConstantAnalysis;
use crate::coverage::{CoverageCounts, CoverageSummary};
use crate::depend::CitedEdge;
use crate::engine::Analysis;
use crate::finding::{Finding, Severity};
use crate::fix::FixOutcome;
use crate::graph::GraphSummary;
use crate::index::IndexAnalysis;
use crate::label::Label;
use crate::layers::LayerAnalysis;
use crate::legacy_profile::LegacyAnalysis;
use crate::matrix::MatrixAnalysis;
use crate::partition::PartitionCounts;
use crate::plan::{
    AssemblyWritePlan, BurnWritePlan, ExecutionPlan, FixWritePlan, ProjectionWritePlan,
    WriteGuardPlan,
};
use crate::profile::ProfileAnalysis;
#[cfg(test)]
use crate::roster::OwnerNames;
use crate::shape::ShapeSummary;
#[cfg(test)]
use crate::snapshot::Snapshot;
use crate::snapshot::{Configuration, configuration};
use crate::todo::TodoAnalysis;
#[cfg(test)]
use crate::universe::IgnoreRow;

/// One finding, as it appears in the report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportedFinding {
    /// Whether the finding blocks the check.
    pub severity: Severity,
    /// A human-readable rendering, carried inside the JSON record.
    pub message: String,
    /// The finding itself, tagged by its code.
    #[serde(flatten)]
    pub finding: Finding,
}

impl From<Finding> for ReportedFinding {
    fn from(finding: Finding) -> Self {
        Self {
            severity: finding.severity(),
            message: finding.to_string(),
            finding,
        }
    }
}

/// The stdout result object of the check command.
#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    /// The root the carrier was taken from.
    pub root: String,
    /// How many carrier sources were scanned.
    pub sources_scanned: usize,
    /// How many mints stand in the carrier.
    pub mints: usize,
    /// How many citations resolved.
    pub citations_resolved: usize,
    /// How many environment heads validated against the kind registry.
    pub heads_validated: usize,
    /// How many findings block the check.
    pub failures: usize,
    /// How many findings merely advise.
    pub warnings: usize,
    /// Whether the carrier is in good standing.
    pub clean: bool,
    /// What the test profile found over the Rust carrier.
    pub profile: ProfileAnalysis,
    /// What the to-do profile found over the Rust carrier.
    pub todo: TodoAnalysis,
    /// What the legacy-implementation profile found over production Rust.
    pub legacy: LegacyAnalysis,
    /// What the claim profile of ADR-T-017 found, and what it counted rather than reported.
    pub claim: ClaimAnalysis,
    /// What the constant profile of ADR-T-018 found over the Rust carrier.
    pub constant: ConstantAnalysis,
    /// What the claim coverage came to: statements written, and intents nobody has kept.
    pub coverage: CoverageCounts,
    /// What the in-file test indexes were found to be.
    pub index: IndexAnalysis,
    /// What the per-folder test matrices were found to be.
    pub matrix: MatrixAnalysis,
    /// What the assembled publications were found to be.
    pub assembly: AssemblyCounts,
    /// What the burn lists were censused at.
    pub burn: BurnCounts,
    /// What the layer owner graph of ADR-T-019 was found to be, and what it refused.
    pub layers: LayerAnalysis,
    /// What the declared configuration was found to be.
    ///
    /// Lower-level verification may omit it only when handed an already-refused
    /// configuration; every compiled command plan carries it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<ConfigurationCounts>,
    /// Every finding, ordered by source and position.
    pub findings: Vec<ReportedFinding>,
}

/// What the burn pass found, in counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BurnCounts {
    /// How many burn lists the adoption declares.
    pub declared: usize,
    /// How many sources their censuses read, counted once per list.
    pub files_scanned: usize,
    /// How many occurrences stand over every family.
    pub occurrences: usize,
}

/// What the assembly pass found, in counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssemblyCounts {
    /// How many pairs the adoption declares.
    pub declared: usize,
    /// How many of them have no parts directory yet.
    pub dormant: usize,
    /// How many of them were assembled and compared.
    pub verified: usize,
    /// How many parts stood in those assemblies.
    pub parts: usize,
}

/// What each pass over the Rust carrier found, gathered for the report.
///
/// The passes travel together because the report presents them together, and
/// because a constructor taking one argument per pass would grow by one every
/// time a record adds a pass — which is the shape this corpus is in.
#[derive(Debug, Clone, Default)]
pub struct Passes {
    /// What the test profile of ADR-T-015 found.
    pub profile: ProfileAnalysis,
    /// What the to-do profile of ADR-T-016 found.
    pub todo: TodoAnalysis,
    /// What the marked legacy-implementation profile found.
    pub legacy: LegacyAnalysis,
    /// What the claim profile of ADR-T-017 found.
    pub claim: ClaimAnalysis,
    /// What the constant profile of ADR-T-018 found.
    pub constant: ConstantAnalysis,
    /// What the claim coverage came to, in the figures a check report carries.
    pub coverage: CoverageCounts,
    /// What the in-file test indexes were found to be.
    pub index: IndexAnalysis,
    /// What the per-folder test matrices were found to be.
    pub matrix: MatrixAnalysis,
}

/// What the whole-tree verifications found, gathered for the report.
///
/// These four are not passes over the Rust carrier — they read the tree's own
/// documents and manifests — so they travel beside [`Passes`] rather than in it.
/// They travel grouped for the reason that record already gives about the
/// passes: a constructor taking one argument per verification would grow by one
/// every time a record adds a verification, which is the shape this corpus is in.
#[derive(Debug, Clone, Default)]
pub struct Verdicts {
    /// What the assembled publications were found to be.
    pub assembly: AssemblyCounts,
    /// What the burn lists were censused at.
    pub burn: BurnCounts,
    /// What the layer owner graph of ADR-T-019 was found to be.
    pub layers: LayerAnalysis,
    /// What the declared configuration was found to be, when one stands.
    pub configuration: Option<ConfigurationCounts>,
}

impl CheckReport {
    /// Build a report from an analysis and any findings raised before it.
    #[must_use]
    pub fn new(
        root: &Path,
        analysis: &Analysis,
        passes: Passes,
        verdicts: Verdicts,
        carrier_findings: Vec<Finding>,
    ) -> Self {
        let Verdicts {
            assembly,
            burn,
            layers,
            configuration,
        } = verdicts;

        let Passes {
            profile,
            todo,
            legacy,
            claim,
            constant,
            coverage,
            index,
            matrix,
        } = passes;

        let findings: Vec<ReportedFinding> = carrier_findings
            .into_iter()
            .chain(analysis.findings().iter().cloned())
            .map(ReportedFinding::from)
            .collect();

        let failures = findings
            .iter()
            .filter(|reported| reported.severity == Severity::Failure)
            .count();

        Self {
            root: root.to_string_lossy().into_owned(),
            sources_scanned: analysis.sources_scanned(),
            mints: analysis.mints(),
            citations_resolved: analysis.citations_resolved(),
            heads_validated: analysis.heads_validated(),
            failures,
            warnings: findings.len() - failures,
            clean: failures == 0,
            profile,
            todo,
            legacy,
            claim,
            constant,
            coverage,
            index,
            matrix,
            assembly,
            burn,
            layers,
            configuration,
            findings,
        }
    }
}

/// Run the check over a repository root.
///
/// The passes run in the order the two-pass invariant
/// (ADR-T-014, A calculus of documentation and source labels) fixes: the adoption data are loaded first —
/// and the signature is part of them, so the workspace is read before anything
/// is scanned — then the carriers are harvested, and only then is anything
/// resolved or validated against completed registries.
///
/// Every census of the Rust sources runs before the carrier rather than beside
/// it, because what they find is what the engine seeds the registries with: a
/// document may cite a test, a notice, or a claim, and the mint that citation
/// reaches has to be in hand before the harvest that will resolve against it.
/// The commentary's own citations are gathered in the same sweep, so that one
/// resolution pass sees both surfaces at once.
#[must_use]
pub fn check(root: &Path) -> CheckReport {
    crate::application::check(root)
}

/// Run the check over a repository root against a configuration already read.
///
/// The command reads the configuration itself, because a refused snapshot is a
/// refused precondition and the binary exits before any of this runs. Tests
/// using this lower-level entry point provide a parsed declaration explicitly.
#[must_use]
pub fn check_with(root: &Path, declared: &Configuration) -> CheckReport {
    crate::application::check_with(root, declared)
}

/// Run the check through one already-compiled execution plan.
#[must_use]
pub fn check_with_plan(plan: &ExecutionPlan) -> CheckReport {
    crate::application::check_with_plan(plan)
}

/// What the declared configuration was found to be, in counts.
///
/// The object is optional only for lower-level verification handed a refused
/// configuration. Every compiled command plan carries the parsed snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConfigurationCounts {
    /// How many owners the declaration registers.
    pub owners: usize,
    /// How many partition rows it carries.
    ///
    /// The member keeps the spelling the surface retired, because a report member
    /// is read by things outside this repository and renaming one changes the
    /// report's shape rather than a declaration. The two spellings part company
    /// here deliberately, and closing the gap is a ruling about the report rather
    /// than a consequence of the one that moved the key.
    pub inclusions: usize,
    /// How many exclusion rules it carries.
    pub excluded: usize,
    /// How many environment-and-kind rows it carries.
    pub environments: usize,
    /// How many owner-and-policy pairs it activates.
    pub pairs: usize,
    /// How many tolerated rows stand over every pair's list.
    pub rows: usize,
    /// What the owner partition over the physical tree came to.
    pub partition: PartitionCounts,
    /// How many activated pairs want a prerequisite pair nobody activated.
    pub missing_dependencies: usize,
}

/// Harvest the cross-owner citations a dependency validation instantiates from.
///
/// A caller that has already analysed the carrier passes its own citations
/// instead; this is for the writing modes, which must answer the same question
/// before they mutate and have no analysis of their own to answer it from.
#[must_use]
pub fn harvest_citations(root: &Path) -> Vec<CitedEdge> {
    crate::application::harvest_citations(root)
}

/// Harvest dependency citations through one already-compiled execution plan.
#[must_use]
pub fn harvest_citations_with_plan(plan: &ExecutionPlan) -> Vec<CitedEdge> {
    crate::application::harvest_citations_with_plan(plan)
}

/// Judge a parsed snapshot against the repository it describes.
///
/// This is the second of the two questions a snapshot faces, and the one that is
/// judged rather than refused: the snapshot is well-formed, and what may be wrong
/// is its description of the tree. A refused declaration answers with no counts
/// and no findings — the refusal is the command's business and it exits before
/// reaching here.
#[must_use]
pub fn verify_configuration(
    root: &Path,
    configuration: &Configuration,
    citations: &[CitedEdge],
) -> (Option<ConfigurationCounts>, Vec<Finding>) {
    crate::application::verify_configuration(root, configuration, citations)
}

/// Judge one execution plan against its materialized corpus.
#[must_use]
pub fn verify_configuration_with_plan(
    plan: &ExecutionPlan,
    citations: &[CitedEdge],
) -> (Option<ConfigurationCounts>, Vec<Finding>) {
    crate::application::verify_configuration_with_plan(plan, citations)
}

/// Judge the common pre-write guard, including cited-owner dependencies.
///
/// The label pass is part of this typed guard rather than a second command-like
/// harvest beside the writer. Its result is consumed immediately and exposes no
/// independent run surface.
#[must_use]
pub fn verify_write_guard(plan: WriteGuardPlan<'_>) -> (Option<ConfigurationCounts>, Vec<Finding>) {
    crate::application::verify_write_guard(plan)
}

/// One burn list, as the burn command reports it.
#[derive(Debug, Clone, Serialize)]
pub struct BurnedFamily {
    /// The family the list counts.
    pub family: String,
    /// The prose trees the family was counted over.
    pub prose: Vec<String>,
    /// The Rust trees whose comments the family was counted over.
    pub code: Vec<String>,
    /// The trees the census never read, whatever surfaces were declared.
    pub excluded: Vec<String>,
    /// How many sources the census read.
    pub files_scanned: usize,
    /// How many occurrences stand over the surfaces.
    pub occurrences: usize,
    /// How many files hold at least one.
    pub files_holding: usize,
    /// How many occurrences the declared ratchet accounts for.
    pub registered: usize,
    /// The census, one row per file holding an occurrence.
    pub rows: Vec<BurnRow>,
}

/// The stdout result object of the burn command.
#[derive(Debug, Clone, Serialize)]
pub struct BurnReport {
    /// The root the censuses were taken over.
    pub root: String,
    /// Whether the command was allowed to write.
    pub write: bool,
    /// Every adopted burn list, in declaration order.
    pub families: Vec<BurnedFamily>,
    /// How many findings block the command.
    pub failures: usize,
    /// Every finding, ordered by source and position.
    pub findings: Vec<ReportedFinding>,
}

/// Census every adopted burn list, verifying or regenerating its register.
///
/// Without `write` the command forms no side effect: it is the ratchet's own
/// judgment and nothing else, which is the mode the check runs.
///
/// With it the registers are rewritten, and the two directions part company. A
/// register that overstated is repaired, and the entry it overstated by is no
/// longer reported: recording a family's retreat is the migration's ordinary
/// commit, and the decision requires only that the register shrink in the same
/// commit as the occurrences, which is exactly what has happened. Growth is
/// reported still, and the command exits on it, because no write may certify
/// what the ratchet forbids — a register may record debt, and running the
/// generator is not a way to take some on.
#[must_use]
pub fn burn(root: &Path, write: bool) -> BurnReport {
    let plan = compatibility_plan(root);
    crate::application::burn_with_write_plan(plan.burn_write(), write)
}

/// Census migration families through one already-compiled execution plan.
#[must_use]
pub fn burn_with_plan(plan: &ExecutionPlan, write: bool) -> BurnReport {
    crate::application::burn_with_plan(plan, write)
}

/// Census migration families through the narrow burn write plan.
#[must_use]
pub fn burn_with_write_plan(plan: BurnWritePlan<'_>, write: bool) -> BurnReport {
    crate::application::burn_with_write_plan(plan, write)
}

/// The stdout result object of the report command.
///
/// The report is informational: it restates what the graph is, and decides
/// nothing. Its exit status therefore never carries the findings class the check
/// owns — see the command's own contract, where the taxonomy is written down.
#[derive(Debug, Clone, Serialize)]
pub struct GraphReport {
    /// The root the carrier was taken from.
    pub root: String,
    /// How many carrier sources were scanned.
    pub sources_scanned: usize,
    /// What the reference graph looks like.
    #[serde(flatten)]
    pub summary: GraphSummary,
    /// Every citation the check found reaching no mint.
    pub dangling: Vec<ReportedFinding>,
}

/// Report the reference graph over a repository root.
///
/// The passes run exactly as the check runs them, because the graph a report
/// describes must be the graph the check decided on; anything else would let a
/// reviewer read one corpus and a gate judge another.
#[must_use]
pub fn report(root: &Path, label: Option<&Label>, hubs: usize) -> GraphReport {
    crate::application::report(root, label, hubs)
}

/// Report the reference graph through one already-compiled execution plan.
#[must_use]
pub fn report_with_plan(plan: &ExecutionPlan, label: Option<&Label>, hubs: usize) -> GraphReport {
    crate::application::report_with_plan(plan, label, hubs)
}

/// The stdout result object of the shape command.
///
/// The report measures and decides nothing, exactly as the graph report does, so
/// its exit status never carries the findings class the check owns.
#[derive(Debug, Clone, Serialize)]
pub struct ShapeReport {
    /// The root the carrier was taken from.
    pub root: String,
    /// How many carrier sources were scanned.
    pub sources_scanned: usize,
    /// What the corpus's documents and environments measure.
    #[serde(flatten)]
    pub summary: ShapeSummary,
}

/// Report the shape of a repository root's documents and environments.
#[must_use]
pub fn shape(root: &Path) -> ShapeReport {
    crate::application::shape(root)
}

/// Report document shape through one already-compiled execution plan.
#[must_use]
pub fn shape_with_plan(plan: &ExecutionPlan) -> ShapeReport {
    crate::application::shape_with_plan(plan)
}

/// The stdout result object of the coverage command.
///
/// The report is informational in the way the graph report is: it restates what
/// the claims came to and decides nothing. An intent nobody has kept and a
/// statement nobody cites are both ordinary, so the command's exit status never
/// carries the findings class the check owns.
#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    /// The root the claims were counted over.
    pub root: String,
    /// How many carrier sources were scanned for intents.
    pub sources_scanned: usize,
    /// What the claims of the corpus came to.
    #[serde(flatten)]
    pub summary: CoverageSummary,
}

/// Report the claim coverage over a repository root.
///
/// The passes run exactly as the check runs them, for the reason ADR-T-017 gives
/// in as many words: the census the coverage report counts is the same code the
/// check validates against, because a report counting one thing while a gate
/// judged another is a measurement nobody can act on.
///
/// The limit bounds the uncited listing alone. The intents are listed entire,
/// because enumerating the promises nobody has kept is the whole of what this
/// report is for.
#[must_use]
pub fn coverage(root: &Path, uncited: usize) -> CoverageReport {
    crate::application::coverage(root, uncited)
}

/// Report claim coverage through one already-compiled execution plan.
#[must_use]
pub fn coverage_with_plan(plan: &ExecutionPlan, uncited: usize) -> CoverageReport {
    crate::application::coverage_with_plan(plan, uncited)
}

/// One assembled publication, as the assemble command reports it.
#[derive(Debug, Clone, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "four independent flags of one assembly's state, not a state machine"
)]
pub struct AssembledDocument {
    /// The parts directory.
    pub parts: String,
    /// The manifest declaring the order.
    pub manifest: String,
    /// The published document.
    pub target: String,
    /// Whether the parts directory is not there yet.
    pub dormant: bool,
    /// Whether the manifest carries the draft marker.
    ///
    /// A draft assembly forms no freshness verdict and refuses `--write`.
    pub draft: bool,
    /// How many parts were assembled.
    pub assembled: usize,
    /// How many bytes the assembly came to, when one was formed.
    pub bytes: Option<usize>,
    /// Whether the committed publication already carried those exact bytes.
    pub fresh: bool,
    /// Whether this run wrote the publication.
    pub written: bool,
}

/// The stdout result object of the assemble command.
#[derive(Debug, Clone, Serialize)]
pub struct AssembleReport {
    /// The root the assemblies were taken over.
    pub root: String,
    /// Whether the command was allowed to write.
    pub write: bool,
    /// Every adopted assembly, in declaration order.
    pub assemblies: Vec<AssembledDocument>,
    /// How many findings block the command.
    pub failures: usize,
    /// Every finding, ordered by source and position.
    pub findings: Vec<ReportedFinding>,
}

/// Assemble every adopted publication, verifying or writing it.
///
/// Without `write` the command forms no side effect at all: it is the exact-byte
/// freshness comparison of ADR-T-012 and nothing else, which is the mode a gate
/// runs. With it, every publication is rewritten to what its parts say it is, and
/// the freshness findings of that same run are the ones the write repaired.
#[must_use]
pub fn assemble(root: &Path, write: bool) -> AssembleReport {
    let plan = compatibility_plan(root);
    crate::application::assemble_with_write_plan(plan.assembly_write(), write)
}

/// Verify publications through one already-compiled execution plan.
#[must_use]
pub fn assemble_with_plan(plan: &ExecutionPlan, write: bool) -> AssembleReport {
    crate::application::assemble_with_plan(plan, write)
}

/// Verify publications through their typed write subplan.
#[must_use]
pub fn assemble_with_write_plan(plan: AssemblyWritePlan<'_>, write: bool) -> AssembleReport {
    crate::application::assemble_with_write_plan(plan, write)
}

/// What one projection sweep did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectionOutcome {
    /// How many surfaces the sweep considered.
    pub considered: usize,
    /// How many were already what their labels say they are.
    pub unchanged: usize,
    /// How many were rewritten to what their labels say.
    pub rewritten: usize,
    /// How many gained a projection they did not have.
    pub bootstrapped: usize,
}

/// The stdout result object of the project command.
///
/// The command takes the check's own taxonomy, because in its default mode it is
/// one: it regenerates both projections and compares them to the committed bytes,
/// writing nothing. With `--write` the projections are rewritten first, and the
/// staleness it repaired is no longer reported.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectReport {
    /// The root the projections were taken over.
    pub root: String,
    /// Whether the command was allowed to write.
    pub write: bool,
    /// What the in-file test indexes came to.
    pub index: ProjectionOutcome,
    /// What the per-folder test matrices came to.
    pub matrix: ProjectionOutcome,
    /// What the constant pins of ADR-T-018 came to.
    pub constant: ProjectionOutcome,
    /// How many findings block the command.
    pub failures: usize,
    /// Every finding, ordered by source and position.
    pub findings: Vec<ReportedFinding>,
}

/// Verify, or regenerate, three generated surfaces: both projections of ADR-T-017 and
/// the constant pins of ADR-T-018.
///
/// The two travel together because the record holds them to one contract: they are
/// computed from the same labels by the same rules, and a run that refreshed one
/// while leaving the other stale would leave a reader two tables disagreeing about
/// one test.
///
/// Without `write` the command forms no side effect at all, which is the mode the
/// check runs. With it, every projection is rewritten to what the labels say it
/// is, the bootstrap included: a source with no index gains one, and a folder with
/// no readme gains one.
#[must_use]
pub fn project(root: &Path, write: bool) -> ProjectReport {
    let plan = compatibility_plan(root);
    crate::application::project_with_write_plan(&plan.projection_write(), write)
}

/// Verify generated projections through one already-compiled execution plan.
#[must_use]
pub fn project_with_plan(plan: &ExecutionPlan, write: bool) -> ProjectReport {
    crate::application::project_with_plan(plan, write)
}

/// Verify or regenerate projections through their narrow write plan.
#[must_use]
pub fn project_with_write_plan(plan: &ProjectionWritePlan<'_>, write: bool) -> ProjectReport {
    crate::application::project_with_write_plan(plan, write)
}

/// Project against an already-read reconciliation.
///
/// The split is the check command's: a caller holding the declaration passes it,
/// and the convenience form reads one. A matrix label opens with the owner
/// spelling its package derives, so a corpus that has not said how its crate
/// names become owner spellings has not said what its matrices are called.
///
/// The snapshot carries the activations and partition the projections are
/// routed by. The optional form exists only for compatibility tests, which
/// compile a complete fictional snapshot when none is supplied.
#[must_use]
#[cfg(test)]
pub fn project_with(
    root: &Path,
    names: Option<&OwnerNames>,
    ignore: &[IgnoreRow],
    snapshot: Option<&Snapshot>,
    write: bool,
) -> ProjectReport {
    crate::application::project_with(root, names, ignore, snapshot, write)
}

/// The stdout result object of the fix command.
#[derive(Debug, Clone, Serialize)]
pub struct FixReport {
    /// The root the sweep was taken over.
    pub root: String,
    /// The profile whose labels were written.
    pub profile: &'static str,
    /// Whether the sweep only reported what it would do.
    pub dry_run: bool,
    /// What the sweep did.
    #[serde(flatten)]
    pub outcome: FixOutcome,
    /// How many findings the sweep raised.
    pub failures: usize,
    /// Every finding, ordered by source and position.
    pub findings: Vec<ReportedFinding>,
}

/// Sweep a repository root, writing the test profile's labels.
#[must_use]
pub fn fix(root: &Path, dry_run: bool) -> FixReport {
    let plan = compatibility_plan(root);
    crate::application::fix_with_write_plan(plan.fix_write(), dry_run)
}

/// Sweep test labels through one already-compiled execution plan.
#[must_use]
pub fn fix_with_plan(plan: &ExecutionPlan, dry_run: bool) -> FixReport {
    crate::application::fix_with_plan(plan, dry_run)
}

/// Sweep test labels through their typed write subplan.
#[must_use]
pub fn fix_with_write_plan(plan: FixWritePlan<'_>, dry_run: bool) -> FixReport {
    crate::application::fix_with_write_plan(plan, dry_run)
}

/// Sweep a repository root, writing the to-do profile's labels.
///
/// The same report as the test profile's sweep, over the second inventory
/// profile's assets, and naming the profile it swept so that a reader of the
/// JSON never has to infer which of the two produced it.
#[must_use]
pub fn fix_todo(root: &Path, dry_run: bool) -> FixReport {
    let plan = compatibility_plan(root);
    crate::application::fix_todo_with_write_plan(plan.fix_write(), dry_run)
}

/// Sweep to-do labels through one already-compiled execution plan.
#[must_use]
pub fn fix_todo_with_plan(plan: &ExecutionPlan, dry_run: bool) -> FixReport {
    crate::application::fix_todo_with_plan(plan, dry_run)
}

/// Sweep to-do labels through their typed write subplan.
#[must_use]
pub fn fix_todo_with_write_plan(plan: FixWritePlan<'_>, dry_run: bool) -> FixReport {
    crate::application::fix_todo_with_write_plan(plan, dry_run)
}

fn compatibility_plan(root: &Path) -> ExecutionPlan {
    ExecutionPlan::compile(root, configuration(root))
        .unwrap_or_else(|error| panic!("the execution plan did not compile: {error:?}"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CheckReport, Passes, Verdicts};
    use crate::adoption::{Adoption, index_adoption as build_index_adoption};
    use crate::carrier::Source;
    use crate::code::CodeSurface;
    use crate::engine::analyze;

    fn index_adoption(
        packages: &[crate::workspace::Package],
        names: Option<&crate::roster::OwnerNames>,
        assemblies: &[crate::assembly::Assembly],
    ) -> Adoption {
        build_index_adoption(
            packages,
            names,
            assemblies,
            crate::registry::fixture_kind_registry(),
        )
    }

    fn report_for(sources: &[Source]) -> CheckReport {
        let analysis = analyze(
            &index_adoption(
                &[],
                Some(&crate::roster::OwnerNames::new(
                    "ember-",
                    [crate::roster::UnbuiltMember::new(
                        "ember-notime",
                        "packages/notime",
                    )],
                )),
                &[],
            ),
            sources,
            &CodeSurface::default(),
        );
        CheckReport::new(
            Path::new("."),
            &analysis,
            Passes::default(),
            Verdicts::default(),
            Vec::new(),
        )
    }

    /// Failures and warnings are counted apart from one another, and only a
    /// failure spoils the verdict: a corpus carrying one of each reports one of
    /// each and is not clean, on account of the failure alone.
    ///
    /// ´claim:report:failures-and-warnings-are-counted-apart´
    /// ´test:unit:counts-failures-and-warnings-separately´
    #[test]
    fn counts_failures_and_warnings_separately() {
        let sources = [Source::new(
            "one.md",
            "Cites (`sec:demo:missing`) and nearly (`Sec:demo:head`).\n",
        )];
        let report = report_for(&sources);

        assert_eq!(report.failures, 1);
        assert_eq!(report.warnings, 1);
        assert!(!report.clean);
    }

    /// Every finding serialises with a stable machine-readable code beside its
    /// severity and the label it concerns, so a consumer can select the
    /// findings it cares about without parsing the human sentence.
    ///
    /// ´claim:report:findings-serialise-with-a-stable-code-and-severity´
    /// ´test:unit:serializes-findings-with-their-code´
    #[test]
    fn serializes_findings_with_their_code() {
        let sources = [Source::new("one.md", "Cites (`sec:demo:missing`).\n")];
        let report = report_for(&sources);
        let json = serde_json::to_value(&report).expect("serializable");

        assert!(
            json.get("schema").is_none(),
            "the object states no version of itself"
        );
        assert_eq!(json["findings"][0]["code"], "unresolved_citation");
        assert_eq!(json["findings"][0]["severity"], "failure");
        assert_eq!(json["findings"][0]["label"], "sec:demo:missing");
    }
}
