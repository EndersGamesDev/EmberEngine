// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The comment-carrier catalog: which reader answers for a tracked file kind.
//!
//! The tokenization layer already knew how to read two languages, because two
//! were all any rule had ever asked it for: a Rust source and a shell script.
//! The file-path citation policy asks for the rest. Its ruling reaches comments
//! wherever they are written, and a program cannot claim it met that reach by
//! declining to ask whether a file type has comments at all. So the question
//! "what does this kind spell a comment with" is answered here, once, for every
//! kind, and the answer is code rather than declared data for the reason the
//! header-leader catalog beside it is: how a language spells a comment is a
//! property of the language and never a repository choice.
//!
//! # Total, and loud where it is not
//!
//! Every catalogued kind gets a reader or an explicit statement that it has no
//! comment region. A binary or opaque type is classified rather than decoded and
//! sniffed, and a plain-text notice carrying no comment syntax is classified the
//! same way — both are *nothing to read here*, which is a different answer from
//! *nobody asked*. A kind the catalog does not name is answered for by nobody,
//! and the policy that consumes this catalog reports that silence rather than
//! passing the file. This is deliberately unlike the interchange program's
//! ruled behaviour for a carrier it cannot read: that policy was ruled to ignore
//! what it does not know, and this one was ruled to reach the comments.
//!
//! # Five readers, not one leader search
//!
//! The catalog names a *reader* and not a leader string, because a leader-only
//! search is exactly the premise error the census that sized this policy warned
//! about. A pair of solidi inside a Rust string, a hash inside a quoted shell
//! word and a dash pair inside a SQL string are not comment openers merely
//! because a search sees them. Each reader is lexical enough to keep quoted data
//! out, and the boundary of what each one knows is stated at its row rather than
//! implied.
//!
//! Prose is a reader here too. A Markdown document's citation-bearing regions
//! come from the Markdown reader rather than from a comment scan, and saying so
//! in the catalog is what makes the classification total over a corpus whose
//! largest textual kind is prose.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`answers_for_a_kind_by_its_final_component`] | commentary | The catalog is keyed by the final path component, so a suffix row answers for a file at any depth and a whole-name row answers for the extensionless files that carry one. A kind the catalog has classified for nobody is answered for by nobody, which is what lets the policy report the silence rather than pass the file. |
//! | [`classifies_the_kinds_that_carry_no_comment_region`] | commentary | A kind with no comment syntax is classified rather than left out. A data format that admits no comment, a plain notice and a binary are all read as carrying no comment region, and none of them is decoded or sniffed to find that out. Prose is classified too, and is the one classified kind that is scanned without being a comment carrier. |
//! | [`no_two_rows_answer_for_one_selector`] | commentary | Every selector in the catalog is answered for by the row that carries it, which is the property that makes the catalog a function of the final component rather than a list a reader has to scan in order. Two rows claiming one selector would make the answer depend on where a row was written. |
//!
//! The index is a generated projection and stands empty until the projection
//! writer fills it.

use crate::pattern::BytePath;

/// The reader that answers for a file kind's citation-bearing regions.
///
/// The variants name syntax rather than languages, because several languages
/// share one comment syntax and a catalog keyed by language would carry the same
/// reader many times over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    /// A Markdown document, whose regions are running text rather than comments.
    Prose,
    /// Line and block comments opened by solidi, as Rust and C spell them.
    Slash,
    /// Line comments opened by a hash, as a shell, TOML, YAML, make and container file spell them.
    Hash,
    /// Line comments opened by a dash pair, as SQL spells them.
    Dash,
    /// Comments delimited by an angle-bracket pair, as HTML spells them.
    Angle,
    /// A kind with no comment region at all: a binary, a data format with no comment syntax, a plain notice.
    Opaque,
}

impl Reader {
    /// Whether the kind carries a comment region a reader could be run over.
    ///
    /// Prose is not a comment carrier, which is a different statement from
    /// carrying no regions: a Markdown document is scanned, by the other reader.
    #[must_use]
    pub const fn reads_comments(self) -> bool {
        matches!(self, Self::Slash | Self::Hash | Self::Dash | Self::Angle)
    }
}

/// One catalogued row: the kinds it answers for, and the reader that answers.
struct Row {
    /// The final components and suffixes this row answers for.
    ///
    /// A selector opening with a dot is a suffix of the final component; any
    /// other selector is the final component entire. The two forms are the
    /// header-leader catalog's own, so a reader who has learned one catalog has
    /// learned both.
    selectors: &'static [&'static str],
    /// The reader that answers for those kinds.
    reader: Reader,
}

/// Every file kind this binary has classified, and what reads it.
///
/// The rows are the corpus's actual textual carriers and the opaque kinds beside
/// them. A row is a language fact wherever a language supplies one; the
/// whole-component rows for extensionless files are the conventional names the
/// header-leader catalog already carries, extended by the named scripts and
/// notices this corpus keeps without a suffix. Between them the rows answer for
/// every path the tracked universe holds, which is what the ruling's reach
/// requires and what the finding for an unclassified kind exists to keep true.
const CATALOG: &[Row] = &[
    Row {
        selectors: &[".md"],
        reader: Reader::Prose,
    },
    Row {
        selectors: &[".rs", ".c", ".h"],
        reader: Reader::Slash,
    },
    Row {
        selectors: &[
            ".sh",
            ".bash",
            ".py",
            ".toml",
            ".lock",
            ".yaml",
            ".yml",
            ".env",
            ".local",
            ".gitignore",
            ".dockerignore",
            ".containerignore",
            ".gitattributes",
            ".editorconfig",
            ".git-blame-ignore",
        ],
        reader: Reader::Hash,
    },
    Row {
        selectors: &["Makefile", "Containerfile", "Dockerfile", "CODEOWNERS"],
        reader: Reader::Hash,
    },
    Row {
        selectors: &["entry_script_sh", "entry_script_lib_sh"],
        reader: Reader::Hash,
    },
    Row {
        selectors: &[".sql"],
        reader: Reader::Dash,
    },
    Row {
        selectors: &[".html", ".htm"],
        reader: Reader::Angle,
    },
    Row {
        selectors: &[
            ".json", ".txt", ".torrent", ".png", ".jpg", ".ttf", ".crt", ".key",
        ],
        reader: Reader::Opaque,
    },
    Row {
        selectors: &[
            "LICENSE",
            "LICENSE-AGPL_3_0",
            "LICENSE-MIT_0",
            "LINKING-EXCEPTION",
            "message",
        ],
        reader: Reader::Opaque,
    },
];

/// The reader catalogued for a path, or `None` when this binary has classified no kind for it.
///
/// `None` is the loud answer rather than the quiet one. The policy consuming
/// this catalog reports an unclassified kind, because a total reach that
/// silently skipped what it had not learned would be a reach in name only.
#[must_use]
pub fn catalogued(path: &BytePath) -> Option<Reader> {
    let component = final_component(path.as_bytes());

    CATALOG
        .iter()
        .find(|row| {
            row.selectors
                .iter()
                .any(|selector| answers_for(selector, component))
        })
        .map(|row| row.reader)
}

/// Whether one selector answers for a final path component.
fn answers_for(selector: &str, component: &[u8]) -> bool {
    if selector.starts_with('.') {
        component.ends_with(selector.as_bytes())
    } else {
        component == selector.as_bytes()
    }
}

/// The bytes after the last separator, which is the whole path when it has none.
fn final_component(path: &[u8]) -> &[u8] {
    path.iter()
        .rposition(|byte| *byte == b'/')
        .map_or(path, |offset| &path[offset + 1..])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{CATALOG, Reader, catalogued};
    use crate::pattern::BytePath;

    /// The reader catalogued for a path spelled as text, failing the test when
    /// the path itself will not decode.
    fn answering_for(path: &str) -> Option<Reader> {
        catalogued(&BytePath::decode(path).expect("a decodable path"))
    }

    /// The catalog is keyed by the final path component, so a suffix row answers
    /// for a file at any depth and a whole-name row answers for the
    /// extensionless files that carry one. A kind the catalog has classified for
    /// nobody is answered for by nobody, which is what lets the policy report
    /// the silence rather than pass the file.
    ///
    /// ´claim:commentary:the-catalog-answers-by-the-final-path-component´
    /// ´test:unit:answers-for-a-kind-by-its-final-component´
    #[test]
    fn answers_for_a_kind_by_its_final_component() {
        assert_eq!(answering_for("orchard/plum.rs"), Some(Reader::Slash));
        assert_eq!(
            answering_for("orchard/quince/deep/plum.rs"),
            Some(Reader::Slash)
        );
        assert_eq!(answering_for("orchard/plum.sql"), Some(Reader::Dash));
        assert_eq!(answering_for("orchard/plum.md"), Some(Reader::Prose));
        assert_eq!(answering_for("orchard/plum.html"), Some(Reader::Angle));
        assert_eq!(answering_for("orchard/Makefile"), Some(Reader::Hash));
        assert_eq!(answering_for("orchard/.gitignore"), Some(Reader::Hash));
        assert_eq!(
            answering_for("orchard/plum.quince"),
            None,
            "an unclassified kind is answered for by nobody rather than by a default"
        );
    }

    /// A kind with no comment syntax is classified rather than left out. A data
    /// format that admits no comment, a plain notice and a binary are all read
    /// as carrying no comment region, and none of them is decoded or sniffed to
    /// find that out. Prose is classified too, and is the one classified kind
    /// that is scanned without being a comment carrier.
    ///
    /// ´claim:commentary:a-kind-with-no-comments-is-classified-rather-than-omitted´
    /// ´test:unit:classifies-the-kinds-that-carry-no-comment-region´
    #[test]
    fn classifies_the_kinds_that_carry_no_comment_region() {
        for path in ["orchard/plum.json", "orchard/plum.png", "orchard/LICENSE"] {
            assert_eq!(
                answering_for(path),
                Some(Reader::Opaque),
                "{path} is classified"
            );
            assert!(
                !answering_for(path)
                    .expect("a classified kind")
                    .reads_comments(),
                "{path} carries no comment region"
            );
        }

        let prose = answering_for("orchard/plum.md").expect("prose is classified");

        assert!(
            !prose.reads_comments(),
            "prose is scanned by the Markdown reader rather than by a comment scan"
        );
        assert!(
            answering_for("orchard/plum.rs")
                .expect("a classified kind")
                .reads_comments(),
            "a source is a comment carrier"
        );
    }

    /// Every selector in the catalog is answered for by the row that carries it,
    /// which is the property that makes the catalog a function of the final
    /// component rather than a list a reader has to scan in order. Two rows
    /// claiming one selector would make the answer depend on where a row was
    /// written.
    ///
    /// ´claim:commentary:no-two-rows-answer-for-one-selector´
    /// ´test:unit:no-two-rows-answer-for-one-selector´
    #[test]
    fn no_two_rows_answer_for_one_selector() {
        let mut seen = BTreeSet::new();

        for row in CATALOG {
            for selector in row.selectors {
                assert!(seen.insert(*selector), "{selector} is claimed by two rows");
            }
        }

        assert!(
            seen.len() > 20,
            "the catalog classifies the corpus's kinds rather than a few"
        );
    }
}
