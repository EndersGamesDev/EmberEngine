// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The augmented Backus-Naur form of RFC 5234: a rule-list parser and a matcher.
//!
//! One pattern language spans the whole declared configuration surface and it is
//! this one (´lang:isolation:patterns´), so the checker carries the engine for it
//! rather than borrowing a second dialect from a crate. What stands here is the
//! engine alone: a parser over rule lists, and a decision procedure asking
//! whether an input string derives from a named rule. The declared surface's
//! patterns are read through this engine.
//!
//! The form has no capture semantics, and the designs written over it are built
//! so that none is needed: a value a program must read back is a declared entry
//! of its own rather than a fragment a match hands over
//! (´gram:isolation:declaration´). An engine offering captures would therefore
//! be offering a door the grammar cannot open.
//!
//! Eight readings are settled here, because each is a place two readers would
//! otherwise read one rule differently.
//!
//! *A match is over the whole input.* A rule matches a string when some
//! derivation spans it end to end. The configuration's patterns decide paths
//! totally — a path is inside a bound or outside it — so a rule reaching only a
//! prefix of a path has not reached the path, and the anchoring is the matcher's
//! rather than something each rule must spell for itself.
//!
//! *A quoted string ignores case, and `%s` binds it.* Case insensitivity is the
//! form's own rule for a quoted string, so ``"a"`` admits `A`, and the
//! consequence reaches the core rules too: ``HEXDIG`` spells its letters as
//! quoted strings and therefore admits lowercase. A configuration naming a path
//! wants the other reading, and RFC 7405 is the standard door to it: ``%s``
//! prefixes a case-sensitive string and ``%i`` names the default aloud. So a
//! manifest is named under the prefix and matches nothing spelled otherwise.
//!
//! *A prose value is refused where it is written.* The form admits ``<...>`` as
//! a placeholder for a rule stated in English, which no engine can match. A
//! pattern carrying one is a declaration whose meaning is not in the file, so it
//! is an error rather than a rule that quietly matches nothing.
//!
//! *The core rules stand predefined.* Appendix B.1's sixteen rules are available
//! to every rule list without being restated, which is what lets a declared
//! pattern spell a path tail as ``*VCHAR``. A rule list may define its own rule
//! of the same name, which shadows the core one for that grammar, and ``=/``
//! extends whichever definition stands.
//!
//! *Left recursion is refused rather than silently narrowed.* A rule that can
//! reference itself without consuming input has no least fixed point a
//! backtracking matcher reaches, and cutting the branch would leave the rule
//! matching a smaller language than it spells. The check is static: a rule is
//! refused when it stands in its own left-corner closure, computed over the
//! nullable rules.
//!
//! *Repetition over an empty match terminates.* An iteration matching the empty
//! string reaches no position the previous one did not, and repeating it
//! satisfies any remaining minimum without moving, so the only end such an
//! iteration contributes is the position it started at. The matcher takes that
//! end and stops iterating; every iteration it does recurse on consumes at least
//! one character, so the recursion is bounded by the input's length. Without
//! that, ``*[ "x" ]`` would not terminate on an input it cannot match.
//!
//! *An indented line beginning a rule begins a rule.* The form continues a rule
//! across a line ending followed by whitespace, so an indented rule list would
//! read as one long rule under the letter of it — and then refuse, because a
//! defining symbol may not stand within a rule. The reading taken here is the
//! lookahead: a line whose first tokens are a rule name and a defining symbol
//! starts a rule. It costs no rule list the standard admits, because every line
//! it decides differently is one the standard refuses, and it earns a rule list
//! that can be written indented inside a declaration that has its own layout.
//!
//! *Backtracking is exhaustive rather than linear.* The matcher explores
//! alternatives and repetition counts until one derivation spans the input, so a
//! pathological grammar can cost exponentially in the input's length. The
//! patterns the surface writes are path-shaped and shallow, and the honest
//! record of the cost belongs here rather than in a promise the engine does not
//! keep.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`parses_every_construct_the_form_defines`] | abnf | The parser reads every construct RFC 5234 states — the rule name, both defining symbols, alternation, concatenation, the three repetition spellings, the group, the option, the quoted string, the three numeric bases in their value, sequence and range forms, the comment, and a reference to a core rule — so a declaration written to the standard is read by this engine rather than by a subset of it. |
//! | [`comments_and_continuations_are_read_as_the_form_reads_them`] | abnf | A comment runs to the end of its line and a line ending followed by whitespace continues the rule it interrupts, so a rule list may be laid out and annotated the way the standard's own examples are without changing what it means. An indented line whose first tokens are a rule name and a defining symbol starts a rule instead, which is the one place the letter of the form would read a rule list nobody could write indented. |
//! | [`refuses_source_that_is_not_the_form`] | abnf | Source outside the form is refused where it is written, with the position and what was expected there: an unterminated group or option, an alternation arm with nothing in it, a rule with no defining symbol, an unterminated quoted string, a repetition counting downward, and a numeric value naming no character. A pattern the engine cannot read is a defect of the declaration rather than a rule that matches nothing. |
//! | [`refuses_a_prose_value_rather_than_accepting_it_silently`] | abnf | A prose value is refused by its own error naming its position. The construct is lawful in the form and unmatchable by any engine, so accepting it would leave a declaration whose meaning is not in the file behaving as a rule that matches nothing at all. |
//! | [`refuses_a_rule_defined_twice_and_an_increment_with_no_base`] | abnf | A name defined twice is refused rather than resolved by order, and incremental alternatives extending a name nothing defines are refused rather than treated as a definition. Both are the same guard: a rule's meaning is assembled from declarations that agree about which rule they are about. |
//! | [`refuses_a_reference_no_rule_defines`] | abnf | A reference to a name neither the rule list nor the core rules define is refused when the grammar is built, naming the position where the reference stands. A forward reference is ordinary, so the check waits for the whole list rather than reading it in order. |
//! | [`refuses_a_rule_that_can_reference_itself_without_consuming`] | abnf | A left-recursive rule is refused, whether it references itself directly, through another rule, or behind a construct that can match the empty string. Matching it would take cutting the recursion, and a cut rule matches a smaller language than it spells while still reporting a verdict. |
//! | [`alternation_admits_its_arms_whatever_their_order`] | abnf | An alternation admits every string any arm admits, in whatever order the arms are written, and a shorter arm that would strand the rest of the rule is backtracked out of. Order-dependence here would make a declaration's meaning turn on how its author happened to lay the arms out. |
//! | [`repetition_bounds_are_inclusive_and_an_exact_count_binds`] | abnf | Both repetition bounds are inclusive, an omitted bound is unbounded on that side, and the exact spelling admits that count and no other. So a rule spelled to admit two or three of something admits two and three, and neither one nor four. |
//! | [`a_repetition_over_an_empty_match_terminates`] | abnf | Repetition over a rule that can match the empty string terminates and decides, both where the input derives from it and where it does not. The empty iteration contributes the position it started at and nothing else, and it satisfies a minimum count without moving, so the matcher neither loops nor loses the derivations that minimum admits. |
//! | [`a_quoted_string_ignores_case_and_the_sensitive_prefix_binds_it`] | abnf | A quoted string is case-insensitive as the form defines it, the RFC 7405 prefixes name each reading aloud, and the case-sensitive one admits exactly the spelling it carries. A pattern naming a path in the repository wants the sensitive reading and has a standard way to ask for it. |
//! | [`numeric_values_name_characters_ranges_and_sequences`] | abnf | A numeric value names a character in binary, decimal or hexadecimal, and its dotted form names a sequence while its hyphenated form names an inclusive range. That is how a rule reaches the characters a quoted string cannot carry, the separator and the line ending among them. |
//! | [`incremental_alternatives_extend_the_rule_they_name`] | abnf | Incremental alternatives add arms to a rule already defined, in the order they are written, and they extend a core rule as readily as one the list defines. A vocabulary can therefore be stated once and widened where the widening belongs, rather than restated whole. |
//! | [`the_core_rules_stand_predefined_and_referable`] | abnf | The sixteen core rules of Appendix B.1 are defined without being restated, and they mean what the appendix says: the visible characters, the digits, the letters, the line ending as a pair, and the hexadecimal digits admitting lowercase because the appendix spells its letters as quoted strings. |
//! | [`a_match_is_whole_input_and_anchored_at_both_ends`] | abnf | A rule matches a string when a derivation spans it entire, so a rule reaching only a prefix or only a suffix does not match. A configuration whose patterns decided paths by prefix would give every rule a reach its author never wrote. |
//! | [`an_unknown_rule_name_is_an_error_rather_than_a_refusal`] | abnf | Asking whether an input derives from a rule the grammar does not define is an error naming the rule, not the answer no. A rule name that resolves to nothing is a defect of whoever asked, and answering no would hide it behind a verdict about the input. |
//! | [`a_path_grammar_selects_a_subtree_and_stops_at_the_separator`] | abnf | A grammar written the way the declared surface writes one selects a directory and everything beneath it, and stops at the separator: a sibling whose name merely begins with the same characters is outside the bound. Both the register subtree and the linter-package exclusion of the ratified examples are read here. |
//! | [`a_path_grammar_names_a_file_at_any_depth`] | abnf | A grammar naming a file at any depth reaches the file wherever it stands, admits it at the root, and holds it to its exact spelling and to being the last component. Depth is written as a repetition over segments rather than as a wildcard that could swallow the separator. |
//! | [`a_rule_reports_the_literal_openings_its_arms_commit_to`] | abnf | A rule is read back for the literal openings its arms commit to, in the order they are written, and each opening says whether it is the whole of its arm or the beginning of a reach. A rule committing to nothing opens with the empty prefix, which is the true answer for a reach over everything and the only one a caller could act on without inventing a bound the author never wrote. |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One character of a terminal value, or of the input being matched.
type Ch = char;

/// The core rules of RFC 5234's Appendix B.1, in the form's own syntax.
///
/// They are parsed by the same reader every rule list goes through, so the
/// appendix is transcribed rather than reimplemented, and a defect in the
/// transcription is a defect the parser's own tests reach.
const fn core_rules() -> &'static str {
    r#"
ALPHA  = %x41-5A / %x61-7A
BIT    = "0" / "1"
CHAR   = %x01-7F
CR     = %x0D
CRLF   = CR LF
CTL    = %x00-1F / %x7F
DIGIT  = %x30-39
DQUOTE = %x22
HEXDIG = DIGIT / "A" / "B" / "C" / "D" / "E" / "F"
HTAB   = %x09
LF     = %x0A
LWSP   = *(WSP / CRLF WSP)
OCTET  = %x00-FF
SP     = %x20
VCHAR  = %x21-7E
WSP    = SP / HTAB
"#
}

/// Why a rule list is not a grammar.
///
/// Every defect but one carries the position it stands at, as a byte offset into
/// the rule list's own text: a declaration is repaired where it is written, and a
/// message that only said what was wrong would leave the reader searching for it.
/// Left recursion is the exception, because the defect is a rule's relation to
/// the whole list rather than anything at one offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarDefect {
    /// The text does not read as the construct that position calls for.
    Expected {
        /// The byte offset the reader stopped at.
        position: usize,
        /// What the form admits there.
        expected: &'static str,
    },
    /// A bound is well-formed as text and names nothing as a bound.
    Bound {
        /// The byte offset the bound starts at.
        position: usize,
        /// What is wrong with it.
        defect: &'static str,
    },
    /// A prose value stands where a matchable element must.
    ProseValue {
        /// The byte offset the value starts at.
        position: usize,
    },
    /// A reference names a rule neither the list nor the core rules define.
    UndefinedRule {
        /// The byte offset the reference stands at.
        position: usize,
        /// The name that resolves to nothing.
        name: String,
    },
    /// One name carries two definitions.
    DuplicateRule {
        /// The byte offset the second definition starts at.
        position: usize,
        /// The name defined twice.
        name: String,
    },
    /// Incremental alternatives extend a rule that has no definition to extend.
    IncrementWithoutBase {
        /// The byte offset the increment starts at.
        position: usize,
        /// The name nothing defines.
        name: String,
    },
    /// A rule can reference itself without consuming input.
    LeftRecursion {
        /// The rule standing in its own left-corner closure.
        name: String,
    },
}

impl GrammarDefect {
    /// The byte offset the defect stands at, where the defect has one.
    #[must_use]
    pub const fn position(&self) -> Option<usize> {
        match self {
            Self::Expected { position, .. }
            | Self::Bound { position, .. }
            | Self::ProseValue { position }
            | Self::UndefinedRule { position, .. }
            | Self::DuplicateRule { position, .. }
            | Self::IncrementWithoutBase { position, .. } => Some(*position),
            Self::LeftRecursion { .. } => None,
        }
    }
}

impl fmt::Display for GrammarDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expected { position, expected } => {
                write!(formatter, "expected {expected} at byte {position}")
            }
            Self::Bound { position, defect } => {
                write!(formatter, "the bound at byte {position} {defect}")
            }
            Self::ProseValue { position } => {
                write!(
                    formatter,
                    "the prose value at byte {position} states a rule no matcher can read"
                )
            }
            Self::UndefinedRule { position, name } => {
                write!(
                    formatter,
                    "the reference to `{name}` at byte {position} names no rule"
                )
            }
            Self::DuplicateRule { position, name } => {
                write!(
                    formatter,
                    "the rule `{name}` is defined again at byte {position}"
                )
            }
            Self::IncrementWithoutBase { position, name } => {
                write!(
                    formatter,
                    "the increment at byte {position} extends `{name}`, which is not defined"
                )
            }
            Self::LeftRecursion { name } => {
                write!(
                    formatter,
                    "the rule `{name}` can reference itself without consuming input"
                )
            }
        }
    }
}

impl serde::Serialize for GrammarDefect {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A question asked about a rule the grammar does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownRule {
    name: String,
}

impl UnknownRule {
    /// The name that resolved to nothing, as the caller spelled it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for UnknownRule {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "no rule named `{}` is defined", self.name)
    }
}

impl serde::Serialize for UnknownRule {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// One element of a rule's right-hand side.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// Any one of the arms, tried in the order they are written.
    Alternation(Vec<Self>),
    /// Every item in turn.
    Concatenation(Vec<Self>),
    /// The inner element, between the two counts; an absent maximum is unbounded.
    Repetition {
        minimum: u32,
        maximum: Option<u32>,
        inner: Box<Self>,
    },
    /// Another rule of the same grammar, named in the form's own case-insensitive way.
    Reference { name: String, position: usize },
    /// A quoted string, matched with or without regard to case.
    Literal {
        characters: Vec<Ch>,
        sensitive: bool,
    },
    /// A numeric value, or the dotted sequence of them.
    Values(Vec<Ch>),
    /// An inclusive numeric range.
    Range { low: Ch, high: Ch },
}

/// One literal opening a rule commits to, and how much of an arm it is.
///
/// The distinction is the whole value of reading openings at all: a caller
/// resolving a pattern to places needs to know whether it was handed the name of
/// a thing or the beginning of a reach, and guessing from the spelling would make
/// a file with no extension a directory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Branch {
    text: String,
    complete: bool,
}

impl Branch {
    /// The opening as the pattern spells it.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the opening is the whole of what its arm admits.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// How many openings one node is read for before the reading gives up.
///
/// A concatenation over alternations multiplies, and a pattern written to
/// enumerate a large vocabulary would multiply out to a list nobody asked for.
/// Past the bound the reading falls back to the empty prefix, which is always a
/// true answer and never a useful one — the same answer a pattern reaching
/// everything gets.
const OPENING_BOUND: usize = 64;

/// What a node commits every string it admits to beginning with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Openings {
    branches: Vec<Branch>,
    complete: bool,
}

impl Openings {
    /// The reading of a node admitting the empty string and nothing else.
    fn nothing() -> Self {
        Self::exactly(String::new())
    }

    /// The reading of a node committing to no opening at all.
    fn anything() -> Self {
        Self {
            branches: vec![Branch {
                text: String::new(),
                complete: false,
            }],
            complete: false,
        }
    }

    /// The reading of a node admitting exactly one spelling.
    fn exactly(text: String) -> Self {
        Self {
            branches: vec![Branch {
                text,
                complete: true,
            }],
            complete: true,
        }
    }

    /// This reading with another's openings written after each of its own.
    fn followed_by(self, next: &Self) -> Self {
        if self.branches.len().saturating_mul(next.branches.len()) > OPENING_BOUND {
            return Self::anything();
        }

        let branches = self
            .branches
            .iter()
            .flat_map(|opening| {
                next.branches.iter().map(move |addition| Branch {
                    text: format!("{}{}", opening.text, addition.text),
                    complete: addition.complete,
                })
            })
            .collect();

        Self {
            branches,
            complete: next.complete,
        }
    }
}

/// A parsed rule list, with the core rules standing beside it.
///
/// Construction goes through [`Grammar::parse`], which refuses a list carrying an
/// unreadable construct, a reference to nothing, or a left-recursive rule, so a
/// grammar that exists is one the matcher decides with.
#[derive(Debug, Clone)]
pub struct Grammar {
    rules: BTreeMap<String, Node>,
}

impl Grammar {
    /// Parse a rule list over the core rules of Appendix B.1.
    ///
    /// # Errors
    ///
    /// Returns the defect when the text is not a rule list of the form, when a
    /// reference names no rule, or when a rule can reference itself without
    /// consuming input.
    pub fn parse(text: &str) -> Result<Self, GrammarDefect> {
        let mut rules = BTreeMap::new();

        read_rules(core_rules(), &mut rules, &mut BTreeSet::new())?;
        read_rules(text, &mut rules, &mut BTreeSet::new())?;
        validate(&rules)?;

        Ok(Self { rules })
    }

    /// Whether the grammar defines a rule of that name.
    #[must_use]
    pub fn defines(&self, rule: &str) -> bool {
        self.rules.contains_key(&rule.to_ascii_lowercase())
    }

    /// Whether the input derives from the named rule, over the whole input.
    ///
    /// The match is anchored at both ends: it holds when some derivation of the
    /// rule spans the input entire, and not when one spans a part of it.
    ///
    /// # Errors
    ///
    /// Returns the name when the grammar defines no such rule.
    pub fn matches(&self, rule: &str, input: &str) -> Result<bool, UnknownRule> {
        let Some(node) = self.rules.get(&rule.to_ascii_lowercase()) else {
            return Err(UnknownRule {
                name: rule.to_owned(),
            });
        };

        let characters: Vec<Ch> = input.chars().collect();
        let end = characters.len();

        Ok(self.walk(node, &characters, 0, &mut |position| position == end))
    }

    /// The literal openings every string the named rule admits begins with.
    ///
    /// A pattern of the declared surface names a place rather than describing a
    /// string: it opens with the spelling of a directory or a file and then says
    /// how far below it the reach goes. Reading those openings back out is what
    /// lets a caller holding a pattern walk what it selects, instead of asking
    /// the pattern about every path a corpus carries.
    ///
    /// A branch is complete when its opening is the whole of what that arm
    /// admits, and a prefix otherwise, because the two answer different
    /// questions: a complete branch names a file and a prefix names where a
    /// reach begins. An arm committing to no opening contributes the empty
    /// prefix, which is the honest answer for a pattern reaching everything.
    ///
    /// # Errors
    ///
    /// Returns the name when the grammar defines no such rule.
    pub fn literal_branches(&self, rule: &str) -> Result<Vec<Branch>, UnknownRule> {
        let Some(node) = self.rules.get(&rule.to_ascii_lowercase()) else {
            return Err(UnknownRule {
                name: rule.to_owned(),
            });
        };

        Ok(self.openings(node, &mut BTreeSet::new()).branches)
    }

    /// The openings one node commits to, and whether they are the whole of it.
    fn openings(&self, node: &Node, visiting: &mut BTreeSet<String>) -> Openings {
        match node {
            Node::Alternation(arms) => {
                let read: Vec<Openings> = arms
                    .iter()
                    .map(|arm| self.openings(arm, visiting))
                    .collect();

                Openings {
                    complete: read.iter().all(|arm| arm.complete),
                    branches: read.into_iter().flat_map(|arm| arm.branches).collect(),
                }
            }
            Node::Concatenation(items) => {
                let mut read = Openings::nothing();

                for item in items {
                    let next = self.openings(item, visiting);
                    let whole = next.complete;

                    read = read.followed_by(&next);

                    if !whole {
                        break;
                    }
                }

                read
            }
            // An iteration the pattern may omit commits to nothing, and one it
            // requires commits to what a single iteration opens with. A
            // repetition is the whole of what it admits only where it is written
            // to happen exactly once.
            Node::Repetition {
                minimum,
                maximum,
                inner,
            } => {
                if *minimum == 0 {
                    return Openings::anything();
                }

                let mut read = self.openings(inner, visiting);

                read.complete = read.complete && *minimum == 1 && *maximum == Some(1);

                read
            }
            Node::Reference { name, .. } => {
                if !visiting.insert(name.clone()) {
                    return Openings::anything();
                }

                let read = self
                    .rules
                    .get(name)
                    .map_or_else(Openings::anything, |rule| self.openings(rule, visiting));

                visiting.remove(name);

                read
            }
            // A case-insensitive spelling admits several strings and commits to
            // none of them, so it opens a reach without naming it.
            Node::Literal {
                characters,
                sensitive,
            } => {
                if !*sensitive && !characters.is_empty() {
                    return Openings::anything();
                }

                Openings::exactly(characters.iter().collect())
            }
            Node::Values(values) => Openings::exactly(values.iter().collect()),
            Node::Range { .. } => Openings::anything(),
        }
    }

    /// Whether the node matches at the position with some end the continuation accepts.
    fn walk(
        &self,
        node: &Node,
        input: &[Ch],
        position: usize,
        continuation: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        match node {
            Node::Alternation(arms) => {
                for arm in arms {
                    if self.walk(arm, input, position, continuation) {
                        return true;
                    }
                }

                false
            }
            Node::Concatenation(items) => self.walk_sequence(items, input, position, continuation),
            Node::Repetition {
                minimum,
                maximum,
                inner,
            } => {
                let repeat = Repeat {
                    inner,
                    minimum: *minimum,
                    maximum: *maximum,
                };

                self.walk_repetition(&repeat, 0, input, position, continuation)
            }
            Node::Reference { name, .. } => self
                .rules
                .get(name)
                .is_some_and(|rule| self.walk(rule, input, position, continuation)),
            Node::Literal {
                characters,
                sensitive,
            } => {
                let end = position + characters.len();

                if end > input.len() {
                    return false;
                }

                let held = characters
                    .iter()
                    .zip(&input[position..end])
                    .all(|(declared, found)| {
                        if *sensitive {
                            declared == found
                        } else {
                            declared.eq_ignore_ascii_case(found)
                        }
                    });

                held && continuation(end)
            }
            Node::Values(values) => {
                let end = position + values.len();

                end <= input.len()
                    && values.as_slice() == &input[position..end]
                    && continuation(end)
            }
            Node::Range { low, high } => {
                input
                    .get(position)
                    .is_some_and(|found| found >= low && found <= high)
                    && continuation(position + 1)
            }
        }
    }

    /// Whether the items match in turn from the position.
    fn walk_sequence(
        &self,
        items: &[Node],
        input: &[Ch],
        position: usize,
        continuation: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        match items.split_first() {
            None => continuation(position),
            Some((first, rest)) => self.walk(first, input, position, &mut |next| {
                self.walk_sequence(rest, input, next, continuation)
            }),
        }
    }

    /// Whether the inner element repeats between the bounds from the position.
    ///
    /// An iteration matching the empty string reaches the position it started at
    /// and repeating it satisfies any remaining minimum, so its whole
    /// contribution is that one end and the recursion stops there. Every
    /// iteration recursed on consumes at least one character, which bounds the
    /// recursion by the input's length.
    fn walk_repetition(
        &self,
        repeat: &Repeat<'_>,
        count: u32,
        input: &[Ch],
        position: usize,
        continuation: &mut dyn FnMut(usize) -> bool,
    ) -> bool {
        if count >= repeat.minimum && continuation(position) {
            return true;
        }

        if repeat.maximum.is_some_and(|maximum| count >= maximum) {
            return false;
        }

        self.walk(repeat.inner, input, position, &mut |next| {
            if next == position {
                return continuation(position);
            }

            self.walk_repetition(repeat, count + 1, input, next, continuation)
        })
    }
}

/// The three constants of one repetition, held together while it iterates.
struct Repeat<'a> {
    inner: &'a Node,
    minimum: u32,
    maximum: Option<u32>,
}

/// One rule as the reader took it off the text.
struct ParsedRule {
    name: String,
    position: usize,
    incremental: bool,
    node: Node,
}

/// Read a rule list into the map, refusing a name defined twice within it.
///
/// The core rules go through the same door with a name set of their own, which
/// is what lets a rule list define its own rule of a core name: the definition
/// shadows the core one rather than colliding with it.
fn read_rules(
    text: &str,
    rules: &mut BTreeMap<String, Node>,
    defined: &mut BTreeSet<String>,
) -> Result<(), GrammarDefect> {
    let mut reader = Reader {
        source: text.as_bytes(),
        position: 0,
    };

    loop {
        reader.skip_between_rules();

        if reader.done() {
            break;
        }

        let parsed = reader.rule()?;

        if parsed.incremental {
            match rules.get_mut(&parsed.name) {
                Some(existing) => append_alternative(existing, parsed.node),
                None => {
                    return Err(GrammarDefect::IncrementWithoutBase {
                        position: parsed.position,
                        name: parsed.name,
                    });
                }
            }

            defined.insert(parsed.name);
        } else {
            if !defined.insert(parsed.name.clone()) {
                return Err(GrammarDefect::DuplicateRule {
                    position: parsed.position,
                    name: parsed.name,
                });
            }

            rules.insert(parsed.name, parsed.node);
        }
    }

    Ok(())
}

/// Add an arm to a rule, making it an alternation where it was not one.
fn append_alternative(existing: &mut Node, addition: Node) {
    match existing {
        Node::Alternation(arms) => arms.push(addition),
        other => {
            let base = std::mem::replace(other, Node::Concatenation(Vec::new()));

            *other = Node::Alternation(vec![base, addition]);
        }
    }
}

/// Hold a parsed rule list to the two conditions the matcher needs of it.
fn validate(rules: &BTreeMap<String, Node>) -> Result<(), GrammarDefect> {
    let mut undefined: Option<(usize, String)> = None;

    for node in rules.values() {
        visit_references(node, &mut |name, position| {
            if !rules.contains_key(name)
                && undefined.as_ref().is_none_or(|(seen, _)| position < *seen)
            {
                undefined = Some((position, name.to_owned()));
            }
        });
    }

    if let Some((position, name)) = undefined {
        return Err(GrammarDefect::UndefinedRule { position, name });
    }

    let nullable = nullable_rules(rules);

    for name in rules.keys() {
        if left_corners(rules, &nullable, name).contains(name) {
            return Err(GrammarDefect::LeftRecursion { name: name.clone() });
        }
    }

    Ok(())
}

/// Offer every reference the node carries, at any depth.
fn visit_references(node: &Node, found: &mut dyn FnMut(&str, usize)) {
    match node {
        Node::Alternation(items) | Node::Concatenation(items) => {
            for item in items {
                visit_references(item, found);
            }
        }
        Node::Repetition { inner, .. } => visit_references(inner, found),
        Node::Reference { name, position } => found(name, *position),
        Node::Literal { .. } | Node::Values(_) | Node::Range { .. } => {}
    }
}

/// The rules that can match the empty string, taken to a fixed point.
fn nullable_rules(rules: &BTreeMap<String, Node>) -> BTreeSet<String> {
    let mut nullable = BTreeSet::new();

    loop {
        let mut changed = false;

        for (name, node) in rules {
            if !nullable.contains(name) && is_nullable(node, &nullable) {
                nullable.insert(name.clone());
                changed = true;
            }
        }

        if !changed {
            return nullable;
        }
    }
}

/// Whether the node can match the empty string, given the rules that can.
fn is_nullable(node: &Node, nullable: &BTreeSet<String>) -> bool {
    match node {
        Node::Alternation(arms) => arms.iter().any(|arm| is_nullable(arm, nullable)),
        Node::Concatenation(items) => items.iter().all(|item| is_nullable(item, nullable)),
        Node::Repetition { minimum, inner, .. } => *minimum == 0 || is_nullable(inner, nullable),
        Node::Reference { name, .. } => nullable.contains(name),
        Node::Literal { characters, .. } => characters.is_empty(),
        Node::Values(values) => values.is_empty(),
        Node::Range { .. } => false,
    }
}

/// The rules reachable at the left edge of the named rule, transitively.
fn left_corners(
    rules: &BTreeMap<String, Node>,
    nullable: &BTreeSet<String>,
    name: &str,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![name.to_owned()];

    while let Some(current) = pending.pop() {
        let Some(node) = rules.get(&current) else {
            continue;
        };

        let mut direct = BTreeSet::new();

        collect_left(node, nullable, &mut direct);

        for next in direct {
            if seen.insert(next.clone()) {
                pending.push(next);
            }
        }
    }

    seen
}

/// The rules the node can reference without consuming a character first.
fn collect_left(node: &Node, nullable: &BTreeSet<String>, found: &mut BTreeSet<String>) {
    match node {
        Node::Alternation(arms) => {
            for arm in arms {
                collect_left(arm, nullable, found);
            }
        }
        Node::Concatenation(items) => {
            for item in items {
                collect_left(item, nullable, found);

                if !is_nullable(item, nullable) {
                    break;
                }
            }
        }
        Node::Repetition { inner, .. } => collect_left(inner, nullable, found),
        Node::Reference { name, .. } => {
            found.insert(name.clone());
        }
        Node::Literal { .. } | Node::Values(_) | Node::Range { .. } => {}
    }
}

/// The rule list's text, read one construct at a time.
struct Reader<'a> {
    source: &'a [u8],
    position: usize,
}

impl Reader<'_> {
    /// The byte standing at an offset.
    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.source.get(offset).copied()
    }

    /// The byte standing at the position.
    fn peek(&self) -> Option<u8> {
        self.byte_at(self.position)
    }

    /// Whether the text is spent.
    const fn done(&self) -> bool {
        self.position >= self.source.len()
    }

    /// The text between two offsets, lowercased as the form reads a rule name.
    fn name_between(&self, start: usize, end: usize) -> String {
        String::from_utf8_lossy(&self.source[start..end]).to_ascii_lowercase()
    }

    /// Consume the spaces and horizontal tabs standing here.
    fn skip_wsp(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.position += 1;
        }
    }

    /// Consume a comment and the line ending after it, or a bare line ending.
    ///
    /// The end of the text ends a line too, so a rule list whose last line
    /// carries no line ending is read like any other.
    fn line_end(&mut self) -> bool {
        let start = self.position;

        if self.peek() == Some(b';') {
            self.position += 1;

            while matches!(self.peek(), Some(b' ' | b'\t' | 0x21..=0x7e)) {
                self.position += 1;
            }
        }

        match self.peek() {
            Some(b'\r') => {
                self.position += 1;

                if self.peek() == Some(b'\n') {
                    self.position += 1;
                }

                true
            }
            Some(b'\n') => {
                self.position += 1;

                true
            }
            None => true,
            _ => {
                self.position = start;

                false
            }
        }
    }

    /// Consume the whitespace a rule may carry within itself.
    ///
    /// A line ending counts only when whitespace follows it on the next line,
    /// which is the form's continuation rule, and when that next line does not
    /// itself begin a rule; anything else leaves the position where the rule
    /// ends.
    fn skip_c_wsp(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t') => self.position += 1,
                Some(b';' | b'\r' | b'\n') => {
                    let start = self.position;

                    if !self.line_end()
                        || !matches!(self.peek(), Some(b' ' | b'\t'))
                        || self.starts_rule()
                    {
                        self.position = start;

                        return;
                    }
                }
                _ => return,
            }
        }
    }

    /// Whether a rule name and a defining symbol stand at the position.
    ///
    /// The lookahead reaches past the indentation, and it decides only cases the
    /// form leaves unreadable: a defining symbol may not stand within a rule, so
    /// every line this stops from being read as a continuation is a line that
    /// would have refused the rule list a moment later.
    fn starts_rule(&self) -> bool {
        let mut scan = self.position;

        while matches!(self.byte_at(scan), Some(b' ' | b'\t')) {
            scan += 1;
        }

        if !matches!(self.byte_at(scan), Some(byte) if byte.is_ascii_alphabetic()) {
            return false;
        }

        while matches!(self.byte_at(scan), Some(byte) if byte.is_ascii_alphanumeric() || byte == b'-')
        {
            scan += 1;
        }

        while matches!(self.byte_at(scan), Some(b' ' | b'\t')) {
            scan += 1;
        }

        self.byte_at(scan) == Some(b'=')
    }

    /// Consume the blank and comment lines standing between two rules.
    fn skip_between_rules(&mut self) {
        loop {
            self.skip_wsp();

            if !matches!(self.peek(), Some(b';' | b'\r' | b'\n')) {
                return;
            }

            let start = self.position;

            if !self.line_end() || self.position == start {
                return;
            }
        }
    }

    /// Read a rule name, lowercased, with the offset it starts at.
    fn rulename(&mut self) -> Option<(String, usize)> {
        let start = self.position;

        if !matches!(self.peek(), Some(byte) if byte.is_ascii_alphabetic()) {
            return None;
        }

        self.position += 1;

        while matches!(self.peek(), Some(byte) if byte.is_ascii_alphanumeric() || byte == b'-') {
            self.position += 1;
        }

        Some((self.name_between(start, self.position), start))
    }

    /// Read one rule, from its name to the line ending that closes it.
    fn rule(&mut self) -> Result<ParsedRule, GrammarDefect> {
        let Some((name, position)) = self.rulename() else {
            return Err(GrammarDefect::Expected {
                position: self.position,
                expected: "a rule name",
            });
        };

        self.skip_c_wsp();

        if self.peek() != Some(b'=') {
            return Err(GrammarDefect::Expected {
                position: self.position,
                expected: "a defining symbol",
            });
        }

        self.position += 1;

        let incremental = self.peek() == Some(b'/');

        if incremental {
            self.position += 1;
        }

        self.skip_c_wsp();

        let node = self.alternation()?;

        self.skip_wsp();

        let terminator = self.position;

        if !self.line_end() {
            return Err(GrammarDefect::Expected {
                position: terminator,
                expected: "the end of the rule",
            });
        }

        Ok(ParsedRule {
            name,
            position,
            incremental,
            node,
        })
    }

    /// Read a concatenation and every alternative written after it.
    fn alternation(&mut self) -> Result<Node, GrammarDefect> {
        let mut arms = vec![self.concatenation()?];

        loop {
            let start = self.position;

            self.skip_c_wsp();

            if self.peek() != Some(b'/') {
                self.position = start;

                break;
            }

            self.position += 1;

            self.skip_c_wsp();

            arms.push(self.concatenation()?);
        }

        Ok(if arms.len() == 1 {
            arms.remove(0)
        } else {
            Node::Alternation(arms)
        })
    }

    /// Read the repetitions standing in sequence, separated by whitespace.
    fn concatenation(&mut self) -> Result<Node, GrammarDefect> {
        let mut items = vec![self.repetition()?];

        loop {
            let start = self.position;

            self.skip_c_wsp();

            if self.position == start || !self.starts_element() {
                self.position = start;

                break;
            }

            items.push(self.repetition()?);
        }

        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Node::Concatenation(items)
        })
    }

    /// Whether an element or a repetition count could start here.
    fn starts_element(&self) -> bool {
        matches!(self.peek(), Some(byte) if byte.is_ascii_alphanumeric() || matches!(byte, b'*' | b'(' | b'[' | b'"' | b'%' | b'<'))
    }

    /// Read an element with the repetition count written before it, if any.
    fn repetition(&mut self) -> Result<Node, GrammarDefect> {
        let start = self.position;
        let first = self.count()?;

        let (minimum, maximum) = if self.peek() == Some(b'*') {
            self.position += 1;

            (first.unwrap_or(0), self.count()?)
        } else if let Some(exact) = first {
            (exact, Some(exact))
        } else {
            (1, Some(1))
        };

        if maximum.is_some_and(|maximum| maximum < minimum) {
            return Err(GrammarDefect::Bound {
                position: start,
                defect: "counts downward",
            });
        }

        let element = self.element()?;

        Ok(if minimum == 1 && maximum == Some(1) {
            element
        } else {
            Node::Repetition {
                minimum,
                maximum,
                inner: Box::new(element),
            }
        })
    }

    /// Read a decimal repetition count, where one is written.
    fn count(&mut self) -> Result<Option<u32>, GrammarDefect> {
        let start = self.position;
        let mut value: u32 = 0;

        while let Some(byte) = self.peek() {
            let Some(digit) = (byte as char).to_digit(10) else {
                break;
            };

            let Some(next) = value
                .checked_mul(10)
                .and_then(|shifted| shifted.checked_add(digit))
            else {
                return Err(GrammarDefect::Bound {
                    position: start,
                    defect: "exceeds the largest count the matcher holds",
                });
            };

            value = next;
            self.position += 1;
        }

        Ok((self.position > start).then_some(value))
    }

    /// Read one element: a reference, a group, an option, or a terminal value.
    fn element(&mut self) -> Result<Node, GrammarDefect> {
        match self.peek() {
            Some(byte) if byte.is_ascii_alphabetic() => {
                let Some((name, position)) = self.rulename() else {
                    return Err(GrammarDefect::Expected {
                        position: self.position,
                        expected: "a rule name",
                    });
                };

                Ok(Node::Reference { name, position })
            }
            Some(b'(') => self.group(b')', "a closing parenthesis"),
            Some(b'[') => {
                let inner = self.group(b']', "a closing bracket")?;

                Ok(Node::Repetition {
                    minimum: 0,
                    maximum: Some(1),
                    inner: Box::new(inner),
                })
            }
            Some(b'"') => self.quoted(false),
            Some(b'%') => self.percent(),
            Some(b'<') => Err(GrammarDefect::ProseValue {
                position: self.position,
            }),
            _ => Err(GrammarDefect::Expected {
                position: self.position,
                expected: "an element",
            }),
        }
    }

    /// Read a parenthesised group or a bracketed option, closer and all.
    fn group(&mut self, close: u8, expected: &'static str) -> Result<Node, GrammarDefect> {
        self.position += 1;

        self.skip_c_wsp();

        let inner = self.alternation()?;

        self.skip_c_wsp();

        if self.peek() != Some(close) {
            return Err(GrammarDefect::Expected {
                position: self.position,
                expected,
            });
        }

        self.position += 1;

        Ok(inner)
    }

    /// Read a quoted string, with or without regard to case.
    fn quoted(&mut self, sensitive: bool) -> Result<Node, GrammarDefect> {
        self.position += 1;

        let mut characters = Vec::new();

        loop {
            match self.peek() {
                Some(b'"') => {
                    self.position += 1;

                    return Ok(Node::Literal {
                        characters,
                        sensitive,
                    });
                }
                Some(byte @ (0x20..=0x21 | 0x23..=0x7e)) => {
                    characters.push(Ch::from(byte));
                    self.position += 1;
                }
                _ => {
                    return Err(GrammarDefect::Expected {
                        position: self.position,
                        expected: "a closing quotation mark",
                    });
                }
            }
        }
    }

    /// Read what a percent introduces: a numeric value or a string prefix.
    fn percent(&mut self) -> Result<Node, GrammarDefect> {
        self.position += 1;

        match self.peek() {
            Some(b's' | b'S') => self.prefixed(true),
            Some(b'i' | b'I') => self.prefixed(false),
            Some(b'b' | b'B') => self.numeric(2),
            Some(b'd' | b'D') => self.numeric(10),
            Some(b'x' | b'X') => self.numeric(16),
            _ => Err(GrammarDefect::Expected {
                position: self.position,
                expected: "a numeric base or a string prefix",
            }),
        }
    }

    /// Read the quoted string a case prefix stands before.
    fn prefixed(&mut self, sensitive: bool) -> Result<Node, GrammarDefect> {
        self.position += 1;

        if self.peek() != Some(b'"') {
            return Err(GrammarDefect::Expected {
                position: self.position,
                expected: "a quoted string",
            });
        }

        self.quoted(sensitive)
    }

    /// Read a numeric value, a dotted sequence of them, or a range.
    fn numeric(&mut self, radix: u32) -> Result<Node, GrammarDefect> {
        self.position += 1;

        let first = self.numeric_value(radix)?;

        match self.peek() {
            Some(b'.') => {
                let mut values = vec![first];

                while self.peek() == Some(b'.') {
                    self.position += 1;
                    values.push(self.numeric_value(radix)?);
                }

                Ok(Node::Values(values))
            }
            Some(b'-') => {
                let start = self.position;

                self.position += 1;

                let high = self.numeric_value(radix)?;

                if high < first {
                    return Err(GrammarDefect::Bound {
                        position: start,
                        defect: "runs downward",
                    });
                }

                Ok(Node::Range { low: first, high })
            }
            _ => Ok(Node::Values(vec![first])),
        }
    }

    /// Read the digits of one numeric value in its own base.
    fn numeric_value(&mut self, radix: u32) -> Result<Ch, GrammarDefect> {
        let start = self.position;
        let mut value: u32 = 0;

        while let Some(byte) = self.peek() {
            let Some(digit) = (byte as char).to_digit(radix) else {
                break;
            };

            let Some(next) = value
                .checked_mul(radix)
                .and_then(|shifted| shifted.checked_add(digit))
            else {
                return Err(GrammarDefect::Bound {
                    position: start,
                    defect: "names no character",
                });
            };

            value = next;
            self.position += 1;
        }

        if self.position == start {
            return Err(GrammarDefect::Expected {
                position: start,
                expected: "a numeric value",
            });
        }

        Ch::from_u32(value).ok_or(GrammarDefect::Bound {
            position: start,
            defect: "names no character",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Grammar, GrammarDefect};

    /// Parse a rule list that is expected to be a grammar.
    fn grammar(text: &str) -> Grammar {
        Grammar::parse(text).expect("the rule list is a grammar")
    }

    /// Whether the input derives from a rule the grammar is expected to define.
    fn matches(grammar: &Grammar, rule: &str, input: &str) -> bool {
        grammar.matches(rule, input).expect("the rule is defined")
    }

    /// The parser reads every construct RFC 5234 states — the rule name, both
    /// defining symbols, alternation, concatenation, the three repetition
    /// spellings, the group, the option, the quoted string, the three numeric
    /// bases in their value, sequence and range forms, the comment, and a
    /// reference to a core rule — so a declaration written to the standard is
    /// read by this engine rather than by a subset of it.
    ///
    /// ´claim:abnf:the-parser-reads-every-construct-of-the-form´
    /// ´test:unit:parses-every-construct-the-form-defines´
    #[test]
    fn parses_every_construct_the_form_defines() {
        let parsed = grammar(
            r#"; every construct of the form stands in this list
            greeting = opening SP subject [ "!" ]
            opening  = %s"Hello" / %i"hail" / "hey"
            subject  = 1*4symbol / 3numeral / *"z" / ( "the" SP "world" )
            symbol   = ALPHA / numeral / dash
            dash     = %x2D
            numeral  = %d48-57
            numeral  =/ %b110000
            pair     = %d13.10
            "#,
        );

        assert!(matches(&parsed, "greeting", "Hello the world"));
        assert!(matches(&parsed, "greeting", "hail zzz"));
        assert!(matches(&parsed, "greeting", "HEY 123!"));
        assert!(matches(&parsed, "greeting", "hey zzzzzzz"));
        assert!(matches(&parsed, "pair", "\r\n"));

        // The sensitive prefix, the repetition bound and the concatenation each
        // hold: the opening spelled otherwise, the subject one symbol too long,
        // and the greeting with no subject at all are all outside the rule.
        assert!(!matches(&parsed, "greeting", "hello the world"));
        assert!(!matches(&parsed, "greeting", "hey abcde"));
        assert!(!matches(&parsed, "greeting", "Hello"));
    }

    /// A comment runs to the end of its line and a line ending followed by
    /// whitespace continues the rule it interrupts, so a rule list may be laid
    /// out and annotated the way the standard's own examples are without
    /// changing what it means. An indented line whose first tokens are a rule
    /// name and a defining symbol starts a rule instead, which is the one place
    /// the letter of the form would read a rule list nobody could write
    /// indented.
    ///
    /// ´claim:abnf:comments-and-continuations-do-not-change-a-rule´
    /// ´test:unit:comments-and-continuations-are-read-as-the-form-reads-them´
    #[test]
    fn comments_and_continuations_are_read_as_the_form_reads_them() {
        let spread = grammar(
            r#"
            ; the vocabulary, one arm to a line

            kind = "alpha"      ; the first
                 / "beta"       ; the second
                 / "gamma"
            "#,
        );

        let compact = grammar(r#"kind = "alpha" / "beta" / "gamma""#);

        for arm in ["alpha", "beta", "gamma"] {
            assert!(matches(&spread, "kind", arm));
            assert!(matches(&compact, "kind", arm));
        }

        assert!(!matches(&spread, "kind", "delta"));

        // A rule list whose last line carries no line ending is read like any
        // other, which is what a pattern read out of a declared row looks like.
        assert!(matches(&grammar(r#"tail = "x""#), "tail", "x"));

        // An indented line continues the rule above it, and stops continuing it
        // the moment its first tokens are a rule name and a defining symbol.
        let continued =
            grammar("opening = \"a\"\n    closing\nclosing = \"b\"\n    extra = \"c\"\n");

        assert!(matches(&continued, "opening", "ab"));
        assert!(matches(&continued, "closing", "b"));
        assert!(matches(&continued, "extra", "c"));
    }

    /// Source outside the form is refused where it is written, with the position
    /// and what was expected there: an unterminated group or option, an
    /// alternation arm with nothing in it, a rule with no defining symbol, an
    /// unterminated quoted string, a repetition counting downward, and a numeric
    /// value naming no character. A pattern the engine cannot read is a defect of
    /// the declaration rather than a rule that matches nothing.
    ///
    /// ´claim:abnf:malformed-source-is-refused-where-it-is-written´
    /// ´test:unit:refuses-source-that-is-not-the-form´
    #[test]
    fn refuses_source_that_is_not_the_form() {
        let unterminated =
            Grammar::parse(r#"rule = ( "a" / "b""#).expect_err("the group is unterminated");

        assert_eq!(
            unterminated,
            GrammarDefect::Expected {
                position: 18,
                expected: "a closing parenthesis",
            }
        );

        for (source, expected) in [
            (r#"rule = [ "a""#, "a closing bracket"),
            (r#"rule = "a" /"#, "an element"),
            (r#"rule = / "a""#, "an element"),
            (r#"rule "a""#, "a defining symbol"),
            (r#"rule = "a"#, "a closing quotation mark"),
            ("rule = %q41", "a numeric base or a string prefix"),
            ("rule = %x", "a numeric value"),
            ("1rule = %x41", "a rule name"),
            (r#"rule = "a" "b" ) "c""#, "the end of the rule"),
        ] {
            let defect = Grammar::parse(source).expect_err("the source is not a rule list");

            assert_eq!(
                defect,
                GrammarDefect::Expected {
                    position: defect.position().expect("the defect carries a position"),
                    expected,
                },
                "for `{source}`"
            );
        }

        for source in ["rule = 3*2%x41", "rule = %x41-30", "rule = %x110000"] {
            let defect = Grammar::parse(source).expect_err("the bound names nothing");

            assert!(
                matches!(defect, GrammarDefect::Bound { .. }),
                "for `{source}`: {defect}"
            );
        }
    }

    /// A prose value is refused by its own error naming its position. The
    /// construct is lawful in the form and unmatchable by any engine, so
    /// accepting it would leave a declaration whose meaning is not in the file
    /// behaving as a rule that matches nothing at all.
    ///
    /// ´claim:abnf:a-prose-value-is-refused-rather-than-accepted´
    /// ´test:unit:refuses-a-prose-value-rather-than-accepting-it-silently´
    #[test]
    fn refuses_a_prose_value_rather_than_accepting_it_silently() {
        let alone = Grammar::parse("rule = <any path the reviewer would call a plan>")
            .expect_err("prose is not matchable");

        assert_eq!(alone, GrammarDefect::ProseValue { position: 7 });

        let beside =
            Grammar::parse(r#"rule = "a" / <anything else>"#).expect_err("prose is not matchable");

        assert_eq!(beside, GrammarDefect::ProseValue { position: 13 });
    }

    /// A name defined twice is refused rather than resolved by order, and
    /// incremental alternatives extending a name nothing defines are refused
    /// rather than treated as a definition. Both are the same guard: a rule's
    /// meaning is assembled from declarations that agree about which rule they
    /// are about.
    ///
    /// ´claim:abnf:a-rule-is-defined-once-and-extended-only-where-it-stands´
    /// ´test:unit:refuses-a-rule-defined-twice-and-an-increment-with-no-base´
    #[test]
    fn refuses_a_rule_defined_twice_and_an_increment_with_no_base() {
        let twice = Grammar::parse("kind = \"alpha\"\nkind = \"beta\"\n")
            .expect_err("the name is defined twice");

        assert_eq!(
            twice,
            GrammarDefect::DuplicateRule {
                position: 15,
                name: "kind".to_owned(),
            }
        );

        let baseless = Grammar::parse("kind =/ \"beta\"\n").expect_err("nothing defines the name");

        assert_eq!(
            baseless,
            GrammarDefect::IncrementWithoutBase {
                position: 0,
                name: "kind".to_owned(),
            }
        );

        // A list may define a rule of a core name, which shadows the core one
        // rather than colliding with it.
        let shadowed = grammar("DIGIT = \"7\"\nnumber = 1*DIGIT\n");

        assert!(matches(&shadowed, "number", "777"));
        assert!(!matches(&shadowed, "number", "123"));
    }

    /// A reference to a name neither the rule list nor the core rules define is
    /// refused when the grammar is built, naming the position where the
    /// reference stands. A forward reference is ordinary, so the check waits for
    /// the whole list rather than reading it in order.
    ///
    /// ´claim:abnf:a-reference-resolves-or-the-grammar-is-refused´
    /// ´test:unit:refuses-a-reference-no-rule-defines´
    #[test]
    fn refuses_a_reference_no_rule_defines() {
        let dangling =
            Grammar::parse("rule = missing\n").expect_err("the reference resolves to nothing");

        assert_eq!(
            dangling,
            GrammarDefect::UndefinedRule {
                position: 7,
                name: "missing".to_owned(),
            }
        );

        // A rule may be referenced before it is written, and a reference is
        // resolved without regard to the case it is spelled in.
        assert!(matches(
            &grammar("outer = Inner\ninner = \"x\"\n"),
            "outer",
            "x"
        ));
    }

    /// A left-recursive rule is refused, whether it references itself directly,
    /// through another rule, or behind a construct that can match the empty
    /// string. Matching it would take cutting the recursion, and a cut rule
    /// matches a smaller language than it spells while still reporting a verdict.
    ///
    /// ´claim:abnf:a-left-recursive-rule-is-refused-rather-than-cut´
    /// ´test:unit:refuses-a-rule-that-can-reference-itself-without-consuming´
    #[test]
    fn refuses_a_rule_that_can_reference_itself_without_consuming() {
        for source in [
            "list = list \",\" item\nitem = \"x\"\n",
            "one = two\ntwo = one \"x\"\n",
            "padded = [ \"-\" ] padded \"x\"\n",
            "starred = *\"-\" starred \"x\"\n",
        ] {
            let defect = Grammar::parse(source).expect_err("the rule is left-recursive");

            assert!(
                matches!(defect, GrammarDefect::LeftRecursion { .. }),
                "for `{source}`: {defect}"
            );
            assert_eq!(defect.position(), None);
        }

        // Recursion that consumes before it recurses is ordinary, and it is the
        // shape a nested grammar is written in.
        let nested = grammar("nest = \"(\" [ nest ] \")\"\n");

        assert!(matches(&nested, "nest", "((()))"));
        assert!(!matches(&nested, "nest", "(()"));
    }

    /// An alternation admits every string any arm admits, in whatever order the
    /// arms are written, and a shorter arm that would strand the rest of the rule
    /// is backtracked out of. Order-dependence here would make a declaration's
    /// meaning turn on how its author happened to lay the arms out.
    ///
    /// ´claim:abnf:alternation-is-order-independent´
    /// ´test:unit:alternation-admits-its-arms-whatever-their-order´
    #[test]
    fn alternation_admits_its_arms_whatever_their_order() {
        let ascending = grammar(r#"word = "a" / "ab" / "abc""#);
        let descending = grammar(r#"word = "abc" / "ab" / "a""#);

        for input in ["a", "ab", "abc"] {
            assert!(matches(&ascending, "word", input));
            assert!(matches(&descending, "word", input));
        }

        assert!(!matches(&ascending, "word", "abcd"));

        // The first arm that fits is not the last word: a shorter arm leaving
        // the rest of the rule stranded is backtracked out of.
        let stranded = grammar(r#"word = ( "a" / "ab" ) "c""#);

        assert!(matches(&stranded, "word", "abc"));
        assert!(matches(&stranded, "word", "ac"));
    }

    /// Both repetition bounds are inclusive, an omitted bound is unbounded on
    /// that side, and the exact spelling admits that count and no other. So a
    /// rule spelled to admit two or three of something admits two and three, and
    /// neither one nor four.
    ///
    /// ´claim:abnf:repetition-bounds-are-inclusive´
    /// ´test:unit:repetition-bounds-are-inclusive-and-an-exact-count-binds´
    #[test]
    fn repetition_bounds_are_inclusive_and_an_exact_count_binds() {
        let counted = grammar(
            r#"
            bounded  = 2*3"a"
            exact    = 3"b"
            open     = *"c"
            atleast  = 1*"d"
            atmost   = *2"e"
            "#,
        );

        assert!(!matches(&counted, "bounded", "a"));
        assert!(matches(&counted, "bounded", "aa"));
        assert!(matches(&counted, "bounded", "aaa"));
        assert!(!matches(&counted, "bounded", "aaaa"));

        assert!(!matches(&counted, "exact", "bb"));
        assert!(matches(&counted, "exact", "bbb"));
        assert!(!matches(&counted, "exact", "bbbb"));

        assert!(matches(&counted, "open", ""));
        assert!(matches(&counted, "open", "ccccc"));

        assert!(!matches(&counted, "atleast", ""));
        assert!(matches(&counted, "atleast", "d"));

        assert!(matches(&counted, "atmost", ""));
        assert!(matches(&counted, "atmost", "ee"));
        assert!(!matches(&counted, "atmost", "eee"));
    }

    /// Repetition over a rule that can match the empty string terminates and
    /// decides, both where the input derives from it and where it does not. The
    /// empty iteration contributes the position it started at and nothing else,
    /// and it satisfies a minimum count without moving, so the matcher neither
    /// loops nor loses the derivations that minimum admits.
    ///
    /// ´claim:abnf:repetition-over-an-empty-match-terminates´
    /// ´test:unit:a-repetition-over-an-empty-match-terminates´
    #[test]
    fn a_repetition_over_an_empty_match_terminates() {
        let empty = grammar(
            r#"
            maybe    = [ "x" ]
            any      = *maybe
            atleast  = 2*3maybe
            trailing = *maybe "!"
            "#,
        );

        assert!(matches(&empty, "any", ""));
        assert!(matches(&empty, "any", "xxx"));
        assert!(!matches(&empty, "any", "y"));

        // A minimum count over an element that can match nothing is satisfied
        // without moving, so the empty input derives from it.
        assert!(matches(&empty, "atleast", ""));
        assert!(matches(&empty, "atleast", "xx"));

        // The repetition still gives the rest of the rule its turn, which is
        // what a cut at the first empty iteration would have cost.
        assert!(matches(&empty, "trailing", "!"));
        assert!(matches(&empty, "trailing", "xx!"));
        assert!(!matches(&empty, "trailing", "xx"));
    }

    /// A quoted string is case-insensitive as the form defines it, the RFC 7405
    /// prefixes name each reading aloud, and the case-sensitive one admits
    /// exactly the spelling it carries. A pattern naming a path in the repository
    /// wants the sensitive reading and has a standard way to ask for it.
    ///
    /// ´claim:abnf:a-quoted-string-ignores-case-unless-the-prefix-binds-it´
    /// ´test:unit:a-quoted-string-ignores-case-and-the-sensitive-prefix-binds-it´
    #[test]
    fn a_quoted_string_ignores_case_and_the_sensitive_prefix_binds_it() {
        let cased = grammar(
            r#"
            bare      = "readme"
            insensitive = %i"readme"
            sensitive = %s"README"
            "#,
        );

        for spelling in ["readme", "README", "ReadMe"] {
            assert!(matches(&cased, "bare", spelling));
            assert!(matches(&cased, "insensitive", spelling));
        }

        assert!(matches(&cased, "sensitive", "README"));
        assert!(!matches(&cased, "sensitive", "readme"));
        assert!(!matches(&cased, "sensitive", "ReadMe"));

        // The prefixes are the form's own tokens and are read whatever case they
        // are written in, while the string they carry is not folded.
        assert!(matches(
            &grammar(r#"upper = %S"README""#),
            "upper",
            "README"
        ));
        assert!(!matches(
            &grammar(r#"upper = %S"README""#),
            "upper",
            "readme"
        ));
    }

    /// A numeric value names a character in binary, decimal or hexadecimal, and
    /// its dotted form names a sequence while its hyphenated form names an
    /// inclusive range. That is how a rule reaches the characters a quoted string
    /// cannot carry, the separator and the line ending among them.
    ///
    /// ´claim:abnf:numeric-values-name-characters-ranges-and-sequences´
    /// ´test:unit:numeric-values-name-characters-ranges-and-sequences´
    #[test]
    fn numeric_values_name_characters_ranges_and_sequences() {
        let numeric = grammar(
            r"
            separator = %x2F
            decimal   = %d47
            binary    = %b101111
            ending    = %d13.10
            lower     = %x61-7A
            quote     = %x22
            ",
        );

        for base in ["separator", "decimal", "binary"] {
            assert!(matches(&numeric, base, "/"));
            assert!(!matches(&numeric, base, "\\"));
        }

        assert!(matches(&numeric, "ending", "\r\n"));
        assert!(!matches(&numeric, "ending", "\n"));

        // A range is inclusive at both ends, and it reaches nothing outside them.
        assert!(matches(&numeric, "lower", "a"));
        assert!(matches(&numeric, "lower", "z"));
        assert!(!matches(&numeric, "lower", "A"));

        // The quotation mark a quoted string cannot carry is reachable by value.
        assert!(matches(&numeric, "quote", "\""));
    }

    /// Incremental alternatives add arms to a rule already defined, in the order
    /// they are written, and they extend a core rule as readily as one the list
    /// defines. A vocabulary can therefore be stated once and widened where the
    /// widening belongs, rather than restated whole.
    ///
    /// ´claim:abnf:incremental-alternatives-extend-a-defined-rule´
    /// ´test:unit:incremental-alternatives-extend-the-rule-they-name´
    #[test]
    fn incremental_alternatives_extend_the_rule_they_name() {
        let extended = grammar(
            r#"
            kind = "alpha"
            kind =/ "beta"
            kind =/ "gamma"
            "#,
        );

        for arm in ["alpha", "beta", "gamma"] {
            assert!(matches(&extended, "kind", arm));
        }

        assert!(!matches(&extended, "kind", "delta"));

        // A core rule is extended by the same door, so a vocabulary may be
        // widened where the widening belongs.
        let widened = grammar(
            r#"
            DIGIT =/ "-"
            number = 1*DIGIT
            "#,
        );

        assert!(matches(&widened, "number", "12-3"));
        assert!(!matches(&widened, "number", "12+3"));
    }

    /// The sixteen core rules of Appendix B.1 are defined without being
    /// restated, and they mean what the appendix says: the visible characters,
    /// the digits, the letters, the line ending as a pair, and the hexadecimal
    /// digits admitting lowercase because the appendix spells its letters as
    /// quoted strings.
    ///
    /// ´claim:abnf:the-core-rules-stand-predefined´
    /// ´test:unit:the-core-rules-stand-predefined-and-referable´
    #[test]
    fn the_core_rules_stand_predefined_and_referable() {
        let core = grammar(
            r"
            visible  = 1*VCHAR
            digits   = 1*DIGIT
            letters  = 1*ALPHA
            hex      = 1*HEXDIG
            ending   = CRLF
            spacing  = LWSP
            anything = *OCTET
            ",
        );

        for name in [
            "ALPHA", "BIT", "CHAR", "CR", "CRLF", "CTL", "DIGIT", "DQUOTE", "HEXDIG", "HTAB", "LF",
            "LWSP", "OCTET", "SP", "VCHAR", "WSP",
        ] {
            assert!(
                core.defines(name),
                "expected the core rule `{name}` to be defined"
            );
        }

        assert!(matches(&core, "visible", "packages/linter"));
        assert!(!matches(&core, "visible", "packages linter"));

        assert!(matches(&core, "digits", "091"));
        assert!(!matches(&core, "digits", "09a"));

        assert!(matches(&core, "letters", "VCHAR"));
        assert!(matches(&core, "ending", "\r\n"));
        assert!(matches(&core, "spacing", " \t"));
        assert!(matches(&core, "anything", "\u{0}\u{ff}"));

        // The appendix spells the hexadecimal letters as quoted strings, so they
        // admit lowercase like every other quoted string in the form.
        assert!(matches(&core, "hex", "0F"));
        assert!(matches(&core, "hex", "0f"));
        assert!(!matches(&core, "hex", "0g"));
    }

    /// A rule matches a string when a derivation spans it entire, so a rule
    /// reaching only a prefix or only a suffix does not match. A configuration
    /// whose patterns decided paths by prefix would give every rule a reach its
    /// author never wrote.
    ///
    /// ´claim:abnf:a-match-spans-the-whole-input´
    /// ´test:unit:a-match-is-whole-input-and-anchored-at-both-ends´
    #[test]
    fn a_match_is_whole_input_and_anchored_at_both_ends() {
        let anchored = grammar(r#"word = "ab""#);

        assert!(matches(&anchored, "word", "ab"));
        assert!(!matches(&anchored, "word", "abc"));
        assert!(!matches(&anchored, "word", "xab"));
        assert!(!matches(&anchored, "word", "xabx"));
        assert!(!matches(&anchored, "word", ""));

        // The rule that means to reach a tail writes the tail, and then it is
        // the author's reach rather than the matcher's.
        let tailed = grammar(r#"word = "ab" *VCHAR"#);

        assert!(matches(&tailed, "word", "abc"));
        assert!(!matches(&tailed, "word", "xab"));
    }

    /// Asking whether an input derives from a rule the grammar does not define is
    /// an error naming the rule, not the answer no. A rule name that resolves to
    /// nothing is a defect of whoever asked, and answering no would hide it
    /// behind a verdict about the input.
    ///
    /// ´claim:abnf:an-unknown-rule-name-is-an-error´
    /// ´test:unit:an-unknown-rule-name-is-an-error-rather-than-a-refusal´
    #[test]
    fn an_unknown_rule_name_is_an_error_rather_than_a_refusal() {
        let defined = grammar(r#"word = "ab""#);

        let unknown = defined
            .matches("phrase", "ab")
            .expect_err("no such rule is defined");

        assert_eq!(unknown.name(), "phrase");
        assert!(unknown.to_string().contains("phrase"), "{unknown}");
        assert!(!defined.defines("phrase"));

        // A rule name is case-insensitive, so asking for it in another case is
        // asking for the same rule rather than for an unknown one.
        assert!(matches(&defined, "WORD", "ab"));
        assert!(defined.defines("Word"));
    }

    /// A grammar written the way the declared surface writes one selects a
    /// directory and everything beneath it, and stops at the separator: a sibling
    /// whose name merely begins with the same characters is outside the bound.
    /// Both the register subtree and the linter-package exclusion of the ratified
    /// examples are read here.
    ///
    /// ´claim:abnf:a-path-grammar-selects-a-subtree-and-stops-at-the-separator´
    /// ´test:unit:a-path-grammar-selects-a-subtree-and-stops-at-the-separator´
    #[test]
    fn a_path_grammar_selects_a_subtree_and_stops_at_the_separator() {
        let bounds = grammar(
            r#"
            registers = %s"packages/assayer/docs/plans/burn" [ "/" *VCHAR ]
            linter    = %s"packages/linter" [ "/" *VCHAR ]
            "#,
        );

        assert!(matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans/burn"
        ));
        assert!(matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans/burn/sections.md"
        ));
        assert!(matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans/burn/old/sections.md"
        ));

        // The optional tail begins at the separator, so a sibling whose name
        // merely extends the last component is outside the bound.
        assert!(!matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans/burnt"
        ));
        assert!(!matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans"
        ));
        assert!(!matches(
            &bounds,
            "registers",
            "packages/assayer/docs/plans/burn."
        ));

        assert!(matches(&bounds, "linter", "packages/linter"));
        assert!(matches(&bounds, "linter", "packages/linter/src/abnf.rs"));
        assert!(!matches(
            &bounds,
            "linter",
            "packages/linter-extra/src/lib.rs"
        ));

        // The path is named case-sensitively, which is the reading a repository
        // path wants and the reason the prefix is written.
        assert!(!matches(&bounds, "linter", "packages/Index-Linter"));
    }

    /// A grammar naming a file at any depth reaches the file wherever it stands,
    /// admits it at the root, and holds it to its exact spelling and to being the
    /// last component. Depth is written as a repetition over segments rather than
    /// as a wildcard that could swallow the separator.
    ///
    /// ´claim:abnf:a-path-grammar-names-a-file-at-any-depth´
    /// ´test:unit:a-path-grammar-names-a-file-at-any-depth´
    #[test]
    fn a_path_grammar_names_a_file_at_any_depth() {
        let manifests = grammar(
            r#"
            manifest = *( segment "/" ) %s"Cargo.toml"
            segment  = 1*visible-outside-separator
            visible-outside-separator = %x21-2E / %x30-7E
            "#,
        );

        assert!(matches(&manifests, "manifest", "Cargo.toml"));
        assert!(matches(
            &manifests,
            "manifest",
            "packages/linter/Cargo.toml"
        ));
        assert!(matches(&manifests, "manifest", "a/b/c/d/e/Cargo.toml"));

        assert!(!matches(&manifests, "manifest", "cargo.toml"));
        assert!(!matches(&manifests, "manifest", "Cargo.toml.bak"));
        assert!(!matches(&manifests, "manifest", "packages/Cargo.tomlx"));
        assert!(!matches(
            &manifests,
            "manifest",
            "packages/linter/Cargo.lock"
        ));

        // A segment stops at the separator, so an empty component is no path.
        assert!(!matches(&manifests, "manifest", "packages//Cargo.toml"));
        assert!(!matches(&manifests, "manifest", "/Cargo.toml"));
    }

    /// A rule is read back for the literal openings its arms commit to, in the
    /// order they are written, and each opening says whether it is the whole of
    /// its arm or the beginning of a reach. A rule committing to nothing opens
    /// with the empty prefix, which is the true answer for a reach over
    /// everything and the only one a caller could act on without inventing a
    /// bound the author never wrote.
    ///
    /// ´claim:abnf:literal-openings-are-readable´
    /// ´test:unit:a-rule-reports-the-literal-openings-its-arms-commit-to´
    #[test]
    fn a_rule_reports_the_literal_openings_its_arms_commit_to() {
        let read = |source: &str| {
            let grammar = grammar(&format!("reach = {source}\n"));

            grammar
                .literal_branches("reach")
                .expect("the rule stands")
                .into_iter()
                .map(|branch| (branch.text().to_owned(), branch.is_complete()))
                .collect::<Vec<_>>()
        };

        // A subtree selection opens with the directory and continues below it,
        // so the opening is a prefix rather than the whole of the arm.
        assert_eq!(
            read(r#"( %s"docs" / %s"adr" ) [ "/" *VCHAR ]"#),
            vec![("docs".to_owned(), false), ("adr".to_owned(), false)]
        );

        // The arms come back in the order they are written, because a caller
        // resolving them to places has no other order to prefer.
        assert_eq!(
            read(r#"( %s"adr" / %s"docs" ) [ "/" *VCHAR ]"#),
            vec![("adr".to_owned(), false), ("docs".to_owned(), false)]
        );

        // A prefix written before an alternation is distributed over its arms,
        // and an arm ending there is the whole of what it admits.
        assert_eq!(
            read(r#"%s"su-exec/" ( %s"LICENSE" / %s"Makefile" )"#),
            vec![
                ("su-exec/LICENSE".to_owned(), true),
                ("su-exec/Makefile".to_owned(), true)
            ]
        );

        // A reach over everything commits to no spelling at all.
        assert_eq!(read("*VCHAR"), vec![(String::new(), false)]);

        // A case-insensitive spelling admits several strings and names none of
        // them, so it opens a reach without committing to one.
        assert_eq!(
            read(r#""docs" [ "/" *VCHAR ]"#),
            vec![(String::new(), false)]
        );
    }
}
