// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Markdown prose scanning: which spans participate, and what they read as.
//!
//! The participation judgment of ADR-L-014, A calculus of documentation and
//! source labels, settles what this module may look at.
//! In prose, occurrences in authored text participate, while fenced blocks and
//! double-backtick spans do not — a token shown but not meant is placed in one
//! of these. The judgment also insists that delimiter pairing is settled within
//! a region before any span in it is parsed, and that an unpaired backtick
//! leaves its block's spans undefined: a hard failure bounded by that block,
//! with the rest of the file resolved normally.
//!
//! Both requirements are why this module drives a `CommonMark` tokenizer rather
//! than matching bytes. The tokenizer settles block structure, delimiter
//! pairing, and code-span content normalisation, which together are exactly the
//! "span is logical, never a run of bytes" clause of the well-formed
//! environment (ADR-L-014, A calculus of documentation and source labels): quotation markers, list
//! continuation indentation, and soft line breaks are resolved away before this
//! module sees a span's interior.
//!
//! Two things the tokenizer does not report directly are recovered from source
//! offsets. The number of delimiting backticks, which separates a participating
//! single-backtick span from a displayed double-backtick one, is read from the
//! first bytes of the span's source range. And the parentheses that distinguish
//! a citation from a mint sit outside the span, so they are read from the
//! characters immediately flanking that range.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_a_heading_mint_and_a_prose_citation`] | prose | In prose the same label reads as a mint at a heading and as a citation where it stands parenthesised in a sentence, so one document establishes a statement and refers back to it in the notation a reader already knows, and neither is a defect. |
//! | [`fenced_blocks_do_not_participate`] | prose | A region that does not participate yields no occurrences and no findings: what stands in it is a token shown rather than a statement made. A fenced block is such a region, so a document may exhibit both mints and citations without making either. |
//! | [`indented_code_blocks_do_not_participate`] | prose | cites (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´) |
//! | [`double_backtick_spans_do_not_participate`] | prose | cites (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´) |
//! | [`resolves_block_structure_away`] | prose | A span is logical rather than a run of bytes: quotation markers, list leaders and continuation indentation are resolved away before a span's interior is read, so an occurrence inside a quotation or split across a wrapped list item reads exactly as it would in a plain paragraph. |
//! | [`reads_imported_citations`] | prose | The imported form is available in prose too: a parenthesised span bracketing a prefix and a label cites across an ownership boundary and carries the label it named. |
//! | [`prose_fails_import_shaped_spans_without_parentheses`] | occurrence | cites (´claim:occurrence:an-import-without-parentheses-is-a-failure-not-text´) |
//! | [`unpaired_backtick_fails_its_block_only`] | prose | An unpaired delimiter leaves its own block's spans undefined and is reported, while every later block is read normally. The failure is hard but bounded, so one stray character costs one block rather than the whole document. |
//! | [`escaped_backticks_are_not_unpaired`] | prose | An escaped delimiter is a character rather than a delimiter, so it pairs with nothing and leaves the occurrence beside it intact. Prose may write about the notation using the notation's own punctuation. |
//! | [`prose_warns_on_near_miss_spans`] | occurrence | cites (´claim:occurrence:a-span-one-repair-from-a-label-is-a-named-near-miss´) |
//! | [`warns_on_an_undelimited_bracket_import`] | occurrence | A parenthesis directly wrapping bracketed import-shaped text with no span delimiters is warned on rather than left inert: the author wrote the exact bytes an imported citation means, minus the marks that would make it one, and no span reader would otherwise ever see it. The warning changes nothing about the text's status as text, and a correctly delimited import beside it still resolves as itself. |
//! | [`plain_code_spans_are_text`] | prose | Ordinary technical writing stays ordinary: a command line and a file name in code font are text, yielding neither occurrences nor findings, so adopting the calculus does not change how documentation is written. |
//! | [`records_the_blocks_a_head_can_stand_in`] | prose | The blocks a head can stand in are recorded with what a later pass needs of them: which kind of block each is, the text it carries, the bold run opening it if there is one, and an extent that answers which block a given occurrence stands inside. |
//! | [`records_only_a_bold_run_that_opens_its_block`] | prose | Only a bold run that opens its block is recorded as one: emphasis in the middle of a sentence opens nothing, so a paragraph stressing a word partway through is not mistaken for an environment head. |

use std::path::Path;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::finding::{Finding, Location};
use crate::occurrence::{
    Occurrence, Reading, Syntax, UNDELIMITED_IMPORT_REASON, occurrence_from_reading, read_span,
    undelimited_imports,
};

/// Which block of a document a span-bearing region is.
///
/// Only the two blocks that can head an environment are distinguished: the
/// format's own sectioning rung, and the paragraph a bold environment head opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A heading of the given rung depth, counted from one.
    Heading {
        /// How deep the rung sits, with the document title at one.
        level: usize,
    },
    /// An ordinary paragraph.
    Paragraph,
}

/// One block of a Markdown source, as the head reader needs to see it.
///
/// The reader needs three things a raw event stream does not hand it: which block
/// this is, the block's extent, so the spans standing inside it can be found, and
/// whether the block opens with a bold run, which is how this corpus writes an
/// environment head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProseBlock {
    kind: BlockKind,
    start: usize,
    end: usize,
    text: String,
    strong: Option<String>,
}

impl ProseBlock {
    /// Which block this is.
    #[must_use]
    pub const fn kind(&self) -> BlockKind {
        self.kind
    }

    /// The byte offset the block opens at.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// The byte offset the block closes at.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// The block's authored text, with its spans left out.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The bold run the block opens with, when it opens with one.
    #[must_use]
    pub fn strong(&self) -> Option<&str> {
        self.strong.as_deref()
    }

    /// Whether an offset falls inside this block.
    #[must_use]
    pub const fn holds(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }
}

/// What one Markdown source yielded.
#[derive(Debug, Default)]
pub struct ProseScan {
    occurrences: Vec<Occurrence>,
    blocks: Vec<ProseBlock>,
    findings: Vec<Finding>,
}

impl ProseScan {
    /// The participating occurrences, in source order.
    #[must_use]
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }

    /// The heading and paragraph blocks, in source order.
    #[must_use]
    pub fn blocks(&self) -> &[ProseBlock] {
        &self.blocks
    }

    /// The findings raised while scanning, in source order.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Consume the scan, yielding its occurrences, blocks, and findings.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Occurrence>, Vec<ProseBlock>, Vec<Finding>) {
        (self.occurrences, self.blocks, self.findings)
    }
}

/// Scan one Markdown source for participating occurrences.
#[must_use]
pub fn scan_markdown(path: &Path, source: &str) -> ProseScan {
    Scanner::new(path, source).run()
}

/// A prose block, tracked so that an unpaired backtick can be bounded by it.
struct Block {
    start: usize,
    unpaired_backtick: bool,
}

/// A candidate result, held until its block is known to be well-delimited.
struct Pending {
    block: Option<usize>,
    item: PendingItem,
}

enum PendingItem {
    Occurrence(Box<Occurrence>),
    Finding(Box<Finding>),
}

struct Scanner<'a> {
    path: &'a Path,
    source: &'a str,
    blocks: Vec<Block>,
    open: Vec<usize>,
    code_block_depth: usize,
    pending: Vec<Pending>,
    prose_blocks: Vec<ProseBlock>,
    open_prose: Option<usize>,
    strong_from: Option<usize>,
    /// The contiguous run of authored text the import warning reads, as its
    /// source range and the innermost block open when it began.
    import_run: Option<(usize, usize, Option<usize>)>,
}

impl<'a> Scanner<'a> {
    const fn new(path: &'a Path, source: &'a str) -> Self {
        Self {
            path,
            source,
            blocks: Vec::new(),
            open: Vec::new(),
            code_block_depth: 0,
            pending: Vec::new(),
            prose_blocks: Vec::new(),
            open_prose: None,
            strong_from: None,
            import_run: None,
        }
    }

    fn run(mut self) -> ProseScan {
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;

        for (event, range) in Parser::new_ext(self.source, options).into_offset_iter() {
            match event {
                Event::Start(tag) => self.start(&tag, range.start, range.end),
                Event::End(tag) => self.end(tag),
                Event::Code(interior) if self.code_block_depth == 0 => {
                    self.span(&interior, range.start, range.end);
                }
                Event::Text(text) if self.code_block_depth == 0 => {
                    self.collect(&text);
                    self.text(range.start, range.end);
                }
                _ => {}
            }
        }

        self.finish()
    }

    fn start(&mut self, tag: &Tag<'_>, start: usize, end: usize) {
        if matches!(
            tag,
            Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)
        ) {
            self.code_block_depth += 1;
        }

        if let Some(kind) = head_bearing_block(tag) {
            self.prose_blocks.push(ProseBlock {
                kind,
                start,
                end,
                text: String::new(),
                strong: None,
            });
            self.open_prose = Some(self.prose_blocks.len() - 1);
            self.strong_from = None;
        }

        if matches!(tag, Tag::Strong) {
            self.open_strong();
        }

        if is_block_level(tag) {
            self.blocks.push(Block {
                start,
                unpaired_backtick: false,
            });
            self.open.push(self.blocks.len() - 1);
        }
    }

    fn end(&mut self, tag: TagEnd) {
        if matches!(tag, TagEnd::CodeBlock) {
            self.code_block_depth = self.code_block_depth.saturating_sub(1);
        }

        if matches!(tag, TagEnd::Strong) {
            self.close_strong();
        }

        if matches!(tag, TagEnd::Heading(_) | TagEnd::Paragraph) {
            self.open_prose = None;
            self.strong_from = None;
        }

        if is_block_level_end(tag) {
            self.open.pop();
        }
    }

    /// Accumulate authored text into the open heading or paragraph.
    fn collect(&mut self, text: &str) {
        if let Some(block) = self.open_prose {
            self.prose_blocks[block].text.push_str(text);
        }
    }

    /// Open a bold run, but only where it can still open its block.
    ///
    /// A bold run that begins after any authored text is emphasis inside a
    /// paragraph, not the head of an environment, so only a run standing at the
    /// very front of its block is recorded.
    fn open_strong(&mut self) {
        let Some(block) = self.open_prose else {
            return;
        };

        if self.prose_blocks[block].strong.is_none()
            && self.prose_blocks[block].text.trim().is_empty()
        {
            self.strong_from = Some(self.prose_blocks[block].text.len());
        }
    }

    fn close_strong(&mut self) {
        let (Some(block), Some(from)) = (self.open_prose, self.strong_from.take()) else {
            return;
        };

        let strong = self.prose_blocks[block].text[from..].trim().to_owned();

        if !strong.is_empty() {
            self.prose_blocks[block].strong = Some(strong);
        }
    }

    /// Read one inline code span.
    fn span(&mut self, interior: &str, start: usize, end: usize) {
        if delimiter_backticks(&self.source[start..end]) != 1 {
            // A double-backtick span displays a token without meaning it.
            return;
        }

        let location = Location::new(self.path, self.source, start);
        let reading = read_span(interior, self.is_parenthesized(start, end));

        let item = match &reading {
            Reading::Occurrence { .. } => {
                occurrence_from_reading(&reading, Syntax::Prose, &location)
                    .map(|occurrence| PendingItem::Occurrence(Box::new(occurrence)))
            }
            Reading::NonParenthesizedImport { prefix, label } => Some(PendingItem::Finding(
                Box::new(Finding::NonParenthesizedImport {
                    prefix: prefix.clone(),
                    label: label.clone(),
                    location,
                }),
            )),
            Reading::NearMiss { reason } => {
                Some(PendingItem::Finding(Box::new(Finding::NearMiss {
                    text: interior.to_owned(),
                    reason: (*reason).to_owned(),
                    location,
                })))
            }
            Reading::Text => None,
        };

        if let Some(item) = item {
            let block = self.open.last().copied();
            self.pending.push(Pending { block, item });
        }
    }

    /// Whether the characters immediately flanking a span are parentheses.
    fn is_parenthesized(&self, start: usize, end: usize) -> bool {
        let before = self.source[..start].chars().next_back();
        let after = self.source[end..].chars().next();

        before == Some('(') && after == Some(')')
    }

    /// Record an unpaired backtick against the innermost open block, and grow
    /// the contiguous text run the import warning reads.
    ///
    /// The tokenizer reports an escaped character's text without the backslash
    /// that escaped it, so the scan starts one byte early whenever the preceding
    /// byte is a backslash. Otherwise a deliberately escaped backtick would read
    /// as an unpaired one.
    fn text(&mut self, start: usize, end: usize) {
        self.note_text(start, end);

        let begin = if start > 0 && self.source.as_bytes()[start - 1] == b'\\' {
            start - 1
        } else {
            start
        };

        if !has_unescaped_backtick(&self.source[begin..end]) {
            return;
        }

        if let Some(block) = self.open.last().copied() {
            self.blocks[block].unpaired_backtick = true;
        }
    }

    /// Grow, or flush and restart, the contiguous run of authored text.
    ///
    /// The tokenizer splits a run over brackets into several events, so the
    /// import warning cannot read one event at a time: the pattern it wants —
    /// a parenthesis directly wrapping import-shaped bracket text with no span
    /// delimiters — arrives as its pieces. Adjacent source ranges are one run
    /// of bytes; anything that breaks adjacency, a code span above all, breaks
    /// the run, which is exactly right because a correctly delimited citation
    /// puts its delimiters between the parenthesis and the bracket.
    fn note_text(&mut self, start: usize, end: usize) {
        match &mut self.import_run {
            Some((_run_start, run_end, _block)) if *run_end == start => *run_end = end,
            _ => {
                self.flush_import_run();
                self.import_run = Some((start, end, self.open.last().copied()));
            }
        }
    }

    /// Warn on the undelimited bracketed imports the finished run carries.
    ///
    /// The bytes here are still text: no span reader would ever start inside
    /// them — verified inert by probe — and the near-miss clause of the
    /// total-resolution invariant (ADR-L-014, A calculus of documentation and source labels) is
    /// exactly for spans like these. The warning changes nothing about their
    /// status as text.
    fn flush_import_run(&mut self) {
        let Some((start, end, block)) = self.import_run.take() else {
            return;
        };

        for (at, interior) in undelimited_imports(&self.source[start..end]) {
            self.pending.push(Pending {
                block,
                item: PendingItem::Finding(Box::new(Finding::NearMiss {
                    text: interior.to_owned(),
                    reason: UNDELIMITED_IMPORT_REASON.to_owned(),
                    location: Location::new(self.path, self.source, start + at),
                })),
            });
        }
    }

    fn finish(mut self) -> ProseScan {
        self.flush_import_run();

        let mut scan = ProseScan {
            blocks: std::mem::take(&mut self.prose_blocks),
            ..ProseScan::default()
        };

        for pending in self.pending {
            let undefined = pending
                .block
                .is_some_and(|block| self.blocks[block].unpaired_backtick);

            if undefined {
                continue;
            }

            match pending.item {
                PendingItem::Occurrence(occurrence) => scan.occurrences.push(*occurrence),
                PendingItem::Finding(finding) => scan.findings.push(*finding),
            }
        }

        for block in &self.blocks {
            if block.unpaired_backtick {
                scan.findings.push(Finding::UnpairedBacktick {
                    location: Location::new(self.path, self.source, block.start),
                });
            }
        }

        scan.occurrences
            .sort_by_key(|occurrence| occurrence.location().offset());
        scan.findings.sort_by_key(finding_offset);

        scan
    }
}

fn finding_offset(finding: &Finding) -> usize {
    finding.primary_location().map_or(0, Location::offset)
}

/// Count the backticks opening a code span's source range.
fn delimiter_backticks(raw: &str) -> usize {
    raw.bytes().take_while(|byte| *byte == b'`').count()
}

/// Whether a run of source text carries a backtick that no escape accounts for.
///
/// A backtick reaching a text event is one the tokenizer could not pair, unless
/// the author escaped it deliberately.
fn has_unescaped_backtick(raw: &str) -> bool {
    let mut bytes = raw.bytes();

    while let Some(byte) = bytes.next() {
        match byte {
            b'\\' => {
                let _escaped = bytes.next();
            }
            b'`' => return true,
            _ => {}
        }
    }

    false
}

/// Which head-bearing block a tag opens, when it opens one.
const fn head_bearing_block(tag: &Tag<'_>) -> Option<BlockKind> {
    match tag {
        Tag::Heading { level, .. } => Some(BlockKind::Heading {
            level: heading_depth(*level),
        }),
        Tag::Paragraph => Some(BlockKind::Paragraph),
        _ => None,
    }
}

/// The depth of a heading rung, counted from one at the document title.
const fn heading_depth(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Whether a tag opens a region within which delimiter pairing is settled.
const fn is_block_level(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::TableCell
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::MetadataBlock(_)
    )
}

const fn is_block_level_end(tag: TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::TableCell
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::MetadataBlock(_)
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{BlockKind, ProseScan, scan_markdown};
    use crate::finding::Finding;
    use crate::occurrence::Form;

    fn scan(source: &str) -> ProseScan {
        scan_markdown(Path::new("doc.md"), source)
    }

    fn labels(scan: &ProseScan) -> Vec<String> {
        scan.occurrences()
            .iter()
            .map(|occurrence| occurrence.label().to_string())
            .collect()
    }

    /// In prose the same label reads as a mint at a heading and as a citation
    /// where it stands parenthesised in a sentence, so one document establishes
    /// a statement and refers back to it in the notation a reader already
    /// knows, and neither is a defect.
    ///
    /// ´claim:prose:a-heading-mints-and-a-parenthesised-span-cites´
    /// ´test:unit:reads-a-heading-mint-and-a-prose-citation´
    #[test]
    fn reads_a_heading_mint_and_a_prose_citation() {
        let scan = scan("## Syntax · `sec:labels:syntax`\n\nSee (`sec:labels:syntax`) for more.\n");

        assert_eq!(labels(&scan), ["sec:labels:syntax", "sec:labels:syntax"]);
        assert_eq!(scan.occurrences()[0].form(), &Form::Mint);
        assert_eq!(scan.occurrences()[1].form(), &Form::SameOwnerCitation);
        assert_eq!(scan.findings(), []);
    }

    /// A region that does not participate yields no occurrences and no
    /// findings: what stands in it is a token shown rather than a statement
    /// made. A fenced block is such a region, so a document may exhibit both
    /// mints and citations without making either.
    ///
    /// ´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´
    /// ´test:unit:fenced-blocks-do-not-participate´
    #[test]
    fn fenced_blocks_do_not_participate() {
        let scan = scan("Text.\n\n```text\n`sec:labels:syntax`\n(`inv:labels:unique-mint`)\n```\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
        assert_eq!(scan.findings(), []);
    }

    /// A code block written by indentation rather than by fences is the same
    /// kind of region and participates no more than a fenced one.
    ///
    /// (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´)
    /// ´test:unit:indented-code-blocks-do-not-participate´
    #[test]
    fn indented_code_blocks_do_not_participate() {
        let scan = scan("Text.\n\n    `sec:labels:syntax`\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
    }

    /// The displayed span is the third such region: doubling the delimiters
    /// shows a label without meaning it, in the bare form and the parenthesised
    /// one alike.
    ///
    /// (´claim:prose:a-non-participating-region-yields-neither-occurrence-nor-finding´)
    /// ´test:unit:double-backtick-spans-do-not-participate´
    #[test]
    fn double_backtick_spans_do_not_participate() {
        let scan =
            scan("Shown but not meant: ``sec:labels:syntax`` and (``inv:labels:unique-mint``).\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
        assert_eq!(scan.findings(), []);
    }

    /// A span is logical rather than a run of bytes: quotation markers, list
    /// leaders and continuation indentation are resolved away before a span's
    /// interior is read, so an occurrence inside a quotation or split across a
    /// wrapped list item reads exactly as it would in a plain paragraph.
    ///
    /// ´claim:prose:block-structure-is-resolved-away-before-a-span-is-read´
    /// ´test:unit:resolves-block-structure-away´
    #[test]
    fn resolves_block_structure_away() {
        let quoted = scan("> A quotation mints `sec:labels:syntax` here.\n");
        assert_eq!(labels(&quoted), ["sec:labels:syntax"]);

        let listed = scan("- An item cites (`sec:labels:syntax`)\n  across a continuation line.\n");
        assert_eq!(labels(&listed), ["sec:labels:syntax"]);
        assert_eq!(listed.occurrences()[0].form(), &Form::SameOwnerCitation);
    }

    /// The imported form is available in prose too: a parenthesised span
    /// bracketing a prefix and a label cites across an ownership boundary and
    /// carries the label it named.
    ///
    /// ´claim:prose:prose-may-cite-across-an-ownership-boundary´
    /// ´test:unit:reads-imported-citations´
    #[test]
    fn reads_imported_citations() {
        let scan = scan("Imported: (`[SPEC-def:parser:tokenizer]`).\n");

        assert_eq!(labels(&scan), ["def:parser:tokenizer"]);
        assert!(matches!(
            scan.occurrences()[0].form(),
            Form::ImportedCitation { .. }
        ));
    }

    /// The carve-out reaches the prose surface as well: an import-shaped span
    /// standing without its parentheses yields no occurrence and is reported.
    ///
    /// (´claim:occurrence:an-import-without-parentheses-is-a-failure-not-text´)
    /// ´test:unit:prose-fails-import-shaped-spans-without-parentheses´
    #[test]
    fn prose_fails_import_shaped_spans_without_parentheses() {
        let scan = scan("Bare import: `[SPEC-def:parser:tokenizer]` stands alone.\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
        assert!(matches!(
            scan.findings(),
            [Finding::NonParenthesizedImport { .. }]
        ));
    }

    /// An unpaired delimiter leaves its own block's spans undefined and is
    /// reported, while every later block is read normally. The failure is hard
    /// but bounded, so one stray character costs one block rather than the
    /// whole document.
    ///
    /// ´claim:prose:an-unpaired-delimiter-fails-its-own-block-and-no-other´
    /// ´test:unit:unpaired-backtick-fails-its-block-only´
    #[test]
    fn unpaired_backtick_fails_its_block_only() {
        let scan = scan(
            "A stray ` backtick with `sec:labels:syntax` nearby.\n\nA later block mints `inv:labels:unique-mint`.\n",
        );

        assert_eq!(labels(&scan), ["inv:labels:unique-mint"]);
        assert!(matches!(
            scan.findings(),
            [Finding::UnpairedBacktick { .. }]
        ));
    }

    /// An escaped delimiter is a character rather than a delimiter, so it
    /// pairs with nothing and leaves the occurrence beside it intact. Prose may
    /// write about the notation using the notation's own punctuation.
    ///
    /// ´claim:prose:an-escaped-delimiter-does-not-count-as-one´
    /// ´test:unit:escaped-backticks-are-not-unpaired´
    #[test]
    fn escaped_backticks_are_not_unpaired() {
        let scan = scan("An escaped \\` backtick beside `sec:labels:syntax`.\n");

        assert_eq!(labels(&scan), ["sec:labels:syntax"]);
        assert_eq!(scan.findings(), []);
    }

    /// Near misses are reported on the prose surface too: a participating span
    /// one repair from a label yields no occurrence and a warning instead.
    ///
    /// (´claim:occurrence:a-span-one-repair-from-a-label-is-a-named-near-miss´)
    /// ´test:unit:prose-warns-on-near-miss-spans´
    #[test]
    fn prose_warns_on_near_miss_spans() {
        let scan = scan("Nearly: (`Sec:labels:syntax`).\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
        assert!(matches!(scan.findings(), [Finding::NearMiss { .. }]));
    }

    /// A parenthesis directly wrapping bracketed import-shaped text with no
    /// span delimiters is warned on rather than left inert: the author wrote
    /// the exact bytes an imported citation means, minus the marks that would
    /// make it one, and no span reader would otherwise ever see it. The
    /// warning changes nothing about the text's status as text, and a
    /// correctly delimited import beside it still resolves as itself.
    ///
    /// ´claim:occurrence:an-undelimited-bracketed-import-in-parentheses-warns´
    /// ´test:unit:warns-on-an-undelimited-bracket-import´
    #[test]
    fn warns_on_an_undelimited_bracket_import() {
        let scan = scan("Inert: ([SPEC-def:parser:tokenizer]) with no delimiters.\n");

        assert_eq!(labels(&scan), Vec::<String>::new());

        let [Finding::NearMiss { text, reason, .. }] = scan.findings() else {
            panic!(
                "expected one warning for the undelimited import, got {:?}",
                scan.findings()
            );
        };

        assert_eq!(text, "[SPEC-def:parser:tokenizer]");
        assert!(
            reason.contains("delimiters"),
            "the warning names the repair: {reason}"
        );
    }

    /// Ordinary technical writing stays ordinary: a command line and a file
    /// name in code font are text, yielding neither occurrences nor findings,
    /// so adopting the calculus does not change how documentation is written.
    ///
    /// ´claim:prose:ordinary-code-font-in-documentation-stays-text´
    /// ´test:unit:plain-code-spans-are-text´
    #[test]
    fn plain_code_spans_are_text() {
        let scan = scan("Run `cargo test --all-features` in `Cargo.toml`'s directory.\n");

        assert_eq!(labels(&scan), Vec::<String>::new());
        assert_eq!(scan.findings(), []);
    }

    /// The blocks a head can stand in are recorded with what a later pass needs
    /// of them: which kind of block each is, the text it carries, the bold run
    /// opening it if there is one, and an extent that answers which block a
    /// given occurrence stands inside.
    ///
    /// ´claim:prose:candidate-head-blocks-are-recorded-with-their-kind-and-extent´
    /// ´test:unit:records-the-blocks-a-head-can-stand-in´
    #[test]
    fn records_the_blocks_a_head_can_stand_in() {
        let scan = scan(
            "## Syntax · `sec:labels:syntax`\n\n**Language (Labels)** · `lang:labels:label-language`\n",
        );
        let blocks = scan.blocks();

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind(), BlockKind::Heading { level: 2 });
        assert_eq!(blocks[0].text().trim(), "Syntax ·");
        assert_eq!(blocks[0].strong(), None);

        assert_eq!(blocks[1].kind(), BlockKind::Paragraph);
        assert_eq!(blocks[1].strong(), Some("Language (Labels)"));

        let mint = scan.occurrences()[1].location().offset();
        assert!(
            blocks[1].holds(mint),
            "the paragraph holds the span standing in it"
        );
        assert!(!blocks[0].holds(mint));
    }

    /// Only a bold run that opens its block is recorded as one: emphasis in the
    /// middle of a sentence opens nothing, so a paragraph stressing a word
    /// partway through is not mistaken for an environment head.
    ///
    /// ´claim:prose:only-a-bold-run-opening-its-block-is-recorded´
    /// ´test:unit:records-only-a-bold-run-that-opens-its-block´
    #[test]
    fn records_only_a_bold_run_that_opens_its_block() {
        let scan = scan("An entry is **ACTIVE (in a wave)** when a wave works it.\n");

        assert_eq!(scan.blocks().len(), 1);
        assert_eq!(
            scan.blocks()[0].strong(),
            None,
            "a bold run inside running text opens nothing"
        );
    }
}
