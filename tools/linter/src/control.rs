// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The burn command's controlled maintenance modes: audit, append and the
//! lowering write.
//!
//! The three are modes of one command rather than three subcommands because they
//! are three readings of one relation — what the tree holds against what the
//! lists declare (´spec:commandcontract:burn-modes´). Bare verification is the
//! fourth reading and the default.
//!
//! *Audit* is read-only and formally selective: it takes a request naming the
//! pairs and identities to compare and answers with each one classified exactly
//! once — equal, grown, shrunk, or stale — beside the structurally parseable
//! defects a declaration can carry. It repairs none of them. Audit is where a
//! caller finds out what the lists say before deciding what to do about it,
//! which is why it reads the list file permissively: a defect the ordinary
//! loader refuses is a defect audit must be able to report.
//!
//! *Append* is the one growth door (ADR-L-020, The migration disciplines).
//! It re-reads the tree, and accepts a row only when that row names growth the
//! tree currently holds and its ceiling equals exactly that current observation,
//! so an append records a debt the tree already carries and cannot pre-authorize
//! a violation not yet written. The batch is atomic: one refused row refuses all
//! of them, so a request is accepted as the ruling it claims to be rather than
//! partially applied into a state nobody ruled on.
//!
//! *Write* is the lowering writer and takes no request at all, because lowering
//! needs no authority: it records what the corpus has already earned. It lowers
//! a ceiling to what the census now finds and removes a row whose occurrences
//! are gone, and it never raises one.
//!
//! Request bytes are caller data rather than a delegated path: the command-line
//! entrypoint reads them from standard input, and the library surface accepts the
//! bytes directly (´dec:controlsurface:request-stream´).
//!
//! The response file and the stdout object are the same bytes, made durable as a
//! file and then copied to stdout (´req:commandcontract:control-artifacts´).
//! The durable file is a direct child of the corpus root, selected through a
//! lexical guard that never probes the response path
//! (´dec:controlsurface:lexical-receipt´). The pair is the whole provenance the
//! corpus gets, since a direct hand edit leaves none.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`a_family_less_pair_renders_as_it_always_did`] | control | A pair the corpus declares once renders exactly as it always did: two key components and no third. The family component is what an instanced pair gains, not a spelling every pair now carries, so a declaration holding no instanced pair crosses a rewrite byte for byte. |
//! | [`path_set_requests_carry_a_path_and_no_ceiling`] | control | The path-set request syntax uses the existing path selector for both modes. An audit selector and an append row therefore each carry a path and no maximum, because a file-level identity has no ceiling to choose. |

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::burn::{census, index_burn_lists};
use crate::catalogue::{Codec, Observer, catalogued, spans_corpus};
use crate::finding::Finding;
use crate::pattern::BytePath;
use crate::plan::{
    ControlObservationPlan, ControlWritePlan, CorpusPlan, ExecutionPlan, PartitionPlan,
};
use crate::reference::Lexicon;
use crate::report::{ReportedFinding, verify_write_guard};
use crate::snapshot::{
    Allowance, Configuration, CoreConfiguration, DIRECTORY, DeclarationCore, LISTS_FILE, OwnerRow,
    Pair, PathCount, Rows, Snapshot, core_configuration,
};
use crate::spdx::violating_paths;
use crate::surface::SurfaceAst;

/// The request schema this binary reads.
const REQUEST_SCHEMA: u32 = 1;

/// Which maintenance mode is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// The read-only, formally selective comparison.
    Audit,
    /// The atomic growth door.
    Append,
}

impl Operation {
    /// The mode's identifier, as a request and a response spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::Append => "append",
        }
    }
}

/// The evidence an append travels with.
///
/// The linter validates that all three strings are nonempty and preserves them
/// byte for byte in the response. It neither decides that the named person may
/// rule nor resolves the ruling reference, and no reader should read the receipt
/// as authentication (ADR-L-020, The migration disciplines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    /// Who ruled.
    pub authorized_by: String,
    /// Which ruling.
    pub ruling: String,
    /// Why.
    pub reason: String,
}

impl Authority {
    /// Whether all three strings carry something.
    const fn is_complete(&self) -> bool {
        !self.authorized_by.is_empty() && !self.ruling.is_empty() && !self.reason.is_empty()
    }
}

/// One tolerated violation's identity, in whichever codec its policy selects.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(untagged)]
pub enum Identity {
    /// The legacy codec's identity: the file holding the occurrences.
    Path {
        /// The file.
        path: BytePath,
    },
    /// The ordinary codec's identity: a digest of the violation's own fields.
    Fingerprint {
        /// The digest.
        fingerprint: String,
    },
}

/// One row of a list, as a request and a response carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// What the row tolerates.
    #[serde(flatten)]
    pub identity: Identity,
    /// How many occurrences of it are tolerated, when the codec has a ceiling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<u64>,
}

impl Row {
    /// The ceiling the universal comparison reads from this row.
    ///
    /// A path set omits the field because a path either stands in the set or
    /// does not. Its standing row therefore contributes the sole positive
    /// value the codec can express.
    const fn ceiling(&self) -> u64 {
        match self.maximum {
            Some(maximum) => maximum,
            None => 1,
        }
    }
}

/// One identity compared: what the list declares against what the tree holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    /// The identity compared.
    pub identity: Identity,
    /// What the list declares.
    pub registered: u64,
    /// What the tree holds.
    pub observed: u64,
}

/// One structurally parseable defect a declaration carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anomaly {
    /// Which defect it is.
    pub code: &'static str,
    /// What the defect is, in words.
    pub message: String,
    /// The identity it attaches to, where it attaches to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<Identity>,
}

/// One target of an audit, with every selected identity classified exactly once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditTarget {
    /// The owner whose list was compared.
    pub owner: String,
    /// The policy whose list was compared.
    pub policy: String,
    /// The codec the list is written in.
    pub syntax: &'static str,
    /// The authority the request carried, preserved byte for byte.
    pub authority: Authority,
    /// The identities the request selected.
    pub requested: Vec<Identity>,
    /// The list as it stands.
    pub state: Vec<Row>,
    /// Registered equals observed, and both are positive.
    pub equal: Vec<Comparison>,
    /// Observed exceeds registered.
    pub growth: Vec<Comparison>,
    /// Registered exceeds observed, and observed remains positive.
    pub shrink: Vec<Comparison>,
    /// Registered is positive and the tree now holds nothing.
    pub stale: Vec<Comparison>,
    /// Why paths in the owner's share are absent from one half's observation.
    pub explanations: Vec<String>,
    /// The defects the declaration carries and the audit repairs none of.
    pub anomalies: Vec<Anomaly>,
}

/// The audit's response object, which is also its receipt.
#[derive(Debug, Clone, Serialize)]
pub struct AuditResponse {
    /// The mode that ran.
    pub operation: &'static str,
    /// The repository root the comparison was taken over.
    pub root: String,
    /// The request source.
    pub input: String,
    /// The response file.
    pub output: String,
    /// The digest of the request bytes.
    pub request_sha256: String,
    /// The digest of the list file as it stands.
    pub lists_sha256: String,
    /// Whether the audit found neither anomaly nor configuration finding.
    pub clean: bool,
    /// One record per requested target.
    pub targets: Vec<AuditTarget>,
    /// How many findings of failing severity the run reports.
    pub failures: usize,
    /// The configuration findings the run raised.
    pub findings: Vec<ReportedFinding>,
}

/// One appended row, with the ceiling before it, the observation, and the ceiling after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppendChange {
    /// The row as the request proposed it.
    pub row: Row,
    /// The ceiling the list declared before.
    pub before: u64,
    /// What the tree holds now, which is the only ceiling append accepts.
    pub observed: u64,
    /// The ceiling the list declares after.
    pub after: u64,
}

/// One refused row, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppendRefusal {
    /// The row as the request proposed it.
    pub row: Row,
    /// Which refusal it is.
    pub code: &'static str,
    /// What the refusal is, in words.
    pub message: String,
}

/// One target of an append.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppendTarget {
    /// The owner whose list the request names.
    pub owner: String,
    /// The policy whose list the request names.
    pub policy: String,
    /// The codec the list is written in.
    pub syntax: &'static str,
    /// The authority the request carried, preserved byte for byte.
    pub authority: Authority,
    /// The rows the request proposed.
    pub requested: Vec<Row>,
    /// The rows the batch applied, empty when any row was refused.
    pub appended: Vec<AppendChange>,
    /// The rows the batch refused.
    pub refused: Vec<AppendRefusal>,
    /// The list as it stands after the batch.
    pub new_state: Vec<Row>,
}

/// The append's response object, which is also its receipt.
#[derive(Debug, Clone, Serialize)]
pub struct AppendResponse {
    /// The mode that ran.
    pub operation: &'static str,
    /// The repository root the observation was taken over.
    pub root: String,
    /// The request source.
    pub input: String,
    /// The response file.
    pub output: String,
    /// The digest of the request bytes.
    pub request_sha256: String,
    /// The digest of the list file before the batch.
    pub before_lists_sha256: String,
    /// The digest of the list file after the batch, equal to the first when nothing applied.
    pub after_lists_sha256: String,
    /// Whether the whole batch applied.
    pub applied: bool,
    /// One record per requested target.
    pub targets: Vec<AppendTarget>,
    /// How many findings of failing severity the run reports.
    pub failures: usize,
    /// The configuration findings the run raised.
    pub findings: Vec<ReportedFinding>,
}

/// What a maintenance run came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    /// A precondition was refused: the command exits the failure class with an empty stdout.
    Refused {
        /// What was refused, in words.
        message: String,
    },
    /// The run completed and made its response durable.
    Completed {
        /// The exact bytes of the response file, which are also the stdout object.
        bytes: Vec<u8>,
        /// How many failures the response reports, which decides the exit class.
        failures: usize,
    },
}

/// The request's shape, held to its closed schema.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRequest {
    schema: u32,
    operation: Operation,
    targets: Vec<RawTarget>,
}

/// One raw target.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    owner: String,
    policy: String,
    syntax: String,
    rows: Vec<RawRow>,
    authority: Authority,
}

/// One raw row, which is a selector for an audit and a complete row for an append.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    path: Option<String>,
    fingerprint: Option<String>,
    maximum: Option<i64>,
}

/// The hexadecimal digest of some bytes, in the form a declaration writes.
fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);

    let mut text = String::from("sha256:");

    for byte in hasher.finalize() {
        let _ignored = write!(text, "{byte:02x}");
    }

    text
}

/// Render a list file in the canonical form a writer emits.
///
/// The tables are emitted in the activation order, because that order is the
/// human declaration's own and a writer that resorted it would rewrite a file
/// nobody asked it to rewrite. The rows inside each table are emitted in decoded
/// byte-path or fingerprint order, which is the order they compare in.
///
/// A pair's whole identity stands in its table's key, family included. A pair a
/// corpus declares once is keyed by its owner and its policy, exactly as it
/// always was; a pair naming one deployment of a program the corpus deploys
/// several times is keyed by its owner, its policy and its family, which is the
/// key the loader reads it back under. A writer that emitted only the first two
/// components would collapse every deployment of one program onto one key and
/// hand back a file no loader could read, so the family is not a decoration on
/// the header but the part of it that keeps the deployments apart.
#[must_use]
pub fn render_lists(pairs: &[Pair], lists: &BTreeMap<Pair, Rows>) -> String {
    let mut text = String::new();

    for pair in pairs {
        let Some(rows) = lists.get(pair) else {
            continue;
        };

        if !text.is_empty() {
            text.push('\n');
        }

        let _ignored = match &pair.family {
            Some(family) => writeln!(text, "[{}.\"{}\".\"{family}\"]", pair.owner, pair.policy),
            None => writeln!(text, "[{}.\"{}\"]", pair.owner, pair.policy),
        };

        match rows {
            Rows::Allowances(rows) if rows.is_empty() => text.push_str("allowances = []\n"),
            Rows::PathCounts(rows) if rows.is_empty() => text.push_str("path_counts = []\n"),
            Rows::Paths(rows) if rows.is_empty() => text.push_str("paths = []\n"),
            Rows::Allowances(rows) => {
                text.push_str("allowances = [\n");

                for Allowance {
                    fingerprint,
                    maximum,
                } in rows
                {
                    let _ignored = writeln!(
                        text,
                        "  {{ fingerprint = \"{fingerprint}\", maximum = {maximum} }},"
                    );
                }

                text.push_str("]\n");
            }
            Rows::PathCounts(rows) => {
                text.push_str("path_counts = [\n");

                for PathCount { path, maximum } in rows {
                    let _ignored = writeln!(
                        text,
                        "  {{ path = \"{}\", maximum = {maximum} }},",
                        path.display()
                    );
                }

                text.push_str("]\n");
            }
            Rows::Paths(rows) => {
                text.push_str("paths = [\n");

                for path in rows {
                    let _ignored = writeln!(text, "  \"{}\",", path.display());
                }

                text.push_str("]\n");
            }
        }
    }

    text
}

/// What the tree currently holds for one pair, beside its non-failing explanations.
struct Observation {
    /// Each observed identity and its positive count.
    rows: BTreeMap<Identity, u64>,
    /// Human reasons that paths in the share are not governed by one half.
    explanations: Vec<String>,
}

/// Observe one pair through its policy's own recognizer.
///
/// The observation comes from the policy's own recognizer. An unrouted policy
/// observes nothing, and that is not the same as observing zero: a declaration
/// against one carries identities this binary could not have emitted, which the
/// audit reports rather than silently calling stale.
fn observe(root: &Path, route: Option<ControlObservationPlan<'_>>) -> Observation {
    let mut observation = Observation {
        rows: BTreeMap::new(),
        explanations: Vec::new(),
    };
    let Some(route) = route else {
        return observation;
    };
    let Some(observer) = route.observer() else {
        return observation;
    };
    match observer {
        Observer::BurnFamily(family) => observe_burn(root, &route, family, &mut observation),
        Observer::SpdxHeaders => observe_spdx(root, &route, &mut observation),
        Observer::InterchangeEnvelopes => observe_interchange(root, &route, &mut observation),
        Observer::FilePathCitations => observe_file_path_citations(root, &route, &mut observation),
        Observer::LabelGraph
        | Observer::TestProfile
        | Observer::TodoProfile
        | Observer::LegacyProfile
        | Observer::ClaimProfile
        | Observer::ConstantProfile
        | Observer::TestIndexes
        | Observer::TestMatrices
        | Observer::ConstantPins
        | Observer::AssemblyPublications
        | Observer::OwnerReach
        | Observer::OwnerRoster
        | Observer::ReferenceMigration => {}
    }

    observation
}

fn observe_burn(
    root: &Path,
    route: &ControlObservationPlan<'_>,
    family: &str,
    observation: &mut Observation,
) {
    let planned = route
        .migrations()
        .and_then(|migrations| {
            migrations
                .runs()
                .iter()
                .find(|run| run.list().family() == family)
        })
        .map(crate::plan::BurnRun::list);
    let compatibility = planned
        .is_none()
        .then(|| index_burn_lists(route.snapshot()))
        .and_then(|lists| lists.into_iter().find(|list| list.family() == family));
    let Some(list) = planned.or(compatibility.as_ref()) else {
        return;
    };
    let (censused, _findings) = census(root, list, route.corpus());

    // A census declared over the corpus root is observed entire, because the
    // ratchet it earns is the repository's rather than any share's.
    let whole = list.spans_corpus();

    for row in censused.rows() {
        let Ok(path) = BytePath::decode(&row.path) else {
            continue;
        };
        let attributed = if route.migrations().is_some() {
            route.partition().owner(&path) == Some(route.owner())
        } else {
            attributed_to(&path, route.owner(), route.snapshot().partitions())
        };
        if whole || attributed {
            observation
                .rows
                .insert(Identity::Path { path }, row.count as u64);
        }
    }
}

fn observe_spdx(root: &Path, route: &ControlObservationPlan<'_>, observation: &mut Observation) {
    let attribution = route.partition().attribution_view();
    let selection = route.content().map_or_else(
        || {
            Cow::Owned(crate::spdx::selection_plan(
                route.snapshot().spdx(),
                &attribution,
            ))
        },
        |content| Cow::Borrowed(content.spdx()),
    );
    let governed: Vec<_> = selection
        .governed()
        .iter()
        .filter(|entry| entry.owner == route.owner())
        .cloned()
        .collect();

    for path in violating_paths(root, route.snapshot().spdx(), &governed) {
        observation
            .rows
            .insert(Identity::Path { path: path.clone() }, 1);
    }
    observation.explanations = selection
        .exclusions()
        .iter()
        .filter(|excluded| excluded.owner == route.owner())
        .map(|excluded| {
            format!(
                "spdx section: {}: excluded from the {} {} half by rule {}",
                excluded.path, excluded.owner, excluded.half, excluded.name
            )
        })
        .collect();
}

fn observe_interchange(
    root: &Path,
    route: &ControlObservationPlan<'_>,
    observation: &mut Observation,
) {
    let attribution = route.partition().attribution_view();
    let selection = route.content().map_or_else(
        || {
            Cow::Owned(crate::interchange::selection_plan(
                route.snapshot().interchange(),
                &attribution,
            ))
        },
        |content| Cow::Borrowed(content.interchange()),
    );
    let governed: Vec<_> = selection
        .governed()
        .iter()
        .filter(|entry| entry.owner == route.owner())
        .cloned()
        .collect();

    for path in crate::interchange::violating_paths(root, &governed) {
        observation
            .rows
            .insert(Identity::Path { path: path.clone() }, 1);
    }
    observation.explanations = selection
        .exclusions()
        .iter()
        .filter(|excluded| excluded.owner == route.owner())
        .map(|excluded| {
            format!(
                "interchange section: {}: excluded from the {} section by rule {}",
                excluded.path, excluded.owner, excluded.name
            )
        })
        .collect();
}

fn observe_file_path_citations(
    root: &Path,
    route: &ControlObservationPlan<'_>,
    observation: &mut Observation,
) {
    let attribution = route.partition().attribution_view();
    let selection = route.content().map_or_else(
        || {
            Cow::Owned(crate::reference::selection_plan(
                route.snapshot().references(),
                &attribution,
            ))
        },
        |content| Cow::Borrowed(content.references()),
    );
    let lexicon = Lexicon::from_tracked(route.corpus().base());
    let governed: Vec<_> = selection
        .governed()
        .iter()
        .filter(|entry| entry.owner == route.owner())
        .cloned()
        .collect();

    for (path, count) in
        crate::reference::counted(&crate::reference::census(root, &lexicon, &governed))
    {
        observation
            .rows
            .insert(Identity::Path { path: path.clone() }, count);
    }
    observation.explanations = selection
        .exclusions()
        .iter()
        .filter(|excluded| excluded.owner == route.owner())
        .map(|excluded| crate::reference::exclusion_line(&excluded.path, &excluded.name))
        .collect();
}

/// Whether the inclusion relation attributes a path to exactly this owner.
fn attributed_to(path: &BytePath, owner: &str, partitions: &[OwnerRow]) -> bool {
    let mut matched = partitions
        .iter()
        .filter(|row| row.pattern.admits_path(path));

    matches!((matched.next(), matched.next()), (Some(row), None) if row.owner == owner)
}

/// What a permissive read of the list file found.
struct Declared {
    /// The tables that could be read, keyed by their pair.
    lists: BTreeMap<Pair, Rows>,
    /// The defects each pair's table carries.
    anomalies: BTreeMap<Pair, Vec<Anomaly>>,
    /// Every declared row and table this read did not carry into `lists`.
    ///
    /// A permissive read exists so that a defect can be *reported*, and reporting
    /// a defect is the opposite of writing over it. What the reader drops is
    /// therefore remembered rather than merely counted: the whole-file rewrite an
    /// append performs renders `lists` and nothing else, so any row missing from
    /// it is a row the rewrite would delete without saying so. The names
    /// collected here are what a refusal says back to the caller.
    dropped: Vec<String>,
    /// The file's bytes as they stand, for the digest.
    bytes: Vec<u8>,
    /// What stands above the first table, carried across a rewrite unchanged.
    prefix: String,
}

/// The list file's text standing above its first table: its envelope, and any
/// comment written with it.
///
/// The ratchet writer maintains the row array under an already-declared key and
/// stays out of the two reserved keys entirely, so what stands above the first
/// table crosses a rewrite byte for byte instead of being re-minted from a
/// constant this writer would then own. The envelope is a human declaration under
/// a ruling, exactly as every other declaration in this surface is, and a writer
/// that reproduced it from a literal would be quietly deciding what a document
/// claims about its own schema.
///
/// A file with nothing above its first table carries nothing across, and its
/// rendering is what it always was.
fn held_prefix(text: &str) -> &str {
    let mut at = 0;

    for line in text.split_inclusive('\n') {
        if line.trim_start().starts_with('[') {
            return &text[..at];
        }

        at += line.len();
    }

    text
}

/// Read the list file permissively, turning every semantic defect into an anomaly.
///
/// Only a lexical failure refuses, because a declaration that does not parse has
/// nothing for an audit to report on. Everything the ordinary loader refuses —
/// a wrong codec, an orphan table, a pair with no table, a non-canonical
/// encoding or ordering, a duplicate identity, a path attributed elsewhere —
/// survives here as an anomaly, which is the recovery role that makes audit
/// worth running against a configuration no policy could run against.
///
/// Permissive is not a licence to read a row differently from the way the
/// ordinary loader reads it. The two readings answer different questions — one
/// reports a defect and the other refuses it — and they must agree on what a
/// defect *is*, or the permissive reading manufactures defects the strict one
/// does not have. Two agreements are load-bearing here. A key's program is
/// resolved through the deployment relation exactly as the loader resolves it,
/// so a key naming an instance document and one of its set entries is read as
/// the program that entry deploys rather than looked up as though its middle
/// component were a policy name. And owner containment is asked only of a policy
/// that divides the owner partition, because a census declared over the corpus
/// root reaches every share at once and its rows stand at the activating owner
/// wherever their files are (ADR-L-019, The layer owner graph).
trait ListContext {
    fn surface(&self) -> &SurfaceAst;
    fn policies(&self) -> &[Pair];
    fn partitions(&self) -> &[OwnerRow];
    fn program(&self, pair: &Pair) -> Option<&str>;
}

impl ListContext for Snapshot {
    fn surface(&self) -> &SurfaceAst {
        self.surface()
    }

    fn policies(&self) -> &[Pair] {
        self.policies()
    }

    fn partitions(&self) -> &[OwnerRow] {
        self.partitions()
    }

    fn program(&self, pair: &Pair) -> Option<&str> {
        self.program(pair)
    }
}

impl ListContext for DeclarationCore {
    fn surface(&self) -> &SurfaceAst {
        self.surface()
    }

    fn policies(&self) -> &[Pair] {
        self.policies()
    }

    fn partitions(&self) -> &[OwnerRow] {
        self.partitions()
    }

    fn program(&self, pair: &Pair) -> Option<&str> {
        self.program(pair)
    }
}

fn read_list_syntax(
    root: &Path,
    context: &impl ListContext,
) -> Result<(Vec<u8>, crate::surface::ListDocument, String), String> {
    let path = root.join(DIRECTORY).join(LISTS_FILE);
    let document = context
        .surface()
        .document(LISTS_FILE)
        .ok_or_else(|| format!("{}: No such file or directory", path.display()))?;
    let bytes = document
        .bytes()
        .map_err(|error| format!("{}: {error}", path.display()))?
        .to_vec();
    let text = document
        .text()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let table = document
        .table()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let raw: crate::surface::ListDocument =
        table
            .clone()
            .try_into()
            .map_err(|mut error: toml::de::Error| {
                error.set_input(Some(text));
                format!("{}: {error}", path.display())
            })?;
    let prefix = String::from(held_prefix(text));

    Ok((bytes, raw, prefix))
}

#[allow(clippy::too_many_lines)]
fn read_lists(root: &Path, context: &impl ListContext) -> Result<Declared, String> {
    let activated = context.policies();
    let partitions = context.partitions();
    let (bytes, raw, prefix) = read_list_syntax(root, context)?;

    let declared: BTreeSet<&Pair> = activated.iter().collect();
    let mut lists = BTreeMap::new();
    let mut anomalies: BTreeMap<Pair, Vec<Anomaly>> = BTreeMap::new();
    let mut dropped: Vec<String> = Vec::new();

    for (owner, per_policy) in &raw.tables {
        for (policy, entry) in per_policy {
            let per_family: Vec<(Option<String>, &crate::surface::ListTable)> = match entry {
                crate::surface::ListEntry::Singleton(list) => vec![(None, list)],
                crate::surface::ListEntry::Instanced(families) => families
                    .iter()
                    .map(|(family, list)| (Some(family.clone()), list))
                    .collect(),
            };

            for (family, list) in per_family {
                let pair = Pair {
                    owner: owner.clone(),
                    policy: policy.clone(),
                    family,
                };

                let found = anomalies.entry(pair.clone()).or_default();

                if !declared.contains(&pair) {
                    found.push(Anomaly {
                        code: "pair_mismatch",
                        message: format!("{pair}: a list stands at a pair nothing activates"),
                        identity: None,
                    });
                }

                let Some(program) = context.program(&pair).map(str::to_owned) else {
                    found.push(Anomaly {
                        code: "pair_mismatch",
                        message: format!("{pair}: this binary catalogues no such policy"),
                        identity: None,
                    });

                    dropped.push(pair.to_string());

                    continue;
                };

                let Some(catalogued) = catalogued(&program) else {
                    found.push(Anomaly {
                        code: "pair_mismatch",
                        message: format!("{pair}: this binary catalogues no such policy"),
                        identity: None,
                    });

                    dropped.push(pair.to_string());

                    continue;
                };

                match (
                    catalogued.codec,
                    &list.allowances,
                    &list.path_counts,
                    &list.paths,
                ) {
                    (Codec::Fingerprint, Some(rows), None, None) => {
                        lists.insert(
                            pair.clone(),
                            Rows::Allowances(allowances(&pair, rows, found, &mut dropped)),
                        );
                    }
                    (Codec::PathCount, None, Some(rows), None) => {
                        let read =
                            path_counts(&pair, &program, rows, partitions, found, &mut dropped);

                        lists.insert(pair.clone(), Rows::PathCounts(read));
                    }
                    (Codec::PathSet, None, None, Some(rows)) => {
                        let read = paths(&pair, &program, rows, partitions, found, &mut dropped);

                        lists.insert(pair.clone(), Rows::Paths(read));
                    }
                    _ => {
                        found.push(Anomaly {
                            code: "wrong_codec",
                            message: format!(
                                "{pair}: the policy selects `{}`",
                                catalogued.codec.field()
                            ),
                            identity: None,
                        });

                        dropped.push(pair.to_string());
                    }
                }
            }
        }
    }

    for pair in activated {
        if !lists.contains_key(pair) {
            anomalies.entry(pair.clone()).or_default().push(Anomaly {
                code: "pair_mismatch",
                message: format!("{pair}: the activated pair carries no list"),
                identity: None,
            });
        }
    }

    anomalies.retain(|_pair, found| !found.is_empty());

    Ok(Declared {
        lists,
        anomalies,
        dropped,
        bytes,
        prefix,
    })
}

/// Read one fingerprint table, reporting its defects rather than refusing them.
fn allowances(
    pair: &Pair,
    rows: &[crate::surface::AllowanceRow],
    anomalies: &mut Vec<Anomaly>,
    dropped: &mut Vec<String>,
) -> Vec<Allowance> {
    let mut parsed: Vec<Allowance> = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        let identity = Identity::Fingerprint {
            fingerprint: row.fingerprint.clone(),
        };

        if !is_fingerprint(&row.fingerprint) {
            anomalies.push(Anomaly {
                code: "noncanonical_encoding",
                message: format!("{pair}: {} is not a canonical fingerprint", row.fingerprint),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", row.fingerprint));

            continue;
        }

        if !seen.insert(row.fingerprint.clone()) {
            anomalies.push(Anomaly {
                code: "duplicate_identity",
                message: format!("{pair}: {} stands twice", row.fingerprint),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", row.fingerprint));

            continue;
        }

        // The linter is the only producer of fingerprints, and no policy emits
        // one until its verdicts are routed through the declared surface, so a
        // fingerprint standing today is one this binary could not have written.
        anomalies.push(Anomaly {
            code: "unrecognized_fingerprint",
            message: format!("{pair}: this binary emits no fingerprint for that policy yet"),
            identity: Some(identity),
        });

        let Ok(maximum) = u64::try_from(row.maximum) else {
            anomalies.push(Anomaly {
                code: "noncanonical_encoding",
                message: format!("{pair}: a ceiling is a positive integer"),
                identity: None,
            });

            dropped.push(format!("{pair}: {}", row.fingerprint));

            continue;
        };

        parsed.push(Allowance {
            fingerprint: row.fingerprint.clone(),
            maximum,
        });
    }

    if parsed.windows(2).any(|pair| pair[0] > pair[1]) {
        anomalies.push(Anomaly {
            code: "noncanonical_order",
            message: format!("{pair}: the rows are not in fingerprint order"),
            identity: None,
        });
    }

    parsed.sort();
    parsed
}

/// Read one path-count table, reporting its defects rather than refusing them.
///
/// Owner containment is asked exactly when the ordinary loader asks it, which is
/// exactly when the policy divides the owner partition. A census declared over
/// the corpus root does not divide it, and its rows are held at the activating
/// owner wherever their files stand.
fn path_counts(
    pair: &Pair,
    program: &str,
    rows: &[crate::surface::PathCountRow],
    partitions: &[OwnerRow],
    anomalies: &mut Vec<Anomaly>,
    dropped: &mut Vec<String>,
) -> Vec<PathCount> {
    let mut parsed: Vec<PathCount> = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for row in rows {
        let path = match BytePath::decode(&row.path) {
            Ok(path) => path,
            Err(defect) => {
                anomalies.push(Anomaly {
                    code: "noncanonical_encoding",
                    message: format!("{pair}: {}: {defect}", row.path),
                    identity: None,
                });

                dropped.push(format!("{pair}: {}", row.path));

                continue;
            }
        };

        let identity = Identity::Path { path: path.clone() };

        let Ok(maximum) = u64::try_from(row.maximum) else {
            anomalies.push(Anomaly {
                code: "noncanonical_encoding",
                message: format!("{pair}: a ceiling is a positive integer"),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", path.display()));

            continue;
        };

        if !seen.insert(path.clone()) {
            anomalies.push(Anomaly {
                code: "duplicate_identity",
                message: format!("{pair}: {} stands twice", path.display()),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", path.display()));

            continue;
        }

        if !spans_corpus(program) && !attributed_to(&path, &pair.owner, partitions) {
            anomalies.push(Anomaly {
                code: "owner_path_mismatch",
                message: format!(
                    "{pair}: the inclusion relation attributes {} elsewhere",
                    path.display()
                ),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", path.display()));

            continue;
        }

        parsed.push(PathCount { path, maximum });
    }

    if parsed.windows(2).any(|window| window[0] > window[1]) {
        anomalies.push(Anomaly {
            code: "noncanonical_order",
            message: format!("{pair}: the rows are not in decoded byte-path order"),
            identity: None,
        });
    }

    parsed.sort();
    parsed
}

/// Read one path-set table, reporting its defects rather than refusing them.
///
/// Owner containment is asked under the same condition a path-count table's is,
/// because the question belongs to the policy and not to the codec.
fn paths(
    pair: &Pair,
    program: &str,
    rows: &[String],
    partitions: &[OwnerRow],
    anomalies: &mut Vec<Anomaly>,
    dropped: &mut Vec<String>,
) -> Vec<BytePath> {
    let mut parsed = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    for encoded in rows {
        let path = match BytePath::decode(encoded) {
            Ok(path) => path,
            Err(defect) => {
                anomalies.push(Anomaly {
                    code: "noncanonical_encoding",
                    message: format!("{pair}: {encoded}: {defect}"),
                    identity: None,
                });

                dropped.push(format!("{pair}: {encoded}"));

                continue;
            }
        };

        let identity = Identity::Path { path: path.clone() };

        if !seen.insert(path.clone()) {
            anomalies.push(Anomaly {
                code: "duplicate_identity",
                message: format!("{pair}: {} stands twice", path.display()),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", path.display()));

            continue;
        }

        if !spans_corpus(program) && !attributed_to(&path, &pair.owner, partitions) {
            anomalies.push(Anomaly {
                code: "owner_path_mismatch",
                message: format!(
                    "{pair}: the inclusion relation attributes {} elsewhere",
                    path.display()
                ),
                identity: Some(identity),
            });

            dropped.push(format!("{pair}: {}", path.display()));

            continue;
        }

        parsed.push(path);
    }

    if parsed.windows(2).any(|window| window[0] > window[1]) {
        anomalies.push(Anomaly {
            code: "noncanonical_order",
            message: format!("{pair}: the rows are not in decoded byte-path order"),
            identity: None,
        });
    }

    parsed.sort();
    parsed
}

/// Whether text is this producer's canonical fingerprint form.
fn is_fingerprint(text: &str) -> bool {
    text.strip_prefix("sha256:").is_some_and(|digits| {
        digits.len() == 64
            && digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// The rows of a list, as a response carries them.
fn rows_of(rows: &Rows) -> Vec<Row> {
    match rows {
        Rows::Allowances(rows) => rows
            .iter()
            .map(|row| Row {
                identity: Identity::Fingerprint {
                    fingerprint: row.fingerprint.clone(),
                },
                maximum: Some(row.maximum),
            })
            .collect(),
        Rows::PathCounts(rows) => rows
            .iter()
            .map(|row| Row {
                identity: Identity::Path {
                    path: row.path.clone(),
                },
                maximum: Some(row.maximum),
            })
            .collect(),
        // The path-set codec carries no ceiling, because the identity holds at
        // most one violation. The universal comparison supplies its sole
        // positive value when it reads the row.
        Rows::Paths(rows) => rows
            .iter()
            .map(|path| Row {
                identity: Identity::Path { path: path.clone() },
                maximum: None,
            })
            .collect(),
    }
}

/// The ceiling a list declares for each identity.
fn registered_of(rows: Option<&Rows>) -> BTreeMap<Identity, u64> {
    rows.map(rows_of)
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let maximum = row.ceiling();
            (row.identity, maximum)
        })
        .collect()
}

/// One target of a request, after its meaning has been validated.
struct Target {
    pair: Pair,
    codec: Codec,
    authority: Authority,
    selectors: Vec<Identity>,
    rows: Vec<Row>,
}

/// Read and validate a request against its schema, this binary's catalog and the declaration.
///
/// The JSON schema validates shape; the catalog and repository predicates
/// validate meaning, and both classes of defect refuse the run — a request the
/// command cannot make sense of is one it has no standing to answer.
fn read_request(
    bytes: &[u8],
    operation: Operation,
    activated: &[Pair],
) -> Result<(Vec<Target>, Vec<u8>), String> {
    let raw: RawRequest =
        serde_json::from_slice(bytes).map_err(|error| format!("standard input: {error}"))?;

    if raw.schema != REQUEST_SCHEMA {
        return Err(format!(
            "the request declares schema {} and this binary reads {REQUEST_SCHEMA}",
            raw.schema
        ));
    }

    if raw.operation != operation {
        return Err(format!(
            "the request declares the {} operation and the command is running {}",
            raw.operation.as_str(),
            operation.as_str()
        ));
    }

    if raw.targets.is_empty() {
        return Err(String::from("the request names no target"));
    }

    let declared: BTreeSet<&Pair> = activated.iter().collect();
    let mut targets = Vec::with_capacity(raw.targets.len());
    let mut seen = BTreeSet::new();

    for target in &raw.targets {
        let pair = Pair::singleton(target.owner.clone(), target.policy.clone());

        if !seen.insert(pair.clone()) {
            return Err(format!("{pair}: the request names this pair twice"));
        }

        if !target.authority.is_complete() {
            return Err(format!("{pair}: the authority is not complete"));
        }

        let Some(catalogued) = catalogued(&target.policy) else {
            return Err(format!("{pair}: this binary catalogues no such policy"));
        };

        if !declared.contains(&pair) {
            return Err(format!("{pair}: the declaration activates no such pair"));
        }

        if target.syntax != catalogued.codec.as_str() {
            return Err(format!(
                "{pair}: the request declares `{}` and the policy selects `{}`",
                target.syntax,
                catalogued.codec.as_str()
            ));
        }

        let (selectors, rows) = read_rows(&pair, operation, catalogued.codec, &target.rows)?;

        targets.push(Target {
            pair,
            codec: catalogued.codec,
            authority: target.authority.clone(),
            selectors,
            rows,
        });
    }

    Ok((targets, bytes.to_vec()))
}

/// Read one target's rows as selectors or as complete rows, per the operation.
fn read_rows(
    pair: &Pair,
    operation: Operation,
    codec: Codec,
    rows: &[RawRow],
) -> Result<(Vec<Identity>, Vec<Row>), String> {
    let mut selectors = Vec::with_capacity(rows.len());
    let mut complete = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();

    if operation == Operation::Append && rows.is_empty() {
        return Err(format!(
            "{pair}: an append names no row, and a growth door cannot be opened onto nothing"
        ));
    }

    for row in rows {
        let identity = match (codec, &row.path, &row.fingerprint) {
            (Codec::PathCount | Codec::PathSet, Some(path), None) => Identity::Path {
                path: BytePath::decode(path)
                    .map_err(|defect| format!("{pair}: {path}: {defect}"))?,
            },
            (Codec::Fingerprint, None, Some(fingerprint)) => {
                if !is_fingerprint(fingerprint) {
                    return Err(format!(
                        "{pair}: {fingerprint}: not a canonical fingerprint"
                    ));
                }

                Identity::Fingerprint {
                    fingerprint: fingerprint.clone(),
                }
            }
            _ => {
                return Err(format!(
                    "{pair}: a row names no identity in this policy's codec"
                ));
            }
        };

        if !seen.insert(identity.clone()) {
            return Err(format!("{pair}: the request names one identity twice"));
        }

        match (operation, codec, row.maximum) {
            (Operation::Audit, _, None) => selectors.push(identity),
            (Operation::Audit, _, Some(_)) => {
                return Err(format!("{pair}: an audit selector carries no ceiling"));
            }
            (Operation::Append, Codec::PathSet, None) => complete.push(Row {
                identity,
                maximum: None,
            }),
            (Operation::Append, Codec::PathSet, Some(_)) => {
                return Err(format!("{pair}: a path-set append row carries no ceiling"));
            }
            (Operation::Append, _, Some(maximum)) => {
                let maximum = u64::try_from(maximum)
                    .map_err(|_error| format!("{pair}: a ceiling is a positive integer"))?;

                if maximum == 0 {
                    return Err(format!("{pair}: a ceiling is a positive integer"));
                }

                complete.push(Row {
                    identity,
                    maximum: Some(maximum),
                });
            }
            (Operation::Append, _, None) => {
                return Err(format!(
                    "{pair}: an append row carries its complete ceiling"
                ));
            }
        }
    }

    Ok((selectors, complete))
}

/// Classify one identity against what the list declares and the tree holds.
fn classify(target: &mut AuditTarget, identity: Identity, registered: u64, observed: u64) {
    let comparison = Comparison {
        identity,
        registered,
        observed,
    };

    if observed > registered {
        target.growth.push(comparison);
    } else if observed == 0 {
        target.stale.push(comparison);
    } else if registered > observed {
        target.shrink.push(comparison);
    } else {
        target.equal.push(comparison);
    }
}

/// Run the read-only comparison over every requested target.
fn audit(
    plan: ControlWritePlan<'_>,
    declared: &Declared,
    targets: Vec<Target>,
) -> Result<Vec<AuditTarget>, String> {
    let mut audited = Vec::with_capacity(targets.len());

    for target in targets {
        let rows = declared.lists.get(&target.pair);
        let registered = registered_of(rows);
        let observation = observe(plan.root(), plan.observation(&target.pair));
        let observed = &observation.rows;

        let selected: Vec<Identity> = if target.selectors.is_empty() {
            registered
                .keys()
                .chain(observed.keys())
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        } else {
            for identity in &target.selectors {
                if !registered.contains_key(identity) && !observed.contains_key(identity) {
                    return Err(format!(
                        "{}: the request selects an identity the list does not declare and the tree does not hold",
                        target.pair
                    ));
                }
            }

            target.selectors.clone()
        };

        let anomalies = declared
            .anomalies
            .get(&target.pair)
            .cloned()
            .unwrap_or_default();

        let mut record = AuditTarget {
            owner: target.pair.owner.clone(),
            policy: target.pair.policy.clone(),
            syntax: target.codec.as_str(),
            authority: target.authority,
            requested: selected.clone(),
            state: rows.map(rows_of).unwrap_or_default(),
            equal: Vec::new(),
            growth: Vec::new(),
            shrink: Vec::new(),
            stale: Vec::new(),
            explanations: observation.explanations,
            anomalies,
        };

        for identity in selected {
            let held = observed.get(&identity).copied().unwrap_or_default();
            let ceiling = registered.get(&identity).copied().unwrap_or_default();

            classify(&mut record, identity, ceiling, held);
        }

        audited.push(record);
    }

    Ok(audited)
}

/// The refusals a target earns because a write would truncate the declaration.
///
/// An append rewrites the whole file from what the permissive read carried, so a
/// row that read did not carry is a row the rewrite would delete. The question is
/// therefore asked of the declaration entire and not of the target's own table:
/// a defect anywhere in the file puts every row in the file at risk, and the pair
/// being appended to is no safer for being clean itself. What the caller is owed
/// is the list of rows a write would have dropped, so the refusal names them
/// (ADR-L-020, The migration disciplines).
fn truncation_refusals(declared: &Declared, target: &Target) -> Vec<AppendRefusal> {
    if declared.dropped.is_empty() {
        return Vec::new();
    }

    target
        .rows
        .iter()
        .map(|row| AppendRefusal {
            row: row.clone(),
            code: "lossy_declaration",
            message: format!(
                "{}: the declaration carries {} row(s) this reading could not carry, and a rewrite would delete them: {}",
                target.pair,
                declared.dropped.len(),
                declared.dropped.join(", ")
            ),
        })
        .collect()
}

/// Apply an append batch, atomically or not at all.
///
/// A row is accepted only when it names growth the tree currently holds and its
/// ceiling equals exactly that current observation. Raising an existing ceiling
/// and inserting an absent identity are both appends; a row already equal, lower
/// than the observation, greater than it, or absent from the tree is refused,
/// and one refusal refuses the batch.
fn append(
    plan: ControlWritePlan<'_>,
    declared: &Declared,
    targets: &[Target],
) -> (Vec<AppendTarget>, BTreeMap<Pair, Rows>, bool) {
    let mut appended = Vec::with_capacity(targets.len());
    let mut staged = declared.lists.clone();
    let mut applied = true;

    for target in targets {
        let registered = registered_of(declared.lists.get(&target.pair));
        let observed = observe(plan.root(), plan.observation(&target.pair)).rows;

        let mut changes = Vec::new();
        let mut refused = Vec::new();

        for row in &target.rows {
            let held = observed.get(&row.identity).copied().unwrap_or_default();
            let before = registered.get(&row.identity).copied().unwrap_or_default();
            let proposed = row.ceiling();

            if held == 0 {
                refused.push(AppendRefusal {
                    row: row.clone(),
                    code: "not_observed",
                    message: format!("{}: the tree holds no such occurrence", target.pair),
                });

                continue;
            }

            if held <= before {
                refused.push(AppendRefusal {
                    row: row.clone(),
                    code: "not_growth",
                    message: format!(
                        "{}: the list already declares {before} and the tree holds {held}",
                        target.pair
                    ),
                });

                continue;
            }

            if proposed != held {
                refused.push(AppendRefusal {
                    row: row.clone(),
                    code: "maximum_not_observed",
                    message: format!(
                        "{}: the tree holds {held} and the row proposes {proposed}",
                        target.pair
                    ),
                });

                continue;
            }

            changes.push(AppendChange {
                row: row.clone(),
                before,
                observed: held,
                after: proposed,
            });
        }

        if declared.anomalies.contains_key(&target.pair) {
            refused.extend(target.rows.iter().map(|row| AppendRefusal {
                row: row.clone(),
                code: "list_anomaly",
                message: format!(
                    "{}: the declaration carries a defect an append may not write over",
                    target.pair
                ),
            }));
        }

        refused.extend(truncation_refusals(declared, target));

        if !refused.is_empty() {
            applied = false;
        }

        appended.push(AppendTarget {
            owner: target.pair.owner.clone(),
            policy: target.pair.policy.clone(),
            syntax: target.codec.as_str(),
            authority: target.authority.clone(),
            requested: target.rows.clone(),
            appended: changes,
            refused,
            new_state: Vec::new(),
        });
    }

    if applied {
        for (target, record) in targets.iter().zip(appended.iter_mut()) {
            let rows = staged
                .entry(target.pair.clone())
                .or_insert_with(|| match target.codec {
                    Codec::Fingerprint => Rows::Allowances(Vec::new()),
                    Codec::PathCount => Rows::PathCounts(Vec::new()),
                    Codec::PathSet => Rows::Paths(Vec::new()),
                });

            for change in &record.appended {
                raise(rows, &change.row);
            }

            record.new_state = rows_of(rows);
        }
    } else {
        for (target, record) in targets.iter().zip(appended.iter_mut()) {
            record.appended.clear();
            record.new_state = declared
                .lists
                .get(&target.pair)
                .map(rows_of)
                .unwrap_or_default();
        }
    }

    (appended, staged, applied)
}

/// Raise or insert one row in a staged list.
fn raise(rows: &mut Rows, row: &Row) {
    match (rows, &row.identity) {
        (Rows::PathCounts(rows), Identity::Path { path }) => {
            let Some(maximum) = row.maximum else {
                return;
            };

            match rows.iter_mut().find(|held| held.path == *path) {
                Some(held) => held.maximum = maximum,
                None => rows.push(PathCount {
                    path: path.clone(),
                    maximum,
                }),
            }

            rows.sort();
        }
        (Rows::Paths(rows), Identity::Path { path }) => {
            // The codec has no ceiling to raise, so a row already standing is
            // already what an append would make it.
            if !rows.contains(path) {
                rows.push(path.clone());
            }

            rows.sort();
        }
        (Rows::Allowances(rows), Identity::Fingerprint { fingerprint }) => {
            let Some(maximum) = row.maximum else {
                return;
            };

            match rows
                .iter_mut()
                .find(|held| held.fingerprint == *fingerprint)
            {
                Some(held) => held.maximum = maximum,
                None => rows.push(Allowance {
                    fingerprint: fingerprint.clone(),
                    maximum,
                }),
            }

            rows.sort();
        }
        // The codec was validated against the catalog before the batch was read,
        // so a row in the other codec cannot reach here.
        _ => {}
    }
}

/// Lower every declared list to what the census now finds, and never raise one.
///
/// A ceiling above the observation comes down to it, a row whose occurrences are
/// gone is removed, and a ceiling below the observation is left exactly where it
/// stands — that is growth, and growth passes the other door or none.
#[must_use]
pub fn lower(
    root: &Path,
    snapshot: &Snapshot,
    corpus: &CorpusPlan,
    partition: &PartitionPlan,
) -> BTreeMap<Pair, Rows> {
    let mut lowered = snapshot.lists().clone();

    for (pair, rows) in &mut lowered {
        let route = ControlObservationPlan::compatibility(snapshot, pair, corpus, partition);
        let observed = observe(root, Some(route)).rows;

        lower_rows(rows, &observed);
    }

    lowered
}

fn lower_planned(plan: ControlWritePlan<'_>) -> BTreeMap<Pair, Rows> {
    let snapshot = plan.snapshot();
    let mut lowered = snapshot.lists().clone();

    for (pair, rows) in &mut lowered {
        let observed = observe(plan.root(), plan.observation(pair)).rows;

        lower_rows(rows, &observed);
    }

    lowered
}

fn lower_rows(rows: &mut Rows, observed: &BTreeMap<Identity, u64>) {
    match rows {
        Rows::PathCounts(rows) => {
            rows.retain_mut(|row| {
                let identity = Identity::Path {
                    path: row.path.clone(),
                };
                let held = observed.get(&identity).copied().unwrap_or_default();

                // A ceiling above the observation comes down to it; one
                // below it is growth and stays exactly where it stands.
                row.maximum = row.maximum.min(held);

                held > 0
            });
        }
        Rows::Paths(rows) => rows.retain(|path| {
            let identity = Identity::Path { path: path.clone() };
            observed.contains_key(&identity)
        }),
        // No policy emits a fingerprint, so there is nothing to lower an
        // allowance to. Treating that unrouted state as an empty observation
        // would discard a debt rather than record one the corpus earned.
        Rows::Allowances(_) => {}
    }
}

/// Whether an output path is one a maintenance run may not overwrite.
///
/// A maintenance run may not overwrite the thing it is reporting on, so the
/// response may name neither a declared configuration file nor a generated
/// register (´spec:commandcontract:burn-modes´).
fn is_protected(root: &Path, output: &Path) -> bool {
    let output = normalize_lexically(output);

    // The declared surface is a directory rather than a filename list, so what is
    // protected is lexical membership of it. A parameter document that arrived
    // without this function being told about it is exactly the file a maintenance
    // run must not land on (´dec:snapshot:physical-membership´). Resolving the two
    // paths lexically keeps an arbitrary response path from becoming a metadata
    // probe while still collapsing the relative components that could disguise a
    // declaration as an unrelated destination.
    let directory = normalize_lexically(&root.join(DIRECTORY));
    output == directory || output.parent().is_some_and(|parent| parent == directory)
}

/// Resolve current- and parent-directory components without consulting the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|tail| matches!(tail, Component::Normal(_))) =>
            {
                normalized.pop();
            }
            Component::ParentDir if normalized.has_root() => {}
            Component::ParentDir
            | Component::Normal(_)
            | Component::RootDir
            | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    normalized
}

/// Write bytes to a path by writing beside it and renaming, so no reader sees a half file.
fn replace_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let staging = path.with_extension("linter-staging");

    std::fs::write(&staging, bytes)?;
    std::fs::rename(&staging, path)
}

/// Run one maintenance mode, making its response durable before returning it.
///
/// The response file and the stdout object are the same bytes: the file is made
/// durable first and the caller copies it to stdout, so the receipt that travels
/// with a change and the object the caller in the loop reads cannot tell
/// different stories.
///
/// An append makes the list durable before the response, so a crash between the
/// two loses the external receipt while the batch stands committed entire. It
/// cannot partially apply the batch, and the digests the response carries expose
/// the window to the next audit.
#[must_use]
pub fn maintain(root: &Path, operation: Operation, request: &[u8], output: &Path) -> Control {
    if let Some(refusal) = maintenance_path_refusal(root, output) {
        return refusal;
    }

    let core = match core_configuration(root) {
        CoreConfiguration::Absent => {
            return Control::Refused {
                message: String::from("no declared configuration stands at this root"),
            };
        }
        CoreConfiguration::Refused(refusals) => {
            return Control::Refused {
                message: refusals
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join("; "),
            };
        }
        CoreConfiguration::Present(core) => core,
    };

    let declared = match read_lists(root, core.as_ref()) {
        Ok(declared) => declared,
        Err(message) => return Control::Refused { message },
    };

    let context = Configuration::Present(Box::new((*core).with_lists(declared.lists.clone())));
    let plan = match ExecutionPlan::compile(root, context) {
        Ok(plan) => plan,
        Err(error) => {
            return Control::Refused {
                message: format!("the execution topology could not be compiled: {error:?}"),
            };
        }
    };

    maintain_loaded(
        plan.control_write(),
        operation,
        request,
        output,
        Some(declared),
    )
}

/// Run one maintenance mode against a configuration the caller already loaded.
///
/// The command-line entrypoint performs the strict configuration preflight for
/// every command. Passing that snapshot here lets maintenance project its
/// permissive view from the same syntax tree instead of opening and parsing the
/// declaration surface again.
#[must_use]
pub fn maintain_with_configuration(
    root: &Path,
    operation: Operation,
    request: &[u8],
    output: &Path,
    context: &Configuration,
) -> Control {
    if let Some(refusal) = maintenance_path_refusal(root, output) {
        return refusal;
    }
    if let Configuration::Refused(refusals) = context {
        return Control::Refused {
            message: refusals
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("; "),
        };
    }

    let plan = match ExecutionPlan::compile(root, context.clone()) {
        Ok(plan) => plan,
        Err(error) => {
            return Control::Refused {
                message: format!("the execution topology could not be compiled: {error:?}"),
            };
        }
    };

    maintain_loaded(plan.control_write(), operation, request, output, None)
}

/// Run one maintenance mode through the control subplan of an execution plan.
///
/// The caller has already loaded configuration and compiled topology, so audit
/// and append neither reopen declarations nor reconstruct observer dispatch.
#[must_use]
pub fn maintain_with_plan(
    plan: ControlWritePlan<'_>,
    operation: Operation,
    request: &[u8],
    output: &Path,
) -> Control {
    if let Some(refusal) = maintenance_path_refusal(plan.root(), output) {
        return refusal;
    }

    maintain_loaded(plan, operation, request, output, None)
}

fn maintenance_path_refusal(root: &Path, output: &Path) -> Option<Control> {
    let refused = |message: String| Control::Refused { message };

    if is_protected(root, output) {
        return Some(refused(String::from(
            "the response may name neither a declared configuration file nor a generated register",
        )));
    }

    if !is_sanctioned_response(root, output) {
        return Some(refused(String::from(
            "the response must be a direct child of the repository root",
        )));
    }

    None
}

/// Whether a response stays in the output location sanctioned by the control contract.
fn is_sanctioned_response(root: &Path, output: &Path) -> bool {
    let root = normalize_lexically(root);
    let output = normalize_lexically(output);

    output.file_name().is_some() && output.parent().is_some_and(|parent| parent == root)
}

#[allow(
    clippy::too_many_lines,
    reason = "one ordered read-verify-serialize-write transaction preserves mutation and receipt ordering"
)]
fn maintain_loaded(
    plan: ControlWritePlan<'_>,
    operation: Operation,
    request: &[u8],
    output: &Path,
    declared: Option<Declared>,
) -> Control {
    let refused = |message: String| Control::Refused { message };
    let root = plan.root();

    let snapshot = plan.snapshot();

    let declared = match declared {
        Some(declared) => declared,
        None => match read_lists(root, snapshot) {
            Ok(declared) => declared,
            Err(message) => return refused(message),
        },
    };

    let (targets, request) = match read_request(request, operation, snapshot.policies()) {
        Ok(read) => read,
        Err(message) => return refused(message),
    };

    // The partition, dependency closure and parameter sections are asked of the
    // declarations the list file is checked against, never of the list file, so
    // an audit can answer them over exactly the configuration it reads
    // permissively. An audit reports them; an append refuses, because every
    // writing mode refuses to mutate under a snapshot that disagrees with the
    // tree (´rule:commandcontract:configuration-verdicts´). Header failures are
    // the observation these modes compare and record, not configuration
    // disagreement, so they reach the target comparison instead of also making
    // the audit incoherent or blocking the append growth door.
    //
    // An envelope defect is the same kind of thing at the second path-set
    // policy, and is exempted on the same ground: it is what a row of that
    // policy's list tolerates, so a door that refused to open under it could
    // never record the debt it exists to record. The section findings beside it
    // are not exempted and go on blocking, because a share that is not divided
    // is a configuration that disagrees with the tree rather than an observation
    // of one.
    //
    // A file-path citation is the third of the same kind and is exempted on the
    // same ground, which bites hardest here: the permissive reading carries no
    // strict list at all, so every citation the corpus holds reads as untolerated
    // here however exactly the tables declare it. Left in, it would report a
    // whole census as configuration disagreement and would refuse the append door
    // under precisely the debt that door exists to record. The section findings
    // beside it go on blocking for the reason they do at the other two policies.
    let (_counts, mut coherence) = verify_write_guard(plan.guard());

    coherence.retain(|finding| {
        !matches!(
            finding,
            Finding::SpdxMissingHeader { .. }
                | Finding::SpdxWrongIdentifier { .. }
                | Finding::SpdxRepeatedIdentifier { .. }
                | Finding::InterchangeEnvelope { .. }
                | Finding::FilePathCitation { .. }
        )
    });

    if operation == Operation::Append && !coherence.is_empty() {
        return refused(
            coherence
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("; "),
        );
    }

    let findings: Vec<ReportedFinding> = coherence.into_iter().map(ReportedFinding::from).collect();
    let incoherent = findings.len();

    let request_sha256 = digest(&request);
    let before = digest(&declared.bytes);

    let response = match operation {
        Operation::Audit => {
            let audited = match audit(plan, &declared, targets) {
                Ok(audited) => audited,
                Err(message) => return refused(message),
            };

            let anomalies: usize = audited.iter().map(|target| target.anomalies.len()).sum();
            let failures = anomalies + incoherent;

            serde_json::to_vec(&AuditResponse {
                operation: operation.as_str(),
                root: root.display().to_string(),
                input: String::from("stdin"),
                output: output.display().to_string(),
                request_sha256,
                lists_sha256: before,
                clean: failures == 0,
                targets: audited,
                failures,
                findings,
            })
        }
        Operation::Append => {
            let (targets, staged, applied) = append(plan, &declared, &targets);
            let rendered = format!(
                "{}{}",
                declared.prefix,
                render_lists(&snapshot.ordered(&staged), &staged)
            );

            let after = if applied {
                let path = root.join(DIRECTORY).join(LISTS_FILE);

                if let Err(error) = replace_atomically(&path, rendered.as_bytes()) {
                    return refused(format!("{}: {error}", path.display()));
                }

                digest(rendered.as_bytes())
            } else {
                before.clone()
            };

            let failures: usize = targets.iter().map(|target| target.refused.len()).sum();

            serde_json::to_vec(&AppendResponse {
                operation: operation.as_str(),
                root: root.display().to_string(),
                input: String::from("stdin"),
                output: output.display().to_string(),
                request_sha256,
                before_lists_sha256: before,
                after_lists_sha256: after,
                applied,
                targets,
                failures,
                findings,
            })
        }
    };

    let mut bytes = match response {
        Ok(bytes) => bytes,
        Err(error) => return refused(error.to_string()),
    };

    bytes.push(b'\n');

    let failures = failures_of(&bytes);

    if let Err(error) = replace_atomically(output, &bytes) {
        return refused(format!("{}: {error}", output.display()));
    }

    Control::Completed { bytes, failures }
}

/// The failure count a rendered response reports.
fn failures_of(bytes: &[u8]) -> usize {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("failures").and_then(serde_json::Value::as_u64))
        .and_then(|failures| usize::try_from(failures).ok())
        .unwrap_or_default()
}

/// Lower the declared lists in place, answering whether the file moved.
///
/// This is the declared half of the lowering write. The Markdown registers keep
/// their own writer, because a view and the declaration it renders are two
/// artifacts and the campaign still reads both; what this adds is that a wave
/// which has earned a lower ceiling records it in the declaration as well.
///
/// A tree with no declaration has nothing to lower and is not a failure.
///
/// # Errors
///
/// Returns the filesystem's error when the list file cannot be replaced.
///
pub fn lower_lists(plan: &ExecutionPlan) -> std::io::Result<bool> {
    lower_lists_with_plan(plan.control_write())
}

/// Lower the declared list file through its typed control write subplan.
///
/// # Errors
///
/// Returns the filesystem's error when the list file cannot be replaced.
pub fn lower_lists_with_plan(plan: ControlWritePlan<'_>) -> std::io::Result<bool> {
    let snapshot = plan.snapshot();

    let root = plan.root();
    let path = root.join(DIRECTORY).join(LISTS_FILE);
    let held = std::fs::read_to_string(&path).unwrap_or_default();
    let rendered = format!(
        "{}{}",
        held_prefix(&held),
        render_lists(&snapshot.list_keys(), &lower_planned(plan),)
    );

    if held == rendered {
        return Ok(false);
    }

    replace_atomically(&path, rendered.as_bytes())?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Operation, Pair, PathCount, RawRow, Rows, read_rows, render_lists};
    use crate::catalogue::Codec;
    use crate::pattern::BytePath;

    /// The pair every fixture declares: the Assayer's section-reference debt.
    fn pair() -> Pair {
        Pair::singleton("ASSAYER", "legacy.section-references")
    }

    /// A pair the corpus declares once renders exactly as it always did: two key
    /// components and no third. The family component is what an instanced pair
    /// gains, not a spelling every pair now carries, so a declaration holding no
    /// instanced pair crosses a rewrite byte for byte.
    ///
    /// ´claim:control:a-family-less-pair-renders-as-it-always-did´
    /// ´test:unit:a-family-less-pair-renders-as-it-always-did´
    #[test]
    fn a_family_less_pair_renders_as_it_always_did() {
        let singleton = pair();

        assert_eq!(singleton.family, None, "the fixture pair carries no family");

        let mut lists = BTreeMap::new();
        lists.insert(
            singleton.clone(),
            Rows::PathCounts(vec![PathCount {
                path: BytePath::decode("packages/assayer/docs/note.md").expect("a path"),
                maximum: 1,
            }]),
        );

        assert_eq!(
            render_lists(std::slice::from_ref(&singleton), &lists),
            "[ASSAYER.\"legacy.section-references\"]\npath_counts = [\n  \
             { path = \"packages/assayer/docs/note.md\", maximum = 1 },\n]\n"
        );
    }

    /// The path-set request syntax uses the existing path selector for both
    /// modes. An audit selector and an append row therefore each carry a path
    /// and no maximum, because a file-level identity has no ceiling to choose.
    ///
    /// ´claim:control:path-set-requests-carry-a-path-and-no-ceiling´
    /// ´test:unit:path-set-requests-carry-a-path-and-no-ceiling´
    #[test]
    fn path_set_requests_carry_a_path_and_no_ceiling() {
        let pair = Pair::singleton("INDEX", "spdx.headers-conform");
        let path = RawRow {
            path: Some(String::from("src/one.rs")),
            fingerprint: None,
            maximum: None,
        };

        let (selectors, rows) = read_rows(
            &pair,
            Operation::Audit,
            Codec::PathSet,
            std::slice::from_ref(&path),
        )
        .expect("a path-set selector");

        assert_eq!(selectors.len(), 1);
        assert_eq!(rows, Vec::<super::Row>::new());

        let (selectors, rows) = read_rows(
            &pair,
            Operation::Append,
            Codec::PathSet,
            std::slice::from_ref(&path),
        )
        .expect("a path-set append row");

        assert_eq!(selectors, Vec::<super::Identity>::new());
        assert_eq!(
            serde_json::to_value(&rows[0]).expect("a row"),
            serde_json::json!({ "path": "src/one.rs" })
        );
        assert_eq!(Codec::PathSet.as_str(), "path-set");

        let counted = RawRow {
            maximum: Some(1),
            ..path
        };
        let defect = read_rows(&pair, Operation::Append, Codec::PathSet, &[counted])
            .expect_err("a ceiling is not in the codec");

        assert!(defect.contains("carries no ceiling"), "{defect}");
    }
}
