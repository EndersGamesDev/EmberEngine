// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Comment extraction from Rust sources: what a comment is, and what a string
//! is not.
//!
//! A burn list censuses a legacy reference form wherever it is written, and the
//! campaign writes references in code comments as well as in prose. Reading a
//! Rust source for them needs one fact the Markdown side gets for free: which
//! bytes are commentary and which are program text. The distinction has to be
//! exact in one direction in particular. A string literal may carry anything at
//! all, and this repository's sources carry plenty of them holding the very
//! shapes the burn lists count — a linter's own reject table, a test's expected
//! output, a URL whose authority is introduced by two solidi. Reading a string
//! as a comment would make the census report the linter's fixtures as corpus
//! references, and no amount of care in the recognizers would repair that.
//!
//! # Why a lexical scan rather than the parser already in the crate
//!
//! The crate parses Rust with syn for the test census, and the obvious move is
//! to reuse it. It cannot be reused, for a reason that is a property of the
//! language rather than of the library: Rust's lexer discards ordinary comments
//! entirely. Only documentation comments survive, and only because they are
//! rewritten into documentation attributes before the parser sees them. A syn
//! tree therefore cannot answer where the line comments are, because by the time
//! there is a tree there are none.
//!
//! The second candidate is the token stream, with source locations switched on:
//! tokens carry spans, so the comments are the gaps between them. That inverts
//! the problem rather than solving it — the gaps also hold whitespace, and
//! recovering a comment means re-lexing the gap anyway — and it needs the whole
//! file to tokenize, which is a precondition a census over a migrating corpus
//! should not have.
//!
//! So this module lexes, and only as far as it must. Separating comments from
//! strings is a small and closed part of Rust's lexical grammar, and every part
//! of it is here: line comments, block comments and their nesting, plain and raw
//! strings with every prefix the language admits, character literals with their
//! escapes, and the one ambiguity that makes a naive scanner wrong — a leading
//! quote begins a character literal in some places and a lifetime in others, and
//! a scanner that guesses "literal" swallows everything up to the next quote.
//!
//! Nothing here needs the source to be valid Rust. An unterminated comment or
//! string runs to the end of the file, which is what the compiler's own lexer
//! reports, and the census over a half-written source is then wrong only about
//! that source's tail rather than about the file.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_every_line_comment_form`] | comment | All three line-comment spellings are commentary: the plain one and both documentation forms, inner and outer. Which one an author reached for does not change whether what follows is prose. |
//! | [`reads_block_comments_and_their_nesting`] | comment | Block comments nest as the language says they do: an inner comment's close does not end the outer one early, and separate blocks on a line are read as the separate comments they are. |
//! | [`reads_an_unterminated_block_comment_to_the_end`] | comment | A block comment nobody closed runs to the end of the file rather than aborting the scan, so a source that is not valid Rust can still be censused — which is the condition a migrating corpus is usually in. |
//! | [`leaves_string_literals_unread`] | comment | A string literal is never commentary, whatever it holds: a URL whose authority is introduced by two solidi, a comment delimiter written out as data, an escaped quote, a trailing pair of backslashes. Without this the census would report a linter's own fixtures as corpus references. |
//! | [`leaves_raw_strings_of_every_hash_count_unread`] | comment | A raw string closes on the hash run it opened with and on no earlier quote, at every hash count the language admits. A raw string may therefore carry a quote, a shorter hash run, or a comment delimiter without ending, and commentary after it is still found. |
//! | [`leaves_prefixed_literals_unread`] | comment | cites (´claim:comment:a-string-literal-is-never-read-as-commentary´) |
//! | [`leaves_words_ending_in_a_prefix_letter_alone`] | comment | A literal prefix is only a prefix where a literal follows it: a raw identifier, and an ordinary word ending in a letter that elsewhere introduces a literal, both leave the comment after them intact. |
//! | [`reads_comments_after_a_lifetime`] | comment | A leading quote that opens a lifetime is not read as opening a character literal, so a comment after a function or a structure carrying lifetimes is still found. A scanner that guessed the other way would swallow everything up to the next quote in the file. |
//! | [`reads_comments_after_every_character_literal`] | comment | A character literal ends where it really ends, through every escape the language admits — an escaped quote, an escaped backslash, a named escape, a numeric one, and a character written directly outside ASCII — so the comment standing after any of them is read. |
//! | [`reads_a_comment_holding_a_quote`] | comment | Commentary is text and is not lexed any further: an unpaired quote inside a comment opens nothing, and the code and comments after it are read normally. |
//! | [`keeps_regions_on_character_boundaries`] | comment | The regions a scan reports fall on character boundaries, so text outside ASCII in a comment, in a string between comments, or in a block comment can be taken as text without the slice panicking. |

/// The interior of one comment, as a byte range into the source it stands in.
///
/// The range excludes the opening delimiter and, for a block comment, the
/// closing one. Both ends fall on character boundaries, because a delimiter is
/// always ASCII, so the range may be used to slice the source directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommentRegion {
    start: usize,
    end: usize,
}

impl CommentRegion {
    /// The byte offset the comment's interior begins at.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// The byte offset one past the comment's interior.
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// The comment's interior text, taken from the source it was read from.
    ///
    /// # Panics
    ///
    /// Panics when the source is not the one the region was read from, which is
    /// a caller's mistake rather than a condition this type can be in.
    #[must_use]
    pub fn text<'source>(&self, source: &'source str) -> &'source str {
        &source[self.start..self.end]
    }
}

/// What a literal prefix opens, once the prefix itself has been read.
enum Opening {
    /// A string with escapes, at the offset of its opening quote.
    Plain(usize),
    /// A raw string: the offset of its hash run, and how long that run is.
    Raw(usize, usize),
    /// A character literal, at the offset of its opening quote.
    Quote(usize),
}

/// How far past an escape's backslash a character literal may be looked for.
///
/// Only the brace-delimited scalar escape is variable in length, and it is read
/// by finding its brace rather than by counting, so this bound is reached only
/// by text that is not a character literal at all.
///
/// The step counts the two bytes the language spends before an escape says
/// anything — the opening quote and the backslash — so what fixes it is Rust's
/// own literal syntax rather than a decision of this corpus. No record of this
/// repository states that syntax, and the honest reading is that the citation is
/// owed rather than absent: this module reads the language's lexical shapes by
/// hand, for the reason its own preamble gives, and a hand-written lexer owes a
/// record of which shapes it was written against.
///
/// TODO ´todo:code:record-which-reading-of-the-language´: record which reading of the language's literal syntax this scanner is written against.
///
/// ´const:indexlinter:escape-body-offset´ (´[ORCHESTRATION-alg:const:count]´)
/// ´const:indexlinter:escape-body-offset-count-2´
const ESCAPE_BODY: usize = 2;

/// The characters a comment's own leaders may add to the front of a line.
///
/// The set is not a choice any reader of a comment makes. A span is logical
/// rather than a run of bytes, and a comment's own leaders are resolved away
/// before spans are determined (´[ORCHESTRATION-gram:labels:well-formed]´); what a Rust
/// comment can put in front of a line is exactly the solidus of every comment
/// opener, the bang and the second solidus that mark the two documentation
/// forms, the asterisk a block comment's continuation carries, and the space and
/// tab of indentation. So the set is that grammar's requirement read against
/// this language, and a character outside it is content the reading must keep.
///
/// It stands here because it is a fact about the language's comment forms rather
/// than about any one profile's sweep, and three of them read it: the constant
/// profile resolves a comment's decoration away before comparing and writing a
/// pin (´[ORCHESTRATION-req:constants:mechanization]´), the to-do profile resolves it
/// before a notice's first word can be read, and the code carrier resolves it
/// before the mints and citations standing in commentary can be found. Declared
/// once, a character added to it reaches all three at once, which is what a set
/// declared three times could not promise.
///
/// ´const:indexlinter:comment-leader-set´ (´[ORCHESTRATION-alg:const:form]´)
/// ´const:indexlinter:comment-leader-set-form-x03112501´
pub const LEADERS: &[char] = &['/', '!', '*', ' ', '\t'];

/// Read every comment of a Rust source, in the order they stand.
///
/// The result is the commentary and nothing else: no byte of a string literal,
/// of a character literal, or of program text appears in any returned range.
#[must_use]
pub fn comment_regions(source: &str) -> Vec<CommentRegion> {
    let bytes = source.as_bytes();
    let mut regions = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        index = match bytes[index] {
            b'/' if bytes.get(index + 1) == Some(&b'/') => line_comment(bytes, index, &mut regions),
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                block_comment(bytes, index, &mut regions)
            }
            b'"' => skip_plain_string(bytes, index),
            b'\'' => skip_quote(source, bytes, index),
            b'b' | b'c' | b'r' if opens_literal(bytes, index) => skip_literal(source, bytes, index),
            // Every other byte is program text. Advancing one byte at a time is
            // safe over multi-byte characters because every byte this scan
            // matches on is ASCII, and no continuation byte can equal one.
            _ => index + 1,
        };
    }

    regions
}

/// Read a line comment, and report where the line it ends ends.
fn line_comment(bytes: &[u8], index: usize, regions: &mut Vec<CommentRegion>) -> usize {
    let start = index + 2;
    let end = bytes
        .get(start..)
        .and_then(|rest| rest.iter().position(|byte| *byte == b'\n'))
        .map_or(bytes.len(), |offset| start + offset);

    regions.push(CommentRegion { start, end });

    end
}

/// Read a block comment, which may hold block comments of its own.
///
/// Rust's block comments nest, so the closing delimiter of an inner comment does
/// not close the outer one. A scanner that stops at the first closing delimiter
/// would read the rest of the outer comment as program text, and the first
/// string literal standing after it would be read as commentary.
fn block_comment(bytes: &[u8], index: usize, regions: &mut Vec<CommentRegion>) -> usize {
    let start = index + 2;
    let mut depth = 1_usize;
    let mut cursor = start;

    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'/' && bytes[cursor + 1] == b'*' {
            depth += 1;
            cursor += 2;
        } else if bytes[cursor] == b'*' && bytes[cursor + 1] == b'/' {
            depth -= 1;
            cursor += 2;

            if depth == 0 {
                regions.push(CommentRegion {
                    start,
                    end: cursor - 2,
                });

                return cursor;
            }
        } else {
            cursor += 1;
        }
    }

    // An unterminated block comment runs to the end of the source, which is what
    // the compiler's lexer says it does. Reading it as commentary censuses what
    // stands in it; reading it as program text would census nothing and say so.
    regions.push(CommentRegion {
        start,
        end: bytes.len(),
    });

    bytes.len()
}

/// Whether a byte may stand inside an identifier.
const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether a prefix byte opens a literal here rather than continuing a word.
///
/// The prefixes are single letters, so the test that keeps `crate` and `bar`
/// out is the one the language itself applies: a prefix is a prefix only where
/// an identifier could begin.
fn opens_literal(bytes: &[u8], index: usize) -> bool {
    let continues_a_word = index > 0 && is_identifier_byte(bytes[index - 1]);

    !continues_a_word && literal_opening(bytes, index).is_some()
}

/// What the literal prefix at an offset opens, when it opens anything.
fn literal_opening(bytes: &[u8], index: usize) -> Option<Opening> {
    let (rest, raw) = match bytes[index] {
        b'r' => (index + 1, true),
        b'b' if bytes.get(index + 1) == Some(&b'\'') => return Some(Opening::Quote(index + 1)),
        b'b' | b'c' if bytes.get(index + 1) == Some(&b'r') => (index + 2, true),
        b'b' | b'c' => (index + 1, false),
        _ => return None,
    };

    if raw {
        // A hash run followed by a quote opens a raw string; a hash run followed
        // by anything else is a raw identifier, which opens nothing.
        let hashes = bytes[rest..]
            .iter()
            .take_while(|byte| **byte == b'#')
            .count();

        (bytes.get(rest + hashes) == Some(&b'"')).then_some(Opening::Raw(rest, hashes))
    } else {
        (bytes.get(rest) == Some(&b'"')).then_some(Opening::Plain(rest))
    }
}

/// Skip the literal a prefix opens, and report where it ends.
fn skip_literal(source: &str, bytes: &[u8], index: usize) -> usize {
    match literal_opening(bytes, index) {
        Some(Opening::Plain(quote)) => skip_plain_string(bytes, quote),
        Some(Opening::Raw(run, count)) => skip_raw_string(bytes, run, count),
        Some(Opening::Quote(quote)) => skip_quote(source, bytes, quote),
        None => index + 1,
    }
}

/// Skip a string with escapes, and report where it ends.
const fn skip_plain_string(bytes: &[u8], quote: usize) -> usize {
    let mut index = quote + 1;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }

    bytes.len()
}

/// Skip a raw string, which has no escapes and closes on its own hash run.
fn skip_raw_string(bytes: &[u8], run: usize, count: usize) -> usize {
    let mut index = run + count + 1;

    while index < bytes.len() {
        let closes = bytes[index] == b'"'
            && bytes.get(index + 1..).is_some_and(|rest| {
                rest.iter()
                    .take(count)
                    .filter(|byte| **byte == b'#')
                    .count()
                    == count
            });

        if closes {
            return index + 1 + count;
        }

        index += 1;
    }

    bytes.len()
}

/// Skip whatever a quote opens, which is a character literal or a lifetime.
///
/// This is the ambiguity that makes a byte-wise scanner wrong. A lifetime opens
/// with a quote and never closes with one, so a scanner that reads every quote
/// as a character literal reads from one lifetime to the next quote in the file
/// as literal text — and every comment standing between them disappears.
fn skip_quote(source: &str, bytes: &[u8], quote: usize) -> usize {
    if bytes.get(quote + 1) == Some(&b'\\') {
        return escaped_character(bytes, quote);
    }

    let Some(character) = source[quote + 1..].chars().next() else {
        return quote + 1;
    };
    let close = quote + 1 + character.len_utf8();

    // Exactly one character and then a quote is a character literal. Anything
    // else is a lifetime, which consumes only its own opening quote.
    if bytes.get(close) == Some(&b'\'') {
        close + 1
    } else {
        quote + 1
    }
}

/// Skip a character literal whose character is written as an escape.
///
/// The escaped quote is the case worth naming: its body is itself a quote, so a
/// scanner looking for the next quote after the backslash closes the literal one
/// character early and reads the real closing quote as the opening of another.
fn escaped_character(bytes: &[u8], quote: usize) -> usize {
    let body = quote + ESCAPE_BODY;

    let close = if bytes.get(body) == Some(&b'u') {
        match bytes[body..].iter().position(|byte| *byte == b'}') {
            Some(offset) => body + offset + 1,
            None => return quote + 1,
        }
    } else {
        body + 1
    };

    if bytes.get(close) == Some(&b'\'') {
        close + 1
    } else {
        quote + 1
    }
}

#[cfg(test)]
mod tests {
    use super::comment_regions;

    fn comments(source: &str) -> Vec<&str> {
        comment_regions(source)
            .iter()
            .map(|region| region.text(source))
            .collect()
    }

    /// All three line-comment spellings are commentary: the plain one and both
    /// documentation forms, inner and outer. Which one an author reached for
    /// does not change whether what follows is prose.
    ///
    /// ´claim:comment:every-line-comment-spelling-is-commentary´
    /// ´test:unit:reads-every-line-comment-form´
    #[test]
    fn reads_every_line_comment_form() {
        let source = "//! inner\n/// outer\n// plain\nfn main() {}\n";

        assert_eq!(comments(source), ["! inner", "/ outer", " plain"]);
    }

    /// Block comments nest as the language says they do: an inner comment's
    /// close does not end the outer one early, and separate blocks on a line
    /// are read as the separate comments they are.
    ///
    /// ´claim:comment:block-comments-nest-and-close-at-their-own-depth´
    /// ´test:unit:reads-block-comments-and-their-nesting´
    #[test]
    fn reads_block_comments_and_their_nesting() {
        // The interior begins after the opening delimiter, so the third solidus
        // of a documentation block stands in it. No shape any burn list counts
        // can begin with it, so it is left where it is rather than trimmed.
        assert_eq!(comments("/** doc */\n"), ["* doc "]);
        assert_eq!(
            comments("/* outer /* inner */ still outer */\n"),
            [" outer /* inner */ still outer "]
        );
        assert_eq!(
            comments("/* one */ let x = 1; /* two */\n"),
            [" one ", " two "],
            "a nested comment's close does not close the outer one early"
        );
    }

    /// A block comment nobody closed runs to the end of the file rather than
    /// aborting the scan, so a source that is not valid Rust can still be
    /// censused — which is the condition a migrating corpus is usually in.
    ///
    /// ´claim:comment:an-unclosed-block-comment-runs-to-the-end-of-the-source´
    /// ´test:unit:reads-an-unterminated-block-comment-to-the-end´
    #[test]
    fn reads_an_unterminated_block_comment_to_the_end() {
        assert_eq!(
            comments("/* never closed\nmore\n"),
            [" never closed\nmore\n"]
        );
    }

    /// A string literal is never commentary, whatever it holds: a URL whose
    /// authority is introduced by two solidi, a comment delimiter written out
    /// as data, an escaped quote, a trailing pair of backslashes. Without this
    /// the census would report a linter's own fixtures as corpus references.
    ///
    /// ´claim:comment:a-string-literal-is-never-read-as-commentary´
    /// ´test:unit:leaves-string-literals-unread´
    #[test]
    fn leaves_string_literals_unread() {
        let quiet = [
            "let url = \"https://example.test/path\";\n",
            "let block = \"/* not a comment */\";\n",
            "let escaped = \"a \\\" quote // and solidi\";\n",
            "let trailing = \"ends in backslash \\\\\";\n",
        ];

        for source in quiet {
            assert_eq!(comments(source), Vec::<&str>::new(), "on: {source}");
        }
    }

    /// A raw string closes on the hash run it opened with and on no earlier
    /// quote, at every hash count the language admits. A raw string may
    /// therefore carry a quote, a shorter hash run, or a comment delimiter
    /// without ending, and commentary after it is still found.
    ///
    /// ´claim:comment:a-raw-string-closes-on-its-own-hash-run-and-no-earlier´
    /// ´test:unit:leaves-raw-strings-of-every-hash-count-unread´
    #[test]
    fn leaves_raw_strings_of_every_hash_count_unread() {
        let quiet = [
            "let zero = r\"// not a comment\";\n",
            "let one = r#\"// not a comment \" either\"#;\n",
            "let two = r##\"holds \"# and // both\"##;\n",
        ];

        for source in quiet {
            assert_eq!(comments(source), Vec::<&str>::new(), "on: {source}");
        }

        assert_eq!(
            comments("let one = r#\"// inside\"#; // outside\n"),
            [" outside"],
            "a raw string closes on its own hash run and no earlier"
        );
    }

    /// A prefix does not turn a literal into commentary: byte strings, C
    /// strings, their raw forms and byte characters are all program text
    /// holding whatever they hold.
    ///
    /// (´claim:comment:a-string-literal-is-never-read-as-commentary´)
    /// ´test:unit:leaves-prefixed-literals-unread´
    #[test]
    fn leaves_prefixed_literals_unread() {
        let quiet = [
            "let bytes = b\"// not a comment\";\n",
            "let raw = br#\"// not a comment\"#;\n",
            "let text = c\"// not a comment\";\n",
            "let raw = cr#\"// not a comment\"#;\n",
            "let byte = b'/';\n",
        ];

        for source in quiet {
            assert_eq!(comments(source), Vec::<&str>::new(), "on: {source}");
        }
    }

    /// A literal prefix is only a prefix where a literal follows it: a raw
    /// identifier, and an ordinary word ending in a letter that elsewhere
    /// introduces a literal, both leave the comment after them intact.
    ///
    /// ´claim:comment:a-prefix-letter-only-counts-where-a-literal-follows-it´
    /// ´test:unit:leaves-words-ending-in-a-prefix-letter-alone´
    #[test]
    fn leaves_words_ending_in_a_prefix_letter_alone() {
        // A raw identifier is a hash run that no quote follows, and the words
        // below all end in a letter that is also a literal prefix; neither may
        // swallow the comment standing after it.
        let source = "let r#fn = crate::verb(); // kept\nlet cab = 1; // also kept\n";

        assert_eq!(comments(source), [" kept", " also kept"]);
    }

    /// A leading quote that opens a lifetime is not read as opening a character
    /// literal, so a comment after a function or a structure carrying lifetimes
    /// is still found. A scanner that guessed the other way would swallow
    /// everything up to the next quote in the file.
    ///
    /// ´claim:comment:a-lifetime-quote-does-not-open-a-literal´
    /// ´test:unit:reads-comments-after-a-lifetime´
    #[test]
    fn reads_comments_after_a_lifetime() {
        let source = "fn f<'a>(x: &'a str) -> &'a str { x } // kept\n";

        assert_eq!(comments(source), [" kept"]);

        let bounded = "struct S<'long, 'other>(&'long str, &'other str); // kept\n";

        assert_eq!(comments(bounded), [" kept"]);
    }

    /// A character literal ends where it really ends, through every escape the
    /// language admits — an escaped quote, an escaped backslash, a named
    /// escape, a numeric one, and a character written directly outside ASCII —
    /// so the comment standing after any of them is read.
    ///
    /// ´claim:comment:a-character-literal-ends-correctly-through-every-escape´
    /// ´test:unit:reads-comments-after-every-character-literal´
    #[test]
    fn reads_comments_after_every_character_literal() {
        let sources = [
            "let c = 'a'; // kept\n",
            "let c = '\\''; // kept\n",
            "let c = '\\\\'; // kept\n",
            "let c = '\\n'; // kept\n",
            "let c = '\\u{1F600}'; // kept\n",
            "let c = '\u{a7}'; // kept\n",
        ];

        for source in sources {
            assert_eq!(comments(source), [" kept"], "on: {source}");
        }
    }

    /// Commentary is text and is not lexed any further: an unpaired quote
    /// inside a comment opens nothing, and the code and comments after it are
    /// read normally.
    ///
    /// ´claim:comment:commentary-is-text-and-is-not-lexed-further´
    /// ´test:unit:reads-a-comment-holding-a-quote´
    #[test]
    fn reads_a_comment_holding_a_quote() {
        let source = "// an unpaired \" quote\nlet kept = 1; // after\n";

        assert_eq!(
            comments(source),
            [" an unpaired \" quote", " after"],
            "commentary is text, not something to lex further"
        );
    }

    /// The regions a scan reports fall on character boundaries, so text outside
    /// ASCII in a comment, in a string between comments, or in a block comment
    /// can be taken as text without the slice panicking.
    ///
    /// ´claim:comment:comment-regions-fall-on-character-boundaries´
    /// ´test:unit:keeps-regions-on-character-boundaries´
    #[test]
    fn keeps_regions_on_character_boundaries() {
        // Slicing at a byte that is not a character boundary panics, so a region
        // that survives being taken as text is a region that respects them.
        let source = "// \u{a7}10.3 and \u{e9}\nlet x = \"\u{e9}\"; /* \u{a7}4.2 */\n";

        assert_eq!(comments(source), [" \u{a7}10.3 and \u{e9}", " \u{a7}4.2 "]);
    }
}
