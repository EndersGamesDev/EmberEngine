// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The comment-leader catalog and the header region it defines.
//!
//! A licence header is bytes at the front of a file, and reading them takes one
//! fact the linter cannot ask the file for: how that file spells a comment. The
//! catalog answers it, keyed by the final path component, and it is code rather
//! than declared data for the reason the dependency map is: a human ruling
//! selects which policies apply, while the linter defines what those policies
//! mean, and how Rust spells a comment is not a repository choice.
//!
//! # The catalog lists line comments and nothing else
//!
//! An entry names a line-comment leader and, where a language gives that leader
//! a documentation form, the prefixes that are *not* header lines. A type whose
//! only comment form is a block has no entry, because a block form admits
//! several spellings of one header — one comment per line, or one comment
//! spanning both — and choosing between them would be a canonical-form rule no
//! ruling has made. A type with no comment form at all has no entry for the
//! ordinary reason.
//!
//! A governed file whose selector the catalog does not name can never conform,
//! and the section that governs it says so at configuration time rather than
//! failing it forever. So the declared pattern and this catalog decide together
//! whether a file is a header carrier, and the linter never sniffs a file to
//! guess whether it is binary.
//!
//! # The region, and why it is measured rather than searched
//!
//! The region is the maximal run of lines from the file's start — after an
//! interpreter line, where one stands first — whose bytes begin with the
//! catalogued leader and do not begin with one of that entry's excluded
//! prefixes. Reading the front of the file rather than searching it is what
//! keeps a header that is only *mentioned* from counting: this crate's own index
//! generator carries both header lines inside a string literal as its fixture,
//! and a whole-file search reports a header that file does not carry.
//!
//! The shebang exemption is measured rather than assumed. The corpus's one
//! headed shell script carries its two header lines at the second and third
//! lines, under an interpreter line, and an interpreter line that is not first
//! is not an interpreter line.
//!
//! # The exact line form
//!
//! A header line's bytes are the leader, one space, the field name and its
//! colon, one space, and the text — and nothing further. A line that spells the
//! field any other way is a comment mentioning the field rather than a header
//! line, which is the reading that keeps one spelling per header: a surface that
//! accepted a second spelling would be a surface on which two texts both satisfy
//! one requirement.
//!
//! The region is never decoded. Comparison is byte equality against the
//! declared text's own bytes, in keeping with the rule that this crate compares
//! bytes and never calls a lossy conversion.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`answers_for_a_type_by_its_final_component`] | leader | The catalog is keyed by the final path component, so a suffix entry answers for a file at any depth and a whole-name entry answers for the extensionless files that carry one. A type the catalog does not name is answered for by nobody, which is what makes an uncatalogued governed file a configuration finding rather than a file that fails forever. |
//! | [`the_region_is_the_leading_run_of_ordinary_comments`] | leader | The region is the maximal leading run of lines beginning with the catalogued leader, and the first line that is not one ends it. A comment standing after program text is outside the region however early it stands, because a header is at the front of the file or it is not a header. |
//! | [`an_interpreter_line_moves_the_region_down_one`] | leader | An interpreter line first in the file moves the region to the second line, and the same bytes anywhere else are ordinary content that ends the region. The exemption is what the corpus's one headed script needs and it is measured from that script rather than assumed. |
//! | [`a_documentation_prefix_is_not_a_header_line`] | leader | A documentation comment is not a header line, so a run of them opens no region and a module documentation line that merely mentions the header text is never read as one. The exclusions are the index generator's own, which had to know where a header ends before it could write. |
//! | [`a_header_line_is_the_exact_form_or_it_is_nothing`] | leader | A header line is the leader, one space, the field and its colon, one space and the text, and a line spelling the field any other way carries no header at all. One spelling per header is what keeps the declared text meaningful: a surface admitting a second spelling would be one on which two texts satisfy one requirement. |
//! | [`the_region_is_read_as_bytes_and_never_decoded`] | leader | The region is read as bytes, so a file whose content is not text is measured rather than refused, and a header line's text is compared byte for byte against what the declaration asks for. |
//! | [`no_two_entries_answer_for_one_selector`] | leader | Every selector in the catalog is answered for by the entry that carries it, which is the property that makes the catalog a function of the final component rather than a list a reader has to scan in order. |

use crate::pattern::BytePath;

/// The bytes an interpreter line opens with.
///
/// It stands first in a file or it is not an interpreter line, which is what
/// makes the exemption a rule about position rather than about these two bytes.
const SHEBANG: &[u8] = b"#!";

/// One catalogued file type: what opens a comment in it, and what does not open a header line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leader {
    /// The final components and suffixes this entry answers for.
    ///
    /// A selector opening with a dot is a suffix of the final component; any
    /// other selector is the final component entire.
    selectors: &'static [&'static str],
    /// The bytes a line comment opens with.
    prefix: &'static str,
    /// The prefixes that open a comment and never a header line.
    excluded: &'static [&'static str],
}

impl Leader {
    /// The bytes a line comment opens with.
    #[must_use]
    pub const fn leader(&self) -> &'static str {
        self.prefix
    }

    /// Whether a line stands inside the header region.
    ///
    /// The bytes are read from the first, with no leading whitespace tolerated:
    /// a header is at column one or it is somewhere a reader has to hunt for it.
    #[must_use]
    pub fn opens_region(&self, line: &[u8]) -> bool {
        line.starts_with(self.prefix.as_bytes())
            && !self
                .excluded
                .iter()
                .any(|prefix| line.starts_with(prefix.as_bytes()))
    }

    /// The header region of a source: the leading run of comment lines, after any interpreter line.
    ///
    /// The bytes are split on the line feed and never decoded, so a file whose
    /// content is not text is measured like any other.
    #[must_use]
    pub fn region<'a>(&self, source: &'a [u8]) -> Vec<&'a [u8]> {
        let mut lines = source.split(|byte| *byte == b'\n');

        if source.starts_with(SHEBANG) {
            let _interpreter = lines.next();
        }

        lines.take_while(|line| self.opens_region(line)).collect()
    }

    /// The exact bytes a header line for this field and text is written as.
    #[must_use]
    pub fn header_line(&self, field: &str, text: &str) -> Vec<u8> {
        format!("{} {field}: {text}", self.prefix).into_bytes()
    }

    /// The text a region line declares for a field, when the line is one.
    ///
    /// A line spelling the field any other way declares nothing, because the
    /// exact form is the whole of what a header line is.
    #[must_use]
    pub fn declared<'a>(&self, line: &'a [u8], field: &str) -> Option<&'a [u8]> {
        let prefix = format!("{} {field}: ", self.prefix);

        line.strip_prefix(prefix.as_bytes())
    }
}

/// Every file type this binary knows how to read a header out of.
///
/// The Rust row is not invented here: this crate's index generator already had
/// to know where a leading header ends before it could write below it, and it
/// already excluded the two documentation forms. Those exclusions are this row,
/// and that generator is now a consumer of it rather than a Rust-only special
/// case.
pub const CATALOG: &[Leader] = &[
    Leader {
        selectors: &[".rs"],
        prefix: "//",
        excluded: &["//!", "///"],
    },
    Leader {
        selectors: &[".c", ".h"],
        prefix: "//",
        excluded: &[],
    },
    Leader {
        selectors: &[".sh", ".py"],
        prefix: "#",
        excluded: &[],
    },
    Leader {
        selectors: &[".toml", ".lock"],
        prefix: "#",
        excluded: &[],
    },
    Leader {
        selectors: &[".yaml", ".yml"],
        prefix: "#",
        excluded: &[],
    },
    Leader {
        selectors: &[".sql"],
        prefix: "--",
        excluded: &["---"],
    },
    Leader {
        selectors: &["Makefile", "Containerfile", "Dockerfile", "CODEOWNERS"],
        prefix: "#",
        excluded: &[],
    },
];

/// The selector the Rust row answers for.
const RUST_SELECTOR: &str = ".rs";

/// The catalogued entry for Rust sources.
///
/// Every other caller asks the catalog by path. This one is answered by name,
/// because it is the index generator: it reads a Rust source it is about to
/// write Rust module documentation into, so the file type is not in question and
/// asking by path would mean inventing a path to ask with. The answer is looked
/// up in the catalog rather than restated, so the two cannot drift apart.
///
/// # Panics
///
/// Panics when the catalog carries no Rust row, which is a defect of this file
/// rather than a condition any tree can produce.
#[must_use]
pub fn rust() -> &'static Leader {
    CATALOG
        .iter()
        .find(|entry| entry.selectors.contains(&RUST_SELECTOR))
        .expect("the catalog carries a Rust row")
}

/// The catalogued entry for a path, or `None` when this binary knows no comment form for it.
#[must_use]
pub fn catalogued(path: &BytePath) -> Option<&'static Leader> {
    let component = final_component(path.as_bytes());

    CATALOG.iter().find(|entry| {
        entry
            .selectors
            .iter()
            .any(|selector| answers_for(selector, component))
    })
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
    use super::{CATALOG, catalogued};
    use crate::pattern::BytePath;

    /// The catalogued entry for a path spelled as text, failing the test when
    /// the path itself will not decode.
    fn answering_for(path: &str) -> Option<&'static super::Leader> {
        catalogued(&BytePath::decode(path).expect("a decodable path"))
    }

    /// The catalog is keyed by the final path component, so a suffix entry
    /// answers for a file at any depth and a whole-name entry answers for the
    /// extensionless files that carry one. A type the catalog does not name is
    /// answered for by nobody, which is what makes an uncatalogued governed file
    /// a configuration finding rather than a file that fails forever.
    ///
    /// ´claim:leader:the-catalog-answers-by-the-final-path-component´
    /// ´test:unit:answers-for-a-type-by-its-final-component´
    #[test]
    fn answers_for_a_type_by_its_final_component() {
        assert_eq!(
            answering_for("src/lib.rs").map(super::Leader::leader),
            Some("//")
        );
        assert_eq!(
            answering_for("packages/deep/nested/main.rs").map(super::Leader::leader),
            Some("//")
        );
        assert_eq!(
            answering_for("migrations/001-one.sql").map(super::Leader::leader),
            Some("--")
        );
        assert_eq!(
            answering_for("Cargo.lock").map(super::Leader::leader),
            Some("#")
        );
        assert_eq!(
            answering_for("Makefile").map(super::Leader::leader),
            Some("#")
        );
        assert_eq!(
            answering_for("share/container/Containerfile").map(super::Leader::leader),
            Some("#")
        );

        // A whole-name selector is the component entire and never a suffix of
        // it, so a file whose name merely ends in one is not that file type.
        assert!(answering_for("contrib/NotAMakefile").is_none());

        // The types the corpus holds that no comment form reaches.
        for path in [
            "assets/logo.png",
            "fixtures/one.torrent",
            "data/one.json",
            "share/font.ttf",
        ] {
            assert!(
                answering_for(path).is_none(),
                "{path} carries no line comment"
            );
        }
    }

    /// The region is the maximal leading run of lines beginning with the
    /// catalogued leader, and the first line that is not one ends it. A comment
    /// standing after program text is outside the region however early it
    /// stands, because a header is at the front of the file or it is not a
    /// header.
    ///
    /// ´claim:leader:the-region-is-the-leading-run-and-the-first-other-line-ends-it´
    /// ´test:unit:the-region-is-the-leading-run-of-ordinary-comments´
    #[test]
    fn the_region_is_the_leading_run_of_ordinary_comments() {
        let entry = answering_for("src/lib.rs").expect("the Rust row");

        let source = b"// one\n// two\n\n// three\ncode();\n";
        let region = entry.region(source);

        assert_eq!(
            region,
            vec![&b"// one"[..], &b"// two"[..]],
            "the blank line ends the run"
        );

        // A file opening with program text has no region at all, whatever
        // stands below it.
        assert_eq!(entry.region(b"code();\n// one\n"), Vec::<&[u8]>::new());

        // And a run that reaches the end of the file is the whole of it.
        assert_eq!(entry.region(b"// one\n// two").len(), 2);
    }

    /// An interpreter line first in the file moves the region to the second
    /// line, and the same bytes anywhere else are ordinary content that ends the
    /// region. The exemption is what the corpus's one headed script needs and it
    /// is measured from that script rather than assumed.
    ///
    /// ´claim:leader:an-interpreter-line-stands-first-or-it-is-not-one´
    /// ´test:unit:an-interpreter-line-moves-the-region-down-one´
    #[test]
    fn an_interpreter_line_moves_the_region_down_one() {
        let entry = answering_for("contrib/dev-tools/one.sh").expect("the shell row");

        let headed = b"#!/usr/bin/env bash\n# SPDX-License-Identifier: AGPL-3.0-only\n# SPDX-FileCopyrightText: 2026 Wild Sky Maker\n\nset -e\n";

        assert_eq!(
            entry.region(headed),
            vec![
                &b"# SPDX-License-Identifier: AGPL-3.0-only"[..],
                &b"# SPDX-FileCopyrightText: 2026 Wild Sky Maker"[..],
            ],
            "the two header lines stand under the interpreter line"
        );

        // The mark is the shell's own comment mark, so an interpreter line that
        // is not first is an ordinary comment and stands inside the region.
        let late = b"# one\n#!/usr/bin/env bash\n";
        assert_eq!(entry.region(late).len(), 2);

        // And under a leader the interpreter line does not share, a file opening
        // with one has no region: the exemption removes the line from the run
        // and the run then begins at a line that opens no comment.
        let rust = answering_for("src/main.rs").expect("the Rust row");
        assert_eq!(
            rust.region(b"#!/usr/bin/env run-cargo-script\ncode();\n"),
            Vec::<&[u8]>::new()
        );
    }

    /// A documentation comment is not a header line, so a run of them opens no
    /// region and a module documentation line that merely mentions the header
    /// text is never read as one. The exclusions are the index generator's own,
    /// which had to know where a header ends before it could write.
    ///
    /// ´claim:leader:a-documentation-prefix-is-not-a-header-line´
    /// ´test:unit:a-documentation-prefix-is-not-a-header-line´
    #[test]
    fn a_documentation_prefix_is_not_a_header_line() {
        let entry = answering_for("src/lib.rs").expect("the Rust row");

        assert!(
            entry
                .region(b"//! SPDX-License-Identifier: AGPL-3.0-only\ncode();\n")
                .is_empty(),
            "a module documentation line mentioning the text is not a header"
        );
        assert_eq!(entry.region(b"/// one\ncode();\n"), Vec::<&[u8]>::new());

        // The exclusion is a prefix rule and not a whole-line one, so an
        // ordinary comment that merely opens with the same two bytes is
        // unaffected.
        assert_eq!(
            entry.region(b"// one\n//! two\n").len(),
            1,
            "the doc line ends the run"
        );

        // The SQL row's exclusion is the same shape: its leader is two bytes and
        // its documentation form is three.
        let sql = answering_for("migrations/001-one.sql").expect("the SQL row");
        assert_eq!(sql.region(b"--- one\n"), Vec::<&[u8]>::new());
        assert_eq!(sql.region(b"-- one\n").len(), 1);
    }

    /// A header line is the leader, one space, the field and its colon, one
    /// space and the text, and a line spelling the field any other way carries
    /// no header at all. One spelling per header is what keeps the declared text
    /// meaningful: a surface admitting a second spelling would be one on which
    /// two texts satisfy one requirement.
    ///
    /// ´claim:leader:a-header-line-is-the-exact-form-or-it-is-nothing´
    /// ´test:unit:a-header-line-is-the-exact-form-or-it-is-nothing´
    #[test]
    fn a_header_line_is_the_exact_form_or_it_is_nothing() {
        let entry = answering_for("src/lib.rs").expect("the Rust row");
        let field = "SPDX-License-Identifier";

        assert_eq!(
            entry.header_line(field, "AGPL-3.0-only"),
            b"// SPDX-License-Identifier: AGPL-3.0-only".to_vec()
        );
        assert_eq!(
            entry.declared(b"// SPDX-License-Identifier: AGPL-3.0-only", field),
            Some(&b"AGPL-3.0-only"[..])
        );

        // Every other spelling declares nothing: no space after the colon, two
        // spaces after the leader, leading whitespace, and a carriage return the
        // text would otherwise carry.
        for line in [
            &b"// SPDX-License-Identifier:AGPL-3.0-only"[..],
            &b"//  SPDX-License-Identifier: AGPL-3.0-only"[..],
            &b"  // SPDX-License-Identifier: AGPL-3.0-only"[..],
        ] {
            assert_eq!(
                entry.declared(line, field),
                None,
                "{}",
                String::from_utf8_lossy(line)
            );
        }

        // A carriage return is inside the declared text rather than outside it,
        // so the comparison the caller makes against a text that has none fails
        // rather than passing on a line the surface does not admit.
        assert_eq!(
            entry.declared(b"// SPDX-License-Identifier: AGPL-3.0-only\r", field),
            Some(&b"AGPL-3.0-only\r"[..])
        );
    }

    /// The region is read as bytes, so a file whose content is not text is
    /// measured rather than refused, and a header line's text is compared byte
    /// for byte against what the declaration asks for.
    ///
    /// ´claim:leader:the-region-is-read-as-bytes-and-never-decoded´
    /// ´test:unit:the-region-is-read-as-bytes-and-never-decoded´
    #[test]
    fn the_region_is_read_as_bytes_and_never_decoded() {
        let entry = answering_for("src/lib.rs").expect("the Rust row");

        let mut source = b"// SPDX-License-Identifier: AGPL-3.0-only\n".to_vec();
        source.extend_from_slice(&[0xff, 0xfe, 0x00]);
        source.push(b'\n');

        let region = entry.region(&source);

        assert_eq!(
            region.len(),
            1,
            "the run ends where the bytes stop opening a comment"
        );
        assert_eq!(
            entry.declared(region[0], "SPDX-License-Identifier"),
            Some(&b"AGPL-3.0-only"[..]),
            "and the line before it is read without the file having been decoded"
        );
    }

    /// Every selector in the catalog is answered for by the entry that carries
    /// it, which is the property that makes the catalog a function of the final
    /// component rather than a list a reader has to scan in order.
    ///
    /// ´claim:leader:no-two-entries-answer-for-one-selector´
    /// ´test:unit:no-two-entries-answer-for-one-selector´
    #[test]
    fn no_two_entries_answer_for_one_selector() {
        let mut seen: Vec<&str> = CATALOG
            .iter()
            .flat_map(|entry| entry.selectors.iter().copied())
            .collect();
        let count = seen.len();

        seen.sort_unstable();
        seen.dedup();

        assert_eq!(
            seen.len(),
            count,
            "one selector is answered for by one entry"
        );
    }
}
