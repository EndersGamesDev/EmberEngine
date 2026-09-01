// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The carrier: which sources the checker reads.
//!
//! The minting judgment of ADR-L-014, A calculus of documentation and source labels, draws
//! minting judgments from the carrier — every authored prose and code source of
//! the corpus, excluding version-control internals, build and dependency
//! directories, archived and vendored trees, and generated artifacts.
//!
//! Wave L1 carried the repository's authored prose: the decision records, the
//! documentation tree, and the top-level readme. Wave T1 adds the packages'
//! authored prose beside it — the documentation tree and the decision records of
//! every package — because the migration of a package's corpus onto the calculus
//! cannot be checked by a tool that never reads it. The root's top-level prose
//! then arrives entire rather than one readme of it: the corpus convention
//! (ADR-L-019, The layer owner graph) names the root member's carrier
//! as the repository tree outside the packages tree, top-level prose included,
//! and a register short of that let the repository's own instruction file
//! prescribe retired forms with nothing reading it. The same register then
//! reaches two authored documents that sit below the root under no carried tree
//! — the repository's own audit of the vendored su-exec helper, and the
//! hand-written documentation of the upgrade between the first two major
//! versions — because where a document was filed is no argument about whether
//! the repository wrote it. Rust sources are still deliberately out of the
//! carrier even though the span reader already understands the code syntax;
//! scanning them needs the scanned-region recognition that a later wave fixes.
//!
//! The package trees are discovered rather than transcribed: every directory
//! under `packages/` contributes its own documentation and record subdirectories,
//! so a package joins the carrier by existing. A package with neither directory
//! contributes nothing, which is not a defect. The same discovery reaches one
//! document standing at a package's own root under no carried tree — its
//! changelog — because the coverage decision admits the title-bearing sources
//! that stood outside the carrier rather than leaving governed prose out of the
//! graph (ADR-L-024, Document-title labels).
//!
//! The coexistence caveat (ADR-L-014, A calculus of documentation and source labels) insists that
//! traversal failures surface as diagnostics and that an unreadable tree must
//! never become an empty carrier, so every unreadable directory or file becomes
//! a finding rather than a silent omission.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_markdown_below_the_carried_directories`] | carrier | The carrier gathers the authored prose of a tree: every Markdown document at any depth below the carried directories, together with the top-level readme. Nesting a document deeper does not remove it from the corpus. |
//! | [`carries_every_top_level_prose_document`] | carrier | The top-level prose the corpus convention names is carried entire and not one readme of it: the readme, the instruction file and the changelog all stand at the root with no directory over them, and the register holds each. A checker that read one of the three would leave the repository's own instructions free to prescribe whatever they liked, which is the state this pins shut. |
//! | [`leaves_an_unregistered_top_level_document_out_of_the_carrier`] | carrier | That reach is an enumeration and not a sweep of the root: a Markdown document nobody argued into the register stays outside the corpus, so widening the carrier stays a decision somebody made rather than a consequence of dropping a file at the root. |
//! | [`carries_the_argued_documents_below_the_root`] | carrier | The enumeration is a register of documents and not of root entries: an argued document is carried at the path it was argued at, however deep, while the vendored material beside one of them stays out. So the audit this repository wrote about a vendored helper is held to the calculus and the helper it audits is not, which is the ownership split the register is enforcing here until the declaration enforces it. |
//! | [`skips_excluded_directories`] | carrier | A tree the corpus declared away is never traversed, wherever in the tree it appears, so the corpus is the prose somebody wrote and not whatever a build happened to leave behind. The relation is the corpus's own declaration rather than a list compiled into this binary, so a tree that declares nothing is walked entire — which is the honest answer, since the removal is the corpus's to state and no walk may invent one for it. |
//! | [`skips_non_markdown_files`] | carrier | Only Markdown is carried: a text file or a Rust source sitting in a carried directory contributes nothing, so what the checker reads is decided by the document's kind and not by where it was left. |
//! | [`reaches_a_readme_at_any_depth_under_a_package`] | carrier | A readme is carried wherever it sits — beside a package's manifest, deep inside a test tree, or under the root package's own sources. The rule naming it is the filename rather than a location, so a folder documents itself where it lives instead of relocating its prose. |
//! | [`leaves_other_markdown_under_a_source_tree_out_of_the_carrier`] | carrier | That reach admits one filename and no other document: other Markdown sitting in a source tree stays outside the corpus, so working notes beside the code are not held to the calculus. |
//! | [`carries_a_package_root_changelog`] | carrier | A package's changelog is carried where a package's changelog stands: at the member's own root, under no carried tree, discovered with the member rather than transcribed one package at a time. Release prose the workspace wrote about a thing it publishes is prose of that member's corpus, and a checker that read the repository's changelog but not its members' was reading the same document twice over and governing it once. |
//! | [`leaves_a_changelog_that_is_not_a_packages_own_out_of_the_carrier`] | carrier | The changelog rule is the filename at a package's root and no deeper: the same name below a member is a fixture or a sample rather than the release prose of anything the workspace publishes, and the same name outside the packages tree belongs to no member at all. So the rule that reaches a member's changelog does not become a sweep for the word. |
//! | [`reports_a_missing_root_without_emptying_the_carrier`] | carrier | A tree that simply lacks some of the carried directories is read without complaint: the documents that are there are gathered, and no traversal failure is raised for the ones that are not. Absence is not the same as unreadability, and only the latter is a defect. |

#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::finding::Finding;
#[cfg(test)]
use crate::plan::CorpusPlan;

/// Directories whose Markdown the carrier reads, relative to the root.
///
/// These are the repository's own authored prose trees — its decision records
/// and its documentation — and the minting judgment is what puts them in the
/// carrier: minting is drawn from every committed prose and code source of the
/// corpus (ADR-L-014, A calculus of documentation and source labels). What the value is short of is that
/// judgment's full reach rather than a different rule: Rust sources are still
/// deliberately outside, awaiting the scanned-region recognition a later wave
/// fixes, so this list grows toward the judgment and never away from it.
///
/// ´const:emberlinter:repository-prose-trees´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:repository-prose-trees-form-xd53b2adf´
#[cfg(test)]
const CARRIED_DIRECTORIES: &[&str] = &["adr", "docs"];

/// Files the carrier reads, relative to the root.
///
/// This is the root corpus's *enumerated prose*: the documents the root member
/// carries by name because no carried tree stands over them. The register began
/// as the top-level prose, and that phrase is the record's rather than this
/// module's: the root member's carrier is the repository tree outside the
/// packages tree — `adr/`, `docs/`, `src/`, `tests/`, and the top-level prose
/// (ADR-L-019, The layer owner graph). The four trees are carried by
/// the constants around this one; prose with no directory over it, and prose
/// under a directory no constant carries, is enumerated here, and the filename
/// rule below reaches it only at depths these entries do not cover. Each is
/// committed authored prose of the corpus and so is drawn on by the minting
/// judgment like any other prose source (ADR-L-014, A calculus of documentation and source labels).
///
/// The value is an enumeration and not a sweep, because the judgment excludes
/// generated artifacts and vendored trees and a sweep could tell neither from
/// authored prose. Five documents are argued in today.
///
/// The readme, which wave L1 carried and which is the repository's own front
/// matter. The instruction file, which the convention names in its own text and
/// whose prescriptions had gone unchecked for exactly as long as it stood
/// outside. The changelog, which is hand-written release prose maintained entry
/// by entry rather than assembled by a tool. The su-exec audit, which is this
/// repository's own audit of the vendored helper rather than any part of the
/// helper: the ownership split keeps the repository-authored audit with the
/// repository owner and leaves the upstream material beside it — that tree's own
/// readme included — to a vendored owner that participates in nothing, so the
/// audit is authored prose which merely sits under a vendored directory while
/// the tree around it stays uncarried. The upgrade readme, which is hand-written
/// product documentation for the data upgrade from the first major version to
/// the second, and which no carried tree reaches because the filename rule stops
/// at the packages, source and test trees.
///
/// A further document arriving in the root corpus joins by being argued into
/// this list, which is the cost of not sweeping and is meant to be paid.
///
/// The declared configuration assigns all five to the root owner, by rows
/// broader than this value: it names the instruction file, the changelog and the
/// readme one line each under `INDEX`, and reaches the other two through whole
/// trees. The narrowing that parts the audit from the vendored material beside
/// it is decided but not yet declared, so this constant does that narrowing
/// itself meanwhile. The agreement is corroboration and not the warrant: the
/// warrant is the convention, and this constant answers to it until the register
/// moves out of this file and answers to that declaration instead.
///
/// ´const:emberlinter:repository-root-prose´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:repository-root-prose-form-x95281044´
#[cfg(test)]
const CARRIED_FILES: &[&str] = &[
    "AGENTS.md",
    "CHANGELOG.md",
    "README.md",
    "contrib/dev-tools/su-exec/AUDIT.md",
    "upgrades/from_v1_0_0_to_v2_0_0/README.md",
];

/// The directory holding the workspace's packages, relative to the root.
///
/// The value is a transcription of where this workspace's members sit, and no
/// record of the corpus fixes it: the manifest does, and the owner discovery
/// beside this module reads that manifest rather than this constant, so the two
/// answer the same question from different sources. That is the honest state and
/// it is owed a record rather than missing one.
///
/// TODO ´todo:code:record-where-a-workspace-member-s´: record where a workspace member's directory is fixed for the carrier as well as for the owners.
///
/// ´const:emberlinter:member-tree-home´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:member-tree-home-word-packages´
#[cfg(test)]
const PACKAGES_DIRECTORY: &str = "packages";

/// Subdirectories of a package whose Markdown the carrier reads.
///
/// A package's records and documentation are committed prose of the corpus, so
/// the minting judgment carries them exactly as it carries the repository's own
/// (ADR-L-014, A calculus of documentation and source labels). The reason to name them here rather than to
/// leave them for a later wave is that the migration of a package's corpus onto
/// the calculus cannot be checked by a tool that never reads it. The trees are
/// discovered rather than transcribed — every directory under the packages tree
/// contributes its own — so a package joins the carrier by existing, and one with
/// neither directory contributes nothing, which is no defect.
///
/// ´const:emberlinter:package-prose-trees´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:package-prose-trees-form-xd53b2adf´
#[cfg(test)]
const CARRIED_PACKAGE_DIRECTORIES: &[&str] = &["adr", "docs"];

/// The filename the carrier reaches at any depth.
///
/// The carrier extension is a filename rule and deliberately not a path rule: the
/// carrier reaches this name at any depth under a package, so the readme of a
/// test tree is carried where the test tree expects to find it
/// (ADR-L-017, The test documentation policy). The name is therefore the whole of
/// the rule, and admitting a second one would be admitting a second document
/// class rather than widening this one.
///
/// ´const:emberlinter:self-documenting-filename´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:self-documenting-filename-text-x633a5d62´
#[cfg(test)]
const CARRIED_FILENAME: &str = "README.md";

/// The filename the carrier reads at a package's own root.
///
/// A changelog is hand-written release prose maintained entry by entry rather
/// than assembled by a tool, which is the argument the root register makes for
/// the changelog standing over the repository, and a member's changelog is that
/// same document written about that member's releases: the two differ in whose
/// releases they record and in nothing else, so the minting judgment draws on
/// both alike (ADR-L-014, A calculus of documentation and source labels). What kept one of them out was
/// that no carried tree stood over it, which is a fact about where a document
/// was filed and no argument about whether the repository wrote it. The
/// coverage decision settles that: every title-bearing tracked source standing
/// outside the carrier is admitted to it, admission being part of the decision
/// rather than an expansion somebody might decline
/// (ADR-L-024, Document-title labels).
///
/// The rule is this name at a package's root and deliberately not at any depth,
/// which is where it parts from the readme rule beside it. A readme documents
/// the folder it stands in, so its rule follows it down; a changelog records the
/// releases of a published thing, and the published things of this workspace are
/// its members, so the document has exactly one place per member to stand. A
/// file of this name deeper in a tree is a fixture, a sample, or the prose of
/// something this workspace does not publish, and carrying it would be reading
/// the name rather than the document.
///
/// The packages are discovered rather than transcribed here as they are for the
/// prose trees, so a member's changelog joins the carrier by existing and a
/// member without one contributes nothing, which is no defect.
///
/// ´const:emberlinter:package-root-prose´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:package-root-prose-text-x3d162ee4´
#[cfg(test)]
const CARRIED_PACKAGE_FILENAME: &str = "CHANGELOG.md";

/// The trees the readme rule is applied over, relative to the root.
///
/// The filename rule reaches at any depth under a package
/// (ADR-L-017, The test documentation policy), and these are this workspace's
/// packages: the packages tree carries every member's readmes at every depth, and
/// the root package — which sits at the root itself — carries its own through its
/// source and test trees. Deciding the matrix carrier question by extension
/// rather than by relocation is what the requirement buys, so every package gains
/// the same reach in one adoption datum rather than one relocation at a time.
///
/// ´const:emberlinter:filename-rule-reach´ (ADR-L-018, The constant label profile)
/// ´const:emberlinter:filename-rule-reach-form-x924fb5f3´
#[cfg(test)]
const README_TREES: &[&str] = &["packages", "src", "tests"];

/// One carrier source, read into memory.
#[derive(Debug, Clone)]
pub struct Source {
    path: PathBuf,
    text: String,
}

impl Source {
    /// Build a source from a path and its text.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
        }
    }

    /// The source's path, as the carrier reported it.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The source's text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// The set of sources the checker reads, rooted at a directory.
#[derive(Debug, Clone)]
#[cfg(test)]
pub struct Carrier {
    root: PathBuf,
    paths: Vec<PathBuf>,
    findings: Vec<Finding>,
}

#[cfg(test)]
impl Carrier {
    /// The root the carrier's paths are relative to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read every source of the carrier, together with any traversal failures.
    ///
    /// Paths in the returned sources are relative to the root, so findings read
    /// the same wherever the repository is checked out.
    #[must_use]
    pub fn read(&self) -> (Vec<Source>, Vec<Finding>) {
        let mut findings = self.findings.clone();

        let sources = self
            .paths
            .iter()
            .cloned()
            .filter_map(
                |relative| match fs::read_to_string(self.root.join(&relative)) {
                    Ok(text) => Some(Source::new(relative, text)),
                    Err(error) => {
                        findings.push(Finding::TraversalFailure {
                            path: relative.to_string_lossy().into_owned(),
                            message: error.to_string(),
                        });
                        None
                    }
                },
            )
            .collect();

        (sources, findings)
    }
}

/// The carrier for this repository: repository prose beside package prose.
///
/// The declared shape is offered rather than required, because the carrier is
/// read over invented trees as well as over this one and a tree that declares no
/// shape has removed nothing. Where a corpus does declare one, the relation
/// reaches the package discovery too: a package directory the corpus disowned is
/// not a member whose prose is skipped, it is not a member.
#[must_use]
#[cfg(test)]
pub fn index_carrier(root: impl Into<PathBuf>, corpus: &CorpusPlan) -> Carrier {
    Carrier {
        root: root.into(),
        paths: corpus
            .native_paths()
            .iter()
            .filter(|path| is_carried(path))
            .cloned()
            .collect(),
        findings: corpus.findings().to_vec(),
    }
}

#[cfg(test)]
fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[cfg(test)]
fn is_carried(path: &Path) -> bool {
    if CARRIED_FILES
        .iter()
        .any(|carried| path == Path::new(carried))
    {
        return true;
    }

    if is_markdown(path)
        && CARRIED_DIRECTORIES
            .iter()
            .any(|tree| path.starts_with(tree))
    {
        return true;
    }

    if path
        .file_name()
        .is_some_and(|name| name == CARRIED_FILENAME)
        && README_TREES.iter().any(|tree| path.starts_with(tree))
    {
        return true;
    }

    let Ok(package_relative) = path.strip_prefix(PACKAGES_DIRECTORY) else {
        return false;
    };
    let mut components = package_relative.components();

    if components.next().is_none() {
        return false;
    }

    let member_relative = components.as_path();

    member_relative == Path::new(CARRIED_PACKAGE_FILENAME)
        || is_markdown(member_relative)
            && CARRIED_PACKAGE_DIRECTORIES
                .iter()
                .any(|tree| member_relative.starts_with(tree))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Carrier, index_carrier};
    use crate::declaration::AbnfPattern;
    use crate::finding::Finding;
    use crate::plan::CorpusPlan;
    use crate::universe::{IgnoreRow, UniverseKind};

    fn carrier(root: &std::path::Path, ignore: &[IgnoreRow]) -> Carrier {
        let corpus =
            CorpusPlan::compile(root, UniverseKind::AsWritten, ignore).expect("fixture topology");

        index_carrier(root, &corpus)
    }

    fn paths(carrier: &super::Carrier) -> Vec<String> {
        let (sources, _findings) = carrier.read();
        sources
            .iter()
            .map(|source| source.path().to_string_lossy().into_owned())
            .collect()
    }

    /// The carrier gathers the authored prose of a tree: every Markdown
    /// document at any depth below the carried directories, together with the
    /// top-level readme. Nesting a document deeper does not remove it from the
    /// corpus.
    ///
    /// ´claim:carrier:authored-markdown-is-gathered-at-any-depth´
    /// ´test:unit:reads-markdown-below-the-carried-directories´
    #[test]
    fn reads_markdown_below_the_carried_directories() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join("adr/nested")).expect("create");
        fs::write(root.path().join("adr/001-one.md"), "one").expect("write");
        fs::write(root.path().join("adr/nested/two.md"), "two").expect("write");
        fs::write(root.path().join("README.md"), "readme").expect("write");

        let mut found = paths(&carrier(root.path(), &[]));
        found.sort();

        assert_eq!(found, ["README.md", "adr/001-one.md", "adr/nested/two.md"]);
    }

    /// The top-level prose the corpus convention names is carried entire and
    /// not one readme of it: the readme, the instruction file and the changelog
    /// all stand at the root with no directory over them, and the register
    /// holds each. A checker that read one of the three would leave the
    /// repository's own instructions free to prescribe whatever they liked,
    /// which is the state this pins shut.
    ///
    /// ´claim:carrier:the-top-level-prose-is-carried-entire´
    /// ´test:unit:carries-every-top-level-prose-document´
    #[test]
    fn carries_every_top_level_prose_document() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(root.path().join("README.md"), "readme").expect("write");
        fs::write(root.path().join("AGENTS.md"), "instructions").expect("write");
        fs::write(root.path().join("CHANGELOG.md"), "releases").expect("write");

        let mut found = paths(&carrier(root.path(), &[]));
        found.sort();

        assert_eq!(
            found,
            ["AGENTS.md", "CHANGELOG.md", "README.md"],
            "the register holds the top-level prose the convention names, not a readme of it"
        );
    }

    /// That reach is an enumeration and not a sweep of the root: a Markdown
    /// document nobody argued into the register stays outside the corpus, so
    /// widening the carrier stays a decision somebody made rather than a
    /// consequence of dropping a file at the root.
    ///
    /// ´claim:carrier:the-top-level-reach-is-enumerated-and-not-swept´
    /// ´test:unit:leaves-an-unregistered-top-level-document-out-of-the-carrier´
    #[test]
    fn leaves_an_unregistered_top_level_document_out_of_the_carrier() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(root.path().join("AGENTS.md"), "instructions").expect("write");
        fs::write(root.path().join("NOTES.md"), "working notes").expect("write");

        assert_eq!(
            paths(&carrier(root.path(), &[])),
            ["AGENTS.md"],
            "the top-level reach is the register's enumeration and not every document at the root"
        );
    }

    /// The enumeration is a register of documents and not of root entries: an
    /// argued document is carried at the path it was argued at, however deep,
    /// while the vendored material beside one of them stays out. So the audit
    /// this repository wrote about a vendored helper is held to the calculus and
    /// the helper it audits is not, which is the ownership split the register is
    /// enforcing here until the declaration enforces it.
    ///
    /// ´claim:carrier:an-argued-document-is-carried-at-the-path-it-was-argued-at´
    /// ´test:unit:carries-the-argued-documents-below-the-root´
    #[test]
    fn carries_the_argued_documents_below_the_root() {
        let root = tempfile::tempdir().expect("temporary directory");

        fs::create_dir_all(root.path().join("contrib/dev-tools/su-exec")).expect("create");
        fs::create_dir_all(root.path().join("upgrades/from_v1_0_0_to_v2_0_0")).expect("create");
        fs::write(
            root.path().join("contrib/dev-tools/su-exec/AUDIT.md"),
            "audit",
        )
        .expect("write");
        fs::write(
            root.path().join("contrib/dev-tools/su-exec/README.md"),
            "upstream",
        )
        .expect("write");
        fs::write(
            root.path().join("upgrades/from_v1_0_0_to_v2_0_0/README.md"),
            "upgrade",
        )
        .expect("write");

        let mut found = paths(&carrier(root.path(), &[]));
        found.sort();

        assert_eq!(
            found,
            [
                "contrib/dev-tools/su-exec/AUDIT.md",
                "upgrades/from_v1_0_0_to_v2_0_0/README.md",
            ],
            "the register carries what was argued into it and leaves the vendored readme beside the audit alone"
        );
    }

    /// A tree the corpus declared away is never traversed, wherever in the tree
    /// it appears, so the corpus is the prose somebody wrote and not whatever a
    /// build happened to leave behind. The relation is the corpus's own
    /// declaration rather than a list compiled into this binary, so a tree that
    /// declares nothing is walked entire — which is the honest answer, since the
    /// removal is the corpus's to state and no walk may invent one for it.
    ///
    /// ´claim:carrier:excluded-directories-are-never-traversed´
    /// ´test:unit:skips-excluded-directories´
    #[test]
    fn skips_excluded_directories() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join("docs/target")).expect("create");
        fs::create_dir_all(root.path().join("docs/.git")).expect("create");
        fs::write(root.path().join("docs/kept.md"), "kept").expect("write");
        fs::write(root.path().join("docs/target/skipped.md"), "skipped").expect("write");
        fs::write(root.path().join("docs/.git/skipped.md"), "skipped").expect("write");

        // Nothing declared, nothing removed: the walk meets every document that
        // stands, because a compiled opinion about which trees are rubbish is
        // exactly what left with the constant.
        let mut walked = paths(&carrier(root.path(), &[]));
        walked.sort();

        assert_eq!(
            walked,
            [
                "docs/.git/skipped.md",
                "docs/kept.md",
                "docs/target/skipped.md"
            ]
        );

        // The rows say it instead, and each reaches its name wherever it stands
        // rather than only at the root the corpus happened to write it from.
        let declared = [
            ignoring("build-output", "target"),
            ignoring("version-control", ".git"),
        ];

        assert_eq!(paths(&carrier(root.path(), &declared)), ["docs/kept.md"]);
    }

    /// A row removing one directory name wherever it appears, as the shape
    /// document writes them.
    fn ignoring(name: &str, directory: &str) -> IgnoreRow {
        let pattern = format!(r#"[ *VCHAR "/" ] %s"{directory}" [ "/" *VCHAR ]"#);

        IgnoreRow::new(
            name,
            AbnfPattern::parse(&pattern).expect("a readable pattern"),
        )
    }

    /// Only Markdown is carried: a text file or a Rust source sitting in a
    /// carried directory contributes nothing, so what the checker reads is
    /// decided by the document's kind and not by where it was left.
    ///
    /// ´claim:carrier:only-markdown-is-carried´
    /// ´test:unit:skips-non-markdown-files´
    #[test]
    fn skips_non_markdown_files() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join("docs")).expect("create");
        fs::write(root.path().join("docs/kept.md"), "kept").expect("write");
        fs::write(root.path().join("docs/skipped.txt"), "skipped").expect("write");
        fs::write(root.path().join("docs/skipped.rs"), "skipped").expect("write");

        assert_eq!(paths(&carrier(root.path(), &[])), ["docs/kept.md"]);
    }

    /// A readme is carried wherever it sits — beside a package's manifest, deep
    /// inside a test tree, or under the root package's own sources. The rule
    /// naming it is the filename rather than a location, so a folder documents
    /// itself where it lives instead of relocating its prose.
    ///
    /// ´claim:carrier:a-readme-is-carried-at-any-depth´
    /// ´test:unit:reaches-a-readme-at-any-depth-under-a-package´
    #[test]
    fn reaches_a_readme_at_any_depth_under_a_package() {
        let root = tempfile::tempdir().expect("temporary directory");

        fs::create_dir_all(root.path().join("packages/demo/src/tests/resonance")).expect("create");
        fs::create_dir_all(root.path().join("src/tests")).expect("create");
        fs::write(root.path().join("packages/demo/README.md"), "package").expect("write");
        fs::write(
            root.path()
                .join("packages/demo/src/tests/resonance/README.md"),
            "deep",
        )
        .expect("write");
        fs::write(root.path().join("src/tests/README.md"), "root package").expect("write");

        let mut found = paths(&carrier(root.path(), &[]));
        found.sort();

        assert_eq!(
            found,
            [
                "packages/demo/README.md",
                "packages/demo/src/tests/resonance/README.md",
                "src/tests/README.md",
            ],
            "the rule is the filename, at any depth, and not a relocation"
        );
    }

    /// That reach admits one filename and no other document: other Markdown
    /// sitting in a source tree stays outside the corpus, so working notes
    /// beside the code are not held to the calculus.
    ///
    /// ´claim:carrier:the-reach-into-source-trees-admits-one-filename-only´
    /// ´test:unit:leaves-other-markdown-under-a-source-tree-out-of-the-carrier´
    #[test]
    fn leaves_other_markdown_under_a_source_tree_out_of_the_carrier() {
        let root = tempfile::tempdir().expect("temporary directory");

        fs::create_dir_all(root.path().join("packages/demo/src")).expect("create");
        fs::write(root.path().join("packages/demo/src/notes.md"), "notes").expect("write");
        fs::write(root.path().join("packages/demo/src/README.md"), "readme").expect("write");

        assert_eq!(
            paths(&carrier(root.path(), &[])),
            ["packages/demo/src/README.md"],
            "the extension names one filename and admits no other document"
        );
    }

    /// A package's changelog is carried where a package's changelog stands: at
    /// the member's own root, under no carried tree, discovered with the member
    /// rather than transcribed one package at a time. Release prose the
    /// workspace wrote about a thing it publishes is prose of that member's
    /// corpus, and a checker that read the repository's changelog but not its
    /// members' was reading the same document twice over and governing it once.
    ///
    /// ´claim:carrier:a-package-changelog-is-carried-at-the-package-root´
    /// ´test:unit:carries-a-package-root-changelog´
    #[test]
    fn carries_a_package_root_changelog() {
        let root = tempfile::tempdir().expect("temporary directory");

        fs::create_dir_all(root.path().join("packages/demo")).expect("create");
        fs::create_dir_all(root.path().join("packages/quiet")).expect("create");
        fs::write(root.path().join("packages/demo/CHANGELOG.md"), "releases").expect("write");

        assert_eq!(
            paths(&carrier(root.path(), &[])),
            ["packages/demo/CHANGELOG.md"],
            "a member's changelog joins by existing, and the member without one contributes nothing"
        );
    }

    /// The changelog rule is the filename at a package's root and no deeper:
    /// the same name below a member is a fixture or a sample rather than the
    /// release prose of anything the workspace publishes, and the same name
    /// outside the packages tree belongs to no member at all. So the rule that
    /// reaches a member's changelog does not become a sweep for the word.
    ///
    /// ´claim:carrier:the-changelog-rule-stops-at-the-package-root´
    /// ´test:unit:leaves-a-changelog-that-is-not-a-packages-own-out-of-the-carrier´
    #[test]
    fn leaves_a_changelog_that_is_not_a_packages_own_out_of_the_carrier() {
        let root = tempfile::tempdir().expect("temporary directory");

        fs::create_dir_all(root.path().join("packages/demo/src/fixtures")).expect("create");
        fs::create_dir_all(root.path().join("contrib/dev-tools")).expect("create");
        fs::write(root.path().join("packages/demo/CHANGELOG.md"), "releases").expect("write");
        fs::write(
            root.path().join("packages/demo/src/fixtures/CHANGELOG.md"),
            "fixture",
        )
        .expect("write");
        fs::write(
            root.path().join("contrib/dev-tools/CHANGELOG.md"),
            "upstream",
        )
        .expect("write");

        assert_eq!(
            paths(&carrier(root.path(), &[])),
            ["packages/demo/CHANGELOG.md"],
            "the rule names one place per member and does not follow the filename down or out"
        );
    }

    /// A tree that simply lacks some of the carried directories is read without
    /// complaint: the documents that are there are gathered, and no traversal
    /// failure is raised for the ones that are not. Absence is not the same as
    /// unreadability, and only the latter is a defect.
    ///
    /// ´claim:carrier:absent-carried-directories-are-not-traversal-failures´
    /// ´test:unit:reports-a-missing-root-without-emptying-the-carrier´
    #[test]
    fn reports_a_missing_root_without_emptying_the_carrier() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::create_dir_all(root.path().join("docs")).expect("create");
        fs::write(root.path().join("docs/kept.md"), "kept").expect("write");

        let carrier = carrier(root.path(), &[]);
        let (sources, findings) = carrier.read();

        assert_eq!(sources.len(), 1);
        assert!(
            findings
                .iter()
                .all(|finding| !matches!(finding, Finding::TraversalFailure { .. })),
            "a readable tree raises no traversal failure"
        );
    }
}
