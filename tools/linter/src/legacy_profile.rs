// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The marked inventory profile for implementation an owner has ruled legacy.
//!
//! The profile follows the to-do profile's established shape: one comment
//! recognizer reads both the labels the check validates and the unlabelled
//! remainder the burn census ratchets. Unlike that profile, this one covers only
//! production sources. The marker is an owner's architectural ruling; this
//! module derives a label from that ruling and never tries to infer legacy
//! status from the code's identifier.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`covers_only_production_source_trees`] | legacy | The profile covers a package's production source tree and excludes its test subtrees, whether tests stand below `src/` or beside it. |
//! | [`validates_labels_with_the_shared_marker_reader`] | legacy | The shared reader distinguishes a correctly labelled marker from a wrong one and an orphan, while leaving marker text inside a string literal outside the inventory. |

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use serde::Serialize;

use crate::comment::{LEADERS, comment_regions};
use crate::finding::{Finding, Location};
use crate::label::Label;
use crate::plan::ProfileSource;
use crate::profile::code_spans;
use crate::todo::transform_summary;

/// One legacy implementation site, as the shared recognizer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacySite {
    package: String,
    summary: String,
    carried: Option<Label>,
    location: Location,
}

impl LegacySite {
    /// The crate name of the package that owns this site.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The marker's summary, from which the label name derives.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// The label standing at the marker's standard place, when one does.
    #[must_use]
    pub const fn carried(&self) -> Option<&Label> {
        self.carried.as_ref()
    }

    /// Whether this site carries no label at its standard place.
    #[must_use]
    pub const fn is_unlabelled(&self) -> bool {
        self.carried.is_none()
    }

    /// Where the marker stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// Whether a kind belongs to the legacy-implementation profile.
#[must_use]
pub fn is_legacy_kind(kind: &str) -> bool {
    kind == "legacy"
}

/// Whether a workspace-relative Rust path belongs to a production source tree.
#[must_use]
pub fn is_production_source(path: &Path) -> bool {
    let mut components = path.components();

    while let Some(component) = components.next() {
        if component == Component::Normal("src".as_ref()) {
            return components
                .next()
                .is_none_or(|next| next != Component::Normal("tests".as_ref()));
        }
    }

    false
}

/// Derive the label of a marked legacy implementation from its summary.
#[must_use]
pub fn derive_legacy(summary: &str) -> Option<Label> {
    let name = transform_summary(summary)?;

    Label::parse(&format!("legacy:code:{name}"))
}

/// Read one Rust source for marked legacy sites and orphan labels of this kind.
#[must_use]
pub fn scan_legacy_sites(
    package: &str,
    path: &Path,
    source: &str,
) -> (Vec<LegacySite>, Vec<Finding>) {
    let mut sites = Vec::new();
    let mut orphans = Vec::new();

    for region in comment_regions(source) {
        let text = region.text(source);
        let mut offset = region.start();

        for line in text.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);

            match marker_of(trimmed) {
                Some(rest) => {
                    let (carried, summary) = read_site(rest);
                    let marker_offset = offset + (trimmed.len() - rest.len() - "LEGACY".len());

                    sites.push(LegacySite {
                        package: package.to_owned(),
                        summary,
                        carried,
                        location: Location::new(path, source, marker_offset),
                    });
                }
                None => orphans.extend(orphan_labels(path, source, offset, trimmed)),
            }

            offset += line.len();
        }
    }

    (sites, orphans)
}

/// The marker's tail when it opens a commentary line as a whole word.
fn marker_of(line: &str) -> Option<&str> {
    let stripped = line.trim_start_matches(LEADERS);
    let rest = stripped.strip_prefix("LEGACY")?;
    let continues = rest
        .chars()
        .next()
        .is_some_and(|character| character.is_alphanumeric() || character == '_');

    (!continues).then_some(rest)
}

/// Read the optional standard-place label and summary after a marker.
fn read_site(rest: &str) -> (Option<Label>, String) {
    let mut tail = rest.trim_start();
    let carried = match leading_label(tail) {
        Some((label, width)) => {
            tail = tail[width..].trim_start();
            Some(label)
        }
        None => None,
    };
    let summary = tail.strip_prefix(':').unwrap_or(tail).trim();

    (carried, summary.to_owned())
}

/// A legacy label opening a text as a bare acute span, with its byte width.
fn leading_label(tail: &str) -> Option<(Label, usize)> {
    let span = code_spans(tail).into_iter().next()?;

    if span.start != 0 || span.parenthesized {
        return None;
    }

    let label = Label::parse(span.interior)?;

    is_legacy_kind(label.kind()).then_some((label, span.end))
}

/// Every legacy label minted on a line no marker heads.
fn orphan_labels(path: &Path, source: &str, base: usize, line: &str) -> Vec<Finding> {
    code_spans(line)
        .into_iter()
        .filter(|span| !span.parenthesized)
        .filter_map(|span| {
            let label = Label::parse(span.interior)?;

            is_legacy_kind(label.kind()).then(|| Finding::OrphanLegacyLabel {
                label,
                location: Location::new(path, source, base + span.start),
            })
        })
        .collect()
}

/// The census over the execution plan's production Rust sources.
#[derive(Debug, Clone, Default)]
pub struct LegacyCensus {
    sites: Vec<LegacySite>,
    files_scanned: usize,
}

impl LegacyCensus {
    /// Every marked site, ordered by source path and position.
    #[must_use]
    pub fn sites(&self) -> &[LegacySite] {
        &self.sites
    }

    /// How many production Rust sources the census read.
    #[must_use]
    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }
}

/// Take the legacy-implementation census from the finite source projection.
#[must_use]
pub fn take_planned_legacy_census(
    root: &Path,
    sources: &[ProfileSource],
) -> (LegacyCensus, Vec<Finding>) {
    let mut census = LegacyCensus::default();
    let mut findings = Vec::new();

    for source in sources
        .iter()
        .filter(|source| is_production_source(source.path()))
    {
        let path = source.path();
        let text = match fs::read_to_string(root.join(path)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        census.files_scanned += 1;
        let (sites, orphans) = scan_legacy_sites(source.package(), path, &text);
        census.sites.extend(sites);
        findings.extend(orphans);
    }

    (census, findings)
}

/// One marked site paired with the label its summary derives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoveredLegacySite {
    site: LegacySite,
    label: Label,
}

impl CoveredLegacySite {
    /// The marked site this covers.
    #[must_use]
    pub const fn site(&self) -> &LegacySite {
        &self.site
    }

    /// The label its summary derives.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }
}

/// Pair every marked site with the label its summary derives.
#[must_use]
pub fn cover_legacy(census: &LegacyCensus) -> (Vec<CoveredLegacySite>, Vec<Finding>) {
    let mut covered = Vec::new();
    let mut findings = Vec::new();

    for site in census.sites() {
        match derive_legacy(site.summary()) {
            Some(label) => covered.push(CoveredLegacySite {
                site: site.clone(),
                label,
            }),
            None => findings.push(Finding::UnderivableAssetName {
                owner: site.package().to_owned(),
                asset: site.summary().to_owned(),
                transformed: String::new(),
                location: site.location().clone(),
            }),
        }
    }

    (covered, findings)
}

/// What the legacy-implementation profile found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LegacyAnalysis {
    /// How many production Rust sources the census read.
    pub files_scanned: usize,
    /// How many marked implementation sites the census covers.
    pub covered: usize,
    /// How many sites carry their derived label at the standard place.
    pub labelled: usize,
    /// How many sites carry no label and therefore remain in the burn family.
    pub unlabelled: usize,
    /// How many sites carry a label other than their derivation.
    pub wrong: usize,
    /// How many groups of labelled sites in one owner derive one label.
    pub collision_groups: usize,
    /// How many labelled sites stand in those groups.
    pub colliding_sites: usize,
    /// How many summaries transform into no well-formed name.
    pub underivable: usize,
}

/// Validate every marked legacy implementation against the profile.
#[must_use]
pub fn analyze_legacy(census: &LegacyCensus) -> (LegacyAnalysis, Vec<Finding>) {
    let mut analysis = LegacyAnalysis {
        files_scanned: census.files_scanned(),
        covered: census.sites().len(),
        ..LegacyAnalysis::default()
    };
    let mut findings = Vec::new();
    let mut labelled = Vec::new();

    for site in census.sites() {
        let Some(carried) = site.carried() else {
            analysis.unlabelled += 1;
            continue;
        };

        let Some(derived) = derive_legacy(site.summary()) else {
            analysis.underivable += 1;
            findings.push(Finding::UnderivableAssetName {
                owner: site.package().to_owned(),
                asset: site.summary().to_owned(),
                transformed: String::new(),
                location: site.location().clone(),
            });
            continue;
        };

        if *carried == derived {
            analysis.labelled += 1;
            labelled.push((site, derived));
        } else {
            analysis.wrong += 1;
            findings.push(Finding::WrongInventoryLabel {
                expected: derived,
                found: carried.clone(),
                owner: site.package().to_owned(),
                asset: site.summary().to_owned(),
                location: site.location().clone(),
            });
        }
    }

    collisions(&labelled, &mut analysis, &mut findings);
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, findings)
}

/// Report groups of labelled sites in one owner deriving one label.
fn collisions(
    labelled: &[(&LegacySite, Label)],
    analysis: &mut LegacyAnalysis,
    findings: &mut Vec<Finding>,
) {
    let mut groups: BTreeMap<(&str, &Label), Vec<&LegacySite>> = BTreeMap::new();

    for (site, label) in labelled {
        groups
            .entry((site.package(), label))
            .or_default()
            .push(site);
    }

    for ((owner, label), group) in groups {
        let Some((first, rest)) = group.split_first() else {
            continue;
        };

        if rest.is_empty() {
            continue;
        }

        analysis.collision_groups += 1;
        analysis.colliding_sites += group.len();

        for other in rest {
            findings.push(Finding::CollidingDerivation {
                asset: first.summary().to_owned(),
                owner: owner.to_owned(),
                first_label: label.clone(),
                second_label: label.clone(),
                first: first.location().clone(),
                second: other.location().clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{LegacyCensus, analyze_legacy, is_production_source, scan_legacy_sites};
    use crate::finding::Finding;

    /// The profile covers a package's production source tree and excludes its
    /// test subtrees, whether tests stand below `src/` or beside it.
    ///
    /// ´claim:legacy:the-profile-covers-only-production-source-trees´
    /// ´test:unit:covers-only-production-source-trees´
    #[test]
    fn covers_only_production_source_trees() {
        assert!(is_production_source(Path::new("packages/demo/src/lib.rs")));
        assert!(!is_production_source(Path::new(
            "packages/demo/src/tests/case.rs"
        )));
        assert!(!is_production_source(Path::new(
            "packages/demo/tests/case.rs"
        )));
    }

    /// The shared reader distinguishes a correctly labelled marker from a
    /// wrong one and an orphan, while leaving marker text inside a string
    /// literal outside the inventory.
    ///
    /// ´claim:legacy:one-reader-decides-the-profile-and-remainder-inventory´
    /// ´test:unit:validates-labels-with-the-shared-marker-reader´
    #[test]
    fn validates_labels_with_the_shared_marker_reader() {
        let acute = '\u{b4}';
        let source = format!(
            "// LEGACY {acute}legacy:code:old-result{acute}: old result\n\
             // LEGACY {acute}legacy:code:not-the-derivation{acute}: other result\n\
             // {acute}legacy:code:orphan{acute}\n\
             let quiet = \"// LEGACY: string data\";\n"
        );
        let (sites, mut findings) =
            scan_legacy_sites("ember-demo", Path::new("packages/demo/src/lib.rs"), &source);
        let census = LegacyCensus {
            sites,
            files_scanned: 1,
        };
        let (analysis, profile_findings) = analyze_legacy(&census);

        findings.extend(profile_findings);

        assert_eq!(analysis.covered, 2);
        assert_eq!(analysis.labelled, 1);
        assert_eq!(analysis.wrong, 1);
        assert!(matches!(
            findings.as_slice(),
            [
                Finding::OrphanLegacyLabel { .. },
                Finding::WrongInventoryLabel { .. }
            ]
        ));
    }
}
