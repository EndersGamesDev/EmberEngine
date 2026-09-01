//! Convert `pulldown-cmark` `Event`s back to the string they were parsed from.
//!
//! This crate provides functions to serialize markdown events back into markdown text format.
//!
//! # Examples
//!
//! ```rust
//! use pulldown_cmark::Parser;
//! use pulldown_cmark_to_cmark::cmark;
//!
//! let input_markdown = "# Hello\n\nWorld!";
//! let events = Parser::new(input_markdown);
//! let mut output_markdown = String::new();
//! cmark(events, &mut output_markdown).unwrap();
//! assert_eq!(output_markdown, input_markdown);
//! ```

#![deny(rust_2018_idioms)]
#![deny(missing_docs)]

use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt,
    ops::Range,
};

use pulldown_cmark::{
    Alignment as TableAlignment, BlockQuoteKind, Event, LinkType, MetadataBlockKind, Tag, TagEnd,
};

mod source_range;
mod text_modifications;

pub use source_range::{
    cmark_resume_with_source_range, cmark_resume_with_source_range_and_options,
    cmark_with_source_range, cmark_with_source_range_and_options,
};
use text_modifications::{
    EscapeLinkLabel, Repeated, close_link, consume_newlines, escape_special_characters,
    list_item_padding_of, max_consecutive_chars, padding, print_text_without_trailing_newline,
    write_padded_newline,
};

/// Similar to [Pulldown-Cmark-Alignment][Alignment], but with required
/// traits for comparison to allow testing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alignment {
    /// No alignment specified
    None,
    /// Left-aligned
    Left,
    /// Center-aligned
    Center,
    /// Right-aligned
    Right,
}

impl From<&TableAlignment> for Alignment {
    fn from(s: &TableAlignment) -> Self {
        match *s {
            TableAlignment::None => Self::None,
            TableAlignment::Left => Self::Left,
            TableAlignment::Center => Self::Center,
            TableAlignment::Right => Self::Right,
        }
    }
}

/// The kind of code block being serialized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodeBlockKind {
    /// An indented code block (4 spaces or 1 tab)
    Indented,
    /// A fenced code block (delimited by backticks or tildes)
    Fenced,
}

/// The state of the [`cmark_resume()`] and [`cmark_resume_with_options()`] functions.
///
/// This does not only allow introspection, but enables the user
/// to halt the serialization at any time, and resume it later.
#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub struct State<'a> {
    /// The amount of newlines to insert after `Event::Start(...)`
    pub newlines_before_start: usize,
    /// The lists and their types for which we have seen a `Event::Start(List(...))` tag
    pub list_stack: Vec<Option<u64>>,
    /// The computed padding and prefix to print after each newline.
    /// This changes with the level of `BlockQuote` and `List` events.
    pub padding: Vec<Cow<'a, str>>,
    /// Keeps the current table alignments, if we are currently serializing a table.
    pub table_alignments: Vec<Alignment>,
    /// Keeps the current table headers, if we are currently serializing a table.
    pub table_headers: Vec<String>,
    /// The last seen text when serializing a header
    pub text_for_header: Option<String>,
    /// Is set while we are handling text in a code block
    pub code_block: Option<CodeBlockKind>,
    /// True if the last event was text and the text does not have trailing newline. Used to inject additional newlines before code block end fence.
    pub last_was_text_without_trailing_newline: bool,
    /// True if the last event was a paragraph start. Used to escape spaces at start of line (prevent spurrious indented code).
    pub last_was_paragraph_start: bool,
    /// True if the next event is a link, image, or footnote.
    pub next_is_link_like: bool,
    /// Currently open links
    pub link_stack: Vec<LinkCategory<'a>>,
    /// Currently open images
    pub image_stack: Vec<ImageLink<'a>>,
    /// Keeps track of the last seen heading's id, classes, and attributes
    pub current_heading: Option<Heading<'a>>,
    /// Keeps track of the last seen shortcut/link
    pub current_shortcut_text: Option<String>,
    /// A list of shortcuts seen so far for later emission
    pub shortcuts: Vec<(String, String, String)>,
    /// Index into the `source` bytes of the end of the range corresponding to the last event.
    ///
    /// It's used to see if the current event didn't capture some bytes because of a
    /// skipped-over backslash.
    pub last_event_end_index: usize,
}

/// The category of link being serialized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkCategory<'a> {
    /// An autolink (e.g., `<http://example.com>`)
    AngleBracketed,
    /// A reference link with an explicit label (e.g., `[text][label]`)
    Reference {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The link title
        title: Cow<'a, str>,
        /// The reference identifier
        id: Cow<'a, str>,
    },
    /// A collapsed reference link (e.g., `[text][]`)
    Collapsed {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The link title
        title: Cow<'a, str>,
    },
    /// A shortcut reference link (e.g., `[text]`)
    Shortcut {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The link title
        title: Cow<'a, str>,
    },
    /// An inline link or other link type
    Other {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The link title
        title: Cow<'a, str>,
    },
}

/// The category of image link being serialized.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImageLink<'a> {
    /// A reference image with an explicit label (e.g., `![alt][label]`)
    Reference {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The image title
        title: Cow<'a, str>,
        /// The reference identifier
        id: Cow<'a, str>,
    },
    /// A collapsed reference image (e.g., `![alt][]`)
    Collapsed {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The image title
        title: Cow<'a, str>,
    },
    /// A shortcut reference image (e.g., `![alt]`)
    Shortcut {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The image title
        title: Cow<'a, str>,
    },
    /// An inline image or other image type
    Other {
        /// The destination URI
        uri: Cow<'a, str>,
        /// The image title
        title: Cow<'a, str>,
    },
}

/// Information about a heading's attributes (id, classes, and other attributes).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Heading<'a> {
    /// The heading's id attribute, or `None` if no id is specified
    id: Option<Cow<'a, str>>,
    /// The heading's CSS class attributes; empty if no classes are specified
    classes: Vec<Cow<'a, str>>,
    /// Other attributes as key-value pairs in the form (`attribute_name`, `optional_value`)
    attributes: Vec<(Cow<'a, str>, Option<Cow<'a, str>>)>,
}

/// The number of code-block tokens needed to produce a valid fenced code block.
pub const DEFAULT_CODE_BLOCK_TOKEN_COUNT: usize = 3;

/// Configuration for the [`cmark_with_options()`] and [`cmark_resume_with_options()`] functions.
///
/// The defaults should provide decent spacing and most importantly, will
/// provide a faithful rendering of your markdown document particularly when
/// rendering it to HTML.
///
/// It's best used with its `Options::default()` implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Options<'a> {
    /// The number of newlines to insert after a headline
    pub newlines_after_headline: usize,
    /// The number of newlines to insert after a paragraph
    pub newlines_after_paragraph: usize,
    /// The number of newlines to insert after a code block
    pub newlines_after_codeblock: usize,
    /// The number of newlines to insert after an HTML block
    pub newlines_after_htmlblock: usize,
    /// The number of newlines to insert after a table
    pub newlines_after_table: usize,
    /// The number of newlines to insert after a horizontal rule
    pub newlines_after_rule: usize,
    /// The number of newlines to insert after a list
    pub newlines_after_list: usize,
    /// The number of newlines to insert after a block quote
    pub newlines_after_blockquote: usize,
    /// The number of newlines to insert after other elements
    pub newlines_after_rest: usize,
    /// The amount of newlines placed after TOML or YAML metadata blocks at the beginning of a document.
    pub newlines_after_metadata: usize,
    /// Token count for fenced code block. An appropriate value of this field can be decided by
    /// [`calculate_code_block_token_count()`].
    ///
    /// Note that the default value is `4` which allows for one level of nested code-blocks,
    /// which is typically a safe value for common kinds of markdown documents.
    pub code_block_token_count: usize,
    /// The character to use for code block fences (backtick or tilde)
    pub code_block_token: char,
    /// The character to use for unordered list items
    pub list_token: char,
    /// The character to use after ordered list numbers (e.g., '.' for `1.`)
    pub ordered_list_token: char,
    /// Whether to increment the number for each ordered list item
    pub increment_ordered_list_bullets: bool,
    /// The character to use for emphasis (italic)
    pub emphasis_token: char,
    /// The string to use for strong emphasis (bold)
    pub strong_token: &'a str,
    /// If `true` (default) then use HTML tags `<sup>` and `<sub>`.
    /// If `false`, use the Markdown symbols `^` and `~` instead.
    ///
    /// If you use [`ENABLE_SUPERSCRIPT`](pulldown_cmark::Options::ENABLE_SUPERSCRIPT) and
    /// [`ENABLE_SUBSCRIPT`](pulldown_cmark::Options::ENABLE_SUBSCRIPT) when parsing, then
    /// you might need this in order to round-trip Markdown byte-for-byte, with knowledge
    /// of whether the parsed documents use `<sub>`/`<sup>` or `^`/`~` instead.
    pub use_html_for_super_sub_script: bool,
}

const DEFAULT_OPTIONS: Options<'_> = Options {
    newlines_after_headline: 2,
    newlines_after_paragraph: 2,
    newlines_after_codeblock: 2,
    newlines_after_htmlblock: 1,
    newlines_after_table: 2,
    newlines_after_rule: 2,
    newlines_after_list: 2,
    newlines_after_blockquote: 2,
    newlines_after_rest: 1,
    newlines_after_metadata: 1,
    code_block_token_count: 4,
    code_block_token: '`',
    list_token: '*',
    ordered_list_token: '.',
    increment_ordered_list_bullets: false,
    emphasis_token: '*',
    strong_token: "**",
    use_html_for_super_sub_script: true,
};

impl Default for Options<'_> {
    fn default() -> Self {
        DEFAULT_OPTIONS
    }
}

impl Options<'_> {
    /// Returns the set of special characters that need escaping based on the current options.
    #[must_use]
    pub fn special_characters(&self) -> Cow<'static, str> {
        // These always need to be escaped, even if reconfigured.
        const BASE: &str = "#\\_*<>`|[]";
        if DEFAULT_OPTIONS.code_block_token == self.code_block_token
            && DEFAULT_OPTIONS.list_token == self.list_token
            && DEFAULT_OPTIONS.emphasis_token == self.emphasis_token
            && DEFAULT_OPTIONS.strong_token == self.strong_token
        {
            BASE.into()
        } else {
            let mut s = String::from(BASE);
            s.push(self.code_block_token);
            s.push(self.list_token);
            s.push(self.emphasis_token);
            s.push_str(self.strong_token);
            s.into()
        }
    }
}

/// The error returned by [`cmark_resume_with_options()`] and
/// [`cmark_resume_with_source_range_and_options()`].
#[derive(Debug)]
pub enum Error {
    /// Formatting to the output writer failed
    FormatFailed(fmt::Error),
    /// An event was encountered that cannot be produced by valid markdown
    UnexpectedEvent,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormatFailed(e) => e.fmt(f),
            Self::UnexpectedEvent => f.write_str("Unexpected event while reconstructing Markdown"),
        }
    }
}

impl std::error::Error for Error {}

impl From<fmt::Error> for Error {
    fn from(e: fmt::Error) -> Self {
        Self::FormatFailed(e)
    }
}

/// As [`cmark_with_options()`], but with default [`Options`].
///
/// # Errors
///
/// Returns an error if writing to the formatter fails or the event stream cannot be represented
/// as valid Markdown.
pub fn cmark<'a, I, E, F>(events: I, mut formatter: F) -> Result<State<'a>, Error>
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
    F: fmt::Write,
{
    cmark_with_options(events, &mut formatter, Options::default())
}

/// As [`cmark_resume_with_options()`], but with default [`Options`].
///
/// # Errors
///
/// Returns an error if writing to the formatter fails or the event stream cannot be represented
/// as valid Markdown.
pub fn cmark_resume<'a, I, E, F>(
    events: I,
    formatter: F,
    state: Option<State<'a>>,
) -> Result<State<'a>, Error>
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
    F: fmt::Write,
{
    cmark_resume_with_options(events, formatter, state, Options::default())
}

/// As [`cmark_resume_with_options()`], but with the [`State`] finalized.
///
/// # Errors
///
/// Returns an error if writing to the formatter fails or the event stream cannot be represented
/// as valid Markdown.
pub fn cmark_with_options<'a, I, E, F>(
    events: I,
    mut formatter: F,
    options: Options<'_>,
) -> Result<State<'a>, Error>
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
    F: fmt::Write,
{
    let state = cmark_resume_with_options(events, &mut formatter, None, options)?;
    state.finalize(formatter)
}

/// Serialize a stream of [pulldown-cmark-Events][Event] into a string-backed buffer.
///
/// 1. **events**
///    * An iterator over [`Events`][Event], for example as returned by the [`Parser`][pulldown_cmark::Parser]
/// 1. **formatter**
///    * A format writer, can be a `String`.
/// 1. **state**
///    * The optional initial state of the serialization.
/// 1. **options**
///    * Customize the appearance of the serialization. All otherwise magic values are contained
///      here.
///
/// *Returns* the [`State`] of the serialization on success. You can use it as initial state in the
/// next call if you are halting event serialization.
///
/// # Errors
///
/// Returns an error if writing to the formatter fails or if the [`Event`] stream cannot be
/// produced by deserializing valid Markdown. Each failure mode corresponds to one of [`Error`]'s
/// variants.
pub fn cmark_resume_with_options<'a, I, E, F>(
    events: I,
    mut formatter: F,
    state: Option<State<'a>>,
    options: Options<'_>,
) -> Result<State<'a>, Error>
where
    I: Iterator<Item = E>,
    E: Borrow<Event<'a>>,
    F: fmt::Write,
{
    let mut state = state.unwrap_or_default();
    let mut events = events.peekable();
    while let Some(event) = events.next() {
        state.next_is_link_like = matches!(
            events.peek().map(Borrow::borrow),
            Some(
                Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::FootnoteDefinition(..))
                    | Event::FootnoteReference(..)
            )
        );
        cmark_resume_one_event(event.borrow(), &mut formatter, &mut state, &options)?;
    }
    Ok(state)
}

fn cmark_resume_one_event<'a>(
    event: &Event<'a>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
    options: &Options<'_>,
) -> Result<(), Error> {
    let last_was_text_without_trailing_newline = state.last_was_text_without_trailing_newline;
    state.last_was_text_without_trailing_newline = false;
    let last_was_paragraph_start = state.last_was_paragraph_start;
    state.last_was_paragraph_start = false;

    match event {
        Event::Rule => {
            consume_newlines(formatter, state)?;
            state.set_minimum_newlines_before_start(options.newlines_after_rule);
            formatter.write_str("---")?;
        }
        Event::Code(text) => write_inline_code(text, formatter, state)?,
        Event::Start(tag) => write_start_tag(tag, formatter, state, options)?,
        Event::End(tag) => write_end_tag(
            *tag,
            formatter,
            state,
            options,
            last_was_text_without_trailing_newline,
        )?,
        Event::HardBreak => {
            formatter.write_str("  ")?;
            write_padded_newline(formatter, state)?;
        }
        Event::SoftBreak => write_padded_newline(formatter, state)?,
        Event::Text(text) => write_text(text, formatter, state, options, last_was_paragraph_start)?,
        Event::InlineHtml(text) => {
            consume_newlines(formatter, state)?;
            print_text_without_trailing_newline(text, formatter, state)?;
        }
        Event::Html(text) => write_html(text, formatter, state)?,
        Event::FootnoteReference(name) => write!(formatter, "[^{name}]")?,
        Event::TaskListMarker(checked) => {
            let check = if *checked { "x" } else { " " };
            write!(formatter, "[{check}] ")?;
        }
        Event::InlineMath(text) => write!(formatter, "${text}$")?,
        Event::DisplayMath(text) => write!(formatter, "$${text}$$")?,
    }

    Ok(())
}

fn write_inline_code(
    text: &str,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
) -> fmt::Result {
    if let Some(shortcut_text) = state.current_shortcut_text.as_mut() {
        shortcut_text.push('`');
        shortcut_text.push_str(text);
        shortcut_text.push('`');
    }
    if let Some(text_for_header) = state.text_for_header.as_mut() {
        text_for_header.push('`');
        text_for_header.push_str(text);
        text_for_header.push('`');
    }

    // (re)-escape `|` when it appears as part of inline code in the
    // body of a table.
    //
    // NOTE: This does not do *general* escaped-character handling
    // because the only character which *requires* this handling in this
    // spot in earlier versions of `pulldown-cmark` is a pipe character
    // in inline code in a table. Other escaping is handled when `Text`
    // events are emitted.
    let text = if state.text_for_header.is_some() {
        Cow::Owned(text.replace('|', "\\|"))
    } else {
        Cow::Borrowed(text)
    };

    // When inline code has leading and trailing ' ' characters, additional space is needed
    // to escape it, unless all characters are space.
    if text.chars().all(|ch| ch == ' ') {
        write!(formatter, "`{text}`")
    } else {
        // More backticks are needed to delimit the inline code than the maximum number of
        // backticks in a consecutive run.
        let backticks = Repeated('`', max_consecutive_chars(text.as_ref(), '`') + 1);
        let space = match text.as_bytes() {
            &[b'`', ..] | &[.., b'`'] | &[b' ', .., b' '] => " ",
            _ => "",
        };
        write!(formatter, "{backticks}{space}{text}{space}{backticks}")
    }
}

fn write_start_tag<'a>(
    tag: &Tag<'a>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
    options: &Options<'_>,
) -> Result<(), Error> {
    let consumed_newlines = prepare_start_tag(tag, formatter, state, options)?;
    let result = match tag {
        Tag::Item => write_list_item_start(formatter, state, options),
        Tag::Table(alignments) => {
            state.table_alignments = alignments.iter().map(Alignment::from).collect();
            Ok(())
        }
        Tag::TableHead | Tag::TableRow | Tag::HtmlBlock | Tag::List(_) | Tag::DefinitionList => {
            Ok(())
        }
        Tag::TableCell => {
            state.text_for_header = Some(String::new());
            formatter.write_char('|')
        }
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => write_link_start(*link_type, dest_url, title, id, formatter, state),
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => write_image_start(*link_type, dest_url, title, id, formatter, state),
        Tag::Emphasis => formatter.write_char(options.emphasis_token),
        Tag::Strong => formatter.write_str(options.strong_token),
        Tag::FootnoteDefinition(name) => {
            state.padding.push("    ".into());
            write!(formatter, "[^{name}]: ")
        }
        Tag::Paragraph => {
            state.last_was_paragraph_start = true;
            Ok(())
        }
        Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } => write_heading_start(*level, id.as_ref(), classes, attrs, formatter, state),
        Tag::BlockQuote(kind) => {
            write_block_quote_start(*kind, formatter, state, consumed_newlines)
        }
        Tag::CodeBlock(kind) => {
            write_code_block_start(kind, formatter, state, options, consumed_newlines)
        }
        Tag::MetadataBlock(MetadataBlockKind::YamlStyle) => formatter.write_str("---\n"),
        Tag::MetadataBlock(MetadataBlockKind::PlusesStyle) => formatter.write_str("+++\n"),
        Tag::Strikethrough => formatter.write_str("~~"),
        Tag::DefinitionListTitle => {
            state.set_minimum_newlines_before_start(options.newlines_after_rest);
            Ok(())
        }
        Tag::DefinitionListDefinition => {
            padding(formatter, &state.padding)?;
            formatter.write_str(": ")?;
            state.padding.push("  ".into());
            Ok(())
        }
        Tag::Superscript => formatter.write_str(if options.use_html_for_super_sub_script {
            "<sup>"
        } else {
            "^"
        }),
        Tag::Subscript => formatter.write_str(if options.use_html_for_super_sub_script {
            "<sub>"
        } else {
            "~"
        }),
    };

    result.map_err(Error::from)
}

fn prepare_start_tag<'a>(
    tag: &Tag<'a>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
    options: &Options<'_>,
) -> Result<bool, Error> {
    if let Tag::List(list_type) = tag {
        state.list_stack.push(*list_type);
        if state.list_stack.len() > 1 {
            state.set_minimum_newlines_before_start(options.newlines_after_rest);
        }
    }

    let consumed_newlines = state.newlines_before_start != 0;
    consume_newlines(formatter, state)?;
    if matches!(tag, Tag::Heading { .. }) && state.current_heading.is_some() {
        return Err(Error::UnexpectedEvent);
    }
    Ok(consumed_newlines)
}

fn write_heading_start<'a>(
    level: pulldown_cmark::HeadingLevel,
    id: Option<&pulldown_cmark::CowStr<'a>>,
    classes: &[pulldown_cmark::CowStr<'a>],
    attributes: &[(
        pulldown_cmark::CowStr<'a>,
        Option<pulldown_cmark::CowStr<'a>>,
    )],
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
) -> fmt::Result {
    state.current_heading = Some(Heading {
        id: id.cloned().map(Into::into),
        classes: classes.iter().cloned().map(Into::into).collect(),
        attributes: attributes
            .iter()
            .map(|(key, value)| (key.clone().into(), value.clone().map(Into::into)))
            .collect(),
    });
    write!(formatter, "{} ", Repeated('#', level as usize))
}

fn write_list_item_start(
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
) -> fmt::Result {
    // Lazy lists act like paragraphs with no event.
    state.last_was_paragraph_start = true;
    let Some(inner) = state.list_stack.last_mut() else {
        return Ok(());
    };

    state.padding.push(list_item_padding_of(*inner));
    match inner {
        Some(number) => {
            let bullet_number = *number;
            if options.increment_ordered_list_bullets {
                *number += 1;
            }
            write!(
                formatter,
                "{}{} ",
                bullet_number, options.ordered_list_token
            )
        }
        None => write!(formatter, "{} ", options.list_token),
    }
}

fn write_link_start<'a>(
    link_type: LinkType,
    dest_url: &pulldown_cmark::CowStr<'a>,
    title: &pulldown_cmark::CowStr<'a>,
    id: &pulldown_cmark::CowStr<'a>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
) -> fmt::Result {
    state.link_stack.push(match link_type {
        LinkType::Autolink | LinkType::Email => {
            formatter.write_char('<')?;
            LinkCategory::AngleBracketed
        }
        LinkType::Reference => {
            formatter.write_char('[')?;
            LinkCategory::Reference {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
                id: id.clone().into(),
            }
        }
        LinkType::Collapsed => {
            state.current_shortcut_text = Some(String::new());
            formatter.write_char('[')?;
            LinkCategory::Collapsed {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
            }
        }
        LinkType::Shortcut => {
            state.current_shortcut_text = Some(String::new());
            formatter.write_char('[')?;
            LinkCategory::Shortcut {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
            }
        }
        _ => {
            formatter.write_char('[')?;
            LinkCategory::Other {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
            }
        }
    });
    Ok(())
}

fn write_image_start<'a>(
    link_type: LinkType,
    dest_url: &pulldown_cmark::CowStr<'a>,
    title: &pulldown_cmark::CowStr<'a>,
    id: &pulldown_cmark::CowStr<'a>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'a>,
) -> fmt::Result {
    state.image_stack.push(match link_type {
        LinkType::Reference => ImageLink::Reference {
            uri: dest_url.clone().into(),
            title: title.clone().into(),
            id: id.clone().into(),
        },
        LinkType::Collapsed => {
            state.current_shortcut_text = Some(String::new());
            ImageLink::Collapsed {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
            }
        }
        LinkType::Shortcut => {
            state.current_shortcut_text = Some(String::new());
            ImageLink::Shortcut {
                uri: dest_url.clone().into(),
                title: title.clone().into(),
            }
        }
        _ => ImageLink::Other {
            uri: dest_url.clone().into(),
            title: title.clone().into(),
        },
    });
    formatter.write_str("![")
}

fn write_block_quote_start(
    kind: Option<BlockQuoteKind>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    consumed_newlines: bool,
) -> fmt::Result {
    let every_line_padding = " > ";
    let first_line_padding = kind.map_or(every_line_padding, |kind| match kind {
        BlockQuoteKind::Note => " > [!NOTE]",
        BlockQuoteKind::Tip => " > [!TIP]",
        BlockQuoteKind::Important => " > [!IMPORTANT]",
        BlockQuoteKind::Warning => " > [!WARNING]",
        BlockQuoteKind::Caution => " > [!CAUTION]",
    });
    state.newlines_before_start = 1;

    // If we consumed some newlines, we can write the next level in the block quote.
    // This works regardless of whether there is other padding or the quote is in a list.
    if !consumed_newlines {
        write_padded_newline(formatter, state)?;
    }
    formatter.write_str(first_line_padding)?;
    state.padding.push(every_line_padding.into());
    Ok(())
}

fn write_code_block_start(
    kind: &pulldown_cmark::CodeBlockKind<'_>,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
    consumed_newlines: bool,
) -> fmt::Result {
    match kind {
        pulldown_cmark::CodeBlockKind::Indented => {
            state.code_block = Some(CodeBlockKind::Indented);
            state.padding.push("    ".into());
            if consumed_newlines {
                formatter.write_str("    ")
            } else {
                write_padded_newline(formatter, state)
            }
        }
        pulldown_cmark::CodeBlockKind::Fenced(info) => {
            state.code_block = Some(CodeBlockKind::Fenced);
            if !consumed_newlines {
                write_padded_newline(formatter, state)?;
            }

            let fence = Repeated(options.code_block_token, options.code_block_token_count);
            write!(formatter, "{fence}{info}")?;
            write_padded_newline(formatter, state)
        }
    }
}

fn write_end_tag(
    tag: TagEnd,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
    last_was_text_without_trailing_newline: bool,
) -> Result<(), Error> {
    match tag {
        TagEnd::Link => write_link_end(formatter, state),
        TagEnd::Image => write_image_end(formatter, state),
        TagEnd::Emphasis => formatter
            .write_char(options.emphasis_token)
            .map_err(Error::from),
        TagEnd::Strong => formatter
            .write_str(options.strong_token)
            .map_err(Error::from),
        TagEnd::Heading(_) => write_heading_end(formatter, state, options),
        TagEnd::Paragraph => {
            state.set_minimum_newlines_before_start(options.newlines_after_paragraph);
            Ok(())
        }
        TagEnd::CodeBlock => write_code_block_end(
            formatter,
            state,
            options,
            last_was_text_without_trailing_newline,
        )
        .map_err(Error::from),
        TagEnd::HtmlBlock => {
            state.set_minimum_newlines_before_start(options.newlines_after_htmlblock);
            Ok(())
        }
        TagEnd::MetadataBlock(kind) => {
            write_metadata_end(kind, formatter, state, options).map_err(Error::from)
        }
        TagEnd::Table => {
            state.set_minimum_newlines_before_start(options.newlines_after_table);
            state.table_alignments.clear();
            state.table_headers.clear();
            Ok(())
        }
        TagEnd::TableCell => {
            state
                .table_headers
                .push(state.text_for_header.take().unwrap_or_default());
            Ok(())
        }
        TagEnd::TableRow => {
            write_table_row_end(formatter, state, options, false).map_err(Error::from)
        }
        TagEnd::TableHead => {
            write_table_row_end(formatter, state, options, true).map_err(Error::from)
        }
        TagEnd::Item => {
            state.padding.pop();
            state.set_minimum_newlines_before_start(options.newlines_after_rest);
            Ok(())
        }
        TagEnd::List(_) => {
            state.list_stack.pop();
            if state.list_stack.is_empty() {
                state.set_minimum_newlines_before_start(options.newlines_after_list);
            }
            Ok(())
        }
        TagEnd::BlockQuote(_) => {
            state.padding.pop();
            state.set_minimum_newlines_before_start(options.newlines_after_blockquote);
            Ok(())
        }
        TagEnd::FootnoteDefinition => {
            state.padding.pop();
            Ok(())
        }
        TagEnd::Strikethrough => formatter.write_str("~~").map_err(Error::from),
        TagEnd::DefinitionList => {
            state.set_minimum_newlines_before_start(options.newlines_after_list);
            Ok(())
        }
        TagEnd::DefinitionListTitle => formatter.write_char('\n').map_err(Error::from),
        TagEnd::DefinitionListDefinition => {
            state.padding.pop();
            write_padded_newline(formatter, state).map_err(Error::from)
        }
        TagEnd::Superscript => {
            let marker = if options.use_html_for_super_sub_script {
                "</sup>"
            } else {
                "^"
            };
            formatter.write_str(marker).map_err(Error::from)
        }
        TagEnd::Subscript => {
            let marker = if options.use_html_for_super_sub_script {
                "</sub>"
            } else {
                "~"
            };
            formatter.write_str(marker).map_err(Error::from)
        }
    }
}

fn write_metadata_end(
    kind: MetadataBlockKind,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
) -> fmt::Result {
    state.set_minimum_newlines_before_start(options.newlines_after_metadata);
    formatter.write_str(match kind {
        MetadataBlockKind::PlusesStyle => "+++\n",
        MetadataBlockKind::YamlStyle => "---\n",
    })
}

fn write_link_end(formatter: &mut impl fmt::Write, state: &mut State<'_>) -> Result<(), Error> {
    let Some(link_category) = state.link_stack.pop() else {
        return Err(Error::UnexpectedEvent);
    };

    match link_category {
        LinkCategory::AngleBracketed => formatter.write_char('>')?,
        LinkCategory::Reference { uri, title, id } => {
            state
                .shortcuts
                .push((id.to_string(), uri.to_string(), title.to_string()));
            formatter.write_str("][")?;
            formatter.write_str(id.as_ref())?;
            formatter.write_char(']')?;
        }
        LinkCategory::Collapsed { uri, title } => {
            record_shortcut(state, uri, title);
            formatter.write_str("][]")?;
        }
        LinkCategory::Shortcut { uri, title } => {
            record_shortcut(state, uri, title);
            formatter.write_char(']')?;
        }
        LinkCategory::Other { uri, title } => {
            close_link(uri.as_ref(), title.as_ref(), formatter, LinkType::Inline)?;
        }
    }
    Ok(())
}

fn write_image_end(formatter: &mut impl fmt::Write, state: &mut State<'_>) -> Result<(), Error> {
    let Some(image_link) = state.image_stack.pop() else {
        return Err(Error::UnexpectedEvent);
    };

    match image_link {
        ImageLink::Reference { uri, title, id } => {
            state
                .shortcuts
                .push((id.to_string(), uri.to_string(), title.to_string()));
            formatter.write_str("][")?;
            formatter.write_str(id.as_ref())?;
            formatter.write_char(']')?;
        }
        ImageLink::Collapsed { uri, title } => {
            record_shortcut(state, uri, title);
            formatter.write_str("][]")?;
        }
        ImageLink::Shortcut { uri, title } => {
            record_shortcut(state, uri, title);
            formatter.write_char(']')?;
        }
        ImageLink::Other { uri, title } => {
            close_link(uri.as_ref(), title.as_ref(), formatter, LinkType::Inline)?;
        }
    }
    Ok(())
}

fn record_shortcut<'a>(state: &mut State<'a>, uri: Cow<'a, str>, title: Cow<'a, str>) {
    if let Some(shortcut_text) = state.current_shortcut_text.take() {
        state.shortcuts.push((
            EscapeLinkLabel(&shortcut_text).to_string(),
            uri.into(),
            title.into(),
        ));
    }
}

fn write_heading_end(
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
) -> Result<(), Error> {
    let Some(Heading {
        id,
        classes,
        attributes,
    }) = state.current_heading.take()
    else {
        return Err(Error::UnexpectedEvent);
    };

    let emit_braces = id.is_some() || !classes.is_empty() || !attributes.is_empty();
    if emit_braces {
        formatter.write_str(" {")?;
    }
    if let Some(id) = id {
        formatter.write_char(' ')?;
        formatter.write_char('#')?;
        formatter.write_str(id.as_ref())?;
    }
    for class in &classes {
        formatter.write_char(' ')?;
        formatter.write_char('.')?;
        formatter.write_str(class.as_ref())?;
    }
    for (key, value) in &attributes {
        formatter.write_char(' ')?;
        formatter.write_str(key.as_ref())?;
        if let Some(value) = value {
            formatter.write_char('=')?;
            formatter.write_str(value.as_ref())?;
        }
    }
    if emit_braces {
        formatter.write_char(' ')?;
        formatter.write_char('}')?;
    }
    state.set_minimum_newlines_before_start(options.newlines_after_headline);
    Ok(())
}

fn write_code_block_end(
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
    last_was_text_without_trailing_newline: bool,
) -> fmt::Result {
    state.set_minimum_newlines_before_start(options.newlines_after_codeblock);
    if last_was_text_without_trailing_newline {
        write_padded_newline(formatter, state)?;
    }
    match state.code_block {
        Some(CodeBlockKind::Fenced) => {
            let fence = Repeated(options.code_block_token, options.code_block_token_count);
            write!(formatter, "{fence}")?;
        }
        Some(CodeBlockKind::Indented) => {
            state.padding.pop();
        }
        None => {}
    }
    state.code_block = None;
    Ok(())
}

fn write_table_row_end(
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
    is_header: bool,
) -> fmt::Result {
    state.set_minimum_newlines_before_start(options.newlines_after_rest);
    formatter.write_char('|')?;

    if is_header {
        write_padded_newline(formatter, state)?;
        for (alignment, name) in state
            .table_alignments
            .iter()
            .zip(state.table_headers.iter())
        {
            formatter.write_char('|')?;
            // NOTE: For perfect counting, count grapheme clusters.
            // The reason this is not done is to avoid the dependency.

            // The minimum width of the column so that we can represent its alignment.
            let min_width = match alignment {
                // Must at least represent `-`.
                Alignment::None => 1,
                // Must at least represent `:-` or `-:`.
                Alignment::Left | Alignment::Right => 2,
                // Must at least represent `:-:`.
                Alignment::Center => 3,
            };
            let length = name.chars().count().max(min_width);
            let last_minus_one = length.saturating_sub(1);
            for position in 0..length {
                let left_colon =
                    position == 0 && matches!(alignment, Alignment::Center | Alignment::Left);
                let right_colon = position == last_minus_one
                    && matches!(alignment, Alignment::Center | Alignment::Right);
                formatter.write_char(if left_colon || right_colon { ':' } else { '-' })?;
            }
        }
        formatter.write_char('|')?;
    }
    Ok(())
}

fn write_text(
    text: &str,
    formatter: &mut impl fmt::Write,
    state: &mut State<'_>,
    options: &Options<'_>,
    last_was_paragraph_start: bool,
) -> fmt::Result {
    if let Some(shortcut_text) = state.current_shortcut_text.as_mut() {
        shortcut_text.push_str(text);
    }
    if let Some(text_for_header) = state.text_for_header.as_mut() {
        text_for_header.push_str(text);
    }
    consume_newlines(formatter, state)?;

    let mut text = text;
    if last_was_paragraph_start {
        if text.starts_with('\t') {
            formatter.write_str("&#9;")?;
            text = &text[1..];
        } else if text.starts_with(' ') {
            formatter.write_str("&#32;")?;
            text = &text[1..];
        }
    }
    state.last_was_text_without_trailing_newline = !text.ends_with('\n');
    let escaped_text = escape_special_characters(text, state, options);
    print_text_without_trailing_newline(escaped_text.as_ref(), formatter, state)
}

fn write_html(text: &str, formatter: &mut impl fmt::Write, state: &State<'_>) -> fmt::Result {
    let mut lines = text.split('\n');
    if let Some(line) = lines.next() {
        formatter.write_str(line)?;
    }
    for line in lines {
        write_padded_newline(formatter, state)?;
        formatter.write_str(line)?;
    }
    Ok(())
}

impl State<'_> {
    /// Finalize the serialization state by writing any remaining shortcuts.
    ///
    /// This should be called after all events have been processed to ensure
    /// reference-style links are written at the end of the document.
    ///
    /// # Errors
    ///
    /// Returns an error if writing a shortcut reference to the formatter fails.
    pub fn finalize<F>(mut self, mut formatter: F) -> Result<Self, Error>
    where
        F: fmt::Write,
    {
        if self.shortcuts.is_empty() {
            return Ok(self);
        }

        formatter.write_str("\n")?;
        let mut written_shortcuts = HashSet::new();
        for shortcut in self.shortcuts.drain(..) {
            if written_shortcuts.contains(&shortcut) {
                continue;
            }
            write!(formatter, "\n[{}", shortcut.0)?;
            close_link(&shortcut.1, &shortcut.2, &mut formatter, LinkType::Shortcut)?;
            written_shortcuts.insert(shortcut);
        }
        Ok(self)
    }

    /// Returns `true` if currently serializing content inside a code block.
    #[must_use]
    pub const fn is_in_code_block(&self) -> bool {
        self.code_block.is_some()
    }

    /// Ensure that [`State::newlines_before_start`] is at least as large as
    /// the provided option value.
    const fn set_minimum_newlines_before_start(&mut self, option_value: usize) {
        if self.newlines_before_start < option_value {
            self.newlines_before_start = option_value;
        }
    }
}

/// Return one more than the longest run of fenced code-block tokens within code-block `events`.
///
/// Use this function to obtain the correct value for `code_block_token_count` field of [`Options`]
/// to assure that the enclosing code-blocks remain functional as such.
///
/// Returns `None` if `events` didn't include any code-block, or the code-block didn't contain
/// a nested block. In that case, the correct amount of fenced code-block tokens is
/// [`DEFAULT_CODE_BLOCK_TOKEN_COUNT`].
///
/// ```rust
/// use pulldown_cmark::Event;
/// use pulldown_cmark_to_cmark::{
///     DEFAULT_CODE_BLOCK_TOKEN_COUNT, Options, calculate_code_block_token_count,
///     cmark_with_options,
/// };
///
/// let events = &[Event::Text("text".into())];
/// let code_block_token_count = calculate_code_block_token_count(events).unwrap_or(DEFAULT_CODE_BLOCK_TOKEN_COUNT);
/// let options = Options {
///     code_block_token_count,
///     ..Options::default()
/// };
/// let mut buf = String::new();
/// cmark_with_options(events.iter(), &mut buf, options).unwrap();
/// ```
pub fn calculate_code_block_token_count<'a, I, E>(events: I) -> Option<usize>
where
    I: IntoIterator<Item = E>,
    E: Borrow<Event<'a>>,
{
    let mut in_codeblock = false;
    let mut max_token_count = 0;

    // token_count should be taken over Text events
    // because continuous text may be split across several Text events.
    let mut token_count = 0;
    let mut prev_token_char = None;
    for event in events {
        match event.borrow() {
            Event::Start(Tag::CodeBlock(_)) => {
                in_codeblock = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                in_codeblock = false;
                prev_token_char = None;
            }
            Event::Text(x) if in_codeblock => {
                for c in x.chars() {
                    let prev_token = prev_token_char.take();
                    if c == '`' || c == '~' {
                        prev_token_char = Some(c);
                        if Some(c) == prev_token {
                            token_count += 1;
                        } else {
                            max_token_count = max_token_count.max(token_count);
                            token_count = 1;
                        }
                    }
                }
            }
            _ => prev_token_char = None,
        }
    }

    max_token_count = max_token_count.max(token_count);
    (max_token_count >= 3).then_some(max_token_count + 1)
}
