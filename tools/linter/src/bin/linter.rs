// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Command-line entrypoint for the label-calculus linter.
//!
//! # Output contract
//!
//! This command follows the global JSON-only output contract of ADR-T-010.
//! Stdout carries exactly one JSON result object followed by one newline, stderr
//! carries JSON control-plane and diagnostic records, and stdout is refused when
//! it is attached to a terminal.
//!
//! # Documented exit-code exception
//!
//! ADR-T-010 leaves the exit-code taxonomy beyond the shared `success`,
//! `failure`, and `usage` classes to the owning command's contract. A check that
//! runs to completion and reports findings has not failed as a command: it has
//! produced its result. Emitting that result on stdout while still signalling
//! the outcome in the exit status needs one command-specific class, so this
//! command adds exit code 3 for "the check completed and found failures". The
//! shared classes keep their meanings, and stdout stays empty for both of them.
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The command completed and the corpus is in good standing. |
//! | 1    | The command itself failed; stdout is empty. |
//! | 2    | Usage failure, including stdout TTY refusal; stdout is empty. |
//! | 3    | The command completed and found failures; stdout carries the report. |
//!
//! The fix command takes the same taxonomy: by default it reports what the
//! sweep would do and writes nothing, while `--write` applies the reported
//! changes. It exits 0 when the sweep handled every label it was asked for, and
//! 3 when it completed but left findings — an asset whose standard place it
//! refused to write, say. Refusing to write against a working tree with changes
//! is a precondition failure of the command itself, so that exits 1 with an
//! empty stdout.
//!
//! The fmt command reaches 3 only in `--check` mode, when it completed and found
//! files whose formatted bytes differ. Its write mode exits 0 after applying all
//! guarded changes. Traversal, serialization, integrity, and write failures are
//! command failures, so they exit 1 with empty stdout.
//!
//! The assemble command takes the same taxonomy as the check, because it is one:
//! its default mode is the exact-byte freshness comparison of ADR-T-012 and
//! nothing else, so a publication that is not what its parts say it is exits 3
//! with the report on stdout. With `--write` the publications are regenerated
//! first, and the staleness it repaired is no longer reported — what remains at
//! exit 3 is what writing could not fix, such as a manifest listing a part that
//! is not there.
//!
//! The project command takes the same taxonomy as the check, because in its
//! default mode it is one: it regenerates both projections of ADR-T-017 and
//! compares them to the committed bytes, writing nothing, so a projection that is
//! not what its labels say it is exits 3 with the report on stdout. With `--write`
//! the projections are rewritten first, and what remains at exit 3 is what writing
//! could not fix.
//!
//! There is no fix mode for the claim profile, and the absence is a consequence of
//! ADR-T-017's own kind assignment rather than an omission. A claim stands on an
//! authorship warrant: the author of a test chooses what the test establishes and
//! what to call that statement, and no rule computes it. A sweep that wrote claims
//! would be inventing the one thing the policy asks a person for. The check runs
//! the claim pass automatically, staged, like every other pass it runs.
//!
//! The report, coverage, and shape commands are informational and therefore never
//! produce code 3. The report describes the reference graph and decides nothing about
//! it: uncited mints are ordinary, and the citations that do fail are the
//! check's verdict, restated here for a reviewer's convenience rather than
//! judged a second time. The shape command measures sizes and decides nothing
//! about them either: a long environment may be exactly right, a short one may
//! be the sharpest paragraph in the corpus, and a gate over size would be wrong
//! about both. The coverage command measures what the claims come to and decides
//! nothing about that: a statement nobody cites is the ordinary case, and an
//! intent nobody has kept is a promise written down rather than a rule broken —
//! which is the whole reason ADR-T-017 makes uncoveredness a report line instead
//! of a tracker column. Either command exits 0 however long its listings are; 1 and 2
//! keep their shared meanings, so a caller can still tell "the report is in your
//! hands" from "the command did not run".

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cli_common as cli;
use linter::{
    AssemblyWritePlan, BurnWritePlan, Configuration, Control, ControlWritePlan, DEFAULT_HUBS,
    DEFAULT_UNCITED, ExecutionPlan, FixWritePlan, Label, Operation, PlanError, ProjectionWritePlan,
    WriteGuardPlan, assemble_with_write_plan, burn_with_write_plan, check_with_plan, configuration,
    coverage_with_plan, fix_todo_with_write_plan, fix_with_write_plan, format_paths, is_tree_dirty,
    lower_lists_with_plan, maintain_with_plan, project_with_write_plan, report_with_plan,
    shape_with_plan, verify_write_guard,
};

/// The command name used in control-plane records.
///
/// The name is the crate's, and the crate is registered under it by the owner
/// convention, which tables this workspace's crates against the prefixes derived
/// from their names (´[ORCHESTRATION-conv:profiles:owner-prefixes]´). That is what keeps
/// the string from being a second name for one program: a control-plane record
/// naming this command names the package a reader can find, and renaming the
/// crate is what would move it.
///
/// ´const:indexlinter:control-plane-command-name´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:indexlinter:control-plane-command-name-text-x989f87b3´
const COMMAND: &str = "linter";

/// Exit code for a completed check that found failures.
///
/// The output contract reserves nothing at this number: zero, one and two carry
/// their shared meanings, and a command-specific non-usage code is left to be
/// documented by the owning command contract. This command's contract now
/// documents one, and fixes the figure rather than deferring to a document that
/// defers back (´rule:commandcontract:findings-exit´): three is this binary's
/// only command-specific code, and it means the run finished, wrote its report,
/// and that report carries a finding of failing severity.
///
/// The class exists because the shared three cannot say that. One would be a lie
/// about the run and would oblige an empty stdout, throwing away the report the
/// caller asked for; zero would be a lie about the corpus and would make every
/// gate running this command useless. The outcome travels in the status and the
/// report travels on stdout, and this is the number that pairs them.
///
/// The configuration probe's own exit 3 belongs to a different program and is no
/// relation (´rem:commandcontract:not-the-config-probe´).
///
/// ´const:indexlinter:unclean-check-exit´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:indexlinter:unclean-check-exit-count-3´
const FINDINGS: u8 = 3;

#[derive(Parser)]
#[command(name = COMMAND, version, about = "Check the corpus against the label calculus of ADR-T-014")]
struct Args {
    #[command(subcommand)]
    command: Command,

    #[command(flatten)]
    base: cli::BaseArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Check every carrier source and report every finding.
    Check {
        /// Repository root whose carrier is checked.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Report the reference graph: orphans, hubs, coverage, and reverse lookups.
    Report {
        /// Repository root whose carrier is read.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Report every citation reaching this label.
        #[arg(long, value_name = "LABEL")]
        cites: Option<String>,

        /// How many of the most-cited mints to list.
        #[arg(long, default_value_t = DEFAULT_HUBS)]
        hubs: usize,
    },
    /// Report what the claims come to: statements written, and intents nobody has kept.
    Coverage {
        /// Repository root whose claims are counted.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// How many of the minted statements nothing cites to list.
        #[arg(long, default_value_t = DEFAULT_UNCITED)]
        uncited: usize,
    },
    /// Report the size of every document and environment the corpus carries.
    Shape {
        /// Repository root whose carrier is measured.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Verify, or regenerate, every publication assembled from parts.
    Assemble {
        /// Repository root whose assemblies are verified.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Rewrite every stale publication instead of only reporting it.
        #[arg(long)]
        write: bool,
    },
    /// Census every burn list, verifying or regenerating its register.
    ///
    /// The three maintenance modes are mutually exclusive, and each is a reading
    /// of the one relation between what the tree holds and what the lists
    /// declare. Audit and append read request JSON from standard input. Bare
    /// verification is the fourth reading and the default.
    Burn {
        /// Repository root whose burn lists are censused.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Rewrite every register to what its census says instead of only verifying it.
        #[arg(long, conflicts_with_all = ["audit", "append"])]
        write: bool,

        /// Compare the declared lists against the tree, writing no declaration.
        #[arg(long, requires = "output", conflicts_with = "append")]
        audit: bool,

        /// Record growth the tree already holds, under an authority the request carries.
        #[arg(long, requires = "output")]
        append: bool,

        /// The response file a maintenance mode writes, which is also its stdout object.
        #[arg(long, value_name = "RESPONSE")]
        output: Option<PathBuf>,
    },
    /// Verify, or regenerate, three generated surfaces: both projections of ADR-T-017 and
    /// the constant pins of ADR-T-018.
    Project {
        /// Repository root whose projections are verified.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Rewrite every projection to what the labels say instead of only reporting it.
        #[arg(long)]
        write: bool,
    },
    /// Sweep an inventory profile's labels at every standard place.
    Fix {
        /// Repository root whose assets are swept.
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// The profile whose labels are swept.
        #[arg(long, value_enum)]
        profile: Profile,

        /// Apply the sweep's changes instead of only reporting them.
        #[arg(long)]
        write: bool,

        /// Sweep even though the working tree has uncommitted changes.
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Format Markdown files into one running line per paragraph and list item.
    Fmt {
        /// Report files that would change without writing them.
        #[arg(long)]
        check: bool,

        /// Markdown files or directories to format recursively.
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<PathBuf>,
    },
}

impl Command {
    /// The repository root this invocation names.
    ///
    /// Every corpus subcommand takes one, and the configuration is read from it
    /// before that subcommand runs. The path-oriented formatter has no root and
    /// is dispatched before this accessor is used.
    const fn root(&self) -> Option<&PathBuf> {
        match self {
            Self::Check { root }
            | Self::Report { root, .. }
            | Self::Coverage { root, .. }
            | Self::Shape { root, .. }
            | Self::Assemble { root, .. }
            | Self::Burn { root, .. }
            | Self::Project { root, .. }
            | Self::Fix { root, .. } => Some(root),
            Self::Fmt { .. } => None,
        }
    }
}

/// The inventory profiles the fix mode can sweep.
#[derive(Clone, Copy, clap::ValueEnum)]
enum Profile {
    /// The test profile of ADR-T-015.
    Test,
    /// The to-do profile of ADR-T-016.
    Todo,
}

fn main() -> ExitCode {
    let args = cli::parse_args_or_exit::<Args>();

    cli::set_panic_payload_reporting_enabled(args.base.debug);
    cli::install_json_panic_hook(COMMAND);
    cli::init_json_tracing_with_debug(args.base.debug, tracing::Level::INFO);

    if let Some(record) = cli::stdout_tty_refusal_record(COMMAND) {
        let _ignored = cli::emit_control_plane_record(&record);
        return cli::CommandExit::Usage.exit_code();
    }

    if let Command::Fmt { check, paths } = &args.command {
        return run_fmt(paths, *check);
    }

    let Some(root) = args.command.root().cloned() else {
        return cli::CommandExit::Failure.exit_code();
    };

    // Every corpus subcommand reads one declared configuration snapshot before
    // it does anything else, and a snapshot that is not one refuses the command
    // entire (´spec:commandcontract:configuration´).
    let declared = match read_configuration(&root) {
        Ok(declared) => declared,
        Err(code) => return code,
    };
    let plan = match read_execution_plan(&root, declared) {
        Ok(plan) => plan,
        Err(code) => return code,
    };

    match args.command {
        Command::Check { .. } => run_check(&plan),
        Command::Report { cites, hubs, .. } => run_report(&plan, cites.as_deref(), hubs),
        Command::Coverage { uncited, .. } => run_coverage(&plan, uncited),
        Command::Shape { .. } => run_shape(&plan),
        Command::Assemble { write, .. } => {
            let writer = plan.assembly_write();

            match refuse_incoherent_write(writer.guard(), write) {
                Err(code) => code,
                Ok(()) => run_assemble(writer, write),
            }
        }
        Command::Burn {
            write,
            audit,
            append,
            output,
            ..
        } => {
            let maintenance = if audit {
                Some((Operation::Audit, output))
            } else if append {
                Some((Operation::Append, output))
            } else {
                None
            };

            run_burn_command(&plan, write, maintenance)
        }
        Command::Project { write, .. } => {
            let writer = plan.projection_write();

            match refuse_incoherent_write(writer.guard(), write) {
                Err(code) => code,
                Ok(()) => run_project(&writer, write),
            }
        }
        Command::Fix {
            profile,
            write,
            allow_dirty,
            ..
        } => {
            let writer = plan.fix_write();

            match refuse_incoherent_write(writer.guard(), write) {
                Err(code) => code,
                Ok(()) => run_fix(writer, profile, write, allow_dirty),
            }
        }
        Command::Fmt { check, paths } => run_fmt(&paths, check),
    }
}

/// Compile the single execution plan every subcommand receives.
fn read_execution_plan(
    root: &std::path::Path,
    declared: Configuration,
) -> Result<ExecutionPlan, ExitCode> {
    match ExecutionPlan::compile(root, declared) {
        Ok(plan) => Ok(plan),
        Err(PlanError::Configuration(refusals)) => {
            for refusal in refusals {
                tracing::error!(refusal = %refusal, "the declared configuration is not a snapshot");
            }

            Err(cli::CommandExit::Failure.exit_code())
        }
        Err(PlanError::Topology(defects)) => {
            for defect in defects {
                tracing::error!(defect = %defect, "the declared topology is not runnable");
            }

            Err(cli::CommandExit::Failure.exit_code())
        }
    }
}

/// Read the declared configuration, refusing the command when it is not a snapshot.
///
/// A snapshot that does not parse, names something this binary does not know,
/// duplicates a row, or fails to pair an activated policy with exactly one list
/// in its policy's codec is a refused precondition: the command exits with the
/// shared failure class and an empty stdout, because a command that cannot read
/// its own configuration has no standing to say anything about the corpus
/// (´rule:commandcontract:configuration-verdicts´).
fn read_configuration(root: &std::path::Path) -> Result<Configuration, ExitCode> {
    let declared = configuration(root);
    let refusals = declared.refusals();

    if refusals.is_empty() {
        return Ok(declared);
    }

    for refusal in refusals {
        tracing::error!(refusal = %refusal, "the declared configuration is not a snapshot");
    }

    Err(cli::CommandExit::Failure.exit_code())
}

/// Refuse a writing mode whose declared configuration disagrees with the tree.
///
/// This is stricter than the exit codes alone require, and deliberately so: a
/// partition with an unaccounted file, or a pair whose prerequisite is missing,
/// means the command does not know whose the file is or what the verdict rests
/// on, and a writer that proceeded would record a conclusion drawn from a
/// question the corpus had not answered.
///
fn refuse_incoherent_write(plan: WriteGuardPlan<'_>, write: bool) -> Result<(), ExitCode> {
    if !write {
        return Ok(());
    }

    let (_counts, findings) = verify_write_guard(plan);

    if findings.is_empty() {
        return Ok(());
    }

    for finding in &findings {
        tracing::error!(finding = %finding, "the declared configuration disagrees with the tree");
    }

    Err(cli::CommandExit::Failure.exit_code())
}

/// Check every carrier source and emit the report.
fn run_check(plan: &ExecutionPlan) -> ExitCode {
    let report = check_with_plan(plan);

    tracing::info!(
        sources_scanned = report.sources_scanned,
        mints = report.mints,
        citations_resolved = report.citations_resolved,
        covered = report.profile.covered,
        missing = report.profile.missing,
        collision_groups = report.profile.collision_groups,
        failures = report.failures,
        warnings = report.warnings,
        "label check complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if report.clean {
        cli::CommandExit::Success.exit_code()
    } else {
        ExitCode::from(FINDINGS)
    }
}

/// Report the reference graph and emit the result.
///
/// A label that is not well-formed is a usage failure rather than an empty
/// lookup: asking for the citations of a token that no occurrence could ever
/// carry is a mistake in the question, and answering "none" would hide it.
fn run_report(plan: &ExecutionPlan, cites: Option<&str>, hubs: usize) -> ExitCode {
    let label = match cites.map(Label::parse) {
        None => None,
        Some(Some(label)) => Some(label),
        Some(None) => {
            tracing::error!(
                cites = cites.unwrap_or_default(),
                "the label to look up is not well-formed"
            );
            return cli::CommandExit::Usage.exit_code();
        }
    };

    let report = report_with_plan(plan, label.as_ref(), hubs);

    tracing::info!(
        sources_scanned = report.sources_scanned,
        mints = report.summary.mints,
        citations = report.summary.citations,
        orphans = report.summary.orphans.len(),
        dangling = report.dangling.len(),
        "graph report complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    cli::CommandExit::Success.exit_code()
}

/// Count the corpus's claims and emit the coverage report.
fn run_coverage(plan: &ExecutionPlan, uncited: usize) -> ExitCode {
    let report = coverage_with_plan(plan, uncited);

    tracing::info!(
        sources_scanned = report.sources_scanned,
        covered = report.summary.covered,
        claimed = report.summary.claimed,
        unclaimed = report.summary.unclaimed,
        mints = report.summary.mints,
        citations = report.summary.citations,
        uncited = report.summary.uncited,
        intents = report.summary.intents,
        unwitnessed = report.summary.unwitnessed,
        "coverage report complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    cli::CommandExit::Success.exit_code()
}

/// Measure the corpus's shape and emit the report.
fn run_shape(plan: &ExecutionPlan) -> ExitCode {
    let report = shape_with_plan(plan);

    tracing::info!(
        sources_scanned = report.sources_scanned,
        documents_measured = report.summary.documents_measured,
        environments = report.summary.environments,
        named = report.summary.named,
        words_p50 = report.summary.words.p50,
        words_p90 = report.summary.words.p90,
        division_words_p50 = report.summary.division_words.p50,
        "shape report complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    cli::CommandExit::Success.exit_code()
}

/// Verify or regenerate every assembled publication and emit the report.
fn run_assemble(plan: AssemblyWritePlan<'_>, write: bool) -> ExitCode {
    let report = assemble_with_write_plan(plan, write);

    tracing::info!(
        declared = report.assemblies.len(),
        dormant = report
            .assemblies
            .iter()
            .filter(|assembly| assembly.dormant)
            .count(),
        written = report
            .assemblies
            .iter()
            .filter(|assembly| assembly.written)
            .count(),
        failures = report.failures,
        write,
        "assembly complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if report.failures == 0 {
        cli::CommandExit::Success.exit_code()
    } else {
        ExitCode::from(FINDINGS)
    }
}

/// Format Markdown paths and emit the result.
fn run_fmt(paths: &[PathBuf], check: bool) -> ExitCode {
    let report = match format_paths(paths, check) {
        Ok(report) => report,
        Err(error) => {
            tracing::error!(error = %error, check, "markdown formatting failed");
            return cli::CommandExit::Failure.exit_code();
        }
    };

    for change in &report.changed {
        if check {
            tracing::info!(
                path = change.path,
                elapsed_micros = change.elapsed_micros,
                "markdown would change"
            );
        } else {
            tracing::info!(
                path = change.path,
                elapsed_micros = change.elapsed_micros,
                "markdown formatted"
            );
        }
    }

    tracing::info!(
        files_scanned = report.files_scanned,
        files_changed = report.files_changed,
        elapsed_micros = report.elapsed_micros,
        check,
        "markdown formatting complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if check && report.has_changes() {
        ExitCode::from(FINDINGS)
    } else {
        cli::CommandExit::Success.exit_code()
    }
}

/// Dispatch the burn command to whichever of its four readings was asked for.
///
/// The three maintenance modes are mutually exclusive. Audit and append read one
/// request from standard input and require a response path, so a mode named
/// without its durable receipt is a usage failure rather than a run with a
/// default. Bare verification and the lowering write are the fourth reading, and
/// the write lowers the declared lists beside the Markdown registers it has
/// always rewritten.
fn run_burn_command(
    plan: &ExecutionPlan,
    write: bool,
    maintenance: Option<(Operation, Option<PathBuf>)>,
) -> ExitCode {
    if let Some((operation, output)) = maintenance {
        let Some(output) = output else {
            return cli::CommandExit::Usage.exit_code();
        };

        return run_maintenance(plan.control_write(), operation, &output);
    }

    let writer = plan.burn_write();

    if let Err(code) = refuse_incoherent_write(writer.guard(), write) {
        return code;
    }

    if let Err(error) = lower_declared_lists(writer, write) {
        tracing::error!(error = %error, "could not lower the declared lists");

        return cli::CommandExit::Failure.exit_code();
    }

    run_burn(writer, write)
}

/// Lower the declared lists to the census, where a lowering write asked for it.
///
/// The Markdown registers keep their own writer; what this adds is that a wave
/// which has earned a lower ceiling records it in the declaration as well. A run
/// that is not writing, and a tree with no declaration, both have nothing to
/// lower.
fn lower_declared_lists(plan: BurnWritePlan<'_>, write: bool) -> std::io::Result<bool> {
    if write {
        lower_lists_with_plan(plan.control())
    } else {
        Ok(false)
    }
}

/// Run one controlled maintenance mode and emit its response.
///
/// The response file and the stdout object are the same bytes: the file is made
/// durable first and copied to stdout afterwards, so the receipt travelling with
/// a change and the object a caller reads cannot tell different stories
/// (´req:commandcontract:control-artifacts´). A refused precondition leaves
/// the configuration and the response path unchanged and writes no stdout. The
/// request arrives through standard input and is handed to the control plan as
/// bytes rather than as a path (´dec:controlsurface:request-stream´).
fn run_maintenance(
    plan: ControlWritePlan<'_>,
    operation: Operation,
    output: &std::path::Path,
) -> ExitCode {
    use std::io::Read as _;

    let mut request = Vec::new();
    let read = std::io::stdin().lock().read_to_end(&mut request);

    if let Err(error) = read {
        tracing::error!(error = %error, "failed to read the maintenance request from stdin");

        return cli::CommandExit::Failure.exit_code();
    }

    match maintain_with_plan(plan, operation, &request, output) {
        Control::Refused { message } => {
            tracing::error!(operation = operation.as_str(), message = %message, "the maintenance mode refused to run");

            cli::CommandExit::Failure.exit_code()
        }
        Control::Completed { bytes, failures } => {
            use std::io::Write as _;

            tracing::info!(
                operation = operation.as_str(),
                failures,
                "maintenance complete"
            );

            if let Err(error) = std::io::stdout().write_all(&bytes) {
                tracing::error!(error = %error, "failed to write JSON to stdout");

                return cli::CommandExit::Failure.exit_code();
            }

            if failures == 0 {
                cli::CommandExit::Success.exit_code()
            } else {
                ExitCode::from(FINDINGS)
            }
        }
    }
}

/// Census every burn list and emit the report.
fn run_burn(plan: BurnWritePlan<'_>, write: bool) -> ExitCode {
    let report = burn_with_write_plan(plan, write);

    for family in &report.families {
        tracing::info!(
            family = family.family,
            files_scanned = family.files_scanned,
            occurrences = family.occurrences,
            files_holding = family.files_holding,
            registered = family.registered,
            excluded = family.excluded.join(", "),
            "burn list censused"
        );
    }

    tracing::info!(
        declared = report.families.len(),
        occurrences = report
            .families
            .iter()
            .map(|family| family.occurrences)
            .sum::<usize>(),
        failures = report.failures,
        write,
        "burn census complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if report.failures == 0 {
        cli::CommandExit::Success.exit_code()
    } else {
        ExitCode::from(FINDINGS)
    }
}

/// Verify or regenerate three generated surfaces — both projections of ADR-T-017 and
/// the constant pins of ADR-T-018 — and emit the report.
fn run_project(plan: &ProjectionWritePlan<'_>, write: bool) -> ExitCode {
    let report = project_with_write_plan(plan, write);

    tracing::info!(
        index_considered = report.index.considered,
        index_unchanged = report.index.unchanged,
        index_rewritten = report.index.rewritten,
        index_bootstrapped = report.index.bootstrapped,
        matrix_considered = report.matrix.considered,
        matrix_unchanged = report.matrix.unchanged,
        matrix_rewritten = report.matrix.rewritten,
        matrix_bootstrapped = report.matrix.bootstrapped,
        failures = report.failures,
        write,
        "projection complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if report.failures == 0 {
        cli::CommandExit::Success.exit_code()
    } else {
        ExitCode::from(FINDINGS)
    }
}

/// Sweep an inventory profile's labels and emit the report.
fn run_fix(plan: FixWritePlan<'_>, profile: Profile, write: bool, allow_dirty: bool) -> ExitCode {
    let dry_run = !write;

    if write && !allow_dirty {
        match is_tree_dirty(plan.root()) {
            Ok(true) => {
                tracing::error!(
                    "the working tree has uncommitted changes; pass --allow-dirty to sweep anyway"
                );
                return cli::CommandExit::Failure.exit_code();
            }
            Ok(false) => {}
            Err(error) => {
                tracing::error!(error = %error, "could not establish whether the working tree is clean");
                return cli::CommandExit::Failure.exit_code();
            }
        }
    }

    let report = match profile {
        Profile::Test => fix_with_write_plan(plan, dry_run),
        Profile::Todo => fix_todo_with_write_plan(plan, dry_run),
    };

    tracing::info!(
        profile = report.profile,
        covered = report.outcome.covered,
        inserted = report.outcome.inserted,
        repaired = report.outcome.repaired,
        unchanged = report.outcome.unchanged,
        refused = report.outcome.refused,
        files_changed = report.outcome.files_changed,
        dry_run,
        "label sweep complete"
    );

    if let Err(error) = cli::emit(&report) {
        tracing::error!(error = %error, "failed to write JSON to stdout");
        return cli::CommandExit::Failure.exit_code();
    }

    if report.failures == 0 {
        cli::CommandExit::Success.exit_code()
    } else {
        ExitCode::from(FINDINGS)
    }
}
