// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The census of the test profile: every test function the harness recognizes.
//!
//! ADR-T-015 fixes the census of its profile as every test function the cargo
//! harness recognizes in every workspace member, and the caveat in ADR-T-014,
//! A calculus of documentation and source labels, insists that a test is whatever
//! the harness says it is and that the profile only reads the facts the harness
//! exposes. This module reads exactly those facts, and it reads them from the
//! abstract syntax rather than from the bytes: sources are parsed with syn, so
//! a test attribute inside a string literal, a macro body, or a comment is
//! never mistaken for the real thing.
//!
//! # What counts as a test attribute
//!
//! An attribute counts when the last segment of its path is `test` and it stands
//! as a bare path or a call. That admits the plain attribute, the asynchronous
//! one this workspace uses, and any future harness attribute of the same shape,
//! without a list of blessed spellings. A function carrying several such
//! attributes is one covered asset, counted once: the census is of functions,
//! not of attributes.
//!
//! Membership is read from the source alone. A test behind a configuration gate
//! is in the census, and so is one marked ignored, because both are functions
//! the harness recognizes and neither fact is visible to a reader of the label.
//!
//! # How sources are enumerated
//!
//! Tracked files are enumerated directly, and module declarations are never
//! followed. That is deliberate. The alternative — walking module declarations
//! from each crate root — visits a file once per target that includes it, and
//! this workspace includes one integration support module from ten separate
//! targets; following declarations would make ten covered assets of one
//! function and report nine collisions that exist nowhere in the source.
//! Enumerating files gives each function exactly one census entry, which is what
//! the invariant (ADR-T-014, A calculus of documentation and source labels) is about. Path attributes
//! therefore need no handling: a file reached by one is still enumerated,
//! exactly once, on its own.
//!
//! The file universe is the repository's tracked corpus, shared with the owner
//! partition. Untracked sources are facts about one working copy rather than
//! the commit being checked, so they enter no census. The listing carries paths
//! as NUL-separated bytes and failure to take it refuses the census rather than
//! returning an empty profile.
//!
//! Test functions written inside macro definitions are beyond a parser's reach
//! and are not covered; this workspace defines none.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`covers_the_plain_and_asynchronous_test_attributes`] | census | A test is whatever the harness recognises as one, so an attribute whose last path segment is the test word marks a covered function: the plain attribute and the asynchronous one both count, and so would any future attribute of the same shape, without a list of blessed spellings. |
//! | [`covers_a_function_carrying_several_test_attributes_once`] | census | A function carrying several test attributes is one covered asset, not several: the census counts functions rather than attributes, so a function cannot be made to look like two tests by being marked twice. |
//! | [`leaves_untested_functions_out_of_the_census`] | census | A function the harness would not run is no part of the census, however much its attributes resemble the test one: a plain function, one merely compiled under the test configuration, one marked as panicking, and one whose attribute is applied conditionally are none of them covered. |
//! | [`descends_into_nested_and_gated_modules`] | census | The census reaches tests at any module depth within a file, and a module gated behind a configuration attribute is descended into like any other, so where an author chose to nest a test does not decide whether it is counted. |
//! | [`never_follows_module_declarations`] | census | A module declaration is never followed: each file is enumerated on its own and contributes only the tests written in it. A support module included from many targets therefore yields one entry per function rather than one per target, and the collisions that would invent are never invented. |
//! | [`reads_line_oriented_documentation`] | census | A test's documentation is gathered as the lines its author wrote, knowing which line each occupies, which line the attributes begin on and where the function itself stands. That is everything a later pass needs to read a label off the last line or to write one above it. |
//! | [`reads_block_documentation_as_several_lines`] | census | Documentation written as one block comment yields the same lines, and is marked as not line-oriented: its lines are not separately editable, so a pass that rewrites documentation knows it cannot simply insert one. |
//! | [`records_no_documentation_when_there_is_none`] | census | An undocumented test records no documentation rather than an empty one, so the absence is distinguishable, and its attribute position is known regardless — which is where a first line would have to go. |
//! | [`records_the_indentation_of_the_first_attribute`] | census | The indentation a test is written at is recorded with it, so a line inserted above it can be laid out to match rather than arriving flush against the margin inside a nested module. |
//! | [`reports_a_source_it_cannot_parse`] | census | A source that will not parse is refused with the parser's own explanation rather than read as a file containing no tests, so a broken file shrinks no census silently. |
//! | [`computes_byte_offsets_from_lines_and_columns`] | census | A line and column convert back to a byte offset in the source, counting characters rather than bytes, so a position taken from a line carrying text outside ASCII still lands where that text actually begins. |

#[cfg(test)]
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Item, ItemFn, Lit, Meta};

use crate::finding::{Finding, Location};
#[cfg(test)]
use crate::plan::CorpusPlan;
use crate::plan::ProfileSource;
#[cfg(test)]
use crate::workspace::Package;

/// Directories of a package whose Rust sources the census reads.
///
/// The areas of ADR-T-015 live under exactly these two roots: the crate's own
/// sources, and the package's integration test targets
/// (ADR-T-015, The test label profile). That classification table is what
/// fixes the pair — a unit test stands inline in a source, a crate test under
/// the sources' own test directory, and an integration test in a target under
/// the package's top-level test root, so the two roots cover all three areas and
/// nothing else. Benchmark and example targets can carry test functions in
/// principle; this workspace's carry none, and the table has no area for them,
/// so they stay out of the census until a decision gives them one.
///
/// ´const:indexlinter:test-census-package-roots´ (ADR-T-018, The constant label profile)
/// ´const:indexlinter:test-census-package-roots-form-xac563a2a´
#[cfg(test)]
pub const CENSUSED_DIRECTORIES: &[&str] = &["src", "tests"];

/// One documentation comment, as the standard-place rule needs to see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocComment {
    lines: Vec<String>,
    first_line: usize,
    last_line: usize,
    line_oriented: bool,
}

impl DocComment {
    /// The documentation's logical lines, in order, each trimmed.
    ///
    /// One line-comment attribute contributes one line; a block comment
    /// contributes each of its own lines.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The final logical line, which is the profile's standard place.
    #[must_use]
    pub fn final_line(&self) -> Option<&str> {
        self.lines.last().map(String::as_str)
    }

    /// The one-based file line where the documentation begins.
    #[must_use]
    pub const fn first_line(&self) -> usize {
        self.first_line
    }

    /// The one-based file line where the documentation ends.
    #[must_use]
    pub const fn last_line(&self) -> usize {
        self.last_line
    }

    /// Whether every documentation attribute occupied exactly one file line.
    ///
    /// Line-oriented documentation can be repaired by whole-line edits. A block
    /// comment cannot, so the fix mode refuses it rather than guessing.
    #[must_use]
    pub const fn is_line_oriented(&self) -> bool {
        self.line_oriented
    }
}

/// One covered asset: a test function the harness recognizes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFunction {
    package: String,
    path: PathBuf,
    function: String,
    location: Location,
    attribute_line: usize,
    indent: String,
    doc: Option<DocComment>,
}

impl TestFunction {
    /// The crate name of the package that owns this asset.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The source file, relative to the workspace root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The function's bare identifier, which the name transformation reads.
    #[must_use]
    pub fn function(&self) -> &str {
        &self.function
    }

    /// Where the function's identifier stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// The one-based line of the function's first attribute.
    ///
    /// This is where documentation is inserted when the function has none.
    #[must_use]
    pub const fn attribute_line(&self) -> usize {
        self.attribute_line
    }

    /// The leading whitespace of the function's first attribute line.
    #[must_use]
    pub fn indent(&self) -> &str {
        &self.indent
    }

    /// The function's documentation comment, when it has one.
    #[must_use]
    pub const fn doc(&self) -> Option<&DocComment> {
        self.doc.as_ref()
    }
}

/// The census over a workspace, together with what it took to read it.
#[derive(Debug, Clone, Default)]
pub struct Census {
    tests: Vec<TestFunction>,
    files_scanned: usize,
}

impl Census {
    /// Build a census from functions already scanned.
    ///
    /// Exposed for tests that assemble a census from source text rather than
    /// from a tree on disk.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_tests(tests: Vec<TestFunction>, files_scanned: usize) -> Self {
        Self {
            tests,
            files_scanned,
        }
    }

    /// Every covered asset, ordered by source path and position.
    #[must_use]
    pub fn tests(&self) -> &[TestFunction] {
        &self.tests
    }

    /// How many Rust sources the census parsed.
    #[must_use]
    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }
}

/// Take the census of every package of a workspace.
#[must_use]
#[cfg(test)]
pub fn take_census(
    root: &Path,
    packages: &[Package],
    corpus: &CorpusPlan,
) -> (Census, Vec<Finding>) {
    let mut census = Census::default();
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();

    for package in packages {
        let mut paths: Vec<PathBuf> = corpus
            .native_paths()
            .iter()
            .filter(|path| is_censused_source(package, path))
            .cloned()
            .collect();

        paths.sort();

        for path in paths {
            if !seen.insert(path.clone()) {
                continue;
            }

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

            census.files_scanned += 1;

            match scan_source(package.name(), &path, &text) {
                Ok(tests) => census.tests.extend(tests),
                Err(message) => findings.push(Finding::SourceParseFailure {
                    path: path.to_string_lossy().into_owned(),
                    message,
                }),
            }
        }
    }

    (census, findings)
}

/// Take the test census from the execution plan's finite source projection.
#[must_use]
pub fn take_planned_census(root: &Path, sources: &[ProfileSource]) -> (Census, Vec<Finding>) {
    let mut census = Census::default();
    let mut findings = Vec::new();

    for source in sources {
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

        match scan_source(source.package(), path, &text) {
            Ok(tests) => census.tests.extend(tests),
            Err(message) => findings.push(Finding::SourceParseFailure {
                path: path.to_string_lossy().into_owned(),
                message,
            }),
        }
    }

    (census, findings)
}

/// Whether a tracked path is one of a package's censused Rust sources.
///
/// The removal is asked of the whole path once rather than of each component of
/// its parent in turn. The relation is over paths, so a path it reaches is gone
/// whether the reason stands at the root of the tree or three directories down,
/// and asking component by component was the shape a compiled list of bare names
/// needed rather than one this question ever had.
#[cfg(test)]
fn is_censused_source(package: &Package, path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "rs") {
        return false;
    }

    CENSUSED_DIRECTORIES
        .iter()
        .any(|directory| path.starts_with(package.directory().join(directory)))
}

/// Take the census of one Rust source.
///
/// # Errors
///
/// Returns the parser's complaint when the text is not a Rust source file.
pub fn scan_source(package: &str, path: &Path, text: &str) -> Result<Vec<TestFunction>, String> {
    let file = syn::parse_file(text).map_err(|error| error.to_string())?;
    let starts = line_starts(text);
    let mut tests = Vec::new();

    walk_items(&file.items, package, path, text, &starts, &mut tests);

    Ok(tests)
}

/// Walk a module body, descending into every inline module.
///
/// Configuration-gated modules are walked like any other: the gate is a fact of
/// the build, not of the source the profile reads.
fn walk_items(
    items: &[Item],
    package: &str,
    path: &Path,
    text: &str,
    starts: &[usize],
    tests: &mut Vec<TestFunction>,
) {
    for item in items {
        match item {
            Item::Fn(function) => {
                if let Some(covered) = cover(function, package, path, text, starts) {
                    tests.push(covered);
                }
            }
            Item::Mod(module) => {
                if let Some((_brace, inner)) = &module.content {
                    walk_items(inner, package, path, text, starts, tests);
                }
            }
            _ => {}
        }
    }
}

/// Build a covered asset from a function, when a test attribute covers it.
fn cover(
    function: &ItemFn,
    package: &str,
    path: &Path,
    text: &str,
    starts: &[usize],
) -> Option<TestFunction> {
    if !function.attrs.iter().any(is_test_attribute) {
        return None;
    }

    let identifier = function.sig.ident.span().start();
    let attribute_line = function
        .attrs
        .iter()
        .map(|attribute| attribute.span().start().line)
        .min()
        .unwrap_or(identifier.line);

    let offset = byte_offset(text, starts, identifier.line, identifier.column);

    Some(TestFunction {
        package: package.to_owned(),
        path: path.to_path_buf(),
        function: function.sig.ident.to_string(),
        location: Location::new(path, text, offset),
        attribute_line,
        indent: indent_of(text, starts, attribute_line),
        doc: read_doc(&function.attrs),
    })
}

/// Whether an attribute is one the test harness recognizes.
fn is_test_attribute(attribute: &Attribute) -> bool {
    let path = attribute.path();
    let ends_in_test = path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test");

    ends_in_test && matches!(attribute.meta, Meta::Path(_) | Meta::List(_))
}

/// Gather the documentation attributes of an item into one comment.
pub fn read_doc(attributes: &[Attribute]) -> Option<DocComment> {
    let mut lines = Vec::new();
    let mut first_line = None;
    let mut last_line = 0;
    let mut line_oriented = true;

    for attribute in attributes {
        let Some(value) = doc_value(attribute) else {
            continue;
        };

        let span = attribute.span();
        let start = span.start().line;
        let end = span.end().line;

        if first_line.is_none() {
            first_line = Some(start);
        }
        last_line = last_line.max(end);

        if end != start {
            line_oriented = false;
        }

        for line in value.split('\n') {
            lines.push(line.trim().to_owned());
        }
    }

    first_line.map(|first_line| DocComment {
        lines,
        first_line,
        last_line,
        line_oriented,
    })
}

/// The string a documentation attribute carries, when it is one.
fn doc_value(attribute: &Attribute) -> Option<String> {
    if !attribute.path().is_ident("doc") {
        return None;
    }

    let Meta::NameValue(name_value) = &attribute.meta else {
        return None;
    };

    match &name_value.value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(literal),
            ..
        }) => Some(literal.value()),
        _ => None,
    }
}

/// The byte offset at which each one-based line of a text begins.
pub fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];

    starts.extend(
        text.bytes()
            .enumerate()
            .filter(|(_index, byte)| *byte == b'\n')
            .map(|(index, _byte)| index + 1),
    );

    starts
}

/// The byte offset of a one-based line and zero-based character column.
pub fn byte_offset(text: &str, starts: &[usize], line: usize, column: usize) -> usize {
    let start = line
        .checked_sub(1)
        .and_then(|index| starts.get(index))
        .copied();
    let Some(start) = start else {
        return text.len();
    };
    let end = starts.get(line).copied().unwrap_or(text.len());
    let slice = &text[start..end];

    start
        + slice
            .char_indices()
            .nth(column)
            .map_or(slice.len(), |(index, _)| index)
}

/// The leading whitespace of a one-based line.
fn indent_of(text: &str, starts: &[usize], line: usize) -> String {
    let start = line
        .checked_sub(1)
        .and_then(|index| starts.get(index))
        .copied();
    let Some(start) = start else {
        return String::new();
    };
    let end = starts.get(line).copied().unwrap_or(text.len());

    text[start..end]
        .chars()
        .take_while(|character| character.is_whitespace() && *character != '\n')
        .collect()
}

/// Walk a directory, gathering Rust sources relative to the root.
#[cfg(test)]
pub fn collect_rust_sources(corpus: &CorpusPlan, relative: &Path, sources: &mut Vec<PathBuf>) {
    sources.extend(
        corpus
            .native_paths()
            .iter()
            .filter(|path| path.starts_with(relative))
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .cloned(),
    );
}

#[cfg(test)]
pub fn retiring_source_rows(packages: &[Package], corpus: &CorpusPlan) -> Vec<(String, PathBuf)> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();

    for package in packages {
        let mut paths: Vec<_> = corpus
            .native_paths()
            .iter()
            .filter(|path| is_censused_source(package, path))
            .cloned()
            .collect();
        paths.sort();

        rows.extend(
            paths
                .into_iter()
                .filter(|path| seen.insert(path.clone()))
                .map(|path| (package.name().to_owned(), path)),
        );
    }

    rows
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::{byte_offset, line_starts, scan_source, take_census};
    use crate::finding::Finding;
    use crate::plan::CorpusPlan;
    use crate::universe::UniverseKind;
    use crate::workspace::Package;

    fn scan(text: &str) -> Vec<super::TestFunction> {
        scan_source("torrust-demo", Path::new("src/demo.rs"), text).expect("a Rust source")
    }

    /// A test is whatever the harness recognises as one, so an attribute whose
    /// last path segment is the test word marks a covered function: the plain
    /// attribute and the asynchronous one both count, and so would any future
    /// attribute of the same shape, without a list of blessed spellings.
    ///
    /// ´claim:census:any-attribute-ending-in-the-test-word-covers-a-function´
    /// ´test:unit:covers-the-plain-and-asynchronous-test-attributes´
    #[test]
    fn covers_the_plain_and_asynchronous_test_attributes() {
        let tests = scan("#[test]\nfn alpha() {}\n\n#[tokio::test]\nasync fn beta() {}\n");

        let names: Vec<&str> = tests.iter().map(super::TestFunction::function).collect();
        assert_eq!(names, ["alpha", "beta"]);
    }

    /// A function carrying several test attributes is one covered asset, not
    /// several: the census counts functions rather than attributes, so a
    /// function cannot be made to look like two tests by being marked twice.
    ///
    /// ´claim:census:a-function-is-covered-once-however-many-test-attributes-it-carries´
    /// ´test:unit:covers-a-function-carrying-several-test-attributes-once´
    #[test]
    fn covers_a_function_carrying_several_test_attributes_once() {
        let tests = scan("#[test]\n#[tokio::test]\nfn alpha() {}\n");

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].function(), "alpha");
    }

    /// A function the harness would not run is no part of the census, however
    /// much its attributes resemble the test one: a plain function, one merely
    /// compiled under the test configuration, one marked as panicking, and one
    /// whose attribute is applied conditionally are none of them covered.
    ///
    /// ´claim:census:a-function-the-harness-would-not-run-is-not-covered´
    /// ´test:unit:leaves-untested-functions-out-of-the-census´
    #[test]
    fn leaves_untested_functions_out_of_the_census() {
        let tests = scan(concat!(
            "fn plain() {}\n",
            "#[cfg(test)]\nfn gated() {}\n",
            "#[should_panic]\nfn panicking() {}\n",
            "#[cfg_attr(test, ignore)]\nfn conditioned() {}\n",
        ));

        assert!(tests.is_empty(), "no function is covered: {tests:?}");
    }

    /// The census reaches tests at any module depth within a file, and a module
    /// gated behind a configuration attribute is descended into like any other,
    /// so where an author chose to nest a test does not decide whether it is
    /// counted.
    ///
    /// ´claim:census:tests-are-found-at-any-module-depth-gated-or-not´
    /// ´test:unit:descends-into-nested-and-gated-modules´
    #[test]
    fn descends_into_nested_and_gated_modules() {
        let tests = scan(concat!(
            "#[cfg(test)]\nmod tests {\n",
            "    #[test]\n    fn outer() {}\n",
            "    mod deeper {\n",
            "        #[test]\n        fn inner() {}\n",
            "    }\n",
            "}\n",
        ));

        let names: Vec<&str> = tests.iter().map(super::TestFunction::function).collect();
        assert_eq!(names, ["outer", "inner"]);
    }

    /// A module declaration is never followed: each file is enumerated on its
    /// own and contributes only the tests written in it. A support module
    /// included from many targets therefore yields one entry per function
    /// rather than one per target, and the collisions that would invent are
    /// never invented.
    ///
    /// ´claim:census:module-declarations-are-never-followed´
    /// ´test:unit:never-follows-module-declarations´
    #[test]
    fn never_follows_module_declarations() {
        let tests = scan("mod elsewhere;\n\n#[test]\nfn here() {}\n");

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].function(), "here");

        let root = TempDir::new().expect("a temporary root");
        let sources = root.path().join("package/src");
        std::fs::create_dir_all(&sources).expect("the source directory");
        std::fs::write(sources.join("tracked.rs"), "#[test]\nfn tracked() {}\n")
            .expect("the tracked source");
        std::fs::write(sources.join("untracked.rs"), "#[test]\nfn untracked() {}\n")
            .expect("the untracked source");

        crate::test_support::track_paths(
            root.path(),
            &[Path::new("package/src/tracked.rs").to_path_buf()],
        );

        let packages = [Package::new("torrust-demo", "package")];
        let corpus = CorpusPlan::compile(root.path(), UniverseKind::GitTracked, &[])
            .expect("fixture topology");
        let (census, findings) = take_census(root.path(), &packages, &corpus);
        let names: Vec<&str> = census
            .tests()
            .iter()
            .map(super::TestFunction::function)
            .collect();

        assert!(
            findings.is_empty(),
            "the tracked corpus reads cleanly: {findings:?}"
        );
        assert_eq!(census.files_scanned(), 1);
        assert_eq!(names, ["tracked"]);
    }

    /// A test's documentation is gathered as the lines its author wrote,
    /// knowing which line each occupies, which line the attributes begin on and
    /// where the function itself stands. That is everything a later pass needs
    /// to read a label off the last line or to write one above it.
    ///
    /// ´claim:census:documentation-is-gathered-as-lines-with-their-positions´
    /// ´test:unit:reads-line-oriented-documentation´
    #[test]
    fn reads_line_oriented_documentation() {
        let tests = scan("/// alpha\n/// beta\n#[test]\nfn covered() {}\n");
        let doc = tests[0].doc().expect("documentation");

        assert_eq!(doc.lines(), ["alpha", "beta"]);
        assert_eq!(doc.final_line(), Some("beta"));
        assert_eq!((doc.first_line(), doc.last_line()), (1, 2));
        assert!(doc.is_line_oriented());
        assert_eq!(tests[0].attribute_line(), 1);
        assert_eq!(tests[0].location().line(), 4);
        assert_eq!(tests[0].location().column(), 4);
    }

    /// Documentation written as one block comment yields the same lines, and is
    /// marked as not line-oriented: its lines are not separately editable, so a
    /// pass that rewrites documentation knows it cannot simply insert one.
    ///
    /// ´claim:census:block-documentation-yields-lines-but-is-marked-unsplittable´
    /// ´test:unit:reads-block-documentation-as-several-lines´
    #[test]
    fn reads_block_documentation_as_several_lines() {
        let tests = scan("/** alpha\nbeta */\n#[test]\nfn covered() {}\n");
        let doc = tests[0].doc().expect("documentation");

        assert_eq!(doc.lines(), ["alpha", "beta"]);
        assert!(
            !doc.is_line_oriented(),
            "a block comment spans several lines"
        );
    }

    /// An undocumented test records no documentation rather than an empty one,
    /// so the absence is distinguishable, and its attribute position is known
    /// regardless — which is where a first line would have to go.
    ///
    /// ´claim:census:an-undocumented-test-records-absence-and-still-knows-its-position´
    /// ´test:unit:records-no-documentation-when-there-is-none´
    #[test]
    fn records_no_documentation_when_there_is_none() {
        let tests = scan("#[test]\nfn covered() {}\n");

        assert!(tests[0].doc().is_none());
        assert_eq!(tests[0].attribute_line(), 1);
    }

    /// The indentation a test is written at is recorded with it, so a line
    /// inserted above it can be laid out to match rather than arriving flush
    /// against the margin inside a nested module.
    ///
    /// ´claim:census:a-tests-indentation-is-recorded-for-whatever-writes-above-it´
    /// ´test:unit:records-the-indentation-of-the-first-attribute´
    #[test]
    fn records_the_indentation_of_the_first_attribute() {
        let tests = scan("mod inner {\n    /// doc\n    #[test]\n    fn covered() {}\n}\n");

        assert_eq!(tests[0].indent(), "    ");
        assert_eq!(tests[0].attribute_line(), 2);
    }

    /// A source that will not parse is refused with the parser's own
    /// explanation rather than read as a file containing no tests, so a broken
    /// file shrinks no census silently.
    ///
    /// ´claim:census:an-unparsable-source-is-refused-with-an-explanation´
    /// ´test:unit:reports-a-source-it-cannot-parse´
    #[test]
    fn reports_a_source_it_cannot_parse() {
        let error = scan_source("torrust-demo", Path::new("src/demo.rs"), "fn broken( {")
            .expect_err("a parse failure");

        assert!(!error.is_empty(), "the parser explains itself");

        let root = TempDir::new().expect("a temporary root");
        let sources = root.path().join("package/src");
        std::fs::create_dir_all(&sources).expect("the source directory");
        std::fs::write(sources.join("present.rs"), "#[test]\nfn present() {}\n")
            .expect("the source");

        let packages = [Package::new("torrust-demo", "package")];
        let corpus = CorpusPlan::compile(root.path(), UniverseKind::GitTracked, &[])
            .expect("fixture topology");
        let (census, findings) = take_census(root.path(), &packages, &corpus);
        let root_display = root.path().display().to_string();

        assert!(census.tests().is_empty(), "a refused census covers nothing");
        assert_eq!(census.files_scanned(), 0);
        assert!(
            findings.is_empty(),
            "the analyzer receives an empty topology: {findings:?}"
        );
        assert!(
            matches!(
                corpus.findings(),
                [Finding::TraversalFailure { path, message }]
                    if path == &root_display && message.starts_with("git ls-files:")
            ),
            "the topology reports the listing failure once against the root: {:?}",
            corpus.findings()
        );
    }

    /// A line and column convert back to a byte offset in the source, counting
    /// characters rather than bytes, so a position taken from a line carrying
    /// text outside ASCII still lands where that text actually begins.
    ///
    /// ´claim:census:a-line-and-column-convert-back-to-the-right-byte-offset´
    /// ´test:unit:computes-byte-offsets-from-lines-and-columns´
    #[test]
    fn computes_byte_offsets_from_lines_and_columns() {
        let text = "alpha\nbéta\ngamma\n";
        let starts = line_starts(text);

        assert_eq!(byte_offset(text, &starts, 1, 0), 0);
        assert_eq!(byte_offset(text, &starts, 2, 0), 6);
        assert_eq!(byte_offset(text, &starts, 2, 2), 9);
        assert_eq!(byte_offset(text, &starts, 3, 0), 12);
    }
}
