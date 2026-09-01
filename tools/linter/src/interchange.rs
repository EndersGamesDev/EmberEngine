// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The interchange envelope: a namespace label and a version triple, in three carriers.
//!
//! The interchange conventions state a base theory satisfied by every document
//! of their data language before any assignment is consulted: key 0 holds a
//! namespace label under a closed grammar with a byte bound, key 1 holds a
//! version triple, and every further key is content the base theory does not
//! constrain. The owner has ruled that the discipline reaches this repository's
//! first-party TOML, JSON and YAML alike, carried by one policy family rather
//! than one family per carrier. This module is the transposition — what the
//! envelope *is* when the carrier is a concrete syntax rather than a canonical
//! name — and it decides only what plays key 0 and key 1, because the semantics
//! stay where the record fixes them.
//!
//! # Two reserved names, spelled as words
//!
//! The integer keys of the data language are part of an encoding and not part of
//! the discipline. They buy a canonical byte order this repository's carriers do
//! not have and a size saving none of them needs, so the transposition spells
//! them `namespace` and `version`. What transposes is the envelope, not the
//! encoding of the envelope.
//!
//! Both names are reserved at the top level of a governed document and may not
//! carry content. Nested keys of those names are untouched, and the distinction
//! is load-bearing rather than pedantic: a manifest's `[package] version` is a
//! content key inside a table and is no part of any envelope, which is exactly
//! why this tree holds files that already carry a version and still carry no
//! envelope.
//!
//! # The grammar and the triple are checked over the spelling, not the value
//!
//! Every rule this module enforces is a rule about the bytes a carrier holds,
//! and none of them survives a round trip through a parser's value model. A TOML
//! reader hands back the integer one for `1`, for `0x1` and for `1_0` alike; a
//! YAML reader resolves `010` to eight or to ten depending on which integer
//! resolution it applies; a JSON reader unescapes a label before the caller sees
//! it. So the envelope readers above this module read the raw spelling and the
//! predicates here are written over text, which is the only place the
//! one-spelling rule can be enforced at all: every label is a fixed point of
//! both Unicode normal forms and has exactly one byte form, so a carrier that
//! offered two spellings of one label would be introducing an ambiguity the
//! discipline does not have.
//!
//! A version whose value depends on the parser is not a version claim, and the
//! canonical-integer predicate removes the question rather than answering it.
//!
//! # The universe is the carrier catalog's domain, and it is closed over types
//!
//! This policy's universe is not the repository. A file is *in domain* exactly
//! when the carrier catalog below resolves a type from its name, and every
//! coverage judgment — governance, exclusion, idleness, containment — is taken
//! over the in-domain set alone. An out-of-domain file needs no row, and a row
//! written for one says nothing the policy could act on: the remedy for a file
//! this policy has no business with was never a row saying so.
//!
//! The domain is therefore decided by the catalog and never by a declaration,
//! and that is what makes the type set worth declaring. The declaration names
//! the types it was written against, the catalog is compared against it before
//! any section is read, and either kind of disagreement refuses. A binary that
//! learned a new carrier would otherwise widen every owner's coverage
//! obligation silently, which is the one way this policy's domain can move
//! without anybody deciding it should. The SPDX policy runs the opposite way and
//! is right to: its accepted types are an opt-in the repository grows, so a new
//! type there changes nothing until somebody adopts it.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`a_label_is_two_or_more_atoms_over_a_closed_alphabet`] | interchange | A label is two or more dot-separated atoms over the thirty-six-character alphabet and the interior hyphen. One atom is a bare top-level word and no label, an empty atom from a leading, trailing or doubled dot fails, and an atom opening or closing on a hyphen fails because the hyphen is interior. Upper case and the underscore are outside the alphabet, and so is every character that would need escaping in any of the three carriers — which is what lets one label have exactly one byte form in all of them. |
//! | [`a_label_occupies_at_most_the_record_s_byte_bound`] | interchange | The byte bound is inclusive at the record's figure, and the character bound coincides with it because every character of the alphabet is printable ASCII. A label of the bound passes and one of a single byte more fails, which is the pair the oracle asks for. |
//! | [`a_triple_member_is_canonical_in_its_spelling`] | interchange | A triple member is plain decimal with no sign, no underscore, no leading zero and none of the alternative bases. Every refused form here denotes an integer, so the predicate reads the spelling: after a parser has resolved `0x1`, `1_0` and `010` there is nothing left to refuse, and a version whose value depends on which resolution a reader applies is not a version claim at all. |
//! | [`a_triple_has_exactly_three_members`] | interchange | The triple has exactly three members. Two and four are the pair the oracle asks for, and neither is a version the record can read: a version is a triple of major, minor and patch, and a document offering some other arity has not made a claim to be misread. |
//! | [`the_reserved_names_are_words_in_the_envelope_s_order`] | interchange | The two reserved names are spelled as words and stand in the order the envelope stands in. The order is a fact about the envelope rather than about any one carrier, and the carriers that cannot require it still read the same two names. |
//! | [`a_toml_envelope_stands_first_and_the_rest_is_free`] | interchange | A TOML document conforms when its two envelope keys are its first two top-level pairs, in order, preceded only by comments and blank lines. The rest of the document is free: every other top-level key, and everything beneath it, is content the policy does not inspect, and a nested key of a reserved name is untouched — which is exactly why a manifest can already carry a version and still carry no envelope. |
//! | [`an_absent_toml_envelope_key_is_named_for_its_own_absence`] | interchange | An absent envelope key is named for the key that is absent, and a key written after a table header is absent rather than misplaced. That is TOML's own rule doing the work: a bare key following the first header belongs to that table, so it is not a top-level key at all and the document has not named itself. |
//! | [`a_toml_label_has_one_spelling_and_the_others_are_quoted_back`] | interchange | A namespace is a basic double-quoted string, unescaped, whose content is a label. A literal single-quoted string carrying the same label fails and so does a basic string escaping a character it could have written directly: no character of the alphabet needs escaping and none is a quote, so the two forms encode identical bytes, and a carrier offering two spellings of one label would introduce an ambiguity the discipline does not have. The value is quoted back exactly as written. |
//! | [`a_toml_triple_is_three_canonical_integers_and_nothing_else`] | interchange | A version is an array of exactly three integers, each canonical in its spelling. Two members and four are refused for arity; a negative member and a hexadecimal one are refused for spelling, and both are integers TOML is happy to hand over — which is why the check reads the source text and never the parsed value. The value is quoted back as written. |
//! | [`the_toml_envelope_keys_stand_in_one_order`] | interchange | The order between the two keys is the part of the rule TOML does not supply, and it is what recovers bounded determination for this carrier: a reader knows what it is holding after a bounded prefix rather than after the whole document. A reversed pair and a third key interleaved between them both break it, and a document that meant the envelope and misplaced it is reported for the order rather than for an absence. |
//! | [`a_reserved_toml_name_that_opens_a_table_carries_content`] | interchange | A reserved name carries content when it opens a table, by a header or as the head of a dotted key, and that is the one thing the reservation rule forbids. A bare top-level key of either name is an envelope key however it is placed, so the two mistakes stay apart: a document that opened a table called `version` meant something else by the word, and a document that wrote the pair backwards did not. |
//! | [`bytes_that_are_no_document_fail_where_bytes_that_are_no_text_fail`] | interchange | Bytes that are no document of the carrier fail at the same door as bytes that are no text. All three formats are defined over Unicode, so a governed file must be decoded to be parsed at all, and this is the one place the surface's byte discipline meets formats whose definitions include an encoding. It is the rule meeting the formats rather than the rule relaxing. |
//! | [`a_json_envelope_may_stand_anywhere_in_the_root_object`] | interchange | A JSON envelope is two members of the root and may stand anywhere in it. Standing first cannot be required, because JSON defines an object as an unordered collection: a root member is a root member wherever its bytes fall, so an order would be a requirement on the serialization rather than on the document, and no tool that reads and rewrites a file could be held to it. The envelope standing last therefore passes, which is the case an implementation is likeliest to get wrong, because the TOML check next to it refuses exactly that shape. |
//! | [`a_json_root_that_is_no_object_can_hold_no_envelope`] | interchange | An envelope is two members of the document's root, so a document whose root is an array has nowhere to put one. The shape is not hypothetical — this tree holds such a file — and there is no third option: a first-party JSON document whose natural root is an array is reshaped or excluded. |
//! | [`a_duplicated_json_envelope_member_is_refused`] | interchange | A root object holding two members of one envelope name is refused. JSON leaves the reading of such a document to the parser, which is exactly the ambiguity this discipline does not have: a label with one byte form cannot be carried by a document whose reading depends on which duplicate a parser happens to keep. The reader therefore reads the source, because a parsed object has already resolved the question by discarding one of them. |
//! | [`a_json_label_and_triple_are_read_from_the_source`] | interchange | The label and the triple are read from the source in this carrier too. A label escaping a character it could have written directly is a second spelling of a label that has one byte form, and a parser would have unescaped it before the check saw it. A fractional member and an exponent-form one are numbers JSON is content with and no canonical non-negative integers. |
//! | [`a_yaml_envelope_may_stand_anywhere_in_the_root_mapping`] | interchange | A YAML envelope is two keys of the root mapping and may stand anywhere in it, for the reason JSON gives: mappings are unordered in the data model, so a root key is a root key wherever it is written. A repository may put the envelope at the head of every file as a house convention, and the policy does not check it. The envelope standing last therefore passes, which with its JSON twin is the case an implementation is likeliest to get wrong, because the TOML check refuses exactly that shape. |
//! | [`a_yaml_envelope_wants_one_document_and_a_root_mapping`] | interchange | The envelope presupposes a document and a root mapping to hold it. A root that is a sequence has nowhere to put two keys, and a stream holding no document or more than one leaves the check no root to read — which is the one place it looks above the root mapping, and it looks there because it must choose which root it reads. |
//! | [`a_yaml_envelope_key_is_written_where_it_is_read`] | interchange | The two envelope keys must be written where they are read. A value arriving through an alias, a root mapping assembled through a merge key, and either key carrying an explicit tag are all refused, on the ground the one-spelling rule gives: a resolved alias is not a spelling of a value but a second place the value lives. The scope is deliberately narrow, and anchors and aliases below the root mapping are content the policy does not inspect. |
//! | [`a_yaml_label_is_plain_and_a_yaml_triple_is_in_flow_form`] | interchange | YAML inverts the other two carriers on the label and constrains the triple more tightly. The label is a plain scalar and the quoted forms fail, because every label the grammar admits is representable plainly and the plain scalar is the one form always available and always unambiguous here. The triple is required in flow form for one spelling, and the leading-zero and underscore prohibitions matter more here than anywhere else: a parser applying the older integer resolution reads a leading-zero scalar as octal and admits underscores as separators, so such a member is a number whose value depends on which reader has it, and that is no version claim. |
//! | [`the_program_reads_the_type_from_the_name_and_never_sniffs`] | interchange | The program reads the file type from the name and never sniffs a file to guess what it is. A type it knows is parsed as that carrier; a type it does not know is ignored, with no verdict, no finding and no burn row. The remedy for an ungovernable file inside an include row is still the exclude row somebody should have written, and what the one-family ruling changed is that the linter no longer announces the omission. |
//! | [`a_declared_file_with_a_defective_envelope_refuses_the_command`] | interchange | A declared file whose envelope is defective refuses the command, in the class a lexical error already occupies. The check is prior to the declaration that would authorize it, and that is exactly right: the requirement holds of the declared files because they are the configuration, not because the configuration says so. One consequence is a guarantee rather than an accident: no declared file can ever appear in a burn row, because debt requires a tolerated failing run and a declared file with a bad envelope produces no run at all. |
//! | [`each_defect_carries_the_finding_text_its_carrier_fixes`] | interchange | Each defect carries the finding text the policy fixes for it, and the carrier decides two things about that text: the word for a root entry, which is *member* in JSON and *key* in the other two, and the format's own name in the parse form. The parse form says *not a well-formed X document* rather than *not an X document*, because under one family the two readings came apart — a file the program cannot identify is ignored and says nothing, while a file of a known type that will not parse fails and says this. |
//! | [`a_section_excludes_then_partitions_what_survives`] | interchange | The section removes the union of its exclude rows, and what survives is the governed set. Where the section also declares an include gloss, that gloss is checked against the set exclusion already computed: a governed path no gloss row names leaves it incomplete, one two rows name leaves it overlapping, and a row reaching nothing in the governed set is idle. None of the three moves a path — the file is governed either way, because exclusion alone decided that. The two passes stay separate, so an overlap between an exclude row and a gloss row is legal and is how a named foreign-schema row sits over a broad gloss row without either being narrowed to accommodate the other. Containment is not among the failures because a row is only ever offered its own owner's paths: a row written for another owner's file reaches nothing and is idle. |
//! | [`a_link_carries_no_document_and_an_unknown_type_is_ignored`] | interchange | A symbolic link is never a document carrier, so a section leaving one in its governed set is named at configuration time and the remedy is an exclude row. A file of a type the program does not know never enters the universe at all: it is out of domain, so no row reaches it, no verdict is formed and no burn row can name it. A repository cannot ask for the loud version by writing a narrow gloss row, because the file the row would leave unnamed is not in the set the gloss is judged over. |
//! | [`the_include_partition_is_a_gloss_and_never_governs`] | interchange | Exclusion alone computes governance, and a declared include list is a diagnostic gloss on the set it computed rather than an instrument that forms it. A section declaring no gloss owes nothing and governs the whole of what its exclusions leave; a section declaring a correct one governs exactly the same paths and adds a name to each. So the governed set is identical with the gloss and without it, which is the sense in which the include rows were doing nothing. A false gloss — incomplete, overlapping, or padded with a row reaching only excluded or out-of-domain files — is a finding against the declaration, and the governed set does not move. |
//! | [`the_universe_is_the_catalog_s_domain_and_a_row_outside_it_is_idle`] | interchange | The universe is pre-filtered by the carrier catalog, and every coverage judgment is taken over what survives that filter. An out-of-domain path is governed by nothing, excluded by nothing and reported as nothing, whatever rows stand; an in-domain path a declared gloss does not name is governed and the gloss is what fails; and a row whose pattern can only reach out-of-domain paths reaches nothing at all and is idle. The three cases are one boundary read from both sides, which is why they are proved together. |
//! | [`a_governed_document_is_judged_in_the_carrier_its_name_names`] | interchange | A governed document is judged in the carrier its name names, and every carrier's verdict reaches one report. A path a list tolerates is silent however it fails, and the violation identity is the path: a file failing three ways is one debt and not three, which is what the path-set codec says. |

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::declaration::AbnfPattern;
use crate::finding::Finding;
use crate::pattern::BytePath;
use crate::selection::{
    GlossDefect, GlossSection, List as SelectionList, Rule as SelectionRule, diagnostic_gloss,
};

/// The reserved top-level key holding the namespace label.
const NAMESPACE_KEY: &str = "namespace";

/// The reserved top-level key holding the version triple.
const VERSION_KEY: &str = "version";

/// The two reserved top-level names, in the order the envelope stands in.
const RESERVED_KEYS: [&str; 2] = [NAMESPACE_KEY, VERSION_KEY];

/// The greatest number of bytes a namespace label may occupy.
///
/// Every character of the label alphabet is printable ASCII, on which UTF-8 acts
/// as the identity, so the byte bound and the character bound coincide and the
/// record states them as one sentence.
const LABEL_BOUND: usize = 255;

/// The number of members a version triple has, which is not negotiable by carrier.
const TRIPLE_MEMBERS: usize = 3;

/// Whether this byte is an atom's alphabet: a lower-case Latin letter or a decimal digit.
const fn is_alnum(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

/// Whether this text is an atom: a nonempty word over the alphabet and the
/// hyphen whose first and last characters lie in the alphabet.
///
/// A single character of the alphabet is an atom, and hyphens occur only in the
/// interior. The grammar's ABNF admits a run of them there, so `a--b` is an
/// atom; the prose sentence and the ABNF agree, and the ABNF is normative for
/// shape where the two could be read to differ.
fn is_atom(text: &str) -> bool {
    let bytes = text.as_bytes();

    match (bytes.first(), bytes.last()) {
        (Some(&first), Some(&last)) => {
            is_alnum(first)
                && is_alnum(last)
                && bytes.iter().all(|&byte| is_alnum(byte) || byte == b'-')
        }
        _ => false,
    }
}

/// Whether this text is a namespace label under the record's closed grammar.
///
/// A label is two or more atoms separated by dots, occupying at most the byte
/// bound. The dot is a separator and is no character of any atom, so a leading,
/// trailing or doubled dot yields an empty atom and fails. The requirement of at
/// least two atoms places every label strictly below the root of the tree the
/// labels descend: no one claims a bare top-level word.
fn is_namespace_label(text: &str) -> bool {
    if text.len() > LABEL_BOUND {
        return false;
    }

    let mut atoms = text.split('.');
    let heads = atoms.next().is_some_and(is_atom) && atoms.next().is_some_and(is_atom);

    heads && atoms.all(is_atom)
}

/// Whether this spelling is a canonical non-negative integer of a version triple.
///
/// Plain decimal, no sign, no underscores, no leading zero, and none of the
/// hexadecimal, octal or binary forms two of the three carriers admit. Zero
/// itself is written `0` and is the one spelling a leading-zero rule must not
/// reach. The predicate is deliberately over the spelling and never over a
/// parsed value: the alternative forms all denote integers, and refusing them
/// after the parser has erased the difference is not possible.
fn is_canonical_integer(spelling: &str) -> bool {
    let bytes = spelling.as_bytes();

    match bytes {
        [b'0'] => true,
        [] | [b'0', ..] => false,
        _ => bytes.iter().all(u8::is_ascii_digit),
    }
}

/// Whether these three spellings are a canonical version triple.
///
/// The carriers narrow the representable range and the record does not: the data
/// language's unsigned integers run to two to the sixty-fourth, TOML's integers
/// are signed sixty-four-bit and JSON's interoperable range is what a double
/// represents exactly. Those are narrowings of the carriers rather than of the
/// theory, and no version this or any repository writes approaches any of them,
/// so the predicate holds the spelling to canonicity and leaves the magnitude to
/// the carrier that has to hold it.
fn is_canonical_triple(spellings: &[String]) -> bool {
    spellings.len() == TRIPLE_MEMBERS
        && spellings
            .iter()
            .all(|spelling| is_canonical_integer(spelling))
}

/// What a governed document's envelope is wrong about.
///
/// A document is conforming when this list is empty. The variants are the
/// finding forms of the envelope decision (´dec:envelope:source-shape´), one to
/// one, and the carrier the document was read as decides which of them can
/// arise: the wrong-order defect belongs to TOML alone, because neither of the
/// other carriers requires an order and a rule that is not made has no finding.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Defect {
    /// No namespace key stands at the document's root.
    NoNamespace,
    /// No version key stands at the document's root.
    NoVersion,
    /// A namespace whose value is no label, quoted back as it was written.
    NotLabel(String),
    /// A version whose value is no canonical triple, quoted back as written.
    NotTriple(String),
    /// A reserved name standing at the root and carrying content.
    Reserved(&'static str),
    /// The two envelope keys do not stand first, in order, in a TOML document.
    WrongOrder,
    /// The bytes are no well-formed document of the carrier they were read as.
    NotWellFormed,
    /// A JSON document whose root is not an object, so it can hold no envelope.
    RootNotObject,
    /// A root member of a JSON object standing more than once.
    Duplicated(&'static str),
    /// A YAML document whose root is not a mapping.
    RootNotMapping,
    /// A YAML stream holding some number of documents other than one.
    Documents(usize),
    /// A YAML envelope key written through an alias, a merge or an explicit tag.
    Indirect(&'static str),
}

/// One key-value pair standing at a TOML document's top level, as written.
struct Pair {
    /// The first segment of the dotted key path, which is the top-level name.
    head: String,
    /// Whether the path had exactly one segment, so the name is the whole key.
    whole: bool,
    /// The value's source text, trimmed of surrounding space and any comment.
    value: String,
}

/// What the prefix scan of a TOML document found.
struct Scan {
    /// The top-level pairs standing before the first table header, in order.
    pairs: Vec<Pair>,
    /// The first segment of every table header the document opens.
    headers: Vec<String>,
}

/// The index just past this line's final byte, the newline excluded.
const fn line_end(bytes: &[u8], from: usize) -> usize {
    let mut at = from;

    while at < bytes.len() && bytes[at] != b'\n' {
        at += 1;
    }

    at
}

/// Whether this byte may stand in a TOML bare key.
const fn is_bare(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Read one key segment, bare or quoted, and hand back its content.
///
/// The quotes are stripped and no escape is interpreted, which is enough: a
/// reserved name is a word of the bare-key alphabet, so a quoted spelling of one
/// is the same name written the long way and is caught by the same comparison.
fn read_segment(bytes: &[u8], at: &mut usize) -> Option<String> {
    let &quote @ (b'"' | b'\'') = bytes.get(*at)? else {
        let from = *at;

        while *at < bytes.len() && is_bare(bytes[*at]) {
            *at += 1;
        }

        return (from < *at).then(|| String::from_utf8_lossy(&bytes[from..*at]).into_owned());
    };

    *at += 1;
    let from = *at;

    while *at < bytes.len() && bytes[*at] != quote {
        // A basic string escapes with a backslash; a literal string does not.
        *at += if quote == b'"' && bytes[*at] == b'\\' {
            2
        } else {
            1
        };
    }

    let content = String::from_utf8_lossy(&bytes[from..(*at).min(bytes.len())]).into_owned();
    *at += 1;

    Some(content)
}

/// Read a dotted key path, stopping at the equals sign that ends it.
///
/// Only the head segment and the path's length are wanted. The head is what the
/// reservation rule compares, because a dotted key opens a table under its first
/// segment and a reserved name opening a table is that name carrying content.
fn read_key(bytes: &[u8], at: &mut usize) -> Option<(String, bool)> {
    let head = read_segment(bytes, at)?;
    let mut whole = true;

    loop {
        while matches!(bytes.get(*at), Some(b' ' | b'\t')) {
            *at += 1;
        }

        match bytes.get(*at) {
            Some(b'.') => {
                *at += 1;
                whole = false;

                while matches!(bytes.get(*at), Some(b' ' | b'\t')) {
                    *at += 1;
                }

                read_segment(bytes, at)?;
            }
            Some(b'=') => {
                *at += 1;
                return Some((head, whole));
            }
            _ => return None,
        }
    }
}

/// Read a value's source text, which may run over several lines.
///
/// The scan tracks the string and bracket state it must in order to find where
/// the value ends, and nothing else: the document has already been parsed for
/// well-formedness, so this reader may trust the shape and look only for the
/// end. A comment outside a string runs to the end of its line at any depth.
fn read_value(bytes: &[u8], at: &mut usize) -> String {
    while matches!(bytes.get(*at), Some(b' ' | b'\t')) {
        *at += 1;
    }

    let from = *at;
    let mut depth = 0_usize;

    while *at < bytes.len() {
        match bytes[*at] {
            b'\n' if depth == 0 => break,
            b'#' => {
                let end = line_end(bytes, *at);

                if depth == 0 {
                    let text = String::from_utf8_lossy(&bytes[from..*at]).into_owned();
                    *at = end;
                    return String::from(text.trim());
                }

                *at = end;
            }
            b'[' | b'{' => {
                depth += 1;
                *at += 1;
            }
            b']' | b'}' => {
                depth = depth.saturating_sub(1);
                *at += 1;
            }
            byte @ (b'"' | b'\'') => skip_string(bytes, at, byte),
            _ => *at += 1,
        }
    }

    String::from(String::from_utf8_lossy(&bytes[from..*at]).trim())
}

/// Advance past a string literal of either quote, single-line or multi-line.
fn skip_string(bytes: &[u8], at: &mut usize, quote: u8) {
    let long = bytes.get(*at + 1) == Some(&quote) && bytes.get(*at + 2) == Some(&quote);
    let fence = if long { 3 } else { 1 };
    *at += fence;

    while *at < bytes.len() {
        if quote == b'"' && bytes[*at] == b'\\' {
            *at += 2;
            continue;
        }

        if bytes[*at] == quote
            && (!long || (bytes.get(*at + 1) == Some(&quote) && bytes.get(*at + 2) == Some(&quote)))
        {
            *at += fence;
            return;
        }

        if !long && bytes[*at] == b'\n' {
            return;
        }

        *at += 1;
    }
}

/// Read a TOML document's top-level prefix and every table header it opens.
fn scan_toml(text: &str) -> Scan {
    let bytes = text.as_bytes();
    let mut scan = Scan {
        pairs: Vec::new(),
        headers: Vec::new(),
    };
    let mut at = 0;
    let mut headed = false;

    while at < bytes.len() {
        match bytes[at] {
            b' ' | b'\t' | b'\r' | b'\n' => at += 1,
            b'#' => at = line_end(bytes, at),
            b'[' => {
                headed = true;
                at += if bytes.get(at + 1) == Some(&b'[') {
                    2
                } else {
                    1
                };

                while matches!(bytes.get(at), Some(b' ' | b'\t')) {
                    at += 1;
                }

                if let Some(head) = read_segment(bytes, &mut at) {
                    scan.headers.push(head);
                }

                at = line_end(bytes, at);
            }
            _ => {
                let Some((head, whole)) = read_key(bytes, &mut at) else {
                    break;
                };
                let value = read_value(bytes, &mut at);

                if headed {
                    continue;
                }

                scan.pairs.push(Pair { head, whole, value });
            }
        }
    }

    scan
}

/// Split a flow-form array's source text into its members' spellings.
///
/// The members of a triple are integers, so the split needs no nesting: a shape
/// that would need one is not a triple and fails on the member instead. An empty
/// array yields no members, and a trailing comma yields an empty final member,
/// which is the refusal a non-canonical spelling earns everywhere else here.
fn flow_members(raw: &str) -> Option<Vec<String>> {
    let inner = raw.strip_prefix('[')?.strip_suffix(']')?;

    if inner.trim().is_empty() {
        return Some(Vec::new());
    }

    Some(
        inner
            .split(',')
            .map(|member| String::from(member.trim()))
            .collect(),
    )
}

/// Whether this quoted value is the canonical spelling of a namespace label.
///
/// The double-quoted form, unescaped, is the one spelling, and the rule serves
/// TOML and JSON alike. A TOML literal single-quoted string carrying the same
/// label fails, and so in either carrier does a string escaping a character it
/// could have written directly: no character of the alphabet needs escaping and
/// none is a quote, so the two forms encode identical bytes and offering both
/// would introduce an ambiguity the discipline does not have. YAML inverts this
/// and is read by its own rule.
fn quoted_label(raw: &str) -> bool {
    raw.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .is_some_and(|label| !label.contains('\\') && is_namespace_label(label))
}

/// Read the envelope of a governed TOML document and name what is wrong with it.
///
/// The envelope is the document's first two top-level pairs, in the order the
/// record fixes, preceded only by comments and blank lines. Most of that rule is
/// TOML's own — a bare key written after the first table header belongs to that
/// table, so a top-level key must precede every header or it is not top-level at
/// all — and what the rule adds is the order between the two and the prohibition
/// on interleaving a third key between them. It adds that much because it
/// recovers bounded determination for this carrier: a reader knows what it is
/// holding after a bounded prefix, rather than after the whole document.
fn toml_defects(bytes: &[u8]) -> Vec<Defect> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![Defect::NotWellFormed];
    };

    if toml::from_str::<toml::Table>(text).is_err() {
        return vec![Defect::NotWellFormed];
    }

    parsed_toml_defects(text)
}

/// Judge the envelope bytes of a TOML document whose syntax already parsed.
fn parsed_toml_defects(text: &str) -> Vec<Defect> {
    let scan = scan_toml(text);
    let mut defects = Vec::new();

    // A reserved name carries content when it opens a table, whether by a header
    // or as the head of a dotted key. A bare top-level key of that name is an
    // envelope key however it is placed, so a misplaced one is an order defect
    // and never a reservation defect — the document meant the envelope and wrote
    // it in the wrong place, which is a different mistake and reads as one.
    let opens_table = |name: &str| {
        scan.headers.iter().any(|head| head == name)
            || scan
                .pairs
                .iter()
                .any(|pair| pair.head == name && !pair.whole)
    };

    // Where each envelope key stands among the top-level pairs, if it stands at
    // all. An absent key is an absence and never a misplacement: a document
    // carrying one key of the pair has not written the envelope in the wrong
    // order, it has failed to write the envelope.
    let place_of = |name: &str| {
        scan.pairs
            .iter()
            .position(|pair| pair.head == name && pair.whole)
    };

    for (place, &name) in RESERVED_KEYS.iter().enumerate() {
        if opens_table(name) {
            defects.push(Defect::Reserved(name));
        } else if place_of(name).is_none() {
            defects.push(if place == 0 {
                Defect::NoNamespace
            } else {
                Defect::NoVersion
            });
        }
    }

    if !defects.is_empty() {
        return defects;
    }

    // Both keys stand at the top level, so the order is the next question, and
    // anything other than first and second in order is one defect rather than
    // one per key: the pair is misplaced, not each half of it separately.
    if place_of(NAMESPACE_KEY) != Some(0) || place_of(VERSION_KEY) != Some(1) {
        return vec![Defect::WrongOrder];
    }

    let namespace = &scan.pairs[0].value;
    let version = &scan.pairs[1].value;

    if !quoted_label(namespace) {
        defects.push(Defect::NotLabel(namespace.clone()));
    }

    if !flow_members(version).is_some_and(|members| is_canonical_triple(&members)) {
        defects.push(Defect::NotTriple(version.clone()));
    }

    defects
}

/// Advance past any run of JSON whitespace.
fn skip_space(bytes: &[u8], at: &mut usize) {
    while matches!(bytes.get(*at), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        *at += 1;
    }
}

/// Advance past a JSON string, the opening quote included.
const fn skip_json_string(bytes: &[u8], at: &mut usize) {
    *at += 1;

    while *at < bytes.len() {
        match bytes[*at] {
            b'\\' => *at += 2,
            b'"' => {
                *at += 1;
                return;
            }
            _ => *at += 1,
        }
    }
}

/// Advance past one JSON value of any shape.
fn skip_json_value(bytes: &[u8], at: &mut usize) {
    match bytes.get(*at) {
        Some(b'"') => skip_json_string(bytes, at),
        Some(b'{' | b'[') => {
            let mut depth = 0_usize;

            while *at < bytes.len() {
                match bytes[*at] {
                    b'"' => {
                        skip_json_string(bytes, at);
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);

                        if depth == 0 {
                            *at += 1;
                            return;
                        }
                    }
                    _ => {}
                }

                *at += 1;
            }
        }
        _ => {
            while *at < bytes.len()
                && !matches!(
                    bytes[*at],
                    b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n'
                )
            {
                *at += 1;
            }
        }
    }
}

/// Read a JSON root object's members, each name and value as written.
///
/// The scan is over the source rather than over a parsed object for two reasons
/// the parsed object cannot serve. A parser resolves a duplicated member by
/// keeping one of them, which is exactly the ambiguity this discipline does not
/// have; and it unescapes a name and a value, which erases the difference
/// between a label written directly and one written through an escape.
fn json_members(text: &str) -> Vec<(String, String)> {
    let bytes = text.as_bytes();
    let mut members = Vec::new();
    let mut at = 0;

    skip_space(bytes, &mut at);

    if bytes.get(at) != Some(&b'{') {
        return members;
    }

    at += 1;

    loop {
        skip_space(bytes, &mut at);

        if bytes.get(at) != Some(&b'"') {
            return members;
        }

        let opened = at;
        skip_json_string(bytes, &mut at);
        let name = String::from_utf8_lossy(&bytes[opened..at]).into_owned();

        skip_space(bytes, &mut at);

        if bytes.get(at) != Some(&b':') {
            return members;
        }

        at += 1;
        skip_space(bytes, &mut at);

        let opened = at;
        skip_json_value(bytes, &mut at);
        members.push((
            name,
            String::from_utf8_lossy(&bytes[opened..at]).into_owned(),
        ));

        skip_space(bytes, &mut at);

        if bytes.get(at) == Some(&b',') {
            at += 1;
        } else {
            return members;
        }
    }
}

/// Read the envelope of a governed JSON document and name what is wrong with it.
///
/// The envelope is two members of the document's root, and standing first cannot
/// be required. JSON defines an object as an unordered collection, so a root
/// member is a root member wherever its bytes fall, and requiring an order would
/// require something of the serialization rather than of the document — a
/// requirement no producer that rewrites a file could be held to. So the check
/// is presence and shape, and bounded determination is not recovered here: a
/// reader must consume the whole root object before it knows the envelope is
/// there. That is the property the TOML rule exists to buy, this carrier cannot
/// sell it, and saying so is more useful than a rule that would appear to.
fn json_defects(bytes: &[u8]) -> Vec<Defect> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![Defect::NotWellFormed];
    };

    let Ok(document) = serde_json::from_str::<serde_json::Value>(text) else {
        return vec![Defect::NotWellFormed];
    };

    // An envelope is two members of the root, so a document whose root is an
    // array has nowhere to put one. There is no third option: such a document is
    // reshaped or excluded.
    if !document.is_object() {
        return vec![Defect::RootNotObject];
    }

    let members = json_members(text);
    let mut defects = Vec::new();

    for (place, &name) in RESERVED_KEYS.iter().enumerate() {
        let quoted = format!("\"{name}\"");
        let standing: Vec<&str> = members
            .iter()
            .filter(|(member, _)| *member == quoted)
            .map(|(_, value)| value.as_str())
            .collect();

        match standing.as_slice() {
            [] => defects.push(if place == 0 {
                Defect::NoNamespace
            } else {
                Defect::NoVersion
            }),
            [value] if name == NAMESPACE_KEY && !quoted_label(value) => {
                defects.push(Defect::NotLabel(String::from(*value)));
            }
            [value]
                if name == VERSION_KEY
                    && !flow_members(value).is_some_and(|read| is_canonical_triple(&read)) =>
            {
                defects.push(Defect::NotTriple(String::from(*value)));
            }
            [_] => {}
            _ => defects.push(Defect::Duplicated(name)),
        }
    }

    defects
}

/// Whether this YAML line carries content rather than blank space, a comment or
/// a directive.
fn significant(line: &str) -> bool {
    let trimmed = line.trim();

    !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with('%')
}

/// Split a YAML stream into the documents it holds.
///
/// This is the one place the check looks above the root mapping, and it looks
/// there because it must choose which root it reads: *the document's envelope*
/// presupposes a document, so a stream holding none, or more than one, fails.
fn yaml_documents(text: &str) -> Vec<Vec<&str>> {
    let mut documents: Vec<Vec<&str>> = vec![Vec::new()];

    for line in text.lines() {
        if line == "---" || line.starts_with("--- ") {
            documents.push(Vec::new());

            if let (Some(rest), Some(opened)) = (line.strip_prefix("--- "), documents.last_mut()) {
                opened.push(rest);
            }

            continue;
        }

        if line == "..." {
            continue;
        }

        if let Some(opened) = documents.last_mut() {
            opened.push(line);
        }
    }

    documents.retain(|lines| lines.iter().any(|line| significant(line)));
    documents
}

/// The offset of the colon that ends a YAML block mapping key, if the line holds
/// one: a colon standing at the end of the line or followed by a space.
fn key_colon(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();

    (0..bytes.len()).find(|&at| bytes[at] == b':' && matches!(bytes.get(at + 1), None | Some(b' ')))
}

/// Read a YAML document's root mapping keys and the values as they are written.
///
/// A key of the root mapping stands at column zero, which is what makes it a
/// root key of a block mapping. A key whose value is empty carries it in the
/// more-indented lines that follow, and those are gathered so that a block-form
/// value can be quoted back rather than reported as nothing at all.
fn yaml_root_pairs(lines: &[&str]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if !significant(line) || line.starts_with([' ', '\t']) {
            continue;
        }

        let Some(colon) = key_colon(line) else {
            continue;
        };

        let key = String::from(line[..colon].trim());
        let mut value = String::from(line[colon + 1..].trim());

        if value.is_empty() {
            let mut block = Vec::new();

            for follow in lines.iter().skip(index + 1) {
                if !significant(follow) {
                    continue;
                }

                if !follow.starts_with([' ', '\t']) {
                    break;
                }

                block.push(follow.trim());
            }

            value = block.join(" ");
        }

        pairs.push((key, value));
    }

    pairs
}

/// Read the envelope of a governed YAML document and name what is wrong with it.
///
/// The label is a plain scalar and the quoted forms fail, which inverts TOML and
/// differs again from JSON. The inversion belongs to the carriers and not to the
/// discipline: every label the grammar admits is representable plainly, and a
/// label of at least two dot-separated atoms is resolved as a number, a boolean
/// or a null by no schema a parser applies, so the plain scalar is the one form
/// always available and always unambiguous here.
///
/// The two envelope keys must be written where they are read: a value arriving
/// through an alias, a root mapping assembled through a merge key, or either key
/// carrying an explicit tag is refused, on the one-spelling ground that a
/// resolved alias is not a spelling of a value but a second place it lives.
/// The scope is deliberately narrow — anchors and aliases below the root mapping
/// are content and are not the policy's business.
fn yaml_defects(bytes: &[u8]) -> Vec<Defect> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![Defect::NotWellFormed];
    };

    let documents = yaml_documents(text);

    let [lines] = documents.as_slice() else {
        return vec![Defect::Documents(documents.len())];
    };

    let Some(opening) = lines
        .iter()
        .find(|line| significant(line))
        .map(|line| line.trim_start())
    else {
        return vec![Defect::RootNotMapping];
    };

    if opening == "-" || opening.starts_with("- ") || opening.starts_with('[') {
        return vec![Defect::RootNotMapping];
    }

    let pairs = yaml_root_pairs(lines);

    if pairs.is_empty() {
        return vec![Defect::RootNotMapping];
    }

    let merged = pairs.iter().any(|(key, _)| key == "<<");
    let mut defects = Vec::new();

    for (place, &name) in RESERVED_KEYS.iter().enumerate() {
        let standing = pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str());

        match standing {
            // A root mapping assembled through a merge key is where the absent
            // key would have come from, and that is a refusal rather than an
            // absence: the key exists and is not written where it is read.
            None if merged => defects.push(Defect::Indirect(name)),
            None => defects.push(if place == 0 {
                Defect::NoNamespace
            } else {
                Defect::NoVersion
            }),
            Some(value) if value.starts_with('*') || value.starts_with('!') => {
                defects.push(Defect::Indirect(name));
            }
            Some(value) if name == NAMESPACE_KEY && !is_namespace_label(value) => {
                defects.push(Defect::NotLabel(String::from(value)));
            }
            Some(value)
                if name == VERSION_KEY
                    && !flow_members(value).is_some_and(|read| is_canonical_triple(&read)) =>
            {
                defects.push(Defect::NotTriple(String::from(value)));
            }
            Some(_) => {}
        }
    }

    defects
}

/// A carrier this policy program can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carrier {
    /// A TOML document, whose envelope stands first.
    Toml,
    /// A JSON document, whose envelope is two members of an object root.
    Json,
    /// A YAML document, whose envelope is two keys of a mapping root.
    Yaml,
}

/// The file types this program can read, by the suffix of the final component.
///
/// This is a catalog in the sense the comment-leader table is one, and the
/// design says so rather than keeping the draft's boast that a single-format
/// policy needs none. Two differences are why it is cheap here and was expensive
/// there. It is code-owned, so no repository extends it by writing a row: a
/// carrier is added by the linter learning to read it. And it carries no
/// per-type shape rule beyond the three transpositions already argued, where a
/// comment-leader row has to name a leader per type.
///
/// What survives of the draft's claim is the part about behaviour: the linter
/// never sniffs a file to guess what it is. It reads the type from the name,
/// finds it here or does not, and either parses one way or ignores the file.
const CARRIERS: &[(&str, Carrier)] = &[
    (".toml", Carrier::Toml),
    (".json", Carrier::Json),
    (".yaml", Carrier::Yaml),
    (".yml", Carrier::Yaml),
];

/// The final path component, which is what the catalog is keyed by.
fn final_component(bytes: &[u8]) -> &[u8] {
    bytes
        .iter()
        .rposition(|&byte| byte == b'/')
        .map_or(bytes, |at| &bytes[at + 1..])
}

/// The carrier this path names, or none when the program does not know the type.
///
/// A file of an unknown type inside an include row is ignored: no verdict, no
/// finding, no burn row. The remedy is still the exclude row somebody should
/// have written, and what the naming ruling changed is that the linter no longer
/// announces the omission. Ignoring is by type and never by parse outcome, so a
/// file of a known type that will not parse is judged and fails.
fn carrier(path: &BytePath) -> Option<Carrier> {
    let component = final_component(path.as_bytes());

    CARRIERS
        .iter()
        .find(|(suffix, _)| {
            component.len() > suffix.len() && component.ends_with(suffix.as_bytes())
        })
        .map(|(_, carrier)| *carrier)
}

/// Read a governed document's envelope in the carrier its name names.
fn defects(carrier: Carrier, bytes: &[u8]) -> Vec<Defect> {
    match carrier {
        Carrier::Toml => toml_defects(bytes),
        Carrier::Json => json_defects(bytes),
        Carrier::Yaml => yaml_defects(bytes),
    }
}

/// The text a declared file with a defective envelope refuses the command with.
fn declared_refusal(display: &str) -> String {
    format!(
        "declared configuration: {display}: no envelope; a declared file carries the envelope it requires"
    )
}

/// The refusal texts the declared directory earns, one per file whose envelope
/// is defective, in the order the files are given.
///
/// The envelope of a declared file is read before that file's content, and a
/// defect refuses: a file that does not identify what it is, is not a snapshot
/// the command can judge. A non-canonical input is not a defective document to
/// be repaired but no document at all, with nothing for satisfaction to hold of,
/// so this belongs in the class a lexical error already occupies — exit one,
/// empty standard output, no report, no policy run, no writer.
///
/// The check is prior to the declaration that would authorize it, and that is
/// exactly right. The include row governing the declared directory is written in
/// a declared file, which circles only if this is read as a policy the loader
/// consults. It is not. The requirement is unconditional and constitutional: it
/// holds of the declared files because they are the configuration, not because
/// the configuration says so, and it is checked before any activation is known.
///
/// One consequence is a guarantee rather than an accident: no declared file can
/// ever appear in a burn row, because debt requires a tolerated failing run and
/// a declared file with a bad envelope produces no run at all.
#[cfg(test)]
fn declared_refusals(directory: &Path, files: &[&str]) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            std::fs::read(directory.join(file)).is_ok_and(|bytes| !toml_defects(&bytes).is_empty())
        })
        .map(|file| declared_refusal(file))
        .collect()
}

/// The declaration refusal for one already-parsed TOML member, when defective.
pub fn declared_refusal_from_parsed(file: &str, text: &str) -> Option<String> {
    (!parsed_toml_defects(text).is_empty()).then(|| declared_refusal(file))
}

/// The word a carrier calls a root entry by.
///
/// The forms common to all three carriers transpose unchanged, reading *member*
/// for *key* in JSON, because that is what JSON calls the thing.
const fn entry_word(carrier: Carrier) -> &'static str {
    match carrier {
        Carrier::Json => "member",
        Carrier::Toml | Carrier::Yaml => "key",
    }
}

/// The carrier's own name, as the parse finding spells it.
const fn carrier_name(carrier: Carrier) -> &'static str {
    match carrier {
        Carrier::Toml => "TOML",
        Carrier::Json => "JSON",
        Carrier::Yaml => "YAML",
    }
}

/// The finding text one defect earns in one carrier.
///
/// The three parse forms say *not a well-formed X document* rather than *not an
/// X document*, because under one family the two readings came apart: a file the
/// program cannot identify is ignored and reports nothing, while a file of a
/// known type that will not parse fails and reports this. The older wording
/// named the condition the naming ruling withdrew; this one names what remains.
///
/// The wrong-order form belongs to TOML alone, and that absence is the one place
/// a reviewer might suspect a gap in the message set. It is not one: neither of
/// the other carriers requires an order, and a rule that is not made has no
/// finding.
fn message(defect: &Defect, carrier: Carrier) -> String {
    let entry = entry_word(carrier);

    match defect {
        Defect::NoNamespace => format!("no namespace {entry}; a governed document names itself"),
        Defect::NoVersion => format!("no version {entry}; a governed document stamps its schema"),
        Defect::NotLabel(found) => format!("namespace {found} is not a namespace label"),
        Defect::NotTriple(found) => {
            format!("version {found} is not a triple of canonical non-negative integers")
        }
        Defect::Reserved(name) => {
            format!("{name} is reserved for the envelope and carries content")
        }
        Defect::WrongOrder => String::from("the envelope keys stand in the wrong order"),
        Defect::NotWellFormed => format!("not a well-formed {} document", carrier_name(carrier)),
        Defect::RootNotObject => String::from("the document's root is not an object"),
        Defect::Duplicated(name) => format!("{name} appears more than once in the root object"),
        Defect::RootNotMapping => String::from("the document's root is not a mapping"),
        Defect::Documents(count) => {
            format!("the stream holds {count} documents; an envelope names one")
        }
        Defect::Indirect(name) => {
            format!("the envelope key {name} is written through an alias, a merge or a tag")
        }
    }
}

/// Which of an owner's two lists a row stands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ListKind {
    /// The rows that remove paths from the owner's share.
    Exclude,
    /// The rows that partition what survives.
    Include,
}

impl ListKind {
    /// The word a finding names this list by.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Exclude => "exclude",
            Self::Include => "include",
        }
    }
}

/// One row of one list: a name and the pattern it reaches paths by.
///
/// A row's name is either a reference to something declared elsewhere or a mint
/// of the rule itself, and which one it is, is fixed by the list and never by the
/// row. An include row here has nothing to refer to, because the requirement
/// names no text the repository chose, so it mints: it answers *why is this file
/// governed* rather than *governed to carry what*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionRow {
    /// The row's name, minted where it stands.
    pub(super) name: String,
    /// The pattern it reaches paths by.
    pub(super) pattern: AbnfPattern,
}

impl std::fmt::Display for SectionRow {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} : {}", self.name, self.pattern.source())
    }
}

/// One owner's section: one pair of lists, and no halves.
///
/// The SPDX file gives each owner two independently shaped halves because a
/// licence identifier and a copyright line are two requirements that can come
/// apart. The envelope does not come apart. A document of the data language is a
/// map in which key 0 is present *and* key 1 is present; there is no document
/// carrying a namespace and no version, and none carrying a version and no
/// namespace. A file governed for its namespace and excused from its version
/// would not be a partially conforming document but a category the record does
/// not have.
///
/// The exclusion list computes the governed set on its own: the governed set is
/// the owner's in-domain share minus the union of its exclusions, and nothing
/// else selects it. The inclusion list is optional and, where it stands, is a
/// diagnostic gloss on that computed set rather than an instrument that forms it
/// (´dec:envelope:computed-governance´). An absent gloss owes nothing; a present
/// one is held to the ordinary partition judgment over the governed set
/// (´dec:rows:subtract-then-partition´), and a false gloss fails configuration
/// without moving one path into or out of governance.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// The rows that remove paths from the owner's share.
    pub(super) exclude: Vec<SectionRow>,
    /// The optional gloss: a declared partition of the computed governed set.
    pub(super) include: Option<Vec<SectionRow>>,
}

impl Section {
    /// The declared gloss rows, which are none where no gloss is declared.
    ///
    /// Absent and empty are deliberately not the same thing, and a caller that
    /// cares asks [`Section::glossed`] which it has: an absent gloss owes
    /// nothing, while an empty gloss is the claim that the governed set is empty
    /// and is judged like any other claim.
    #[cfg(test)]
    pub(super) fn gloss(&self) -> &[SectionRow] {
        self.include.as_deref().unwrap_or_default()
    }

    /// Whether the section declares a gloss at all.
    #[cfg(test)]
    pub(super) const fn glossed(&self) -> bool {
        self.include.is_some()
    }
}

/// The policy's whole parameter set: one section per owner holding the pair.
///
/// The file is thinner than the SPDX one, and the difference is the whole of what
/// the two policies differ in. What this policy needs from a repository is
/// *which files are governed*, and nothing else: the requirement itself is fixed
/// by the record and is no repository choice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parameters {
    /// Each owner's section, by owner.
    pub(crate) sections: BTreeMap<String, Section>,
}

/// One governed entry: a path, its owner, and the gloss row naming it, if any.
///
/// The row is an annotation and never the reason the path is governed. It is
/// present when the section declares a gloss whose rows name this path exactly
/// once, and absent both where no gloss is declared and where the declared gloss
/// failed over this path — in either case the path is governed all the same,
/// because the exclusion list already decided that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Governed {
    /// The governed path.
    pub(super) path: BytePath,
    /// The owner whose section governs it.
    pub(super) owner: String,
    /// The gloss row that names it, where a correct gloss names it.
    pub(super) row: Option<String>,
}

/// One named explanation of why a path left an owner's governed set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    /// The path, in the reversible byte display.
    pub(super) path: String,
    /// The owner whose section excluded it.
    pub(super) owner: String,
    /// The exclude row that names the exclusion.
    pub(super) name: String,
}

/// The plan-time result of the interchange policy's diagnostic-gloss selection.
#[derive(Debug, Clone, Default)]
pub struct SelectionPlan {
    governed: Vec<Governed>,
    findings: Vec<Finding>,
    exclusions: Vec<Exclusion>,
}

impl SelectionPlan {
    /// In-domain files selected by exclusion alone.
    #[must_use]
    pub fn governed(&self) -> &[Governed] {
        &self.governed
    }

    /// Stable policy findings mapped from the optional gloss judgment.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Named exclusion matches retained for audit explanations.
    #[must_use]
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.exclusions
    }
}

/// The full path an entry stands at under this root, when its bytes are text.
fn under(root: &Path, path: &BytePath) -> Option<PathBuf> {
    std::str::from_utf8(path.as_bytes())
        .ok()
        .map(|text| root.join(text))
}

/// The part of an attribution this policy has a universe over.
///
/// A path is in domain exactly when the catalog resolves a type from its name.
/// The filter runs before every coverage judgment rather than after any of them,
/// which is the whole of the polarity: an out-of-domain path is not a path this
/// policy excused, it is a path this policy was never asked about. Judging
/// coverage first and filtering afterwards would give the same governed set and
/// a wholly different obligation — every file in the repository would need a row
/// saying it is none of this policy's business.
fn in_domain<'a>(attribution: &BTreeMap<&'a BytePath, &'a str>) -> BTreeMap<&'a BytePath, &'a str> {
    attribution
        .iter()
        .filter(|(path, _)| carrier(path).is_some())
        .map(|(path, owner)| (*path, *owner))
        .collect()
}

/// Compile the interchange policy's typed exclusion-and-gloss selection.
#[must_use]
pub fn selection_plan(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> SelectionPlan {
    let attribution = in_domain(attribution);
    let sections: Vec<_> = parameters
        .sections
        .iter()
        .map(|(owner, section)| {
            let exclude = section
                .exclude
                .iter()
                .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), ()))
                .collect();
            let gloss = section.include.as_ref().map(|rows| {
                rows.iter()
                    .map(|row| SelectionRule::new(row.name.clone(), row.pattern.clone(), ()))
                    .collect()
            });
            GlossSection::new(owner.clone(), exclude, gloss)
        })
        .collect();
    let selected = diagnostic_gloss(&attribution, &sections);
    let governed = selected
        .governed
        .into_iter()
        .map(|entry| Governed {
            path: entry.path,
            owner: entry.owner,
            row: entry.row,
        })
        .collect();
    let exclusions = selected
        .excluded
        .into_iter()
        .map(|entry| Exclusion {
            path: entry.path.display(),
            owner: entry.owner,
            name: entry.name,
        })
        .collect();
    let findings = selected
        .defects
        .into_iter()
        .map(|defect| match defect {
            GlossDefect::Uncovered { path, owner } => Finding::InterchangeGlossUncovered {
                path: path.display(),
                owner,
            },
            GlossDefect::MultiplyIncluded {
                path,
                owner,
                matches,
            } => Finding::InterchangeMultiplyIncluded {
                path: path.display(),
                owner,
                count: matches.len(),
                matches,
            },
            GlossDefect::IdleRow { owner, list, name } => Finding::InterchangeIdleRow {
                owner,
                list: match list {
                    SelectionList::Exclude => ListKind::Exclude.as_str(),
                    SelectionList::Include => ListKind::Include.as_str(),
                },
                name,
            },
        })
        .collect();

    SelectionPlan {
        governed,
        findings,
        exclusions,
    }
}

/// The paths of one owner's share that its exclusion rows do not remove.
///
/// Every matching row is tallied rather than only the first, because the
/// relation is a union and the per-row reach is what the row names buy. A path
/// removed here is never offered to the gloss at all, which is why an overlap
/// between the two lists is legal and cannot be double accounting.
#[cfg(test)]
fn surviving<'a, 'r>(
    owner: &str,
    section: &'r Section,
    attribution: &BTreeMap<&'a BytePath, &'a str>,
    reached: &mut BTreeMap<(ListKind, &'r str), bool>,
) -> Vec<&'a BytePath> {
    let mut governed = Vec::new();

    for (path, accounted) in attribution {
        if *accounted != owner {
            continue;
        }

        let mut removed = false;

        for row in &section.exclude {
            if row.pattern.admits_path(path) {
                reached.insert((ListKind::Exclude, row.name.as_str()), true);
                removed = true;
            }
        }

        if !removed {
            governed.push(*path);
        }
    }

    governed
}

/// Divide each owner's accounted share into excluded and governed.
///
/// The universe is the in-domain set and nothing wider, and within it exclusion
/// alone computes governance (´dec:envelope:computed-governance´): the governed
/// set is the owner's in-domain share minus the union of its named exclusions.
/// Nothing has to be included for a file to be governed, so there is no totality
/// obligation left to fail and the exclude rows that stand are the ones a reader
/// ever wanted — each names a document whose schema another convention owns.
///
/// A section may still declare an inclusion list, and where it does, that list is
/// a diagnostic gloss and is checked as one. The gloss claims to be a complete,
/// pairwise-disjoint partition of the already computed governed set, so it is
/// judged over that set and nothing wider: a governed path no gloss row names
/// breaks completeness, a governed path two rows name breaks disjointness, and
/// the two together are what equality with the governed set comes to. A gloss
/// row reaching nothing in the governed set — because it only ever reaches paths
/// the exclusions removed, or paths outside the domain — is idle, which is the
/// same judgment an idle row has always earned. None of these verdicts moves a
/// path: a false gloss fails configuration and governance is what it was.
///
/// The two passes stay separate because a path the exclude rows removed is never
/// evaluated against the gloss at all, so an overlap between the two is legal and
/// cannot be double accounting. That is what lets a named foreign-schema row
/// overlap a broad gloss row without either being narrowed to accommodate the
/// other: the exclusion pre-pass is order-independent and runs first, and the
/// gloss judgment is taken over what survives.
///
/// Reach is measured over the owner's own share and never over the repository,
/// which is what retires the containment question rather than answering it: a
/// row cannot reach another owner's file, because it is never shown one.
#[cfg(test)]
pub fn retiring_govern<'a>(
    parameters: &'a Parameters,
    attribution: &BTreeMap<&'a BytePath, &'a str>,
) -> (Vec<Governed>, Vec<Finding>) {
    let attribution = &in_domain(attribution);
    let mut governed = Vec::new();
    let mut findings = Vec::new();

    for (owner, section) in &parameters.sections {
        let mut reached: BTreeMap<(ListKind, &str), bool> = BTreeMap::new();

        for row in &section.exclude {
            reached.insert((ListKind::Exclude, row.name.as_str()), false);
        }

        for row in section.gloss() {
            reached.insert((ListKind::Include, row.name.as_str()), false);
        }

        // What the exclusion pre-pass leaves is the governed set. The gloss,
        // where one is declared, is then held against exactly that set.
        for path in surviving(owner, section, attribution, &mut reached) {
            let matched: Vec<&SectionRow> = section
                .gloss()
                .iter()
                .filter(|row| row.pattern.admits_path(path))
                .collect();

            for row in &matched {
                reached.insert((ListKind::Include, row.name.as_str()), true);
            }

            let row = match (section.glossed(), matched.as_slice()) {
                (false, _) => None,
                (true, [row]) => Some(row.name.clone()),
                (true, []) => {
                    findings.push(Finding::InterchangeGlossUncovered {
                        path: path.display(),
                        owner: owner.clone(),
                    });

                    None
                }
                (true, rows) => {
                    let mut names: Vec<String> = rows.iter().map(ToString::to_string).collect();
                    names.sort();

                    findings.push(Finding::InterchangeMultiplyIncluded {
                        path: path.display(),
                        owner: owner.clone(),
                        count: rows.len(),
                        matches: names,
                    });

                    None
                }
            };

            governed.push(Governed {
                path: path.clone(),
                owner: owner.clone(),
                row,
            });
        }

        for ((kind, name), found) in reached {
            if !found {
                findings.push(Finding::InterchangeIdleRow {
                    owner: owner.clone(),
                    list: kind.as_str(),
                    name: name.to_owned(),
                });
            }
        }
    }

    (governed, findings)
}

/// Divide each owner's in-domain share through diagnostic-gloss selection.
#[cfg(test)]
pub fn govern(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
) -> (Vec<Governed>, Vec<Finding>) {
    let selected = selection_plan(parameters, attribution);
    (selected.governed, selected.findings)
}

/// Name every governed entry that can never carry a document.
///
/// A symbolic link is never a document carrier: the universe counts a link and
/// never follows it, so a link has no content of its own to parse. A section
/// leaving a link in its governed set is a configuration finding, and the remedy
/// is an exclude row.
///
/// A file of a type the program does not know is *not* named here, and the
/// absence is deliberate rather than an omission to be filled in later. Under
/// the one-family ruling such a file is ignored, and a finding would be an
/// announcement the ruling says is not made.
pub fn carriers(root: &Path, governed: &[Governed]) -> Vec<Finding> {
    let mut findings = Vec::new();

    for entry in governed {
        let Some(full) = under(root, &entry.path) else {
            continue;
        };

        if full.is_symlink() {
            findings.push(Finding::InterchangeLinkedPath {
                path: entry.path.display(),
                owner: entry.owner.clone(),
                name: entry.row.clone(),
            });
        }
    }

    findings
}

/// The defects every governed entry's envelope carries, entry by entry.
fn envelopes(root: &Path, governed: &[Governed]) -> Vec<(String, Vec<Defect>, Carrier)> {
    let mut read = Vec::new();

    for entry in governed {
        // A link is named by the configuration pass and has nothing to parse.
        let Some(full) = under(root, &entry.path) else {
            continue;
        };

        if full.is_symlink() {
            continue;
        }

        // The program reads the type from the name. A type it does not know is
        // ignored: no verdict, no finding, no burn row.
        let Some(carrier) = carrier(&entry.path) else {
            continue;
        };

        let Ok(bytes) = std::fs::read(&full) else {
            continue;
        };

        let defects = defects(carrier, &bytes);

        if !defects.is_empty() {
            read.push((entry.path.display(), defects, carrier));
        }
    }

    read
}

/// Judge every governed entry's envelope, excusing the paths a list tolerates.
pub fn conform(
    root: &Path,
    governed: &[Governed],
    tolerated: &BTreeSet<&BytePath>,
) -> Vec<Finding> {
    let excused: BTreeSet<String> = tolerated.iter().map(|path| path.display()).collect();

    envelopes(root, governed)
        .into_iter()
        .filter(|(path, _, _)| !excused.contains(path))
        .flat_map(|(path, defects, carrier)| {
            defects
                .into_iter()
                .map(move |defect| Finding::InterchangeEnvelope {
                    path: path.clone(),
                    defect: message(&defect, carrier),
                })
        })
        .collect()
}

/// The governed paths whose envelope is defective, which is the observation the
/// burn machinery compares a declared list against.
///
/// The identity is a path and a file holds at most one violation, whatever
/// number of defects its envelope carries, which is what the path-set codec
/// says. A file failing three ways is one debt and not three.
pub fn violating_paths<'a>(root: &Path, governed: &'a [Governed]) -> BTreeSet<&'a BytePath> {
    let failing: BTreeSet<String> = envelopes(root, governed)
        .into_iter()
        .map(|(path, _, _)| path)
        .collect();

    governed
        .iter()
        .map(|entry| &entry.path)
        .filter(|path| failing.contains(&path.display()))
        .collect()
}

/// The named explanations one owner's exclusions earn in the audit response.
///
/// The audit sees the same universe the verdict does, because an explanation of
/// why a path left the governed set presupposes that the path was in it to
/// leave. An out-of-domain path was never in the universe, so a row that happens
/// to reach it explains nothing and is not reported as though it had.
#[cfg(test)]
pub fn exclusions(
    parameters: &Parameters,
    attribution: &BTreeMap<&BytePath, &str>,
    owner: &str,
) -> Vec<Exclusion> {
    selection_plan(parameters, attribution)
        .exclusions
        .into_iter()
        .filter(|excluded| excluded.owner == owner)
        .collect()
}

#[cfg(test)]
pub fn retiring_exclusions<'a>(
    parameters: &Parameters,
    attribution: &BTreeMap<&'a BytePath, &'a str>,
    owner: &str,
) -> Vec<Exclusion> {
    let attribution = &in_domain(attribution);
    let mut named = Vec::new();

    let Some(section) = parameters.sections.get(owner) else {
        return named;
    };

    for (path, accounted) in attribution {
        if *accounted != owner {
            continue;
        }

        for row in &section.exclude {
            if row.pattern.admits_path(path) {
                named.push(Exclusion {
                    path: path.display(),
                    owner: owner.to_owned(),
                    name: row.name.clone(),
                });
            }
        }
    }

    named
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use tempfile::TempDir;

    use super::{
        Carrier, Defect, LABEL_BOUND, NAMESPACE_KEY, Parameters, RESERVED_KEYS, Section,
        SectionRow, TRIPLE_MEMBERS, VERSION_KEY, carrier, carriers, conform, declared_refusals,
        exclusions, govern, is_canonical_integer, is_canonical_triple, is_namespace_label,
        json_defects, message, toml_defects, violating_paths, yaml_defects,
    };
    use crate::declaration::AbnfPattern;
    use crate::finding::Finding;
    use crate::pattern::BytePath;

    /// Decode a display into the path it stands for.
    fn path(display: &str) -> BytePath {
        BytePath::decode(display).expect("a decodable path")
    }

    /// The envelope a conforming document carries, over an invented corpus:
    /// the policy reads where the two keys stand and never whose corpus they
    /// name, so a fixture that named this one would be testing the corpus.
    const CONFORMING: &str =
        "namespace = \"com.quarry.linter.owners\"\nversion = [1, 0, 0]\n\nowners = [\"ALPHA\"]\n";

    /// The three spellings of a triple, as the carriers hand them over.
    fn triple(first: &str, second: &str, third: &str) -> Vec<String> {
        vec![
            String::from(first),
            String::from(second),
            String::from(third),
        ]
    }

    /// Read one TOML source's envelope and hand back what is wrong with it.
    fn toml(text: &str) -> Vec<Defect> {
        toml_defects(text.as_bytes())
    }

    /// Read one JSON source's envelope and hand back what is wrong with it.
    fn json(text: &str) -> Vec<Defect> {
        json_defects(text.as_bytes())
    }

    /// Read one YAML source's envelope and hand back what is wrong with it.
    fn yaml(text: &str) -> Vec<Defect> {
        yaml_defects(text.as_bytes())
    }

    /// A label is two or more dot-separated atoms over the thirty-six-character
    /// alphabet and the interior hyphen. One atom is a bare top-level word and no
    /// label, an empty atom from a leading, trailing or doubled dot fails, and an
    /// atom opening or closing on a hyphen fails because the hyphen is interior.
    /// Upper case and the underscore are outside the alphabet, and so is every
    /// character that would need escaping in any of the three carriers — which is
    /// what lets one label have exactly one byte form in all of them.
    ///
    /// ´claim:interchange:a-label-is-two-or-more-atoms-over-a-closed-alphabet´
    /// ´test:unit:a-label-is-two-or-more-atoms-over-a-closed-alphabet´
    #[test]
    fn a_label_is_two_or_more_atoms_over_a_closed_alphabet() {
        assert!(is_namespace_label("com.ember.index.linter.owners"));
        assert!(is_namespace_label("a.b"));
        assert!(is_namespace_label("a1.b2c3"));

        // Hyphens are interior, and the ABNF admits a run of them there.
        assert!(is_namespace_label("com.ember-index.policy-interchange"));
        assert!(is_namespace_label("a--b.c"));

        // One atom is no label: every label stands strictly below the root.
        assert!(!is_namespace_label("com"));
        assert!(!is_namespace_label(""));

        // The dot separates and is no character of an atom, so an empty atom fails.
        assert!(!is_namespace_label(".com.ember"));
        assert!(!is_namespace_label("com.ember."));
        assert!(!is_namespace_label("com..ember"));

        // A hyphen at either end of an atom is not interior.
        assert!(!is_namespace_label("-com.ember"));
        assert!(!is_namespace_label("com-.ember"));
        assert!(!is_namespace_label("com.-ember"));

        // Outside the alphabet: upper case, the underscore, and anything above ASCII.
        assert!(!is_namespace_label("Com.ember"));
        assert!(!is_namespace_label("com_x.ember"));
        assert!(!is_namespace_label("com.tørrust"));
        assert!(!is_namespace_label("com ember.x"));
    }

    /// The byte bound is inclusive at the record's figure, and the character
    /// bound coincides with it because every character of the alphabet is
    /// printable ASCII. A label of the bound passes and one of a single byte more
    /// fails, which is the pair the oracle asks for.
    ///
    /// ´claim:interchange:a-label-occupies-at-most-the-record-s-byte-bound´
    /// ´test:unit:a-label-occupies-at-most-the-record-s-byte-bound´
    #[test]
    fn a_label_occupies_at_most_the_record_s_byte_bound() {
        // "a." and then a tail of the requested length, so the whole is exact.
        let sized = |bytes: usize| format!("a.{}", "b".repeat(bytes - 2));

        let at_bound = sized(LABEL_BOUND);
        assert_eq!(at_bound.len(), LABEL_BOUND);
        assert!(is_namespace_label(&at_bound));

        let over_bound = sized(LABEL_BOUND + 1);
        assert_eq!(over_bound.len(), LABEL_BOUND + 1);
        assert!(!is_namespace_label(&over_bound));
    }

    /// A triple member is plain decimal with no sign, no underscore, no leading
    /// zero and none of the alternative bases. Every refused form here denotes an
    /// integer, so the predicate reads the spelling: after a parser has resolved
    /// `0x1`, `1_0` and `010` there is nothing left to refuse, and a version
    /// whose value depends on which resolution a reader applies is not a version
    /// claim at all.
    ///
    /// ´claim:interchange:a-triple-member-is-canonical-in-its-spelling´
    /// ´test:unit:a-triple-member-is-canonical-in-its-spelling´
    #[test]
    fn a_triple_member_is_canonical_in_its_spelling() {
        assert!(is_canonical_integer("0"));
        assert!(is_canonical_integer("1"));
        assert!(is_canonical_integer("40"));
        assert!(is_canonical_integer("1234567890"));

        // A leading zero reaches every spelling but zero's own.
        assert!(!is_canonical_integer("01"));
        assert!(!is_canonical_integer("00"));
        assert!(!is_canonical_integer("010"));

        // Signs, separators, the alternative bases, and the non-integer forms.
        assert!(!is_canonical_integer("-1"));
        assert!(!is_canonical_integer("+1"));
        assert!(!is_canonical_integer("1_0"));
        assert!(!is_canonical_integer("0x1"));
        assert!(!is_canonical_integer("0o1"));
        assert!(!is_canonical_integer("0b1"));
        assert!(!is_canonical_integer("1.0"));
        assert!(!is_canonical_integer("1e0"));
        assert!(!is_canonical_integer(""));
        assert!(!is_canonical_integer(" 1"));
    }

    /// The triple has exactly three members. Two and four are the pair the oracle
    /// asks for, and neither is a version the record can read: a version is a
    /// triple of major, minor and patch, and a document offering some other arity
    /// has not made a claim to be misread.
    ///
    /// ´claim:interchange:a-triple-has-exactly-three-members´
    /// ´test:unit:a-triple-has-exactly-three-members´
    #[test]
    fn a_triple_has_exactly_three_members() {
        assert_eq!(TRIPLE_MEMBERS, 3);
        assert!(is_canonical_triple(&triple("1", "0", "0")));
        assert!(is_canonical_triple(&triple("0", "0", "0")));

        assert!(!is_canonical_triple(&[
            String::from("1"),
            String::from("0")
        ]));
        assert!(!is_canonical_triple(&[
            String::from("1"),
            String::from("0"),
            String::from("0"),
            String::from("0")
        ]));
        assert!(!is_canonical_triple(&[]));

        // One bad member spoils the triple wherever it stands.
        assert!(!is_canonical_triple(&triple("1", "0", "-1")));
        assert!(!is_canonical_triple(&triple("0x1", "0", "0")));
    }

    /// The two reserved names are spelled as words and stand in the order the
    /// envelope stands in. The order is a fact about the envelope rather than
    /// about any one carrier, and the carriers that cannot require it still read
    /// the same two names.
    ///
    /// ´claim:interchange:the-reserved-names-are-words-in-the-envelope-s-order´
    /// ´test:unit:the-reserved-names-are-words-in-the-envelope-s-order´
    #[test]
    fn the_reserved_names_are_words_in_the_envelope_s_order() {
        assert_eq!(NAMESPACE_KEY, "namespace");
        assert_eq!(VERSION_KEY, "version");
        assert_eq!(RESERVED_KEYS, [NAMESPACE_KEY, VERSION_KEY]);
    }

    /// A TOML document conforms when its two envelope keys are its first two
    /// top-level pairs, in order, preceded only by comments and blank lines. The
    /// rest of the document is free: every other top-level key, and everything
    /// beneath it, is content the policy does not inspect, and a nested key of a
    /// reserved name is untouched — which is exactly why a manifest can already
    /// carry a version and still carry no envelope.
    ///
    /// ´claim:interchange:a-toml-envelope-stands-first-and-the-rest-is-free´
    /// ´test:unit:a-toml-envelope-stands-first-and-the-rest-is-free´
    #[test]
    fn a_toml_envelope_stands_first_and_the_rest_is_free() {
        assert_eq!(toml(CONFORMING), Vec::new());

        // A policy document in the shape declarations take: the envelope, then
        // the table headers the declarations live under. Both the corpus and
        // its set are invented, and the grammar is the ratified one, because a
        // fixture spelled in a grammar no corpus can write is a fixture of
        // nothing this decoder does.
        assert_eq!(
            toml(
                "namespace = \"com.quarry.linter.policy.spdx\"\nversion = [1, 0, 0]\n\n[set.identifier]\nagpl3only = \"AGPL-3.0-only\"\n"
            ),
            Vec::new()
        );

        // Comments and blank lines may precede the envelope and stand between
        // the two keys, because neither is a top-level key of the document.
        assert_eq!(
            toml(
                "# a leading comment\n\nnamespace = \"com.quarry.linter.owners\"\n# between\nversion = [1, 0, 0]\n"
            ),
            Vec::new()
        );

        // The rest of the document is content, including a nested key of either
        // reserved name, which is three tokens deep and no part of any envelope.
        assert_eq!(
            toml(
                "namespace = \"a.b\"\nversion = [1, 0, 0]\n\n[package]\nversion = \"4.0.0-develop\"\nnamespace = \"x\"\n"
            ),
            Vec::new()
        );
    }

    /// An absent envelope key is named for the key that is absent, and a key
    /// written after a table header is absent rather than misplaced. That is
    /// TOML's own rule doing the work: a bare key following the first header
    /// belongs to that table, so it is not a top-level key at all and the
    /// document has not named itself.
    ///
    /// ´claim:interchange:an-absent-toml-envelope-key-is-named-for-its-own-absence´
    /// ´test:unit:an-absent-toml-envelope-key-is-named-for-its-own-absence´
    #[test]
    fn an_absent_toml_envelope_key_is_named_for_its_own_absence() {
        assert_eq!(toml("version = [1, 0, 0]\n"), vec![Defect::NoNamespace]);
        assert_eq!(toml("namespace = \"a.b\"\n"), vec![Defect::NoVersion]);
        assert_eq!(
            toml("owners = []\n"),
            vec![Defect::NoNamespace, Defect::NoVersion]
        );

        // An envelope standing after a table header is no envelope: both keys
        // belong to the table the header opened.
        assert_eq!(
            toml("[owners]\nnamespace = \"a.b\"\nversion = [1, 0, 0]\n"),
            vec![Defect::NoNamespace, Defect::NoVersion]
        );
    }

    /// A namespace is a basic double-quoted string, unescaped, whose content is
    /// a label. A literal single-quoted string carrying the same label fails and
    /// so does a basic string escaping a character it could have written
    /// directly: no character of the alphabet needs escaping and none is a
    /// quote, so the two forms encode identical bytes, and a carrier offering two
    /// spellings of one label would introduce an ambiguity the discipline does
    /// not have. The value is quoted back exactly as written.
    ///
    /// ´claim:interchange:a-toml-label-has-one-spelling-and-the-others-are-quoted-back´
    /// ´test:unit:a-toml-label-has-one-spelling-and-the-others-are-quoted-back´
    #[test]
    fn a_toml_label_has_one_spelling_and_the_others_are_quoted_back() {
        let namespaced = |value: &str| toml(&format!("namespace = {value}\nversion = [1, 0, 0]\n"));

        // The literal string carries identical bytes and is refused all the same.
        assert_eq!(
            namespaced("'com.ember.index'"),
            vec![Defect::NotLabel(String::from("'com.ember.index'"))]
        );

        // An escape of a character the alphabet writes directly is a second
        // spelling of a label that has exactly one byte form.
        assert_eq!(
            namespaced("\"\\u0063om.ember\""),
            vec![Defect::NotLabel(String::from("\"\\u0063om.ember\""))]
        );

        // A bare top-level word is no label, and neither is a value of another type.
        assert_eq!(
            namespaced("\"com\""),
            vec![Defect::NotLabel(String::from("\"com\""))]
        );
        assert_eq!(
            namespaced("[1, 0, 0]"),
            vec![Defect::NotLabel(String::from("[1, 0, 0]"))]
        );

        // The byte bound is the record's, and the carrier does not widen it: a
        // label of the bound passes and one of a single byte more is refused.
        let sized = |bytes: usize| format!("\"a.{}\"", "b".repeat(bytes - 2));
        assert_eq!(namespaced(&sized(LABEL_BOUND)), Vec::new());
        assert_eq!(
            namespaced(&sized(LABEL_BOUND + 1)),
            vec![Defect::NotLabel(sized(LABEL_BOUND + 1))]
        );
    }

    /// A version is an array of exactly three integers, each canonical in its
    /// spelling. Two members and four are refused for arity; a negative member
    /// and a hexadecimal one are refused for spelling, and both are integers TOML
    /// is happy to hand over — which is why the check reads the source text and
    /// never the parsed value. The value is quoted back as written.
    ///
    /// ´claim:interchange:a-toml-triple-is-three-canonical-integers-and-nothing-else´
    /// ´test:unit:a-toml-triple-is-three-canonical-integers-and-nothing-else´
    #[test]
    fn a_toml_triple_is_three_canonical_integers_and_nothing_else() {
        let versioned = |value: &str| toml(&format!("namespace = \"a.b\"\nversion = {value}\n"));

        assert_eq!(versioned("[1, 0, 0]"), Vec::new());
        assert_eq!(versioned("[0,0,0]"), Vec::new());

        // Arity: two members and four.
        assert_eq!(
            versioned("[1, 0]"),
            vec![Defect::NotTriple(String::from("[1, 0]"))]
        );
        assert_eq!(
            versioned("[1, 0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1, 0, 0, 0]"))]
        );

        // Spelling: a negative member and a hexadecimal one, both of which TOML
        // reads as ordinary integers.
        assert_eq!(
            versioned("[1, 0, -1]"),
            vec![Defect::NotTriple(String::from("[1, 0, -1]"))]
        );
        assert_eq!(
            versioned("[0x1, 0, 0]"),
            vec![Defect::NotTriple(String::from("[0x1, 0, 0]"))]
        );

        // The remaining non-shapes the decision names (´dec:envelope:source-shape´), and the underscore form.
        assert_eq!(
            versioned(r#"["1", 0, 0]"#),
            vec![Defect::NotTriple(String::from(r#"["1", 0, 0]"#))]
        );
        assert_eq!(
            versioned("[1.0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1.0, 0, 0]"))]
        );
        assert_eq!(
            versioned("[1_0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1_0, 0, 0]"))]
        );
        assert_eq!(versioned("1"), vec![Defect::NotTriple(String::from("1"))]);

        // A leading zero is refused by this carrier before the envelope reader
        // sees it, because TOML's own integer grammar forbids one. The
        // prohibition still has to be written here, and it earns its keep in
        // YAML, where a leading-zero scalar parses and its value depends on
        // which integer resolution the reader applies.
        assert_eq!(versioned("[01, 0, 0]"), vec![Defect::NotWellFormed]);
    }

    /// The order between the two keys is the part of the rule TOML does not
    /// supply, and it is what recovers bounded determination for this carrier: a
    /// reader knows what it is holding after a bounded prefix rather than after
    /// the whole document. A reversed pair and a third key interleaved between
    /// them both break it, and a document that meant the envelope and misplaced
    /// it is reported for the order rather than for an absence.
    ///
    /// ´claim:interchange:the-toml-envelope-keys-stand-in-one-order´
    /// ´test:unit:the-toml-envelope-keys-stand-in-one-order´
    #[test]
    fn the_toml_envelope_keys_stand_in_one_order() {
        // The reversed pair reports the order once rather than once per key.
        assert_eq!(
            toml("version = [1, 0, 0]\nnamespace = \"a.b\"\n"),
            vec![Defect::WrongOrder]
        );

        // A third top-level key may not be interleaved between the two.
        assert_eq!(
            toml("namespace = \"a.b\"\ntitle = \"x\"\nversion = [1, 0, 0]\n"),
            vec![Defect::WrongOrder]
        );

        // Nor may a third key precede them.
        assert_eq!(
            toml("title = \"x\"\nnamespace = \"a.b\"\nversion = [1, 0, 0]\n"),
            vec![Defect::WrongOrder]
        );
    }

    /// A reserved name carries content when it opens a table, by a header or as
    /// the head of a dotted key, and that is the one thing the reservation rule
    /// forbids. A bare top-level key of either name is an envelope key however it
    /// is placed, so the two mistakes stay apart: a document that opened a table
    /// called `version` meant something else by the word, and a document that
    /// wrote the pair backwards did not.
    ///
    /// ´claim:interchange:a-reserved-toml-name-that-opens-a-table-carries-content´
    /// ´test:unit:a-reserved-toml-name-that-opens-a-table-carries-content´
    #[test]
    fn a_reserved_toml_name_that_opens_a_table_carries_content() {
        assert_eq!(
            toml("[namespace]\nkind = \"content\"\n"),
            vec![Defect::Reserved(NAMESPACE_KEY), Defect::NoVersion]
        );
        // A reserved name cannot open a table beside its own envelope key, because
        // TOML refuses the document for the duplicate before the reader is
        // reached. So the reachable shape is the one where the name was spent on
        // content and the envelope key it should have been is simply missing.
        assert_eq!(
            toml("namespace = \"a.b\"\nversion = [1, 0, 0]\n\n[version]\nmajor = 1\n"),
            vec![Defect::NotWellFormed]
        );
        assert_eq!(
            toml("namespace = \"a.b\"\n\n[version]\nmajor = 1\n"),
            vec![Defect::Reserved(VERSION_KEY)]
        );

        // A dotted key opens a table under its head, so it is the same mistake.
        assert_eq!(
            toml("version.major = 1\n"),
            vec![Defect::NoNamespace, Defect::Reserved(VERSION_KEY)]
        );
    }

    /// Bytes that are no document of the carrier fail at the same door as bytes
    /// that are no text. All three formats are defined over Unicode, so a
    /// governed file must be decoded to be parsed at all, and this is the one
    /// place the surface's byte discipline meets formats whose definitions
    /// include an encoding. It is the rule meeting the formats rather than the
    /// rule relaxing.
    ///
    /// ´claim:interchange:bytes-that-are-no-document-fail-where-bytes-that-are-no-text-fail´
    /// ´test:unit:bytes-that-are-no-document-fail-where-bytes-that-are-no-text-fail´
    #[test]
    fn bytes_that_are_no_document_fail_where_bytes_that_are_no_text_fail() {
        // A governed file that is not TOML at all.
        assert_eq!(
            toml("{\"namespace\": \"a.b\"}\n"),
            vec![Defect::NotWellFormed]
        );
        assert_eq!(toml("namespace = \n"), vec![Defect::NotWellFormed]);

        // A governed file whose bytes are no valid encoding, envelope or not.
        assert_eq!(
            toml_defects(b"namespace = \"a.b\"\nversion = [1, 0, 0]\n\xff\xfe"),
            vec![Defect::NotWellFormed]
        );
    }

    /// A JSON envelope is two members of the root and may stand anywhere in it.
    /// Standing first cannot be required, because JSON defines an object as an
    /// unordered collection: a root member is a root member wherever its bytes
    /// fall, so an order would be a requirement on the serialization rather than
    /// on the document, and no tool that reads and rewrites a file could be held
    /// to it. The envelope standing last therefore passes, which is the case an
    /// implementation is likeliest to get wrong, because the TOML check next to
    /// it refuses exactly that shape.
    ///
    /// ´claim:interchange:a-json-envelope-may-stand-anywhere-in-the-root-object´
    /// ´test:unit:a-json-envelope-may-stand-anywhere-in-the-root-object´
    #[test]
    fn a_json_envelope_may_stand_anywhere_in_the_root_object() {
        assert_eq!(
            json(
                r#"{"namespace": "com.ember.index.linter.owners", "version": [1, 0, 0], "owners": ["INDEX"]}"#
            ),
            Vec::new()
        );

        // The envelope standing last. This must pass.
        assert_eq!(
            json(
                r#"{"owners": ["INDEX"], "namespace": "com.ember.index.linter.owners", "version": [1, 0, 0]}"#
            ),
            Vec::new()
        );

        // And standing either side of the content it names.
        assert_eq!(
            json("{\n  \"namespace\": \"a.b\",\n  \"owners\": [],\n  \"version\": [1, 0, 0]\n}\n"),
            Vec::new()
        );
    }

    /// An envelope is two members of the document's root, so a document whose
    /// root is an array has nowhere to put one. The shape is not hypothetical —
    /// this tree holds such a file — and there is no third option: a first-party
    /// JSON document whose natural root is an array is reshaped or excluded.
    ///
    /// ´claim:interchange:a-json-root-that-is-no-object-can-hold-no-envelope´
    /// ´test:unit:a-json-root-that-is-no-object-can-hold-no-envelope´
    #[test]
    fn a_json_root_that_is_no_object_can_hold_no_envelope() {
        assert_eq!(
            json(r#"[{"name": "bug"}, {"name": "chore"}]"#),
            vec![Defect::RootNotObject]
        );
        assert_eq!(json("42"), vec![Defect::RootNotObject]);
        assert_eq!(json(r#""a.b""#), vec![Defect::RootNotObject]);

        // Bytes that are no JSON document at all, and bytes that are no text.
        assert_eq!(json("{\"namespace\": }"), vec![Defect::NotWellFormed]);
        assert_eq!(
            json_defects(b"{\"namespace\": \"a.b\"\xff}"),
            vec![Defect::NotWellFormed]
        );
    }

    /// A root object holding two members of one envelope name is refused. JSON
    /// leaves the reading of such a document to the parser, which is exactly the
    /// ambiguity this discipline does not have: a label with one byte form cannot
    /// be carried by a document whose reading depends on which duplicate a parser
    /// happens to keep. The reader therefore reads the source, because a parsed
    /// object has already resolved the question by discarding one of them.
    ///
    /// ´claim:interchange:a-duplicated-json-envelope-member-is-refused´
    /// ´test:unit:a-duplicated-json-envelope-member-is-refused´
    #[test]
    fn a_duplicated_json_envelope_member_is_refused() {
        assert_eq!(
            json(r#"{"namespace": "a.b", "namespace": "c.d", "version": [1, 0, 0]}"#),
            vec![Defect::Duplicated(NAMESPACE_KEY)]
        );
        assert_eq!(
            json(r#"{"namespace": "a.b", "version": [1, 0, 0], "version": [2, 0, 0]}"#),
            vec![Defect::Duplicated(VERSION_KEY)]
        );
    }

    /// The label and the triple are read from the source in this carrier too. A
    /// label escaping a character it could have written directly is a second
    /// spelling of a label that has one byte form, and a parser would have
    /// unescaped it before the check saw it. A fractional member and an
    /// exponent-form one are numbers JSON is content with and no canonical
    /// non-negative integers.
    ///
    /// ´claim:interchange:a-json-label-and-triple-are-read-from-the-source´
    /// ´test:unit:a-json-label-and-triple-are-read-from-the-source´
    #[test]
    fn a_json_label_and_triple_are_read_from_the_source() {
        let namespaced = |value: &str| {
            json(&format!(
                r#"{{"namespace": {value}, "version": [1, 0, 0]}}"#
            ))
        };
        let versioned =
            |value: &str| json(&format!(r#"{{"namespace": "a.b", "version": {value}}}"#));

        assert_eq!(
            namespaced(r#""\u0063om.ember""#),
            vec![Defect::NotLabel(String::from(r#""\u0063om.ember""#))]
        );
        assert_eq!(
            namespaced(r#""com""#),
            vec![Defect::NotLabel(String::from(r#""com""#))]
        );
        assert_eq!(
            namespaced("[1, 0, 0]"),
            vec![Defect::NotLabel(String::from("[1, 0, 0]"))]
        );

        assert_eq!(
            versioned("[1.0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1.0, 0, 0]"))]
        );
        assert_eq!(
            versioned("[1e0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1e0, 0, 0]"))]
        );
        assert_eq!(
            versioned("[1, 0]"),
            vec![Defect::NotTriple(String::from("[1, 0]"))]
        );
        assert_eq!(
            versioned(r#"["1", 0, 0]"#),
            vec![Defect::NotTriple(String::from(r#"["1", 0, 0]"#))]
        );

        // Both keys absent, each named for its own absence.
        assert_eq!(
            json(r#"{"owners": []}"#),
            vec![Defect::NoNamespace, Defect::NoVersion]
        );
    }

    /// A YAML envelope is two keys of the root mapping and may stand anywhere in
    /// it, for the reason JSON gives: mappings are unordered in the data model,
    /// so a root key is a root key wherever it is written. A repository may put
    /// the envelope at the head of every file as a house convention, and the
    /// policy does not check it. The envelope standing last therefore passes,
    /// which with its JSON twin is the case an implementation is likeliest to get
    /// wrong, because the TOML check refuses exactly that shape.
    ///
    /// ´claim:interchange:a-yaml-envelope-may-stand-anywhere-in-the-root-mapping´
    /// ´test:unit:a-yaml-envelope-may-stand-anywhere-in-the-root-mapping´
    #[test]
    fn a_yaml_envelope_may_stand_anywhere_in_the_root_mapping() {
        assert_eq!(
            yaml(
                "namespace: com.ember.index.linter.owners\nversion: [1, 0, 0]\n\nowners:\n  - INDEX\n"
            ),
            Vec::new()
        );

        // The envelope standing last, after a block value of its own. This must pass.
        assert_eq!(
            yaml(
                "owners:\n  - INDEX\nnamespace: com.ember.index.linter.owners\nversion: [1, 0, 0]\n"
            ),
            Vec::new()
        );

        // A leading document marker and comments do not disturb it.
        assert_eq!(
            yaml("---\n# a comment\nnamespace: a.b\nversion: [1, 0, 0]\n"),
            Vec::new()
        );
    }

    /// The envelope presupposes a document and a root mapping to hold it. A root
    /// that is a sequence has nowhere to put two keys, and a stream holding no
    /// document or more than one leaves the check no root to read — which is the
    /// one place it looks above the root mapping, and it looks there because it
    /// must choose which root it reads.
    ///
    /// ´claim:interchange:a-yaml-envelope-wants-one-document-and-a-root-mapping´
    /// ´test:unit:a-yaml-envelope-wants-one-document-and-a-root-mapping´
    #[test]
    fn a_yaml_envelope_wants_one_document_and_a_root_mapping() {
        assert_eq!(yaml("- one\n- two\n"), vec![Defect::RootNotMapping]);
        assert_eq!(yaml("[1, 2]\n"), vec![Defect::RootNotMapping]);

        // A stream of two documents, and a stream of none.
        assert_eq!(
            yaml("namespace: a.b\nversion: [1, 0, 0]\n---\nnamespace: c.d\nversion: [1, 0, 0]\n"),
            vec![Defect::Documents(2)]
        );
        assert_eq!(
            yaml("# nothing but a comment\n"),
            vec![Defect::Documents(0)]
        );

        // Bytes that are no text fail where bytes that are no document fail.
        assert_eq!(
            yaml_defects(b"namespace: a.b\n\xff\xfe"),
            vec![Defect::NotWellFormed]
        );
    }

    /// The two envelope keys must be written where they are read. A value
    /// arriving through an alias, a root mapping assembled through a merge key,
    /// and either key carrying an explicit tag are all refused, on the ground the
    /// one-spelling rule gives: a resolved alias is not a spelling of a value but
    /// a second place the value lives. The scope is deliberately narrow, and
    /// anchors and aliases below the root mapping are content the policy does not
    /// inspect.
    ///
    /// ´claim:interchange:a-yaml-envelope-key-is-written-where-it-is-read´
    /// ´test:unit:a-yaml-envelope-key-is-written-where-it-is-read´
    #[test]
    fn a_yaml_envelope_key_is_written_where_it_is_read() {
        // Through an alias.
        assert_eq!(
            yaml("base: &base a.b\nnamespace: *base\nversion: [1, 0, 0]\n"),
            vec![Defect::Indirect(NAMESPACE_KEY)]
        );

        // Through a merge key, which is where the absent key would have come from.
        assert_eq!(
            yaml("<<: *base\nversion: [1, 0, 0]\n"),
            vec![Defect::Indirect(NAMESPACE_KEY)]
        );

        // Through an explicit tag.
        assert_eq!(
            yaml("namespace: !!str a.b\nversion: [1, 0, 0]\n"),
            vec![Defect::Indirect(NAMESPACE_KEY)]
        );
        assert_eq!(
            yaml("namespace: a.b\nversion: !!seq [1, 0, 0]\n"),
            vec![Defect::Indirect(VERSION_KEY)]
        );

        // An anchor below the root mapping is content and is not the policy's business.
        assert_eq!(
            yaml("namespace: a.b\nversion: [1, 0, 0]\nowners:\n  - &first INDEX\n"),
            Vec::new()
        );
    }

    /// YAML inverts the other two carriers on the label and constrains the triple
    /// more tightly. The label is a plain scalar and the quoted forms fail,
    /// because every label the grammar admits is representable plainly and the
    /// plain scalar is the one form always available and always unambiguous here.
    /// The triple is required in flow form for one spelling, and the leading-zero
    /// and underscore prohibitions matter more here than anywhere else: a parser
    /// applying the older integer resolution reads a leading-zero scalar as octal
    /// and admits underscores as separators, so such a member is a number whose
    /// value depends on which reader has it, and that is no version claim.
    ///
    /// ´claim:interchange:a-yaml-label-is-plain-and-a-yaml-triple-is-in-flow-form´
    /// ´test:unit:a-yaml-label-is-plain-and-a-yaml-triple-is-in-flow-form´
    #[test]
    fn a_yaml_label_is_plain_and_a_yaml_triple_is_in_flow_form() {
        let namespaced = |value: &str| yaml(&format!("namespace: {value}\nversion: [1, 0, 0]\n"));
        let versioned = |value: &str| yaml(&format!("namespace: a.b\nversion: {value}\n"));

        // The quoted forms fail where TOML's basic string is canonical.
        assert_eq!(
            namespaced("\"a.b\""),
            vec![Defect::NotLabel(String::from("\"a.b\""))]
        );
        assert_eq!(
            namespaced("'a.b'"),
            vec![Defect::NotLabel(String::from("'a.b'"))]
        );
        assert_eq!(
            namespaced("com"),
            vec![Defect::NotLabel(String::from("com"))]
        );

        // Flow form is required for one spelling; the block form is quoted back.
        assert_eq!(
            yaml("namespace: a.b\nversion:\n  - 1\n  - 0\n  - 0\n"),
            vec![Defect::NotTriple(String::from("- 1 - 0 - 0"))]
        );

        // A leading-zero member and an underscore-separated one.
        assert_eq!(
            versioned("[010, 0, 0]"),
            vec![Defect::NotTriple(String::from("[010, 0, 0]"))]
        );
        assert_eq!(
            versioned("[1_0, 0, 0]"),
            vec![Defect::NotTriple(String::from("[1_0, 0, 0]"))]
        );
        assert_eq!(
            versioned("[1, 0]"),
            vec![Defect::NotTriple(String::from("[1, 0]"))]
        );

        // Both keys absent from a mapping that is otherwise fine.
        assert_eq!(
            yaml("owners: []\n"),
            vec![Defect::NoNamespace, Defect::NoVersion]
        );
    }

    /// The program reads the file type from the name and never sniffs a file to
    /// guess what it is. A type it knows is parsed as that carrier; a type it
    /// does not know is ignored, with no verdict, no finding and no burn row.
    /// The remedy for an ungovernable file inside an include row is still the
    /// exclude row somebody should have written, and what the one-family ruling
    /// changed is that the linter no longer announces the omission.
    ///
    /// ´claim:interchange:the-program-reads-the-type-from-the-name-and-never-sniffs´
    /// ´test:unit:the-program-reads-the-type-from-the-name-and-never-sniffs´
    #[test]
    fn the_program_reads_the_type_from_the_name_and_never_sniffs() {
        assert_eq!(
            carrier(&path("share/default/config/index.toml")),
            Some(Carrier::Toml)
        );
        assert_eq!(carrier(&path(".github/labels.json")), Some(Carrier::Json));
        assert_eq!(carrier(&path("codecov.yaml")), Some(Carrier::Yaml));
        assert_eq!(
            carrier(&path(".github/workflows/ci.yml")),
            Some(Carrier::Yaml)
        );

        // A type the program does not know is ignored rather than judged.
        assert_eq!(carrier(&path("packages/futuredev/config/logo.png")), None);
        assert_eq!(carrier(&path("packages/linter/src/interchange.rs")), None);

        // The lock is TOML by shape and leaves the governed set by a named row
        // rather than by the catalog, so the catalog does not claim it.
        assert_eq!(carrier(&path("Cargo.lock")), None);

        // A suffix wants a stem before it, so a bare dotfile of that name is not
        // a document of the carrier the suffix would otherwise name.
        assert_eq!(carrier(&path(".toml")), None);
    }

    /// A declared file whose envelope is defective refuses the command, in the
    /// class a lexical error already occupies. The check is prior to the
    /// declaration that would authorize it, and that is exactly right: the
    /// requirement holds of the declared files because they are the
    /// configuration, not because the configuration says so.
    ///
    /// One consequence is a guarantee rather than an accident: no declared file
    /// can ever appear in a burn row, because debt requires a tolerated failing
    /// run and a declared file with a bad envelope produces no run at all.
    ///
    /// ´claim:interchange:a-declared-file-with-a-defective-envelope-refuses-the-command´
    /// ´test:unit:a-declared-file-with-a-defective-envelope-refuses-the-command´
    #[test]
    fn a_declared_file_with_a_defective_envelope_refuses_the_command() {
        let root = TempDir::new().expect("a temporary root");
        let directory = root.path();

        std::fs::write(directory.join("owners.toml"), CONFORMING).expect("a file");
        std::fs::write(directory.join("lists.toml"), "owners = []\n").expect("a file");
        std::fs::write(
            directory.join("policies.toml"),
            "version = [1, 0, 0]\nnamespace = \"a.b\"\n",
        )
        .expect("a file");

        let refusals =
            declared_refusals(directory, &["owners.toml", "lists.toml", "policies.toml"]);

        // The conforming file earns no refusal; the two defective ones do, and
        // every defect earns the one text, because a file that has not named
        // itself has made no claim to be misread in some particular way.
        assert_eq!(
            refusals,
            [
                "declared configuration: lists.toml: no envelope; a declared file carries the envelope it requires",
                "declared configuration: policies.toml: no envelope; a declared file carries the envelope it requires",
            ]
        );

        // An absent member is the loader's own refusal and not this check's.
        assert_eq!(
            declared_refusals(directory, &["absent.toml"]),
            [] as [String; 0]
        );
    }

    /// Each defect carries the finding text the policy fixes for it, and the
    /// carrier decides two things about that text: the word for a root entry,
    /// which is *member* in JSON and *key* in the other two, and the format's own
    /// name in the parse form. The parse form says *not a well-formed X
    /// document* rather than *not an X document*, because under one family the
    /// two readings came apart — a file the program cannot identify is ignored
    /// and says nothing, while a file of a known type that will not parse fails
    /// and says this.
    ///
    /// ´claim:interchange:each-defect-carries-the-finding-text-its-carrier-fixes´
    /// ´test:unit:each-defect-carries-the-finding-text-its-carrier-fixes´
    #[test]
    fn each_defect_carries_the_finding_text_its_carrier_fixes() {
        // The forms common to all three, reading member for key in JSON.
        assert_eq!(
            message(&Defect::NoNamespace, Carrier::Toml),
            "no namespace key; a governed document names itself"
        );
        assert_eq!(
            message(&Defect::NoNamespace, Carrier::Json),
            "no namespace member; a governed document names itself"
        );
        assert_eq!(
            message(&Defect::NoVersion, Carrier::Yaml),
            "no version key; a governed document stamps its schema"
        );
        assert_eq!(
            message(&Defect::NotLabel(String::from("'a.b'")), Carrier::Toml),
            "namespace 'a.b' is not a namespace label"
        );
        assert_eq!(
            message(&Defect::NotTriple(String::from("[1, 0]")), Carrier::Toml),
            "version [1, 0] is not a triple of canonical non-negative integers"
        );
        assert_eq!(
            message(&Defect::Reserved(VERSION_KEY), Carrier::Toml),
            "version is reserved for the envelope and carries content"
        );

        // The form that belongs to TOML alone, and the three parse forms.
        assert_eq!(
            message(&Defect::WrongOrder, Carrier::Toml),
            "the envelope keys stand in the wrong order"
        );
        assert_eq!(
            message(&Defect::NotWellFormed, Carrier::Toml),
            "not a well-formed TOML document"
        );
        assert_eq!(
            message(&Defect::NotWellFormed, Carrier::Json),
            "not a well-formed JSON document"
        );
        assert_eq!(
            message(&Defect::NotWellFormed, Carrier::Yaml),
            "not a well-formed YAML document"
        );

        // The forms each of the other two carriers adds of its own.
        assert_eq!(
            message(&Defect::RootNotObject, Carrier::Json),
            "the document's root is not an object"
        );
        assert_eq!(
            message(&Defect::Duplicated(NAMESPACE_KEY), Carrier::Json),
            "namespace appears more than once in the root object"
        );
        assert_eq!(
            message(&Defect::RootNotMapping, Carrier::Yaml),
            "the document's root is not a mapping"
        );
        assert_eq!(
            message(&Defect::Documents(2), Carrier::Yaml),
            "the stream holds 2 documents; an envelope names one"
        );
        assert_eq!(
            message(&Defect::Indirect(NAMESPACE_KEY), Carrier::Yaml),
            "the envelope key namespace is written through an alias, a merge or a tag"
        );
    }

    /// One row of one list.
    fn row(name: &str, pattern: &str) -> SectionRow {
        SectionRow {
            name: String::from(name),
            pattern: AbnfPattern::parse(pattern).expect("a compilable pattern"),
        }
    }

    /// The fixture owner every section below belongs to.
    const OWNER: &str = "INDEX";

    /// One owner's section, given as its exclude rows and a declared gloss.
    fn parameters(exclude: Vec<SectionRow>, include: Vec<SectionRow>) -> Parameters {
        Parameters {
            sections: BTreeMap::from([(
                String::from(OWNER),
                Section {
                    exclude,
                    include: Some(include),
                },
            )]),
        }
    }

    /// One owner's section as the ruling has a repository write it: exclusions
    /// only, and no gloss to be held to.
    fn unglossed(exclude: Vec<SectionRow>) -> Parameters {
        Parameters {
            sections: BTreeMap::from([(
                String::from(OWNER),
                Section {
                    exclude,
                    include: None,
                },
            )]),
        }
    }

    /// Attribute every one of these paths to the fixture owner.
    fn accounted(paths: &[BytePath]) -> BTreeMap<&BytePath, &str> {
        paths.iter().map(|path| (path, OWNER)).collect()
    }

    /// The codes a run of findings carries, in the order it reports them.
    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(Finding::code).collect()
    }

    /// Write a tree of files under a temporary root, creating each parent.
    fn tree(files: &[(&str, &str)]) -> TempDir {
        let root = TempDir::new().expect("a temporary root");

        for (name, body) in files {
            let full = root.path().join(name);

            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).expect("a parent directory");
            }

            std::fs::write(full, body).expect("a file");
        }

        root
    }

    /// The section removes the union of its exclude rows, and what survives is
    /// the governed set. Where the section also declares an include gloss, that
    /// gloss is checked against the set exclusion already computed: a governed
    /// path no gloss row names leaves it incomplete, one two rows name leaves it
    /// overlapping, and a row reaching nothing in the governed set is idle. None
    /// of the three moves a path — the file is governed either way, because
    /// exclusion alone decided that. The two passes stay separate, so an overlap
    /// between an exclude row and a gloss row is legal and is how a named
    /// foreign-schema row sits over a broad gloss row without either being
    /// narrowed to accommodate the other. Containment is not among the failures
    /// because a row is only ever offered its own owner's paths: a row written
    /// for another owner's file reaches nothing and is idle.
    ///
    /// ´claim:interchange:a-section-excludes-then-partitions-what-survives´
    /// ´test:unit:a-section-excludes-then-partitions-what-survives´
    #[test]
    fn a_section_excludes_then_partitions_what_survives() {
        let paths = [
            path("Cargo.toml"),
            path(".linter/owners.toml"),
            path(".vscode/settings.json"),
            path("docs/notes.md"),
        ];
        let declared = parameters(
            vec![
                row("cargo-manifest", "%s\"Cargo.toml\""),
                row(
                    "editor-config",
                    "%s\".vscode/\" 1*( %x21-2E / %x30-7E ) %s\".json\"",
                ),
            ],
            vec![row(
                "linter-config",
                "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let (governed, findings) = govern(&declared, &accounted(&paths));

        // Two foreign schemas leave by name and the declared file is governed.
        // The prose file needs no row at all: it is out of domain, so the
        // section is complete without mentioning it and is silent about it.
        assert_eq!(governed.len(), 1, "{governed:?}");
        assert_eq!(governed[0].path.display(), ".linter/owners.toml");
        assert!(findings.is_empty(), "{findings:?}");

        // A governed path no gloss row names leaves the declared partition
        // incomplete. The path is governed all the same: what failed is the
        // declaration about it, not its governance.
        let narrow = parameters(
            Vec::new(),
            vec![row(
                "service-config",
                "%s\"share\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let paths = [path("packages/futuredev/config/settings.toml")];
        let (governed, findings) = govern(&narrow, &accounted(&paths));

        assert_eq!(governed.len(), 1, "{governed:?}");
        assert!(
            governed[0].row.is_none(),
            "a gloss that failed over a path names no row"
        );
        assert_eq!(
            codes(&findings),
            ["interchange_gloss_uncovered", "interchange_idle_row"]
        );
        assert_eq!(
            findings[0].to_string(),
            "interchange section: packages/futuredev/config/settings.toml: governed under INDEX \
             and named by no include row; the declared include gloss does not cover the governed set"
        );
        assert_eq!(
            findings[1].to_string(),
            "interchange section: INDEX include row service-config: pattern matches no governed path"
        );

        // Two gloss rows reaching one path break disjointness, and there is no
        // priority: an exception is carved by writing disjoint rows. The path is
        // governed here too, and the gloss simply fails to say so once.
        let shadowed = parameters(
            Vec::new(),
            vec![
                row("broad", "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\""),
                row("narrow", "%s\".linter/owners.toml\""),
            ],
        );
        let paths = [path(".linter/owners.toml")];
        let (governed, findings) = govern(&shadowed, &accounted(&paths));

        assert_eq!(governed.len(), 1, "{governed:?}");
        assert!(
            governed[0].row.is_none(),
            "two rows name it, so no one row does"
        );
        assert_eq!(codes(&findings), ["interchange_multiply_included"]);
        assert_eq!(
            findings[0].to_string(),
            "interchange section: .linter/owners.toml: matched by 2 INDEX include rows: \
             broad : %s\".linter\" [ \"/\" *VCHAR ] %s\".toml\", narrow : %s\".linter/owners.toml\""
        );

        // A row is never offered another owner's path, so containment is not a
        // check that passes here but a question the shape of the evaluation
        // removes: the row reaches nothing, and it is idle rather than foreign.
        let reaching = parameters(
            Vec::new(),
            vec![row(
                "linter-config",
                "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let path_of = path(".linter/owners.toml");
        let elsewhere = BTreeMap::from([(&path_of, "ASSAYER")]);
        let (governed, findings) = govern(&reaching, &elsewhere);

        assert!(governed.is_empty(), "{governed:?}");
        assert_eq!(codes(&findings), ["interchange_idle_row"]);
        assert_eq!(
            findings[0].to_string(),
            "interchange section: INDEX include row linter-config: pattern matches no governed path"
        );
    }

    /// A symbolic link is never a document carrier, so a section leaving one in
    /// its governed set is named at configuration time and the remedy is an
    /// exclude row. A file of a type the program does not know never enters the
    /// universe at all: it is out of domain, so no row reaches it, no verdict is
    /// formed and no burn row can name it. A repository cannot ask for the loud
    /// version by writing a narrow gloss row, because the file the row would
    /// leave unnamed is not in the set the gloss is judged over.
    ///
    /// ´claim:interchange:a-link-carries-no-document-and-an-unknown-type-is-ignored´
    /// ´test:unit:a-link-carries-no-document-and-an-unknown-type-is-ignored´
    #[test]
    fn a_link_carries_no_document_and_an_unknown_type_is_ignored() {
        let root = tree(&[
            ("config/index.toml", CONFORMING),
            ("config/logo.png", "not a document"),
        ]);
        std::os::unix::fs::symlink("index.toml", root.path().join("config/link.toml"))
            .expect("a symbolic link");

        let paths = [
            path("config/index.toml"),
            path("config/logo.png"),
            path("config/link.toml"),
        ];
        let declared = parameters(
            Vec::new(),
            vec![row("service-config", "%s\"config\" [ \"/\" *VCHAR ]")],
        );
        let (governed, findings) = govern(&declared, &accounted(&paths));

        // The catch-all include row reaches the image's path and governs it all
        // the same — the filter is over the universe and not over the rows.
        assert_eq!(governed.len(), 2, "{governed:?}");
        assert!(findings.is_empty(), "{findings:?}");

        // The link is named; the image is not, because it is no part of this
        // policy's universe and a finding for it would be about another policy.
        let carrier_findings = carriers(root.path(), &governed);

        assert_eq!(codes(&carrier_findings), ["interchange_linked_path"]);
        assert_eq!(
            carrier_findings[0].to_string(),
            "interchange section: config/link.toml: governed under INDEX, glossed by include row service-config; \
             a symbolic link carries no document"
        );

        // And neither the link nor the image reaches the envelope verdict.
        assert_eq!(
            conform(root.path(), &governed, &BTreeSet::new()),
            [] as [Finding; 0]
        );
    }

    /// Exclusion alone computes governance, and a declared include list is a
    /// diagnostic gloss on the set it computed rather than an instrument that
    /// forms it. A section declaring no gloss owes nothing and governs the whole
    /// of what its exclusions leave; a section declaring a correct one governs
    /// exactly the same paths and adds a name to each. So the governed set is
    /// identical with the gloss and without it, which is the sense in which the
    /// include rows were doing nothing. A false gloss — incomplete, overlapping,
    /// or padded with a row reaching only excluded or out-of-domain files — is a
    /// finding against the declaration, and the governed set does not move.
    ///
    /// ´claim:interchange:the-include-partition-is-a-gloss-and-never-governs´
    /// ´test:unit:the-include-partition-is-a-gloss-and-never-governs´
    #[test]
    #[allow(clippy::too_many_lines)]
    fn the_include_partition_is_a_gloss_and_never_governs() {
        let paths = [
            path("Cargo.toml"),
            path(".linter/owners.toml"),
            path(".linter/policies.toml"),
            path("src/main.rs"),
        ];
        let excludes = || vec![row("cargo-manifest", "%s\"Cargo.toml\"")];

        // No gloss at all: the governed set is the in-domain share minus the
        // exclusions, the Rust source is out of domain and needs no row, and
        // nothing is owed for the absence.
        let plain = unglossed(excludes());
        let (bare, findings) = govern(&plain, &accounted(&paths));
        let bare_set: Vec<String> = bare.iter().map(|entry| entry.path.display()).collect();

        assert_eq!(bare_set, [".linter/owners.toml", ".linter/policies.toml"]);
        assert!(
            bare.iter().all(|entry| entry.row.is_none()),
            "no gloss names anything"
        );
        assert!(
            findings.is_empty(),
            "an absent gloss owes nothing: {findings:?}"
        );

        // The same section with a correct gloss governs exactly the same paths.
        // The rows buy a name per path and change no verdict.
        let correct = parameters(
            excludes(),
            vec![row(
                "linter-config",
                "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let (glossed, findings) = govern(&correct, &accounted(&paths));
        let glossed_set: Vec<String> = glossed.iter().map(|entry| entry.path.display()).collect();

        assert_eq!(
            glossed_set, bare_set,
            "the gloss moved no path into or out of governance"
        );
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(
            glossed
                .iter()
                .map(|entry| entry.row.as_deref())
                .collect::<Vec<Option<&str>>>(),
            [Some("linter-config"), Some("linter-config")]
        );

        // An incomplete gloss names one of the two and fails over the other, and
        // both stay governed.
        let incomplete = parameters(
            excludes(),
            vec![row("owner-file", "%s\".linter/owners.toml\"")],
        );
        let (governed, findings) = govern(&incomplete, &accounted(&paths));

        assert_eq!(
            governed
                .iter()
                .map(|entry| entry.path.display())
                .collect::<Vec<String>>(),
            bare_set,
            "an incomplete gloss governs what a correct one would"
        );
        assert_eq!(codes(&findings), ["interchange_gloss_uncovered"]);
        assert_eq!(
            findings[0].to_string(),
            "interchange section: .linter/policies.toml: governed under INDEX and named by no include row; \
             the declared include gloss does not cover the governed set"
        );

        // An overlapping gloss covers everything twice over, which is the other
        // half of the partition judgment.
        let overlapping = parameters(
            excludes(),
            vec![
                row(
                    "linter-config",
                    "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
                ),
                row("owner-file", "%s\".linter/owners.toml\""),
            ],
        );
        let (governed, findings) = govern(&overlapping, &accounted(&paths));

        assert_eq!(governed.len(), 2, "{governed:?}");
        assert_eq!(codes(&findings), ["interchange_multiply_included"]);

        // A gloss row naming a file the exclusions removed reaches nothing it
        // could partition, and a row naming a file outside the domain reaches
        // nothing at all. Both are idle: the gloss is judged over the governed
        // set, so a row reaching beside it is a dead row and is named as one.
        let padded = parameters(
            excludes(),
            vec![
                row(
                    "linter-config",
                    "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
                ),
                row("manifest-gloss", "%s\"Cargo.toml\""),
                row("source-gloss", "%s\"src/\" *VCHAR %s\".rs\""),
            ],
        );
        let (governed, findings) = govern(&padded, &accounted(&paths));

        assert_eq!(governed.len(), 2, "{governed:?}");
        assert_eq!(
            codes(&findings),
            ["interchange_idle_row", "interchange_idle_row"]
        );
        assert_eq!(
            findings
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<String>>(),
            [
                "interchange section: INDEX include row manifest-gloss: pattern matches no governed path",
                "interchange section: INDEX include row source-gloss: pattern matches no governed path",
            ]
        );

        // An exclude row overlapping a gloss row stays legal, because a path the
        // exclusions removed is never offered to the gloss at all.
        let overlapping_lists = parameters(
            vec![
                row("cargo-manifest", "%s\"Cargo.toml\""),
                row("owner-file", "%s\".linter/owners.toml\""),
            ],
            vec![row(
                "linter-config",
                "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let (governed, findings) = govern(&overlapping_lists, &accounted(&paths));

        assert_eq!(
            governed
                .iter()
                .map(|entry| entry.path.display())
                .collect::<Vec<String>>(),
            [".linter/policies.toml"]
        );
        assert!(findings.is_empty(), "{findings:?}");
    }

    /// The universe is pre-filtered by the carrier catalog, and every coverage
    /// judgment is taken over what survives that filter. An out-of-domain path is
    /// governed by nothing, excluded by nothing and reported as nothing, whatever
    /// rows stand; an in-domain path a declared gloss does not name is governed
    /// and the gloss is what fails; and a row whose pattern can only reach
    /// out-of-domain paths reaches nothing at all and is idle. The three cases
    /// are one boundary read from both sides, which is why they are proved
    /// together.
    ///
    /// ´claim:interchange:the-universe-is-the-catalog-s-domain-and-a-row-outside-it-is-idle´
    /// ´test:unit:the-universe-is-the-catalog-s-domain-and-a-row-outside-it-is-idle´
    #[test]
    fn the_universe_is_the_catalog_s_domain_and_a_row_outside_it_is_idle() {
        // An owner whose whole share is out of domain writes no row and is
        // complete. This is the polarity: a section is not obliged to say of
        // every file that it is none of this policy's business.
        let empty = parameters(Vec::new(), Vec::new());
        let outside = [path("src/main.rs"), path("README.md"), path("Cargo.lock")];
        let (governed, findings) = govern(&empty, &accounted(&outside));

        assert!(governed.is_empty(), "{governed:?}");
        assert!(findings.is_empty(), "{findings:?}");

        // One in-domain path in the same section is governed, and the empty gloss
        // the section declares is the false claim that there was nothing to
        // govern. That is the other side of the same boundary.
        let inside = [path("src/main.rs"), path("cspell.json")];
        let (governed, findings) = govern(&empty, &accounted(&inside));

        assert_eq!(governed.len(), 1, "{governed:?}");
        assert_eq!(governed[0].path.display(), "cspell.json");
        assert_eq!(codes(&findings), ["interchange_gloss_uncovered"]);
        assert_eq!(
            findings[0].to_string(),
            "interchange section: cspell.json: governed under INDEX and named by no include row; \
             the declared include gloss does not cover the governed set"
        );

        // An exclude row that can only reach out-of-domain paths reaches nothing
        // and is idle, so the honest core of a section cannot be padded with
        // rows for files the policy never asks about.
        let padded = parameters(
            vec![row("rust-source", "%s\"src/\" *VCHAR %s\".rs\"")],
            vec![row(
                "linter-config",
                "%s\".linter\" [ \"/\" *VCHAR ] %s\".toml\"",
            )],
        );
        let paths = [path("src/main.rs"), path(".linter/owners.toml")];
        let (governed, findings) = govern(&padded, &accounted(&paths));

        assert_eq!(governed.len(), 1, "{governed:?}");
        assert_eq!(codes(&findings), ["interchange_idle_row"]);
        assert_eq!(
            findings[0].to_string(),
            "interchange section: INDEX exclude row rust-source: pattern matches no accounted path"
        );

        // An include row is idle on the same ground, and the audit that explains
        // exclusions reads the same universe: an out-of-domain path was never in
        // the governed set, so no row can have removed it from one.
        let reaching = parameters(
            vec![row("prose", "%s\"README.md\"")],
            vec![row("image", "*VCHAR %s\".png\"")],
        );
        let paths = [path("README.md"), path("logo.png")];
        let (_governed, findings) = govern(&reaching, &accounted(&paths));

        assert_eq!(
            codes(&findings),
            ["interchange_idle_row", "interchange_idle_row"]
        );
        assert_eq!(exclusions(&reaching, &accounted(&paths), "INDEX"), []);
    }

    /// A governed document is judged in the carrier its name names, and every
    /// carrier's verdict reaches one report. A path a list tolerates is silent
    /// however it fails, and the violation identity is the path: a file failing
    /// three ways is one debt and not three, which is what the path-set codec
    /// says.
    ///
    /// ´claim:interchange:a-governed-document-is-judged-in-the-carrier-its-name-names´
    /// ´test:unit:a-governed-document-is-judged-in-the-carrier-its-name-names´
    #[test]
    fn a_governed_document_is_judged_in_the_carrier_its_name_names() {
        let root = tree(&[
            ("config/good.toml", CONFORMING),
            ("config/bad.toml", "owners = []\n"),
            ("config/bad.json", "[1, 2]"),
            ("config/bad.yaml", "owners: []\n"),
        ]);

        let paths = [
            path("config/bad.json"),
            path("config/bad.toml"),
            path("config/bad.yaml"),
            path("config/good.toml"),
        ];
        let declared = parameters(
            Vec::new(),
            vec![row("service-config", "%s\"config\" [ \"/\" *VCHAR ]")],
        );
        let (governed, findings) = govern(&declared, &accounted(&paths));

        assert!(findings.is_empty(), "{findings:?}");

        let judged = conform(root.path(), &governed, &BTreeSet::new());
        let texts: Vec<String> = judged.iter().map(ToString::to_string).collect();

        assert_eq!(
            texts,
            [
                "interchange envelope: config/bad.json: the document's root is not an object",
                "interchange envelope: config/bad.toml: no namespace key; a governed document names itself",
                "interchange envelope: config/bad.toml: no version key; a governed document stamps its schema",
                "interchange envelope: config/bad.yaml: no namespace key; a governed document names itself",
                "interchange envelope: config/bad.yaml: no version key; a governed document stamps its schema",
            ]
        );

        // The TOML file fails two ways and is one violation, because the identity
        // is the path and a file holds at most one.
        let violating: Vec<String> = violating_paths(root.path(), &governed)
            .iter()
            .map(|path| path.display())
            .collect();

        assert_eq!(
            violating,
            ["config/bad.json", "config/bad.toml", "config/bad.yaml"]
        );

        // A tolerated path is silent however it fails.
        let listed = path("config/bad.toml");
        let tolerated = BTreeSet::from([&listed]);
        let judged = conform(root.path(), &governed, &tolerated);

        assert_eq!(
            codes(&judged),
            [
                "interchange_envelope",
                "interchange_envelope",
                "interchange_envelope"
            ]
        );
    }
}
