//! Mechanical Markdown formatting.
//!
//! The formatter owns one deliberately small style: every paragraph runs on
//! one line, including the inline content of a tight list item. It otherwise
//! delegates Markdown spelling to `pulldown-cmark-to-cmark`, after parsing with
//! the same extensions as the linter's document readers.
//!
//! Formatting is guarded semantically. The rendered document is parsed again,
//! and its event stream must be equivalent to the input after the one intended
//! transformation: a soft break in paragraph-like content becomes a space.
//! Text compares after whitespace normalization, while inline code, fenced
//! bodies and info strings, HTML, and link destinations compare exactly as
//! fields of their events. A failed guard prevents every planned write in the
//! invocation.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`joins_a_hard_wrapped_paragraph`] | fmt | A soft break in a paragraph becomes one space, producing one running line without moving its words. |
//! | [`formats_markdown_idempotently`] | fmt | The bytes emitted by one pass are the fixed point of the formatter, so checking the same document again reports no change. |
//! | [`preserves_a_fenced_code_block_byte_for_byte`] | fmt | A safe three-backtick fence and every byte of its body survive while prose around it is independently formatted. |
//! | [`preserves_calculus_labels_through_a_table`] | fmt | Mints, parenthesized citations, inline-code payloads, and tables pass the integrity guard with every calculus token unchanged. |
//! | [`refuses_a_word_changing_event_stream`] | fmt | The equivalence guard rejects a serializer result that changes a word, even when both sides remain valid Markdown. |
//! | [`preserves_authored_markdown_escapes`] | fmt | Source ranges retain authored escapes, so literal asterisks cannot turn into emphasis during serialization. |
//! | [`raises_every_fence_for_a_hazardous_code_block_body`] | fmt | A backtick run in one body raises the safe global fence width for every block, reflecting the serializer's file-wide option without changing a body byte. |

use std::collections::BTreeSet;
use std::fmt as std_fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use pulldown_cmark::{Event, Options as ParseOptions, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::{
    Options as SerializeOptions, calculate_code_block_token_count,
    cmark_with_source_range_and_options,
};
use serde::Serialize;

/// One Markdown file changed, or found to need a change, by the formatter.
#[derive(Debug, Clone, Serialize)]
pub struct FormatChange {
    /// The path exactly as reached from the command's arguments.
    pub path: String,
    /// Wall time spent parsing, guarding, and, where enabled, writing the file.
    pub elapsed_micros: u128,
}

/// The stdout result object of the `fmt` command.
#[derive(Debug, Clone, Serialize)]
pub struct FormatReport {
    /// Whether the command only checked for changes.
    pub check: bool,
    /// How many Markdown files were parsed and guarded.
    pub files_scanned: usize,
    /// How many files changed or would change.
    pub files_changed: usize,
    /// Every changed path, in deterministic path order, with its action time.
    pub changed: Vec<FormatChange>,
    /// Wall time for discovery, formatting, guarding, and optional writes.
    pub elapsed_micros: u128,
}

impl FormatReport {
    /// Whether at least one file changed or would change.
    #[must_use]
    pub const fn has_changes(&self) -> bool {
        self.files_changed != 0
    }
}

/// A failure that makes a formatting invocation unsafe to complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    message: String,
}

impl FormatError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn at(path: &Path, message: impl std_fmt::Display) -> Self {
        Self::new(format!("{}: {message}", path.display()))
    }
}

impl std_fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std_fmt::Formatter<'_>) -> std_fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FormatError {}

struct PlannedWrite {
    path: PathBuf,
    rendered: String,
    elapsed: Duration,
}

/// Format every Markdown file named by `paths` or found below their directories.
///
/// Directories are walked recursively without following symbolic links. Files
/// reached through directories must have the lowercase `.md` extension; a path
/// naming a file directly must name such a file as well. All inputs are parsed,
/// serialized, and guarded before the first write, so an unsafe document cannot
/// leave an otherwise successful plan half-applied.
///
/// # Errors
///
/// Returns an error when a path cannot be traversed, a file is not Markdown or
/// UTF-8, serialization fails, the equivalence guard refuses output, or a write
/// fails.
pub fn format_paths(paths: &[PathBuf], check: bool) -> Result<FormatReport, FormatError> {
    let started = Instant::now();
    let files = markdown_files(paths)?;
    let files_scanned = files.len();
    let mut planned = Vec::new();

    for path in files {
        let file_started = Instant::now();
        let source = fs::read_to_string(&path)
            .map_err(|error| FormatError::at(&path, format_args!("could not read: {error}")))?;
        let rendered = format_markdown(&source)
            .map_err(|error| FormatError::at(&path, format_args!("{error}")))?;

        if rendered != source {
            planned.push(PlannedWrite {
                path,
                rendered,
                elapsed: file_started.elapsed(),
            });
        }
    }

    let mut changed = Vec::with_capacity(planned.len());

    for mut change in planned {
        if !check {
            let write_started = Instant::now();
            fs::write(&change.path, &change.rendered).map_err(|error| {
                FormatError::at(&change.path, format_args!("could not write: {error}"))
            })?;
            change.elapsed += write_started.elapsed();
        }

        changed.push(FormatChange {
            path: change.path.to_string_lossy().into_owned(),
            elapsed_micros: change.elapsed.as_micros(),
        });
    }

    Ok(FormatReport {
        check,
        files_scanned,
        files_changed: changed.len(),
        changed,
        elapsed_micros: started.elapsed().as_micros(),
    })
}

/// Format one Markdown document in memory.
///
/// A source ending in a newline keeps one after serialization. The serializer
/// itself defers block spacing until another event arrives and would otherwise
/// discard that ordinary end-of-file terminator.
///
/// # Errors
///
/// Returns an error when the serializer rejects the parser's event stream or
/// when re-parsing proves that the rendered stream is not equivalent.
pub fn format_markdown(source: &str) -> Result<String, FormatError> {
    let parsed: Vec<_> = Parser::new_ext(source, parser_options())
        .into_offset_iter()
        .collect();
    let fence_width = calculate_code_block_token_count(parsed.iter().map(|(event, _range)| event))
        .unwrap_or(3)
        .max(3);
    let events = reflow_soft_breaks(parsed);
    let mut rendered = String::new();

    cmark_with_source_range_and_options(
        events
            .iter()
            .map(|(event, range)| (event, Some(range.clone()))),
        source,
        &mut rendered,
        serializer_options(fence_width),
    )
    .map_err(|error| FormatError::new(format!("could not serialize Markdown: {error}")))?;

    if source.ends_with('\n') && !rendered.ends_with('\n') {
        rendered.push('\n');
    }

    ensure_equivalent(source, &rendered)?;
    Ok(rendered)
}

fn parser_options() -> ParseOptions {
    ParseOptions::ENABLE_TABLES
        | ParseOptions::ENABLE_FOOTNOTES
        | ParseOptions::ENABLE_STRIKETHROUGH
        | ParseOptions::ENABLE_TASKLISTS
}

const fn serializer_options(code_block_token_count: usize) -> SerializeOptions<'static> {
    SerializeOptions {
        // These blocks do not emit a trailing newline themselves. Two newlines
        // therefore mean one normalized blank line before the next block.
        newlines_after_headline: 2,
        newlines_after_paragraph: 2,
        newlines_after_codeblock: 2,
        newlines_after_table: 2,
        newlines_after_rule: 2,
        newlines_after_list: 2,
        newlines_after_blockquote: 2,
        // HTML and metadata events already emit their authored final newline;
        // one deferred newline gives them the same single blank-line boundary.
        newlines_after_htmlblock: 1,
        newlines_after_metadata: 1,
        // Rows, items, and other internal boundaries stay adjacent.
        newlines_after_rest: 1,
        // Three backticks are the standard. One hazardous body raises this one
        // global width for every fence because that is the upstream API's unit.
        code_block_token_count,
        code_block_token: '`',
        list_token: '-',
        ordered_list_token: '.',
        increment_ordered_list_bullets: true,
        emphasis_token: '*',
        strong_token: "**",
        use_html_for_super_sub_script: true,
    }
}

fn reflow_soft_breaks(parsed: Vec<(Event<'_>, Range<usize>)>) -> Vec<(Event<'_>, Range<usize>)> {
    let mut heading_depth = 0usize;
    let mut table_depth = 0usize;
    let mut reflowed = Vec::with_capacity(parsed.len());

    for (event, range) in parsed {
        match &event {
            Event::Start(Tag::Heading { .. }) => heading_depth += 1,
            Event::Start(Tag::Table(_)) => table_depth += 1,
            _ => {}
        }

        let event = if matches!(&event, Event::SoftBreak) && heading_depth == 0 && table_depth == 0
        {
            Event::Text(" ".into())
        } else {
            event
        };

        match &event {
            Event::End(TagEnd::Heading(_)) => heading_depth = heading_depth.saturating_sub(1),
            Event::End(TagEnd::Table) => table_depth = table_depth.saturating_sub(1),
            _ => {}
        }

        reflowed.push((event, range));
    }

    reflowed
}

fn markdown_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, FormatError> {
    let mut pending: Vec<PathBuf> = paths.iter().rev().cloned().collect();
    let mut files = BTreeSet::new();

    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| FormatError::at(&path, format_args!("could not inspect: {error}")))?;

        if metadata.file_type().is_symlink() {
            return Err(FormatError::at(
                &path,
                "symbolic links are not followed by the formatter",
            ));
        }

        if metadata.is_file() {
            if !is_markdown(&path) {
                return Err(FormatError::at(&path, "expected a Markdown file"));
            }
            files.insert(path);
            continue;
        }

        if !metadata.is_dir() {
            return Err(FormatError::at(&path, "expected a file or directory"));
        }

        let entries = fs::read_dir(&path)
            .map_err(|error| FormatError::at(&path, format_args!("could not read: {error}")))?;
        let mut children = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|error| {
                FormatError::at(&path, format_args!("could not read an entry: {error}"))
            })?;
            let file_type = entry.file_type().map_err(|error| {
                FormatError::at(&entry.path(), format_args!("could not inspect: {error}"))
            })?;

            if file_type.is_dir() || (file_type.is_file() && is_markdown(&entry.path())) {
                children.push(entry.path());
            }
        }

        children.sort();
        pending.extend(children.into_iter().rev());
    }

    Ok(files.into_iter().collect())
}

fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "md")
}

#[derive(Debug, PartialEq, Eq)]
enum GuardEvent {
    Structure(String),
    Text(String),
    Code(String),
    CodeBlockText(String),
    Html(String),
    InlineHtml(String),
}

impl GuardEvent {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Structure(_) => "structure",
            Self::Text(_) => "text",
            Self::Code(_) => "inline code",
            Self::CodeBlockText(_) => "code-block body",
            Self::Html(_) => "HTML block",
            Self::InlineHtml(_) => "inline HTML",
        }
    }
}

fn ensure_equivalent(input: &str, output: &str) -> Result<(), FormatError> {
    let input = guarded_events(input);
    let output = guarded_events(output);

    if input == output {
        return Ok(());
    }

    let mismatch = input
        .iter()
        .zip(&output)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| input.len().min(output.len()));
    let input_kind = input
        .get(mismatch)
        .map_or("end of stream", GuardEvent::kind);
    let output_kind = output
        .get(mismatch)
        .map_or("end of stream", GuardEvent::kind);

    Err(FormatError::new(format!(
        "integrity guard rejected event {mismatch}: input has {input_kind}, output has {output_kind}"
    )))
}

fn guarded_events(source: &str) -> Vec<GuardEvent> {
    let mut guarded = Vec::new();
    let mut pending_text = String::new();
    let mut code_block_depth = 0usize;
    let mut heading_depth = 0usize;
    let mut table_depth = 0usize;

    for event in Parser::new_ext(source, parser_options()) {
        match &event {
            Event::Start(Tag::Heading { .. }) => heading_depth += 1,
            Event::Start(Tag::Table(_)) => table_depth += 1,
            _ => {}
        }

        match &event {
            Event::Start(Tag::CodeBlock(_)) => {
                flush_text(&mut pending_text, &mut guarded);
                guarded.push(GuardEvent::Structure(format!("{event:?}")));
                code_block_depth += 1;
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_text(&mut pending_text, &mut guarded);
                code_block_depth = code_block_depth.saturating_sub(1);
                guarded.push(GuardEvent::Structure(format!("{event:?}")));
            }
            Event::Text(text) if code_block_depth != 0 => {
                flush_text(&mut pending_text, &mut guarded);
                push_code_block_text(&mut guarded, text);
            }
            Event::Text(text) => pending_text.push_str(text),
            Event::SoftBreak if heading_depth == 0 && table_depth == 0 => {
                pending_text.push(' ');
            }
            Event::Code(text) => {
                flush_text(&mut pending_text, &mut guarded);
                guarded.push(GuardEvent::Code(text.to_string()));
            }
            Event::Html(text) => {
                flush_text(&mut pending_text, &mut guarded);
                guarded.push(GuardEvent::Html(text.to_string()));
            }
            Event::InlineHtml(text) => {
                flush_text(&mut pending_text, &mut guarded);
                guarded.push(GuardEvent::InlineHtml(text.to_string()));
            }
            _ => {
                flush_text(&mut pending_text, &mut guarded);
                guarded.push(GuardEvent::Structure(format!("{event:?}")));
            }
        }

        match &event {
            Event::End(TagEnd::Heading(_)) => heading_depth = heading_depth.saturating_sub(1),
            Event::End(TagEnd::Table) => table_depth = table_depth.saturating_sub(1),
            _ => {}
        }
    }

    flush_text(&mut pending_text, &mut guarded);
    guarded
}

fn flush_text(pending: &mut String, guarded: &mut Vec<GuardEvent>) {
    if pending.is_empty() {
        return;
    }

    let mut words = pending.split_whitespace();
    let mut normalized = words.next().unwrap_or_default().to_owned();
    for word in words {
        normalized.push(' ');
        normalized.push_str(word);
    }
    pending.clear();

    if !normalized.is_empty() {
        guarded.push(GuardEvent::Text(normalized));
    }
}

fn push_code_block_text(guarded: &mut Vec<GuardEvent>, text: &str) {
    if let Some(GuardEvent::CodeBlockText(current)) = guarded.last_mut() {
        current.push_str(text);
    } else {
        guarded.push(GuardEvent::CodeBlockText(text.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_equivalent, format_markdown};

    fn formatted(source: &str) -> String {
        match format_markdown(source) {
            Ok(rendered) => rendered,
            Err(error) => panic!("formatting failed: {error}"),
        }
    }

    /// A soft break in a paragraph becomes one space, producing one running
    /// line without moving its words.
    ///
    /// ´claim:fmt:a-soft-break-formats-as-one-space-without-moving-words´
    /// ´test:unit:joins-a-hard-wrapped-paragraph´
    #[test]
    fn joins_a_hard_wrapped_paragraph() {
        let source = "A paragraph that is\nhard wrapped.\n";
        assert_eq!(formatted(source), "A paragraph that is hard wrapped.\n");
    }

    /// The bytes emitted by one pass are the fixed point of the formatter, so
    /// checking the same document again reports no change.
    ///
    /// ´claim:fmt:one-pass-is-the-formatters-fixed-point´
    /// ´test:unit:formats-markdown-idempotently´
    #[test]
    fn formats_markdown_idempotently() {
        let once = formatted("One line that was\nwrapped by hand.\n");
        let twice = formatted(&once);
        assert_eq!(once, twice);
    }

    /// A safe three-backtick fence and every byte of its body survive while
    /// prose around it is independently formatted.
    ///
    /// ´claim:fmt:a-safe-fence-and-its-body-survive-formatting´
    /// ´test:unit:preserves-a-fenced-code-block-byte-for-byte´
    #[test]
    fn preserves_a_fenced_code_block_byte_for_byte() {
        let block = "```rust\nlet value = `literal`;\n```";
        let source = format!("Before the block.\n\n{block}\n\nAfter the block.\n");
        let rendered = formatted(&source);
        assert!(rendered.contains(block));
    }

    /// Mints, parenthesized citations, inline-code payloads, and tables pass
    /// the integrity guard with every calculus token unchanged.
    ///
    /// ´claim:fmt:calculus-tokens-survive-formatting-unchanged´
    /// ´test:unit:preserves-calculus-labels-through-a-table´
    #[test]
    fn preserves_calculus_labels_through_a_table() {
        let source = concat!(
            "## Head · `sec:area:head`\n\n",
            "The citation (`sec:area:head`) and mint · `claim:area:statement` survive.\n\n",
            "| Form | Label |\n",
            "| --- | --- |\n",
            "| code | `kind:area:slug` |\n",
        );
        let rendered = formatted(source);

        for exact in [
            "· `sec:area:head`",
            "(`sec:area:head`)",
            "· `claim:area:statement`",
            "`kind:area:slug`",
        ] {
            assert!(rendered.contains(exact), "missing exact span {exact}");
        }
    }

    /// The equivalence guard rejects a serializer result that changes a word,
    /// even when both sides remain valid Markdown.
    ///
    /// ´claim:fmt:word-changing-markdown-is-refused-by-the-equivalence-guard´
    /// ´test:unit:refuses-a-word-changing-event-stream´
    #[test]
    fn refuses_a_word_changing_event_stream() {
        let result = ensure_equivalent("The original word.\n", "The changed word.\n");
        assert!(result.is_err());
    }

    /// Source ranges retain authored escapes, so literal asterisks cannot turn
    /// into emphasis during serialization.
    ///
    /// ´claim:fmt:authored-escapes-survive-serialization´
    /// ´test:unit:preserves-authored-markdown-escapes´
    #[test]
    fn preserves_authored_markdown_escapes() {
        let source = "An authored \\*literal\\* stays text.\n";
        assert_eq!(formatted(source), source);
    }

    /// A backtick run in one body raises the safe global fence width for every
    /// block, reflecting the serializer's file-wide option without changing a
    /// body byte.
    ///
    /// ´claim:fmt:a-hazardous-body-raises-every-fence-without-changing-body-bytes´
    /// ´test:unit:raises-every-fence-for-a-hazardous-code-block-body´
    #[test]
    fn raises_every_fence_for_a_hazardous_code_block_body() {
        let source = concat!("```text\nplain\n```\n\n", "````text\n```\n````\n",);
        let rendered = formatted(source);

        assert_eq!(rendered.matches("````").count(), 4);
        assert!(rendered.contains("````text\n```\n````"));
    }
}
