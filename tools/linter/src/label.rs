// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The label language of ADR-T-014: the typed triple and the prefix token.
//!
//! This module implements the character grammar of the environment labelled
//! (ADR-T-014, A calculus of documentation and source labels) in that document, quoted here in
//! ordinary prose so that this comment mints and cites nothing:
//!
//! ```text
//! label       ::=  kind ":" area ":" name
//! kind, area  ::=  word                 name  ::=  word ("-" word)*
//! word        ::=  [a-z0-9]+            PREFIX ::=  [A-Z][A-Z0-9]*
//! ```
//!
//! A value of [`Label`] exists only for text that matches this grammar exactly,
//! so downstream stages never have to re-validate label shape.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`accepts_well_formed_labels`] | label | The grammar admits every triple built to its rule: any lowercase alphanumeric kind and area, and a name of one or more such words joined by single hyphens. Digits are letters as far as the grammar cares, and single-character components are words like any other. |
//! | [`rejects_ill_formed_labels`] | label | Everything outside the grammar is rejected rather than repaired: the wrong number of components, any empty one, capitals anywhere, spaces leading, trailing or interior, hyphens doubled or at an edge or in the kind or area, characters beyond ASCII lowercase and digits, and text already wrapped in brackets or parentheses. A value of the label type therefore never needs re-validating downstream. |
//! | [`exposes_the_three_components`] | label | A parsed label gives its three components back separately, so a consumer asking which kind or area a label belongs to reads it off the value rather than splitting the text again. |
//! | [`renders_back_to_its_source_text`] | label | A label renders as exactly the text it was parsed from, so a label read out of a source and written back into a report or a projection is the same string the author wrote. |
//! | [`orders_labels_bytewise`] | label | Labels order lexicographically by kind, then area, then name, which is also the order of their rendered text because the separator sorts below every character a component admits. A sorted registry and a sorted list of label strings therefore agree. |
//! | [`accepts_well_formed_prefixes`] | label | An owner prefix is an uppercase letter followed by any run of uppercase letters and digits, so a single letter and a lettered-and-numbered designation are both admissible names for an owner. |
//! | [`rejects_ill_formed_prefixes`] | label | A prefix is rejected when it is empty, lowercase anywhere, begins with a digit, or carries punctuation, spaces or characters beyond ASCII, so the prefix space cannot be widened by writing a name carelessly. |

use std::fmt;

use serde::{Serialize, Serializer};

/// A label: the colon-joined triple of kind, area, and name.
///
/// Construction goes through [`Label::parse`], so every value is well-formed.
/// Ordering is the bytewise-lexicographic ordering of the triple, which is also
/// the ordering of the rendered text because the separator sorts below every
/// character the components admit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    kind: String,
    area: String,
    name: String,
}

impl Label {
    /// Parse label text, returning `None` when it is not well-formed.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut components = text.split(':');
        let kind = components.next()?;
        let area = components.next()?;
        let name = components.next()?;

        if components.next().is_some() {
            return None;
        }

        if !is_word(kind) || !is_word(area) || !is_name(name) {
            return None;
        }

        Some(Self {
            kind: kind.to_owned(),
            area: area.to_owned(),
            name: name.to_owned(),
        })
    }

    /// The kind component, which decides which warrant species may stand.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The area component, recording the concept's home at creation.
    #[must_use]
    pub fn area(&self) -> &str {
        &self.area
    }

    /// The name component.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.kind, self.area, self.name)
    }
}

impl Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// An owner prefix, as registered by the adoption signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix(String);

impl Prefix {
    /// Parse prefix text, returning `None` when it is not well-formed.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut characters = text.chars();
        let first = characters.next()?;

        if !first.is_ascii_uppercase() {
            return None;
        }

        if !characters.all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        {
            return None;
        }

        Some(Self(text.to_owned()))
    }

    /// The prefix text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Prefix {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for Prefix {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// A word over lowercase letters and digits.
fn is_word(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// A name: one or more words joined by single hyphens.
fn is_name(text: &str) -> bool {
    text.split('-').all(is_word)
}

#[cfg(test)]
mod tests {
    use super::{Label, Prefix};

    /// The grammar admits every triple built to its rule: any lowercase
    /// alphanumeric kind and area, and a name of one or more such words joined
    /// by single hyphens. Digits are letters as far as the grammar cares, and
    /// single-character components are words like any other.
    ///
    /// ´claim:label:the-grammar-admits-every-well-formed-triple´
    /// ´test:unit:accepts-well-formed-labels´
    #[test]
    fn accepts_well_formed_labels() {
        let accepted = [
            "sec:labels:syntax",
            "lang:labels:label-language",
            "inv:labels:total-resolution",
            "metathm:labels:no-self-support",
            "test:integration:decode-roundtrip",
            "a:b:c",
            "k1:a2:n3",
            "def:parser:tokenizer",
            "x:y:a-b-c-d",
            "9:9:9-9",
        ];

        for text in accepted {
            assert!(Label::parse(text).is_some(), "expected `{text}` to parse");
        }
    }

    /// Everything outside the grammar is rejected rather than repaired: the
    /// wrong number of components, any empty one, capitals anywhere, spaces
    /// leading, trailing or interior, hyphens doubled or at an edge or in the
    /// kind or area, characters beyond ASCII lowercase and digits, and text
    /// already wrapped in brackets or parentheses. A value of the label type
    /// therefore never needs re-validating downstream.
    ///
    /// ´claim:label:the-grammar-rejects-everything-outside-it´
    /// ´test:unit:rejects-ill-formed-labels´
    #[test]
    fn rejects_ill_formed_labels() {
        let rejected = [
            // Wrong arity.
            "",
            "sec",
            "sec:labels",
            "sec:labels:syntax:extra",
            // Empty components.
            ":labels:syntax",
            "sec::syntax",
            "sec:labels:",
            // Wrong casing.
            "Sec:labels:syntax",
            "sec:Labels:syntax",
            "sec:labels:Syntax",
            "SEC:LABELS:SYNTAX",
            // Spacing.
            "sec: labels:syntax",
            "sec :labels:syntax",
            "sec:labels:two words",
            " sec:labels:syntax",
            "sec:labels:syntax ",
            // Hyphenation defects.
            "sec:labels:-syntax",
            "sec:labels:syntax-",
            "sec:labels:two--words",
            "sec-x:labels:syntax",
            "sec:lab-els:syntax",
            // Foreign characters.
            "sec:labels:syn_tax",
            "sec:labels:syn.tax",
            "sec:labels:syntaxé",
            "sec:labels:syn/tax",
            // Bracketed and parenthesised text is not label text.
            "[SPEC-sec:labels:syntax]",
            "(sec:labels:syntax)",
        ];

        for text in rejected {
            assert!(
                Label::parse(text).is_none(),
                "expected `{text}` to be rejected"
            );
        }
    }

    /// A parsed label gives its three components back separately, so a consumer
    /// asking which kind or area a label belongs to reads it off the value
    /// rather than splitting the text again.
    ///
    /// ´claim:label:a-parsed-label-exposes-its-three-components´
    /// ´test:unit:exposes-the-three-components´
    #[test]
    fn exposes_the_three_components() {
        let label = Label::parse("lang:labels:label-language").expect("well-formed");

        assert_eq!(label.kind(), "lang");
        assert_eq!(label.area(), "labels");
        assert_eq!(label.name(), "label-language");
    }

    /// A label renders as exactly the text it was parsed from, so a label read
    /// out of a source and written back into a report or a projection is the
    /// same string the author wrote.
    ///
    /// ´claim:label:a-label-renders-back-as-the-text-it-parsed-from´
    /// ´test:unit:renders-back-to-its-source-text´
    #[test]
    fn renders_back_to_its_source_text() {
        let text = "inf:labels:same-owner-citation";
        let label = Label::parse(text).expect("well-formed");

        assert_eq!(label.to_string(), text);
    }

    /// Labels order lexicographically by kind, then area, then name, which is
    /// also the order of their rendered text because the separator sorts below
    /// every character a component admits. A sorted registry and a sorted list
    /// of label strings therefore agree.
    ///
    /// ´claim:label:labels-order-bytewise-by-their-triple´
    /// ´test:unit:orders-labels-bytewise´
    #[test]
    fn orders_labels_bytewise() {
        let first = Label::parse("a:b:c").expect("well-formed");
        let second = Label::parse("a:b:d").expect("well-formed");
        let third = Label::parse("b:a:a").expect("well-formed");

        assert!(first < second);
        assert!(second < third);
    }

    /// An owner prefix is an uppercase letter followed by any run of uppercase
    /// letters and digits, so a single letter and a lettered-and-numbered
    /// designation are both admissible names for an owner.
    ///
    /// ´claim:label:the-grammar-admits-every-well-formed-prefix´
    /// ´test:unit:accepts-well-formed-prefixes´
    #[test]
    fn accepts_well_formed_prefixes() {
        for text in ["S", "SPEC", "GUIDE", "REC012", "X9"] {
            assert!(Prefix::parse(text).is_some(), "expected `{text}` to parse");
        }
    }

    /// A prefix is rejected when it is empty, lowercase anywhere, begins with a
    /// digit, or carries punctuation, spaces or characters beyond ASCII, so the
    /// prefix space cannot be widened by writing a name carelessly.
    ///
    /// ´claim:label:the-grammar-rejects-every-ill-formed-prefix´
    /// ´test:unit:rejects-ill-formed-prefixes´
    #[test]
    fn rejects_ill_formed_prefixes() {
        for text in [
            "", "spec", "Spec", "9SPEC", "SPEC-", "SP EC", "SPEC_1", "SPÉC",
        ] {
            assert!(
                Prefix::parse(text).is_none(),
                "expected `{text}` to be rejected"
            );
        }
    }
}
