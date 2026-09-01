// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The per-folder test matrix of ADR-L-017: one readme per Rust-bearing folder,
//! each carrying one generated matrix under an authored head.
//!
//! # The head is the authored part
//!
//! Three things are fixed about the head, and all three are derived rather than
//! chosen. The title states the level, which the folder's path classifies exactly
//! as ADR-L-015 classifies a test's: a folder under a package's top-level tests
//! tree is integration, a folder under the crate's tests directory is crate, and
//! any other folder under the sources is unit. One folder has one level, so one
//! readme has one matrix, and a title stating the wrong level is a finding rather
//! than a variation. The label is a Table-kind mint whose area is the owning
//! package's registered prefix lowercased — the label language admits no hyphen in
//! an area, so the prefix spelling is the one that fits — and whose name is the
//! folder's path within its package with the classification's root removed, then
//! the level, then the words naming what the table is. The mint is authored and
//! its spelling is checked against the derivation exactly as an attestation is.
//!
//! # The region is found by the label, never by position
//!
//! The matrix's position in the readme is free: the region runs from the
//! labeled head to the next heading, and everything a folder's authors want to
//! say about the folder stands wherever they want to say it. A readme whose
//! author adds a paragraph above the matrix keeps its matrix where it was,
//! which is the property the rejected Ansatz on positional regions
//! (ADR-L-017, The test documentation policy) was rejected for lacking.
//!
//! # The naming run below the head
//!
//! The head carries a heading, because the level has to stand in a title, and the
//! corpus's head discipline asks every head for a catalogued environment name. A
//! heading may leave its naming to the bold run below it, and this generator
//! writes that run: the heading mints and states the level, the run says which
//! environment was minted, and presentation reduction takes the run back to the
//! catalogued name. Without it the head would carry a title the kind registry
//! catalogues nowhere, and the generator would be writing a corpus that cannot
//! pass its own check.
//!
//! # The rows are citations like any others
//!
//! The Test cell is a real prose citation — the label hugged by parentheses in
//! a single-backtick span — because the row means its test: the assets caveat
//! of ADR-L-014, A calculus of documentation and source labels, says a document wanting the
//! roster wants a generated register, its rows citations like any others, and
//! the generated-compliance invariant
//! (ADR-L-014, A calculus of documentation and source labels) makes a generated citation
//! resolve against the completed registries the generator emitted from. The
//! checker therefore holds every row to the corpus as it actually is: a row
//! citing a renamed or deleted test dangles beside the exactness check instead
//! of quietly describing a folder that moved on. Two earlier forms preceded
//! this one, each correct under the text of its day: the bare acute form, which
//! read to a grep as a second mint and was ruled an error, and the
//! double-backtick display form, the right repair under the old blanket
//! exclusion and an interim state once the re-adopted record replaced that
//! exclusion with participation. A row still mints nothing — a generated
//! occurrence is a mint only where a profile sets its standard place in the
//! register, and none does — and nothing a row carries feeds the census it was
//! generated from.
//!
//! # What is staged
//!
//! A folder with no readme, and a readme with no matrix head, are both counted and
//! reported nowhere: the bootstrap that writes the corpus's hundred readmes is a
//! later wave. A matrix head that exists brings the whole contract with it —
//! exactly one, of the folder's own level, spelled as the derivation gives it, over
//! a region that is what the labels say it is.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`derives_the_records_two_worked_labels`] | projection | A folder's matrix label is derived from the package that owns it, the folder's path within that package, and the level of test the folder holds, so a folder's table is named by where it sits rather than by anyone's choice. |
//! | [`derives_a_nested_unit_folders_label`] | projection | A folder nested several directories deep contributes each of those directories to its label's name, so folders at different depths of one package cannot derive the same table label. |
//! | [`titles_the_head_at_the_folders_level`] | projection | A matrix head states the folder's level in its title as well as in its label, so a reader arriving at the readme is told which kind of test the folder holds without decoding the label. |
//! | [`renders_a_matrix_row_as_a_citation`] | projection | A matrix row cites the test it names — the label hugged by parentheses in a real prose citation — because the register's rows are citations like any others: the row means its test, the citation resolves against the census the generator emitted from, and a row naming a renamed or deleted test dangles instead of quietly describing a folder that moved on. The mint stays where the test is, so the label still occurs mint-shaped exactly once in the corpus, at its standard place. |
//! | [`renders_the_empty_statement_at_the_folders_level`] | projection | A folder holding no tests says so in a sentence naming its level, and carries no table at all. Emptiness written down is emptiness a reader can scan, and an empty table would say less than the sentence does. |
//! | [`finds_a_matrix_head_by_its_label_wherever_it_stands`] | projection | A committed matrix is found by its label wherever it stands in the readme, so a folder may say whatever it likes above its table and the generator still knows which head is its own. |
//! | [`bounds_the_region_at_the_next_heading`] | projection | The generated region ends at the next heading, so prose an author adds in a section of its own below the matrix is outside the region and survives every regeneration. |
//! | [`counts_a_folder_with_no_readme_rather_than_reporting_it`] | projection | A folder of tests with no readme at all is counted as lacking a matrix and reported nowhere, so the projection can be adopted gradually rather than failing every folder that has not yet been reached. |
//! | [`counts_a_readme_with_no_matrix_rather_than_reporting_it`] | projection | A readme of prose alone is likewise counted rather than reported: having a readme and having a matrix are separate facts, and a folder that documents itself without one is not yet a defect. |
//! | [`reports_a_stale_matrix`] | projection | cites (´claim:projection:an-index-that-is-not-what-the-labels-give-is-stale´) |
//! | [`reports_a_hand_edit_inside_the_matrix_region`] | projection | cites (´claim:projection:a-hand-edited-projection-is-stale-and-is-regenerated´) |
//! | [`reports_a_title_stating_the_wrong_level`] | projection | A title stating a level the folder does not hold is reported, naming both the title written and the level the folder actually classifies at, so a matrix cannot misdescribe the tests beneath it. |
//! | [`reports_a_readme_carrying_two_matrix_heads`] | projection | One folder classifies at one level, so one readme carries one matrix: a readme with two matrix heads is reported rather than having one of them chosen as the real one. |
//! | [`bootstraps_a_readme_for_a_folder_that_has_none`] | projection | A folder with no readme gains a whole one: the titled head carrying its derived label, the naming run beneath it, and the table of its tests. The folder documents itself from nothing. |
//! | [`appends_a_head_to_a_readme_that_has_none`] | projection | A readme that already says something keeps every word of it and gains the matrix below, so adopting the projection never costs a folder the prose its authors wrote. |
//! | [`writes_nothing_over_a_current_matrix`] | projection | cites (´claim:projection:projection-settles-after-one-pass´) |
//! | [`rewrites_a_stale_matrix_without_changing_how_the_file_ends`] | projection | Rewriting a stale matrix restores the bytes the bootstrap writes, and how the file ends is one of them. A matrix standing last in its readme runs to the end of the file, so the empty element its trailing newline leaves falls inside the span the rewrite replaces; the bootstrap and the rewrite would otherwise produce two different files from one set of labels, with the verifier accepting both and the corpus drifting into two forms. A readme that genuinely ended without a newline keeps ending without one, because a rewrite rewrites the region and nothing else. |
//! | [`writes_a_head_the_kind_registry_validates`] | projection | A generated readme is a document of the corpus like any other: it scans cleanly, its head is paired with the mint below it, and the registry validates the name that head carries. The generator writes prose the checker accepts, rather than prose exempted from it. |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(test)]
use crate::census::collect_rust_sources;
use crate::claim::{TABLE_DELIMITER, TABLE_HEADER, project_cells};
use crate::finding::{Finding, Location};
use crate::label::Label;
use crate::occurrence::Syntax;
#[cfg(test)]
use crate::plan::CorpusPlan;
use crate::plan::ProfileSource;
use crate::profile::{Area, CoveredAsset, classify};
use crate::roster::OwnerNames;
use crate::subscribe::Subscription;
#[cfg(test)]
use crate::workspace::Package;

/// The kind a matrix head mints, from the displays convention of ADR-L-011.
///
/// The record fixes the mint's kind rather than leaving the generator to choose
/// one: a matrix label is a mint of the Table kind, whose area and name are both
/// derived, and the spelling is that kind's own token in the displays convention
/// (´[EMBER-conv:testdocs:folder-matrix]´).
///
/// ´const:emberlinter:matrix-head-kind-token´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:matrix-head-kind-token-word-tab´
pub const MATRIX_KIND: &str = "tab";

/// The words closing every matrix title and every matrix label.
///
/// The derivation the record states ends here: the name is the folder's path
/// within its package with the level standing in for the root, then the level,
/// then this suffix — so the same words close the label and the title it heads
/// (´[EMBER-conv:testdocs:folder-matrix]´).
///
/// ´const:emberlinter:matrix-name-suffix´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:matrix-name-suffix-text-xb8df3dc1´
pub const MATRIX_SUFFIX: &str = "test-matrix";

/// The readme every Rust-bearing folder carries.
///
/// Two records fix this one name. Every folder containing a Rust source carries
/// a readme, and every such readme carries one generated matrix
/// (´[EMBER-conv:testdocs:folder-matrix]´); and the carrier reaches the file by
/// this filename at any depth under a package rather than by a path, so a test
/// tree's readme is carried where the test tree expects it
/// (´[EMBER-req:testdocs:carrier-extension]´).
///
/// ´const:emberlinter:folder-readme-filename´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:folder-readme-filename-text-x633a5d62´
pub const README: &str = "README.md";

/// The catalogued environment name the generated naming run writes.
///
/// The naming run is what lets the head validate: the title states the level and
/// is no catalogued environment name, so head validation reduces this bold run
/// to the kind the label declares (´[EMBER-conv:testdocs:folder-matrix]´). The
/// value is therefore the catalogued name of that kind and cannot be reworded.
///
/// ´const:emberlinter:matrix-naming-run-environment´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:matrix-naming-run-environment-text-x22a8d19f´
const ENVIRONMENT: &str = "Table";

/// One folder that carries a Rust source, and so wants a readme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixFolder {
    directory: PathBuf,
    package: String,
    level: Area,
    label: Label,
}

impl MatrixFolder {
    /// The folder, relative to the workspace root.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The readme this folder carries, relative to the workspace root.
    #[must_use]
    pub fn readme(&self) -> PathBuf {
        self.directory.join(README)
    }

    /// The crate name of the package owning this folder.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The level the folder's path classifies it at.
    #[must_use]
    pub const fn level(&self) -> Area {
        self.level
    }

    /// The mint the derivation gives this folder's matrix head.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// The title the head states the level in.
    #[must_use]
    pub fn title(&self) -> String {
        format!("{} test matrix", capitalize(self.level.as_str()))
    }

    /// The head, as it stands in the readme.
    #[must_use]
    pub fn head(&self) -> String {
        format!("## {} · `{}`", self.title(), self.label)
    }

    /// Build a folder from parts already derived.
    ///
    /// Exposed for tests that name a folder directly rather than walking a tree.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_parts(
        directory: PathBuf,
        package: String,
        level: Area,
        label: Label,
    ) -> Self {
        Self {
            directory,
            package,
            level,
            label,
        }
    }
}

/// Raise a word's first character, so a level word opens a title.
fn capitalize(word: &str) -> String {
    let mut characters = word.chars();

    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// Derive the matrix label of a folder within a package.
///
/// The name is the folder's path within its package with the classification's own
/// root removed — the level already names it — then the level, then the words
/// naming what the table is. The package's own tests directory therefore gives a
/// name of the level and the suffix alone, and a folder below a root contributes
/// its remaining components before them.
#[must_use]
pub fn derive_matrix(
    prefix: &str,
    package_directory: &Path,
    folder: &Path,
    level: Area,
) -> Option<Label> {
    let relative = folder.strip_prefix(package_directory).ok()?;
    let root = match level {
        Area::Integration => "tests",
        Area::Crate => "src/tests",
        Area::Unit => "src",
    };

    let within = relative.strip_prefix(root).ok()?;

    let mut segments: Vec<String> = within
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('_', "-"))
        .collect();

    segments.push(level.as_str().to_owned());
    segments.push(MATRIX_SUFFIX.to_owned());

    Label::parse(&format!("{MATRIX_KIND}:{prefix}:{}", segments.join("-")))
}

/// Every folder of a workspace that carries a Rust source.
///
/// The roots are the census's own, so a folder the test profile never reads never
/// gains a readme either: the two passes agree about what a package's sources are,
/// and a matrix over a tree with no covered assets would be a table about nothing.
///
/// The subscription is consulted per folder, at the readme the projection would
/// read or write: a folder whose readme stands in an unsubscribed owner's share
/// is not a matrix folder at all, so neither the verification nor the bootstrap
/// sweep reaches it. Routing here is what keeps the two modes agreeing about
/// which folders exist.
#[must_use]
#[cfg(test)]
pub fn folders(
    packages: &[Package],
    names: Option<&OwnerNames>,
    corpus: &CorpusPlan,
    subscription: &Subscription<'_>,
) -> (Vec<MatrixFolder>, Vec<Finding>) {
    let mut found = Vec::new();
    let findings = Vec::new();
    let mut seen = BTreeSet::new();

    for package in packages {
        let Some(prefix) = names.and_then(|names| names.derive(package.name())) else {
            continue;
        };
        let prefix = prefix.as_str().to_lowercase();

        let mut paths = Vec::new();

        for directory in crate::census::CENSUSED_DIRECTORIES {
            collect_rust_sources(corpus, &package.directory().join(directory), &mut paths);
        }

        for path in paths {
            let Some(directory) = path.parent().map(Path::to_path_buf) else {
                continue;
            };

            if !seen.insert(directory.clone()) {
                continue;
            }

            let Some(level) = classify(package.directory(), &directory) else {
                continue;
            };

            let Some(label) = derive_matrix(&prefix, package.directory(), &directory, level) else {
                continue;
            };

            let folder = MatrixFolder {
                directory,
                package: package.name().to_owned(),
                level,
                label,
            };

            if !subscription.governs(&folder.readme()) {
                continue;
            }

            found.push(folder);
        }
    }

    found.sort_by(|left, right| left.directory.cmp(&right.directory));

    (found, findings)
}

/// Derive matrix folders from the execution plan's finite Rust source projection.
#[must_use]
pub fn planned_folders(
    sources: &[ProfileSource],
    names: Option<&OwnerNames>,
    subscription: &Subscription<'_>,
) -> (Vec<MatrixFolder>, Vec<Finding>) {
    let mut found = Vec::new();
    let findings = Vec::new();
    let mut seen = BTreeSet::new();

    for source in sources {
        let Some(prefix) = names.and_then(|names| names.derive(source.package())) else {
            continue;
        };
        let Some(directory) = source.path().parent().map(Path::to_path_buf) else {
            continue;
        };

        if !seen.insert(directory.clone()) {
            continue;
        }

        let Some(level) = classify(source.package_directory(), &directory) else {
            continue;
        };
        let Some(label) = derive_matrix(
            &prefix.as_str().to_lowercase(),
            source.package_directory(),
            &directory,
            level,
        ) else {
            continue;
        };
        let folder = MatrixFolder {
            directory,
            package: source.package().to_owned(),
            level,
            label,
        };

        if subscription.governs(&folder.readme()) {
            found.push(folder);
        }
    }

    found.sort_by(|left, right| left.directory.cmp(&right.directory));

    (found, findings)
}

/// The generated region of one folder's matrix: the naming run, then the body.
///
/// The recipe is fixed and normalising, so a second write over an unchanged corpus
/// reproduces the same bytes. An empty folder gets the honest statement that there
/// are no tests here, at the level its classification gives: emptiness that is
/// written down is scannable, where emptiness that is a missing file is
/// indistinguishable from a folder nobody has reached.
#[must_use]
pub fn region(folder: &MatrixFolder, assets: &[&CoveredAsset]) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        format!("**{ENVIRONMENT} ({})**", folder.title()),
        String::new(),
    ];

    if assets.is_empty() {
        lines.push(format!(
            "No {} tests in this folder.",
            folder.level.as_str()
        ));

        return lines;
    }

    lines.push(TABLE_HEADER.to_owned());
    lines.push(TABLE_DELIMITER.to_owned());

    for asset in assets {
        let cells = project_cells(asset, Syntax::Prose);
        let mut row = String::new();
        let _ignored = write!(
            row,
            "| (`{}`) | {} | {} |",
            asset.label(),
            cells.area,
            cells.claim
        );

        lines.push(row);
    }

    lines
}

/// One matrix head found in a committed readme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedMatrix {
    /// The zero-based line the head stands on.
    pub head_line: usize,
    /// The zero-based line one past the region's last, at the next heading.
    pub past_last_line: usize,
    /// The head's title, as written.
    pub title: String,
    /// The head's mint, as written.
    pub label: Label,
    /// The region's lines, with trailing blanks removed.
    pub lines: Vec<String>,
}

/// Find every matrix head of a readme, and the region each opens.
///
/// A head is found by its label and never by its position: a heading carrying a
/// Table-kind mint whose name closes with the matrix suffix is a matrix head,
/// wherever in the document its author put it. The region runs from the head to
/// the next heading.
#[must_use]
pub fn committed_matrices(text: &str) -> Vec<CommittedMatrix> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut found = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with('#') {
            continue;
        }

        let Some(label) = head_label(line) else {
            continue;
        };

        let past_last_line = lines
            .iter()
            .enumerate()
            .skip(index + 1)
            .find(|(_position, following)| following.trim_start().starts_with('#'))
            .map_or(lines.len(), |(position, _following)| position);

        let mut region: Vec<String> = lines[index + 1..past_last_line]
            .iter()
            .map(|line| (*line).to_owned())
            .collect();

        while region.last().is_some_and(|line| line.trim().is_empty()) {
            let _dropped = region.pop();
        }

        found.push(CommittedMatrix {
            head_line: index,
            past_last_line,
            title: heading_title(line),
            label,
            lines: region,
        });
    }

    found
}

/// The matrix mint a heading carries, when it carries one.
fn head_label(line: &str) -> Option<Label> {
    let mut rest = line;

    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let close = after.find('`')?;
        let interior = &after[..close];

        if let Some(label) = Label::parse(interior)
            && label.kind() == MATRIX_KIND
            && label.name().ends_with(MATRIX_SUFFIX)
        {
            return Some(label);
        }

        rest = &after[close + 1..];
    }

    None
}

/// A heading's title, with its marker and the separator before its mint removed.
fn heading_title(line: &str) -> String {
    let without_marker = line.trim_start().trim_start_matches('#').trim();

    without_marker
        .split('\u{b7}')
        .next()
        .unwrap_or(without_marker)
        .trim()
        .to_owned()
}

/// What the folder-matrix pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MatrixAnalysis {
    /// How many folders carry a Rust source, and so want a readme.
    pub folders: usize,
    /// How many of them carry a readme at all.
    pub with_readme: usize,
    /// How many carry a readme holding a matrix head.
    pub with_matrix: usize,
    /// How many carry none, which the bootstrap sweep will write.
    pub without_matrix: usize,
    /// How many carry a matrix that is not what their labels say it is.
    pub stale: usize,
    /// How many carry a head the derivation does not give them.
    pub misderived: usize,
    /// How many folders classify at each level.
    pub by_level: BTreeMap<String, usize>,
    /// How many of the folders hold no covered test, and say so.
    pub empty: usize,
}

/// Verify every committed folder matrix of a workspace.
///
/// The folders verified are the ones the routed walk yields, so an
/// unsubscribed owner's folders are absent from every figure here rather than
/// counted and excused.
#[must_use]
#[cfg(test)]
pub fn verify_matrices(
    root: &Path,
    packages: &[Package],
    assets: &[CoveredAsset],
    names: Option<&OwnerNames>,
    corpus: &CorpusPlan,
    subscription: &Subscription<'_>,
) -> (MatrixAnalysis, Vec<Finding>) {
    let (folders, findings) = folders(packages, names, corpus, subscription);

    verify_folder_rows(root, &folders, assets, findings)
}

/// Verify the execution plan's finite matrix-folder projection.
#[must_use]
pub fn verify_planned_matrices(
    root: &Path,
    folders: &[MatrixFolder],
    assets: &[CoveredAsset],
) -> (MatrixAnalysis, Vec<Finding>) {
    verify_folder_rows(root, folders, assets, Vec::new())
}

fn verify_folder_rows(
    root: &Path,
    folders: &[MatrixFolder],
    assets: &[CoveredAsset],
    mut findings: Vec<Finding>,
) -> (MatrixAnalysis, Vec<Finding>) {
    let mut analysis = MatrixAnalysis::default();
    let by_folder = assets_by_folder(assets);

    for area in Area::all() {
        analysis.by_level.insert(area.as_str().to_owned(), 0);
    }

    for folder in folders {
        analysis.folders += 1;
        *analysis
            .by_level
            .entry(folder.level.as_str().to_owned())
            .or_default() += 1;

        let empty = by_folder
            .get(folder.directory())
            .map_or(&Vec::new(), |assets| assets)
            .is_empty();

        if empty {
            analysis.empty += 1;
        }

        let readme = folder.readme();
        let Ok(text) = fs::read_to_string(root.join(&readme)) else {
            analysis.without_matrix += 1;
            continue;
        };

        analysis.with_readme += 1;

        let committed = committed_matrices(&text);
        let [matrix] = committed.as_slice() else {
            if committed.is_empty() {
                analysis.without_matrix += 1;
            } else {
                analysis.with_matrix += 1;
                report_repetition(&readme, &text, &committed, &mut findings);
            }
            continue;
        };

        analysis.with_matrix += 1;

        let displayed = readme.to_string_lossy().into_owned();
        let location = line_location(&readme, &text, matrix.head_line);

        if matrix.title != folder.title() || matrix.label != folder.label {
            analysis.misderived += 1;
            findings.push(Finding::WrongMatrixLevel {
                path: displayed.clone(),
                title: matrix.title.clone(),
                expected: folder.level.as_str().to_owned(),
                label: folder.label.clone(),
                location: location.clone(),
            });
        }

        let owned: Vec<&CoveredAsset> = by_folder
            .get(folder.directory())
            .map_or_else(Vec::new, Clone::clone);

        if matrix.lines != region(folder, &owned) {
            analysis.stale += 1;
            findings.push(Finding::StaleFolderMatrix {
                path: displayed,
                label: folder.label.clone(),
                location,
            });
        }
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, findings)
}

/// Report a readme carrying more than one matrix head.
fn report_repetition(
    readme: &Path,
    text: &str,
    committed: &[CommittedMatrix],
    findings: &mut Vec<Finding>,
) {
    let Some((first, rest)) = committed.split_first() else {
        return;
    };

    for other in rest {
        findings.push(Finding::RepeatedFolderMatrix {
            path: readme.to_string_lossy().into_owned(),
            first: line_location(readme, text, first.head_line),
            second: line_location(readme, text, other.head_line),
        });
    }
}

/// Group covered assets by the folder their source stands in.
#[must_use]
pub fn assets_by_folder(assets: &[CoveredAsset]) -> BTreeMap<PathBuf, Vec<&CoveredAsset>> {
    let mut folders: BTreeMap<PathBuf, Vec<&CoveredAsset>> = BTreeMap::new();

    for asset in assets {
        if let Some(directory) = asset.test().path().parent() {
            folders
                .entry(directory.to_path_buf())
                .or_default()
                .push(asset);
        }
    }

    folders
}

/// The location of a zero-based line of a text.
fn line_location(path: &Path, text: &str, line: usize) -> Location {
    let offset = text
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>()
        .min(text.len());

    Location::new(path, text, offset)
}

/// Rewrite one readme's matrix region, or author the readme and its head.
///
/// Returns the new text when it differs from the old, and nothing when the
/// committed matrix is already what the labels say it is. A readme carrying no
/// head at all gains the head at its end, below whatever its authors wrote; a
/// folder carrying no readme gains one holding the head and the region alone.
#[must_use]
pub fn write_matrix(
    existing: Option<&str>,
    folder: &MatrixFolder,
    assets: &[&CoveredAsset],
) -> Option<String> {
    let body = region(folder, assets);

    let Some(text) = existing else {
        return Some(format!("{}\n{}\n", folder.head(), body.join("\n")));
    };

    let committed = committed_matrices(text);
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();

    let Some(matrix) = committed.first() else {
        let mut written = Vec::new();

        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            let _dropped = lines.pop();
        }

        written.push(String::new());
        written.push(folder.head());
        written.extend(body);
        written.push(String::new());

        lines.extend(written);

        return Some(lines.join("\n"));
    };

    let head = folder.head();

    if matrix.lines == body && lines.get(matrix.head_line) == Some(&head) {
        return None;
    }

    let mut replacement = vec![head];
    replacement.extend(body);

    let _removed: Vec<String> = lines
        .splice(matrix.head_line..matrix.past_last_line, replacement)
        .collect();

    let mut rewritten = lines.join("\n");

    // A matrix standing last in its readme runs to the end of the file, so the
    // empty final element the file's trailing newline produces falls inside the
    // span the splice replaces and is dropped along with it. The bootstrap writes
    // that newline and the constant sweep preserves it the same way: a rewrite
    // rewrites the region, never how the file ends.
    if text.ends_with('\n') && !rewritten.ends_with('\n') {
        rewritten.push('\n');
    }

    Some(rewritten)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        MatrixFolder, committed_matrices, derive_matrix, region, verify_matrices, write_matrix,
    };
    use crate::census::{Census, scan_source};
    use crate::finding::Finding;
    use crate::plan::CorpusPlan;
    use crate::profile::{Area, CoveredAsset, cover};
    use crate::roster::OwnerNames;
    use crate::subscribe::Subscription;
    use crate::universe::UniverseKind;
    use crate::workspace::Package;

    /// The reconciliation the owner-name document declares, as these tests read it.
    fn names() -> OwnerNames {
        OwnerNames::new("ember-", [])
    }

    fn corpus(root: &Path) -> CorpusPlan {
        CorpusPlan::compile(root, UniverseKind::AsWritten, &[]).expect("fixture topology")
    }

    /// The acute the code syntax delimits an occurrence with.
    const ACUTE: char = '\u{b4}';

    fn demo_folder(level: Area, directory: &str) -> MatrixFolder {
        let label = derive_matrix(
            "demo",
            Path::new("packages/demo"),
            Path::new(directory),
            level,
        )
        .expect("a well-formed label");

        MatrixFolder::from_parts(
            PathBuf::from(directory),
            "ember-demo".to_owned(),
            level,
            label,
        )
    }

    /// A folder's matrix label is derived from the package that owns it, the
    /// folder's path within that package, and the level of test the folder
    /// holds, so a folder's table is named by where it sits rather than by
    /// anyone's choice.
    ///
    /// ´claim:projection:a-matrix-label-is-derived-from-the-folders-owner-path-and-level´
    /// ´test:unit:derives-the-records-two-worked-labels´
    #[test]
    fn derives_the_records_two_worked_labels() {
        let package = Path::new("packages/assayer");

        assert_eq!(
            derive_matrix(
                "assayer",
                package,
                Path::new("packages/assayer/tests"),
                Area::Integration
            )
            .expect("a label")
            .to_string(),
            "tab:assayer:integration-test-matrix"
        );
        assert_eq!(
            derive_matrix(
                "assayer",
                package,
                Path::new("packages/assayer/src/tests/resonance"),
                Area::Crate
            )
            .expect("a label")
            .to_string(),
            "tab:assayer:resonance-crate-test-matrix"
        );
    }

    /// A folder nested several directories deep contributes each of those
    /// directories to its label's name, so folders at different depths of one
    /// package cannot derive the same table label.
    ///
    /// ´claim:projection:every-directory-of-a-nested-folders-path-enters-its-label´
    /// ´test:unit:derives-a-nested-unit-folders-label´
    #[test]
    fn derives_a_nested_unit_folders_label() {
        assert_eq!(
            derive_matrix(
                "assayer",
                Path::new("packages/assayer"),
                Path::new("packages/assayer/src/maths/spectral"),
                Area::Unit
            )
            .expect("a label")
            .to_string(),
            "tab:assayer:maths-spectral-unit-test-matrix"
        );
    }

    /// A matrix head states the folder's level in its title as well as in its
    /// label, so a reader arriving at the readme is told which kind of test the
    /// folder holds without decoding the label.
    ///
    /// ´claim:projection:a-matrix-head-states-its-folders-level-in-its-title´
    /// ´test:unit:titles-the-head-at-the-folders-level´
    #[test]
    fn titles_the_head_at_the_folders_level() {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests/resonance");

        assert_eq!(folder.title(), "Crate test matrix");
        assert_eq!(
            folder.head(),
            "## Crate test matrix · `tab:demo:resonance-crate-test-matrix`"
        );
    }

    fn assets_of(path: &str, text: &str) -> Vec<CoveredAsset> {
        let packages = vec![Package::new("ember-demo", "packages/demo")];
        let tests = scan_source("ember-demo", Path::new(path), text).expect("a Rust source");
        let (assets, findings) = cover(&packages, &Census::from_tests(tests, 1));

        assert!(
            findings.is_empty(),
            "the fixture covers cleanly: {findings:?}"
        );

        assets
    }

    fn minting() -> String {
        format!(
            "/// The widths are identical.\n///\n/// {ACUTE}claim:resonance:crossover-widths{ACUTE}\n\
             /// {ACUTE}test:crate:widths-are-identical{ACUTE}\n#[test]\nfn widths_are_identical() {{}}\n"
        )
    }

    /// A matrix row cites the test it names — the label hugged by parentheses
    /// in a real prose citation — because the register's rows are citations
    /// like any others: the row means its test, the citation resolves against
    /// the census the generator emitted from, and a row naming a renamed or
    /// deleted test dangles instead of quietly describing a folder that moved
    /// on. The mint stays where the test is, so the label still occurs
    /// mint-shaped exactly once in the corpus, at its standard place.
    ///
    /// ´claim:projection:a-matrix-rows-are-citations-like-any-others´
    /// ´test:unit:renders-a-matrix-row-as-a-citation´
    #[test]
    fn renders_a_matrix_row_as_a_citation() {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests");
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let lines = region(&folder, &borrowed);

        assert_eq!(lines[1], "**Table (Crate test matrix)**");
        assert_eq!(lines[3], "| Test | Area | Claim |");
        assert_eq!(
            lines[5],
            "| (`test:crate:widths-are-identical`) | resonance | The widths are identical. |"
        );
    }

    /// A folder holding no tests says so in a sentence naming its level, and
    /// carries no table at all. Emptiness written down is emptiness a reader
    /// can scan, and an empty table would say less than the sentence does.
    ///
    /// ´claim:projection:an-empty-folder-states-its-emptiness-instead-of-tabling-it´
    /// ´test:unit:renders-the-empty-statement-at-the-folders-level´
    #[test]
    fn renders_the_empty_statement_at_the_folders_level() {
        let folder = demo_folder(Area::Unit, "packages/demo/src/maths");
        let lines = region(&folder, &[]);

        assert_eq!(
            lines.last().expect("a statement"),
            "No unit tests in this folder."
        );
        assert!(
            !lines.iter().any(|line| line.starts_with("| Test")),
            "an empty folder carries no table: {lines:?}"
        );
    }

    /// A committed matrix is found by its label wherever it stands in the
    /// readme, so a folder may say whatever it likes above its table and the
    /// generator still knows which head is its own.
    ///
    /// ´claim:projection:a-matrix-is-found-by-its-label-wherever-it-stands´
    /// ´test:unit:finds-a-matrix-head-by-its-label-wherever-it-stands´
    #[test]
    fn finds_a_matrix_head_by_its_label_wherever_it_stands() {
        let text = concat!(
            "# The folder\n\nWhatever the authors wanted to say.\n\n",
            "## Crate test matrix · `tab:demo:resonance-crate-test-matrix`\n\n",
            "**Table (Crate test matrix)**\n\nNo crate tests in this folder.\n",
        );

        let found = committed_matrices(text);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].head_line, 4);
        assert_eq!(found[0].title, "Crate test matrix");
        assert_eq!(
            found[0].label.to_string(),
            "tab:demo:resonance-crate-test-matrix"
        );
    }

    /// The generated region ends at the next heading, so prose an author adds
    /// in a section of its own below the matrix is outside the region and
    /// survives every regeneration.
    ///
    /// ´claim:projection:the-matrix-region-ends-at-the-next-heading´
    /// ´test:unit:bounds-the-region-at-the-next-heading´
    #[test]
    fn bounds_the_region_at_the_next_heading() {
        let text = concat!(
            "## Crate test matrix · `tab:demo:crate-test-matrix`\n\n",
            "**Table (Crate test matrix)**\n\nNo crate tests in this folder.\n\n",
            "## Something the authors added\n\nMore prose.\n",
        );

        let found = committed_matrices(text);

        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].lines,
            [
                "",
                "**Table (Crate test matrix)**",
                "",
                "No crate tests in this folder."
            ]
        );
    }

    /// A workspace root carrying one package, one test source, and a readme.
    fn root_with(readme: Option<&str>) -> (tempfile::TempDir, Vec<Package>, Vec<CoveredAsset>) {
        let root = tempfile::tempdir().expect("temporary directory");
        let tests = root.path().join("packages/demo/src/tests");

        fs::create_dir_all(&tests).expect("create");
        fs::write(tests.join("demo.rs"), minting()).expect("write");

        if let Some(text) = readme {
            fs::write(tests.join("README.md"), text).expect("write");
        }

        let packages = vec![Package::new("ember-demo", "packages/demo")];
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());

        (root, packages, assets)
    }

    /// A folder of tests with no readme at all is counted as lacking a matrix
    /// and reported nowhere, so the projection can be adopted gradually rather
    /// than failing every folder that has not yet been reached.
    ///
    /// ´claim:projection:a-folder-with-no-readme-is-counted-not-reported´
    /// ´test:unit:counts-a-folder-with-no-readme-rather-than-reporting-it´
    #[test]
    fn counts_a_folder_with_no_readme_rather_than_reporting_it() {
        let (root, packages, assets) = root_with(None);
        let (analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert_eq!(analysis.folders, 1);
        assert_eq!(analysis.with_readme, 0);
        assert_eq!(analysis.without_matrix, 1);
        assert!(
            findings.is_empty(),
            "the bootstrap sweep is a later wave: {findings:?}"
        );
    }

    /// A readme of prose alone is likewise counted rather than reported: having
    /// a readme and having a matrix are separate facts, and a folder that
    /// documents itself without one is not yet a defect.
    ///
    /// ´claim:projection:a-readme-without-a-matrix-is-counted-not-reported´
    /// ´test:unit:counts-a-readme-with-no-matrix-rather-than-reporting-it´
    #[test]
    fn counts_a_readme_with_no_matrix_rather_than_reporting_it() {
        let (root, packages, assets) = root_with(Some("# The folder\n\nProse only.\n"));
        let (analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert_eq!(analysis.with_readme, 1);
        assert_eq!(analysis.with_matrix, 0);
        assert_eq!(analysis.without_matrix, 1);
        assert!(findings.is_empty(), "findings: {findings:?}");
    }

    /// The readme a current matrix produces for the demonstration folder.
    fn current_readme() -> String {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests");
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        write_matrix(None, &folder, &borrowed).expect("a first write")
    }

    /// A matrix that exists must be exactly what the labels give: one whose
    /// cell no longer matches the claim it projects is stale and reported.
    ///
    /// (´claim:projection:an-index-that-is-not-what-the-labels-give-is-stale´)
    /// ´test:unit:reports-a-stale-matrix´
    #[test]
    fn reports_a_stale_matrix() {
        let stale =
            current_readme().replace("The widths are identical.", "Something else entirely.");
        let (root, packages, assets) = root_with(Some(&stale));
        let (analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert_eq!(analysis.with_matrix, 1);
        assert_eq!(analysis.stale, 1);
        assert!(
            matches!(findings.as_slice(), [Finding::StaleFolderMatrix { .. }]),
            "expected one stale matrix, got {findings:?}"
        );
    }

    /// A sentence added by hand inside the matrix region is staleness by
    /// another name: the region belongs to the generator, and prose an author
    /// wants to keep belongs outside it.
    ///
    /// (´claim:projection:a-hand-edited-projection-is-stale-and-is-regenerated´)
    /// ´test:unit:reports-a-hand-edit-inside-the-matrix-region´
    #[test]
    fn reports_a_hand_edit_inside_the_matrix_region() {
        let edited = format!(
            "{}\nA sentence a reader added.\n",
            current_readme().trim_end()
        );
        let (root, packages, assets) = root_with(Some(&edited));
        let (analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert_eq!(
            analysis.stale, 1,
            "a hand-edit is staleness by another name"
        );
        assert!(matches!(
            findings.as_slice(),
            [Finding::StaleFolderMatrix { .. }]
        ));
    }

    /// A title stating a level the folder does not hold is reported, naming
    /// both the title written and the level the folder actually classifies at,
    /// so a matrix cannot misdescribe the tests beneath it.
    ///
    /// ´claim:projection:a-matrix-title-stating-the-wrong-level-is-reported-with-both´
    /// ´test:unit:reports-a-title-stating-the-wrong-level´
    #[test]
    fn reports_a_title_stating_the_wrong_level() {
        let wrong = current_readme().replace("## Crate test matrix", "## Unit test matrix");
        let (root, packages, assets) = root_with(Some(&wrong));
        let (analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert_eq!(analysis.misderived, 1);
        let [
            Finding::WrongMatrixLevel {
                title, expected, ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one wrong level, got {findings:?}");
        };

        assert_eq!(title, "Unit test matrix");
        assert_eq!(expected, "crate");
    }

    /// One folder classifies at one level, so one readme carries one matrix: a
    /// readme with two matrix heads is reported rather than having one of them
    /// chosen as the real one.
    ///
    /// ´claim:projection:a-readme-carrying-two-matrices-is-reported´
    /// ´test:unit:reports-a-readme-carrying-two-matrix-heads´
    #[test]
    fn reports_a_readme_carrying_two_matrix_heads() {
        let twice = format!("{}\n{}", current_readme(), current_readme());
        let (root, packages, assets) = root_with(Some(&twice));
        let (_analysis, findings) = verify_matrices(
            root.path(),
            &packages,
            &assets,
            Some(&names()),
            &corpus(root.path()),
            &Subscription::fictional_all(),
        );

        assert!(
            matches!(findings.as_slice(), [Finding::RepeatedFolderMatrix { .. }]),
            "one folder has one level, so one readme has one matrix: {findings:?}"
        );
    }

    /// A folder with no readme gains a whole one: the titled head carrying its
    /// derived label, the naming run beneath it, and the table of its tests.
    /// The folder documents itself from nothing.
    ///
    /// ´claim:projection:a-folder-with-no-readme-gains-a-complete-one´
    /// ´test:unit:bootstraps-a-readme-for-a-folder-that-has-none´
    #[test]
    fn bootstraps_a_readme_for_a_folder_that_has_none() {
        let readme = current_readme();

        assert!(
            readme.starts_with("## Crate test matrix · `tab:demo:crate-test-matrix`\n"),
            "{readme}"
        );
        assert!(readme.contains("**Table (Crate test matrix)**"), "{readme}");
        assert!(readme.contains("| Test | Area | Claim |"), "{readme}");
    }

    /// A readme that already says something keeps every word of it and gains
    /// the matrix below, so adopting the projection never costs a folder the
    /// prose its authors wrote.
    ///
    /// ´claim:projection:an-existing-readme-keeps-its-prose-and-gains-the-matrix-below´
    /// ´test:unit:appends-a-head-to-a-readme-that-has-none´
    #[test]
    fn appends_a_head_to_a_readme_that_has_none() {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests");
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let written = write_matrix(
            Some("# The folder\n\nProse the authors wrote.\n"),
            &folder,
            &borrowed,
        )
        .expect("a write");

        assert!(
            written.starts_with("# The folder\n\nProse the authors wrote.\n"),
            "{written}"
        );
        assert!(
            written.contains("\n## Crate test matrix · `tab:demo:crate-test-matrix`\n"),
            "{written}"
        );
    }

    /// A readme whose matrix is already current is not rewritten, so
    /// regenerating a settled tree produces no change.
    ///
    /// (´claim:projection:projection-settles-after-one-pass´)
    /// ´test:unit:writes-nothing-over-a-current-matrix´
    #[test]
    fn writes_nothing_over_a_current_matrix() {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests");
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();
        let once = write_matrix(None, &folder, &borrowed).expect("a first write");

        assert_eq!(
            write_matrix(Some(&once), &folder, &borrowed),
            None,
            "a second run changes nothing"
        );
    }

    /// Rewriting a stale matrix restores the bytes the bootstrap writes, and
    /// how the file ends is one of them. A matrix standing last in its readme
    /// runs to the end of the file, so the empty element its trailing newline
    /// leaves falls inside the span the rewrite replaces; the bootstrap and the
    /// rewrite would otherwise produce two different files from one set of
    /// labels, with the verifier accepting both and the corpus drifting into
    /// two forms. A readme that genuinely ended without a newline keeps ending
    /// without one, because a rewrite rewrites the region and nothing else.
    ///
    /// ´claim:projection:a-rewritten-matrix-keeps-how-its-file-ends´
    /// ´test:unit:rewrites-a-stale-matrix-without-changing-how-the-file-ends´
    #[test]
    fn rewrites_a_stale_matrix_without_changing_how_the_file_ends() {
        let folder = demo_folder(Area::Crate, "packages/demo/src/tests");
        let assets = assets_of("packages/demo/src/tests/demo.rs", &minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();
        let current = current_readme();

        assert!(
            current.ends_with('\n'),
            "the bootstrap ends the file with a newline"
        );

        let stale = current.replace("The widths are identical.", "Something else entirely.");
        let repaired =
            write_matrix(Some(&stale), &folder, &borrowed).expect("a stale matrix is rewritten");

        assert_eq!(
            repaired, current,
            "so the rewrite restores the bootstrap's bytes exactly"
        );

        let unterminated = stale.trim_end().to_owned();
        let repaired = write_matrix(Some(&unterminated), &folder, &borrowed)
            .expect("a stale matrix is rewritten");

        assert!(
            !repaired.ends_with('\n'),
            "and a readme that ended without a newline still does: {repaired:?}"
        );
    }

    /// A generated readme is a document of the corpus like any other: it scans
    /// cleanly, its head is paired with the mint below it, and the registry
    /// validates the name that head carries. The generator writes prose the
    /// checker accepts, rather than prose exempted from it.
    ///
    /// ´claim:projection:a-generated-readme-passes-the-checks-every-document-passes´
    /// ´test:unit:writes-a-head-the-kind-registry-validates´
    #[test]
    fn writes_a_head_the_kind_registry_validates() {
        let readme = current_readme();
        let path = Path::new("packages/demo/src/tests/README.md");
        let (occurrences, blocks, scan_findings) =
            crate::prose::scan_markdown(path, &readme).into_parts();

        assert!(
            scan_findings.is_empty(),
            "the generated readme scans cleanly: {scan_findings:?}"
        );

        let (heads, head_findings) = crate::head::read_heads(path, &readme, &blocks, &occurrences);

        assert!(
            head_findings.is_empty(),
            "every head is paired: {head_findings:?}"
        );
        assert_eq!(heads.len(), 1, "the matrix head, and nothing else");

        let defects =
            crate::head::validate_heads(&crate::registry::fixture_kind_registry(), &heads);

        assert!(
            defects.is_empty(),
            "the naming run takes the head back to a catalogued name: {defects:?}"
        );
    }
}
