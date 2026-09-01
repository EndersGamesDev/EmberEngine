// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Immutable execution topology compiled before any analyzer runs.
//!
//! The declared surface answers what the corpus is and which programmes it
//! activates. This module turns those answers, together with generic workspace
//! and filesystem facts, into one value. Consumers receive a typed projection
//! from that value; they do not enumerate the corpus, remove ignore rows, reopen
//! declarations, or reconstruct attribution for themselves.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`materializes_both_universe_kinds_without_following_links`] | plan | The declared universe kind alone selects tracked or as-written enumeration, and the as-written walk treats a symbolic link as one entry rather than following it. |
//! | [`distinguishes_an_ignored_path_from_one_that_was_absent`] | plan | Ignore is removed once while its membership remains inspectable, so an ignored path participates in no projection and is distinguishable from a path the base never held. |
//! | [`refuses_one_non_text_path_once`] | plan | A path no declared pattern can read refuses topology once in reversible display form. |
//! | [`bounds_selection_by_owner_before_subtracting_exclusions`] | plan | Generic selection begins inside one owner share, admits by the inclusion union, and subtracts the exclusion union. |
//! | [`detects_prefix_overlap_only_over_shared_membership`] | plan | Prefix domains overlap only where both their recognizers and resolved source memberships intersect. |
//! | [`plan_universe_matches_the_current_git_constructor`] | plan | The plan's git-tracked materialization agrees path-for-path and finding-for-finding with the constructor it will replace. |
//! | [`plan_ignore_union_matches_the_current_constructor`] | plan | The plan's ignored and participating sets agree with the resolver it replaces, while the plan additionally retains per-row reach. |
//! | [`plan_partition_matches_the_current_constructor`] | plan | The plan's partition counts, findings, and attribution agree with the constructor it will replace. |
//! | [`plan_workspace_matches_the_current_constructor`] | plan | The plan's generic workspace discovery preserves the package identities, crate-name ordering, and traversal findings of the constructor it replaces. |
//! | [`keeps_literal_workspace_members_unchanged`] | plan | A literal workspace member still reads exactly the directory it names, preserving the behavior from before glob expansion. |
//! | [`expands_workspace_member_globs_in_path_order`] | plan | A member glob expands to manifest-bearing directories in deterministic lexicographic path order. |
//! | [`matches_one_character_in_workspace_member_globs`] | plan | The `?` member glob matches exactly one character within one component, never an empty string or multiple characters. |
//! | [`reports_a_workspace_member_glob_that_matches_nothing`] | plan | A member glob that finds no manifest-bearing directory reports the pattern at the workspace manifest instead of silently shrinking owners. |
//! | [`deduplicates_literal_and_glob_workspace_members`] | plan | A member selected by both a literal and a glob joins the owner partition once, without reading or reporting its manifest twice. |
//! | [`skips_globbed_directories_without_manifests`] | plan | A directory whose name matches a glob but which carries no `Cargo.toml` is skipped while manifest-bearing matches still join the workspace. |
//! | [`keeps_unsupported_member_glob_syntax_literal`] | plan | Bracket and brace forms keep literal treatment even when their entries also contain supported metacharacters. |
//! | [`plan_publications_match_the_current_constructor`] | plan | The plan carries a fictional declaration's publication rows and every derived projection exactly as the constructor it replaces did. |
//! | [`plan_dependencies_match_the_current_constructor`] | plan | Compiled dependency templates preserve the retiring verifier's exact findings, while their stored schedule places declared prerequisites before the pairs that require them. |
//! | [`label_plan_matches_the_current_carrier_and_observations`] | plan | The activated-owner label projection agrees source-for-source and observation-for-observation with the retiring carrier and package-derived ownership. |
//! | [`profile_plan_matches_the_current_census_sources_and_observations`] | plan | The finite test, to-do, constant, commentary, and matrix projections agree row-for-row with the analyzer-owned source walks they replace. |
//! | [`content_plan_matches_the_three_current_policy_selections`] | plan | SPDX constitutive inclusion and the interchange and file-path diagnostic glosses preserve every governed row, selection finding, and named exclusion. |
//! | [`migration_and_publication_runs_match_the_current_programs`] | plan | Burn surfaces, ratchet rows, census observations, publication rows, assembled bytes, and assembly findings agree with the retiring paths. |

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use crate::adoption::{Adoption, index_signature, owner_of_package};
use crate::assembly::{Assembly, PublicationDefect, Publications};
use crate::burn::{BurnList, RegisterRow, declared_rows, index_burn_lists, undeclared_surfaces};
use crate::carrier::Source;
use crate::catalogue::{Observer, Scope, catalogued};
use crate::declaration::AbnfPattern;
use crate::depend::CitedEdge;
use crate::finding::{Finding, Location};
use crate::label::Prefix;
use crate::partition::PartitionCounts;
use crate::pattern::BytePath;
use crate::program::PrefixNumbers;
use crate::snapshot::{Configuration, OwnerRow, Pair, Refusal, Snapshot};
use crate::subscribe::Subscription;
use crate::universe::{IgnoreRow, UniverseKind};
use crate::workspace::Package;

/// A fully compiled, immutable command input.
#[derive(Debug)]
pub struct ExecutionPlan {
    root: PathBuf,
    snapshot: Box<Snapshot>,
    topology: TopologyPlan,
    workspace: WorkspacePlan,
    activations: ActivationPlan,
    publications: PublicationPlan,
    labels: LabelPlan,
    profiles: ProfilePlan,
    content: ContentPlan,
    migrations: MigrationPlan,
    dependencies: DependencyPlan,
}

impl ExecutionPlan {
    /// Compile every topology projection from one already-loaded configuration.
    ///
    /// # Errors
    ///
    /// Returns declaration refusals without reading the corpus, or a topology
    /// refusal when a materialized path cannot enter the declared pattern
    /// language.
    pub fn compile(root: &Path, configuration: Configuration) -> Result<Self, PlanError> {
        let snapshot = match configuration {
            Configuration::Refused(refusals) => return Err(PlanError::Configuration(refusals)),
            Configuration::Present(snapshot) => snapshot,
        };

        let universe = snapshot.shape().universe();
        let ignore = snapshot.shape().ignore();
        let corpus = CorpusPlan::compile(root, universe, ignore).map_err(PlanError::Topology)?;
        let partition = PartitionPlan::compile(snapshot.partitions(), &corpus);
        let topology = TopologyPlan { corpus, partition };
        let workspace = WorkspacePlan::compile(root);
        let activations = ActivationPlan::compile(&snapshot);
        let publications = PublicationPlan::compile(&snapshot);
        let labels = LabelPlan::compile(
            &snapshot,
            &topology,
            &workspace,
            &activations,
            &publications,
        );
        let profiles = ProfilePlan::compile(&topology, &workspace);
        let content = ContentPlan::compile(&snapshot, &topology);
        let migrations = MigrationPlan::compile(&snapshot);
        let dependencies = DependencyPlan::compile(&snapshot);

        Ok(Self {
            root: root.to_path_buf(),
            snapshot,
            topology,
            workspace,
            activations,
            publications,
            labels,
            profiles,
            content,
            migrations,
            dependencies,
        })
    }

    /// The root every path in the plan is relative to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The single declared snapshot the plan was compiled from.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// The resolved corpus and owner partition.
    #[must_use]
    pub const fn topology(&self) -> &TopologyPlan {
        &self.topology
    }

    /// The generic workspace discovery result.
    #[must_use]
    pub const fn workspace(&self) -> &WorkspacePlan {
        &self.workspace
    }

    /// The compiled activation projections.
    #[must_use]
    pub const fn activations(&self) -> &ActivationPlan {
        &self.activations
    }

    /// The compiled publication relation and its derived containment facts.
    #[must_use]
    pub const fn publications(&self) -> &PublicationPlan {
        &self.publications
    }

    /// The compiled label-calculus run.
    #[must_use]
    pub const fn labels(&self) -> &LabelPlan {
        &self.labels
    }

    /// The finite Rust source projections consumed by profiles and projections.
    #[must_use]
    pub const fn profiles(&self) -> &ProfilePlan {
        &self.profiles
    }

    /// The three typed content-policy selections compiled over the partition.
    #[must_use]
    pub const fn content(&self) -> &ContentPlan {
        &self.content
    }

    /// The typed burn-family runs compiled from the declaration.
    #[must_use]
    pub const fn migrations(&self) -> &MigrationPlan {
        &self.migrations
    }

    /// The compiled dependency templates and stable schedule.
    #[must_use]
    pub const fn dependencies(&self) -> &DependencyPlan {
        &self.dependencies
    }

    /// The configuration and topology needed to guard a write.
    #[must_use]
    pub fn write_guard(&self) -> WriteGuardPlan<'_> {
        WriteGuardPlan {
            root: &self.root,
            snapshot: &self.snapshot,
            topology: &self.topology,
            labels: &self.labels,
            content: &self.content,
            dependencies: &self.dependencies,
        }
    }

    /// The declared lists and typed observation routes used by control writes.
    #[must_use]
    pub fn control_write(&self) -> ControlWritePlan<'_> {
        ControlWritePlan {
            root: &self.root,
            snapshot: &self.snapshot,
            topology: &self.topology,
            content: &self.content,
            migrations: &self.migrations,
            guard: self.write_guard(),
        }
    }

    /// The migration schedule and lowering plan used by burn writes.
    #[must_use]
    pub fn burn_write(&self) -> BurnWritePlan<'_> {
        BurnWritePlan {
            root: &self.root,
            migrations: &self.migrations,
            control: self.control_write(),
            guard: self.write_guard(),
        }
    }

    /// The finite source and activation projections used by projection writes.
    #[must_use]
    pub fn projection_write(&self) -> ProjectionWritePlan<'_> {
        ProjectionWritePlan {
            root: &self.root,
            workspace: &self.workspace,
            profiles: &self.profiles,
            activations: &self.activations,
            names: self.snapshot.declared_owner_names(),
            guard: self.write_guard(),
        }
    }

    /// The publication schedule used by assembly writes.
    #[must_use]
    pub fn assembly_write(&self) -> AssemblyWritePlan<'_> {
        AssemblyWritePlan {
            root: &self.root,
            publications: &self.publications,
            guard: self.write_guard(),
        }
    }

    /// The finite source projections used by both fix profiles.
    #[must_use]
    pub fn fix_write(&self) -> FixWritePlan<'_> {
        FixWritePlan {
            root: &self.root,
            workspace: &self.workspace,
            profiles: &self.profiles,
            guard: self.write_guard(),
        }
    }
}

/// The already-compiled facts every writing mode checks before mutation.
#[derive(Debug, Clone, Copy)]
pub struct WriteGuardPlan<'a> {
    root: &'a Path,
    snapshot: &'a Snapshot,
    topology: &'a TopologyPlan,
    labels: &'a LabelPlan,
    content: &'a ContentPlan,
    dependencies: &'a DependencyPlan,
}

impl<'a> WriteGuardPlan<'a> {
    /// The root whose files a writer may mutate.
    #[must_use]
    pub const fn root(self) -> &'a Path {
        self.root
    }

    /// The single declared snapshot.
    #[must_use]
    pub const fn snapshot(self) -> &'a Snapshot {
        self.snapshot
    }

    /// The resolved corpus and owner partition.
    #[must_use]
    pub const fn topology(self) -> &'a TopologyPlan {
        self.topology
    }

    /// The finite label run used to instantiate cited-owner dependencies.
    #[must_use]
    pub const fn labels(self) -> &'a LabelPlan {
        self.labels
    }

    /// The typed content selections used by configuration judgment.
    #[must_use]
    pub const fn content(self) -> &'a ContentPlan {
        self.content
    }

    /// The compiled dependency schedule.
    #[must_use]
    pub const fn dependencies(self) -> &'a DependencyPlan {
        self.dependencies
    }
}

/// One control observation with catalogue dispatch and plan inputs resolved.
#[derive(Debug, Clone)]
pub struct ControlObservationPlan<'a> {
    observer: Option<Observer>,
    owner: String,
    snapshot: &'a Snapshot,
    corpus: &'a CorpusPlan,
    partition: &'a PartitionPlan,
    content: Option<&'a ContentPlan>,
    migrations: Option<&'a MigrationPlan>,
}

impl<'a> ControlObservationPlan<'a> {
    pub(crate) fn compatibility(
        snapshot: &'a Snapshot,
        pair: &Pair,
        corpus: &'a CorpusPlan,
        partition: &'a PartitionPlan,
    ) -> Self {
        Self {
            observer: snapshot
                .program(pair)
                .and_then(catalogued)
                .map(|policy| policy.observer),
            owner: pair.owner.clone(),
            snapshot,
            corpus,
            partition,
            content: None,
            migrations: None,
        }
    }

    pub(crate) const fn observer(&self) -> Option<Observer> {
        self.observer
    }

    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }

    pub(crate) const fn snapshot(&self) -> &'a Snapshot {
        self.snapshot
    }

    pub(crate) const fn corpus(&self) -> &'a CorpusPlan {
        self.corpus
    }

    pub(crate) const fn partition(&self) -> &'a PartitionPlan {
        self.partition
    }

    pub(crate) const fn content(&self) -> Option<&'a ContentPlan> {
        self.content
    }

    pub(crate) const fn migrations(&self) -> Option<&'a MigrationPlan> {
        self.migrations
    }
}

/// The narrow plan consumed by audit, append, and list lowering.
#[derive(Debug, Clone, Copy)]
pub struct ControlWritePlan<'a> {
    root: &'a Path,
    snapshot: &'a Snapshot,
    topology: &'a TopologyPlan,
    content: &'a ContentPlan,
    migrations: &'a MigrationPlan,
    guard: WriteGuardPlan<'a>,
}

impl<'a> ControlWritePlan<'a> {
    /// The root whose list file is maintained.
    #[must_use]
    pub const fn root(self) -> &'a Path {
        self.root
    }

    /// The snapshot whose list syntax was read once.
    #[must_use]
    pub const fn snapshot(self) -> &'a Snapshot {
        self.snapshot
    }

    /// The resolved corpus and owner partition.
    #[must_use]
    pub const fn topology(self) -> &'a TopologyPlan {
        self.topology
    }

    /// The common pre-write configuration guard.
    #[must_use]
    pub const fn guard(self) -> WriteGuardPlan<'a> {
        self.guard
    }

    /// Resolve one activated pair to its typed observation route.
    #[must_use]
    pub fn observation(self, pair: &Pair) -> Option<ControlObservationPlan<'a>> {
        let snapshot = self.snapshot();
        let partition = self.topology.partition();

        Some(ControlObservationPlan {
            observer: snapshot
                .program(pair)
                .and_then(catalogued)
                .map(|policy| policy.observer),
            owner: pair.owner.clone(),
            snapshot,
            corpus: self.topology.corpus(),
            partition,
            content: Some(self.content),
            migrations: Some(self.migrations),
        })
    }
}

/// The narrow plan consumed by burn census and lowering writes.
#[derive(Debug, Clone, Copy)]
pub struct BurnWritePlan<'a> {
    root: &'a Path,
    migrations: &'a MigrationPlan,
    control: ControlWritePlan<'a>,
    guard: WriteGuardPlan<'a>,
}

impl<'a> BurnWritePlan<'a> {
    /// The root whose ratchets are maintained.
    #[must_use]
    pub const fn root(self) -> &'a Path {
        self.root
    }

    /// The typed burn-family schedule.
    #[must_use]
    pub const fn migrations(self) -> &'a MigrationPlan {
        self.migrations
    }

    /// The declared-list lowering plan.
    #[must_use]
    pub const fn control(self) -> ControlWritePlan<'a> {
        self.control
    }

    /// The common pre-write configuration guard.
    #[must_use]
    pub const fn guard(self) -> WriteGuardPlan<'a> {
        self.guard
    }
}

/// The narrow plan consumed by the three generated projections.
#[derive(Debug, Clone)]
pub struct ProjectionWritePlan<'a> {
    root: &'a Path,
    workspace: &'a WorkspacePlan,
    profiles: &'a ProfilePlan,
    activations: &'a ActivationPlan,
    names: Option<crate::roster::OwnerNames>,
    guard: WriteGuardPlan<'a>,
}

impl<'a> ProjectionWritePlan<'a> {
    /// The root whose projections are maintained.
    #[must_use]
    pub const fn root(&self) -> &'a Path {
        self.root
    }

    /// The workspace packages owning projected sources.
    #[must_use]
    pub const fn workspace(&self) -> &'a WorkspacePlan {
        self.workspace
    }

    /// The finite source lists consumed by projection analyzers.
    #[must_use]
    pub const fn profiles(&self) -> &'a ProfilePlan {
        self.profiles
    }

    /// The compiled projection subscriptions.
    #[must_use]
    pub const fn activations(&self) -> &'a ActivationPlan {
        self.activations
    }

    /// The declared owner-name reconciliation.
    #[must_use]
    pub const fn names(&self) -> Option<&crate::roster::OwnerNames> {
        self.names.as_ref()
    }

    /// The common pre-write configuration guard.
    #[must_use]
    pub const fn guard(&self) -> WriteGuardPlan<'a> {
        self.guard
    }
}

/// The narrow plan consumed by publication assembly writes.
#[derive(Debug, Clone, Copy)]
pub struct AssemblyWritePlan<'a> {
    root: &'a Path,
    publications: &'a PublicationPlan,
    guard: WriteGuardPlan<'a>,
}

impl<'a> AssemblyWritePlan<'a> {
    /// The root whose publications are maintained.
    #[must_use]
    pub const fn root(self) -> &'a Path {
        self.root
    }

    /// The typed publication schedule.
    #[must_use]
    pub const fn publications(self) -> &'a PublicationPlan {
        self.publications
    }

    /// The common pre-write configuration guard.
    #[must_use]
    pub const fn guard(self) -> WriteGuardPlan<'a> {
        self.guard
    }
}

/// The narrow plan consumed by both label-fix profiles.
#[derive(Debug, Clone, Copy)]
pub struct FixWritePlan<'a> {
    root: &'a Path,
    workspace: &'a WorkspacePlan,
    profiles: &'a ProfilePlan,
    guard: WriteGuardPlan<'a>,
}

impl<'a> FixWritePlan<'a> {
    /// The root whose labels are maintained.
    #[must_use]
    pub const fn root(self) -> &'a Path {
        self.root
    }

    /// The workspace packages owning the fix inputs.
    #[must_use]
    pub const fn workspace(self) -> &'a WorkspacePlan {
        self.workspace
    }

    /// The finite source lists consumed by the fix analyzers.
    #[must_use]
    pub const fn profiles(self) -> &'a ProfilePlan {
        self.profiles
    }

    /// The common pre-write configuration guard.
    #[must_use]
    pub const fn guard(self) -> WriteGuardPlan<'a> {
        self.guard
    }
}

/// Why an execution plan could not be formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// The declared surface was not a configuration.
    Configuration(Vec<Refusal>),
    /// Materialized corpus facts could not enter the topology language.
    Topology(Vec<TopologyDefect>),
}

/// A materialized corpus fact the topology language cannot represent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyDefect {
    /// A path cannot become the text every declared pattern decides over.
    NonTextPath(BytePath),
}

impl fmt::Display for TopologyDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTextPath(path) => {
                write!(
                    formatter,
                    "{}: the path is not text, so no pattern decides it",
                    path.display()
                )
            }
        }
    }
}

/// The corpus after universe materialization and ignore removal, beside its partition.
#[derive(Debug)]
pub struct TopologyPlan {
    corpus: CorpusPlan,
    partition: PartitionPlan,
}

impl TopologyPlan {
    /// The materialized corpus projection.
    #[must_use]
    pub const fn corpus(&self) -> &CorpusPlan {
        &self.corpus
    }

    /// The declared owner partition.
    #[must_use]
    pub const fn partition(&self) -> &PartitionPlan {
        &self.partition
    }

    /// Select paths inside one owner share, then subtract the exclusion union.
    #[must_use]
    pub fn select(
        &self,
        owner: &str,
        include: &[AbnfPattern],
        exclude: &[AbnfPattern],
    ) -> BTreeSet<BytePath> {
        self.corpus
            .readable()
            .iter()
            .filter(|(path, _text)| self.partition.owner(path) == Some(owner))
            .filter(|(_path, text)| include.iter().any(|pattern| pattern.admits(text)))
            .filter(|(_path, text)| !exclude.iter().any(|pattern| pattern.admits(text)))
            .map(|(path, _text)| path.clone())
            .collect()
    }

    /// Whether two prefix domains can read an occurrence from one resolved source.
    #[must_use]
    pub fn prefix_domains_overlap(
        one: &PrefixNumbers,
        one_sources: &BTreeSet<BytePath>,
        other: &PrefixNumbers,
        other_sources: &BTreeSet<BytePath>,
    ) -> bool {
        one.overlaps(other) && one_sources.intersection(other_sources).next().is_some()
    }
}

/// A path's relation to the compiled corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathParticipation {
    /// The declared base universe never held the path.
    Absent,
    /// The base held the path and the declared ignore union removed it.
    Ignored,
    /// The path survived and may enter further projections.
    Participating,
}

/// One universe materialization with ignore removed exactly once.
#[derive(Debug)]
pub struct CorpusPlan {
    universe: UniverseKind,
    base: Vec<BytePath>,
    ignored: BTreeSet<BytePath>,
    participating: BTreeSet<BytePath>,
    native: Vec<PathBuf>,
    readable: BTreeMap<BytePath, String>,
    ignored_by: BTreeMap<String, usize>,
    findings: Vec<Finding>,
}

impl CorpusPlan {
    pub(crate) fn compile(
        root: &Path,
        universe: UniverseKind,
        ignore: &[IgnoreRow],
    ) -> Result<Self, Vec<TopologyDefect>> {
        let (base, findings) = materialize(root, universe);
        let defects: Vec<TopologyDefect> = base
            .iter()
            .filter(|path| std::str::from_utf8(path.as_bytes()).is_err())
            .cloned()
            .map(TopologyDefect::NonTextPath)
            .collect();

        if !defects.is_empty() {
            return Err(defects);
        }

        let mut ignored = BTreeSet::new();
        let mut participating = BTreeSet::new();
        let mut readable = BTreeMap::new();
        let mut ignored_by = BTreeMap::new();

        for path in &base {
            let text = std::str::from_utf8(path.as_bytes())
                .expect("non-text paths refuse before projection");
            let matching: Vec<&IgnoreRow> = ignore
                .iter()
                .filter(|row| row.pattern().admits(text))
                .collect();

            if matching.is_empty() {
                participating.insert(path.clone());
                readable.insert(path.clone(), text.to_owned());
            } else {
                ignored.insert(path.clone());

                for row in matching {
                    *ignored_by.entry(row.name().to_owned()).or_default() += 1;
                }
            }
        }

        let native = readable.values().map(PathBuf::from).collect();

        Ok(Self {
            universe,
            base,
            ignored,
            participating,
            native,
            readable,
            ignored_by,
            findings,
        })
    }

    /// The declared universe authority.
    #[must_use]
    pub const fn universe(&self) -> UniverseKind {
        self.universe
    }

    /// Every entry the selected universe authority materialized, in byte order.
    #[must_use]
    pub fn base(&self) -> &[BytePath] {
        &self.base
    }

    /// The entries removed by the declared ignore union.
    #[must_use]
    pub const fn ignored(&self) -> &BTreeSet<BytePath> {
        &self.ignored
    }

    /// The entries every higher projection ranges over.
    #[must_use]
    pub const fn participating(&self) -> &BTreeSet<BytePath> {
        &self.participating
    }

    /// Participating paths in byte order, converted once for filesystem reads.
    #[must_use]
    pub fn native_paths(&self) -> &[PathBuf] {
        &self.native
    }

    /// Participating paths paired with the one text conversion topology made.
    #[must_use]
    pub const fn readable(&self) -> &BTreeMap<BytePath, String> {
        &self.readable
    }

    /// Per-row ignore reach, where overlapping rows each receive their tally.
    #[must_use]
    pub const fn ignored_by(&self) -> &BTreeMap<String, usize> {
        &self.ignored_by
    }

    /// Failures encountered while materializing the selected universe.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Whether a path was absent, ignored, or retained for participation.
    #[must_use]
    pub fn participation(&self, path: &BytePath) -> PathParticipation {
        if self.participating.contains(path) {
            PathParticipation::Participating
        } else if self.ignored.contains(path) {
            PathParticipation::Ignored
        } else {
            PathParticipation::Absent
        }
    }
}

/// One owner partition compiled over the post-ignore corpus.
#[derive(Debug)]
pub struct PartitionPlan {
    counts: PartitionCounts,
    attribution: BTreeMap<BytePath, String>,
    findings: Vec<Finding>,
}

impl PartitionPlan {
    fn compile(rows: &[OwnerRow], corpus: &CorpusPlan) -> Self {
        let mut counts = PartitionCounts {
            universe: corpus.base.len(),
            excluded: corpus.ignored.len(),
            excluded_by: corpus.ignored_by.clone(),
            surviving: corpus.participating.len(),
            ..PartitionCounts::default()
        };
        let mut attribution = BTreeMap::new();
        let mut findings = Vec::new();

        for path in &corpus.participating {
            let accounting: Vec<&OwnerRow> = rows
                .iter()
                .filter(|row| row.pattern.admits_path(path))
                .collect();

            match accounting.as_slice() {
                [row] => {
                    counts.accounted += 1;
                    attribution.insert(path.clone(), row.owner.clone());
                }
                [] => {
                    counts.unaccounted += 1;
                    findings.push(Finding::UnaccountedPath {
                        path: path.display(),
                    });
                }
                rows => {
                    counts.multiply_accounted += 1;
                    let mut matches: Vec<String> = rows.iter().map(ToString::to_string).collect();
                    matches.sort();
                    findings.push(Finding::MultiplyAccountedPath {
                        path: path.display(),
                        count: rows.len(),
                        matches,
                    });
                }
            }
        }

        Self {
            counts,
            attribution,
            findings,
        }
    }

    /// Partition accounting over the one compiled corpus.
    #[must_use]
    pub const fn counts(&self) -> &PartitionCounts {
        &self.counts
    }

    /// The unambiguous owner of a participating path.
    #[must_use]
    pub fn owner(&self, path: &BytePath) -> Option<&str> {
        self.attribution.get(path).map(String::as_str)
    }

    /// Every unambiguous path-to-owner attribution.
    #[must_use]
    pub const fn attribution(&self) -> &BTreeMap<BytePath, String> {
        &self.attribution
    }

    /// The attribution view consumed by the existing policy analyzers.
    #[must_use]
    pub fn attribution_view(&self) -> BTreeMap<&BytePath, &str> {
        self.attribution
            .iter()
            .map(|(path, owner)| (path, owner.as_str()))
            .collect()
    }

    /// Totality and exclusivity findings from the partition.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

/// Workspace packages and the findings made while discovering them.
#[derive(Debug)]
pub struct WorkspacePlan {
    packages: Vec<Package>,
    findings: Vec<Finding>,
}

impl WorkspacePlan {
    pub(crate) fn compile(root: &Path) -> Self {
        let mut packages = Vec::new();
        let mut findings = Vec::new();
        let mut seen_directories = BTreeSet::new();

        if !root.join("Cargo.toml").is_file() {
            return Self { packages, findings };
        }

        let Some(manifest) = Self::read_manifest(root, Path::new("."), &mut findings) else {
            return Self { packages, findings };
        };

        let members = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("members"))
            .and_then(toml::Value::as_array);

        let Some(members) = members else {
            findings.push(Finding::TraversalFailure {
                path: "Cargo.toml".to_owned(),
                message: "the root manifest declares no workspace members".to_owned(),
            });
            return Self { packages, findings };
        };

        for member in members {
            let Some(member) = member.as_str() else {
                findings.push(Finding::TraversalFailure {
                    path: "Cargo.toml".to_owned(),
                    message: "a workspace member is not a string".to_owned(),
                });
                continue;
            };

            let directories = if Self::is_member_glob(member) {
                let directories = Self::expand_member_glob(root, member);
                if directories.is_empty() {
                    findings.push(Finding::TraversalFailure {
                        path: "Cargo.toml".to_owned(),
                        message: format!(
                            "the workspace member pattern `{member}` matches no directories containing Cargo.toml"
                        ),
                    });
                }
                directories
            } else {
                vec![PathBuf::from(member)]
            };

            for directory in directories {
                if !seen_directories.insert(directory.clone()) {
                    continue;
                }

                let Some(member_manifest) = Self::read_manifest(root, &directory, &mut findings)
                else {
                    continue;
                };
                let name = member_manifest
                    .get("package")
                    .and_then(|package| package.get("name"))
                    .and_then(toml::Value::as_str);

                match name {
                    Some(name) => packages.push(Package::new(name, Self::normalize(&directory))),
                    None => findings.push(Finding::TraversalFailure {
                        path: Self::manifest_path(&directory)
                            .to_string_lossy()
                            .into_owned(),
                        message: "the manifest declares no package name".to_owned(),
                    }),
                }
            }
        }

        packages.sort();

        Self { packages, findings }
    }

    /// Whether a member uses only the supported `*` and `?` glob syntax.
    fn is_member_glob(member: &str) -> bool {
        (member.contains('*') || member.contains('?'))
            && !member
                .chars()
                .any(|character| matches!(character, '[' | ']' | '{' | '}'))
    }

    /// Expand a supported member glob to manifest-bearing directories.
    fn expand_member_glob(root: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut directories = vec![PathBuf::new()];

        for component in pattern.split('/') {
            if component.contains('*') || component.contains('?') {
                let mut matched_directories = Vec::new();
                for directory in &directories {
                    let Ok(entries) = fs::read_dir(root.join(directory)) else {
                        continue;
                    };
                    matched_directories.extend(entries.filter_map(Result::ok).filter_map(
                        |entry| {
                            let name = entry.file_name();
                            let name = name.to_str()?;
                            (Self::glob_component_matches(component, name) && entry.path().is_dir())
                                .then(|| directory.join(name))
                        },
                    ));
                }
                directories = matched_directories;
            } else {
                for directory in &mut directories {
                    directory.push(component);
                }
            }
        }

        directories.retain(|directory| root.join(directory).join("Cargo.toml").is_file());
        directories.sort();
        directories.dedup();
        directories
    }

    /// Match one path component without allowing wildcards to cross `/`.
    fn glob_component_matches(pattern: &str, candidate: &str) -> bool {
        let candidate: Vec<_> = candidate.chars().collect();
        let mut previous = vec![false; candidate.len() + 1];
        previous[0] = true;

        for pattern_character in pattern.chars() {
            previous = if pattern_character == '*' {
                let mut matched = false;
                previous
                    .iter()
                    .map(|previously_matched| {
                        matched |= *previously_matched;
                        matched
                    })
                    .collect()
            } else {
                std::iter::once(false)
                    .chain(candidate.iter().zip(previous.iter()).map(
                        |(candidate_character, previously_matched)| {
                            *previously_matched
                                && (pattern_character == '?'
                                    || pattern_character == *candidate_character)
                        },
                    ))
                    .collect()
            };
        }

        previous.last().copied().unwrap_or_default()
    }

    fn read_manifest(
        root: &Path,
        directory: &Path,
        findings: &mut Vec<Finding>,
    ) -> Option<toml::Table> {
        let relative = Self::manifest_path(directory);
        let text = match fs::read_to_string(root.join(&relative)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: relative.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                return None;
            }
        };

        match toml::from_str::<toml::Table>(&text) {
            Ok(table) => Some(table),
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: relative.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                None
            }
        }
    }

    fn manifest_path(directory: &Path) -> PathBuf {
        Self::normalize(directory).join("Cargo.toml")
    }

    fn normalize(directory: &Path) -> PathBuf {
        if directory == Path::new(".") {
            PathBuf::new()
        } else {
            directory.to_path_buf()
        }
    }

    /// Workspace members ordered by crate name.
    #[must_use]
    pub fn packages(&self) -> &[Package] {
        &self.packages
    }

    /// Failures encountered during generic manifest discovery.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

/// Owner activations indexed once for every policy projection.
#[derive(Debug)]
pub struct ActivationPlan {
    partitions: Vec<OwnerRow>,
    pairs: Vec<Pair>,
    owners_by_policy: BTreeMap<String, BTreeSet<String>>,
}

impl ActivationPlan {
    fn compile(snapshot: &Snapshot) -> Self {
        Self::from_parts(snapshot.partitions(), snapshot.policies())
    }

    pub(crate) fn from_parts(partitions: &[OwnerRow], pairs: &[Pair]) -> Self {
        let mut owners_by_policy: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for pair in pairs {
            if pair.family.is_none() {
                owners_by_policy
                    .entry(pair.policy.clone())
                    .or_default()
                    .insert(pair.owner.clone());
            }
        }

        Self {
            partitions: partitions.to_vec(),
            pairs: pairs.to_vec(),
            owners_by_policy,
        }
    }

    /// Activated owner-and-policy pairs in declaration order.
    #[must_use]
    pub fn pairs(&self) -> &[Pair] {
        &self.pairs
    }

    /// Owners activating one singleton policy.
    #[must_use]
    pub fn owners(&self, policy: &str) -> Option<&BTreeSet<String>> {
        self.owners_by_policy.get(policy)
    }

    /// The compiled routing view for one policy projection.
    #[must_use]
    pub fn subscription(&self, policy: &str) -> Subscription<'_> {
        Subscription::planned(&self.partitions, self.owners(policy))
    }
}

/// The exact ownership and finite prose surface of one label-calculus run.
#[derive(Debug)]
pub struct LabelPlan {
    adoption: Adoption,
    sources: Vec<PathBuf>,
    findings: Vec<Finding>,
}

impl LabelPlan {
    fn compile(
        snapshot: &Snapshot,
        topology: &TopologyPlan,
        workspace: &WorkspacePlan,
        activations: &ActivationPlan,
        publications: &PublicationPlan,
    ) -> Self {
        let names = snapshot.declared_owner_names();
        let mut adoption = index_signature(
            workspace.packages(),
            names.as_ref(),
            publications.assemblies(),
            snapshot.kind_registry(),
        )
        .with_declared_owners(snapshot.owners(), snapshot.root_owner());

        let mut path_owners: Vec<_> = topology
            .partition()
            .attribution()
            .iter()
            .filter_map(|(path, owner)| {
                let prefix = Prefix::parse(owner)?;
                let owner = adoption.owner_of_prefix(&prefix)?.clone();
                let path = topology.corpus().readable().get(path)?;
                Some((PathBuf::from(path), owner))
            })
            .collect();

        path_owners.extend(
            workspace
                .packages()
                .iter()
                .map(|package| (package.directory().to_path_buf(), owner_of_package(package))),
        );

        adoption = adoption.with_path_owners(path_owners);

        let sources =
            activations
                .owners("labels.mints-well-formed")
                .map_or_else(Vec::new, |owners| {
                    topology
                        .corpus()
                        .readable()
                        .iter()
                        .filter(|(_path, text)| is_markdown(Path::new(text)))
                        .filter(|(path, _text)| {
                            topology
                                .partition()
                                .owner(path)
                                .is_some_and(|owner| owners.contains(owner))
                        })
                        .map(|(_path, text)| PathBuf::from(text))
                        .collect()
                });

        Self {
            adoption,
            sources,
            findings: topology.corpus().findings().to_vec(),
        }
    }

    /// The exact adoption relation consumed by the label engine.
    #[must_use]
    pub const fn adoption(&self) -> &Adoption {
        &self.adoption
    }

    /// Participating prose paths in corpus byte order.
    #[must_use]
    pub fn source_paths(&self) -> &[PathBuf] {
        &self.sources
    }

    /// Read the finite prose surface without performing discovery.
    #[must_use]
    pub fn read(&self, root: &Path) -> (Vec<Source>, Vec<Finding>) {
        let mut findings = self.findings.clone();
        let sources = self
            .sources
            .iter()
            .filter_map(|path| match fs::read_to_string(root.join(path)) {
                Ok(text) => Some(Source::new(path.clone(), text)),
                Err(error) => {
                    findings.push(Finding::TraversalFailure {
                        path: path.to_string_lossy().into_owned(),
                        message: error.to_string(),
                    });
                    None
                }
            })
            .collect();

        (sources, findings)
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

/// One finite Rust source attributed to its workspace package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSource {
    package: String,
    package_directory: PathBuf,
    path: PathBuf,
}

impl ProfileSource {
    fn new(package: &Package, path: PathBuf) -> Self {
        Self {
            package: package.name().to_owned(),
            package_directory: package.directory().to_path_buf(),
            path,
        }
    }

    /// The workspace package owning the source.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The package root used by profile classification.
    #[must_use]
    pub fn package_directory(&self) -> &Path {
        &self.package_directory
    }

    /// The repository-relative source path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The finite Rust surfaces used by the census and projection family.
#[derive(Debug)]
pub struct ProfilePlan {
    sources: Vec<ProfileSource>,
    constant_sources: Vec<ProfileSource>,
}

impl ProfilePlan {
    fn compile(topology: &TopologyPlan, workspace: &WorkspacePlan) -> Self {
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();

        for package in workspace.packages() {
            let mut paths: Vec<_> = topology
                .corpus()
                .native_paths()
                .iter()
                .filter(|path| is_profile_source(package, path))
                .cloned()
                .collect();
            paths.sort();

            for path in paths {
                if seen.insert(path.clone()) {
                    sources.push(ProfileSource::new(package, path));
                }
            }
        }

        let mut constant_sources = Vec::new();

        for package in workspace.packages() {
            let production = package.directory().join("src");
            let tests = package.directory().join("src/tests");
            let mut paths: Vec<_> = topology
                .corpus()
                .native_paths()
                .iter()
                .filter(|path| path.starts_with(&production))
                .filter(|path| !path.starts_with(&tests))
                .filter(|path| is_rust(path))
                .cloned()
                .collect();
            paths.sort();
            constant_sources.extend(
                paths
                    .into_iter()
                    .map(|path| ProfileSource::new(package, path)),
            );
        }

        Self {
            sources,
            constant_sources,
        }
    }

    /// Sources shared by the test, to-do, commentary, and matrix passes.
    #[must_use]
    pub fn sources(&self) -> &[ProfileSource] {
        &self.sources
    }

    /// Production sources read by the constant profile.
    #[must_use]
    pub fn constant_sources(&self) -> &[ProfileSource] {
        &self.constant_sources
    }
}

fn is_profile_source(package: &Package, path: &Path) -> bool {
    is_rust(path)
        && ["src", "tests"]
            .iter()
            .any(|directory| path.starts_with(package.directory().join(directory)))
}

fn is_rust(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
}

/// Typed selections for the three content policies.
#[derive(Debug)]
pub struct ContentPlan {
    spdx: crate::spdx::SelectionPlan,
    interchange: crate::interchange::SelectionPlan,
    references: crate::reference::SelectionPlan,
}

impl ContentPlan {
    fn compile(snapshot: &Snapshot, topology: &TopologyPlan) -> Self {
        let attribution = topology.partition().attribution_view();

        Self {
            spdx: crate::spdx::selection_plan(snapshot.spdx(), &attribution),
            interchange: crate::interchange::selection_plan(snapshot.interchange(), &attribution),
            references: crate::reference::selection_plan(snapshot.references(), &attribution),
        }
    }

    /// The constitutive SPDX selection.
    #[must_use]
    pub const fn spdx(&self) -> &crate::spdx::SelectionPlan {
        &self.spdx
    }

    /// The interchange exclusion-and-gloss selection.
    #[must_use]
    pub const fn interchange(&self) -> &crate::interchange::SelectionPlan {
        &self.interchange
    }

    /// The file-path exclusion-and-gloss selection.
    #[must_use]
    pub const fn references(&self) -> &crate::reference::SelectionPlan {
        &self.references
    }
}

/// One migration census paired with the ratchet rows it is judged against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurnRun {
    list: BurnList,
    declared: Vec<RegisterRow>,
}

impl BurnRun {
    /// The family and finite surfaces the census reads.
    #[must_use]
    pub const fn list(&self) -> &BurnList {
        &self.list
    }

    /// The declared ratchet rows for this family.
    #[must_use]
    pub fn declared(&self) -> &[RegisterRow] {
        &self.declared
    }
}

/// The finite migration schedule compiled once from the declared snapshot.
#[derive(Debug)]
pub struct MigrationPlan {
    runs: Vec<BurnRun>,
    findings: Vec<Finding>,
}

impl MigrationPlan {
    fn compile(snapshot: &Snapshot) -> Self {
        let runs = index_burn_lists(snapshot)
            .into_iter()
            .map(|list| BurnRun {
                declared: declared_rows(snapshot, list.family()),
                list,
            })
            .collect();
        let findings = undeclared_surfaces(snapshot);

        Self { runs, findings }
    }

    /// Burn-family runs in the catalog's stable order.
    #[must_use]
    pub fn runs(&self) -> &[BurnRun] {
        &self.runs
    }

    /// Findings formed while compiling the migration schedule.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

/// One publication programme run, carrying both owner and assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyRun {
    owner: String,
    assembly: Assembly,
}

impl AssemblyRun {
    /// The owner declaring the publication.
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The assembly to verify or publish.
    #[must_use]
    pub const fn assembly(&self) -> &Assembly {
        &self.assembly
    }
}

/// Publication rows with their derived targets and containment defects.
#[derive(Debug)]
pub struct PublicationPlan {
    #[cfg(test)]
    publications: Publications,
    runs: Vec<AssemblyRun>,
    assemblies: Vec<Assembly>,
    generated_targets: Vec<PathBuf>,
    defects: Vec<PublicationDefect>,
}

impl PublicationPlan {
    fn compile(snapshot: &Snapshot) -> Self {
        let publications = Publications::new(snapshot.declared_publications());
        let runs = publications
            .rows()
            .iter()
            .map(|publication| AssemblyRun {
                owner: publication.owner().to_owned(),
                assembly: publication.assembly().clone(),
            })
            .collect();
        let assemblies = publications
            .rows()
            .iter()
            .map(|publication| publication.assembly().clone())
            .collect();
        let generated_targets = publications.generated_targets();
        let defects = publications.defects();

        Self {
            #[cfg(test)]
            publications,
            runs,
            assemblies,
            generated_targets,
            defects,
        }
    }

    /// The decoded publication programme input.
    #[must_use]
    #[cfg(test)]
    pub const fn publications(&self) -> &Publications {
        &self.publications
    }

    /// Typed publication runs in declaration order.
    #[must_use]
    pub fn runs(&self) -> &[AssemblyRun] {
        &self.runs
    }

    /// Assemblies in publication declaration order.
    #[must_use]
    pub fn assemblies(&self) -> &[Assembly] {
        &self.assemblies
    }

    /// Generated targets derived once from the publication rows.
    #[must_use]
    pub fn generated_targets(&self) -> &[PathBuf] {
        &self.generated_targets
    }

    /// Generator uniqueness and self-containment defects.
    #[must_use]
    pub fn defects(&self) -> &[PublicationDefect] {
        &self.defects
    }
}

/// Activated dependency templates and their deterministic run order.
#[derive(Debug)]
pub struct DependencyPlan {
    pairs: Vec<Pair>,
    requirements: Vec<DependencyRequirement>,
    schedule: Vec<Pair>,
}

#[derive(Debug)]
struct DependencyRequirement {
    requiring: Pair,
    scope: Scope,
    owner: RequiredOwner,
    policy: &'static str,
}

#[derive(Debug)]
enum RequiredOwner {
    Exact(String),
    Cited,
}

impl DependencyPlan {
    fn compile(snapshot: &Snapshot) -> Self {
        Self::from_parts(snapshot.policies(), snapshot.root_owner())
    }

    fn from_parts(pairs: &[Pair], root_owner: Option<&str>) -> Self {
        let mut requirements = Vec::new();

        for pair in pairs {
            let Some(policy) = catalogued(&pair.policy) else {
                continue;
            };

            for dependency in policy.dependencies {
                let owner = match dependency.scope {
                    Scope::SameOwner => RequiredOwner::Exact(pair.owner.clone()),
                    Scope::FixedOwner => {
                        let Some(root_owner) = root_owner else {
                            continue;
                        };
                        RequiredOwner::Exact(root_owner.to_owned())
                    }
                    Scope::CitedOwner => RequiredOwner::Cited,
                };
                requirements.push(DependencyRequirement {
                    requiring: pair.clone(),
                    scope: dependency.scope,
                    owner,
                    policy: dependency.policy,
                });
            }
        }

        let schedule = dependency_schedule(pairs, &requirements);

        Self {
            pairs: pairs.to_vec(),
            requirements,
            schedule,
        }
    }

    /// Dependency findings instantiated by the cited-owner relation.
    #[must_use]
    pub fn verify(&self, citations: &[CitedEdge]) -> Vec<Finding> {
        let declared: BTreeSet<&Pair> = self.pairs.iter().collect();
        let cited = first_citations(citations);
        let mut findings = Vec::new();

        for requirement in &self.requirements {
            match &requirement.owner {
                RequiredOwner::Exact(owner) => {
                    record_missing(&declared, requirement, owner, None, &mut findings);
                }
                RequiredOwner::Cited => {
                    for ((owner, target), location) in &cited {
                        if owner == &requirement.requiring.owner {
                            record_missing(
                                &declared,
                                requirement,
                                target,
                                Some(location),
                                &mut findings,
                            );
                        }
                    }
                }
            }
        }

        findings.sort_by(|left, right| dependency_sort_key(left).cmp(&dependency_sort_key(right)));
        findings
    }

    /// A stable prerequisite-before-dependent schedule over activated pairs.
    #[must_use]
    pub fn schedule(&self) -> &[Pair] {
        &self.schedule
    }
}

fn first_citations(citations: &[CitedEdge]) -> BTreeMap<(String, String), Location> {
    let mut first: BTreeMap<(String, String), Location> = BTreeMap::new();

    for edge in citations {
        let key = (edge.owner.clone(), edge.target.clone());
        first
            .entry(key)
            .and_modify(|held| {
                if edge.location < *held {
                    *held = edge.location.clone();
                }
            })
            .or_insert_with(|| edge.location.clone());
    }

    first
}

fn record_missing(
    declared: &BTreeSet<&Pair>,
    requirement: &DependencyRequirement,
    owner: &str,
    location: Option<&Location>,
    findings: &mut Vec<Finding>,
) {
    let required = Pair::singleton(owner, requirement.policy);

    if !declared.contains(&required) {
        findings.push(Finding::MissingPolicyDependency {
            owner: requirement.requiring.owner.clone(),
            policy: requirement.requiring.policy.clone(),
            scope: requirement.scope.as_str(),
            required_owner: required.owner,
            required_policy: required.policy,
            location: location.cloned(),
        });
    }
}

fn dependency_sort_key(finding: &Finding) -> (&str, &str, &str, &str, &str) {
    match finding {
        Finding::MissingPolicyDependency {
            owner,
            policy,
            scope,
            required_owner,
            required_policy,
            ..
        } => (owner, policy, scope, required_owner, required_policy),
        _ => ("", "", "", "", ""),
    }
}

fn dependency_schedule(pairs: &[Pair], requirements: &[DependencyRequirement]) -> Vec<Pair> {
    let declared: BTreeSet<Pair> = pairs.iter().cloned().collect();
    let mut successors: BTreeMap<Pair, BTreeSet<Pair>> = BTreeMap::new();
    let mut indegree: BTreeMap<Pair, usize> =
        declared.iter().cloned().map(|pair| (pair, 0)).collect();

    for requirement in requirements {
        let required: Vec<Pair> = match &requirement.owner {
            RequiredOwner::Exact(owner) => vec![Pair::singleton(owner, requirement.policy)],
            RequiredOwner::Cited => declared
                .iter()
                .filter(|candidate| {
                    candidate.owner != requirement.requiring.owner
                        && candidate.policy == requirement.policy
                        && candidate.family.is_none()
                })
                .cloned()
                .collect(),
        };

        for required in required {
            if !declared.contains(&required) || required == requirement.requiring {
                continue;
            }
            if successors
                .entry(required)
                .or_default()
                .insert(requirement.requiring.clone())
            {
                *indegree.entry(requirement.requiring.clone()).or_default() += 1;
            }
        }
    }

    let mut ready: BTreeSet<Pair> = indegree
        .iter()
        .filter(|(_pair, degree)| **degree == 0)
        .map(|(pair, _degree)| pair.clone())
        .collect();
    let mut ordered = Vec::with_capacity(declared.len());

    while let Some(pair) = ready.pop_first() {
        if let Some(dependents) = successors.get(&pair) {
            for dependent in dependents {
                let Some(degree) = indegree.get_mut(dependent) else {
                    continue;
                };
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
        ordered.push(pair);
    }

    if ordered.len() != declared.len() {
        let remaining: Vec<Pair> = declared
            .into_iter()
            .filter(|pair| !ordered.contains(pair))
            .collect();
        ordered.extend(remaining);
    }

    ordered
}

fn materialize(root: &Path, universe: UniverseKind) -> (Vec<BytePath>, Vec<Finding>) {
    match universe {
        UniverseKind::GitTracked => git_tracked(root),
        UniverseKind::AsWritten => as_written(root),
    }
}

fn git_tracked(root: &Path) -> (Vec<BytePath>, Vec<Finding>) {
    let mut findings = Vec::new();
    let output = match std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        Ok(output) => {
            findings.push(Finding::TraversalFailure {
                path: root.display().to_string(),
                message: format!(
                    "git ls-files: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
            return (Vec::new(), findings);
        }
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: root.display().to_string(),
                message: format!("git ls-files: {error}"),
            });
            return (Vec::new(), findings);
        }
    };
    let mut paths = Vec::new();

    for record in output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
    {
        match BytePath::from_bytes(record.to_vec()) {
            Ok(path) => paths.push(path),
            Err(defect) => findings.push(Finding::TraversalFailure {
                path: String::from_utf8_lossy(record).into_owned(),
                message: defect.to_string(),
            }),
        }
    }

    paths.sort();
    (paths, findings)
}

fn as_written(root: &Path) -> (Vec<BytePath>, Vec<Finding>) {
    let mut paths = Vec::new();
    let mut findings = Vec::new();

    collect_as_written(root, Path::new(""), &mut paths, &mut findings);
    paths.sort();
    paths.dedup();

    (paths, findings)
}

fn collect_as_written(
    root: &Path,
    relative: &Path,
    paths: &mut Vec<BytePath>,
    findings: &mut Vec<Finding>,
) {
    let entries = match fs::read_dir(root.join(relative)) {
        Ok(entries) => entries,
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: relative.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: relative.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };
        let child = relative.join(entry.file_name());
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: child.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        if file_type.is_dir() {
            collect_as_written(root, &child, paths, findings);
            continue;
        }

        match byte_path(&child) {
            Ok(path) => paths.push(path),
            Err(message) => findings.push(Finding::TraversalFailure {
                path: child.to_string_lossy().into_owned(),
                message,
            }),
        }
    }
}

#[cfg(unix)]
fn byte_path(path: &Path) -> Result<BytePath, String> {
    use std::os::unix::ffi::OsStrExt as _;

    BytePath::from_bytes(path.as_os_str().as_bytes().to_vec()).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn byte_path(path: &Path) -> Result<BytePath, String> {
    let text = path
        .to_str()
        .ok_or_else(|| String::from("the native path is not text"))?;

    BytePath::from_bytes(text.as_bytes().to_vec()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use petgraph::visit::EdgeRef as _;
    use tempfile::TempDir;

    use super::{
        CorpusPlan, DependencyPlan, ExecutionPlan, PartitionPlan, PathParticipation,
        PublicationPlan, TopologyDefect, TopologyPlan, WorkspacePlan,
    };
    use crate::assembly::Publications;
    use crate::code::CodeSurface;
    use crate::declaration::AbnfPattern;
    use crate::depend::CitedEdge;
    use crate::finding::{Finding, Location};
    use crate::pattern::BytePath;
    use crate::program::{PrefixBound, PrefixNumbers};
    use crate::snapshot::{OwnerRow, Pair};
    use crate::universe::{IgnoreRow, UniverseKind};
    use crate::workspace::Package;

    fn path(display: &str) -> BytePath {
        BytePath::from_bytes(display.as_bytes()).expect("a relative fixture path")
    }

    fn row(name: &str, pattern: &str) -> IgnoreRow {
        IgnoreRow::new(
            name,
            AbnfPattern::parse(pattern).expect("a compiling fixture pattern"),
        )
    }

    fn retiring_ignore(
        base: &[BytePath],
        ignore: &[IgnoreRow],
    ) -> (BTreeSet<BytePath>, BTreeSet<BytePath>) {
        base.iter()
            .cloned()
            .partition(|path| ignore.iter().any(|row| row.matches(path)))
    }

    fn owner(name: &str, owner: &str, pattern: &str) -> OwnerRow {
        OwnerRow {
            name: name.to_owned(),
            owner: owner.to_owned(),
            pattern: AbnfPattern::parse(pattern).expect("a compiling ownership pattern"),
        }
    }

    fn write_workspace(root: &Path, members: &str) {
        fs::write(
            root.join("Cargo.toml"),
            format!("[workspace]\nmembers = [{members}]\n"),
        )
        .expect("write workspace manifest");
    }

    fn write_package(root: &Path, directory: &str, name: &str) {
        let directory = root.join(directory);
        fs::create_dir_all(&directory).expect("create package directory");
        fs::write(
            directory.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\n"),
        )
        .expect("write package manifest");
    }

    const FIXTURE_OWNERS: &str = "namespace = \"com.torrust.index.linter.owners\"\n\
        version = [1, 0, 0]\n\
        \n\
        owners = [\"INDEX\", \"DEMO\"]\n\
        partitions = [\
          { name = \"root-manifest\", owner = \"INDEX\", pattern = '%s\"Cargo.toml\"' },\
          { name = \"declared-surface\", owner = \"INDEX\", pattern = '%s\".linter\" [ \"/\" *VCHAR ]' },\
          { name = \"demo-package\", owner = \"DEMO\", pattern = '%s\"packages/demo\" [ \"/\" *VCHAR ]' },\
        ]\n\
        may_cite = []\n";

    const FIXTURE_ENVIRONMENTS: &str = "namespace = \"com.torrust.index.linter.environments\"\n\
        version = [1, 0, 0]\n\
        \n\
        reserved_kinds = [\"test\"]\n\
        reserved_extensions = [\"todo\"]\n\
        environments = [{ environment = \"Section\", kind = \"sec\" }]\n\
        extensions = []\n";

    const FIXTURE_OWNER_NAMES: &str = "namespace = \"com.torrust.index.linter.policy.owner.names\"\n\
        version = [1, 0, 0]\n\
        \n\
        [set.name-prefix-ignore]\n\
        torrust = \"torrust-\"\n";

    const FIXTURE_POLICIES: &str = "namespace = \"com.torrust.index.linter.policies\"\n\
        version = [1, 0, 0]\n\
        \n\
        policies = [\
          { owner = \"DEMO\", policy = \"labels.mints-well-formed\" },\
          { owner = \"DEMO\", policy = \"profile.tests-conform\" },\
          { owner = \"DEMO\", policy = \"projection.test-matrices-current\" },\
          { owner = \"DEMO\", policy = \"spdx.headers-conform\" },\
          { owner = \"DEMO\", policy = \"interchange.envelope-conform\" },\
          { owner = \"DEMO\", policy = \"references.file-paths-absent\" },\
          { owner = \"DEMO\", policy = \"legacy.section-references\" },\
        ]\n";

    const FIXTURE_LISTS: &str = "namespace = \"com.torrust.index.linter.lists\"\n\
        version = [1, 0, 0]\n\
        \n\
        [DEMO.\"labels.mints-well-formed\"]\nallowances = []\n\
        [DEMO.\"profile.tests-conform\"]\nallowances = []\n\
        [DEMO.\"projection.test-matrices-current\"]\nallowances = []\n\
        [DEMO.\"spdx.headers-conform\"]\npaths = []\n\
        [DEMO.\"interchange.envelope-conform\"]\npaths = []\n\
        [DEMO.\"references.file-paths-absent\"]\npath_counts = []\n\
        [DEMO.\"legacy.section-references\"]\n\
        path_counts = [{ path = \"packages/demo/docs/guide.md\", maximum = 1 }]\n";

    const FIXTURE_SHAPE: &str = "namespace = \"com.torrust.index.linter.shape\"\n\
        version = [1, 0, 0]\n\
        \n\
        universe = \"git-tracked\"\n\
        ignore = []\n";

    const FIXTURE_SPDX: &str = "namespace = \"com.torrust.index.linter.policy.spdx\"\n\
        version = [1, 0, 0]\n\
        \n\
        [set.identifier]\nfixture = \"AGPL-3.0-only\"\n\
        [set.copyright]\nfixture = \"2026 Fixture contributors\"\n\
        [owners.DEMO.identifier]\n\
        exclude = []\n\
        partitions = [{ name = \"rust\", identifier = \"fixture\", pattern = '%s\"packages/demo/src\" [ \"/\" *VCHAR ]' }]\n\
        [owners.DEMO.copyright]\n\
        exclude = []\n\
        partitions = [{ name = \"rust\", copyright = \"fixture\", pattern = '%s\"packages/demo/src\" [ \"/\" *VCHAR ]' }]\n";

    const FIXTURE_INTERCHANGE: &str = "namespace = \"com.torrust.index.linter.policy.interchange\"\n\
        version = [1, 0, 0]\n\
        \n\
        [set.interchange-documents]\ncode = \"the fictional Rust surface\"\n\
        [owners.DEMO.interchange-documents]\n\
        exclude = [{ name = \"guide\", pattern = '%s\"packages/demo/docs/guide.md\"' }]\n\
        include = [{ name = \"rust\", interchange-documents = \"code\", pattern = '%s\"packages/demo/src\" [ \"/\" *VCHAR ]' }]\n";

    const FIXTURE_REFERENCES: &str = "namespace = \"com.torrust.index.linter.policy.references.path-linking\"\n\
        version = [1, 0, 0]\n\
        \n\
        [owners.DEMO.path-references]\n\
        exclude = [{ name = \"publication\", pattern = '%s\"packages/demo/docs/spec.md\"' }]\n";

    const FIXTURE_LEGACY: &str = "namespace = \"com.torrust.index.linter.policy.legacy.section-references\"\n\
        version = [1, 0, 0]\n\
        \n\
        [owners.DEMO]\n\
        prose = '%s\"packages/demo/docs\" [ \"/\" *VCHAR ]'\n\
        code = '%s\"packages/demo/src\" [ \"/\" *VCHAR ]'\n";

    const FIXTURE_PUBLICATIONS: &str = "namespace = \"com.torrust.index.linter.policy.assembly-publications\"\n\
        version = [1, 0, 0]\n\
        \n\
        [owners.DEMO]\n\
        guide = { parts = \"packages/demo/docs/spec\", target = \"packages/demo/docs/spec.md\" }\n";

    fn write_fixture(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
        fs::write(path, text).expect("write fixture file");
    }

    fn execution_fixture() -> TempDir {
        let root = TempDir::new().expect("temporary root");
        let acute = '\u{b4}';

        for (relative, text) in [
            (".linter/owners.toml", FIXTURE_OWNERS),
            (".linter/environments.toml", FIXTURE_ENVIRONMENTS),
            (".linter/policy-owner-names.toml", FIXTURE_OWNER_NAMES),
            (".linter/policies.toml", FIXTURE_POLICIES),
            (".linter/lists.toml", FIXTURE_LISTS),
            (".linter/shape.toml", FIXTURE_SHAPE),
            (".linter/policy-spdx.toml", FIXTURE_SPDX),
            (".linter/policy-interchange.toml", FIXTURE_INTERCHANGE),
            (".linter/policy-references.toml", FIXTURE_REFERENCES),
            (
                ".linter/policy-legacy-section-references.toml",
                FIXTURE_LEGACY,
            ),
            (
                ".linter/policy-assembly-publications.toml",
                FIXTURE_PUBLICATIONS,
            ),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"packages/demo\"]\nresolver = \"2\"\n",
            ),
            (
                "packages/demo/Cargo.toml",
                "[package]\nname = \"torrust-demo\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
            ),
            (
                "packages/demo/docs/guide.md",
                "## The quarry guide stands alone · `sec:fixture:guide`\n\nCites (`sec:fixture:guide`) and retires §4.2.\n",
            ),
            (
                "packages/demo/docs/spec/assembly.md",
                "| Part |\n| --- |\n| ``010-guide.md`` |\n",
            ),
            (
                "packages/demo/docs/spec/010-guide.md",
                "Fixture publication.\n",
            ),
            (
                "packages/demo/docs/spec.md",
                "<!-- Assembled from packages/demo/docs/spec/ under packages/demo/docs/spec/assembly.md. Edit the parts, not this file. -->\n\nFixture publication.\n",
            ),
        ] {
            write_fixture(root.path(), relative, text);
        }

        write_fixture(
            root.path(),
            "packages/demo/src/lib.rs",
            &format!(
                "// SPDX-License-Identifier: AGPL-3.0-only\n\
                 // SPDX-FileCopyrightText: 2026 Fixture contributors\n\n\
                 /// Exercises the fixture.\n///\n/// {acute}test:unit:exercises-the-fixture{acute}\n\
                 #[test]\nfn exercises_the_fixture() {{}}\n\n\
                 pub const LIMIT: usize = 1;\n\
                 // TODO: replace the fictional limit after measurement\n"
            ),
        );

        crate::test_support::track_all(root.path());
        root
    }

    /// The declared universe kind alone selects tracked or as-written
    /// enumeration, and the as-written walk treats a symbolic link as one entry
    /// rather than following it.
    ///
    /// ´claim:plan:universe-kind-selects-one-byte-safe-materialization´
    /// ´test:unit:materializes-both-universe-kinds-without-following-links´
    #[test]
    fn materializes_both_universe_kinds_without_following_links() {
        let root = TempDir::new().expect("temporary root");
        fs::write(root.path().join("tracked.md"), "tracked").expect("write tracked source");
        crate::test_support::track_paths(root.path(), &[Path::new("tracked.md").to_path_buf()]);
        fs::write(root.path().join("draft.md"), "draft").expect("write untracked source");

        #[cfg(unix)]
        std::os::unix::fs::symlink("missing-target", root.path().join("link.md"))
            .expect("write fixture link");

        let tracked = CorpusPlan::compile(root.path(), UniverseKind::GitTracked, &[])
            .expect("tracked topology");
        let written = CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &[])
            .expect("as-written topology");

        assert_eq!(tracked.base(), [path("tracked.md")]);
        assert!(written.base().contains(&path("tracked.md")));
        assert!(written.base().contains(&path("draft.md")));
        #[cfg(unix)]
        assert!(written.base().contains(&path("link.md")));
    }

    /// Ignore is removed once while its membership remains inspectable, so an
    /// ignored path participates in no projection and is distinguishable from a
    /// path the base never held.
    ///
    /// ´claim:plan:ignored-and-absent-are-distinct-nonparticipation-states´
    /// ´test:unit:distinguishes-an-ignored-path-from-one-that-was-absent´
    #[test]
    fn distinguishes_an_ignored_path_from_one_that_was_absent() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir(root.path().join("quarry")).expect("create fixture directory");
        fs::write(root.path().join("quarry/kept.md"), "kept").expect("write kept source");
        fs::write(root.path().join("quarry/draft.md"), "draft").expect("write ignored source");

        let ignore = [row("draft", r#"%s"quarry/draft.md""#)];
        let corpus = CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &ignore)
            .expect("resolved corpus");
        let kept = path("quarry/kept.md");
        let draft = path("quarry/draft.md");
        let absent = path("quarry/absent.md");

        assert_eq!(
            corpus.participation(&kept),
            PathParticipation::Participating
        );
        assert_eq!(corpus.participation(&draft), PathParticipation::Ignored);
        assert_eq!(corpus.participation(&absent), PathParticipation::Absent);
        assert!(!corpus.readable().contains_key(&draft));
        assert_eq!(corpus.ignored_by().get("draft"), Some(&1));
    }

    /// A path no declared pattern can read refuses topology once in reversible
    /// display form.
    ///
    /// ´claim:plan:non-text-paths-refuse-once-before-projection´
    /// ´test:unit:refuses-one-non-text-path-once´
    #[cfg(unix)]
    #[test]
    fn refuses_one_non_text_path_once() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt as _;

        let root = TempDir::new().expect("temporary root");
        let name = OsString::from_vec(vec![b'o', b'p', b'a', b'q', b'u', b'e', 0xff]);
        fs::write(root.path().join(name), "opaque").expect("write opaque fixture path");

        let defects = CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &[])
            .expect_err("opaque topology refuses");

        assert_eq!(defects.len(), 1);
        assert_eq!(
            defects[0].to_string(),
            "opaque%FF: the path is not text, so no pattern decides it"
        );
        assert!(matches!(
            defects.as_slice(),
            [TopologyDefect::NonTextPath(_)]
        ));
    }

    /// Generic selection begins inside one owner share, admits by the inclusion
    /// union, and subtracts the exclusion union.
    ///
    /// ´claim:plan:selection-is-owner-bounded-and-subtractive´
    /// ´test:unit:bounds-selection-by-owner-before-subtracting-exclusions´
    #[test]
    fn bounds_selection_by_owner_before_subtracting_exclusions() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir_all(root.path().join("quarry/docs/notes")).expect("create quarry tree");
        fs::create_dir_all(root.path().join("spoil/docs")).expect("create spoil tree");
        fs::write(root.path().join("quarry/docs/slate.md"), "slate").expect("write slate");
        fs::write(root.path().join("quarry/docs/notes/draft.md"), "draft").expect("write draft");
        fs::write(root.path().join("spoil/docs/heap.md"), "heap").expect("write heap");

        let corpus = CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &[])
            .expect("resolved corpus");
        let rows = [
            owner("quarry", "QUARRY", r#"%s"quarry" [ "/" *VCHAR ]"#),
            owner("spoil", "SPOIL", r#"%s"spoil" [ "/" *VCHAR ]"#),
        ];
        let partition = PartitionPlan::compile(&rows, &corpus);
        let topology = TopologyPlan { corpus, partition };
        let chosen = topology.select(
            "QUARRY",
            &[AbnfPattern::parse(r#"%s"quarry" [ "/" *VCHAR ]"#).expect("inclusion")],
            &[AbnfPattern::parse(r#"%s"quarry/docs/notes" [ "/" *VCHAR ]"#).expect("exclusion")],
        );

        assert_eq!(chosen, BTreeSet::from([path("quarry/docs/slate.md")]));
    }

    /// Prefix domains overlap only where both their recognizers and resolved
    /// source memberships intersect.
    ///
    /// ´claim:plan:prefix-overlap-requires-domain-and-membership-intersection´
    /// ´test:unit:detects-prefix-overlap-only-over-shared-membership´
    #[test]
    fn detects_prefix_overlap_only_over_shared_membership() {
        let range = PrefixNumbers::new(
            "L-",
            PrefixBound::LeadingRange {
                minimum: 1,
                maximum: 30,
            },
            true,
        );
        let set = PrefixNumbers::new("L-", PrefixBound::LeadingSet(vec![4, 9]), true);
        let prose = BTreeSet::from([path("quarry/docs/slate.md")]);
        let same = BTreeSet::from([path("quarry/docs/slate.md")]);
        let code = BTreeSet::from([path("quarry/src/lode.rs")]);

        assert!(TopologyPlan::prefix_domains_overlap(
            &range, &prose, &set, &same
        ));
        assert!(!TopologyPlan::prefix_domains_overlap(
            &range, &prose, &set, &code
        ));
    }

    /// The plan's git-tracked materialization agrees path-for-path and
    /// finding-for-finding with the constructor it will replace.
    ///
    /// ´claim:plan:git-universe-agrees-with-retiring-constructor´
    /// ´test:unit:plan-universe-matches-the-current-git-constructor´
    #[test]
    fn plan_universe_matches_the_current_git_constructor() {
        let root = TempDir::new().expect("temporary root");
        fs::write(root.path().join("tracked.md"), "tracked").expect("write tracked source");
        crate::test_support::track_paths(root.path(), &[Path::new("tracked.md").to_path_buf()]);

        let legacy = [path("tracked.md")];
        let planned = CorpusPlan::compile(root.path(), UniverseKind::GitTracked, &[])
            .expect("planned corpus");

        assert_eq!(planned.base(), legacy);
        assert!(
            planned.findings().is_empty(),
            "the fixture repository is readable"
        );
    }

    /// The plan's ignored and participating sets agree with the resolver it
    /// replaces, while the plan additionally retains per-row reach.
    ///
    /// ´claim:plan:ignore-union-agrees-with-retiring-constructor´
    /// ´test:unit:plan-ignore-union-matches-the-current-constructor´
    #[test]
    fn plan_ignore_union_matches_the_current_constructor() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir(root.path().join("quarry")).expect("fixture tree");
        fs::write(root.path().join("quarry/kept.md"), "kept").expect("kept source");
        fs::write(root.path().join("quarry/draft.md"), "draft").expect("ignored source");
        let ignore = [row("draft", r#"%s"quarry/draft.md""#)];
        let planned = CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &ignore)
            .expect("planned corpus");
        let (retiring_ignored, retiring_participating) = retiring_ignore(planned.base(), &ignore);

        assert_eq!(planned.ignored(), &retiring_ignored);
        assert_eq!(planned.participating(), &retiring_participating);
    }

    /// The plan's partition counts, findings, and attribution agree with the
    /// constructor it will replace.
    ///
    /// ´claim:plan:partition-agrees-with-retiring-constructor´
    /// ´test:unit:plan-partition-matches-the-current-constructor´
    #[test]
    fn plan_partition_matches_the_current_constructor() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir(root.path().join("quarry")).expect("create fixture tree");
        fs::write(root.path().join("quarry/slate.md"), "slate").expect("write slate");

        let corpus =
            CorpusPlan::compile(root.path(), UniverseKind::AsWritten, &[]).expect("planned corpus");
        let rows = vec![owner("quarry", "QUARRY", r#"%s"quarry" [ "/" *VCHAR ]"#)];
        let planned = PartitionPlan::compile(&rows, &corpus);
        let fixture = crate::test_support::snapshot_with_partition(&rows);
        let (legacy_counts, legacy_findings) =
            crate::partition::retiring_verify(&fixture, corpus.base());
        let legacy_attribution = crate::partition::retiring_attribution(&fixture, corpus.base());
        let legacy_owned: BTreeMap<BytePath, String> = legacy_attribution
            .into_iter()
            .map(|(path, owner)| (path.clone(), owner.to_owned()))
            .collect();

        assert_eq!(planned.counts(), &legacy_counts);
        assert_eq!(planned.findings(), legacy_findings);
        assert_eq!(planned.attribution(), &legacy_owned);
    }

    /// The plan's generic workspace discovery preserves the package identities,
    /// crate-name ordering, and traversal findings of the constructor it
    /// replaces.
    ///
    /// ´claim:plan:workspace-agrees-with-retiring-constructor´
    /// ´test:unit:plan-workspace-matches-the-current-constructor´
    #[test]
    fn plan_workspace_matches_the_current_constructor() {
        let root = TempDir::new().expect("temporary root");
        fs::create_dir_all(root.path().join("packages/alpha")).expect("create fixture package");
        fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\".\", \"packages/alpha\", \"packages/absent\"]\n\n[package]\nname = \"torrust-root\"\n",
        )
        .expect("write root manifest");
        fs::write(
            root.path().join("packages/alpha/Cargo.toml"),
            "[package]\nname = \"torrust-alpha\"\n",
        )
        .expect("write member manifest");

        let (legacy_packages, legacy_findings) =
            crate::workspace::retiring_read_workspace(root.path());
        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(planned.packages(), legacy_packages);
        assert_eq!(planned.findings(), legacy_findings);
    }

    /// A literal workspace member still reads exactly the directory it names,
    /// preserving the behavior from before glob expansion.
    ///
    /// ´claim:plan:literal-workspace-members-keep-their-established-behavior´
    /// ´test:unit:keeps-literal-workspace-members-unchanged´
    #[test]
    fn keeps_literal_workspace_members_unchanged() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/one\"");
        write_package(root.path(), "packages/one", "torrust-one");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(
            planned.packages(),
            [Package::new("torrust-one", "packages/one")]
        );
        assert!(
            planned.findings().is_empty(),
            "the literal member reads: {:?}",
            planned.findings()
        );
    }

    /// A member glob expands to manifest-bearing directories in deterministic
    /// lexicographic path order.
    ///
    /// ´claim:plan:workspace-member-globs-expand-in-path-order´
    /// ´test:unit:expands-workspace-member-globs-in-path-order´
    #[test]
    fn expands_workspace_member_globs_in_path_order() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/*\"");
        write_package(root.path(), "packages/zeta", "same-name");
        write_package(root.path(), "packages/alpha", "same-name");

        assert_eq!(
            WorkspacePlan::expand_member_glob(root.path(), "packages/*"),
            vec![
                PathBuf::from("packages/alpha"),
                PathBuf::from("packages/zeta")
            ]
        );

        let planned = WorkspacePlan::compile(root.path());
        assert_eq!(
            planned.packages(),
            [
                Package::new("same-name", "packages/alpha"),
                Package::new("same-name", "packages/zeta")
            ]
        );
        assert!(
            planned.findings().is_empty(),
            "both globbed members read: {:?}",
            planned.findings()
        );
    }

    /// The `?` member glob matches exactly one character within one component,
    /// never an empty string or multiple characters.
    ///
    /// ´claim:plan:a-question-mark-member-glob-matches-one-character´
    /// ´test:unit:matches-one-character-in-workspace-member-globs´
    #[test]
    fn matches_one_character_in_workspace_member_globs() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/tw?\"");
        write_package(root.path(), "packages/two", "torrust-two");
        write_package(root.path(), "packages/twelve", "torrust-twelve");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(
            planned.packages(),
            [Package::new("torrust-two", "packages/two")]
        );
        assert!(
            planned.findings().is_empty(),
            "the one-character match reads: {:?}",
            planned.findings()
        );
    }

    /// A member glob that finds no manifest-bearing directory reports the
    /// pattern at the workspace manifest instead of silently shrinking owners.
    ///
    /// ´claim:plan:an-unmatched-workspace-member-glob-is-reported´
    /// ´test:unit:reports-a-workspace-member-glob-that-matches-nothing´
    #[test]
    fn reports_a_workspace_member_glob_that_matches_nothing() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/*\"");
        fs::create_dir_all(root.path().join("packages/empty")).expect("create empty directory");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(planned.packages(), []);
        assert_eq!(
            planned.findings(),
            [Finding::TraversalFailure {
                path: "Cargo.toml".to_owned(),
                message: "the workspace member pattern `packages/*` matches no directories containing Cargo.toml"
                    .to_owned(),
            }]
        );
    }

    /// A member selected by both a literal and a glob joins the owner partition
    /// once, without reading or reporting its manifest twice.
    ///
    /// ´claim:plan:workspace-member-expansion-deduplicates-directories´
    /// ´test:unit:deduplicates-literal-and-glob-workspace-members´
    #[test]
    fn deduplicates_literal_and_glob_workspace_members() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/one\", \"packages/*\"");
        write_package(root.path(), "packages/one", "torrust-one");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(
            planned.packages(),
            [Package::new("torrust-one", "packages/one")]
        );
        assert!(
            planned.findings().is_empty(),
            "the deduplicated member reads: {:?}",
            planned.findings()
        );
    }

    /// A directory whose name matches a glob but which carries no `Cargo.toml`
    /// is skipped while manifest-bearing matches still join the workspace.
    ///
    /// ´claim:plan:workspace-member-globs-select-only-manifest-bearing-directories´
    /// ´test:unit:skips-globbed-directories-without-manifests´
    #[test]
    fn skips_globbed_directories_without_manifests() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/*\"");
        write_package(root.path(), "packages/real", "torrust-real");
        fs::create_dir_all(root.path().join("packages/empty")).expect("create empty directory");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(
            planned.packages(),
            [Package::new("torrust-real", "packages/real")]
        );
        assert!(
            planned.findings().is_empty(),
            "the directory without a manifest is skipped: {:?}",
            planned.findings()
        );
    }

    /// Bracket and brace forms keep literal treatment even when their entries
    /// also contain supported metacharacters.
    ///
    /// ´claim:plan:unsupported-workspace-member-glob-syntax-remains-literal´
    /// ´test:unit:keeps-unsupported-member-glob-syntax-literal´
    #[test]
    fn keeps_unsupported_member_glob_syntax_literal() {
        let root = TempDir::new().expect("temporary root");
        write_workspace(root.path(), "\"packages/[ab]*\", \"crates/{one,two}?\"");
        write_package(root.path(), "packages/[ab]*", "literal-bracket-pattern");
        write_package(root.path(), "packages/[ab]extra", "would-be-bracket-match");
        write_package(root.path(), "crates/{one,two}?", "literal-brace-pattern");
        write_package(root.path(), "crates/{one,two}x", "would-be-brace-match");

        let planned = WorkspacePlan::compile(root.path());

        assert_eq!(
            planned.packages(),
            [
                Package::new("literal-brace-pattern", "crates/{one,two}?"),
                Package::new("literal-bracket-pattern", "packages/[ab]*")
            ]
        );
        assert!(
            planned.findings().is_empty(),
            "the literal metacharacters name real directories: {:?}",
            planned.findings()
        );
    }

    /// The plan carries a fictional declaration's publication rows and every
    /// derived projection exactly as the constructor it replaces did.
    ///
    /// ´claim:plan:publications-agree-with-retiring-constructor´
    /// ´test:unit:plan-publications-match-the-current-constructor´
    #[test]
    fn plan_publications_match_the_current_constructor() {
        let fixture = execution_fixture();
        let root = fixture.path();
        let configured = crate::snapshot::configuration(root);
        let crate::snapshot::Configuration::Present(snapshot) = configured else {
            panic!("the fictional declaration loads")
        };
        let legacy = crate::assembly::retiring_index_publications(&snapshot);
        let legacy_assemblies = crate::assembly::retiring_index_assemblies(&snapshot);
        let planned = PublicationPlan::compile(&snapshot);

        assert_eq!(planned.publications(), &legacy);
        assert_eq!(planned.assemblies(), legacy_assemblies);
        assert_eq!(planned.generated_targets(), legacy.generated_targets());
        assert_eq!(planned.defects(), legacy.defects());
    }

    /// Compiled dependency templates preserve the retiring verifier's exact
    /// findings, while their stored schedule places declared prerequisites
    /// before the pairs that require them.
    ///
    /// ´claim:plan:dependencies-agree-with-retiring-constructor´
    /// ´test:unit:plan-dependencies-match-the-current-constructor´
    #[test]
    fn plan_dependencies_match_the_current_constructor() {
        let pairs = vec![
            Pair::singleton("ASSAYER", "labels.citations-imported-resolve"),
            Pair::singleton("ASSAYER", "labels.citations-import-form"),
            Pair::singleton("ASSAYER", "labels.mints-well-formed"),
        ];
        let citations = vec![CitedEdge {
            owner: "ASSAYER".to_owned(),
            target: "MUDLARK".to_owned(),
            location: Location::new("packages/assayer/a.md", "line one\nline two\n", 9),
        }];
        let planned = DependencyPlan::from_parts(&pairs, Some("INDEX"));

        assert_eq!(
            planned.verify(&citations),
            crate::depend::retiring_verify(&pairs, &citations, Some("INDEX"))
        );

        let closed = vec![
            Pair::singleton("INDEX", "labels.citations-local-resolve"),
            Pair::singleton("INDEX", "labels.mints-unique"),
            Pair::singleton("INDEX", "labels.mints-well-formed"),
        ];
        let scheduled = DependencyPlan::from_parts(&closed, Some("INDEX"));
        let position = |policy: &str| {
            scheduled
                .schedule()
                .iter()
                .position(|pair| pair.policy == policy)
                .expect("scheduled policy")
        };

        assert!(position("labels.mints-well-formed") < position("labels.mints-unique"));
        assert!(position("labels.mints-unique") < position("labels.citations-local-resolve"));
    }

    /// The activated-owner label projection agrees source-for-source and
    /// observation-for-observation with the retiring carrier and
    /// package-derived ownership.
    ///
    /// ´claim:plan:label-run-agrees-with-retiring-carrier´
    /// ´test:unit:label-plan-matches-the-current-carrier-and-observations´
    #[test]
    fn label_plan_matches_the_current_carrier_and_observations() {
        let fixture = execution_fixture();
        let root = fixture.path();
        let configured = crate::snapshot::configuration(root);
        assert!(
            configured.refusals().is_empty(),
            "the fictional declaration loads"
        );
        let plan = ExecutionPlan::compile(root, configured)
            .expect("the fictional execution plan compiles");
        let legacy_adoption = crate::application::adopted(plan.workspace().packages(), &plan);
        let (legacy_sources, legacy_findings) =
            crate::carrier::index_carrier(root, plan.topology().corpus()).read();
        let (planned_sources, planned_findings) = plan.labels().read(root);

        let source_rows = |sources: &[crate::carrier::Source]| {
            sources
                .iter()
                .map(|source| (source.path().to_path_buf(), source.text().to_owned()))
                .collect::<Vec<_>>()
        };

        assert_eq!(source_rows(&planned_sources), source_rows(&legacy_sources));
        assert_eq!(planned_findings, legacy_findings);

        let legacy =
            crate::engine::analyze(&legacy_adoption, &legacy_sources, &CodeSurface::default());
        let planned = crate::engine::analyze(
            plan.labels().adoption(),
            &planned_sources,
            &CodeSurface::default(),
        );

        assert_eq!(planned.findings(), legacy.findings());
        let planned_nodes: Vec<_> = planned.graph().node_weights().cloned().collect();
        let legacy_nodes: Vec<_> = legacy.graph().node_weights().cloned().collect();
        let planned_edges: Vec<_> = planned
            .graph()
            .edge_references()
            .map(|edge| {
                (
                    edge.source().index(),
                    edge.target().index(),
                    edge.weight().clone(),
                )
            })
            .collect();
        let legacy_edges: Vec<_> = legacy
            .graph()
            .edge_references()
            .map(|edge| {
                (
                    edge.source().index(),
                    edge.target().index(),
                    edge.weight().clone(),
                )
            })
            .collect();

        assert_eq!(planned_nodes, legacy_nodes);
        assert_eq!(planned_edges, legacy_edges);
        assert_eq!(planned.imports(), legacy.imports());
        assert_eq!(planned.sources_scanned(), legacy.sources_scanned());
        assert_eq!(planned.mints(), legacy.mints());
        assert_eq!(planned.derived_mints(), legacy.derived_mints());
        assert_eq!(planned.code_mints(), legacy.code_mints());
        assert_eq!(planned.citations_resolved(), legacy.citations_resolved());
        assert_eq!(planned.heads_validated(), legacy.heads_validated());
    }

    /// The finite test, to-do, constant, commentary, and matrix projections
    /// agree row-for-row with the analyzer-owned source walks they replace.
    ///
    /// ´claim:plan:profile-runs-agree-with-retiring-walks´
    /// ´test:unit:profile-plan-matches-the-current-census-sources-and-observations´
    #[test]
    fn profile_plan_matches_the_current_census_sources_and_observations() {
        let fixture = execution_fixture();
        let root = fixture.path();
        let configured = crate::snapshot::configuration(root);
        assert!(
            configured.refusals().is_empty(),
            "the fictional declaration loads"
        );
        let plan = ExecutionPlan::compile(root, configured)
            .expect("the fictional execution plan compiles");
        let packages = plan.workspace().packages();
        let corpus = plan.topology().corpus();
        let planned_rows: Vec<_> = plan
            .profiles()
            .sources()
            .iter()
            .map(|source| (source.package().to_owned(), source.path().to_path_buf()))
            .collect();
        let planned_constant_rows: Vec<_> = plan
            .profiles()
            .constant_sources()
            .iter()
            .map(|source| (source.package().to_owned(), source.path().to_path_buf()))
            .collect();

        assert_eq!(
            planned_rows,
            crate::census::retiring_source_rows(packages, corpus)
        );
        assert_eq!(
            planned_constant_rows,
            crate::constant::retiring_constant_source_rows(packages, corpus)
        );

        let (legacy_tests, legacy_test_findings) =
            crate::census::take_census(root, packages, corpus);
        let (planned_tests, planned_test_findings) =
            crate::census::take_planned_census(root, plan.profiles().sources());
        assert_eq!(planned_tests.tests(), legacy_tests.tests());
        assert_eq!(planned_tests.files_scanned(), legacy_tests.files_scanned());
        assert_eq!(planned_test_findings, legacy_test_findings);

        let (legacy_todos, legacy_todo_findings) =
            crate::todo::take_todo_census(root, packages, corpus);
        let (planned_todos, planned_todo_findings) =
            crate::todo::take_planned_todo_census(root, plan.profiles().sources());
        assert_eq!(planned_todos.notices(), legacy_todos.notices());
        assert_eq!(planned_todos.files_scanned(), legacy_todos.files_scanned());
        assert_eq!(planned_todo_findings, legacy_todo_findings);

        let (legacy_constants, legacy_constant_findings) =
            crate::constant::take_constant_census(root, packages, corpus);
        let (planned_constants, planned_constant_findings) =
            crate::constant::take_planned_constant_census(root, plan.profiles().constant_sources());
        assert_eq!(
            format!("{:?}", planned_constants.declarations()),
            format!("{:?}", legacy_constants.declarations())
        );
        assert_eq!(
            planned_constants.files_scanned(),
            legacy_constants.files_scanned()
        );
        assert_eq!(planned_constant_findings, legacy_constant_findings);

        let (_profile, assets, _profile_findings) =
            crate::profile::analyze_profile(packages, &legacy_tests);
        let (legacy_citations, legacy_code_findings) =
            crate::code::take_code_citations(root, packages, &assets, corpus);
        let (planned_citations, planned_code_findings) =
            crate::code::take_planned_code_citations(root, plan.profiles().sources(), &assets);
        assert_eq!(planned_citations, legacy_citations);
        assert_eq!(planned_code_findings, legacy_code_findings);

        let names = plan.snapshot().declared_owner_names();
        let subscription = plan
            .activations()
            .subscription(crate::subscribe::TEST_MATRICES_POLICY);
        let (legacy_folders, legacy_folder_findings) =
            crate::matrix::folders(packages, names.as_ref(), corpus, &subscription);
        let (planned_folders, planned_folder_findings) = crate::matrix::planned_folders(
            plan.profiles().sources(),
            names.as_ref(),
            &subscription,
        );
        assert_eq!(planned_folders, legacy_folders);
        assert_eq!(planned_folder_findings, legacy_folder_findings);
    }

    /// SPDX constitutive inclusion and the interchange and file-path diagnostic
    /// glosses preserve every governed row, selection finding, and named exclusion.
    ///
    /// ´claim:plan:content-selection-runs-agree-with-retiring-policy-copies´
    /// ´test:unit:content-plan-matches-the-three-current-policy-selections´
    #[test]
    fn content_plan_matches_the_three_current_policy_selections() {
        let fixture = execution_fixture();
        let root = fixture.path();
        let configured = crate::snapshot::configuration(root);
        assert!(
            configured.refusals().is_empty(),
            "the fictional declaration loads"
        );
        let plan = ExecutionPlan::compile(root, configured)
            .expect("the fictional execution plan compiles");
        let snapshot = plan.snapshot();
        let attribution = plan.topology().partition().attribution_view();

        let (legacy_spdx, legacy_spdx_findings) =
            crate::spdx::retiring_govern(snapshot.spdx(), &attribution);
        assert_eq!(plan.content().spdx().governed(), legacy_spdx);
        assert_eq!(plan.content().spdx().findings(), legacy_spdx_findings);
        let mut legacy_spdx_exclusions = Vec::new();
        for owner in snapshot.spdx().sections().keys() {
            legacy_spdx_exclusions.extend(crate::spdx::retiring_exclusions(
                snapshot.spdx(),
                &attribution,
                owner,
            ));
        }
        legacy_spdx_exclusions.sort();
        assert_eq!(plan.content().spdx().exclusions(), legacy_spdx_exclusions);

        let (legacy_interchange, legacy_interchange_findings) =
            crate::interchange::retiring_govern(snapshot.interchange(), &attribution);
        assert_eq!(plan.content().interchange().governed(), legacy_interchange);
        assert_eq!(
            plan.content().interchange().findings(),
            legacy_interchange_findings
        );
        let mut legacy_interchange_exclusions = Vec::new();
        for owner in snapshot.interchange().sections.keys() {
            legacy_interchange_exclusions.extend(crate::interchange::retiring_exclusions(
                snapshot.interchange(),
                &attribution,
                owner,
            ));
        }
        assert_eq!(
            plan.content().interchange().exclusions(),
            legacy_interchange_exclusions
        );

        let legacy_references =
            crate::reference::retiring_govern(snapshot.references(), &attribution);
        let legacy_reference_findings =
            crate::reference::retiring_gloss(snapshot.references(), &attribution);
        assert_eq!(plan.content().references().governed(), legacy_references);
        assert_eq!(
            plan.content().references().findings(),
            legacy_reference_findings
        );
        let mut legacy_reference_exclusions = Vec::new();
        for owner in snapshot.references().sections.keys() {
            legacy_reference_exclusions.extend(crate::reference::retiring_exclusions(
                snapshot.references(),
                &attribution,
                owner,
            ));
        }
        legacy_reference_exclusions.sort();
        assert_eq!(
            plan.content().references().exclusions(),
            legacy_reference_exclusions
        );
    }

    /// Burn surfaces, ratchet rows, census observations, publication rows,
    /// assembled bytes, and assembly findings agree with the retiring paths.
    ///
    /// ´claim:plan:migration-and-publication-runs-agree-with-retiring-programs´
    /// ´test:unit:migration-and-publication-runs-match-the-current-programs´
    #[test]
    fn migration_and_publication_runs_match_the_current_programs() {
        let fixture = execution_fixture();
        let root = fixture.path();
        let configured = crate::snapshot::configuration(root);
        assert!(
            configured.refusals().is_empty(),
            "the fictional declaration loads"
        );
        let plan = ExecutionPlan::compile(root, configured)
            .expect("the fictional execution plan compiles");
        let snapshot = plan.snapshot();

        let legacy_lists = crate::burn::index_burn_lists(snapshot);
        assert_eq!(plan.migrations().runs().len(), legacy_lists.len());
        for (run, legacy) in plan.migrations().runs().iter().zip(&legacy_lists) {
            assert_eq!(run.list(), legacy);
            assert_eq!(
                run.declared(),
                crate::burn::declared_rows(snapshot, legacy.family())
            );
        }

        let (legacy_taken, legacy_findings) =
            crate::burn::verify_all(root, snapshot, plan.topology().corpus());
        let mut planned_taken = Vec::new();
        let mut planned_findings = plan.migrations().findings().to_vec();
        for run in plan.migrations().runs() {
            let (census, census_findings) =
                crate::burn::census(root, run.list(), plan.topology().corpus());
            planned_findings.extend(census_findings);
            planned_findings.extend(crate::burn::verify(run.list(), &census, run.declared()));
            planned_taken.push((run.list().clone(), census));
        }
        assert_eq!(planned_findings, legacy_findings);
        assert_eq!(planned_taken.len(), legacy_taken.len());
        for ((planned_list, planned), (legacy_list, legacy)) in
            planned_taken.iter().zip(&legacy_taken)
        {
            assert_eq!(planned_list, legacy_list);
            assert_eq!(planned.occurrences(), legacy.occurrences());
            assert_eq!(planned.rows(), legacy.rows());
            assert_eq!(planned.files_scanned(), legacy.files_scanned());
        }

        let legacy_publications = Publications::new(snapshot.declared_publications());
        assert_eq!(
            plan.publications().runs().len(),
            legacy_publications.rows().len()
        );
        for (run, legacy) in plan
            .publications()
            .runs()
            .iter()
            .zip(legacy_publications.rows())
        {
            assert_eq!(run.owner(), legacy.owner());
            assert_eq!(run.assembly(), legacy.assembly());

            let (planned, planned_findings) =
                crate::assembly::verify_assembly(root, run.assembly());
            let (retiring, retiring_findings) =
                crate::assembly::verify_assembly(root, legacy.assembly());
            assert_eq!(planned.dormant, retiring.dormant);
            assert_eq!(planned.draft, retiring.draft);
            assert_eq!(planned.parts, retiring.parts);
            assert_eq!(planned.text, retiring.text);
            assert_eq!(planned_findings, retiring_findings);
            assert_eq!(
                crate::assembly::part_duplicate_mints(root, run.assembly(), run.owner()),
                crate::assembly::part_duplicate_mints(root, legacy.assembly(), legacy.owner())
            );
        }
    }
}
