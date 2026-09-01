// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The constant profile of ADR-T-018: the census, the nine pinning programs,
//! and the two standard places.
//!
//! The profiles signature of ADR-T-014, A calculus of documentation and source labels, says
//! what a profile must fix: its kind token, its census, its classification
//! rule, its name transformation, and its standard place. This profile fixes
//! them differently from the two before it, and the difference is the record's
//! whole point. The classification and the name are the author's — the identity
//! mint names the concept a value fixes, and the screaming identifier is
//! deliberately no part of it — while the profile fixes the kind, two standard
//! places rather than one, and the derivation of the second from the first,
//! from the cited program, and from the declaration's value.
//!
//! # Binding
//!
//! The binding is positional: an identity stands in the documentation comment
//! attached to a constant item, and the value it is pinned against is read from
//! the declaration beneath that comment. No label's text is ever computed from
//! the screaming identifier, so a rename moves nothing and dangles nothing.
//!
//! A macro invocation binds the same way and is adopted the same way, its value
//! being the argument tokens standing between its delimiters rather than an
//! expression after an equals sign (ADR-T-018, The constant label profile). Two
//! asymmetries with the constants beside it are deliberate. An invocation
//! carrying no mint of this kind is not censused at all, so the population here
//! depends on adoption where the constants' does not; and a `macro_rules!`
//! definition, which the parser hands back in the same item shape, is passed
//! over, a definition being no invocation.
//!
//! Identifiers are read for one purpose and it is not naming. A trait's
//! declaration and the implementations that supply its values are bound to each
//! other by the language, through the trait's name and the member's, and the
//! program that pins such a declaration follows that binding rather than
//! inventing one. What reaches a label from those reads is the implementing
//! type's own word, which the program is defined to carry.
//!
//! # The programs
//!
//! Each pinning program is a documented algorithm minting in ADR-T-018 and
//! implemented here beside its mint. A program decides two things at once: what
//! it accepts, so that a float cited as a count is reported rather than pinned,
//! and how what it accepts derives the slug the pin carries. Every derivation but
//! one is a function of the value's normalised source text alone, so a reader can
//! recompute a pin from what stands on the line; the exception is the program
//! over a trait's implementations, which reads the package's implementations of
//! that member because that is what it pins.
//!
//! The two digest programs hash their input rather than spelling it, which is
//! what lets a table or a format string be pinned at all. No derivation emits the
//! source text of a type: the value alone is read, save for the implementing
//! type's lowercased final path segment, which is the hazard ADR-T-018 records at
//! its caveat.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`derives_a_count_from_its_digits`] | constant | The count program reads a decimal integer and nothing else: the digits stand as they are written, a separator is a reading aid the value does not carry, and a sign, a point, or a suffix is refused rather than repaired. |
//! | [`derives_a_version_as_the_count_program_does`] | constant | The version program accepts and derives exactly what the count program does, and is a separate citation because it records an intent rather than a shape: a version is expected to be bumped, so its pin is expected to re-mint and the citations that dangle are the migration checklist. |
//! | [`derives_a_scalar_by_rewriting_the_inadmissible_characters`] | constant | The scalar program rewrites the three characters the grammar of names does not admit and nothing else: the leading minus becomes a word, the point becomes a letter, and an exponent's minus becomes a letter beside the exponent's own. An integer is refused, because it is the count program's and a value two programs accepted would have two pins. |
//! | [`derives_seconds_through_the_duration_constructor`] | constant | The seconds program reads the seconds a value counts, whether the source writes the number bare or wraps it in the duration constructor, so that renaming the type moves no pin. A constructor of anything else is refused. |
//! | [`derives_a_word_from_a_string_that_is_one`] | constant | The word program accepts a string that is already a word of the label grammar and carries it verbatim. A string with a capital, a space, or a punctuation mark in it is refused rather than slugified, because a lossy name over a value is the non-injective encoding the record refuses. |
//! | [`derives_a_codepoint_from_its_scalar_value`] | constant | The codepoint program reads the character rather than its spelling, so an escape and the character itself derive one slug — which is the property a pin over characters needs, since a source may spell one either way. |
//! | [`digests_a_text_by_the_value_it_carries`] | constant | The text program digests the value a string carries rather than the spelling that carries it, so a raw literal and a plain one of the same content derive one pin and a symbol-heavy value is pinned at all. |
//! | [`digests_a_form_over_its_normalised_source`] | constant | The form program digests the value's normalised source, so a value the formatter wrapped across lines derives the pin it would derive on one, and a table with no plausible spelling is pinned by its bytes. |
//! | [`recomputes_the_digest_word_from_its_constants`] | constant | The digest word is arithmetic and nothing else, so it can be recomputed by hand and by another implementation: the empty sequence gives the offset basis's own low half, and every other input follows from the two constants the record states. |
//! | [`encodes_an_uncited_value_by_the_catalogue_in_order`] | constant | A value given no citation of its own is encoded by the catalogue read in order, so the program over implementations always has an answer: an integer counts, a float is a scalar, a word stands as itself, and anything else falls through to a digest. |
//! | [`derives_a_pin_that_carries_its_program`] | constant | The pin extends the identity it pins and carries the program between them, so a reader picks the program out of the label and recomputes the slug — and two programs deriving one slug still mint two different pins. |
//! | [`accepts_a_constant_carrying_all_four_occurrences`] | constant | A constant carrying its warrant, its identity, its program citation and its derived pin is clean, and is counted as covered and pinned. This is what a conforming constant looks like. |
//! | [`renaming_the_identifier_moves_nothing`] | constant | The binding is positional and never through the identifier: the same documentation over a differently named declaration derives the same labels, so a rename moves nothing and dangles nothing. |
//! | [`re_derives_the_pin_when_the_value_changes`] | constant | Changing the value re-derives the pin, which is the tripwire the record exists to build: the label a document cited is not the label the declaration now mints, so every citation of it dangles in that commit. |
//! | [`reports_a_stale_pin_with_both_labels`] | constant | A pin that is not the derivation is reported showing both what stands there and what the declaration gives, so a value moved under a pin nobody re-swept is caught rather than believed. |
//! | [`reports_a_missing_pin_with_its_derivation`] | constant | A constant whose pin is not written at all is reported with the pin the derivation gives, so the author is told what the sweep would write. |
//! | [`reports_a_pin_no_identity_heads`] | constant | A pin standing where no identity heads it warrants nothing and is reported: the generated line is warranted by the identity above it, and without one it is a hand writing a line no census gives. |
//! | [`reports_a_pin_beneath_an_identity_that_derives_none`] | constant | cites (´claim:constant:a-pin-no-identity-heads-warrants-nothing´) |
//! | [`reports_a_citation_of_no_catalogued_program`] | constant | A citation naming no catalogued program is reported at the constant, where the repair is: the class of a constant is a citation rather than a word, so adding one means minting a program's environment and implementing it. |
//! | [`reports_a_value_its_program_refuses`] | constant | A value its cited program refuses is reported naming both, and the check is free: the citation says what the value is supposed to be, so a float under a count citation is one of the two wrong. |
//! | [`reports_an_identity_place_of_another_shape`] | constant | The identity's place holds the identity mint and its program citation alone, so a line minting the kind in any other shape is reported rather than passed over: a shape the checker did not recognise is the case an author most needs told about. |
//! | [`reports_an_identity_citing_no_warrant`] | constant | An identity over documentation that neither cites a warrant nor admits owing one is the state the record refuses, and it is reported at the identity's own line. |
//! | [`accepts_a_to_do_standard_place_as_the_interim_warrant`] | constant | A to-do standard place standing in the documentation is the bootstrap's accepted warrant: it says in the corpus's own vocabulary that the record is owed rather than absent, and it dangles when the notice is discharged. |
//! | [`censuses_production_declarations_and_no_others`] | constant | The census reads the declarations that ship and no others: a binding inside a function body is a local, a declaration under the test configuration is fixed for the test, and an identifier not written in the screaming case is no constant of this policy. An associated constant of an implementation is read like any other. |
//! | [`censuses_an_adopted_invocation_and_no_other`] | constant | The census admits a macro invocation its own documentation adopts and no other: an invocation carrying no mint of this profile's kind is no declaration of the census, so a source full of them acquires no coverage burden, and a `macro_rules!` definition is passed over however it is documented, a definition being no invocation. |
//! | [`pins_an_invocation_over_the_tokens_between_its_delimiters`] | constant | An adopted invocation is pinned over the bytes standing between its opening and closing delimiters, exclusive of both, normalised as every form value is. So the macro's own name is no part of the pin and the formatter moves nothing, while reordering an argument re-mints exactly one label. |
//! | [`refuses_an_invocation_citing_any_program_but_the_form`] | constant | An adopted invocation cites the form program and no other, its argument tokens being read by nothing else. A citation of another program is reported as a value that program refuses, rather than derived from tokens the program was never defined over. |
//! | [`writes_an_adopted_invocations_pin_and_settles`] | constant | The sweep writes an adopted invocation's pin beneath the identity that warrants it and settles, exactly as it does at a constant: a second pass over what it wrote finds nothing to do, so a check run after a sweep means something here too. |
//! | [`pins_a_trait_declaration_once_per_implementation`] | constant | A trait's declaration is one concept with several values, so it carries one identity and one pin per implementation of its package, each slug naming the implementing type and the value that implementation supplies. |
//! | [`writes_the_pins_the_sweep_owes_and_settles`] | constant | The sweep writes the pin lines a constant owes, beneath the identity that warrants them, and settles: a second pass over what it wrote finds nothing to do, so a check run after a sweep means something. |
//! | [`reads_and_sweeps_a_block_comments_standard_places`] | constant | A constant documented in a block comment carries its standard places like any other: the comment's own continuation asterisks are decoration, so the identity is read through them and the pin is written beside them, in the leaders the identity's own line already uses. |
//! | [`rewrites_a_stale_pin_and_authors_nothing_else`] | constant | The sweep rewrites a pin that is not the derivation, and writes nothing at all where no identity stands: a warrant, an identity, and a program citation are the judgments the policy asks a person for. |
//!
//! (Regenerated by the projection sweep.)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ImplItem, Item, Lit, Meta, TraitItem, Type};

#[cfg(test)]
use crate::census::collect_rust_sources;
use crate::census::{DocComment, byte_offset, line_starts, read_doc};
use crate::comment::LEADERS;
use crate::finding::{Finding, Location, UnwarrantedPinReason};
use crate::label::Label;
use crate::occurrence::{Form, Reading, read_span};
#[cfg(test)]
use crate::plan::CorpusPlan;
use crate::plan::ProfileSource;
use crate::profile::{ACUTE, code_spans};
use crate::subscribe::Subscription;
#[cfg(test)]
use crate::workspace::Package;

/// The kind token this profile governs.
///
/// The token is the record's own local extension row, which catalogues the
/// Constant environment against this spelling and no other; the registry of
/// ADR-T-011 offers none, a constant being neither what a deployment chooses nor
/// what a build chooses (´[EMBER-sig:constants:kind-extension]´).
///
/// ´const:emberlinter:constant-kind-token´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:constant-kind-token-word-const´
pub const CONST_KIND: &str = "const";

/// The kind a pinning program's own mint carries.
///
/// A program citation resolves to the mint of a pinning program, and the
/// programs are the environments the catalogue names and no others — each of
/// them minted there as an Algorithm, which is what fixes this half of the pair
/// a citation is checked against (´[EMBER-sec:constants:programs]´).
///
/// ´const:emberlinter:pinning-program-mint-kind´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:pinning-program-mint-kind-word-alg´
const PROGRAM_KIND: &str = "alg";

/// The area a pinning program's own mint carries.
///
/// The other half of the same pair: the catalogued programs are minted in one
/// area, so a citation naming that kind in any other area reaches no program of
/// this profile and is refused rather than resolved
/// (´[EMBER-sec:constants:programs]´).
///
/// ´const:emberlinter:pinning-program-mint-area´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:pinning-program-mint-area-word-const´
const PROGRAM_AREA: &str = "const";

/// The offset basis of the 64-bit FNV-1a hash the digest word is taken from.
///
/// The record states the basis outright rather than leaving the choice here,
/// and the reason is the property the digest word is defined to have: a reader
/// recomputes a pin by hand and a future implementation in another language
/// agrees with this one, neither of which follows from a basis this file merely
/// picked (´[EMBER-def:const:digest-word]´).
///
/// ´const:emberlinter:digest-offset-basis´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:digest-offset-basis-form-x4a4117f8´
const DIGEST_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// The prime of the 64-bit FNV-1a hash the digest word is taken from.
///
/// Stated by the record beside the basis and for the same reason: the digest is
/// specified as arithmetic a person can carry out, so both operands of the
/// round belong to the record rather than to whichever implementation ran first
/// (´[EMBER-def:const:digest-word]´).
///
/// ´const:emberlinter:digest-multiplier-prime´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:digest-multiplier-prime-form-x1994b04b´
const DIGEST_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The half of the hash the digest word is written from.
///
/// The digest word is the low thirty-two bits of the 64-bit value the rounds
/// leave, written as eight hexadecimal digits; this mask is that width stated as
/// a value, and widening it would write a word the grammar of pins does not
/// admit (´[EMBER-def:const:digest-word]´).
///
/// ´const:emberlinter:digest-retained-bits-mask´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:digest-retained-bits-mask-form-x043bdbfc´
const LOW_HALF: u64 = 0x0000_0000_ffff_ffff;

/// The to-do markers a notice may be written with, per ADR-T-016.
///
/// This module reads them to recognise the interim warrant: a to-do standard
/// place standing in a constant's documentation is the bootstrap's accepted
/// warrant, so the shapes recognised here must be exactly the shapes that
/// profile censuses. It fixes three conventional spellings — the corpus writes
/// the first and the other two are censused so no synonym escapes the policy by
/// being spelled differently (´[EMBER-conv:todos:census]´).
///
/// ´const:emberlinter:interim-warrant-marker-spellings´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:interim-warrant-marker-spellings-form-x2efce54a´
const NOTICE_MARKERS: [&str; 3] = ["TODO", "FIXME", "XXX"];

/// The path a macro definition wears where the census expects an invocation.
///
/// The parser hands back one item shape for both, so the word the language
/// spells a definition with is what separates them here. The separation is the
/// convention's own: a definition is not an invocation, and what the census
/// reads is the invocation that reaches a body rather than the body it reaches
/// (´[EMBER-conv:constants:invocation]´).
///
/// ´const:emberlinter:macro-definition-path´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:macro-definition-path-text-x7fcea50b´
const MACRO_RULES: &str = "macro_rules";

/// What every trait member's implementations supply, gathered per owner.
///
/// The key is the owner, the trait, and the member; the value is one pair per
/// implementation, of the implementing type's word and the value it supplies.
type Implementations<'a> = BTreeMap<(&'a str, &'a str, &'a str), Vec<(&'a str, &'a str)>>;

/// One pinning program: what it accepts, and what it derives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Program {
    /// A non-negative decimal integer.
    Count,
    /// A version number, which is a count whose churn is intended.
    Version,
    /// A decimal floating value.
    Scalar,
    /// A count of seconds, written bare or as a duration constructor.
    Seconds,
    /// A string that is already a word of the label grammar.
    Word,
    /// A character.
    Codepoint,
    /// A string of any content, pinned by digest.
    Text,
    /// Any other value expression, pinned by digest over its source.
    Form,
    /// A trait's declaration, pinned once per implementation of it.
    Impls,
}

impl Program {
    /// Every catalogued program, in the order ADR-T-018 tables them.
    #[must_use]
    pub const fn all() -> [Self; 9] {
        [
            Self::Count,
            Self::Version,
            Self::Scalar,
            Self::Seconds,
            Self::Word,
            Self::Codepoint,
            Self::Text,
            Self::Form,
            Self::Impls,
        ]
    }

    /// The program's name, as it stands in the label that mints it and in the
    /// pins it derives.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Version => "version",
            Self::Scalar => "scalar",
            Self::Seconds => "seconds",
            Self::Word => "word",
            Self::Codepoint => "codepoint",
            Self::Text => "text",
            Self::Form => "form",
            Self::Impls => "impls",
        }
    }

    /// The program a label names, when the label names a catalogued one.
    ///
    /// A citation reaches a program only through its own mint, so the kind and
    /// area are read as well as the name: a label of another kind naming one of
    /// these words is not a program, and reading it as one would let a class be
    /// summoned by a coincidence of spelling.
    #[must_use]
    pub fn of_label(label: &Label) -> Option<Self> {
        if label.kind() != PROGRAM_KIND || label.area() != PROGRAM_AREA {
            return None;
        }

        Self::all()
            .into_iter()
            .find(|program| program.as_str() == label.name())
    }

    /// Derive a slug from a value's normalised source text.
    ///
    /// Returns `None` when the program refuses what it was given, which is a
    /// defect of the constant rather than of the program: the citation says what
    /// the value is supposed to be, and a value that is not that is reported. The
    /// program over a trait's implementations refuses every value, because a
    /// value is not what it pins.
    #[must_use]
    pub fn derive(self, value: &str) -> Option<String> {
        match self {
            Self::Count | Self::Version => decimal_digits(value),
            Self::Scalar => scalar_slug(value),
            Self::Seconds => seconds_slug(value),
            Self::Word => word_slug(value),
            Self::Codepoint => codepoint_slug(value),
            Self::Text => string_value(value).map(|text| digest_word(text.as_bytes())),
            Self::Form => Some(digest_word(value.as_bytes())),
            Self::Impls => None,
        }
    }
}

/// The encoding a value takes when no program was cited for it in particular.
///
/// The program over a trait's implementations pins values it was not given a
/// class for, one per implementation, so it needs a rule that always answers.
/// The rule is the catalogue itself, read in order: the first program that
/// accepts the value encodes it, and the form program, which accepts everything,
/// is what remains. The order is the table's, save that the two programs that
/// exist to record intent rather than shape — the version program, which is the
/// count program under another name, and the seconds program, which reads an
/// integer as a duration — are passed over, because nothing here states an
/// intent for them to record.
#[must_use]
pub fn natural_slug(value: &str) -> String {
    for program in [
        Program::Count,
        Program::Scalar,
        Program::Word,
        Program::Codepoint,
        Program::Text,
    ] {
        if let Some(slug) = program.derive(value) {
            return slug;
        }
    }

    digest_word(value.as_bytes())
}

/// The digits of a decimal integer literal, with its separators removed.
///
/// A sign, a base prefix, a type suffix, and anything else are refused: the
/// declaration states the type, and a slug that erased a base would collide two
/// spellings of one value into one name.
fn decimal_digits(value: &str) -> Option<String> {
    let mut digits = String::new();

    for character in value.chars() {
        match character {
            '_' => {}
            digit if digit.is_ascii_digit() => digits.push(digit),
            _ => return None,
        }
    }

    (!digits.is_empty()).then_some(digits)
}

/// The slug of a decimal floating literal.
///
/// The three characters the grammar of names does not admit are rewritten and
/// nothing else is: the leading minus becomes a word, the point becomes a letter,
/// and an exponent's minus becomes a letter beside the exponent's own. The
/// rewriting is injective on the literals the program accepts.
fn scalar_slug(value: &str) -> Option<String> {
    let (negative, rest) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let (mantissa, exponent) = match rest.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(exponent)),
        None => (rest, None),
    };

    let (whole, fraction) = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole, Some(decimal_digits(fraction)?)),
        None => (mantissa, None),
    };

    if fraction.is_none() && exponent.is_none() {
        // An integer is the count program's, and a value two programs accepted
        // would have two pins a reader could not choose between.
        return None;
    }

    let mut slug = String::new();

    if negative {
        slug.push_str("neg");
    }

    slug.push_str(&decimal_digits(whole)?);

    if let Some(fraction) = fraction {
        slug.push('p');
        slug.push_str(&fraction);
    }

    if let Some(exponent) = exponent {
        slug.push('e');

        let negative_exponent = exponent.strip_prefix('-');

        if negative_exponent.is_some() {
            slug.push('n');
        }

        let digits =
            negative_exponent.unwrap_or_else(|| exponent.strip_prefix('+').unwrap_or(exponent));

        slug.push_str(&decimal_digits(digits)?);
    }

    Some(slug)
}

/// The seconds a value counts, written bare or as a duration constructor.
fn seconds_slug(value: &str) -> Option<String> {
    if let Some(digits) = decimal_digits(value) {
        return Some(digits);
    }

    let Ok(Expr::Call(call)) = syn::parse_str::<Expr>(value) else {
        return None;
    };

    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };

    if path
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != "from_secs")
    {
        return None;
    }

    let arguments: Vec<&Expr> = call.args.iter().collect();
    let [argument] = arguments.as_slice() else {
        return None;
    };

    match literal_of(argument) {
        Some(Lit::Int(integer)) => decimal_digits(integer.base10_digits()),
        _ => None,
    }
}

/// The content of a string literal that is already a word of the grammar.
fn word_slug(value: &str) -> Option<String> {
    let text = string_value(value)?;
    let is_word = !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit());

    is_word.then_some(text)
}

/// The slug of a character literal: the letter and the scalar value in hex.
fn codepoint_slug(value: &str) -> Option<String> {
    match literal_of_text(value)? {
        Lit::Char(literal) => Some(format!("u{:x}", literal.value() as u32)),
        _ => None,
    }
}

/// The value a string literal carries, escapes resolved and raw content as written.
fn string_value(value: &str) -> Option<String> {
    match literal_of_text(value)? {
        Lit::Str(literal) => Some(literal.value()),
        _ => None,
    }
}

/// The literal a value's source text is, when it is one.
fn literal_of_text(value: &str) -> Option<Lit> {
    syn::parse_str::<Expr>(value)
        .ok()
        .as_ref()
        .and_then(literal_of)
}

/// The literal an expression is, when it is one.
fn literal_of(expression: &Expr) -> Option<Lit> {
    match expression {
        Expr::Lit(literal) => Some(literal.lit.clone()),
        _ => None,
    }
}

/// The digest word of a byte sequence, as ADR-T-018 defines it.
///
/// The 64-bit FNV-1a hash, of which the low half is written in eight lowercase
/// hexadecimal digits behind the letter that keeps the word from beginning like a
/// number nobody meant. Nothing but arithmetic is used, so the word can be
/// recomputed by hand and a future implementation agrees with this one.
#[must_use]
pub fn digest_word(bytes: &[u8]) -> String {
    let mut hash = DIGEST_BASIS;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(DIGEST_PRIME);
    }

    format!("x{:08x}", hash & LOW_HALF)
}

/// How a declaration is bound to the others the language relates it to.
///
/// The identifiers here name nothing: they are read to follow the binding the
/// compiler itself makes between a trait's declaration and the implementations
/// that supply its values, which is the one thing positional binding cannot see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// A declaration standing on its own, in a module or an inherent block.
    Free,
    /// A trait's own declaration of an associated constant.
    Declaration {
        /// The trait's name.
        trait_name: String,
        /// The member's name.
        member: String,
    },
    /// An adopted macro invocation, pinned over its argument tokens.
    Invocation,
    /// One implementation's supply of a trait's associated constant.
    Implementation {
        /// The trait's name.
        trait_name: String,
        /// The member's name.
        member: String,
        /// The implementing type's word: its path's final segment, lowercased.
        type_word: String,
    },
}

/// One declaration the census covers: a screaming constant of a production source.
#[derive(Debug, Clone)]
pub struct ConstantDeclaration {
    package: String,
    path: PathBuf,
    location: Location,
    doc: Option<DocComment>,
    doc_locations: Vec<Location>,
    value: Option<String>,
    binding: Binding,
}

impl ConstantDeclaration {
    /// The crate name of the package that owns this declaration.
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The source file, relative to the workspace root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where the declaration stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// The declaration's documentation comment, when it has one.
    #[must_use]
    pub const fn doc(&self) -> Option<&DocComment> {
        self.doc.as_ref()
    }

    /// The declaration's value, as normalised source text.
    ///
    /// A trait's valueless declaration carries none.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// How the language binds this declaration to the others.
    #[must_use]
    pub const fn binding(&self) -> &Binding {
        &self.binding
    }

    /// Where one line of the documentation stands, when the lines are known.
    fn line_location(&self, index: usize) -> Location {
        self.doc_locations
            .get(index)
            .unwrap_or(&self.location)
            .clone()
    }
}

/// The census over a workspace, together with what it took to read it.
#[derive(Debug, Clone, Default)]
pub struct ConstantCensus {
    declarations: Vec<ConstantDeclaration>,
    files_scanned: usize,
}

impl ConstantCensus {
    /// Build a census from declarations already scanned.
    #[doc(hidden)]
    #[must_use]
    pub const fn from_declarations(
        declarations: Vec<ConstantDeclaration>,
        files_scanned: usize,
    ) -> Self {
        Self {
            declarations,
            files_scanned,
        }
    }

    /// Every declaration the census covers.
    #[must_use]
    pub fn declarations(&self) -> &[ConstantDeclaration] {
        &self.declarations
    }

    /// How many Rust sources the census parsed.
    #[must_use]
    pub const fn files_scanned(&self) -> usize {
        self.files_scanned
    }

    /// What each trait member's implementations supply, gathered per owner.
    ///
    /// The implementations counted are those of the trait's own package. An
    /// implementation elsewhere would otherwise re-mint a label its own package
    /// does not own, which is a tripwire pointing at the wrong tree.
    #[must_use]
    fn implementations(&self) -> Implementations<'_> {
        let mut found: Implementations<'_> = BTreeMap::new();

        for declaration in &self.declarations {
            let Binding::Implementation {
                trait_name,
                member,
                type_word,
            } = &declaration.binding
            else {
                continue;
            };

            let Some(value) = declaration.value() else {
                continue;
            };

            found
                .entry((declaration.package(), trait_name.as_str(), member.as_str()))
                .or_default()
                .push((type_word.as_str(), value));
        }

        for supplied in found.values_mut() {
            supplied.sort_unstable();
        }

        found
    }
}

/// One covered constant: a declaration whose documentation carries an identity.
#[derive(Debug, Clone)]
pub struct CoveredConstant {
    path: PathBuf,
    identity: Label,
    program: Program,
    identity_line: String,
    pins: Vec<Label>,
    carried: bool,
    location: Location,
}

impl CoveredConstant {
    /// The source the constant stands in, which decides its owner.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The authored identity mint.
    #[must_use]
    pub const fn identity(&self) -> &Label {
        &self.identity
    }

    /// The pinning program the identity's citation names.
    #[must_use]
    pub const fn program(&self) -> Program {
        self.program
    }

    /// The identity's standard-place line, as its documentation writes it.
    #[must_use]
    pub fn identity_line(&self) -> &str {
        &self.identity_line
    }

    /// The pins the derivation gives: one, or one per implementation, or none.
    #[must_use]
    pub fn pins(&self) -> &[Label] {
        &self.pins
    }

    /// Whether every derived pin stands at its standard place already.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.carried
    }

    /// Where the constant stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }

    /// The pins as they stand at their standard place, in the code syntax.
    #[must_use]
    pub fn pin_lines(&self) -> Vec<String> {
        self.pins
            .iter()
            .map(|pin| format!("{ACUTE}{pin}{ACUTE}"))
            .collect()
    }
}

/// Derive the pin of an identity, its program, and a slug.
///
/// The pin extends the identity it pins: same kind, same area, and the identity's
/// own name followed by the program's name and the derived slug. Carrying the
/// program makes the pin self-describing — a reader picks the program out of the
/// label and recomputes the slug by hand — and it keeps a re-classed identity's
/// new pin from colliding with its old one where two programs derive one slug.
#[must_use]
pub fn derive_pin(identity: &Label, program: Program, slug: &str) -> Option<Label> {
    Label::parse(&format!(
        "{}:{}:{}-{}-{}",
        identity.kind(),
        identity.area(),
        identity.name(),
        program.as_str(),
        slug
    ))
}

/// What one line of a covered constant's documentation is, to this profile.
enum Line {
    /// The identity's standard place: the mint, then its program citation.
    Identity {
        /// The identity the line mints.
        identity: Label,
        /// The label its citation names.
        cited: Label,
    },
    /// A bare span of this profile's kind standing alone on its line.
    Bare {
        /// The label the bare span names.
        label: Label,
    },
    /// A line carrying a span of this profile's kind in no shape the profile owns.
    Malformed,
    /// A line carrying a citation, which a warrant may be.
    Citation,
    /// A to-do notice's standard place, the interim warrant of the bootstrap.
    Notice,
    /// Anything else.
    Prose,
}

/// One documentation line with its own decoration resolved away.
///
/// A documentation attribute hands back what its comment carried, and a block
/// comment's continuation asterisk is part of that. Resolving the leaders here
/// is what lets a constant documented in a block comment carry its standard
/// places like any other, and it is the same resolution the code carrier makes
/// for the same reason.
fn doc_line(line: &str) -> &str {
    line.trim_start_matches(LEADERS).trim_end()
}

/// Read one documentation line as this profile reads it.
fn read_line(line: &str) -> Line {
    let spans = code_spans(line);
    let readings: Vec<Reading> = spans
        .iter()
        .map(|span| read_span(span.interior, span.parenthesized))
        .collect();
    let mints: Vec<&Label> = readings
        .iter()
        .filter_map(|reading| match reading {
            Reading::Occurrence {
                form: Form::Mint,
                label,
            } => Some(label),
            _ => None,
        })
        .collect();

    if mints.iter().any(|label| label.kind() == CONST_KIND) {
        return read_constant_line(line, &readings, &spans, &mints);
    }

    if is_notice(line) {
        return Line::Notice;
    }

    if readings
        .iter()
        .any(|reading| matches!(reading, Reading::Occurrence { form, .. } if *form != Form::Mint))
    {
        return Line::Citation;
    }

    Line::Prose
}

/// Read a line that mints this profile's kind against the two standard places.
fn read_constant_line(
    line: &str,
    readings: &[Reading],
    spans: &[crate::profile::CodeSpan<'_>],
    mints: &[&Label],
) -> Line {
    let [identity] = mints else {
        return Line::Malformed;
    };

    if identity.kind() != CONST_KIND {
        return Line::Malformed;
    }

    if let [
        Reading::Occurrence {
            form: Form::Mint, ..
        },
        Reading::Occurrence { form, label },
    ] = readings
        && *form != Form::Mint
        && line == identity_place_text(identity, spans[1].interior)
    {
        return Line::Identity {
            identity: (*identity).clone(),
            cited: label.clone(),
        };
    }

    if line == format!("{ACUTE}{identity}{ACUTE}") {
        return Line::Bare {
            label: (*identity).clone(),
        };
    }

    Line::Malformed
}

/// The identity's standard place, written out for an identity and a citation.
fn identity_place_text(identity: &Label, cited: &str) -> String {
    format!("{ACUTE}{identity}{ACUTE} ({ACUTE}{cited}{ACUTE})")
}

/// Whether a documentation line heads a to-do notice, per ADR-T-016's census.
fn is_notice(line: &str) -> bool {
    NOTICE_MARKERS.iter().any(|marker| {
        line.strip_prefix(marker)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t', ':', '(']))
    })
}

/// What the constant pass found.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConstantAnalysis {
    /// How many Rust sources the census parsed.
    pub files_scanned: usize,
    /// How many declarations the census covers.
    ///
    /// The count runs over the constants and statics a production source
    /// declares and the macro invocations its documentation adopts. The second
    /// class depends on adoption where the first does not, so this figure is a
    /// fact about the source tree only up to the invocations somebody has
    /// adopted in it (ADR-T-018, The constant label profile).
    pub declarations: usize,
    /// How many of them carry an identity, and are therefore held to the record.
    pub covered: usize,
    /// How many covered constants carry every derived pin at its standard place.
    pub pinned: usize,
    /// How many pins the derivations give over the whole census.
    pub pins: usize,
    /// How many covered constants carry each program's citation.
    pub by_program: BTreeMap<String, usize>,
    /// How many derived pins are not standing where they should.
    pub missing_pins: usize,
    /// How many pins at a standard place are not the derivation.
    pub stale_pins: usize,
    /// How many pins stand where nothing warrants one.
    pub unwarranted_pins: usize,
    /// How many identity places are written in a shape the profile does not own.
    pub malformed_identities: usize,
    /// How many citations name no catalogued pinning program.
    pub uncatalogued_programs: usize,
    /// How many values their cited program refuses.
    pub refused_values: usize,
    /// How many covered constants cite no warrant.
    pub missing_warrants: usize,
}

/// Read every covered constant of a census, and report what fails.
#[must_use]
pub fn cover_constants(census: &ConstantCensus) -> (Vec<CoveredConstant>, Vec<Finding>) {
    let implementations = census.implementations();
    let mut covered = Vec::new();
    let mut findings = Vec::new();

    for declaration in census.declarations() {
        cover_one(declaration, &implementations, &mut covered, &mut findings);
    }

    (covered, findings)
}

/// Read one declaration's documentation for the occurrences the record fixes.
fn cover_one(
    declaration: &ConstantDeclaration,
    implementations: &Implementations<'_>,
    covered: &mut Vec<CoveredConstant>,
    findings: &mut Vec<Finding>,
) {
    let Some(doc) = declaration.doc() else {
        return;
    };

    let read: Vec<Line> = doc
        .lines()
        .iter()
        .map(|line| read_line(doc_line(line)))
        .collect();
    let identity_index = read
        .iter()
        .position(|line| matches!(line, Line::Identity { .. }));

    for (index, line) in read.iter().enumerate() {
        if matches!(line, Line::Malformed) {
            findings.push(Finding::MalformedConstantIdentity {
                location: declaration.line_location(index),
            });
        }
    }

    let Some(index) = identity_index else {
        report_orphan_pins(declaration, &read, findings);
        return;
    };

    let Line::Identity { identity, cited } = &read[index] else {
        return;
    };

    if !read
        .iter()
        .enumerate()
        .any(|(other, line)| other != index && matches!(line, Line::Citation | Line::Notice))
    {
        findings.push(Finding::MissingConstantWarrant {
            identity: identity.clone(),
            location: declaration.line_location(index),
        });
    }

    let Some(program) = Program::of_label(cited) else {
        findings.push(Finding::UncataloguedPinningProgram {
            cited: cited.clone(),
            location: declaration.line_location(index),
        });
        return;
    };

    let pins = expected_pins(
        declaration,
        identity,
        program,
        implementations,
        &declaration.line_location(index),
        findings,
    );
    let carried = carried_pins(&read, index);
    let standing = compare_pins(declaration, index, &pins, &carried, findings);

    covered.push(CoveredConstant {
        path: declaration.path().to_path_buf(),
        identity: identity.clone(),
        program,
        identity_line: doc_line(&doc.lines()[index]).to_owned(),
        pins,
        carried: standing,
        location: declaration.location().clone(),
    });
}

/// Report every bare span of this kind in a documentation carrying no identity.
fn report_orphan_pins(
    declaration: &ConstantDeclaration,
    read: &[Line],
    findings: &mut Vec<Finding>,
) {
    for (index, line) in read.iter().enumerate() {
        if let Line::Bare { label } = line {
            findings.push(Finding::UnwarrantedConstantPin {
                label: label.clone(),
                reason: UnwarrantedPinReason::NoIdentity,
                location: declaration.line_location(index),
            });
        }
    }
}

/// The run of bare spans standing directly beneath an identity's standard place.
fn carried_pins(read: &[Line], identity: usize) -> Vec<Label> {
    read.iter()
        .skip(identity + 1)
        .map_while(|line| match line {
            Line::Bare { label } => Some(label.clone()),
            _ => None,
        })
        .collect()
}

/// Hold the pins standing beneath an identity to the pins its value derives.
fn compare_pins(
    declaration: &ConstantDeclaration,
    identity: usize,
    expected: &[Label],
    carried: &[Label],
    findings: &mut Vec<Finding>,
) -> bool {
    let mut standing = true;

    for (offset, pin) in expected.iter().enumerate() {
        let line = identity + 1 + offset;

        match carried.get(offset) {
            Some(found) if found == pin => {}
            Some(found) => {
                standing = false;
                findings.push(Finding::WrongConstantPin {
                    expected: pin.clone(),
                    found: found.clone(),
                    location: declaration.line_location(line),
                });
            }
            None => {
                standing = false;
                findings.push(Finding::MissingConstantPin {
                    expected: pin.clone(),
                    location: declaration.line_location(identity),
                });
            }
        }
    }

    for (offset, found) in carried.iter().enumerate().skip(expected.len()) {
        standing = false;
        findings.push(Finding::UnwarrantedConstantPin {
            label: found.clone(),
            reason: UnwarrantedPinReason::NoValue,
            location: declaration.line_location(identity + 1 + offset),
        });
    }

    standing
}

/// The pins a declaration derives, reporting what its program refuses.
fn expected_pins(
    declaration: &ConstantDeclaration,
    identity: &Label,
    program: Program,
    implementations: &Implementations<'_>,
    location: &Location,
    findings: &mut Vec<Finding>,
) -> Vec<Label> {
    if matches!(declaration.binding(), Binding::Invocation) && program != Program::Form {
        findings.push(Finding::RefusedConstantValue {
            identity: identity.clone(),
            program: program.as_str().to_owned(),
            value: declaration.value().map(ToOwned::to_owned),
            location: location.clone(),
        });
        return Vec::new();
    }

    if program == Program::Impls {
        return implementation_pins(declaration, identity, implementations, location, findings);
    }

    let Some(value) = declaration.value() else {
        findings.push(Finding::RefusedConstantValue {
            identity: identity.clone(),
            program: program.as_str().to_owned(),
            value: None,
            location: location.clone(),
        });
        return Vec::new();
    };

    let pin = program
        .derive(value)
        .and_then(|slug| derive_pin(identity, program, &slug));

    let Some(pin) = pin else {
        findings.push(Finding::RefusedConstantValue {
            identity: identity.clone(),
            program: program.as_str().to_owned(),
            value: Some(value.to_owned()),
            location: location.clone(),
        });

        return Vec::new();
    };

    vec![pin]
}

/// One pin per implementation of a trait's associated constant.
fn implementation_pins(
    declaration: &ConstantDeclaration,
    identity: &Label,
    implementations: &Implementations<'_>,
    location: &Location,
    findings: &mut Vec<Finding>,
) -> Vec<Label> {
    let Binding::Declaration { trait_name, member } = declaration.binding() else {
        findings.push(Finding::RefusedConstantValue {
            identity: identity.clone(),
            program: Program::Impls.as_str().to_owned(),
            value: declaration.value().map(ToOwned::to_owned),
            location: location.clone(),
        });
        return Vec::new();
    };

    let key = (declaration.package(), trait_name.as_str(), member.as_str());

    implementations
        .get(&key)
        .map(|supplied| {
            supplied
                .iter()
                .filter_map(|(type_word, value)| {
                    derive_pin(
                        identity,
                        Program::Impls,
                        &format!("{type_word}-{}", natural_slug(value)),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Run the constant pass over a census.
///
/// The subscription routes the pin projection alone. Identity coverage is the
/// conformance profile's and stays a census over the whole workspace, so
/// `covered` and `by_program` count every owner's share; the pin figures and
/// the pin findings are the projection's, so a covered constant standing in an
/// unsubscribed owner's share derives no pin census and no pin verdict here.
#[must_use]
pub fn analyze_constants(
    census: &ConstantCensus,
    subscription: &Subscription<'_>,
) -> (ConstantAnalysis, Vec<CoveredConstant>, Vec<Finding>) {
    let (covered, mut findings) = cover_constants(census);
    let mut analysis = ConstantAnalysis {
        files_scanned: census.files_scanned(),
        declarations: census.declarations().len(),
        covered: covered.len(),
        ..ConstantAnalysis::default()
    };

    findings.retain(|finding| match finding {
        Finding::MissingConstantPin { location, .. }
        | Finding::WrongConstantPin { location, .. }
        | Finding::UnwarrantedConstantPin { location, .. } => subscription.governs(location.path()),
        _ => true,
    });

    for program in Program::all() {
        analysis.by_program.insert(program.as_str().to_owned(), 0);
    }

    for constant in &covered {
        *analysis
            .by_program
            .entry(constant.program.as_str().to_owned())
            .or_default() += 1;

        if !subscription.governs(constant.path()) {
            continue;
        }

        analysis.pins += constant.pins.len();

        if constant.carried {
            analysis.pinned += 1;
        }
    }

    for finding in &findings {
        match finding {
            Finding::MissingConstantPin { .. } => analysis.missing_pins += 1,
            Finding::WrongConstantPin { .. } => analysis.stale_pins += 1,
            Finding::UnwarrantedConstantPin { .. } => analysis.unwarranted_pins += 1,
            Finding::MalformedConstantIdentity { .. } => analysis.malformed_identities += 1,
            Finding::UncataloguedPinningProgram { .. } => analysis.uncatalogued_programs += 1,
            Finding::RefusedConstantValue { .. } => analysis.refused_values += 1,
            Finding::MissingConstantWarrant { .. } => analysis.missing_warrants += 1,
            _ => {}
        }
    }

    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    (analysis, covered, findings)
}

/// The directory of a package whose Rust sources the constant census reads.
///
/// One root, not two: what a test fixes is fixed for the test, and the record's
/// census admits production sources alone — a covered constant stands in a Rust
/// source under the package's own source root, which is the tree named here
/// (ADR-T-018, The constant label profile).
///
/// ´const:emberlinter:constant-census-production-root´ (ADR-T-018, The constant label profile)
/// ´const:emberlinter:constant-census-production-root-word-src´
#[cfg(test)]
const CENSUSED_DIRECTORY: &str = "src";

/// The directory of a package's crate tests, which the census passes over.
///
/// The same census carves this tree back out of the root above, because the
/// argument for warranting a value — that a reader meeting it will ask why it is
/// that and not another — is an argument about code that ships
/// (ADR-T-018, The constant label profile).
///
/// ´const:emberlinter:constant-census-excluded-tree´ (ADR-T-018, The constant label profile)
/// ´const:emberlinter:constant-census-excluded-tree-text-x9365b9f3´
#[cfg(test)]
const CRATE_TESTS: &str = "src/tests";

/// Take the constant census of every package of a workspace.
#[must_use]
#[cfg(test)]
pub fn take_constant_census(
    root: &Path,
    packages: &[Package],
    corpus: &CorpusPlan,
) -> (ConstantCensus, Vec<Finding>) {
    let mut census = ConstantCensus::default();
    let mut findings = Vec::new();

    for package in packages {
        let mut paths = Vec::new();

        collect_rust_sources(
            corpus,
            &package.directory().join(CENSUSED_DIRECTORY),
            &mut paths,
        );
        paths.sort();

        for path in paths {
            if path.starts_with(package.directory().join(CRATE_TESTS)) {
                continue;
            }

            let text = match fs::read_to_string(root.join(&path)) {
                Ok(text) => text,
                Err(error) => {
                    findings.push(Finding::TraversalFailure {
                        path: path.to_string_lossy().into_owned(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };

            census.files_scanned += 1;

            match scan_constants(package.name(), &path, &text) {
                Ok(declarations) => census.declarations.extend(declarations),
                Err(message) => findings.push(Finding::SourceParseFailure {
                    path: path.to_string_lossy().into_owned(),
                    message,
                }),
            }
        }
    }

    (census, findings)
}

/// Take the constant census from the execution plan's finite source projection.
#[must_use]
pub fn take_planned_constant_census(
    root: &Path,
    sources: &[ProfileSource],
) -> (ConstantCensus, Vec<Finding>) {
    let mut census = ConstantCensus::default();
    let mut findings = Vec::new();

    for source in sources {
        let path = source.path();
        let text = match fs::read_to_string(root.join(path)) {
            Ok(text) => text,
            Err(error) => {
                findings.push(Finding::TraversalFailure {
                    path: path.to_string_lossy().into_owned(),
                    message: error.to_string(),
                });
                continue;
            }
        };

        census.files_scanned += 1;

        match scan_constants(source.package(), path, &text) {
            Ok(declarations) => census.declarations.extend(declarations),
            Err(message) => findings.push(Finding::SourceParseFailure {
                path: path.to_string_lossy().into_owned(),
                message,
            }),
        }
    }

    (census, findings)
}

#[cfg(test)]
pub fn retiring_constant_source_rows(
    packages: &[Package],
    corpus: &CorpusPlan,
) -> Vec<(String, PathBuf)> {
    let mut rows = Vec::new();

    for package in packages {
        let mut paths = Vec::new();
        collect_rust_sources(
            corpus,
            &package.directory().join(CENSUSED_DIRECTORY),
            &mut paths,
        );
        paths.sort();
        rows.extend(
            paths
                .into_iter()
                .filter(|path| !path.starts_with(package.directory().join(CRATE_TESTS)))
                .map(|path| (package.name().to_owned(), path)),
        );
    }

    rows
}

/// Take the constant census of one Rust source.
///
/// # Errors
///
/// Returns the parser's complaint when the text is not a Rust source file.
pub fn scan_constants(
    package: &str,
    path: &Path,
    text: &str,
) -> Result<Vec<ConstantDeclaration>, String> {
    let file = syn::parse_file(text).map_err(|error| error.to_string())?;
    let starts = line_starts(text);
    let mut declarations = Vec::new();
    let source = Source {
        package,
        path,
        text,
        starts: &starts,
    };

    walk_items(&file.items, &source, &mut declarations);

    Ok(declarations)
}

/// The source one census entry is being read out of.
struct Source<'a> {
    package: &'a str,
    path: &'a Path,
    text: &'a str,
    starts: &'a [usize],
}

/// Walk a module body, descending into every inline module the census reads.
fn walk_items(items: &[Item], source: &Source<'_>, found: &mut Vec<ConstantDeclaration>) {
    for item in items {
        match item {
            Item::Const(declaration) if !gated_by_test(&declaration.attrs) => {
                push_declaration(
                    &declaration.attrs,
                    &declaration.ident,
                    Some(declaration.expr.as_ref()),
                    Binding::Free,
                    source,
                    found,
                );
            }
            Item::Static(declaration) if !gated_by_test(&declaration.attrs) => {
                push_declaration(
                    &declaration.attrs,
                    &declaration.ident,
                    Some(declaration.expr.as_ref()),
                    Binding::Free,
                    source,
                    found,
                );
            }
            Item::Impl(block) if !gated_by_test(&block.attrs) => {
                walk_implementation(block, source, found);
            }
            Item::Trait(declaration) if !gated_by_test(&declaration.attrs) => {
                for member in &declaration.items {
                    if let TraitItem::Const(associated) = member
                        && !gated_by_test(&associated.attrs)
                    {
                        push_declaration(
                            &associated.attrs,
                            &associated.ident,
                            associated.default.as_ref().map(|(_token, value)| value),
                            Binding::Declaration {
                                trait_name: declaration.ident.to_string(),
                                member: associated.ident.to_string(),
                            },
                            source,
                            found,
                        );
                    }
                }
            }
            Item::Macro(invocation) if !gated_by_test(&invocation.attrs) => {
                push_invocation(invocation, source, found);
            }
            Item::Mod(module) if !gated_by_test(&module.attrs) => {
                if let Some((_brace, inner)) = &module.content {
                    walk_items(inner, source, found);
                }
            }
            _ => {}
        }
    }
}

/// Read the associated constants one implementation block supplies.
fn walk_implementation(
    block: &syn::ItemImpl,
    source: &Source<'_>,
    found: &mut Vec<ConstantDeclaration>,
) {
    let implemented = block
        .trait_
        .as_ref()
        .and_then(|(path, _for)| path.segments.last())
        .map(|segment| segment.ident.to_string());

    for member in &block.items {
        let ImplItem::Const(declaration) = member else {
            continue;
        };

        if gated_by_test(&declaration.attrs) {
            continue;
        }

        let binding =
            implemented
                .as_ref()
                .map_or(Binding::Free, |trait_name| Binding::Implementation {
                    trait_name: trait_name.clone(),
                    member: declaration.ident.to_string(),
                    type_word: type_word(block.self_ty.as_ref()),
                });

        push_declaration(
            &declaration.attrs,
            &declaration.ident,
            Some(&declaration.expr),
            binding,
            source,
            found,
        );
    }
}

/// The word an implementing type carries into a pin.
///
/// A path's final segment, lowercased and reduced to the letters and digits the
/// grammar of names admits — so an implementation for a machine integer carries
/// its own spelling and one for a named type carries that name. A type that is no
/// path carries its digest word instead: what stands in a pin is never the source
/// text of a type, which is the invariant the record's caveat records.
fn type_word(kind: &Type) -> String {
    let word: Option<String> = match kind {
        Type::Path(path) => path.path.segments.last().map(|segment| {
            segment
                .ident
                .to_string()
                .to_lowercase()
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .collect()
        }),
        _ => None,
    };

    word.filter(|word| !word.is_empty())
        .unwrap_or_else(|| digest_word(normalise_text(&quote_type(kind)).as_bytes()))
}

/// A type's source text, for a digest and for nothing else.
fn quote_type(kind: &Type) -> String {
    use syn::__private::ToTokens;

    kind.to_token_stream().to_string()
}

/// Record one declaration, when its identifier is written in the screaming case.
fn push_declaration(
    attributes: &[Attribute],
    identifier: &syn::Ident,
    value: Option<&Expr>,
    binding: Binding,
    source: &Source<'_>,
    found: &mut Vec<ConstantDeclaration>,
) {
    if !is_screaming(&identifier.to_string()) {
        return;
    }

    let position = identifier.span().start();
    let offset = byte_offset(source.text, source.starts, position.line, position.column);
    let doc = read_doc(attributes);
    let doc_locations = doc.as_ref().map_or_else(Vec::new, |doc| {
        doc_line_locations(doc, source.path, source.text, source.starts)
    });

    found.push(ConstantDeclaration {
        package: source.package.to_owned(),
        path: source.path.to_path_buf(),
        location: Location::new(source.path, source.text, offset),
        doc,
        doc_locations,
        value: value.map(|value| normalise_value(value, source.text, source.starts)),
        binding,
    });
}

/// Record one macro invocation, when its own documentation adopts it.
///
/// An invocation carrying no mint of this profile's kind is no declaration of
/// this census, which is the staged adoption held exactly as it is held for
/// constants: presence is not enforced, so a source full of invocations acquires
/// no coverage burden from the convention that admits them
/// (ADR-T-018, The constant label profile).
///
/// A `macro_rules!` definition is passed over whatever its documentation says. A
/// definition is not an invocation, it declares no arguments of its own, and the
/// body it defines is expanded by whichever invocation reaches it — which is the
/// invocation this census reads instead.
fn push_invocation(
    invocation: &syn::ItemMacro,
    source: &Source<'_>,
    found: &mut Vec<ConstantDeclaration>,
) {
    if invocation.ident.is_some() || invocation.mac.path.is_ident(MACRO_RULES) {
        return;
    }

    let Some(doc) = read_doc(&invocation.attrs) else {
        return;
    };

    if !adopts_a_constant(&doc) {
        return;
    }

    let position = invocation.mac.path.span().start();
    let offset = byte_offset(source.text, source.starts, position.line, position.column);
    let doc_locations = doc_line_locations(&doc, source.path, source.text, source.starts);

    found.push(ConstantDeclaration {
        package: source.package.to_owned(),
        path: source.path.to_path_buf(),
        location: Location::new(source.path, source.text, offset),
        doc: Some(doc),
        doc_locations,
        value: Some(invocation_tokens(
            &invocation.mac,
            source.text,
            source.starts,
        )),
        binding: Binding::Invocation,
    });
}

/// Whether a documentation comment carries a mint of this profile's kind.
///
/// This is what adopts an invocation, and it is deliberately the widest of the
/// readings that carry such a mint rather than the identity's alone: an adoption
/// written in a shape the profile does not own is still an adoption somebody
/// attempted, and censusing it is what lets the malformed place be reported
/// rather than silently passed over.
fn adopts_a_constant(doc: &DocComment) -> bool {
    doc.lines().iter().any(|line| {
        matches!(
            read_line(doc_line(line)),
            Line::Identity { .. } | Line::Bare { .. } | Line::Malformed
        )
    })
}

/// The argument tokens of an invocation, with their whitespace normalised.
///
/// The bytes read are those standing between the invocation's opening delimiter
/// and its closing delimiter, exclusive of both, whichever of the three
/// delimiters the invocation is written with. So the macro's own name is no part
/// of the value and neither is the body it expands to: renaming the macro moves
/// no pin, editing its definition re-mints nothing, and changing, adding, or
/// reordering an argument re-mints exactly one thing.
fn invocation_tokens(invocation: &syn::Macro, text: &str, starts: &[usize]) -> String {
    let delimiters = invocation.delimiter.span();
    let opening = delimiters.open().end();
    let closing = delimiters.close().start();
    let start = byte_offset(text, starts, opening.line, opening.column);
    let end = byte_offset(text, starts, closing.line, closing.column);

    text.get(start..end)
        .map_or_else(String::new, normalise_text)
}

/// Where each line of a documentation comment stands, when the lines are its own.
///
/// A comment whose logical lines are not its file lines — a block comment, or a
/// documentation attribute carrying several lines in one — yields none, and the
/// declaration's own position stands for every line instead.
fn doc_line_locations(
    doc: &DocComment,
    path: &Path,
    text: &str,
    starts: &[usize],
) -> Vec<Location> {
    if !doc.is_line_oriented() || doc.last_line() + 1 != doc.first_line() + doc.lines().len() {
        return Vec::new();
    }

    (doc.first_line()..=doc.last_line())
        .map(|line| Location::new(path, text, byte_offset(text, starts, line, 0)))
        .collect()
}

/// Whether an identifier is written in the case the corpus reserves for constants.
fn is_screaming(identifier: &str) -> bool {
    identifier.starts_with(|character: char| character.is_ascii_uppercase())
        && identifier.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Whether an item is gated behind the test configuration.
fn gated_by_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let Meta::List(list) = &attribute.meta else {
            return false;
        };

        list.path.is_ident("cfg")
            && list
                .tokens
                .to_string()
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .any(|token| token == "test")
    })
}

/// The source text of a declaration's value, with its whitespace normalised.
///
/// The value alone is read — never the declaration's type — and every maximal run
/// of whitespace becomes one space, so a value the formatter wrapped derives the
/// pin it would derive on one line.
fn normalise_value(value: &Expr, text: &str, starts: &[usize]) -> String {
    let span = value.span();
    let start = byte_offset(text, starts, span.start().line, span.start().column);
    let end = byte_offset(text, starts, span.end().line, span.end().column);

    text.get(start..end)
        .map_or_else(String::new, normalise_text)
}

/// Collapse every maximal run of whitespace to one space, and trim the ends.
fn normalise_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Rewrite one source so that every covered constant carries its derived pins.
///
/// Returns `None` when the source already says what its labels say. The sweep
/// writes pin lines and nothing else: an identity, a warrant, and a program
/// citation are the judgments the policy asks a person for, and the claim
/// profile's precedent is that a tool never writes those.
#[must_use]
pub fn write_pins(text: &str, covered: &[&CoveredConstant]) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    let mut written = false;

    for constant in covered {
        let Some(index) = lines
            .iter()
            .position(|line| is_line(line, constant.identity_line()))
        else {
            continue;
        };

        let prefix = leader_of(&lines[index]).to_owned();
        let wanted: Vec<String> = constant
            .pin_lines()
            .iter()
            .map(|pin| format!("{prefix}{pin}"))
            .collect();
        let standing = lines
            .iter()
            .skip(index + 1)
            .take_while(|line| holds_bare_constant(line))
            .count();

        if lines[index + 1..index + 1 + standing] == wanted[..] {
            continue;
        }

        lines.splice(index + 1..index + 1 + standing, wanted);
        written = true;
    }

    written.then(|| {
        let mut rewritten = lines.join("\n");

        if text.ends_with('\n') {
            rewritten.push('\n');
        }

        rewritten
    })
}

/// Whether a file line carries exactly one documentation line's content.
fn is_line(line: &str, content: &str) -> bool {
    doc_line(line) == content
}

/// The leaders and indentation a documentation line begins with.
fn leader_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start_matches(LEADERS).len()]
}

/// Whether a file line is a bare span of this profile's kind and nothing else.
fn holds_bare_constant(line: &str) -> bool {
    matches!(read_line(doc_line(line)), Line::Bare { .. })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        ConstantAnalysis, ConstantCensus, CoveredConstant, Program, analyze_constants, derive_pin,
        digest_word, natural_slug, scan_constants, write_pins,
    };
    use crate::finding::{Finding, UnwarrantedPinReason};
    use crate::label::Label;

    const ACUTE: char = '\u{b4}';

    /// A label, parsed for a fixture.
    fn label(text: &str) -> Label {
        Label::parse(text).expect("well-formed")
    }

    /// An identity's standard place, for an identity name and a program.
    fn identity(name: &str, program: &str) -> String {
        format!("{ACUTE}const:demo:{name}{ACUTE} ({ACUTE}[INDEX-alg:const:{program}]{ACUTE})")
    }

    /// A pin's standard place, for an identity name, a program, and a slug.
    fn pin(name: &str, program: &str, slug: &str) -> String {
        format!("{ACUTE}const:demo:{name}-{program}-{slug}{ACUTE}")
    }

    /// A warrant citation, standing in the documentation prose.
    fn warrant() -> String {
        format!("Bounded by the measurement ({ACUTE}sec:demo:witness{ACUTE}).")
    }

    /// A source declaring one constant under the documentation lines given.
    fn declared(doc: &[String], identifier: &str, value: &str) -> String {
        let mut text = String::new();

        for line in doc {
            text.push_str("/// ");
            text.push_str(line);
            text.push('\n');
        }

        text.push_str("const ");
        text.push_str(identifier);
        text.push_str(": usize = ");
        text.push_str(value);
        text.push_str(";\n");

        text
    }

    /// Analyse a census taken from sources given as path and text.
    fn analyze(sources: &[(&str, &str)]) -> (ConstantAnalysis, Vec<CoveredConstant>, Vec<Finding>) {
        let mut declarations = Vec::new();

        for (path, text) in sources {
            declarations.extend(
                scan_constants("ember-demo", Path::new(path), text).expect("a Rust source"),
            );
        }

        analyze_constants(
            &ConstantCensus::from_declarations(declarations, sources.len()),
            &crate::subscribe::Subscription::fictional_all(),
        )
    }

    /// Analyse one source under the package's own directory.
    fn analyze_one(text: &str) -> (ConstantAnalysis, Vec<CoveredConstant>, Vec<Finding>) {
        analyze(&[("packages/demo/src/engine.rs", text)])
    }

    /// The count program reads a decimal integer and nothing else: the digits
    /// stand as they are written, a separator is a reading aid the value does not
    /// carry, and a sign, a point, or a suffix is refused rather than repaired.
    ///
    /// ´claim:constant:the-count-program-reads-decimal-digits-and-refuses-the-rest´
    /// ´test:unit:derives-a-count-from-its-digits´
    #[test]
    fn derives_a_count_from_its_digits() {
        assert_eq!(Program::Count.derive("16").as_deref(), Some("16"));
        assert_eq!(Program::Count.derive("1_000").as_deref(), Some("1000"));
        assert_eq!(Program::Count.derive("720000").as_deref(), Some("720000"));

        for refused in ["-4", "0.5", "16usize", "0xff", ""] {
            assert!(
                Program::Count.derive(refused).is_none(),
                "expected {refused} refused"
            );
        }
    }

    /// The version program accepts and derives exactly what the count program
    /// does, and is a separate citation because it records an intent rather than
    /// a shape: a version is expected to be bumped, so its pin is expected to
    /// re-mint and the citations that dangle are the migration checklist.
    ///
    /// ´claim:constant:the-version-program-is-the-count-program-under-an-intent´
    /// ´test:unit:derives-a-version-as-the-count-program-does´
    #[test]
    fn derives_a_version_as_the_count_program_does() {
        assert_eq!(Program::Version.derive("10").as_deref(), Some("10"));
        assert_eq!(Program::Version.derive("10"), Program::Count.derive("10"));
        assert!(Program::Version.derive("10.1").is_none());
    }

    /// The scalar program rewrites the three characters the grammar of names does
    /// not admit and nothing else: the leading minus becomes a word, the point
    /// becomes a letter, and an exponent's minus becomes a letter beside the
    /// exponent's own. An integer is refused, because it is the count program's
    /// and a value two programs accepted would have two pins.
    ///
    /// ´claim:constant:the-scalar-program-rewrites-the-three-inadmissible-characters´
    /// ´test:unit:derives-a-scalar-by-rewriting-the-inadmissible-characters´
    #[test]
    fn derives_a_scalar_by_rewriting_the_inadmissible_characters() {
        assert_eq!(Program::Scalar.derive("0.9995").as_deref(), Some("0p9995"));
        assert_eq!(Program::Scalar.derive("1e-14").as_deref(), Some("1en14"));
        assert_eq!(Program::Scalar.derive("-4.0").as_deref(), Some("neg4p0"));
        assert_eq!(Program::Scalar.derive("8_760.0").as_deref(), Some("8760p0"));
        assert_eq!(Program::Scalar.derive("1E5").as_deref(), Some("1e5"));
        assert_eq!(Program::Scalar.derive("1e+5").as_deref(), Some("1e5"));

        assert!(
            Program::Scalar.derive("15").is_none(),
            "an integer is the count program's"
        );
        assert!(
            Program::Scalar.derive("1.0 / 12.0").is_none(),
            "an expression is the form program's"
        );
    }

    /// The seconds program reads the seconds a value counts, whether the source
    /// writes the number bare or wraps it in the duration constructor, so that
    /// renaming the type moves no pin. A constructor of anything else is refused.
    ///
    /// ´claim:constant:the-seconds-program-reads-the-count-through-its-constructor´
    /// ´test:unit:derives-seconds-through-the-duration-constructor´
    #[test]
    fn derives_seconds_through_the_duration_constructor() {
        assert_eq!(Program::Seconds.derive("3600").as_deref(), Some("3600"));
        assert_eq!(
            Program::Seconds
                .derive("Duration::from_secs(3600)")
                .as_deref(),
            Some("3600")
        );
        assert_eq!(
            Program::Seconds
                .derive("std::time::Duration::from_secs(86_400)")
                .as_deref(),
            Some("86400")
        );

        assert!(
            Program::Seconds
                .derive("Duration::from_millis(500)")
                .is_none()
        );
        assert!(
            Program::Seconds
                .derive("Duration::from_secs(SOME)")
                .is_none()
        );
    }

    /// The word program accepts a string that is already a word of the label
    /// grammar and carries it verbatim. A string with a capital, a space, or a
    /// punctuation mark in it is refused rather than slugified, because a lossy
    /// name over a value is the non-injective encoding the record refuses.
    ///
    /// ´claim:constant:the-word-program-carries-an-already-well-formed-string´
    /// ´test:unit:derives-a-word-from-a-string-that-is-one´
    #[test]
    fn derives_a_word_from_a_string_that_is_one() {
        assert_eq!(Program::Word.derive("\"v1\"").as_deref(), Some("v1"));
        assert_eq!(
            Program::Word.derive("\"assayer\"").as_deref(),
            Some("assayer")
        );

        for refused in ["\"V1\"", "\"two words\"", "\"\"", "\"kebab-case\"", "16"] {
            assert!(
                Program::Word.derive(refused).is_none(),
                "expected {refused} refused"
            );
        }
    }

    /// The codepoint program reads the character rather than its spelling, so an
    /// escape and the character itself derive one slug — which is the property a
    /// pin over characters needs, since a source may spell one either way.
    ///
    /// ´claim:constant:the-codepoint-program-reads-the-character-not-its-spelling´
    /// ´test:unit:derives-a-codepoint-from-its-scalar-value´
    #[test]
    fn derives_a_codepoint_from_its_scalar_value() {
        assert_eq!(
            Program::Codepoint.derive("'\\u{b4}'").as_deref(),
            Some("ub4")
        );
        assert_eq!(Program::Codepoint.derive("'a'").as_deref(), Some("u61"));
        assert_eq!(Program::Codepoint.derive("'\\n'").as_deref(), Some("ua"));
        assert_eq!(
            Program::Codepoint.derive("'\\u{61}'"),
            Program::Codepoint.derive("'a'"),
            "one character, one slug, however it is written"
        );

        assert!(
            Program::Codepoint.derive("\"a\"").is_none(),
            "a string is not a character"
        );
    }

    /// The text program digests the value a string carries rather than the
    /// spelling that carries it, so a raw literal and a plain one of the same
    /// content derive one pin and a symbol-heavy value is pinned at all.
    ///
    /// ´claim:constant:the-text-program-digests-the-value-not-its-spelling´
    /// ´test:unit:digests-a-text-by-the-value-it-carries´
    #[test]
    fn digests_a_text_by_the_value_it_carries() {
        assert_eq!(
            Program::Text.derive("\"%Y-%m-%d %H:%M:%S\"").as_deref(),
            Some("xe8113875")
        );
        assert_eq!(
            Program::Text.derive("r\"%Y-%m-%d %H:%M:%S\""),
            Program::Text.derive("\"%Y-%m-%d %H:%M:%S\""),
            "the raw spelling carries the same value"
        );
        assert_eq!(Program::Text.derive("\"\"").as_deref(), Some("x84222325"));

        assert!(Program::Text.derive("16").is_none(), "a number is no text");
    }

    /// The form program digests the value's normalised source, so a value the
    /// formatter wrapped across lines derives the pin it would derive on one, and
    /// a table with no plausible spelling is pinned by its bytes.
    ///
    /// ´claim:constant:the-form-program-digests-the-normalised-source-of-a-value´
    /// ´test:unit:digests-a-form-over-its-normalised-source´
    #[test]
    fn digests_a_form_over_its_normalised_source() {
        assert_eq!(
            Program::Form.derive("&[1, 2, 3]").as_deref(),
            Some("xd5ea4b85")
        );
        assert_eq!(
            Program::Form.derive("24 * 3600").as_deref(),
            Program::Form.derive("24 * 3600").as_deref()
        );
        assert_ne!(
            Program::Form.derive("24 * 3600"),
            Program::Form.derive("86400")
        );
    }

    /// The digest word is arithmetic and nothing else, so it can be recomputed by
    /// hand and by another implementation: the empty sequence gives the offset
    /// basis's own low half, and every other input follows from the two constants
    /// the record states.
    ///
    /// ´claim:constant:the-digest-word-is-recomputable-from-the-records-constants´
    /// ´test:unit:recomputes-the-digest-word-from-its-constants´
    #[test]
    fn recomputes_the_digest_word_from_its_constants() {
        assert_eq!(
            digest_word(b""),
            "x84222325",
            "the offset basis's own low half"
        );
        assert_eq!(digest_word(b"a"), "x8601ec8c");
        assert_eq!(digest_word(b"&[1, 2, 3]"), "xd5ea4b85");
        assert_eq!(digest_word(b"").len(), 9, "one letter and eight digits");
    }

    /// A value given no citation of its own is encoded by the catalogue read in
    /// order, so the program over implementations always has an answer: an
    /// integer counts, a float is a scalar, a word stands as itself, and anything
    /// else falls through to a digest.
    ///
    /// ´claim:constant:an-uncited-value-is-encoded-by-the-catalogue-in-order´
    /// ´test:unit:encodes-an-uncited-value-by-the-catalogue-in-order´
    #[test]
    fn encodes_an_uncited_value_by_the_catalogue_in_order() {
        assert_eq!(natural_slug("32"), "32");
        assert_eq!(natural_slug("0.5"), "0p5");
        assert_eq!(natural_slug("\"v1\""), "v1");
        assert_eq!(natural_slug("'a'"), "u61");
        assert_eq!(natural_slug("\"two words\""), digest_word(b"two words"));
        assert_eq!(natural_slug("&[1, 2, 3]"), digest_word(b"&[1, 2, 3]"));
    }

    /// The pin extends the identity it pins and carries the program between them,
    /// so a reader picks the program out of the label and recomputes the slug —
    /// and two programs deriving one slug still mint two different pins.
    ///
    /// ´claim:constant:a-pin-extends-its-identity-and-names-its-program´
    /// ´test:unit:derives-a-pin-that-carries-its-program´
    #[test]
    fn derives_a_pin_that_carries_its_program() {
        let identity = label("const:assayer:eviction-batch-bound");

        assert_eq!(
            derive_pin(&identity, Program::Count, "16")
                .expect("well-formed")
                .to_string(),
            "const:assayer:eviction-batch-bound-count-16"
        );
        assert_ne!(
            derive_pin(&identity, Program::Count, "3600"),
            derive_pin(&identity, Program::Seconds, "3600"),
            "one slug under two programs is two pins"
        );
    }

    /// A constant carrying its warrant, its identity, its program citation and
    /// its derived pin is clean, and is counted as covered and pinned. This is
    /// what a conforming constant looks like.
    ///
    /// ´claim:constant:a-constant-carrying-all-four-occurrences-is-clean´
    /// ´test:unit:accepts-a-constant-carrying-all-four-occurrences´
    #[test]
    fn accepts_a_constant_carrying_all_four_occurrences() {
        let text = declared(
            &[
                warrant(),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
            ],
            "MAX_EVICTIONS_PER_INSERT",
            "16",
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(
            findings.len(),
            0,
            "a conforming constant is clean: {findings:?}"
        );
        assert_eq!(analysis.covered, 1);
        assert_eq!(analysis.pinned, 1);
        assert_eq!(analysis.by_program["count"], 1);
        assert_eq!(
            covered[0].identity().to_string(),
            "const:demo:eviction-batch-bound"
        );
    }

    /// The binding is positional and never through the identifier: the same
    /// documentation over a differently named declaration derives the same
    /// labels, so a rename moves nothing and dangles nothing.
    ///
    /// ´claim:constant:renaming-the-identifier-moves-no-label´
    /// ´test:unit:renaming-the-identifier-moves-nothing´
    #[test]
    fn renaming_the_identifier_moves_nothing() {
        let doc = [
            warrant(),
            identity("eviction-batch-bound", "count"),
            pin("eviction-batch-bound", "count", "16"),
        ];
        let (_before, first, _clean) =
            analyze_one(&declared(&doc, "MAX_EVICTIONS_PER_INSERT", "16"));
        let (_after, second, findings) = analyze_one(&declared(&doc, "EVICTION_BUDGET", "16"));

        assert_eq!(findings.len(), 0, "the rename is no defect: {findings:?}");
        assert_eq!(
            first[0].pins(),
            second[0].pins(),
            "the pins are the same labels"
        );
        assert_eq!(first[0].identity(), second[0].identity());
    }

    /// Changing the value re-derives the pin, which is the tripwire the record
    /// exists to build: the label a document cited is not the label the
    /// declaration now mints, so every citation of it dangles in that commit.
    ///
    /// ´claim:constant:changing-the-value-re-mints-the-pin´
    /// ´test:unit:re-derives-the-pin-when-the-value-changes´
    #[test]
    fn re_derives_the_pin_when_the_value_changes() {
        let doc = [warrant(), identity("eviction-batch-bound", "count")];
        let (_analysis, covered, _findings) = analyze_one(&declared(&doc, "BOUND", "32"));

        assert_eq!(
            covered[0].pins()[0].to_string(),
            "const:demo:eviction-batch-bound-count-32",
            "the pin follows the value"
        );
    }

    /// A pin that is not the derivation is reported showing both what stands
    /// there and what the declaration gives, so a value moved under a pin nobody
    /// re-swept is caught rather than believed.
    ///
    /// ´claim:constant:a-pin-that-is-not-the-derivation-is-reported-with-both´
    /// ´test:unit:reports-a-stale-pin-with-both-labels´
    #[test]
    fn reports_a_stale_pin_with_both_labels() {
        let text = declared(
            &[
                warrant(),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
            ],
            "BOUND",
            "32",
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(analysis.stale_pins, 1);
        assert!(!covered[0].is_pinned(), "a stale pin is not standing");

        let [
            Finding::WrongConstantPin {
                expected, found, ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one stale pin, got {findings:?}");
        };
        assert_eq!(
            expected.to_string(),
            "const:demo:eviction-batch-bound-count-32"
        );
        assert_eq!(
            found.to_string(),
            "const:demo:eviction-batch-bound-count-16"
        );
    }

    /// A constant whose pin is not written at all is reported with the pin the
    /// derivation gives, so the author is told what the sweep would write.
    ///
    /// ´claim:constant:a-missing-pin-is-reported-with-its-derivation´
    /// ´test:unit:reports-a-missing-pin-with-its-derivation´
    #[test]
    fn reports_a_missing_pin_with_its_derivation() {
        let text = declared(
            &[warrant(), identity("eviction-batch-bound", "count")],
            "BOUND",
            "16",
        );
        let (analysis, _covered, findings) = analyze_one(&text);

        assert_eq!(analysis.missing_pins, 1);

        let [Finding::MissingConstantPin { expected, .. }] = findings.as_slice() else {
            panic!("expected one missing pin, got {findings:?}");
        };
        assert_eq!(
            expected.to_string(),
            "const:demo:eviction-batch-bound-count-16"
        );
    }

    /// A pin standing where no identity heads it warrants nothing and is
    /// reported: the generated line is warranted by the identity above it, and
    /// without one it is a hand writing a line no census gives.
    ///
    /// ´claim:constant:a-pin-no-identity-heads-warrants-nothing´
    /// ´test:unit:reports-a-pin-no-identity-heads´
    #[test]
    fn reports_a_pin_no_identity_heads() {
        let text = declared(
            &[warrant(), pin("eviction-batch-bound", "count", "16")],
            "BOUND",
            "16",
        );
        let (analysis, _covered, findings) = analyze_one(&text);

        assert_eq!(analysis.unwarranted_pins, 1);

        let [Finding::UnwarrantedConstantPin { reason, .. }] = findings.as_slice() else {
            panic!("expected one unwarranted pin, got {findings:?}");
        };
        assert_eq!(*reason, UnwarrantedPinReason::NoIdentity);
    }

    /// A pin beneath an identity that derives none is the other way of warranting
    /// nothing, and is reported under its own reason: a second line beneath a
    /// declaration pinned once is not a pin of anything.
    ///
    /// (´claim:constant:a-pin-no-identity-heads-warrants-nothing´)
    /// ´test:unit:reports-a-pin-beneath-an-identity-that-derives-none´
    #[test]
    fn reports_a_pin_beneath_an_identity_that_derives_none() {
        let text = declared(
            &[
                warrant(),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
                pin("eviction-batch-bound", "count", "32"),
            ],
            "BOUND",
            "16",
        );
        let (analysis, _covered, findings) = analyze_one(&text);

        assert_eq!(analysis.unwarranted_pins, 1);

        let [Finding::UnwarrantedConstantPin { reason, label, .. }] = findings.as_slice() else {
            panic!("expected one unwarranted pin, got {findings:?}");
        };
        assert_eq!(*reason, UnwarrantedPinReason::NoValue);
        assert_eq!(
            label.to_string(),
            "const:demo:eviction-batch-bound-count-32"
        );
    }

    /// A citation naming no catalogued program is reported at the constant, where
    /// the repair is: the class of a constant is a citation rather than a word,
    /// so adding one means minting a program's environment and implementing it.
    ///
    /// ´claim:constant:a-citation-of-no-catalogued-program-is-reported-at-the-constant´
    /// ´test:unit:reports-a-citation-of-no-catalogued-program´
    #[test]
    fn reports_a_citation_of_no_catalogued_program() {
        let text = declared(
            &[warrant(), identity("eviction-batch-bound", "furlongs")],
            "BOUND",
            "16",
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(analysis.uncatalogued_programs, 1);
        assert!(
            covered.is_empty(),
            "a constant with no program is pinned by nothing"
        );

        let [Finding::UncataloguedPinningProgram { cited, .. }] = findings.as_slice() else {
            panic!("expected one uncatalogued program, got {findings:?}");
        };
        assert_eq!(cited.to_string(), "alg:const:furlongs");
    }

    /// A value its cited program refuses is reported naming both, and the check
    /// is free: the citation says what the value is supposed to be, so a float
    /// under a count citation is one of the two wrong.
    ///
    /// ´claim:constant:a-value-its-program-refuses-is-reported-naming-both´
    /// ´test:unit:reports-a-value-its-program-refuses´
    #[test]
    fn reports_a_value_its_program_refuses() {
        let text = declared(
            &[warrant(), identity("decay-rate", "count")],
            "RATE",
            "0.999",
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(analysis.refused_values, 1);
        assert!(
            covered[0].pins().is_empty(),
            "a refused value derives no pin"
        );

        let [Finding::RefusedConstantValue { program, value, .. }] = findings.as_slice() else {
            panic!("expected one refused value, got {findings:?}");
        };
        assert_eq!(program, "count");
        assert_eq!(value.as_deref(), Some("0.999"));
    }

    /// The identity's place holds the identity mint and its program citation
    /// alone, so a line minting the kind in any other shape is reported rather
    /// than passed over: a shape the checker did not recognise is the case an
    /// author most needs told about.
    ///
    /// ´claim:constant:an-identity-place-of-another-shape-is-reported´
    /// ´test:unit:reports-an-identity-place-of-another-shape´
    #[test]
    fn reports_an_identity_place_of_another_shape() {
        let text = declared(
            &[
                warrant(),
                format!(
                    "{ACUTE}const:demo:eviction-batch-bound{ACUTE} pinned by the count program"
                ),
            ],
            "BOUND",
            "16",
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(analysis.malformed_identities, 1);
        assert!(covered.is_empty(), "a malformed place mints nothing");
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::MalformedConstantIdentity { .. }]
            ),
            "expected the malformed identity, got {findings:?}"
        );
    }

    /// An identity over documentation that neither cites a warrant nor admits
    /// owing one is the state the record refuses, and it is reported at the
    /// identity's own line.
    ///
    /// ´claim:constant:an-identity-citing-no-warrant-is-reported´
    /// ´test:unit:reports-an-identity-citing-no-warrant´
    #[test]
    fn reports_an_identity_citing_no_warrant() {
        let text = declared(
            &[
                "Caps the latency of an insert.".to_owned(),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
            ],
            "BOUND",
            "16",
        );
        let (analysis, _covered, findings) = analyze_one(&text);

        assert_eq!(analysis.missing_warrants, 1);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::MissingConstantWarrant { .. }]
            ),
            "expected the missing warrant, got {findings:?}"
        );
    }

    /// A to-do standard place standing in the documentation is the bootstrap's
    /// accepted warrant: it says in the corpus's own vocabulary that the record
    /// is owed rather than absent, and it dangles when the notice is discharged.
    ///
    /// ´claim:constant:a-to-do-standard-place-is-an-accepted-interim-warrant´
    /// ´test:unit:accepts-a-to-do-standard-place-as-the-interim-warrant´
    #[test]
    fn accepts_a_to_do_standard_place_as_the_interim_warrant() {
        let text = declared(
            &[
                format!(
                    "TODO {ACUTE}todo:code:record-the-latency-budget{ACUTE}: record the latency budget."
                ),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
            ],
            "BOUND",
            "16",
        );
        let (analysis, _covered, findings) = analyze_one(&text);

        assert_eq!(analysis.missing_warrants, 0);
        assert_eq!(
            findings.len(),
            0,
            "the interim warrant is a warrant: {findings:?}"
        );
    }

    /// The census reads the declarations that ship and no others: a binding
    /// inside a function body is a local, a declaration under the test
    /// configuration is fixed for the test, and an identifier not written in the
    /// screaming case is no constant of this policy. An associated constant of an
    /// implementation is read like any other.
    ///
    /// ´claim:constant:the-census-reads-production-declarations-and-no-others´
    /// ´test:unit:censuses-production-declarations-and-no-others´
    #[test]
    fn censuses_production_declarations_and_no_others() {
        let text = concat!(
            "const SHIPPED: usize = 1;\n",
            "static ALSO_SHIPPED: usize = 2;\n",
            "fn work() {\n    const LOCAL: f64 = 1e-14;\n}\n",
            "#[cfg(test)]\nmod tests {\n    const GATED: usize = 3;\n}\n",
            "const lowercase: usize = 4;\n",
            "impl Widget {\n    const ASSOCIATED: usize = 5;\n}\n",
        );
        let declarations = scan_constants(
            "ember-demo",
            Path::new("packages/demo/src/engine.rs"),
            text,
        )
        .expect("a source");

        assert_eq!(
            declarations.len(),
            3,
            "shipped, also shipped, and the associated one: {declarations:?}"
        );
        assert_eq!(
            declarations
                .iter()
                .filter(|declaration| declaration.value() == Some("1"))
                .count(),
            1
        );
    }

    /// The census admits a macro invocation its own documentation adopts and no
    /// other: an invocation carrying no mint of this profile's kind is no
    /// declaration of the census, so a source full of them acquires no coverage
    /// burden, and a `macro_rules!` definition is passed over however it is
    /// documented, a definition being no invocation.
    ///
    /// ´claim:constant:the-census-admits-the-adopted-invocation-alone´
    /// ´test:unit:censuses-an-adopted-invocation-and-no-other´
    #[test]
    fn censuses_an_adopted_invocation_and_no_other() {
        let text = format!(
            concat!(
                "/// {}\n/// {}\nmarkers! {{ Alpha, Beta }}\n",
                "undocumented! {{ Gamma }}\n",
                "/// Prose adopting nothing at all.\nnarrated! {{ Delta }}\n",
                "/// {}\n/// {}\nmacro_rules! defined {{ () => {{}} }}\n",
            ),
            warrant(),
            identity("catalogue", "form"),
            warrant(),
            identity("definition", "form"),
        );
        let declarations = scan_constants(
            "ember-demo",
            Path::new("packages/demo/src/engine.rs"),
            &text,
        )
        .expect("a source");

        assert_eq!(
            declarations.len(),
            1,
            "the adopted invocation alone: {declarations:?}"
        );
        assert_eq!(
            declarations[0].value(),
            Some("Alpha, Beta"),
            "and its argument tokens are its value"
        );
    }

    /// An adopted invocation is pinned over the bytes standing between its
    /// opening and closing delimiters, exclusive of both, normalised as every
    /// form value is. So the macro's own name is no part of the pin and the
    /// formatter moves nothing, while reordering an argument re-mints exactly
    /// one label.
    ///
    /// ´claim:constant:an-invocation-is-pinned-over-the-tokens-between-its-delimiters´
    /// ´test:unit:pins-an-invocation-over-the-tokens-between-its-delimiters´
    #[test]
    fn pins_an_invocation_over_the_tokens_between_its_delimiters() {
        let invoked = |written: &str| {
            format!(
                "/// {}\n/// {}\n{written}\n",
                warrant(),
                identity("catalogue", "form")
            )
        };

        for (written, expected, why) in [
            (
                "markers! { Alpha, Beta }",
                "xa1edf3d5",
                "the tokens as written",
            ),
            (
                "markers! {\n    Alpha,\n    Beta\n}",
                "xa1edf3d5",
                "the formatter moves nothing",
            ),
            (
                "others![ Alpha, Beta ];",
                "xa1edf3d5",
                "neither the name, the delimiter, nor the semicolon is read",
            ),
            (
                "markers! { Beta, Alpha }",
                "x23e25645",
                "reordering re-mints the pin",
            ),
        ] {
            let (analysis, covered, findings) = analyze_one(&invoked(written));

            assert_eq!(analysis.covered, 1, "{why}: {findings:?}");
            assert_eq!(
                covered[0]
                    .pins()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>(),
                vec![format!("const:demo:catalogue-form-{expected}")],
                "{why}"
            );
        }
    }

    /// An adopted invocation cites the form program and no other, its argument
    /// tokens being read by nothing else. A citation of another program is
    /// reported as a value that program refuses, rather than derived from tokens
    /// the program was never defined over.
    ///
    /// ´claim:constant:an-invocation-cites-the-form-program-and-no-other´
    /// ´test:unit:refuses-an-invocation-citing-any-program-but-the-form´
    #[test]
    fn refuses_an_invocation_citing_any_program_but_the_form() {
        for cited in ["count", "text", "impls"] {
            let text = format!(
                "/// {}\n/// {}\nmarkers! {{ 16 }}\n",
                warrant(),
                identity("catalogue", cited)
            );
            let (analysis, _covered, findings) = analyze_one(&text);

            assert_eq!(analysis.pins, 0, "{cited} derives nothing at an invocation");
            assert!(
                findings.iter().any(|finding| matches!(
                    finding,
                    Finding::RefusedConstantValue { program, .. } if program == cited
                )),
                "expected {cited} refused: {findings:?}"
            );
        }
    }

    /// The sweep writes an adopted invocation's pin beneath the identity that
    /// warrants it and settles, exactly as it does at a constant: a second pass
    /// over what it wrote finds nothing to do, so a check run after a sweep
    /// means something here too.
    ///
    /// ´claim:constant:the-sweep-writes-an-invocations-pin-and-settles´
    /// ´test:unit:writes-an-adopted-invocations-pin-and-settles´
    #[test]
    fn writes_an_adopted_invocations_pin_and_settles() {
        let text = format!(
            "/// {}\n/// {}\nmarkers! {{ Alpha, Beta }}\n",
            warrant(),
            identity("catalogue", "form")
        );
        let (_analysis, covered, _findings) = analyze_one(&text);
        let swept = write_pins(&text, &covered.iter().collect::<Vec<&CoveredConstant>>())
            .expect("a pin to write");

        assert!(
            swept.contains(&pin("catalogue", "form", "xa1edf3d5")),
            "wrote the pin: {swept}"
        );

        let (analysis, again, _findings) = analyze_one(&swept);

        assert_eq!(
            analysis.pinned, 1,
            "and the pin stands where it was written"
        );
        assert!(
            write_pins(&swept, &again.iter().collect::<Vec<&CoveredConstant>>()).is_none(),
            "a second pass writes nothing"
        );
    }

    /// A trait's declaration is one concept with several values, so it carries
    /// one identity and one pin per implementation of its package, each slug
    /// naming the implementing type and the value that implementation supplies.
    ///
    /// ´claim:constant:a-trait-declaration-carries-one-pin-per-implementation´
    /// ´test:unit:pins-a-trait-declaration-once-per-implementation´
    #[test]
    fn pins_a_trait_declaration_once_per_implementation() {
        let declaration = format!(
            "trait Coordinate {{\n    /// {}\n    /// {}\n    const BITS: usize;\n}}\n",
            warrant(),
            identity("coordinate-width", "impls")
        );
        let implementations = concat!(
            "impl Coordinate for u32 {\n    const BITS: usize = 32;\n}\n",
            "impl Coordinate for u64 {\n    const BITS: usize = 64;\n}\n",
        );
        let (analysis, covered, findings) = analyze(&[
            ("packages/demo/src/coordinate.rs", &declaration),
            ("packages/demo/src/widths.rs", implementations),
        ]);

        assert_eq!(analysis.covered, 1, "one identity, at the trait");
        assert_eq!(analysis.pins, 2, "and one pin per implementation");

        let pins: Vec<String> = covered[0].pins().iter().map(ToString::to_string).collect();
        assert_eq!(
            pins,
            [
                "const:demo:coordinate-width-impls-u32-32",
                "const:demo:coordinate-width-impls-u64-64"
            ]
        );
        assert_eq!(
            analysis.missing_pins, 2,
            "and the sweep owes both: {findings:?}"
        );
    }

    /// The sweep writes the pin lines a constant owes, beneath the identity that
    /// warrants them, and settles: a second pass over what it wrote finds nothing
    /// to do, so a check run after a sweep means something.
    ///
    /// ´claim:constant:the-sweep-writes-the-pins-it-owes-and-then-settles´
    /// ´test:unit:writes-the-pins-the-sweep-owes-and-settles´
    #[test]
    fn writes_the_pins_the_sweep_owes_and_settles() {
        let text = declared(
            &[warrant(), identity("eviction-batch-bound", "count")],
            "BOUND",
            "16",
        );
        let (_analysis, covered, _findings) = analyze_one(&text);
        let borrowed: Vec<&CoveredConstant> = covered.iter().collect();
        let written = write_pins(&text, &borrowed).expect("a pin was owed");

        assert!(
            written.contains(&pin("eviction-batch-bound", "count", "16")),
            "the pin stands after the sweep: {written}"
        );

        let (analysis, again, findings) = analyze_one(&written);
        let borrowed: Vec<&CoveredConstant> = again.iter().collect();

        assert_eq!(findings.len(), 0, "the swept source is clean: {findings:?}");
        assert_eq!(analysis.pinned, 1);
        assert!(
            write_pins(&written, &borrowed).is_none(),
            "and the sweep settles"
        );
    }

    /// A constant documented in a block comment carries its standard places like
    /// any other: the comment's own continuation asterisks are decoration, so the
    /// identity is read through them and the pin is written beside them, in the
    /// leaders the identity's own line already uses.
    ///
    /// ´claim:constant:a-block-comment-carries-the-standard-places-like-any-other´
    /// ´test:unit:reads-and-sweeps-a-block-comments-standard-places´
    #[test]
    fn reads_and_sweeps_a_block_comments_standard_places() {
        let text = format!(
            "/** Caps the latency of an insert.\n * {}\n * {}\n */\nconst BOUND: usize = 16;\n",
            warrant(),
            identity("eviction-batch-bound", "count")
        );
        let (analysis, covered, findings) = analyze_one(&text);

        assert_eq!(
            analysis.covered, 1,
            "the identity is read through the asterisks: {findings:?}"
        );

        let borrowed: Vec<&CoveredConstant> = covered.iter().collect();
        let written = write_pins(&text, &borrowed).expect("a pin was owed");

        assert!(
            written.contains(&format!(
                " * {}",
                pin("eviction-batch-bound", "count", "16")
            )),
            "the pin is written in the leaders the identity uses: {written}"
        );

        let (after, _covered, findings) = analyze_one(&written);

        assert_eq!(
            findings.len(),
            0,
            "and the swept source is clean: {findings:?}"
        );
        assert_eq!(after.pinned, 1);
    }

    /// The sweep rewrites a pin that is not the derivation, and writes nothing at
    /// all where no identity stands: a warrant, an identity, and a program
    /// citation are the judgments the policy asks a person for.
    ///
    /// ´claim:constant:the-sweep-rewrites-pins-and-authors-nothing´
    /// ´test:unit:rewrites-a-stale-pin-and-authors-nothing-else´
    #[test]
    fn rewrites_a_stale_pin_and_authors_nothing_else() {
        let stale = declared(
            &[
                warrant(),
                identity("eviction-batch-bound", "count"),
                pin("eviction-batch-bound", "count", "16"),
            ],
            "BOUND",
            "32",
        );
        let (_analysis, covered, _findings) = analyze_one(&stale);
        let borrowed: Vec<&CoveredConstant> = covered.iter().collect();
        let written = write_pins(&stale, &borrowed).expect("a pin was stale");

        assert!(
            written.contains(&pin("eviction-batch-bound", "count", "32")),
            "the pin is rewritten"
        );
        assert!(
            !written.contains(&pin("eviction-batch-bound", "count", "16")),
            "and the stale one is gone"
        );

        let unadopted = declared(
            &["Caps the latency of an insert.".to_owned()],
            "BOUND",
            "16",
        );
        let (_bare, none, _quiet) = analyze_one(&unadopted);

        assert!(
            none.is_empty(),
            "an unadopted constant is covered by nothing"
        );
        assert!(
            write_pins(&unadopted, &[]).is_none(),
            "so the sweep writes nothing"
        );
    }
}
