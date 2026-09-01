// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The tokenization layer: what a recognizer is handed, before any rule reads it.
//!
//! Every reference rule in this crate asks the same question of a document —
//! where is the text a reference could be *made* in, as opposed to shown in —
//! and for a long time each rule answered it for itself. The Markdown lint read
//! one text event at a time, the comment census read one comment at a time, and
//! the residual recognizer read a run of adjacent comment lines joined into one
//! region. Three answers to one question is two too many, and the corpus proved
//! it: a reference that ended one doc-comment line with its locator opening the
//! next was one reference to the recognizer that joined, and two halves of
//! nothing to the two that did not.
//!
//! That was a defect of this layer rather than of any policy. A retired
//! convention wrapped its ranges and lists across comment lines, so the wrapped
//! spelling is as much the retired form as the unwrapped one; a rule that cannot
//! see it is not a narrower rule, it is a blind one. So the joined reading the
//! residual recognizer already had is declared here once, and every recognizer
//! consumes it: a reference wrapped across a comment line boundary is one
//! reference to every rule that reads its shape
//! (ADR-L-020, The migration disciplines).
//!
//! # Tokenization sees more; participation is unchanged
//!
//! Widening what a recognizer is handed is not widening where a reference may
//! stand. The display boundaries are exactly what they were: a form in code
//! font for the label calculus, in a fenced block, in a double-backtick exhibit
//! or inside a string literal is a token shown rather than a reference made.
//! The path policy has one deliberate difference: a single-backtick span is
//! technical prose and enters its own [`Role::Span`] region unless its whole
//! value parses as pattern or configuration data. What otherwise changes is
//! only that two pieces of *the same* referring surface, separated by a line
//! break and a comment leader, are handed over as one run instead of two. The
//! participation judgment
//! (ADR-L-014, A calculus of documentation and source labels) decides what is referring surface; this
//! module decides only how much of it a rule sees at once.
//!
//! # A region is joined text plus a way back to the source
//!
//! A rule reports where a reference stands, and it must report a real offset
//! into the real file — the joined text is this module's fiction and no reader
//! can be sent to it. So a [`Region`] carries the pieces it was built from and
//! maps an offset in the joined text back to the source offset it came from.
//! Pieces are separated by a single space, because a comment's leaders are
//! resolved away before the pieces meet and two halves of a reference glued into
//! one word would be read by no rule at all.
//!
//! # Display spans are a fact of the surface, not of a rule
//!
//! One rule of the migration lint reads inline code spans rather than running
//! text, because the superseded citation syntax put its tags there. Whether a
//! span is a citation or an exhibit is decided by how many backticks opened it,
//! which is a fact about the Markdown rather than about tags, so it is answered
//! here beside the regions and handed over as [`CodeSpan`]. A span's interior
//! arrives with its own line breaks already resolved to spaces by the Markdown
//! reader, which is why a tag wrapped inside one is one tag without this module
//! doing anything further.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`joins_adjacent_comment_lines_into_one_region`] | token | A run of adjacent comment lines is one region with its leaders resolved away, so a reference wrapped across the boundary is one run of text. Anything that is not commentary between two comments ends the run, because a reference does not wrap across program text. |
//! | [`joins_a_region_across_however_many_lines_it_wraps`] | token | Joining is not a two-line rule. A run of adjacent comment lines is one region however long it is, so a reference spread over three lines is as visible as one spread over two. |
//! | [`maps_a_joined_offset_back_to_its_source`] | token | An offset into a region's joined text maps back to the offset in the source the piece came from, so a rule reading a region still reports where in the file the form stands. |
//! | [`joins_markdown_running_text_across_a_soft_break`] | token | Markdown running text is one region across a soft line break and is interrupted by anything else, which is the paragraph-joining semantics the prose surfaces already had. |
//! | [`keeps_displayed_markdown_out_of_every_region`] | token | Inline code, a double-backtick exhibit and a fenced block enter no Markdown running-text region. The path-policy reader routes eligible single-backtick spans separately, while the displayed forms remain absent everywhere. |
//! | [`joins_script_comments_and_leaves_quoted_text_alone`] | token | A shell script's comments join by the same rule, and the mark opens a comment only where a word does not continue and only outside quotation, so a form in a quoted word is data the script carries. |
//! | [`reads_code_spans_with_the_backtick_count_that_opened_them`] | token | An inline code span is reported with its interior and with whether it was displayed, which is the backtick count and nothing else. A span wrapped across a line arrives as one interior, so a form written inside one is one form. |
//! | [`routes_a_naked_single_backtick_value_and_omits_complete_data`] | token | A single-backtick span carrying a concrete filename enters the path surface in the span role, while double-backtick exhibits and complete glob, regular-expression and configuration values contribute no region. An incomplete configuration value remains technical prose and therefore participates. |
//! | [`reads_a_link_destination_as_its_own_region`] | token | A link or image destination is a region of its own, in the destination role, and its offset lands on the destination in the source rather than on the link that carries it. A destination inside a fenced block is part of that block's display and is no destination at all, which is the same boundary every other reading here keeps. |
//! | [`reads_dash_comments_without_reading_their_quoted_data`] | token | A dash pair opens a comment outside quotation and nowhere else, so a pair inside a quoted string or a quoted identifier is data the statement carries. Adjacent dash comments join by the same rule every other commentary joins by. |
//! | [`reads_delimited_markup_comments_and_an_unclosed_one_to_the_end`] | token | The delimited markup form is the whole of that language's comment syntax: everything between the delimiters is commentary and everything else is markup, and a comment nobody closed runs to the end of the document rather than aborting the scan. |
//! | [`hands_a_rule_the_regions_of_the_kind_it_was_catalogued_with`] | token | A rule reaching every carrier asks for regions by the reader its kind is catalogued with, and gets them. A kind classified as carrying no region yields none — which is the classified answer and not the unclassified one — and a prose document yields its running text and its destinations together, ordered by where they stand rather than by which reader found them. |
//! | [`reads_every_role_as_citation_bearing_but_the_path_valued_one`] | token | Every role a region can stand in is citation-bearing except the one a machine or a schema established, and a structural reader states that role rather than leaving a recognizer to infer it from the text's shape. Display is still not a role: it enters no region at all. |

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use regex::Regex;

use crate::comment::{LEADERS, comment_regions};
use crate::commentary::Reader;

/// The leaders a dash-comment line resolves away.
const DASH_LEADERS: &[char] = &['-', ' ', '\t'];

/// The leaders an angle-bracket comment's own lines resolve away.
const ANGLE_LEADERS: &[char] = &[' ', '\t'];

/// What may stand between two solidus or hash comments and still let them join.
const SLASH_BETWEEN: &[char] = &['\n', ' ', '\t', '/', '!', '#'];

/// What may stand between two dash comments and still let them join.
const DASH_BETWEEN: &[char] = &['\n', ' ', '\t', '-'];

/// What may stand between two angle-bracket comments and still let them join.
const ANGLE_BETWEEN: &[char] = &['\n', ' ', '\t', '<', '>', '!', '-'];

/// The mark a shell script opens a comment with.
///
/// It is the language's own, not this corpus's, and it stands here because the
/// script tree is a declared surface of the residual family and a surface is
/// read for the commentary its language defines
/// (´[EMBER-conv:migration:burn-surface-reading]´).
///
/// ´const:emberlinter:script-comment-mark´ (´[EMBER-alg:const:codepoint]´)
/// ´const:emberlinter:script-comment-mark-codepoint-u23´
const SCRIPT_COMMENT_MARK: char = '#';

/// The Markdown extensions every reader in this crate parses with.
///
/// The set is a property of the corpus's own dialect rather than of any one
/// reader, and it stands here because two readers used to declare it separately
/// and a third would have made three. A reader that parsed with a smaller set
/// would see a table's cells as paragraph text and report offsets into a
/// document nobody wrote.
fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

/// What a region is, structurally, to a rule that asks where a citation may be made.
///
/// Two rules read the same joined text and want different things from it. The
/// label calculus asks only whether a form stands in referring surface, and
/// every region here is that. The file-path citation policy asks a second
/// question — is this text a statement its author *made*, or a value a machine
/// or a schema *presents* — because a generated cell transcribing its source's
/// path is output data, and rewriting it to a label would be rewriting the
/// generator's output rather than anybody's prose. The role is the tokenization
/// layer's answer to that second question, carried beside the text so a rule
/// never has to guess it from the text's shape.
///
/// Display is still not a role. A fenced block, an indented block and a
/// double-backtick exhibit enter no region at all, exactly as before; a role
/// distinguishes kinds of region, not region from non-region.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Authored running text: Markdown prose, headings, list text and ordinary table prose.
    #[default]
    Prose,
    /// A Markdown link or image destination, which is a locator its author wrote.
    Destination,
    /// A single-backtick span, whose typography does not make its contents display.
    Span,
    /// A lexically recognized code comment.
    Comment,
    /// A value a generator or a registered table schema presents as a path.
    PathValue,
}

impl Role {
    /// The role's identifier, as a machine finding spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prose => "prose",
            Self::Destination => "destination",
            Self::Span => "span",
            Self::Comment => "comment",
            Self::PathValue => "path_value",
        }
    }

    /// Whether a citation may be *made* in a region of this role.
    ///
    /// Only the path-valued role is not: a machine wrote it, and the policy that
    /// reads roles governs what authors write. Authored prose standing around a
    /// path-valued region is its own region and stays governed.
    #[must_use]
    pub const fn is_citation_bearing(self) -> bool {
        !matches!(self, Self::PathValue)
    }
}

/// A run of referring text read as one region, with offsets back into the source.
///
/// Joining is what makes a reference wrapped across two comment lines one
/// reference. The pieces are joined by a single space because a comment's
/// leaders are resolved away before the pieces meet, and without a separator two
/// halves of a reference would be glued into one word that no rule reads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Region {
    text: String,
    /// Where each piece begins, in the joined text and in the source.
    pieces: Vec<(usize, usize)>,
    /// What the region is, structurally, to a rule that reads roles.
    role: Role,
}

impl Region {
    /// Add one piece of source text, separated from the piece before it.
    fn push(&mut self, source: usize, piece: &str) {
        if !self.text.is_empty() {
            self.text.push(' ');
        }

        self.pieces.push((self.text.len(), source));
        self.text.push_str(piece);
    }

    /// The joined text of the region, which is what a rule reads.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where an offset into the joined text stands in the source.
    ///
    /// An occurrence always opens inside a piece rather than on a separator,
    /// because every shape any rule reads opens with a character no separator
    /// carries.
    #[must_use]
    pub fn source_offset(&self, at: usize) -> usize {
        self.pieces
            .iter()
            .rev()
            .find(|(joined, _source)| *joined <= at)
            .map_or(at, |(joined, source)| source + (at - joined))
    }

    /// Whether the region holds no text at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// What the region is to a rule that reads roles.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Whether the whole region stands inside one structural source range.
    ///
    /// A generator and a registered table schema know their source boundaries;
    /// the region reader knows how joined text maps back to those bytes. Keeping
    /// the containment question here lets the path router consume both facts
    /// without reconstructing either one from prose.
    #[must_use]
    #[allow(dead_code)] // Justified: the shadow router lands before its policy integration.
    pub(crate) fn stands_within(&self, start: usize, end: usize) -> bool {
        let Some(last) = self.text.len().checked_sub(1) else {
            return false;
        };
        let first = self.source_offset(0);
        let past_last = self.source_offset(last) + 1;

        first >= start && past_last <= end
    }

    /// The same region, restated in the role a structural reader has established for it.
    ///
    /// A generator or a registered table schema knows a region's bounds before
    /// any recognizer reads its text, and this is how that structural fact
    /// reaches the recognizer: as the region's role, decided by the reader that
    /// had the schema, rather than as a shape the recognizer has to guess.
    #[must_use]
    pub const fn in_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

/// One inline code span: its interior, where it opened, and whether it displays.
///
/// The superseded citation syntax put a tag in a single-backtick span, so a span
/// opened with one backtick participates and a span opened with more exhibits
/// what it holds. That is a fact about the Markdown rather than about tags,
/// which is why the count is resolved here and the rule is handed the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSpan {
    interior: String,
    offset: usize,
    interior_offset: usize,
    displayed: bool,
}

impl CodeSpan {
    /// The span's interior text, with its own line breaks already resolved.
    #[must_use]
    pub fn interior(&self) -> &str {
        &self.interior
    }

    /// Where the span opens in the source.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Whether the span shows what it holds rather than citing it.
    #[must_use]
    pub const fn displayed(&self) -> bool {
        self.displayed
    }
}

/// Every region of referring text in a Markdown document, in the order they stand.
///
/// Running text is joined across a soft line break, because a soft break
/// separates two lines of one sentence rather than two sentences. Anything else
/// ends the region: a code span, an emphasis boundary or a block edge all
/// interrupt the text a reference could be written in. Nothing inside a fenced
/// or indented code block enters any region at all.
#[must_use]
pub fn markdown_regions(source: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut depth = 0_usize;
    let mut run = Region::default();

    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                depth += 1;
            }
            Event::End(TagEnd::CodeBlock) => depth = depth.saturating_sub(1),
            Event::Text(_) if depth == 0 => run.push(range.start, &source[range]),
            // A soft break separates two lines of one sentence, so the run
            // continues; the separator the join adds is the line break itself.
            Event::SoftBreak => {}
            _ => flush(&mut run, &mut regions),
        }
    }

    flush(&mut run, &mut regions);

    regions
}

/// Every inline code span of a Markdown document, in the order they stand.
///
/// A span inside a fenced or indented block is part of that block's display and
/// is not a span at all here, which is the same boundary the regions keep.
#[must_use]
pub fn markdown_code_spans(source: &str) -> Vec<CodeSpan> {
    let mut spans = Vec::new();
    let mut depth = 0_usize;

    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                depth += 1;
            }
            Event::End(TagEnd::CodeBlock) => depth = depth.saturating_sub(1),
            Event::Code(interior) if depth == 0 => {
                let offset = range.start;
                let raw = &source[range];
                let opening = opening_backticks(raw);
                let displayed = opening != 1;
                let interior_offset = raw
                    .get(opening..raw.len().saturating_sub(opening))
                    .and_then(|value| value.find(interior.as_ref()))
                    .map_or(offset + opening, |at| offset + opening + at);

                spans.push(CodeSpan {
                    interior: interior.into_string(),
                    offset,
                    interior_offset,
                    displayed,
                });
            }
            _ => {}
        }
    }

    spans
}

/// Count the backticks opening a code span's source range.
fn opening_backticks(raw: &str) -> usize {
    raw.bytes().take_while(|byte| *byte == b'`').count()
}

/// The single-backtick spans that remain technical prose for the path policy.
///
/// Typography alone does not exempt a concrete filename. A whole value that
/// parses as a glob, a regular expression or TOML data is different: the span
/// presents input data rather than making a citation, so it contributes no
/// region. Double-backtick exhibits likewise contribute no region.
fn markdown_span_regions(source: &str) -> Vec<Region> {
    markdown_code_spans(source)
        .into_iter()
        .filter(|span| !span.displayed() && !is_complete_inline_data(span.interior()))
        .map(|span| {
            let mut region = Region {
                role: Role::Span,
                ..Region::default()
            };

            region.push(span.interior_offset, span.interior());
            region
        })
        .collect()
}

/// Whether a whole single-backtick value is parsed data rather than technical prose.
fn is_complete_inline_data(value: &str) -> bool {
    is_complete_configuration(value) || is_complete_glob(value) || is_complete_regex(value)
}

/// Whether the whole value is a TOML row or value.
fn is_complete_configuration(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    if toml::from_str::<toml::Table>(value).is_ok_and(|table| !table.is_empty()) {
        return true;
    }

    let mut row = String::from("value = ");
    row.push_str(value);

    toml::from_str::<toml::Table>(&row).is_ok()
}

/// Whether the whole value is a small, complete shell-style glob.
///
/// The policy needs only the data boundary, not a matcher. A glob therefore has
/// to carry an unmistakable metacharacter, balance every bracket class and carry
/// no whitespace that would make it several shell words. Escapes quote the next
/// character and a trailing escape is incomplete.
fn is_complete_glob(value: &str) -> bool {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return false;
    }

    let mut characters = value.chars();
    let mut metacharacter = false;

    while let Some(character) = characters.next() {
        match character {
            '\\' => {
                if characters.next().is_none() {
                    return false;
                }
            }
            '*' | '?' => metacharacter = true,
            '[' => {
                let mut held = false;
                let mut closed = false;

                for member in characters.by_ref() {
                    if member == ']' && held {
                        closed = true;
                        break;
                    }

                    held = true;
                }

                if !closed {
                    return false;
                }

                metacharacter = true;
            }
            _ => {}
        }
    }

    metacharacter
}

/// Whether the whole value is a regular expression with unmistakable syntax.
///
/// The engine is asked directly rather than through a declared-pattern type. The
/// question here is whether a token a comment carries is a regular expression at
/// all, which is the engine's own question about the text; the declared surface's
/// pattern language is a separate matter and no longer this one's.
fn is_complete_regex(value: &str) -> bool {
    let signalled = value.starts_with('^')
        || value.ends_with('$')
        || value.contains('\\')
        || value
            .chars()
            .any(|character| matches!(character, '(' | ')' | '|' | '{' | '}'));

    signalled && Regex::new(value).is_ok()
}

/// Every link and image destination of a Markdown document, in the order they stand.
///
/// A destination is a locator its author wrote, so it is referring surface and
/// not display: the visible prose of a link says what the target is, and the
/// destination says where it stands. A rule that read only the visible prose
/// would let the whole of a document's linking escape it.
///
/// Each destination is its own region rather than part of the paragraph's run,
/// because it is a separate role and joining it into the surrounding sentence
/// would lose that. The offset is the destination's own where the source spells
/// it verbatim, which is the ordinary inline case; where the reader has
/// normalized the text away from the bytes — a reference definition resolved
/// elsewhere, an entity or an escape decoded — the offset falls back to where
/// the link opens, because a real offset into the wrong end of the link is
/// better than an offset into a string nobody wrote.
#[must_use]
pub fn markdown_destinations(source: &str) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut depth = 0_usize;

    for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        let destination = match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented)) => {
                depth += 1;
                continue;
            }
            Event::End(TagEnd::CodeBlock) => {
                depth = depth.saturating_sub(1);
                continue;
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. })
                if depth == 0 =>
            {
                dest_url
            }
            _ => continue,
        };

        if destination.is_empty() {
            continue;
        }

        let offset = source
            .get(range.clone())
            .and_then(|link| link.find(destination.as_ref()))
            .map_or(range.start, |at| range.start + at);

        let mut region = Region {
            role: Role::Destination,
            ..Region::default()
        };

        region.push(offset, destination.as_ref());
        regions.push(region);
    }

    regions
}

/// Every citation-bearing region of a source, chosen by the reader its kind is catalogued with.
///
/// This is the one entry point a rule reaching every carrier needs. The catalog
/// answers what reads a kind; this answers with the regions. A kind classified
/// as carrying no region at all yields none, which is a different outcome from a
/// kind nobody has classified: the catalog returns nothing at all for that one,
/// and the caller reports it.
///
/// A prose document yields its running text and its link destinations together,
/// ordered by where they stand in the source, because a document's regions
/// arriving out of order would make a rule's findings arrive out of order too.
#[must_use]
pub fn regions(reader: Reader, source: &str) -> Vec<Region> {
    match reader {
        Reader::Prose => {
            let mut found = markdown_regions(source);

            found.extend(markdown_destinations(source));
            found.extend(markdown_span_regions(source));
            found.sort_by_key(|region| region.source_offset(0));
            found
        }
        Reader::Slash => rust_regions(source),
        Reader::Hash => script_regions(source),
        Reader::Dash => sql_regions(source),
        Reader::Angle => angle_regions(source),
        Reader::Opaque => Vec::new(),
    }
}

/// Every region of commentary in a Rust source, in the order they stand.
///
/// Adjacent comment lines are read as one region with their leaders resolved
/// away. Only the comments are read: a form in a string literal is data the
/// program carries rather than a reference the corpus makes, and one in an
/// identifier is a name only renaming could retire.
///
/// A C source is read by this reader too. The lexical grammar it separates —
/// solidus line and block comments, quoted strings, character literals — is a
/// superset of C's, and the parts that are Rust's alone are prefixes and hash
/// runs that no C source spells.
#[must_use]
pub fn rust_regions(source: &str) -> Vec<Region> {
    let spans: Vec<(usize, usize)> = comment_regions(source)
        .into_iter()
        .map(|region| (region.start(), region.end()))
        .collect();

    joined(source, &spans, LEADERS, SLASH_BETWEEN, Role::Comment)
}

/// Every region of commentary in a shell script, in the order they stand.
///
/// A script is read exactly as a Rust source is, for the same reason: a
/// provenance comment is a reference the corpus makes, and a form inside a
/// quoted word is data the script carries.
///
/// Every hash-commented kind is read by this reader: a TOML or YAML
/// declaration, a make or container recipe, and the dot-files that carry
/// ignore lists. The quotation rule it enforces is the shell's, which is the
/// strictest of the family in the direction that matters — it keeps quoted data
/// out — and the places the others differ from it are places they quote *more*
/// rather than less.
#[must_use]
pub fn script_regions(source: &str) -> Vec<Region> {
    joined(
        source,
        &script_comment_spans(source),
        LEADERS,
        SLASH_BETWEEN,
        Role::Comment,
    )
}

/// Every region of commentary in a SQL source, in the order they stand.
///
/// The dash pair opens a comment outside quotation and nowhere else, so a pair
/// inside a quoted string or a quoted identifier is data the statement carries.
/// A doubled quote is how the language escapes a quote inside its own kind of
/// quotation, and it needs no special case: closing and reopening leaves the
/// scanner exactly where doubling means it should be.
///
/// Block comments are not read. The migration corpus spells its SQL commentary
/// with the dash pair, the ruling that reaches comments names the dash form,
/// and a reader claiming a form it has not implemented would be worse than one
/// that says which form it reads.
#[must_use]
pub fn sql_regions(source: &str) -> Vec<Region> {
    joined(
        source,
        &dash_comment_spans(source),
        DASH_LEADERS,
        DASH_BETWEEN,
        Role::Comment,
    )
}

/// Every region of commentary in a markup document, in the order they stand.
///
/// The delimited form is the whole of the language's comment syntax, so there is
/// no line form to join and no quotation to step over: everything between the
/// opener and the closer is commentary, and everything else is markup. A comment
/// nobody closed runs to the end of the document, which is what a parser reading
/// the same bytes would do.
#[must_use]
pub fn angle_regions(source: &str) -> Vec<Region> {
    joined(
        source,
        &angle_comment_spans(source),
        ANGLE_LEADERS,
        ANGLE_BETWEEN,
        Role::Comment,
    )
}

/// Join adjacent comment spans into regions, in the order they stand.
///
/// Two spans join when nothing but a line break, indentation and a comment
/// opener stands between them. Anything else — a blank line, a line of program
/// text, an attribute — ends the region, because a reference does not wrap
/// across something that is not commentary. Joining is transitive over the run,
/// so a reference spread over three lines is as visible as one spread over two.
fn joined(
    source: &str,
    spans: &[(usize, usize)],
    leaders: &[char],
    between: &[char],
    role: Role,
) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut run = Region {
        role,
        ..Region::default()
    };
    let mut previous_end: Option<usize> = None;

    for (start, end) in spans.iter().copied() {
        if previous_end.is_some_and(|last| !adjoins(&source[last..start], between)) {
            flush(&mut run, &mut regions);
            run.role = role;
        }

        // A span may itself cover several lines, as a block comment does, so
        // each of its lines is pushed with its own leaders resolved away.
        let mut line_start = start;

        for line in source[start..end].split('\n') {
            let trimmed = line.trim_start_matches(leaders);

            run.push(line_start + (line.len() - trimmed.len()), trimmed);
            line_start += line.len() + 1;
        }

        previous_end = Some(end);
    }

    flush(&mut run, &mut regions);

    regions
}

/// Whether the text between two comments lets the second continue the first.
///
/// The bytes a language may write between one comment and the next are the
/// language's own — a solidus run, a hash, a dash pair, an angle-bracket pair —
/// so the caller supplies them and this decides only the shape of the rule: one
/// line break, and nothing that is not the opener.
fn adjoins(between: &str, allowed: &[char]) -> bool {
    between
        .chars()
        .filter(|character| *character == '\n')
        .count()
        == 1
        && between
            .chars()
            .all(|character| allowed.contains(&character))
}

/// Close the region under construction and start a fresh one.
fn flush(run: &mut Region, regions: &mut Vec<Region>) {
    if !run.is_empty() {
        regions.push(std::mem::take(run));
    }
}

/// Every comment of a shell script, as byte ranges into the source.
///
/// The mark opens a comment only where a word does not continue — a mark inside
/// a word is that word's, which is what keeps a parameter expansion and a
/// fragment identifier out — and only outside quotation. Single quotes carry
/// everything literally; double quotes admit a backslash escape. Nothing else of
/// the language is read, because nothing else changes where a comment begins.
fn script_comment_spans(source: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut single = false;
    let mut double = false;
    let mut previous = None;
    let mut indices = source.char_indices();

    while let Some((offset, character)) = indices.next() {
        match character {
            '\\' if double => {
                let _escaped = indices.next();
                previous = None;
                continue;
            }
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            SCRIPT_COMMENT_MARK
                if !single && !double && previous.is_none_or(char::is_whitespace) =>
            {
                let start = offset + SCRIPT_COMMENT_MARK.len_utf8();
                let end = source[start..]
                    .find('\n')
                    .map_or(source.len(), |at| start + at);

                spans.push((start, end));

                // The scan resumes after the line, because the whole of a
                // comment is text and holds nothing that opens anything.
                while indices.next().is_some_and(|(at, _character)| at + 1 < end) {}

                previous = Some('\n');
                continue;
            }
            _ => {}
        }

        previous = Some(character);
    }

    spans
}

/// Every dash comment of a SQL source, as byte ranges into the source.
///
/// The pair opens a comment outside quotation only. Both quotation forms are
/// tracked — the string's and the delimited identifier's — and a doubled quote
/// inside one of them closes and reopens it, which is where the language's own
/// escape leaves the scanner anyway. Nothing else of the language is read,
/// because nothing else changes where a comment begins.
fn dash_comment_spans(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut single = false;
    let mut double = false;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'-' if !single && !double && bytes.get(index + 1) == Some(&b'-') => {
                let start = index + 2;
                let end = source[start..]
                    .find('\n')
                    .map_or(source.len(), |at| start + at);

                spans.push((start, end));
                index = end;
                continue;
            }
            _ => {}
        }

        index += 1;
    }

    spans
}

/// The bytes a markup comment opens with.
const ANGLE_OPENER: &str = "<!--";

/// The bytes a markup comment closes with.
const ANGLE_CLOSER: &str = "-->";

/// Every delimited comment of a markup document, as byte ranges into the source.
///
/// The interior is what stands between the delimiters. A comment nobody closed
/// runs to the end of the document rather than aborting the scan, so a document
/// that is not well formed is still read for the commentary it does carry.
fn angle_comment_spans(source: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;

    while let Some(at) = source[from..].find(ANGLE_OPENER) {
        let start = from + at + ANGLE_OPENER.len();
        let end = source[start..]
            .find(ANGLE_CLOSER)
            .map_or(source.len(), |to| start + to);

        spans.push((start, end));
        from = (end + ANGLE_CLOSER.len()).min(source.len());
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::{
        Region, Role, angle_regions, markdown_code_spans, markdown_destinations, markdown_regions,
        rust_regions, script_regions, sql_regions,
    };
    use crate::commentary::Reader;

    fn texts(regions: &[Region]) -> Vec<&str> {
        regions.iter().map(Region::text).collect()
    }

    /// A run of adjacent comment lines is one region with its leaders resolved
    /// away, so a reference wrapped across the boundary is one run of text.
    /// Anything that is not commentary between two comments ends the run,
    /// because a reference does not wrap across program text.
    ///
    /// ´claim:token:adjacent-comment-lines-are-one-region´
    /// ´test:unit:joins-adjacent-comment-lines-into-one-region´
    #[test]
    fn joins_adjacent_comment_lines_into_one_region() {
        let wrapped = "/// the \u{a7}SPEC\n/// Q-16.4.2 formula binds.\n";

        assert_eq!(
            texts(&rust_regions(wrapped)),
            ["the \u{a7}SPEC Q-16.4.2 formula binds."]
        );

        let apart = "/// the \u{a7}SPEC\nlet binding = 1;\n/// Q-16.4.2 formula binds.\n";

        assert_eq!(
            texts(&rust_regions(apart)),
            ["the \u{a7}SPEC", "Q-16.4.2 formula binds."],
            "program text between two comments ends the region"
        );

        let blank = "// first\n\n// second\n";

        assert_eq!(
            texts(&rust_regions(blank)),
            ["first", "second"],
            "a blank line ends the region too"
        );
    }

    /// Joining is not a two-line rule. A run of adjacent comment lines is one
    /// region however long it is, so a reference spread over three lines is as
    /// visible as one spread over two.
    ///
    /// ´claim:token:a-region-joins-however-many-lines-it-spans´
    /// ´test:unit:joins-a-region-across-however-many-lines-it-wraps´
    #[test]
    fn joins_a_region_across_however_many_lines_it_wraps() {
        let thrice = "/// opening the\n/// \u{a7}SPEC\n/// Q-16.4.2 formula.\n";

        assert_eq!(
            texts(&rust_regions(thrice)),
            ["opening the \u{a7}SPEC Q-16.4.2 formula."]
        );

        let block = "/* opening the\n * \u{a7}SPEC\n * Q-16.4.2 formula. */\n";

        assert_eq!(
            texts(&rust_regions(block)),
            ["opening the \u{a7}SPEC Q-16.4.2 formula. "],
            "a block comment's own lines join by the same rule"
        );
    }

    /// An offset into a region's joined text maps back to the offset in the
    /// source the piece came from, so a rule reading a region still reports
    /// where in the file the form stands.
    ///
    /// ´claim:token:a-joined-offset-maps-back-to-the-source´
    /// ´test:unit:maps-a-joined-offset-back-to-its-source´
    #[test]
    fn maps_a_joined_offset_back_to_its_source() {
        let source = "// a first line\n// and then WP-8 here\n";
        let regions = rust_regions(source);

        assert_eq!(regions.len(), 1);

        let region = &regions[0];
        let at = region
            .text()
            .find("WP-8")
            .expect("the form stands in the joined text");

        assert_eq!(
            &source[region.source_offset(at)..region.source_offset(at) + 4],
            "WP-8",
            "the mapped offset lands on the form in the source"
        );
    }

    /// Markdown running text is one region across a soft line break and is
    /// interrupted by anything else, which is the paragraph-joining semantics
    /// the prose surfaces already had.
    ///
    /// ´claim:token:markdown-running-text-joins-across-a-soft-break´
    /// ´test:unit:joins-markdown-running-text-across-a-soft-break´
    #[test]
    fn joins_markdown_running_text_across_a_soft_break() {
        assert_eq!(
            texts(&markdown_regions(
                "As the \u{a7}SPEC\nQ-16.4.2 formula requires.\n"
            )),
            ["As the \u{a7}SPEC Q-16.4.2 formula requires."]
        );
        assert_eq!(
            texts(&markdown_regions("One paragraph.\n\nAnother one.\n")),
            ["One paragraph.", "Another one."],
            "a block edge ends the region"
        );
        assert_eq!(
            texts(&markdown_regions("An *emphasised* word.\n")),
            ["An ", "emphasised", " word."],
            "an emphasis boundary ends it as well"
        );
    }

    /// Inline code, a double-backtick exhibit and a fenced block enter no
    /// Markdown running-text region. The path-policy reader routes eligible
    /// single-backtick spans separately, while the displayed forms remain
    /// absent everywhere.
    ///
    /// ´claim:token:display-never-enters-a-region´
    /// ´test:unit:keeps-displayed-markdown-out-of-every-region´
    #[test]
    fn keeps_displayed_markdown_out_of_every_region() {
        let displayed =
            "Shown: `\u{a7}10.3` and ``ADR-008``.\n\n```text\n\u{a7}4.2 and WP-8\n```\n";

        for region in markdown_regions(displayed) {
            for form in ["\u{a7}10.3", "ADR-008", "\u{a7}4.2", "WP-8"] {
                assert!(
                    !region.text().contains(form),
                    "displayed {form} entered a region"
                );
            }
        }
    }

    /// A shell script's comments join by the same rule, and the mark opens a
    /// comment only where a word does not continue and only outside quotation,
    /// so a form in a quoted word is data the script carries.
    ///
    /// ´claim:token:a-script-is-read-for-its-comments-alone´
    /// ´test:unit:joins-script-comments-and-leaves-quoted-text-alone´
    #[test]
    fn joins_script_comments_and_leaves_quoted_text_alone() {
        let script = "#!/bin/sh\n# built under the \u{a7}SPEC\n# Q-16.4.2 formula.\necho \"nothing of WP-2 here\"\n";

        assert_eq!(
            texts(&script_regions(script)),
            ["bin/sh built under the \u{a7}SPEC Q-16.4.2 formula."],
            "the shebang line is a comment to this reading and joins the run, its own leaders resolved away with the rest"
        );

        let quoted = "echo 'a # mark inside quotes opens nothing'\necho \"nor here\"\n";

        assert_eq!(texts(&script_regions(quoted)), Vec::<&str>::new());
    }

    /// An inline code span is reported with its interior and with whether it was
    /// displayed, which is the backtick count and nothing else. A span wrapped
    /// across a line arrives as one interior, so a form written inside one is
    /// one form.
    ///
    /// ´claim:token:a-code-span-carries-its-interior-and-its-display´
    /// ´test:unit:reads-code-spans-with-the-backtick-count-that-opened-them´
    #[test]
    fn reads_code_spans_with_the_backtick_count_that_opened_them() {
        let spans = markdown_code_spans("Cites `land:rigid` and shows ``land:rigid``.\n");

        assert_eq!(
            spans
                .iter()
                .map(|span| (span.interior(), span.displayed()))
                .collect::<Vec<_>>(),
            [("land:rigid", false), ("land:rigid", true)]
        );

        let wrapped = markdown_code_spans("A tag `run:update\n-path#13` wrapped.\n");

        assert_eq!(
            wrapped
                .iter()
                .map(super::CodeSpan::interior)
                .collect::<Vec<_>>(),
            ["run:update -path#13"],
            "the Markdown reader resolves a span's own line break before this module sees it"
        );

        assert!(
            markdown_code_spans("Text.\n\n```text\n`land:rigid`\n```\n").is_empty(),
            "a span inside a fenced block is part of that block's display"
        );
    }

    /// A single-backtick span carrying a concrete filename enters the path
    /// surface in the span role, while double-backtick exhibits and complete
    /// glob, regular-expression and configuration values contribute no region.
    /// An incomplete configuration value remains technical prose and therefore
    /// participates.
    ///
    /// ´claim:token:a-single-backtick-concrete-value-is-technical-prose´
    /// ´test:unit:routes-a-naked-single-backtick-value-and-omits-complete-data´
    #[test]
    fn routes_a_naked_single_backtick_value_and_omits_complete_data() {
        let source = concat!(
            "`orchard/plum.rs` ",
            "``orchard/quince.rs`` ",
            "`orchard/*.rs` ",
            "`^orchard/.*[.]rs$` ",
            "`path = \"orchard/quince.rs\"` ",
            "`path = orchard/quince.rs`\n",
        );
        let found = super::markdown_span_regions(source);

        assert_eq!(
            found
                .iter()
                .map(|region| (region.text(), region.role()))
                .collect::<Vec<_>>(),
            [
                ("orchard/plum.rs", Role::Span),
                ("path = orchard/quince.rs", Role::Span),
            ],
            "only a naked concrete value and an incomplete data value participate"
        );

        let at = found[0].source_offset(0);

        assert_eq!(
            &source[at..at + "orchard/plum.rs".len()],
            "orchard/plum.rs",
            "the region opens on the span interior rather than its backtick"
        );
    }

    /// A link or image destination is a region of its own, in the destination
    /// role, and its offset lands on the destination in the source rather than
    /// on the link that carries it. A destination inside a fenced block is part
    /// of that block's display and is no destination at all, which is the same
    /// boundary every other reading here keeps.
    ///
    /// ´claim:token:a-link-destination-is-a-region-in-its-own-role´
    /// ´test:unit:reads-a-link-destination-as-its-own-region´
    #[test]
    fn reads_a_link_destination_as_its_own_region() {
        let source = "See [the plum](orchard/plum.md) and ![a fig](orchard/fig.png).\n";
        let found = markdown_destinations(source);

        assert_eq!(texts(&found), ["orchard/plum.md", "orchard/fig.png"]);
        assert!(
            found
                .iter()
                .all(|region| region.role() == Role::Destination),
            "a destination stands in the destination role"
        );

        let at = found[0].source_offset(0);

        assert_eq!(
            &source[at..at + "orchard/plum.md".len()],
            "orchard/plum.md",
            "the offset lands on the destination itself"
        );
        assert!(
            markdown_destinations("Text.\n\n```text\n[a plum](orchard/plum.md)\n```\n").is_empty(),
            "a destination inside a fenced block is part of that block's display"
        );
    }

    /// A dash pair opens a comment outside quotation and nowhere else, so a pair
    /// inside a quoted string or a quoted identifier is data the statement
    /// carries. Adjacent dash comments join by the same rule every other
    /// commentary joins by.
    ///
    /// ´claim:token:a-dash-comment-opens-outside-quotation-alone´
    /// ´test:unit:reads-dash-comments-without-reading-their-quoted-data´
    #[test]
    fn reads_dash_comments_without_reading_their_quoted_data() {
        let statement = "-- the plum\n-- and the quince\nSELECT 1;\n";

        assert_eq!(texts(&sql_regions(statement)), ["the plum and the quince"]);

        let quoted = "SELECT '-- not a comment', \"-- nor this\" FROM basket;\n";

        assert_eq!(
            texts(&sql_regions(quoted)),
            Vec::<&str>::new(),
            "a dash pair inside either quotation form opens nothing"
        );

        let doubled = "SELECT 'it''s quoted' FROM basket;\n-- the quince\n";

        assert_eq!(
            texts(&sql_regions(doubled)),
            ["the quince"],
            "a doubled quote leaves the scanner where the language's own escape means it should be"
        );
    }

    /// The delimited markup form is the whole of that language's comment syntax:
    /// everything between the delimiters is commentary and everything else is
    /// markup, and a comment nobody closed runs to the end of the document
    /// rather than aborting the scan.
    ///
    /// ´claim:token:a-delimited-markup-comment-runs-to-its-closer-or-to-the-end´
    /// ´test:unit:reads-delimited-markup-comments-and-an-unclosed-one-to-the-end´
    #[test]
    fn reads_delimited_markup_comments_and_an_unclosed_one_to_the_end() {
        let document = "<!-- the plum-->\nvisible markup\n<!-- the quince-->\n";

        assert_eq!(
            texts(&angle_regions(document)),
            ["the plum", "the quince"],
            "markup between two comments ends the region"
        );

        let unclosed = "<!-- the plum\nand the quince";

        assert_eq!(
            texts(&angle_regions(unclosed)),
            ["the plum and the quince"],
            "a comment nobody closed runs to the end of the document"
        );
    }

    /// A rule reaching every carrier asks for regions by the reader its kind is
    /// catalogued with, and gets them. A kind classified as carrying no region
    /// yields none — which is the classified answer and not the unclassified one
    /// — and a prose document yields its running text and its destinations
    /// together, ordered by where they stand rather than by which reader found
    /// them.
    ///
    /// ´claim:token:regions-are-asked-for-by-the-catalogued-reader´
    /// ´test:unit:hands-a-rule-the-regions-of-the-kind-it-was-catalogued-with´
    #[test]
    fn hands_a_rule_the_regions_of_the_kind_it_was_catalogued_with() {
        assert_eq!(
            texts(&super::regions(
                Reader::Opaque,
                "orchard/plum.md stands here"
            )),
            Vec::<&str>::new(),
            "a kind classified as carrying no region yields none"
        );
        assert_eq!(
            texts(&super::regions(
                Reader::Prose,
                "See [the plum](orchard/plum.md) now.\n"
            )),
            ["See ", "the plum", "orchard/plum.md", " now."],
            "a document's prose and its destinations arrive in the order they stand"
        );
        assert_eq!(
            texts(&super::regions(Reader::Slash, "// the plum\n")),
            ["the plum"],
            "a solidus carrier is read for its comments"
        );
        assert_eq!(
            texts(&super::regions(Reader::Hash, "# the plum\n")),
            ["the plum"],
            "a hash carrier is read for its comments"
        );
    }

    /// Every role a region can stand in is citation-bearing except the one a
    /// machine or a schema established, and a structural reader states that role
    /// rather than leaving a recognizer to infer it from the text's shape.
    /// Display is still not a role: it enters no region at all.
    ///
    /// ´claim:token:only-a-path-valued-region-is-not-citation-bearing´
    /// ´test:unit:reads-every-role-as-citation-bearing-but-the-path-valued-one´
    #[test]
    fn reads_every_role_as_citation_bearing_but_the_path_valued_one() {
        for role in [Role::Prose, Role::Destination, Role::Span, Role::Comment] {
            assert!(
                role.is_citation_bearing(),
                "{role:?} is surface a citation is made in"
            );
        }

        assert!(
            !Role::PathValue.is_citation_bearing(),
            "a value a generator or a schema presents is not a citation anybody made"
        );

        let established = markdown_regions("orchard/plum.md\n")
            .pop()
            .expect("the paragraph is a region")
            .in_role(Role::PathValue);

        assert_eq!(established.role(), Role::PathValue);
        assert_eq!(
            established.text(),
            "orchard/plum.md",
            "restating the role leaves the text and its offsets alone"
        );
    }
}
