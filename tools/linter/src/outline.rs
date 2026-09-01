// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Outline tracking: an outline document and the document it tracks.
//!
//! An outline is a document that says, entry by labeled entry, what another
//! document must contain. The layer documents and the record register of the
//! Assayer's campaign are outlines in exactly this sense, and the point of them
//! is that they can be trusted — which they cannot be while nothing checks that
//! the outline and its document still agree. This module is that check, and it
//! runs both ways: an entry claiming a head the tracked document does not carry
//! is drift, and a head the tracked document carries that no entry claims is
//! drift too. Either way the report carries both ends, because a reader repairing
//! drift has to see the outline and the document at once.
//!
//! # The declaration
//!
//! The relation is declared, never inferred. Inferring it would mean guessing
//! which of an entry's citations is the head it tracks, and a guess is exactly
//! what an outline exists to remove. An outline therefore carries a table, in the
//! idiom this corpus already reads adoption data in — the registry of ADR-T-011
//! is read from that document's own tables in the same way — under a heading of
//! its own:
//!
//! ```text
//! **Convention (Tracking)** · `conv:layers:one-tracking`
//!
//! | Entry | Head | Document |
//! | --- | --- | --- |
//! | ``entry:example:commitments`` | ``sec:guide:commitments`` | ``handbook/guide.md`` |
//! ```
//!
//! A table is a tracking table exactly when its header names a Head column and a
//! Document column. The Head cell names the label the tracked document's head
//! must mint, and the Document cell names that document by its path from the
//! repository root, which is the path the carrier reads and every finding prints.
//! Those two are the relation; everything else a header names is the outline's
//! own business, carried past untouched.
//!
//! An Entry column, when the header names one, adds an assertion beside the
//! relation rather than a part of it: the cell names a label the outline itself
//! mints, the entry environment doing the tracking, and the check holds the
//! outline to minting it. The column is optional because an outline may be
//! written in either of two registers and both are honest. An outline whose
//! entries are few and individually argued mints one label each and says so; an
//! outline whose entries run to the hundreds — a rewrite's chapter-by-chapter
//! contract, where a row is a line of a table and not a passage — would have to
//! mint a label per row to name them, doubling the corpus to give a name to
//! something no reader ever cites. Where the Entry column is absent the row is
//! identified the way a table row is otherwise identified, by where it stands,
//! and every finding cites the row's own file and line. Positional identity is
//! weaker than a label and that is the trade: what it buys is that the outline
//! stays the size of the thing it outlines.
//!
//! What does not vary is that the relation is declared. Reading the Document
//! from a heading above the table, or the Head from whichever citation of a row
//! looked most like a target, would put the checker back to guessing — and a
//! guess is exactly what an outline exists to remove. Both cells stand in the
//! row, in every form.
//!
//! The relation's cells are written as displayed double-backtick spans, and
//! that is not decoration. A single-backtick span participates: written that
//! way, the Entry cell would mint a second time and the Head cell would mint a
//! label belonging to another document. Those cells display labels as data
//! rather than using them, so the display syntax of the participation judgment
//! (ADR-T-014, A calculus of documentation and source labels) is the correct one, and one of them
//! written to participate is reported rather than read past — the duplicate
//! mint it would otherwise cause says nothing about tracking.
//!
//! The rule reaches those cells and no others. A column the outline keeps for its
//! own purposes — what an environment is for, what force it carries, what the
//! rewrite must resolve there — is ordinary prose that happens to sit in a table,
//! and ordinary prose cites. Holding a scope note to the display syntax would
//! forbid it the citations the surrounding paragraphs are made of, for a reason
//! that was never about it: such a cell mints nothing and claims nothing, so a
//! label written in it resolves exactly as it would one line above the table.
//!
//! Naming the document per row rather than once per outline is what lets one
//! outline track many documents, which the campaign's record register needs: the
//! register tracks a whole set, and the set is the point. An outline tracking one
//! document repeats one path, which is a small price for one rule instead of two.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_a_tracking_table`] | outline | A tracking table declares, row by row, which entry claims which head in which document, and all three are recovered from the row. An outline can therefore say what a document is supposed to contain. |
//! | [`reads_no_tracking_from_an_ordinary_table`] | outline | The column names are what make a table a declaration: an ordinary table of other columns declares no tracking and raises nothing, so a document may tabulate whatever it likes without being held to an outline. |
//! | [`reports_a_row_that_is_not_a_tracking`] | outline | Once a table declares tracking, every row in it must be one: a row missing a cell, one whose head is not a label, and one whose document is blank are each reported, and none of them is read as a tracking. |
//! | [`reports_a_cell_written_to_participate`] | outline | A tracking cell displays a label rather than citing it: a cell written to participate is reported, though the row is still read, because an outline that cited every head it tracked would make the whole corpus depend on the outline. |
//! | [`tracks_a_fixture_pair_both_ways`] | outline | An outline and the document it tracks agree when every declared row is fulfilled by a head and every head is claimed by a row. The relation is checked in both directions at once, and agreement raises nothing. |
//! | [`reports_a_head_no_entry_claims`] | outline | A head no row claims is reported at both places at once — where the head stands and where the outline omits it — so the reader is shown the document that grew and the outline that did not. |
//! | [`reports_an_entry_no_head_fulfils`] | outline | A row no head fulfils is reported the other way round, naming the head promised, the document that should carry it, and the row that promised it. An outline may not describe a document that does not exist. |
//! | [`reports_a_head_two_entries_claim`] | outline | A head is claimed by exactly one row: two rows claiming it is reported with both claims in the order they were written, so the tracking stays a relation a reader can follow one way. |
//! | [`reports_an_entry_the_outline_never_mints`] | outline | A row naming an entry the outline never mints is reported: an outline's rows point at its own entries, so a row cannot claim on behalf of something that does not exist in the outline. |
//! | [`reports_a_document_the_carrier_never_read`] | outline | A row naming a document the carrier never read is reported as untrackable, so an outline cannot quietly track a file that was renamed, deleted, or never written. |
//! | [`reads_a_tracking_table_that_names_no_entry`] | outline | An outline may track without minting an entry for each row: where there is no entry column, a row is identified by where it stands. A document can be tracked before its outline has been written up as entries. |
//! | [`reads_a_tracking_table_carrying_columns_of_its_own`] | outline | The columns a tracking table needs may stand anywhere among columns of the outline's own, in any order and any number, so an outline can carry whatever else it wants to record beside the tracking. |
//! | [`reads_no_tracking_without_a_document_column`] | outline | Both the head column and the document column are needed to declare tracking: a head column alone declares none, because a head with no document named is not a claim about any document. |
//! | [`leaves_a_citation_in_a_column_of_the_outline_s_own`] | outline | The display rule reaches the tracking cells and no further: a scope note in a column of the outline's own cites exactly as the prose around it does, so an outline can explain its entries in ordinary language. |
//! | [`tracks_a_positional_fixture_pair_both_ways`] | outline | cites (´claim:outline:an-outline-and-its-document-agree-in-both-directions´) |
//! | [`reports_a_head_no_positional_row_claims`] | outline | cites (´claim:outline:a-head-no-row-claims-is-reported-at-both-places´) |
//! | [`reports_a_positional_row_no_head_fulfils`] | outline | cites (´claim:outline:a-row-no-head-fulfils-is-reported-at-both-places´) |
//! | [`reads_an_outline_that_declares_nothing`] | outline | A document declaring no tracking at all is read as declaring none, with nothing reported, so outline tracking is something a document opts into by writing a table rather than something every document owes. |

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::finding::{Finding, Location};
use crate::head::Head;
use crate::label::Label;

/// The header cell naming the column that carries the claimed head.
///
/// The three cells are what makes a table a tracking declaration: this tool
/// reads a table as one exactly when its header cells are Entry, Head and
/// Document (´[EMBER-conv:migration:tracking-columns]´). The spelling is
/// therefore a recognition contract rather than a caption — a table naming its
/// columns otherwise is not a tracking table with a defect, it is not one.
///
/// ´const:emberlinter:tracking-claimed-head-column´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:tracking-claimed-head-column-text-x6108cce3´
const HEAD_COLUMN: &str = "Head";

/// The header cell naming the column that carries the tracked document.
///
/// The second of the three cells of the recognition contract
/// (´[EMBER-conv:migration:tracking-columns]´).
///
/// ´const:emberlinter:tracking-document-column´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:tracking-document-column-text-x1471a974´
const DOCUMENT_COLUMN: &str = "Document";

/// The header cell naming the optional column that carries the outline entry.
///
/// The third of the three, and the one an outline of records most needs to
/// avoid: were its per-record tables read as tracking declarations, every row
/// would verify a declared head against a document that does not yet exist, and
/// would fail correctly and uselessly
/// (´[EMBER-conv:migration:tracking-columns]´).
///
/// ´const:emberlinter:tracking-entry-column´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:tracking-entry-column-text-xbe21fb73´
const ENTRY_COLUMN: &str = "Entry";

/// Where a tracking table's meaningful cells stand in its rows.
///
/// A header names its columns in whatever order the outline finds readable, and
/// may name columns this module has no interest in, so the reader records the
/// positions once per table rather than assuming an arity or an order.
#[derive(Debug, Clone, Copy)]
struct Columns {
    entry: Option<usize>,
    head: usize,
    document: usize,
    width: usize,
}

impl Columns {
    /// Whether a column carries one end of the declared relation.
    ///
    /// Only these cells are held to the display syntax; the rest of a row is the
    /// outline's own prose and cites as prose does.
    const fn is_declaring(&self, column: usize) -> bool {
        column == self.head
            || column == self.document
            || match self.entry {
                Some(entry) => column == entry,
                None => false,
            }
    }

    /// Read a header row, when it declares a tracking.
    fn read(header: &[String]) -> Option<Self> {
        let position = |name: &str| header.iter().position(|cell| cell == name);

        Some(Self {
            entry: position(ENTRY_COLUMN),
            head: position(HEAD_COLUMN)?,
            document: position(DOCUMENT_COLUMN)?,
            width: header.len(),
        })
    }
}

/// One declared tracking: the head an outline claims, and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingRow {
    entry: Option<Label>,
    head: Label,
    document: PathBuf,
    location: Location,
}

impl TrackingRow {
    /// The outline entry doing the tracking, where the outline names one.
    ///
    /// A tracking table without an Entry column identifies its rows by position,
    /// and every row of such a table answers with nothing here.
    #[must_use]
    pub const fn entry(&self) -> Option<&Label> {
        self.entry.as_ref()
    }

    /// The head the entry claims the tracked document carries.
    #[must_use]
    pub const fn head(&self) -> &Label {
        &self.head
    }

    /// The tracked document, by its path from the repository root.
    #[must_use]
    pub fn document(&self) -> &Path {
        &self.document
    }

    /// Where the declaring row stands in the outline.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// One outline document and everything it declares.
#[derive(Debug, Clone)]
pub struct Outline {
    path: PathBuf,
    rows: Vec<TrackingRow>,
}

impl Outline {
    /// The outline document's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every tracking the outline declares, in document order.
    #[must_use]
    pub fn rows(&self) -> &[TrackingRow] {
        &self.rows
    }

    /// Whether the document declares any tracking at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Read one source's tracking declaration, and what its rows failed to be.
///
/// A document that carries no tracking table declares nothing and is not an
/// outline; that is not a defect, so the reader stays total and simply returns an
/// empty outline.
#[must_use]
pub fn read_outline(path: &Path, source: &str) -> (Outline, Vec<Finding>) {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let mut reader = Reader::new(path, source);

    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        reader.event(&event, range.start, range.end);
    }

    reader.finish()
}

/// Check every declared tracking, both ways.
///
/// The heads are the authored heads of every scanned source, and the mints the
/// labels each source minted, both as the harvest produced them. Grouping by
/// tracked document rather than by outline is what makes "claimed by exactly one
/// entry" mean what it says: a head claimed once by each of two outlines is
/// claimed twice, and the report names both claims.
#[must_use]
pub fn validate_tracking(
    outlines: &[Outline],
    heads: &BTreeMap<PathBuf, Vec<Head>>,
    mints: &BTreeMap<PathBuf, BTreeSet<Label>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut claims: BTreeMap<PathBuf, BTreeMap<Label, Vec<(PathBuf, Location)>>> = BTreeMap::new();
    let mut declared_at: BTreeMap<(PathBuf, PathBuf), Location> = BTreeMap::new();

    for outline in outlines {
        for row in &outline.rows {
            if let Some(entry) = row.entry.as_ref() {
                let known = mints
                    .get(&outline.path)
                    .is_some_and(|minted| minted.contains(entry));

                if !known {
                    findings.push(Finding::UnknownOutlineEntry {
                        entry: entry.clone(),
                        location: row.location.clone(),
                    });
                }
            }

            claims
                .entry(row.document.clone())
                .or_default()
                .entry(row.head.clone())
                .or_default()
                .push((outline.path.clone(), row.location.clone()));

            declared_at
                .entry((row.document.clone(), outline.path.clone()))
                .or_insert_with(|| row.location.clone());
        }
    }

    for (document, claimed) in &claims {
        let Some(carried) = heads.get(document) else {
            for (head, sites) in claimed {
                for (_outline, location) in sites {
                    findings.push(Finding::UntrackableDocument {
                        head: head.clone(),
                        document: document.to_string_lossy().into_owned(),
                        location: location.clone(),
                    });
                }
            }

            continue;
        };

        let by_label: BTreeMap<&Label, &Head> =
            carried.iter().map(|head| (head.label(), head)).collect();

        for (head, sites) in claimed {
            match by_label.get(head) {
                Some(_carried) => report_double_claims(head, document, sites, &mut findings),
                None => {
                    for (_outline, location) in sites {
                        findings.push(Finding::UnfulfilledOutlineEntry {
                            head: head.clone(),
                            document: document.to_string_lossy().into_owned(),
                            location: location.clone(),
                        });
                    }
                }
            }
        }

        report_unclaimed_heads(document, carried, claimed, &declared_at, &mut findings);
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    findings
}

/// Report a head that more than one entry claims, naming every extra claim.
fn report_double_claims(
    head: &Label,
    document: &Path,
    sites: &[(PathBuf, Location)],
    findings: &mut Vec<Finding>,
) {
    let Some((_first_outline, first)) = sites.first() else {
        return;
    };

    for (_outline, second) in sites.iter().skip(1) {
        findings.push(Finding::DoublyClaimedHead {
            head: head.clone(),
            document: document.to_string_lossy().into_owned(),
            first: first.clone(),
            second: second.clone(),
        });
    }
}

/// Report every head of a tracked document that no entry claims.
fn report_unclaimed_heads(
    document: &Path,
    carried: &[Head],
    claimed: &BTreeMap<Label, Vec<(PathBuf, Location)>>,
    declared_at: &BTreeMap<(PathBuf, PathBuf), Location>,
    findings: &mut Vec<Finding>,
) {
    let declaration = declared_at
        .iter()
        .find_map(|((tracked, _outline), location)| (tracked == document).then_some(location));

    let Some(declaration) = declaration else {
        return;
    };

    for head in carried {
        if claimed.contains_key(head.label()) {
            continue;
        }

        findings.push(Finding::UnclaimedHead {
            head: head.label().clone(),
            outline: declaration.path().to_string_lossy().into_owned(),
            location: head.location().clone(),
            declaration: declaration.clone(),
        });
    }
}

/// The table reader, which is a small state machine over the event stream.
struct Reader<'a> {
    path: &'a Path,
    source: &'a str,
    columns: Option<Columns>,
    cells: Vec<String>,
    cell: String,
    in_cell: bool,
    row_start: usize,
    rows: Vec<TrackingRow>,
    findings: Vec<Finding>,
}

impl<'a> Reader<'a> {
    const fn new(path: &'a Path, source: &'a str) -> Self {
        Self {
            path,
            source,
            columns: None,
            cells: Vec::new(),
            cell: String::new(),
            in_cell: false,
            row_start: 0,
            rows: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn event(&mut self, event: &Event<'_>, start: usize, end: usize) {
        match event {
            Event::Start(Tag::Table(_)) => self.columns = None,
            Event::Start(Tag::TableHead) => self.cells.clear(),
            Event::Start(Tag::TableRow) => {
                self.cells.clear();
                self.row_start = start;
            }
            Event::End(TagEnd::TableHead) => {
                self.columns = Columns::read(&std::mem::take(&mut self.cells));
            }
            Event::End(TagEnd::TableRow) => self.row(),
            Event::Start(Tag::TableCell) => {
                self.in_cell = true;
                self.cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                self.in_cell = false;
                let cell = std::mem::take(&mut self.cell);
                self.cells.push(cell.trim().to_owned());
            }
            Event::Text(text) if self.in_cell => self.cell.push_str(text),
            Event::Code(text) if self.in_cell => {
                let declaring = self
                    .columns
                    .is_some_and(|columns| columns.is_declaring(self.cells.len()));

                if declaring && opening_backticks(&self.source[start..end]) == 1 {
                    self.findings.push(Finding::ParticipatingTrackingCell {
                        text: text.to_string(),
                        location: Location::new(self.path, self.source, start),
                    });
                }

                self.cell.push_str(text);
            }
            _ => {}
        }
    }

    /// Read one body row of a tracking table.
    fn row(&mut self) {
        let Some(columns) = self.columns else {
            return;
        };

        let location = Location::new(self.path, self.source, self.row_start);

        if self.cells.len() != columns.width {
            self.reject(
                format!(
                    "a tracking row has {} cells, not {}",
                    columns.width,
                    self.cells.len()
                ),
                location,
            );
            return;
        }

        let Some(head) = Label::parse(&self.cells[columns.head]) else {
            self.reject(
                "the Head cell carries a well-formed label".to_owned(),
                location,
            );
            return;
        };

        let entry = match columns
            .entry
            .map(|position| Label::parse(&self.cells[position]))
        {
            None => None,
            Some(Some(entry)) => Some(entry),
            Some(None) => {
                self.reject(
                    "the Entry cell carries a well-formed label".to_owned(),
                    location,
                );
                return;
            }
        };

        let document = &self.cells[columns.document];

        if document.is_empty() {
            self.reject(
                "the Document cell names a path from the repository root".to_owned(),
                location,
            );
            return;
        }

        self.rows.push(TrackingRow {
            entry,
            head,
            document: PathBuf::from(document),
            location,
        });
    }

    /// Report a row of a tracking table that is not a tracking.
    fn reject(&mut self, reason: String, location: Location) {
        self.findings.push(Finding::MalformedTrackingRow {
            text: self.cells.join(" | "),
            reason,
            location,
        });
    }

    fn finish(self) -> (Outline, Vec<Finding>) {
        (
            Outline {
                path: self.path.to_path_buf(),
                rows: self.rows,
            },
            self.findings,
        )
    }
}

/// Count the backticks opening a code span's source range.
fn opening_backticks(raw: &str) -> usize {
    raw.bytes().take_while(|byte| *byte == b'`').count()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use super::{Outline, read_outline, validate_tracking};
    use crate::finding::Finding;
    use crate::head::{Head, read_heads};
    use crate::label::Label;
    use crate::prose::scan_markdown;

    const TRACKED: &str = "docs/tracked.md";
    const OUTLINE: &str = "docs/outline.md";

    fn label(text: &str) -> Label {
        Label::parse(text).expect("well-formed")
    }

    /// One outline declaring the two heads the tracked fixture carries.
    fn outline_source(rows: &str) -> String {
        format!(
            "# Outline\n\n\
             **Convention (Tracking)** · `conv:fixture:tracking`\n\n\
             | Entry | Head | Document |\n| --- | --- | --- |\n{rows}\n\
             \n**Entry (First)** · `entry:fixture:first`\n\n\
             Scope note.\n\n\
             **Entry (Second)** · `entry:fixture:second`\n\n\
             Scope note.\n"
        )
    }

    /// The same outline in the positional register: no Entry column, no mints.
    fn positional_source(rows: &str) -> String {
        format!("# Outline\n\n| Head | Document |\n| --- | --- |\n{rows}\n")
    }

    fn positional_rows() -> String {
        format!(
            "| ``sec:fixture:first`` | ``{TRACKED}`` |\n\
             | ``sec:fixture:second`` | ``{TRACKED}`` |"
        )
    }

    fn tracked_source(heads: &str) -> String {
        format!("# Tracked\n\n{heads}")
    }

    const BOTH_HEADS: &str =
        "## First · `sec:fixture:first`\n\nProse.\n\n## Second · `sec:fixture:second`\n\nProse.\n";

    fn both_rows() -> String {
        format!(
            "| ``entry:fixture:first`` | ``sec:fixture:first`` | ``{TRACKED}`` |\n\
             | ``entry:fixture:second`` | ``sec:fixture:second`` | ``{TRACKED}`` |"
        )
    }

    /// Read one source's heads and mints exactly as the harvest would.
    fn harvest(path: &str, source: &str) -> (Vec<Head>, BTreeSet<Label>) {
        let path = Path::new(path);
        let (occurrences, blocks, _findings) = scan_markdown(path, source).into_parts();
        let minted = occurrences
            .iter()
            .filter(|occurrence| occurrence.is_mint())
            .map(|occurrence| occurrence.label().clone())
            .collect();
        let (heads, _head_findings) = read_heads(path, source, &blocks, &occurrences);

        (heads, minted)
    }

    /// Check one outline against one tracked document, as the engine would.
    fn check(outline_text: &str, tracked_text: &str) -> Vec<Finding> {
        let (outline, mut findings) = read_outline(Path::new(OUTLINE), outline_text);

        let mut heads: BTreeMap<PathBuf, Vec<Head>> = BTreeMap::new();
        let mut mints: BTreeMap<PathBuf, BTreeSet<Label>> = BTreeMap::new();

        for (path, text) in [(OUTLINE, outline_text), (TRACKED, tracked_text)] {
            let (carried, minted) = harvest(path, text);
            heads.insert(PathBuf::from(path), carried);
            mints.insert(PathBuf::from(path), minted);
        }

        findings.extend(validate_tracking(&[outline], &heads, &mints));

        findings
    }

    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(Finding::code).collect()
    }

    /// A tracking table declares, row by row, which entry claims which head in
    /// which document, and all three are recovered from the row. An outline can
    /// therefore say what a document is supposed to contain.
    ///
    /// ´claim:outline:a-tracking-row-declares-an-entry-a-head-and-a-document´
    /// ´test:unit:reads-a-tracking-table´
    #[test]
    fn reads_a_tracking_table() {
        let (outline, findings) = read_outline(Path::new(OUTLINE), &outline_source(&both_rows()));

        assert_eq!(findings, []);
        assert_eq!(outline.rows().len(), 2);
        assert_eq!(
            outline.rows()[0].entry(),
            Some(&label("entry:fixture:first"))
        );
        assert_eq!(outline.rows()[0].head(), &label("sec:fixture:first"));
        assert_eq!(outline.rows()[0].document(), Path::new(TRACKED));
    }

    /// The column names are what make a table a declaration: an ordinary table
    /// of other columns declares no tracking and raises nothing, so a document
    /// may tabulate whatever it likes without being held to an outline.
    ///
    /// ´claim:outline:the-column-names-are-what-declare-a-tracking-table´
    /// ´test:unit:reads-no-tracking-from-an-ordinary-table´
    #[test]
    fn reads_no_tracking_from_an_ordinary_table() {
        let source = "| Document | Lines |\n| --- | --- |\n| ``docs/spec.md`` | 6,084 |\n";
        let (outline, findings) = read_outline(Path::new(OUTLINE), source);

        assert!(
            outline.is_empty(),
            "a table is a declaration only under the three names"
        );
        assert_eq!(findings, []);
    }

    /// Once a table declares tracking, every row in it must be one: a row
    /// missing a cell, one whose head is not a label, and one whose document is
    /// blank are each reported, and none of them is read as a tracking.
    ///
    /// ´claim:outline:every-row-of-a-tracking-table-must-be-a-tracking´
    /// ´test:unit:reports-a-row-that-is-not-a-tracking´
    #[test]
    fn reports_a_row_that_is_not_a_tracking() {
        let rows = format!(
            "| ``entry:fixture:first`` | ``sec:fixture:first`` |\n\
             | ``entry:fixture:second`` | ``not a label`` | ``{TRACKED}`` |\n\
             | ``entry:fixture:second`` | ``sec:fixture:second`` |  |"
        );
        let (outline, findings) = read_outline(Path::new(OUTLINE), &outline_source(&rows));

        assert!(outline.is_empty());
        assert_eq!(
            codes(&findings),
            [
                "malformed_tracking_row",
                "malformed_tracking_row",
                "malformed_tracking_row"
            ]
        );
    }

    /// A tracking cell displays a label rather than citing it: a cell written
    /// to participate is reported, though the row is still read, because an
    /// outline that cited every head it tracked would make the whole corpus
    /// depend on the outline.
    ///
    /// ´claim:outline:a-tracking-cell-displays-its-label-and-does-not-cite-it´
    /// ´test:unit:reports-a-cell-written-to-participate´
    #[test]
    fn reports_a_cell_written_to_participate() {
        let rows = format!("| `entry:fixture:first` | ``sec:fixture:first`` | ``{TRACKED}`` |");
        let (outline, findings) = read_outline(Path::new(OUTLINE), &outline_source(&rows));

        assert_eq!(outline.rows().len(), 1, "the row is still read");
        assert_eq!(codes(&findings), ["participating_tracking_cell"]);
    }

    /// An outline and the document it tracks agree when every declared row is
    /// fulfilled by a head and every head is claimed by a row. The relation is
    /// checked in both directions at once, and agreement raises nothing.
    ///
    /// ´claim:outline:an-outline-and-its-document-agree-in-both-directions´
    /// ´test:unit:tracks-a-fixture-pair-both-ways´
    #[test]
    fn tracks_a_fixture_pair_both_ways() {
        let findings = check(&outline_source(&both_rows()), &tracked_source(BOTH_HEADS));

        assert_eq!(codes(&findings), Vec::<&str>::new(), "got {findings:?}");
    }

    /// A head no row claims is reported at both places at once — where the head
    /// stands and where the outline omits it — so the reader is shown the
    /// document that grew and the outline that did not.
    ///
    /// ´claim:outline:a-head-no-row-claims-is-reported-at-both-places´
    /// ´test:unit:reports-a-head-no-entry-claims´
    #[test]
    fn reports_a_head_no_entry_claims() {
        let one = format!("| ``entry:fixture:first`` | ``sec:fixture:first`` | ``{TRACKED}`` |");
        let findings = check(&outline_source(&one), &tracked_source(BOTH_HEADS));

        let [
            Finding::UnclaimedHead {
                head,
                location,
                declaration,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one unclaimed head, got {findings:?}");
        };

        assert_eq!(head.to_string(), "sec:fixture:second");
        assert_eq!(location.path(), Path::new(TRACKED), "the head's own place");
        assert_eq!(
            declaration.path(),
            Path::new(OUTLINE),
            "and the outline that omits it"
        );
    }

    /// A row no head fulfils is reported the other way round, naming the head
    /// promised, the document that should carry it, and the row that promised
    /// it. An outline may not describe a document that does not exist.
    ///
    /// ´claim:outline:a-row-no-head-fulfils-is-reported-at-both-places´
    /// ´test:unit:reports-an-entry-no-head-fulfils´
    #[test]
    fn reports_an_entry_no_head_fulfils() {
        let only_first = "## First · `sec:fixture:first`\n\nProse.\n";
        let findings = check(&outline_source(&both_rows()), &tracked_source(only_first));

        let [
            Finding::UnfulfilledOutlineEntry {
                head,
                document,
                location,
            },
        ] = findings.as_slice()
        else {
            panic!("expected one unfulfilled entry, got {findings:?}");
        };

        assert_eq!(head.to_string(), "sec:fixture:second");
        assert_eq!(document, TRACKED, "the document that should carry it");
        assert_eq!(
            location.path(),
            Path::new(OUTLINE),
            "and the row that claims it"
        );
    }

    /// A head is claimed by exactly one row: two rows claiming it is reported
    /// with both claims in the order they were written, so the tracking stays a
    /// relation a reader can follow one way.
    ///
    /// ´claim:outline:a-head-claimed-twice-is-reported-with-both-claims´
    /// ´test:unit:reports-a-head-two-entries-claim´
    #[test]
    fn reports_a_head_two_entries_claim() {
        let twice = format!(
            "| ``entry:fixture:first`` | ``sec:fixture:first`` | ``{TRACKED}`` |\n\
             | ``entry:fixture:second`` | ``sec:fixture:first`` | ``{TRACKED}`` |"
        );
        let only_first = "## First · `sec:fixture:first`\n\nProse.\n";
        let findings = check(&outline_source(&twice), &tracked_source(only_first));

        let [
            Finding::DoublyClaimedHead {
                head,
                first,
                second,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one doubly claimed head, got {findings:?}");
        };

        assert_eq!(head.to_string(), "sec:fixture:first");
        assert!(
            first.offset() < second.offset(),
            "both claims, in the order written"
        );
    }

    /// A row naming an entry the outline never mints is reported: an outline's
    /// rows point at its own entries, so a row cannot claim on behalf of
    /// something that does not exist in the outline.
    ///
    /// ´claim:outline:a-row-naming-an-unminted-entry-is-reported´
    /// ´test:unit:reports-an-entry-the-outline-never-mints´
    #[test]
    fn reports_an_entry_the_outline_never_mints() {
        let stray = format!("| ``entry:fixture:third`` | ``sec:fixture:first`` | ``{TRACKED}`` |");
        let only_first = "## First · `sec:fixture:first`\n\nProse.\n";
        let findings = check(&outline_source(&stray), &tracked_source(only_first));

        assert_eq!(
            codes(&findings),
            ["unknown_outline_entry"],
            "got {findings:?}"
        );
    }

    /// A row naming a document the carrier never read is reported as
    /// untrackable, so an outline cannot quietly track a file that was renamed,
    /// deleted, or never written.
    ///
    /// ´claim:outline:a-row-naming-a-document-nobody-read-is-untrackable´
    /// ´test:unit:reports-a-document-the-carrier-never-read´
    #[test]
    fn reports_a_document_the_carrier_never_read() {
        let elsewhere = "| ``entry:fixture:first`` | ``sec:fixture:first`` | ``docs/absent.md`` |";
        let (outline, _findings) = read_outline(Path::new(OUTLINE), &outline_source(elsewhere));

        let findings = validate_tracking(&[outline], &BTreeMap::new(), &BTreeMap::new());

        assert_eq!(
            codes(&findings),
            ["unknown_outline_entry", "untrackable_document"]
        );
    }

    /// An outline may track without minting an entry for each row: where there
    /// is no entry column, a row is identified by where it stands. A document
    /// can be tracked before its outline has been written up as entries.
    ///
    /// ´claim:outline:a-row-without-an-entry-is-identified-by-where-it-stands´
    /// ´test:unit:reads-a-tracking-table-that-names-no-entry´
    #[test]
    fn reads_a_tracking_table_that_names_no_entry() {
        let (outline, findings) =
            read_outline(Path::new(OUTLINE), &positional_source(&positional_rows()));

        assert_eq!(findings, []);
        assert_eq!(outline.rows().len(), 2);
        assert_eq!(
            outline.rows()[0].entry(),
            None,
            "the row is identified by where it stands"
        );
        assert_eq!(outline.rows()[0].head(), &label("sec:fixture:first"));
        assert_eq!(outline.rows()[0].document(), Path::new(TRACKED));
    }

    /// The columns a tracking table needs may stand anywhere among columns of
    /// the outline's own, in any order and any number, so an outline can carry
    /// whatever else it wants to record beside the tracking.
    ///
    /// ´claim:outline:a-tracking-table-may-carry-columns-of-its-own-in-any-order´
    /// ´test:unit:reads-a-tracking-table-carrying-columns-of-its-own´
    #[test]
    fn reads_a_tracking_table_carrying_columns_of_its_own() {
        let source = format!(
            "# Outline\n\n| Kind | Head | Derived from | Scope | Document |\n\
             | --- | --- | --- | --- | --- |\n\
             | ``sec`` | ``sec:fixture:first`` | ``old:one`` | what it covers | ``{TRACKED}`` |\n"
        );
        let (outline, findings) = read_outline(Path::new(OUTLINE), &source);

        assert_eq!(findings, []);
        assert_eq!(
            outline.rows().len(),
            1,
            "the columns between are the outline's business"
        );
        assert_eq!(outline.rows()[0].head(), &label("sec:fixture:first"));
        assert_eq!(outline.rows()[0].document(), Path::new(TRACKED));
    }

    /// Both the head column and the document column are needed to declare
    /// tracking: a head column alone declares none, because a head with no
    /// document named is not a claim about any document.
    ///
    /// ´claim:outline:a-head-column-alone-declares-no-tracking´
    /// ´test:unit:reads-no-tracking-without-a-document-column´
    #[test]
    fn reads_no_tracking_without_a_document_column() {
        let source = "| Kind | Head |\n| --- | --- |\n| ``sec`` | ``sec:fixture:first`` |\n";
        let (outline, findings) = read_outline(Path::new(OUTLINE), source);

        assert!(
            outline.is_empty(),
            "a head column alone declares no tracking"
        );
        assert_eq!(findings, []);
    }

    /// The display rule reaches the tracking cells and no further: a scope note
    /// in a column of the outline's own cites exactly as the prose around it
    /// does, so an outline can explain its entries in ordinary language.
    ///
    /// ´claim:outline:a-column-of-the-outlines-own-cites-like-ordinary-prose´
    /// ´test:unit:leaves-a-citation-in-a-column-of-the-outline-s-own´
    #[test]
    fn leaves_a_citation_in_a_column_of_the_outline_s_own() {
        let source = format!(
            "# Outline\n\n| Head | Scope | Document |\n| --- | --- | --- |\n\
             | ``sec:fixture:first`` | what it covers, per (`sec:fixture:second`) | ``{TRACKED}`` |\n"
        );
        let (outline, findings) = read_outline(Path::new(OUTLINE), &source);

        assert_eq!(
            findings,
            [],
            "a scope note cites as the prose around it does"
        );
        assert_eq!(outline.rows().len(), 1);
    }

    /// Agreement is checked the same way in the positional register: an outline
    /// tracking without entries and the document it tracks agree, and nothing
    /// is reported.
    ///
    /// (´claim:outline:an-outline-and-its-document-agree-in-both-directions´)
    /// ´test:unit:tracks-a-positional-fixture-pair-both-ways´
    #[test]
    fn tracks_a_positional_fixture_pair_both_ways() {
        let findings = check(
            &positional_source(&positional_rows()),
            &tracked_source(BOTH_HEADS),
        );

        assert_eq!(codes(&findings), Vec::<&str>::new(), "got {findings:?}");
    }

    /// Drift on the document side is caught in the positional register too: an
    /// unclaimed head is reported with the head's own place and the outline
    /// that omits it.
    ///
    /// (´claim:outline:a-head-no-row-claims-is-reported-at-both-places´)
    /// ´test:unit:reports-a-head-no-positional-row-claims´
    #[test]
    fn reports_a_head_no_positional_row_claims() {
        let one = format!("| ``sec:fixture:first`` | ``{TRACKED}`` |");
        let findings = check(&positional_source(&one), &tracked_source(BOTH_HEADS));

        let [
            Finding::UnclaimedHead {
                head,
                location,
                declaration,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one unclaimed head, got {findings:?}");
        };

        assert_eq!(head.to_string(), "sec:fixture:second");
        assert_eq!(location.path(), Path::new(TRACKED), "the head's own place");
        assert_eq!(
            declaration.path(),
            Path::new(OUTLINE),
            "and the outline that omits it"
        );
    }

    /// Drift on the outline side is caught likewise, and a row having no entry
    /// to be cited by is cited by its line instead, so the reader is still sent
    /// to the exact row that promised what is missing.
    ///
    /// (´claim:outline:a-row-no-head-fulfils-is-reported-at-both-places´)
    /// ´test:unit:reports-a-positional-row-no-head-fulfils´
    #[test]
    fn reports_a_positional_row_no_head_fulfils() {
        let only_first = "## First · `sec:fixture:first`\n\nProse.\n";
        let findings = check(
            &positional_source(&positional_rows()),
            &tracked_source(only_first),
        );

        let [
            Finding::UnfulfilledOutlineEntry {
                head,
                document,
                location,
            },
        ] = findings.as_slice()
        else {
            panic!("expected one unfulfilled row, got {findings:?}");
        };

        assert_eq!(head.to_string(), "sec:fixture:second");
        assert_eq!(document, TRACKED, "the document that should carry it");
        assert_eq!(
            location.path(),
            Path::new(OUTLINE),
            "and the row that claims it"
        );
        assert_eq!(
            location.line(),
            6,
            "cited by line, having no entry to be cited by"
        );
    }

    /// A document declaring no tracking at all is read as declaring none, with
    /// nothing reported, so outline tracking is something a document opts into
    /// by writing a table rather than something every document owes.
    ///
    /// ´claim:outline:a-document-declaring-no-tracking-owes-nothing´
    /// ´test:unit:reads-an-outline-that-declares-nothing´
    #[test]
    fn reads_an_outline_that_declares_nothing() {
        let (outline, findings): (Outline, Vec<Finding>) =
            read_outline(Path::new(OUTLINE), "# Just prose\n\nNothing here.\n");

        assert!(outline.is_empty());
        assert_eq!(findings, []);
    }
}
