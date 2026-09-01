// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The in-file test index of ADR-T-017: one generated table per test-carrying
//! Rust source, in that source's module documentation.
//!
//! The index is a projection of the two labels and of nothing else, so it is
//! written by the fix mode and compared byte for byte by the check. A hand-edit
//! inside it is indistinguishable from staleness and is treated as staleness:
//! the check reports it and the fix overwrites it, which is the ordinary
//! consequence of a generated region being derivative output.
//!
//! # What the cells are, and why
//!
//! The Test cell is a rustdoc intra-doc link to the test function, which does two
//! jobs at once. It is clickable in the generated documentation, so the index is
//! a navigation surface rather than a list of strings; and it keeps the derived
//! test label out of the cells, so ADR-T-015's misplaced-label rule never engages
//! against a table that is not a standard place. The Area and Claim cells are the
//! claim's area and the test's gloss, computed once beside the claim profile so
//! that a file's index and its folder's matrix can never say different things
//! about one test.
//!
//! # How the region is recognised
//!
//! The header row is the identity, matched by exact equality, which is the
//! discipline the kind registry's own table parsing already uses and the thing
//! that makes a column ruling enforceable at all: a table whose header is
//! nearly right is not a projection with a defect, it is not the projection. A
//! position is not an identity either, so a module doc gaining a paragraph
//! above the index keeps its index where it was — which is what the rejected
//! Ansatz named (ADR-T-017, The test documentation policy) settles.
//!
//! The corpus's eight hundred hand-written test tables are exactly the near-miss
//! case: they head the same section and rule different columns. They read as no
//! projection at all, which is what leaves them the migration's backlog rather
//! than the check's failures on the day this machinery lands.
//!
//! # What is staged
//!
//! A test-carrying source with no index at all is counted and reported nowhere:
//! the sweep that bootstraps the corpus's indexes is a later wave, and reporting
//! its backlog would bury the files it has reached. An index that exists is held
//! to exactness from the commit that wrote it.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`renders_the_records_displayed_index`] | projection | An in-file index renders as a titled table of three columns, one row per covered test, naming the test as a documentation link and carrying its area and the author's own gloss. The reader of the source meets the statements its tests establish before meeting the tests. |
//! | [`renders_a_citing_test_as_its_citation`] | projection | A citing test renders as the citation itself rather than as a copy of the statement, so a reader scanning the table sees at once that the statement is established elsewhere and exactly where — and the citation is a real one, in the code syntax of the surface it stands on, so the checker resolves it against the claim it names. The word naming the relation stays outside the parenthesis, which hugs the label alone. |
//! | [`renders_an_unclaimed_test_with_a_placeholder`] | projection | A test with no claim yet renders with an empty area and a placeholder rather than a blank cell, so a reader sees that the statement is unwritten rather than that the generator lost it. |
//! | [`escapes_a_pipe_in_a_gloss`] | projection | A gloss may say anything its author needs it to, mathematics included: a character that would otherwise end the cell early is escaped, which is the only change a projection makes to an author's words. |
//! | [`finds_a_committed_index_by_its_header_row`] | projection | A committed index is recognised by its header row rather than by where it sits, and its extent is recovered exactly, so an index can be found and replaced wherever an author has left it in the documentation. |
//! | [`reads_no_index_where_the_header_row_is_nearly_right`] | projection | Recognition is by exact equality: a header differing by a single letter is not the projection, so the generator never claims ownership of a table it did not write. |
//! | [`reads_the_corpus_hand_written_table_as_no_projection`] | projection | The corpus's own hand-written test tables rule different columns, so they are read as no projection at all: they are the migration's backlog rather than a defect, and no sweep silently overwrites one. |
//! | [`counts_a_source_with_no_index_rather_than_reporting_it`] | projection | A source carrying covered tests but no index yet is counted as unindexed and reported nowhere, so the projection can be adopted a package at a time instead of all at once. |
//! | [`reports_a_stale_index`] | projection | An index that exists must be exactly what the labels give: one naming a test that is not there is stale and reported, so a committed table cannot quietly describe a file that has moved on. |
//! | [`reports_a_hand_edit_inside_the_index_region`] | projection | cites (´claim:projection:a-hand-edited-projection-is-stale-and-is-regenerated´) |
//! | [`rewrites_a_stale_index_to_what_the_labels_say`] | projection | Rewriting puts back exactly what the labels give and nothing of what was there: the rows the tests derive appear, and a row naming a test that does not exist does not survive. |
//! | [`writes_an_index_below_existing_module_documentation`] | projection | Bootstrapping an index into a documented module puts it below the documentation already there, separated from it, so the prose an author wrote about the module keeps its place at the top. |
//! | [`writes_an_index_into_a_source_with_no_documentation`] | projection | A source with no module documentation at all gains the index at its top, with a blank line between the index and the code, so the result is a well-formed source rather than a table pressed against a function. |
//! | [`writes_nothing_over_a_current_index`] | projection | cites (´claim:projection:projection-settles-after-one-pass´) |
//! | [`writes_an_index_after_a_license_header`] | projection | A licence header stays the first thing in the file and the index follows it, so the legal notice tooling expects at the top is never displaced, and the placement settles on the first pass. |
//! | [`rewrites_a_misplaced_index_after_a_license_header`] | projection | An index left above a licence header by an earlier bootstrap is moved below it, and the repositioned source then settles: rewriting repairs the misplacement rather than perpetuating it. |
//! | [`keeps_the_index_first_when_the_source_opens_with_an_attribute`] | projection | It is the licence header and only the licence header that the index yields to: a source opening with an inner attribute and no header still takes the index first. |

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::claim::{TABLE_DELIMITER, TABLE_HEADER, project_cells};
use crate::finding::{Finding, Location};
use crate::occurrence::Syntax;
use crate::profile::CoveredAsset;
use crate::subscribe::Subscription;

/// The heading that identifies the generated region, without its leader.
///
/// The record displays the index it fixes, and this heading opens it: the
/// module documentation of every Rust source carrying a covered test holds one
/// generated index, and the region is found by this line rather than by a
/// position (´[ORCHESTRATION-conv:testdocs:file-index]´).
///
/// ´const:indexlinter:test-index-region-heading´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:indexlinter:test-index-region-heading-text-x233620ee´
pub const INDEX_HEADING: &str = "# Test index";

/// The leader a module documentation line carries.
///
/// The index lives in module documentation rather than in an outer comment,
/// because what it indexes is the file (´[ORCHESTRATION-conv:testdocs:file-index]´), and
/// the inner form is the one Rust gives a module for documenting itself. The
/// leader is that form's own spelling.
///
/// ´const:indexlinter:module-documentation-leader´ (´[ORCHESTRATION-alg:const:text]´)
/// ´const:indexlinter:module-documentation-leader-text-xe7152d66´
const MODULE_DOC: &str = "//!";

/// True when a line stands inside a leading licence header region.
///
/// The question is the comment-leader catalog's rather than this module's. This
/// generator was the first caller to need it — it has to know where a header
/// ends before it may write below one — and its two exclusions, the module
/// documentation line and the outer documentation line, are the catalog's Rust
/// row as it now stands. Asking the catalog rather than restating the rule is
/// what keeps one answer to the question of where a header ends: a generator
/// that wrote below a line the header policy did not count as a header would put
/// the index somewhere the policy cannot see.
fn is_ordinary_comment(line: &str) -> bool {
    crate::leader::rust().opens_region(line.as_bytes())
}

/// Where a leading license header ends, blank separator line included.
///
/// A source that opens with a run of ordinary comments — an SPDX header,
/// typically — keeps that run first no matter what the index generator does:
/// this is the line one past the run and the blank line that follows it. A
/// source that opens with anything else (a module doc, an attribute, code)
/// has no such header, and this is zero.
fn license_header_end(lines: &[&str]) -> usize {
    let mut end = 0;

    while lines.get(end).is_some_and(|line| is_ordinary_comment(line)) {
        end += 1;
    }

    if end == 0 {
        return 0;
    }

    while lines.get(end).is_some_and(|line| line.trim().is_empty()) {
        end += 1;
    }

    end
}

/// True when a committed index stands at the very top of the file, directly
/// above a license header: the placement bug this module fixes. A committed
/// index anywhere else — including one a license header already correctly
/// precedes — is left exactly where it is, per the position-is-not-an-identity
/// rule `committed_index` documents.
fn misplaced_before_license(lines: &[&str], committed: &CommittedIndex) -> bool {
    if committed.first_line != 0 {
        return false;
    }

    let mut after = committed.past_last_line;

    if lines.get(after).is_some_and(|line| line.trim().is_empty()) {
        after += 1;
    }

    lines
        .get(after)
        .is_some_and(|line| is_ordinary_comment(line))
}

/// The committed index region of one Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedIndex {
    /// The zero-based line the heading stands on.
    pub first_line: usize,
    /// The zero-based line one past the region's last.
    pub past_last_line: usize,
    /// The region's lines, leaders and indentation included.
    pub lines: Vec<String>,
    /// The leading whitespace the region's lines carry.
    pub indent: String,
}

/// Find the committed index region of a Rust source, when it carries one.
///
/// The header row is the identity, matched by exact equality. That is the
/// discipline the kind registry's own table parsing already uses, and it is what
/// makes a column ruling enforceable at all: a table whose header is nearly right
/// is not a projection with a defect, it is not the projection. The corpus's
/// hand-written test tables are exactly that case — they head the same section
/// and rule different columns — so they read as no projection here and stand as
/// the migration's backlog rather than as the check's failures.
///
/// The region runs from the heading above the header, when the heading is there,
/// through the table's rows. A table whose header is exactly right but which has
/// lost its heading is a projection with a defect: the region opens at the header,
/// the regeneration puts the heading back, and the difference is the staleness the
/// check reports and the fix repairs.
#[must_use]
pub fn committed_index(text: &str) -> Option<CommittedIndex> {
    let lines: Vec<&str> = text.split('\n').collect();

    let header = lines
        .iter()
        .position(|line| doc_content(line).is_some_and(|content| content == TABLE_HEADER))?;

    let indent = lines[header]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect();

    let heading_above = header >= 2
        && doc_content(lines[header - 1]).is_some_and(str::is_empty)
        && doc_content(lines[header - 2]).is_some_and(|content| content == INDEX_HEADING);

    let first_line = if heading_above { header - 2 } else { header };

    let mut past_last_line = header + 1;

    while lines
        .get(past_last_line)
        .and_then(|line| doc_content(line))
        .is_some_and(|content| content.starts_with('|'))
    {
        past_last_line += 1;
    }

    Some(CommittedIndex {
        first_line,
        past_last_line,
        lines: lines[first_line..past_last_line]
            .iter()
            .map(|line| (*line).to_owned())
            .collect(),
        indent,
    })
}

/// The content of a module documentation line, once its leader is resolved away.
fn doc_content(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(MODULE_DOC)?;

    // A doc-comment leader is followed by one space or by nothing at all; a
    // leader immediately followed by another slash is an ordinary comment.
    if rest.starts_with('/') {
        return None;
    }

    Some(rest.trim())
}

/// Render the index region one source's covered tests give it.
///
/// The recipe is fixed and normalising, so a second write over an unchanged
/// corpus reproduces the same bytes and changes nothing.
#[must_use]
pub fn region(assets: &[&CoveredAsset], indent: &str) -> Vec<String> {
    let mut lines = vec![
        format!("{indent}{MODULE_DOC} {INDEX_HEADING}"),
        format!("{indent}{MODULE_DOC}"),
        format!("{indent}{MODULE_DOC} {TABLE_HEADER}"),
        format!("{indent}{MODULE_DOC} {TABLE_DELIMITER}"),
    ];

    for asset in assets {
        let cells = project_cells(asset, Syntax::Code);
        let mut row = String::new();
        let _ignored = write!(
            row,
            "{indent}{MODULE_DOC} | [`{}`] | {} | {} |",
            asset.test().function(),
            cells.area,
            cells.claim
        );

        lines.push(row);
    }

    lines
}

/// What the in-file index pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexAnalysis {
    /// How many Rust sources carry a covered test, and so want an index.
    pub sources_covered: usize,
    /// How many of them carry one.
    pub indexed: usize,
    /// How many carry none, which the bootstrap sweep will write.
    pub unindexed: usize,
    /// How many carry one that is not what their labels say it is.
    pub stale: usize,
}

/// Group a workspace's covered assets by the source they stand in.
///
/// Census order is preserved, so the rows of a file's index stand in the order
/// its tests do and a reader comparing table with file reads down both together.
#[must_use]
pub fn by_source(assets: &[CoveredAsset]) -> BTreeMap<PathBuf, Vec<&CoveredAsset>> {
    let mut sources: BTreeMap<PathBuf, Vec<&CoveredAsset>> = BTreeMap::new();

    for asset in assets {
        sources
            .entry(asset.test().path().to_path_buf())
            .or_default()
            .push(asset);
    }

    sources
}

/// Verify every committed index of a workspace against its source's labels.
///
/// The walk is over the sources carrying a covered test, and the subscription
/// is consulted per source: a source standing in an unsubscribed owner's share
/// is not censused at all, so nothing there is counted as wanting an index and
/// nothing there is read for one.
#[must_use]
pub fn verify_indexes(
    root: &Path,
    assets: &[CoveredAsset],
    subscription: &Subscription<'_>,
) -> (IndexAnalysis, Vec<Finding>) {
    let mut analysis = IndexAnalysis::default();
    let mut findings = Vec::new();

    for (path, sources) in by_source(assets) {
        if !subscription.governs(&path) {
            continue;
        }

        analysis.sources_covered += 1;

        let Ok(text) = fs::read_to_string(root.join(&path)) else {
            findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: "the source carrying a covered test could not be read".to_owned(),
            });
            continue;
        };

        let Some(committed) = committed_index(&text) else {
            analysis.unindexed += 1;
            continue;
        };

        analysis.indexed += 1;

        let wanted = region(&sources, &committed.indent);
        let line_refs: Vec<&str> = text.split('\n').collect();
        let misplaced = misplaced_before_license(&line_refs, &committed);

        if committed.lines != wanted || misplaced {
            analysis.stale += 1;
            findings.push(Finding::StaleTestIndex {
                path: path.to_string_lossy().into_owned(),
                expected: wanted.len().saturating_sub(4),
                found: committed.lines.len().saturating_sub(4),
                location: line_location(&path, &text, committed.first_line),
            });
        }
    }

    (analysis, findings)
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

/// Rewrite one source's index region, or write one where there is none.
///
/// Returns the new text when it differs from the old, and nothing when the
/// committed index is already what the labels say it is: a run over an unchanged
/// corpus leaves the working tree exactly as it found it.
#[must_use]
pub fn write_index(text: &str, assets: &[&CoveredAsset]) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();

    if let Some(committed) = committed_index(text) {
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();

        if misplaced_before_license(&line_refs, &committed) {
            // The index sits above a license header that belongs first. Lift
            // it out, then reinsert it right after the header, where a fresh
            // bootstrap would have put it.
            let mut after = committed.past_last_line;

            if lines.get(after).is_some_and(|line| line.trim().is_empty()) {
                after += 1;
            }

            let _removed: Vec<String> = lines.splice(0..after, std::iter::empty()).collect();

            let remaining_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let at = license_header_end(&remaining_refs);

            let mut written = region(assets, &committed.indent);
            written.push(String::new());

            let _inserted: Vec<String> = lines.splice(at..at, written).collect();

            return Some(lines.join("\n"));
        }

        let wanted = region(assets, &committed.indent);

        if committed.lines == wanted {
            return None;
        }

        let range = committed.first_line..committed.past_last_line;
        let _replaced: Vec<String> = lines.splice(range, wanted).collect();

        return Some(lines.join("\n"));
    }

    let (at, written) = bootstrap(&lines, assets);
    let _inserted: Vec<String> = lines.splice(at..at, written).collect();

    Some(lines.join("\n"))
}

/// Where a source with no index gains one, and what it gains.
///
/// A source already carrying module documentation gains the index at the end of
/// it, below a blank documentation line; a source carrying none gains the index
/// right after any leading license header (so the header stays first), or at
/// the very top when there is no such header, followed by a blank line. Either
/// way the index stands in the module documentation, which is where the
/// convention puts it.
fn bootstrap(lines: &[String], assets: &[&CoveredAsset]) -> (usize, Vec<String>) {
    let doc_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .take_while(|(_index, line)| doc_content(line).is_some() || line.trim().is_empty())
        .filter(|(_index, line)| doc_content(line).is_some())
        .map(|(index, _line)| index)
        .collect();

    doc_lines.last().map_or_else(
        || {
            // A source with no module documentation gains the index after any
            // leading license header, separated from what follows by one
            // blank line. A source with no header gains it at line zero,
            // unchanged from before.
            let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let at = license_header_end(&line_refs);

            let mut written = region(assets, "");

            written.push(String::new());

            (at, written)
        },
        |last| {
            let indent: String = lines[*last]
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect();
            let mut written = vec![format!("{indent}{MODULE_DOC}")];

            written.extend(region(assets, &indent));

            (last + 1, written)
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{committed_index, region, verify_indexes, write_index};
    use crate::census::{Census, scan_source};
    use crate::finding::Finding;
    use crate::profile::{CoveredAsset, cover};
    use crate::subscribe::Subscription;
    use crate::workspace::Package;

    /// The acute the code syntax delimits an occurrence with.
    const ACUTE: char = '\u{b4}';

    fn assets_of(text: &str) -> Vec<CoveredAsset> {
        let packages = vec![Package::new("torrust-demo", "packages/demo")];
        let tests = scan_source(
            "torrust-demo",
            Path::new("packages/demo/src/tests/demo.rs"),
            text,
        )
        .expect("a Rust source");
        let (assets, findings) = cover(&packages, &Census::from_tests(tests, 1));

        assert!(
            findings.is_empty(),
            "the fixture covers cleanly: {findings:?}"
        );

        assets
    }

    fn rendered(text: &str) -> Vec<String> {
        let assets = assets_of(text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        region(&borrowed, "")
    }

    /// One test minting a claim, with a gloss.
    fn minting() -> String {
        format!(
            "/// The widths are identical across the sweep.\n\
             ///\n\
             /// {ACUTE}claim:resonance:crossover-widths{ACUTE}\n\
             /// {ACUTE}test:crate:widths-are-identical{ACUTE}\n\
             #[test]\nfn widths_are_identical() {{}}\n"
        )
    }

    /// An in-file index renders as a titled table of three columns, one row per
    /// covered test, naming the test as a documentation link and carrying its
    /// area and the author's own gloss. The reader of the source meets the
    /// statements its tests establish before meeting the tests.
    ///
    /// ´claim:projection:an-in-file-index-renders-as-a-titled-three-column-table´
    /// ´test:unit:renders-the-records-displayed-index´
    #[test]
    fn renders_the_records_displayed_index() {
        let lines = rendered(&minting());

        assert_eq!(
            lines,
            [
                "//! # Test index",
                "//!",
                "//! | Test | Area | Claim |",
                "//! |------|------|-------|",
                "//! | [`widths_are_identical`] | resonance | The widths are identical across the sweep. |",
            ]
        );
    }

    /// A citing test renders as the citation itself rather than as a copy of
    /// the statement, so a reader scanning the table sees at once that the
    /// statement is established elsewhere and exactly where — and the citation
    /// is a real one, in the code syntax of the surface it stands on, so the
    /// checker resolves it against the claim it names. The word naming the
    /// relation stays outside the parenthesis, which hugs the label alone.
    ///
    /// ´claim:projection:a-citing-test-renders-as-its-citation´
    /// ´test:unit:renders-a-citing-test-as-its-citation´
    #[test]
    fn renders_a_citing_test_as_its_citation() {
        let lines = rendered(&format!(
            "{}\n/// ({ACUTE}claim:resonance:crossover-widths{ACUTE})\n\
             /// {ACUTE}test:crate:widths-survive{ACUTE}\n#[test]\nfn widths_survive() {{}}\n",
            minting()
        ));

        assert_eq!(
            lines[5],
            format!(
                "//! | [`widths_survive`] | resonance | cites ({ACUTE}claim:resonance:crossover-widths{ACUTE}) |"
            )
        );
    }

    /// A test with no claim yet renders with an empty area and a placeholder
    /// rather than a blank cell, so a reader sees that the statement is
    /// unwritten rather than that the generator lost it.
    ///
    /// ´claim:projection:an-unclaimed-test-renders-with-a-placeholder´
    /// ´test:unit:renders-an-unclaimed-test-with-a-placeholder´
    #[test]
    fn renders_an_unclaimed_test_with_a_placeholder() {
        let lines = rendered(&format!(
            "/// {ACUTE}test:crate:not-claimed-yet{ACUTE}\n#[test]\nfn not_claimed_yet() {{}}\n"
        ));

        assert_eq!(lines[4], "//! | [`not_claimed_yet`] |  | \u{2014} |");
    }

    /// A gloss may say anything its author needs it to, mathematics included: a
    /// character that would otherwise end the cell early is escaped, which is
    /// the only change a projection makes to an author's words.
    ///
    /// ´claim:projection:a-gloss-is-escaped-so-an-authors-words-cannot-break-the-cell´
    /// ´test:unit:escapes-a-pipe-in-a-gloss´
    #[test]
    fn escapes_a_pipe_in_a_gloss() {
        let lines = rendered(&format!(
            "/// The norm |x| is preserved.\n///\n/// {ACUTE}claim:norms:preserved{ACUTE}\n\
             /// {ACUTE}test:crate:norm-is-preserved{ACUTE}\n#[test]\nfn norm_is_preserved() {{}}\n"
        ));

        assert_eq!(
            lines[4],
            "//! | [`norm_is_preserved`] | norms | The norm \\|x\\| is preserved. |"
        );
    }

    /// A committed index is recognised by its header row rather than by where
    /// it sits, and its extent is recovered exactly, so an index can be found
    /// and replaced wherever an author has left it in the documentation.
    ///
    /// ´claim:projection:a-committed-index-is-recognised-by-its-header-not-its-position´
    /// ´test:unit:finds-a-committed-index-by-its-header-row´
    #[test]
    fn finds_a_committed_index_by_its_header_row() {
        let text = concat!(
            "//! A module.\n",
            "//!\n",
            "//! # Test index\n",
            "//!\n",
            "//! | Test | Area | Claim |\n",
            "//! |------|------|-------|\n",
            "//! | [`one`] |  | \u{2014} |\n",
            "\n",
            "fn main() {}\n",
        );

        let found = committed_index(text).expect("an index");

        assert_eq!(found.first_line, 2);
        assert_eq!(found.past_last_line, 7);
        assert_eq!(found.lines.len(), 5);
    }

    /// Recognition is by exact equality: a header differing by a single letter
    /// is not the projection, so the generator never claims ownership of a
    /// table it did not write.
    ///
    /// ´claim:projection:a-nearly-right-header-is-not-the-projection´
    /// ´test:unit:reads-no-index-where-the-header-row-is-nearly-right´
    #[test]
    fn reads_no_index_where_the_header_row_is_nearly_right() {
        assert!(
            committed_index("//! # Test index\n//!\n//! | Test | Area | Claims |\n").is_none(),
            "a header that is nearly right is not the projection"
        );
    }

    /// The corpus's own hand-written test tables rule different columns, so
    /// they are read as no projection at all: they are the migration's backlog
    /// rather than a defect, and no sweep silently overwrites one.
    ///
    /// ´claim:projection:a-hand-written-table-of-other-columns-is-no-projection´
    /// ´test:unit:reads-the-corpus-hand-written-table-as-no-projection´
    #[test]
    fn reads_the_corpus_hand_written_table_as_no_projection() {
        let hand_written = concat!(
            "    //! # Test index\n",
            "    //!\n",
            "    //! | Test | Focus |\n",
            "    //! |------|-------|\n",
            "    //! | [`cusum_accumulates_positive_residuals`] | grows with positive residuals |\n",
        );

        assert!(
            committed_index(hand_written).is_none(),
            "the corpus's own tables rule different columns, so they are the backlog and not a defect"
        );
    }

    /// Write one source into a temporary root and verify its index.
    fn verify(text: &str) -> (super::IndexAnalysis, Vec<Finding>) {
        let root = tempfile::tempdir().expect("temporary directory");
        let relative = Path::new("packages/demo/src/tests/demo.rs");

        std::fs::create_dir_all(root.path().join("packages/demo/src/tests")).expect("create");
        std::fs::write(root.path().join(relative), text).expect("write");

        verify_indexes(
            root.path(),
            &assets_of(text),
            &Subscription::fictional_all(),
        )
    }

    /// A source carrying covered tests but no index yet is counted as
    /// unindexed and reported nowhere, so the projection can be adopted a
    /// package at a time instead of all at once.
    ///
    /// ´claim:projection:a-source-with-no-index-yet-is-counted-not-reported´
    /// ´test:unit:counts-a-source-with-no-index-rather-than-reporting-it´
    #[test]
    fn counts_a_source_with_no_index_rather_than_reporting_it() {
        let (analysis, findings) = verify(&minting());

        assert_eq!(analysis.sources_covered, 1);
        assert_eq!(analysis.unindexed, 1);
        assert_eq!(analysis.indexed, 0);
        assert!(
            findings.is_empty(),
            "the bootstrap sweep is a later wave: {findings:?}"
        );
    }

    /// An index that exists must be exactly what the labels give: one naming a
    /// test that is not there is stale and reported, so a committed table
    /// cannot quietly describe a file that has moved on.
    ///
    /// ´claim:projection:an-index-that-is-not-what-the-labels-give-is-stale´
    /// ´test:unit:reports-a-stale-index´
    #[test]
    fn reports_a_stale_index() {
        let text = format!(
            "//! # Test index\n//!\n//! | Test | Area | Claim |\n//! |------|------|-------|\n\
             //! | [`gone`] | resonance | A test that is not there. |\n\n{}",
            minting()
        );
        let (analysis, findings) = verify(&text);

        assert_eq!(analysis.indexed, 1);
        assert_eq!(analysis.stale, 1);
        assert!(
            matches!(findings.as_slice(), [Finding::StaleTestIndex { .. }]),
            "expected one stale index, got {findings:?}"
        );
    }

    /// Editing a cell by hand inside the index region is staleness by another
    /// name, and is reported as such: the table is owned by the labels rather
    /// than by whoever typed in it last.
    ///
    /// (´claim:projection:a-hand-edited-projection-is-stale-and-is-regenerated´)
    /// ´test:unit:reports-a-hand-edit-inside-the-index-region´
    #[test]
    fn reports_a_hand_edit_inside_the_index_region() {
        let assets = assets_of(&minting());
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();
        let current = region(&borrowed, "").join("\n");
        let edited = current.replace("resonance", "decay");
        let text = format!("{edited}\n\n{}", minting());

        let (analysis, findings) = verify(&text);

        assert_eq!(
            analysis.stale, 1,
            "a hand-edit is staleness by another name"
        );
        assert!(matches!(
            findings.as_slice(),
            [Finding::StaleTestIndex { .. }]
        ));
    }

    /// Rewriting puts back exactly what the labels give and nothing of what was
    /// there: the rows the tests derive appear, and a row naming a test that
    /// does not exist does not survive.
    ///
    /// ´claim:projection:a-rewrite-puts-back-exactly-what-the-labels-give´
    /// ´test:unit:rewrites-a-stale-index-to-what-the-labels-say´
    #[test]
    fn rewrites_a_stale_index_to_what_the_labels_say() {
        let text = format!(
            "//! # Test index\n//!\n//! | Test | Area | Claim |\n//! |------|------|-------|\n\
             //! | [`gone`] | decay | Wrong. |\n\n{}",
            minting()
        );
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&text, &borrowed).expect("a rewrite");

        assert!(
            rewritten.contains("//! | [`widths_are_identical`] | resonance | The widths are identical across the sweep. |"),
            "the labels decide the rows: {rewritten}"
        );
        assert!(
            !rewritten.contains("[`gone`]"),
            "and the hand-edit does not survive"
        );
    }

    /// Bootstrapping an index into a documented module puts it below the
    /// documentation already there, separated from it, so the prose an author
    /// wrote about the module keeps its place at the top.
    ///
    /// ´claim:projection:a-bootstrapped-index-goes-below-existing-module-documentation´
    /// ´test:unit:writes-an-index-below-existing-module-documentation´
    #[test]
    fn writes_an_index_below_existing_module_documentation() {
        let text = format!("//! A module.\n\n{}", minting());
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&text, &borrowed).expect("a write");

        assert!(
            rewritten.starts_with("//! A module.\n//!\n//! # Test index\n"),
            "{rewritten}"
        );
    }

    /// A source with no module documentation at all gains the index at its top,
    /// with a blank line between the index and the code, so the result is a
    /// well-formed source rather than a table pressed against a function.
    ///
    /// ´claim:projection:an-undocumented-source-gains-the-index-at-its-top´
    /// ´test:unit:writes-an-index-into-a-source-with-no-documentation´
    #[test]
    fn writes_an_index_into_a_source_with_no_documentation() {
        let text = minting();
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&text, &borrowed).expect("a write");

        assert!(rewritten.starts_with("//! # Test index\n"), "{rewritten}");
        assert!(
            rewritten.contains("\n\n/// The widths"),
            "the source follows a blank line"
        );
    }

    /// A source whose index is already current is not rewritten: the second
    /// pass reports that there is nothing to do, so regenerating produces no
    /// churn in a tree that is already correct.
    ///
    /// (´claim:projection:projection-settles-after-one-pass´)
    /// ´test:unit:writes-nothing-over-a-current-index´
    #[test]
    fn writes_nothing_over_a_current_index() {
        let text = minting();
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let once = write_index(&text, &borrowed).expect("a write");
        let again = assets_of(&once);
        let borrowed_again: Vec<&CoveredAsset> = again.iter().collect();

        assert_eq!(
            write_index(&once, &borrowed_again),
            None,
            "a second run changes nothing"
        );
    }

    /// The two lines an SPDX header takes in this corpus, followed by the
    /// blank line that separates it from what follows.
    fn license_header() -> &'static str {
        "// SPDX-License-Identifier: AGPL-3.0-only\n\
         // SPDX-FileCopyrightText: 2026 Wild Sky Maker\n\n"
    }

    /// A licence header stays the first thing in the file and the index follows
    /// it, so the legal notice tooling expects at the top is never displaced,
    /// and the placement settles on the first pass.
    ///
    /// ´claim:projection:a-licence-header-stays-first-and-the-index-follows-it´
    /// ´test:unit:writes-an-index-after-a-license-header´
    #[test]
    fn writes_an_index_after_a_license_header() {
        let text = format!("{}{}", license_header(), minting());
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&text, &borrowed).expect("a write");

        assert!(
            rewritten.starts_with(&format!("{}//! # Test index\n", license_header())),
            "the license header stays first, the index follows it: {rewritten}"
        );

        let again = assets_of(&rewritten);
        let borrowed_again: Vec<&CoveredAsset> = again.iter().collect();

        assert_eq!(
            write_index(&rewritten, &borrowed_again),
            None,
            "a second run changes nothing"
        );
    }

    /// An index left above a licence header by an earlier bootstrap is moved
    /// below it, and the repositioned source then settles: rewriting repairs
    /// the misplacement rather than perpetuating it.
    ///
    /// ´claim:projection:an-index-above-a-licence-header-is-moved-below-it´
    /// ´test:unit:rewrites-a-misplaced-index-after-a-license-header´
    #[test]
    fn rewrites_a_misplaced_index_after_a_license_header() {
        let placeholder = assets_of(&minting());
        let placeholder_borrowed: Vec<&CoveredAsset> = placeholder.iter().collect();
        let index = region(&placeholder_borrowed, "").join("\n");

        // The shape a source lands in when the old bootstrap wrote the index
        // above the license header instead of below it.
        let broken = format!("{index}\n\n{}{}", license_header(), minting());

        let assets = assets_of(&broken);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&broken, &borrowed).expect("a reposition");

        assert!(
            rewritten.starts_with(&format!("{}//! # Test index\n", license_header())),
            "the misplaced index moves below the license header: {rewritten}"
        );

        let again = assets_of(&rewritten);
        let borrowed_again: Vec<&CoveredAsset> = again.iter().collect();

        assert_eq!(
            write_index(&rewritten, &borrowed_again),
            None,
            "the repositioned index settles: a second run changes nothing"
        );
    }

    /// It is the licence header and only the licence header that the index
    /// yields to: a source opening with an inner attribute and no header still
    /// takes the index first.
    ///
    /// ´claim:projection:only-a-licence-header-displaces-the-index-from-the-top´
    /// ´test:unit:keeps-the-index-first-when-the-source-opens-with-an-attribute´
    #[test]
    fn keeps_the_index_first_when_the_source_opens_with_an_attribute() {
        let text = format!("#![allow(dead_code)]\n\n{}", minting());
        let assets = assets_of(&text);
        let borrowed: Vec<&CoveredAsset> = assets.iter().collect();

        let rewritten = write_index(&text, &borrowed).expect("a write");

        assert!(
            rewritten.starts_with("//! # Test index\n"),
            "no license header precedes the attribute, so the index still goes first: {rewritten}"
        );
    }
}
