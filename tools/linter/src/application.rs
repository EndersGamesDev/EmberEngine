// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Application coordination behind the stable report protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::assembly::{Assembly, part_duplicate_mints, verify_assembly, write_assembly};
use crate::burn::{BurnCensus, BurnList, RegisterRow, census, verify};
use crate::census::take_planned_census;
use crate::claim::{analyze_claims, closed_waves};
use crate::code::{CodeSurface, take_planned_code_citations};
use crate::constant::{
    CoveredConstant, analyze_constants, take_planned_constant_census, write_pins,
};
use crate::coverage::{DEFAULT_UNCITED, summarise_coverage};
use crate::depend::{CitedEdge, cited_edges};
use crate::engine::analyze;
use crate::finding::{Finding, Severity};
use crate::fix::{fix_profile, fix_todos};
use crate::graph::{dangling, summarise};
use crate::index::{by_source, committed_index, verify_indexes, write_index};
use crate::label::Label;
use crate::layers::verify_layers;
use crate::legacy_profile::{analyze_legacy, cover_legacy, take_planned_legacy_census};
use crate::matrix::{
    MatrixFolder, assets_by_folder, planned_folders, verify_planned_matrices, write_matrix,
};
use crate::plan::{
    AssemblyWritePlan, BurnWritePlan, ExecutionPlan, FixWritePlan, ProjectionWritePlan,
    WriteGuardPlan,
};
use crate::profile::{CoveredAsset, TEST_KIND, analyze_profile, cover};
use crate::report::{
    AssembleReport, AssembledDocument, AssemblyCounts, BurnCounts, BurnReport, BurnedFamily,
    CheckReport, ConfigurationCounts, CoverageReport, FixReport, GraphReport, Passes,
    ProjectReport, ProjectionOutcome, ReportedFinding, ShapeReport, Verdicts,
};
use crate::roster::OwnerNames;
use crate::shape::summarise_shape;
use crate::snapshot::{Configuration, Rows, configuration};
use crate::spdx::{carriers, conform};
use crate::subscribe::Subscription;
use crate::todo::{TODO_KIND, analyze_todos, cover_todos, take_planned_todo_census};
#[cfg(test)]
use crate::workspace::Package;

pub fn check(root: &Path) -> CheckReport {
    let plan = execution_plan(root, configuration(root));
    check_with_plan(&plan)
}

pub fn check_with(root: &Path, declared: &Configuration) -> CheckReport {
    let plan = execution_plan(root, declared.clone());
    check_with_plan(&plan)
}

pub fn check_with_plan(plan: &ExecutionPlan) -> CheckReport {
    let root = plan.root();
    let declared = plan.snapshot();
    let packages = plan.workspace().packages().to_vec();
    let workspace_findings = plan.workspace().findings().to_vec();
    let names = declared.declared_owner_names();
    let adoption = plan.labels().adoption();
    let (test_census, census_findings) = take_planned_census(root, plan.profiles().sources());
    let (profile, assets, profile_findings) = analyze_profile(&packages, &test_census);

    let (todo_census, todo_census_findings) =
        take_planned_todo_census(root, plan.profiles().sources());
    let (todo, todo_findings) = analyze_todos(&packages, &todo_census);
    // The covering is taken for the labels it derives and not for its findings:
    // an underivable summary is already reported by the pass above, and
    // reporting it here as well would give one defect two entries.
    let (notices, _notice_findings) = cover_todos(&packages, &todo_census);
    let (legacy_census, legacy_census_findings) =
        take_planned_legacy_census(root, plan.profiles().sources());
    let (legacy, legacy_findings) = analyze_legacy(&legacy_census);
    let (legacy_sites, _legacy_cover_findings) = cover_legacy(&legacy_census);
    let closed = closed_waves(&packages, names.as_ref(), declared.policies());
    let (claim, claims, claim_findings) = analyze_claims(&assets, &closed);
    let pins = plan
        .activations()
        .subscription(crate::subscribe::CONSTANT_PINS_POLICY);
    let (constant_census, constant_census_findings) =
        take_planned_constant_census(root, plan.profiles().constant_sources());
    let (constant, constants, constant_findings) = analyze_constants(&constant_census, &pins);
    let (code_citations, code_findings) =
        take_planned_code_citations(root, plan.profiles().sources(), &assets);

    let code = CodeSurface::default()
        .with_tests(&assets)
        .with_notices(&notices)
        .with_legacy(&legacy_sites)
        .with_claims(&claims)
        .with_constants(&constants)
        .with_citations(code_citations);

    let (sources, carrier_findings) = plan.labels().read(root);
    let analysis = analyze(adoption, &sources, &code);

    let coverage =
        summarise_coverage(&analysis, adoption, &claims, claim.covered, DEFAULT_UNCITED).counts();
    let indexes = plan
        .activations()
        .subscription(crate::subscribe::TEST_INDEXES_POLICY);
    let matrices = plan
        .activations()
        .subscription(crate::subscribe::TEST_MATRICES_POLICY);
    let (index, index_findings) = verify_indexes(root, &assets, &indexes);
    let (matrix_folders, matrix_walk_findings) =
        planned_folders(plan.profiles().sources(), names.as_ref(), &matrices);
    let (matrix, matrix_findings) = verify_planned_matrices(root, &matrix_folders, &assets);
    let (assembly, assembly_findings) = verify_assemblies(root, plan.publications());
    let (censused, burn_findings) = verify_migrations(root, plan);
    let (layers, layer_findings) =
        verify_layers(root, &packages, adoption, &analysis, declared.may_cite());
    let (configuration, configuration_findings) =
        verify_configuration_with_plan(plan, &cited_edges(adoption, &analysis));

    let before = workspace_findings
        .into_iter()
        .chain(carrier_findings)
        .chain(census_findings)
        .chain(profile_findings)
        .chain(todo_census_findings)
        .chain(todo_findings)
        .chain(legacy_census_findings)
        .chain(legacy_findings)
        .chain(code_findings)
        .chain(claim_findings)
        .chain(constant_census_findings)
        .chain(constant_findings)
        .chain(index_findings)
        .chain(matrix_walk_findings)
        .chain(matrix_findings)
        .chain(assembly_findings)
        .chain(burn_findings)
        .chain(layer_findings)
        .chain(configuration_findings)
        .collect();

    let passes = Passes {
        profile,
        todo,
        legacy,
        claim,
        constant,
        coverage,
        index,
        matrix,
    };

    let verdicts = Verdicts {
        assembly,
        burn: burn_counts(&censused),
        layers,
        configuration,
    };

    CheckReport::new(root, &analysis, passes, verdicts, before)
}

pub fn harvest_citations(root: &Path) -> Vec<CitedEdge> {
    let Ok(plan) = ExecutionPlan::compile(root, configuration(root)) else {
        return Vec::new();
    };
    harvest_citations_with_plan(&plan)
}

pub fn harvest_citations_with_plan(plan: &ExecutionPlan) -> Vec<CitedEdge> {
    let root = plan.root();
    let adoption = plan.labels().adoption();
    let (sources, _carrier_findings) = plan.labels().read(root);
    let analysis = analyze(adoption, &sources, &CodeSurface::default());

    cited_edges(adoption, &analysis)
}

pub fn verify_configuration(
    root: &Path,
    declared: &Configuration,
    citations: &[CitedEdge],
) -> (Option<ConfigurationCounts>, Vec<crate::finding::Finding>) {
    if declared.snapshot().is_none() {
        return (None, Vec::new());
    }

    let plan = execution_plan(root, declared.clone());
    verify_configuration_with_plan(&plan, citations)
}

pub fn verify_configuration_with_plan(
    plan: &ExecutionPlan,
    citations: &[CitedEdge],
) -> (Option<ConfigurationCounts>, Vec<crate::finding::Finding>) {
    verify_configuration_with_guard(plan.write_guard(), citations)
}

pub fn verify_configuration_with_guard(
    plan: WriteGuardPlan<'_>,
    citations: &[CitedEdge],
) -> (Option<ConfigurationCounts>, Vec<crate::finding::Finding>) {
    let root = plan.root();
    let snapshot = plan.snapshot();

    let corpus = plan.topology().corpus();
    let universe = corpus.base();
    let mut findings = corpus.findings().to_vec();
    let partition_plan = plan.topology().partition();
    let partition = partition_plan.counts().clone();
    let partition_findings = partition_plan.findings();
    let dependency_findings = plan.dependencies().verify(citations);

    let counts = ConfigurationCounts {
        owners: snapshot.owners().len(),
        inclusions: snapshot.partitions().len(),
        excluded: snapshot.shape().ignore().len(),
        environments: snapshot.environments().len(),
        pairs: snapshot.policies().len(),
        rows: snapshot.lists().values().map(Rows::len).sum(),
        partition,
        missing_dependencies: dependency_findings.len(),
    };

    let spdx_selection = plan.content().spdx();
    let governed = spdx_selection.governed();
    let section_findings = spdx_selection.findings();
    let carrier_findings = carriers(root, governed);

    let tolerated = |policy: &str| -> BTreeSet<&crate::pattern::BytePath> {
        snapshot
            .lists()
            .iter()
            .filter(|(pair, _)| pair.policy == policy)
            .filter_map(|(_, rows)| match rows {
                Rows::Paths(paths) => Some(paths.iter()),
                Rows::Allowances(_) | Rows::PathCounts(_) => None,
            })
            .flatten()
            .collect()
    };

    let header_findings = conform(root, snapshot.spdx(), governed, &tolerated(SPDX_POLICY));

    let interchange_selection = plan.content().interchange();
    let enveloped = interchange_selection.governed();
    let envelope_section_findings = interchange_selection.findings();
    let envelope_carrier_findings = crate::interchange::carriers(root, enveloped);
    let envelope_findings =
        crate::interchange::conform(root, enveloped, &tolerated(INTERCHANGE_POLICY));

    let ceilings: BTreeMap<&crate::pattern::BytePath, u64> = snapshot
        .lists()
        .iter()
        .filter(|(pair, _)| pair.policy == REFERENCES_POLICY)
        .filter_map(|(_, rows)| match rows {
            Rows::PathCounts(rows) => Some(rows.iter()),
            Rows::Allowances(_) | Rows::Paths(_) => None,
        })
        .flatten()
        .map(|row| (&row.path, row.maximum))
        .collect();

    let reference_selection = plan.content().references();
    let governed = reference_selection.governed();
    let reference_gloss_findings = reference_selection.findings();
    let lexicon = crate::reference::Lexicon::from_tracked(universe);
    let censused = crate::reference::census(root, &lexicon, governed);
    let carrier_kind_findings = crate::reference::carriers(&censused);
    let citation_findings = crate::reference::conform(&censused, &ceilings);

    findings.extend_from_slice(partition_findings);
    findings.extend(dependency_findings);
    findings.extend_from_slice(section_findings);
    findings.extend(carrier_findings);
    findings.extend(header_findings);
    findings.extend_from_slice(envelope_section_findings);
    findings.extend(envelope_carrier_findings);
    findings.extend(envelope_findings);
    findings.extend_from_slice(reference_gloss_findings);
    findings.extend(carrier_kind_findings);
    findings.extend(citation_findings);

    (Some(counts), findings)
}

pub fn verify_write_guard(
    plan: WriteGuardPlan<'_>,
) -> (Option<ConfigurationCounts>, Vec<crate::finding::Finding>) {
    let adoption = plan.labels().adoption();
    let (sources, _carrier_findings) = plan.labels().read(plan.root());
    let analysis = analyze(adoption, &sources, &CodeSurface::default());

    verify_configuration_with_guard(plan, &cited_edges(adoption, &analysis))
}

fn verify_assemblies(
    root: &Path,
    publications: &crate::plan::PublicationPlan,
) -> (AssemblyCounts, Vec<Finding>) {
    let mut counts = AssemblyCounts::default();
    let mut findings = Vec::new();

    for run in publications.runs() {
        let assembly = run.assembly();
        counts.declared += 1;

        let (assembled, assembly_findings) = verify_assembly(root, assembly);

        if assembled.dormant {
            counts.dormant += 1;
        } else {
            counts.verified += 1;
            counts.parts += assembled.parts;
        }

        findings.extend(assembly_findings);
    }

    (counts, findings)
}

fn verify_migrations(
    root: &Path,
    plan: &ExecutionPlan,
) -> (Vec<(BurnList, BurnCensus)>, Vec<Finding>) {
    let mut taken = Vec::new();
    let mut findings = plan.migrations().findings().to_vec();

    for run in plan.migrations().runs() {
        let (census, census_findings) = census(root, run.list(), plan.topology().corpus());
        findings.extend(census_findings);

        findings.extend(verify(run.list(), &census, run.declared()));

        taken.push((run.list().clone(), census));
    }

    (taken, findings)
}

#[cfg(test)]
pub fn adopted(packages: &[Package], plan: &ExecutionPlan) -> crate::adoption::Adoption {
    let declared = plan.snapshot();
    let names = declared.declared_owner_names();
    crate::adoption::index_adoption(
        packages,
        names.as_ref(),
        plan.publications().assemblies(),
        declared.kind_registry(),
    )
    .with_declared_owners(declared.owners(), declared.root_owner())
}

const SPDX_POLICY: &str = "spdx.headers-conform";
const INTERCHANGE_POLICY: &str = "interchange.envelope-conform";
const REFERENCES_POLICY: &str = "references.file-paths-absent";

fn burn_counts(censused: &[(BurnList, BurnCensus)]) -> BurnCounts {
    BurnCounts {
        declared: censused.len(),
        files_scanned: censused
            .iter()
            .map(|(_list, census)| census.files_scanned())
            .sum(),
        occurrences: censused.iter().map(|(_list, census)| census.total()).sum(),
    }
}

pub fn burn_with_plan(plan: &ExecutionPlan, write: bool) -> BurnReport {
    burn_with_write_plan(plan.burn_write(), write)
}

pub fn burn_with_write_plan(plan: BurnWritePlan<'_>, write: bool) -> BurnReport {
    let root = plan.root();
    let mut families = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();

    for run in plan.migrations().runs() {
        let list = run.list();
        let (taken, census_findings) = census(root, list, plan.control().topology().corpus());
        findings.extend(census_findings);

        let mut judgment = verify(list, &taken, run.declared());

        if write {
            judgment.retain(|finding| matches!(finding, Finding::BurnListGrowth { .. }));
        }

        findings.append(&mut judgment);

        families.push(BurnedFamily {
            family: list.family().to_owned(),
            prose: displayed(list.prose()),
            code: displayed(list.code()),
            excluded: displayed(list.excluded()),
            files_scanned: taken.files_scanned(),
            occurrences: taken.total(),
            files_holding: taken.rows().len(),
            registered: run.declared().iter().map(RegisterRow::count).sum(),
            rows: taken.rows().to_vec(),
        });
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let reported: Vec<ReportedFinding> = findings.into_iter().map(ReportedFinding::from).collect();
    let failures = reported
        .iter()
        .filter(|finding| finding.severity == Severity::Failure)
        .count();

    BurnReport {
        root: root.to_string_lossy().into_owned(),
        write,
        families,
        failures,
        findings: reported,
    }
}

fn displayed(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

pub fn code_surface(plan: &ExecutionPlan, assets: &[CoveredAsset]) -> CodeSurface {
    let root = plan.root();
    let packages = plan.workspace().packages();
    let (todo_census, _census_findings) = take_planned_todo_census(root, plan.profiles().sources());
    let (notices, _cover_findings) = cover_todos(packages, &todo_census);
    // The wave closure is the check's verdict and no part of the surface: what is
    // taken here is the claims themselves, which stand where they were written
    // whether or not their package is held to writing them.
    let (_claim, claims, _claim_findings) = analyze_claims(assets, &[]);
    let (constant_census, _constant_findings) =
        take_planned_constant_census(root, plan.profiles().constant_sources());
    let pins = plan
        .activations()
        .subscription(crate::subscribe::CONSTANT_PINS_POLICY);
    let (_constant, constants, _constant_defects) = analyze_constants(&constant_census, &pins);
    let (citations, _code_findings) =
        take_planned_code_citations(root, plan.profiles().sources(), assets);

    CodeSurface::default()
        .with_tests(assets)
        .with_notices(&notices)
        .with_claims(&claims)
        .with_constants(&constants)
        .with_citations(citations)
}

pub fn report(root: &Path, label: Option<&Label>, hubs: usize) -> GraphReport {
    let plan = execution_plan(root, configuration(root));
    report_with_plan(&plan, label, hubs)
}

pub fn report_with_plan(plan: &ExecutionPlan, label: Option<&Label>, hubs: usize) -> GraphReport {
    let root = plan.root();
    let packages = plan.workspace().packages().to_vec();
    let adoption = plan.labels().adoption();
    let (test_census, _census_findings) = take_planned_census(root, plan.profiles().sources());
    let (assets, _cover_findings) = cover(&packages, &test_census);

    let (sources, _carrier_findings) = plan.labels().read(root);
    let analysis = analyze(adoption, &sources, &code_surface(plan, &assets));

    GraphReport {
        root: root.to_string_lossy().into_owned(),
        sources_scanned: analysis.sources_scanned(),
        summary: summarise(&analysis, label, hubs),
        dangling: dangling(&analysis)
            .into_iter()
            .map(ReportedFinding::from)
            .collect(),
    }
}

pub fn shape(root: &Path) -> ShapeReport {
    let plan = execution_plan(root, configuration(root));
    shape_with_plan(&plan)
}

pub fn shape_with_plan(plan: &ExecutionPlan) -> ShapeReport {
    let root = plan.root();
    let adoption = plan.labels().adoption();
    let (sources, _carrier_findings) = plan.labels().read(root);

    ShapeReport {
        root: root.to_string_lossy().into_owned(),
        sources_scanned: sources.len(),
        summary: summarise_shape(adoption, &sources),
    }
}

pub fn coverage(root: &Path, uncited: usize) -> CoverageReport {
    let plan = execution_plan(root, configuration(root));
    coverage_with_plan(&plan, uncited)
}

pub fn coverage_with_plan(plan: &ExecutionPlan, uncited: usize) -> CoverageReport {
    let root = plan.root();
    let packages = plan.workspace().packages().to_vec();
    let adoption = plan.labels().adoption();
    let (test_census, _census_findings) = take_planned_census(root, plan.profiles().sources());
    let (assets, _cover_findings) = cover(&packages, &test_census);

    let (sources, _carrier_findings) = plan.labels().read(root);
    let analysis = analyze(adoption, &sources, &code_surface(plan, &assets));

    // The report counts what is written and holds nobody to writing it, so the
    // closed set is the one input this command has no use for.
    let (claim, claims, _claim_findings) = analyze_claims(&assets, &[]);

    CoverageReport {
        root: root.to_string_lossy().into_owned(),
        sources_scanned: analysis.sources_scanned(),
        summary: summarise_coverage(&analysis, adoption, &claims, claim.covered, uncited),
    }
}

pub fn assemble_with_plan(plan: &ExecutionPlan, write: bool) -> AssembleReport {
    assemble_with_write_plan(plan.assembly_write(), write)
}

pub fn assemble_with_write_plan(plan: AssemblyWritePlan<'_>, write: bool) -> AssembleReport {
    let root = plan.root();

    let mut assemblies = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();

    for run in plan.publications().runs() {
        let assembly = run.assembly();
        let (assembled, mut assembly_findings) = verify_assembly(root, assembly);

        if !assembled.dormant {
            assembly_findings.extend(part_duplicate_mints(root, assembly, run.owner()));
        }

        let fresh =
            !assembled.draft && assembled.text.is_some() && !assembly_findings.iter().any(is_stale);

        if write && assembled.draft && assembled.text.is_some() {
            assembly_findings.push(Finding::DraftAssemblyWrite {
                target: assembly.target().to_string_lossy().into_owned(),
                parts: assembly.parts().to_string_lossy().into_owned(),
                manifest: assembly.manifest().to_string_lossy().into_owned(),
            });
        }

        let written = write
            && !assembled.draft
            && !fresh
            && write_one(
                root,
                assembly,
                assembled.text.as_deref(),
                &mut assembly_findings,
            );

        assemblies.push(AssembledDocument {
            parts: assembly.parts().to_string_lossy().into_owned(),
            manifest: assembly.manifest().to_string_lossy().into_owned(),
            target: assembly.target().to_string_lossy().into_owned(),
            dormant: assembled.dormant,
            draft: assembled.draft,
            assembled: assembled.parts,
            bytes: assembled.text.as_ref().map(String::len),
            fresh,
            written,
        });

        findings.extend(assembly_findings);
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let reported: Vec<ReportedFinding> = findings.into_iter().map(ReportedFinding::from).collect();
    let failures = reported
        .iter()
        .filter(|finding| finding.severity == Severity::Failure)
        .count();

    AssembleReport {
        root: root.to_string_lossy().into_owned(),
        write,
        assemblies,
        failures,
        findings: reported,
    }
}

const fn is_stale(finding: &Finding) -> bool {
    matches!(finding, Finding::StaleAssembly { .. })
}

fn write_one(
    root: &Path,
    assembly: &Assembly,
    text: Option<&str>,
    findings: &mut Vec<Finding>,
) -> bool {
    let Some(text) = text else {
        return false;
    };

    match write_assembly(root, assembly, text) {
        Ok(()) => {
            findings.retain(|finding| !is_stale(finding));

            true
        }
        Err(message) => {
            findings.push(Finding::TraversalFailure {
                path: assembly.target().to_string_lossy().into_owned(),
                message,
            });

            false
        }
    }
}

pub fn project_with_plan(plan: &ExecutionPlan, write: bool) -> ProjectReport {
    let writer = plan.projection_write();

    project_with_write_plan(&writer, write)
}

pub fn project_with_write_plan(plan: &ProjectionWritePlan<'_>, write: bool) -> ProjectReport {
    project_from_plan(plan, plan.names(), write)
}

#[cfg(test)]
pub fn project_with(
    root: &Path,
    names: Option<&crate::roster::OwnerNames>,
    _ignore: &[crate::universe::IgnoreRow],
    declared: Option<&crate::snapshot::Snapshot>,
    write: bool,
) -> ProjectReport {
    let snapshot = declared
        .cloned()
        .unwrap_or_else(crate::test_support::projection_snapshot);
    let configured = Configuration::Present(Box::new(snapshot));
    let plan = execution_plan(root, configured);
    let writer = plan.projection_write();

    project_from_plan(&writer, names, write)
}

fn project_from_plan(
    plan: &ProjectionWritePlan<'_>,
    names: Option<&crate::roster::OwnerNames>,
    write: bool,
) -> ProjectReport {
    let root = plan.root();
    let packages = plan.workspace().packages().to_vec();
    let workspace_findings = plan.workspace().findings().to_vec();
    let (census, census_findings) = take_planned_census(root, plan.profiles().sources());
    let (assets, cover_findings) = cover(&packages, &census);

    let mut findings: Vec<Finding> = workspace_findings
        .into_iter()
        .chain(census_findings)
        .chain(cover_findings)
        .collect();

    let indexes = plan
        .activations()
        .subscription(crate::subscribe::TEST_INDEXES_POLICY);
    let matrices = plan
        .activations()
        .subscription(crate::subscribe::TEST_MATRICES_POLICY);
    let pins = plan
        .activations()
        .subscription(crate::subscribe::CONSTANT_PINS_POLICY);

    let index = sweep_indexes(root, &assets, &indexes, write, &mut findings);
    let matrix = sweep_matrices(plan, &assets, names, &matrices, write, &mut findings);
    let constant = sweep_constants(plan, &pins, write, &mut findings);

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let reported: Vec<ReportedFinding> = findings.into_iter().map(ReportedFinding::from).collect();
    let failures = reported
        .iter()
        .filter(|finding| finding.severity == Severity::Failure)
        .count();

    ProjectReport {
        root: root.to_string_lossy().into_owned(),
        write,
        index,
        matrix,
        constant,
        failures,
        findings: reported,
    }
}

fn sweep_constants(
    plan: &ProjectionWritePlan<'_>,
    subscription: &Subscription<'_>,
    write: bool,
    findings: &mut Vec<Finding>,
) -> ProjectionOutcome {
    let root = plan.root();
    let mut outcome = ProjectionOutcome::default();
    let (census, census_findings) =
        take_planned_constant_census(root, plan.profiles().constant_sources());
    let (analysis, covered, constant_findings) = analyze_constants(&census, subscription);

    findings.extend(census_findings);

    let governed: Vec<&CoveredConstant> = covered
        .iter()
        .filter(|constant| subscription.governs(constant.path()))
        .collect();

    outcome.considered = governed.len();
    outcome.unchanged = analysis.pinned;
    outcome.rewritten = analysis.stale_pins;
    outcome.bootstrapped = analysis.missing_pins;

    if !write {
        findings.extend(constant_findings);

        return outcome;
    }

    findings.extend(constant_findings.into_iter().filter(|finding| {
        !matches!(
            finding,
            Finding::MissingConstantPin { .. } | Finding::WrongConstantPin { .. }
        )
    }));

    let mut by_source: BTreeMap<PathBuf, Vec<&CoveredConstant>> = BTreeMap::new();

    for constant in governed {
        by_source
            .entry(constant.path().to_path_buf())
            .or_default()
            .push(constant);
    }

    for (path, constants) in by_source {
        let text = match fs::read_to_string(root.join(&path)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        let Some(rewritten) = write_pins(&text, &constants) else {
            continue;
        };

        if let Err(error) = fs::write(root.join(&path), rewritten) {
            findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
        }
    }

    outcome
}

fn sweep_indexes(
    root: &Path,
    assets: &[CoveredAsset],
    subscription: &Subscription<'_>,
    write: bool,
    findings: &mut Vec<Finding>,
) -> ProjectionOutcome {
    let mut outcome = ProjectionOutcome::default();

    if !write {
        let (analysis, verification) = verify_indexes(root, assets, subscription);

        findings.extend(verification);
        outcome.considered = analysis.sources_covered;
        outcome.unchanged = analysis.indexed - analysis.stale;
        outcome.rewritten = analysis.stale;
        outcome.bootstrapped = analysis.unindexed;

        return outcome;
    }

    for (path, sources) in by_source(assets) {
        if !subscription.governs(&path) {
            continue;
        }

        outcome.considered += 1;

        let text = match fs::read_to_string(root.join(&path)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        let had = committed_index(&text).is_some();

        let Some(rewritten) = write_index(&text, &sources) else {
            outcome.unchanged += 1;
            continue;
        };

        if had {
            outcome.rewritten += 1;
        } else {
            outcome.bootstrapped += 1;
        }

        if let Err(error) = fs::write(root.join(&path), rewritten) {
            findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
        }
    }

    outcome
}

fn sweep_matrices(
    plan: &ProjectionWritePlan<'_>,
    assets: &[CoveredAsset],
    names: Option<&OwnerNames>,
    subscription: &Subscription<'_>,
    write: bool,
    findings: &mut Vec<Finding>,
) -> ProjectionOutcome {
    let root = plan.root();
    let mut outcome = ProjectionOutcome::default();
    let (found, walk_findings) = planned_folders(plan.profiles().sources(), names, subscription);
    findings.extend(walk_findings);

    if !write {
        let (analysis, verification) = verify_planned_matrices(root, &found, assets);

        findings.extend(verification);
        outcome.considered = analysis.folders;
        outcome.unchanged = analysis.with_matrix - analysis.stale;
        outcome.rewritten = analysis.stale;
        outcome.bootstrapped = analysis.without_matrix;

        return outcome;
    }

    let by_folder = assets_by_folder(assets);

    for folder in &found {
        outcome.considered += 1;
        write_one_matrix(root, folder, &by_folder, &mut outcome, findings);
    }

    outcome
}

fn write_one_matrix(
    root: &Path,
    folder: &MatrixFolder,
    by_folder: &BTreeMap<PathBuf, Vec<&CoveredAsset>>,
    outcome: &mut ProjectionOutcome,
    findings: &mut Vec<Finding>,
) {
    let readme = folder.readme();
    let existing = fs::read_to_string(root.join(&readme)).ok();
    let owned: Vec<&CoveredAsset> = by_folder
        .get(folder.directory())
        .map_or_else(Vec::new, Clone::clone);

    let Some(written) = write_matrix(existing.as_deref(), folder, &owned) else {
        outcome.unchanged += 1;
        return;
    };

    if existing.is_some() {
        outcome.rewritten += 1;
    } else {
        outcome.bootstrapped += 1;
    }

    if let Err(error) = fs::write(root.join(&readme), written) {
        findings.push(Finding::TraversalFailure {
            path: readme.to_string_lossy().into_owned(),
            message: error.to_string(),
        });
    }
}

pub fn fix_with_plan(plan: &ExecutionPlan, dry_run: bool) -> FixReport {
    fix_with_write_plan(plan.fix_write(), dry_run)
}

pub fn fix_with_write_plan(plan: FixWritePlan<'_>, dry_run: bool) -> FixReport {
    let root = plan.root();
    let packages = plan.workspace().packages().to_vec();
    let workspace_findings = plan.workspace().findings().to_vec();
    let (census, census_findings) = take_planned_census(root, plan.profiles().sources());
    let (assets, cover_findings) = cover(&packages, &census);
    let (outcome, mut fix_findings) = fix_profile(root, &assets, dry_run);

    let mut findings: Vec<Finding> = workspace_findings
        .into_iter()
        .chain(census_findings)
        .chain(cover_findings)
        .collect();

    findings.append(&mut fix_findings);
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let reported: Vec<ReportedFinding> = findings.into_iter().map(ReportedFinding::from).collect();
    let failures = reported
        .iter()
        .filter(|finding| finding.severity == Severity::Failure)
        .count();

    FixReport {
        root: root.to_string_lossy().into_owned(),
        profile: TEST_KIND,
        dry_run,
        outcome,
        failures,
        findings: reported,
    }
}

pub fn fix_todo_with_plan(plan: &ExecutionPlan, dry_run: bool) -> FixReport {
    fix_todo_with_write_plan(plan.fix_write(), dry_run)
}

pub fn fix_todo_with_write_plan(plan: FixWritePlan<'_>, dry_run: bool) -> FixReport {
    let root = plan.root();
    let packages = plan.workspace().packages().to_vec();
    let workspace_findings = plan.workspace().findings().to_vec();
    let (census, census_findings) = take_planned_todo_census(root, plan.profiles().sources());
    let (notices, cover_findings) = cover_todos(&packages, &census);
    let (outcome, mut fix_findings) = fix_todos(root, &notices, dry_run);

    let mut findings: Vec<Finding> = workspace_findings
        .into_iter()
        .chain(census_findings)
        .chain(cover_findings)
        .collect();

    findings.append(&mut fix_findings);
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let reported: Vec<ReportedFinding> = findings.into_iter().map(ReportedFinding::from).collect();
    let failures = reported
        .iter()
        .filter(|finding| finding.severity == Severity::Failure)
        .count();

    FixReport {
        root: root.to_string_lossy().into_owned(),
        profile: TODO_KIND,
        dry_run,
        outcome,
        failures,
        findings: reported,
    }
}

fn execution_plan(root: &Path, configuration: Configuration) -> ExecutionPlan {
    ExecutionPlan::compile(root, configuration)
        .unwrap_or_else(|error| panic!("the execution plan did not compile: {error:?}"))
}
