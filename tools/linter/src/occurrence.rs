// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Occurrence forms and the reader that turns one delimited span into one.
//!
//! ADR-T-014 admits three occurrence forms in two concrete syntaxes. Writing
//! them in ordinary prose rather than in code font, so that this comment neither
//! mints nor cites: a bare span is a mint, a span wrapped in parentheses is a
//! same-owner citation, and a parenthesised span whose interior is a bracketed
//! prefix-label pair is an imported citation. The prose syntax delimits spans
//! with single backticks and the code syntax with acute accents.
//!
//! The well-formed environment (ADR-T-014, A calculus of documentation and source labels) makes the
//! reading total: a span either parses completely as one of the three forms or
//! is ordinary text, and there is no partially well-formed occurrence. Two
//! carve-outs sharpen that. An import-shaped span standing without its
//! parentheses is a hard failure rather than text, because the total-resolution
//! invariant (ADR-T-014, A calculus of documentation and source labels) fails non-parenthesised
//! imports explicitly. And a span that misses a form only by casing, spacing,
//! or stray brackets is reported as a near miss — a warning that changes
//! nothing about the span's status as text.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reads_bare_label_spans_as_mints`] | occurrence | A delimited span holding a bare label and nothing else is a mint: the place the statement is established. Standing unadorned is what makes it one, so minting takes no keyword and no extra punctuation. |
//! | [`reads_parenthesized_label_spans_as_same_owner_citations`] | occurrence | Wrapping the same span in parentheses turns it from a mint into a citation of a statement the owner minted elsewhere. The label is unchanged; the parentheses alone carry the difference between establishing a statement and referring to one. |
//! | [`reads_parenthesized_bracketed_spans_as_imported_citations`] | occurrence | A citation reaches across an ownership boundary by naming the owner: a parenthesised span whose interior brackets a prefix and a label is an imported citation, and the prefix it carries says whose statement is being cited. Prefixes bearing digits work as plain ones do. |
//! | [`fails_import_shaped_spans_without_parentheses`] | occurrence | An import-shaped span standing without its parentheses is a failure rather than ordinary text. Reading it as text would let a citation of another owner pass unresolved simply by dropping two characters, so the reader keeps it and reports it. |
//! | [`reads_unshaped_spans_as_text`] | occurrence | A span matching no occurrence form is ordinary text, whether parenthesised or not. This is what lets prose quote commands, file names, command-line flags and Rust paths in the same delimiters the calculus uses without any of them becoming occurrences. |
//! | [`warns_on_near_miss_spans`] | occurrence | A span that a single named repair would turn into a label is reported as a near miss naming that repair — wrong casing, interior spacing, surrounding whitespace, both at once, or stray brackets. The span stays text; only the author's attention is asked for. |
//! | [`forms_do_not_nest_or_admit_other_bracketing`] | occurrence | The occurrence forms are atomic: they do not nest, do not admit doubled or unbalanced brackets, and do not accept a prefix-label pair with its brackets missing. No arrangement of these becomes an occurrence, so there is no compound form to reason about. |

use serde::Serialize;

use crate::finding::Location;
use crate::label::{Label, Prefix};

/// Which concrete syntax delimited an occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Syntax {
    /// Prose: the span is delimited by single backticks.
    Prose,
    /// Code comments: the span is delimited by acute accents.
    Code,
}

/// Which of the three forms an occurrence takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "form", rename_all = "snake_case")]
pub enum Form {
    /// A bare occurrence, which mints when a warrant admits it.
    Mint,
    /// A parenthesised occurrence citing within its own owner.
    SameOwnerCitation,
    /// A parenthesised, bracketed occurrence citing across an ownership boundary.
    ImportedCitation {
        /// The prefix naming the cited owner.
        prefix: Prefix,
    },
}

/// One occurrence of a label in a carrier source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Occurrence {
    form: Form,
    label: Label,
    syntax: Syntax,
    location: Location,
}

impl Occurrence {
    /// Build an occurrence.
    #[must_use]
    pub const fn new(form: Form, label: Label, syntax: Syntax, location: Location) -> Self {
        Self {
            form,
            label,
            syntax,
            location,
        }
    }

    /// The occurrence form.
    #[must_use]
    pub const fn form(&self) -> &Form {
        &self.form
    }

    /// The label this occurrence carries.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// The concrete syntax that delimited this occurrence.
    #[must_use]
    pub const fn syntax(&self) -> Syntax {
        self.syntax
    }

    /// Where this occurrence stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// Whether this occurrence is a bare one, and so a candidate mint.
    #[must_use]
    pub const fn is_mint(&self) -> bool {
        matches!(self.form, Form::Mint)
    }
}

/// What a single delimited span turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// The span parses completely as one occurrence form.
    Occurrence {
        /// The form the span takes.
        form: Form,
        /// The label the span carries.
        label: Label,
    },
    /// The span is import-shaped but stands without its parentheses.
    NonParenthesizedImport {
        /// The prefix the span named.
        prefix: Prefix,
        /// The label the span named.
        label: Label,
    },
    /// The span is ordinary text that nearly parsed as an occurrence.
    NearMiss {
        /// Why the span did not parse.
        reason: &'static str,
    },
    /// The span is ordinary text.
    Text,
}

/// Read one delimited span, given its logical interior and whether the
/// delimiters are immediately wrapped in parentheses.
///
/// The interior is the span's content after the source format has resolved its
/// own structure away, so this reader never sees delimiters, leaders, or
/// continuation indentation.
pub fn read_span(interior: &str, parenthesized: bool) -> Reading {
    if let Some((prefix, label)) = import_interior(interior) {
        return if parenthesized {
            Reading::Occurrence {
                form: Form::ImportedCitation { prefix },
                label,
            }
        } else {
            Reading::NonParenthesizedImport { prefix, label }
        };
    }

    if let Some(label) = Label::parse(interior) {
        return Reading::Occurrence {
            form: if parenthesized {
                Form::SameOwnerCitation
            } else {
                Form::Mint
            },
            label,
        };
    }

    near_miss_reason(interior).map_or(Reading::Text, |reason| Reading::NearMiss { reason })
}

/// Split a bracketed prefix-label interior, when it is exactly that.
fn import_interior(interior: &str) -> Option<(Prefix, Label)> {
    let inner = interior.strip_prefix('[')?.strip_suffix(']')?;
    let (prefix_text, label_text) = inner.split_once('-')?;

    Some((Prefix::parse(prefix_text)?, Label::parse(label_text)?))
}

/// Report why a span nearly parsed as a label, when it nearly did.
///
/// Only spans that a single named repair turns into a well-formed label are
/// reported, which keeps the warning precise rather than merely suspicious.
fn near_miss_reason(interior: &str) -> Option<&'static str> {
    let trimmed = interior.trim();

    if Label::parse(trimmed).is_some() {
        return Some("surrounding whitespace");
    }

    let unspaced: String = interior
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if Label::parse(&unspaced).is_some() {
        return Some("interior spacing");
    }

    if Label::parse(&trimmed.to_ascii_lowercase()).is_some() {
        return Some("wrong casing");
    }

    if Label::parse(&unspaced.to_ascii_lowercase()).is_some() {
        return Some("wrong casing and spacing");
    }

    let unbracketed = trimmed
        .trim_start_matches(['[', '('])
        .trim_end_matches([']', ')']);
    if Label::parse(unbracketed).is_some() {
        return Some("stray brackets");
    }

    None
}

/// The near-miss reason a parenthesised bracket form without delimiters draws.
///
/// Total resolution asks the checker to warn on near-miss spans — label-shaped
/// interiors wanting only casing, spacing or brackets — without treating any of
/// them as occurrences (´[EMBER-inv:labels:total-resolution]´), and this class is
/// the one such warning no span reader can raise, because the delimiters that
/// would start a reading are exactly what the author left out. The reason stands
/// as a value rather than as a literal at each site because both surfaces that
/// can meet the class, the prose reader and the code reader, must say the same
/// thing about it: a warning worded two ways would read as two classes.
///
/// ´const:emberlinter:undelimited-import-near-miss´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:undelimited-import-near-miss-text-xd14c8bf2´
pub const UNDELIMITED_IMPORT_REASON: &str = "a bracketed import without its span delimiters";

/// Every parenthesis of a text run directly wrapping bracketed import-shaped
/// text with no span delimiters, as pairs of the bracket's byte offset and the
/// bracketed interior.
///
/// This is the near-miss class that no span reader can see: the author wrote
/// the exact bytes an imported citation means, minus the delimiters that would
/// make it one, so reading starts nowhere and the text is silently inert. A
/// correctly delimited citation cannot match, because its parenthesis is
/// followed by a delimiter rather than a bracket.
#[must_use]
pub fn undelimited_imports(text: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut rest = text;
    let mut consumed = 0;

    while let Some(open) = rest.find("([") {
        let after = &rest[open + 1..];

        let Some(close) = after.find(']') else {
            break;
        };

        let candidate = &after[..=close];

        if after[close + 1..].starts_with(')')
            && matches!(read_span(candidate, true), Reading::Occurrence { .. })
        {
            found.push((consumed + open + 1, candidate));
        }

        consumed += open + 1;
        rest = &rest[open + 1..];
    }

    found
}

/// Build an occurrence at a location from a reading, when the reading is one.
pub fn occurrence_from_reading(
    reading: &Reading,
    syntax: Syntax,
    location: &Location,
) -> Option<Occurrence> {
    match reading {
        Reading::Occurrence { form, label } => Some(Occurrence::new(
            form.clone(),
            label.clone(),
            syntax,
            location.clone(),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Form, Reading, read_span};
    use crate::label::{Label, Prefix};

    fn label(text: &str) -> Label {
        Label::parse(text).expect("well-formed")
    }

    /// A delimited span holding a bare label and nothing else is a mint: the
    /// place the statement is established. Standing unadorned is what makes it
    /// one, so minting takes no keyword and no extra punctuation.
    ///
    /// ´claim:occurrence:a-bare-span-is-a-mint´
    /// ´test:unit:reads-bare-label-spans-as-mints´
    #[test]
    fn reads_bare_label_spans_as_mints() {
        assert_eq!(
            read_span("sec:labels:syntax", false),
            Reading::Occurrence {
                form: Form::Mint,
                label: label("sec:labels:syntax"),
            }
        );
    }

    /// Wrapping the same span in parentheses turns it from a mint into a
    /// citation of a statement the owner minted elsewhere. The label is
    /// unchanged; the parentheses alone carry the difference between
    /// establishing a statement and referring to one.
    ///
    /// ´claim:occurrence:a-parenthesised-span-cites-within-its-own-owner´
    /// ´test:unit:reads-parenthesized-label-spans-as-same-owner-citations´
    #[test]
    fn reads_parenthesized_label_spans_as_same_owner_citations() {
        assert_eq!(
            read_span("inv:labels:unique-mint", true),
            Reading::Occurrence {
                form: Form::SameOwnerCitation,
                label: label("inv:labels:unique-mint"),
            }
        );
    }

    /// A citation reaches across an ownership boundary by naming the owner: a
    /// parenthesised span whose interior brackets a prefix and a label is an
    /// imported citation, and the prefix it carries says whose statement is
    /// being cited. Prefixes bearing digits work as plain ones do.
    ///
    /// ´claim:occurrence:a-parenthesised-bracketed-span-cites-another-owner´
    /// ´test:unit:reads-parenthesized-bracketed-spans-as-imported-citations´
    #[test]
    fn reads_parenthesized_bracketed_spans_as_imported_citations() {
        assert_eq!(
            read_span("[SPEC-def:parser:tokenizer]", true),
            Reading::Occurrence {
                form: Form::ImportedCitation {
                    prefix: Prefix::parse("SPEC").expect("well-formed"),
                },
                label: label("def:parser:tokenizer"),
            }
        );

        assert_eq!(
            read_span("[REC012-test:integration:decode-roundtrip]", true),
            Reading::Occurrence {
                form: Form::ImportedCitation {
                    prefix: Prefix::parse("REC012").expect("well-formed"),
                },
                label: label("test:integration:decode-roundtrip"),
            }
        );
    }

    /// An import-shaped span standing without its parentheses is a failure
    /// rather than ordinary text. Reading it as text would let a citation of
    /// another owner pass unresolved simply by dropping two characters, so the
    /// reader keeps it and reports it.
    ///
    /// ´claim:occurrence:an-import-without-parentheses-is-a-failure-not-text´
    /// ´test:unit:fails-import-shaped-spans-without-parentheses´
    #[test]
    fn fails_import_shaped_spans_without_parentheses() {
        assert_eq!(
            read_span("[SPEC-def:parser:tokenizer]", false),
            Reading::NonParenthesizedImport {
                prefix: Prefix::parse("SPEC").expect("well-formed"),
                label: label("def:parser:tokenizer"),
            }
        );
    }

    /// A span matching no occurrence form is ordinary text, whether
    /// parenthesised or not. This is what lets prose quote commands, file
    /// names, command-line flags and Rust paths in the same delimiters the
    /// calculus uses without any of them becoming occurrences.
    ///
    /// ´claim:occurrence:a-span-matching-no-form-is-ordinary-text´
    /// ´test:unit:reads-unshaped-spans-as-text´
    #[test]
    fn reads_unshaped_spans_as_text() {
        let text_spans = [
            "sec",
            "cargo test",
            "Cargo.toml",
            "--all-features",
            "a::b",
            "one:two",
            "[SPEC]",
            "[spec-sec:labels:syntax]",
            "[SPEC-not a label]",
            "[SPEC]-sec:labels:syntax",
            "{SPEC-sec:labels:syntax}",
        ];

        for span in text_spans {
            for parenthesized in [false, true] {
                assert_eq!(
                    read_span(span, parenthesized),
                    Reading::Text,
                    "expected `{span}` (parenthesized: {parenthesized}) to be text"
                );
            }
        }
    }

    /// A span that a single named repair would turn into a label is reported as
    /// a near miss naming that repair — wrong casing, interior spacing,
    /// surrounding whitespace, both at once, or stray brackets. The span stays
    /// text; only the author's attention is asked for.
    ///
    /// ´claim:occurrence:a-span-one-repair-from-a-label-is-a-named-near-miss´
    /// ´test:unit:warns-on-near-miss-spans´
    #[test]
    fn warns_on_near_miss_spans() {
        let near_misses = [
            ("Sec:labels:syntax", "wrong casing"),
            ("SEC:LABELS:SYNTAX", "wrong casing"),
            ("sec: labels:syntax", "interior spacing"),
            ("sec:labels:syntax\n", "surrounding whitespace"),
            ("Sec: Labels: Syntax", "wrong casing and spacing"),
            ("(sec:labels:syntax)", "stray brackets"),
            ("[sec:labels:syntax]", "stray brackets"),
        ];

        for (span, reason) in near_misses {
            assert_eq!(
                read_span(span, false),
                Reading::NearMiss { reason },
                "expected `{span}` to be a near miss"
            );
        }
    }

    /// The occurrence forms are atomic: they do not nest, do not admit doubled
    /// or unbalanced brackets, and do not accept a prefix-label pair with its
    /// brackets missing. No arrangement of these becomes an occurrence, so
    /// there is no compound form to reason about.
    ///
    /// ´claim:occurrence:occurrence-forms-neither-nest-nor-admit-other-bracketing´
    /// ´test:unit:forms-do-not-nest-or-admit-other-bracketing´
    #[test]
    fn forms_do_not_nest_or_admit_other_bracketing() {
        for span in [
            "[[SPEC-sec:labels:syntax]]",
            "[SPEC-[GUIDE-sec:labels:syntax]]",
            "[SPEC-sec:labels:syntax",
            "SPEC-sec:labels:syntax]",
            "SPEC-sec:labels:syntax",
        ] {
            assert!(
                matches!(
                    read_span(span, true),
                    Reading::Text | Reading::NearMiss { .. }
                ),
                "expected `{span}` not to be an occurrence"
            );
        }
    }
}
