// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The declared corpus shape: one universe answer, and the ignore relation
//! removed from it.
//!
//! Beyond its envelope the shape document declares exactly two things, and the
//! reason there are two rather than one of each kind is that they answer the two
//! questions nothing else can answer for a corpus: which paths are there at all,
//! and which of those the corpus has decided not to account for
//! (´just:isolation:policy-data´). Compiling either answer into the programs was
//! the alternative, and it is the arrangement in which a linter can serve only
//! the corpus it was written beside — a repository that does not use git would
//! have had to be told it was one.
//!
//! # The two questions are the document's whole content, and a third is an owner act
//!
//! The universe answer is a kind rather than a walk: `git-tracked` says the base
//! set is what version control holds, `as-written` says it is what the filesystem
//! holds, and this module carries the answer without performing either. What it
//! does perform is the second question, because the ignore relation is the same
//! removal over either base and has no business being written twice.
//!
//! A third question would be a third thing every policy, profile, carrier and
//! partition had to be taught, and it would arrive without a verdict or an owning
//! program to give it one — which is how a shape document becomes an unbounded
//! census of the tree. So the document is closed at these two, and widening it is
//! an owner act rather than a schema extension an implementation inferred.
//!
//! # Removal is a set union, which is why row order decides nothing
//!
//! Rows are removed together rather than in sequence, so two rows reaching one
//! path is ordinary rather than a conflict, and no row can restore what another
//! removed. Sequential application was the alternative and it buys a precedence
//! question nobody asked: a reader would have to hold the list in their head in
//! order to know what survived it, and a row moved for tidiness could change the
//! corpus.
//!
//! What the rows are held to instead is identity. Each carries a unique declared
//! name, because a report saying *some row removed this path* is not a report
//! anybody can act on, and two rows answering to one name would make the naming
//! report unreadable exactly as it does in the owner surface's exclusion set.
//!
//! # The removal happens once, before anything ranges over the universe
//!
//! A globally ignored path is not a path that policies agree to skip. It is
//! absent from the set every projection is taken from, so the partition never
//! attributes it, no policy selects it, no profile censuses it and the label
//! carrier never sees it. Leaving the removal to each consumer was the
//! alternative, and it is the arrangement in which one consumer forgets and a
//! path the corpus disowned produces a finding against nobody.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`carries_either_declared_universe_answer`] | universe | A corpus declares one of exactly two universe answers, and the resolver carries the answer it was given rather than resolving it away. An answer outside the pair reads as nothing rather than falling back to either kind, because a misspelling that defaulted would hand a corpus a universe it did not declare and the two kinds differ in exactly the paths a reviewer could then not account for. |
//! | [`an_empty_ignore_relation_removes_nothing`] | universe | An empty ignore relation is a live declaration that nothing is removed, and it leaves the base universe entire. That is the recommended state of a corpus whose universe answer and owner activations already resolve every exclusion class, and it must be distinguishable from a relation that removes something rather than being the same code path with no rows in it by accident. |
//! | [`removes_the_union_of_every_matching_row`] | universe | A nonempty relation removes the union of every matching row, so two rows reaching one path is ordinary rather than a conflict and the order they are written in decides nothing. Sequential application was the alternative, and it buys a precedence question nobody asked: a row moved for tidiness could then change which paths the corpus accounts for. |
//! | [`an_ignored_path_survives_in_no_projection`] | universe | A globally ignored path is absent from the set every projection is taken from rather than skipped by each of them in turn, so nothing above the removal can see it: the partition never attributes it, no policy selects it, and the carrier never reads it. The row that removed it is still nameable, because a report saying only that some row removed a path is not a report anybody can act on. |
//! | [`an_ignore_name_is_unique_and_an_ignore_region_is_not`] | universe | An ignore row's name is unique and its region is not. Two rows answering to one name refuse the document and the refusal carries both patterns, because the repair is a choice between two regions and the name alone does not say which two. Two differently named rows removing the very same path are no defect at all: a union takes both, and a path answering to several rules is an ordinary thing for one to do. |
//! | [`a_nameless_ignore_row_refuses_in_the_surfaces_own_words`] | universe | An ignore row states the name of the region it removes, and a row carrying only a pattern is refused in the surface's own words rather than by the parser's. The name is what the accounting says when it reports the paths this row took out of the universe, so a row without one removes files for a reason no report can state — and an author who wrote a bare pattern is owed that sentence rather than the name of a missing field. |

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use crate::declaration::AbnfPattern;
use crate::pattern::BytePath;

/// Which base set the corpus universe is taken from.
///
/// Two answers rather than a boolean, because the pair is not a switch on one
/// behaviour: each names a different authority for what exists, and a corpus
/// picks the one that is true of it. The kind is carried rather than resolved
/// here, since materializing either base is the topology phase's work and this
/// document must be readable before that phase can run at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UniverseKind {
    /// The base set is what version control tracks, so an untracked draft is outside it.
    ///
    /// A corpus answering this way gets the finite committed set and nothing
    /// else: an uncommitted file creates neither a finding nor debt, because it
    /// is not a member of the universe rather than a member that was excused.
    GitTracked,
    /// The base set is what stands in the checkout, whether version control knows it or not.
    ///
    /// A corpus with no version control, or one that governs generated material
    /// before it is committed, answers this way. The removal below and every
    /// projection above behave identically over it, which is the whole point of
    /// making the base a declared kind.
    AsWritten,
}

impl UniverseKind {
    /// The answer's spelling, as the declaration writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitTracked => "git-tracked",
            Self::AsWritten => "as-written",
        }
    }

    /// Read a declared universe answer.
    ///
    /// An answer outside the pair reads as nothing rather than as a default. A
    /// misspelling that fell back to either kind would silently give a corpus a
    /// universe it did not declare, and the two differ in exactly the paths a
    /// reviewer would then be unable to account for.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "git-tracked" => Some(Self::GitTracked),
            "as-written" => Some(Self::AsWritten),
            _ => None,
        }
    }
}

/// One row of the global-ignore relation: a name to report it by, and a pattern.
///
/// The vocabulary is the owner surface's own rather than a second one invented
/// here. A name held to the declared-name grammar and a standard pattern matched
/// in full against a repository-relative byte path are what every other declared
/// exclusion already is, and a reader who has learnt one relation has learnt this
/// one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreRow {
    name: String,
    pattern: AbnfPattern,
}

impl IgnoreRow {
    /// Declare an ignore row by its name and its compiled pattern.
    #[must_use]
    pub fn new(name: impl Into<String>, pattern: AbnfPattern) -> Self {
        Self {
            name: name.into(),
            pattern,
        }
    }

    /// The name a report says this row removed a path by.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The pattern the row matches paths with.
    #[must_use]
    pub const fn pattern(&self) -> &AbnfPattern {
        &self.pattern
    }

    /// Whether this row reaches a path.
    #[must_use]
    pub fn matches(&self, path: &BytePath) -> bool {
        self.pattern.admits_path(path)
    }
}

/// The whole content of the shape document: the universe answer and the ignore relation.
///
/// The value is small on purpose. Everything else a corpus declares stands in the
/// parameter document of the policy that consumes it, so this type can never grow
/// into the omnibus deployment record the outline attack rejected — there is no
/// third field for one to arrive in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusShape {
    universe: UniverseKind,
    ignore: Vec<IgnoreRow>,
}

impl CorpusShape {
    /// Declare a corpus shape from its universe answer and its ignore rows.
    #[must_use]
    pub fn new(universe: UniverseKind, ignore: impl IntoIterator<Item = IgnoreRow>) -> Self {
        Self {
            universe,
            ignore: ignore.into_iter().collect(),
        }
    }

    /// The declared universe answer.
    #[must_use]
    pub const fn universe(&self) -> UniverseKind {
        self.universe
    }

    /// The declared ignore rows, in the order they were written.
    ///
    /// The order is carried and means nothing to the removal. It is kept so that
    /// a report naming a row names the row a reader can find, and so that a
    /// document round-trips through the decoder unchanged.
    #[must_use]
    pub fn ignore(&self) -> &[IgnoreRow] {
        &self.ignore
    }
}

/// What a shape document said that its schema does not admit.
///
/// The variants are the whole of what the document can get wrong, because the
/// document is closed at two content declarations: it is a document or it is not,
/// its universe answer is one of the closed pair or it is not, and each ignore row
/// carries a unique declared name and a pattern the engine compiles. A third
/// question refuses as a surplus key rather than being read, because widening this
/// document is an owner act and never a schema extension an implementation
/// inferred (´just:isolation:policy-data´).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeDefect {
    /// The text is not a well-formed document.
    Malformed(String),
    /// The universe answer is outside the closed pair.
    UnknownUniverse(String),
    /// An ignore row carries a pattern the engine declines.
    MalformedPattern {
        /// The row's own name, which is what a report can act on.
        name: String,
        /// The engine's account of why the value is not the form.
        message: String,
    },
    /// Two ignore rows answer to one name.
    ///
    /// The defect carries both patterns and not merely the name they share,
    /// because the repair is a choice between two regions and a reader holding
    /// only the name has to go and find them. What is unique is the name and
    /// never the region: two named rows may remove the same path, and one path
    /// answering to several rules is an ordinary thing for a union to do.
    DuplicateName {
        /// The name both rows answer to.
        name: String,
        /// The pattern of the row that stood first.
        first: String,
        /// The pattern of the row that repeated the name.
        second: String,
    },
    /// An ignore row states no name for the region it removes.
    ///
    /// Every row of this surface names the region it claims, and an ignore row
    /// is no exception: the name is what the accounting says when it reports the
    /// paths this row took out of the universe, and a row without one removes
    /// files for a reason no report can state.
    NamelessRow {
        /// The pattern the row did carry, which is all there is to identify it by.
        pattern: String,
    },
}

impl fmt::Display for ShapeDefect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(message) => write!(formatter, "not a well-formed document: {message}"),
            Self::UnknownUniverse(answer) => {
                write!(formatter, "`{answer}` is no declared universe answer")
            }
            Self::MalformedPattern { name, message } => {
                write!(formatter, "ignore: {name}: {message}")
            }
            Self::DuplicateName {
                name,
                first,
                second,
            } => write!(
                formatter,
                "ignore: {name}: two rows answer to one name, over `{first}` and `{second}`"
            ),
            Self::NamelessRow { pattern } => write!(
                formatter,
                "ignore: {pattern}: a row names the region it removes, and name is the word for it"
            ),
        }
    }
}

/// The shape document as its bytes stand, before either declaration is read.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShape {
    /// The allocated schema label the envelope carries, read by the envelope pass.
    #[serde(default, rename = "namespace")]
    _namespace: String,
    /// The schema's version triple, read by the envelope pass.
    #[serde(default, rename = "version")]
    _version: [u64; 3],
    #[serde(default)]
    universe: String,
    #[serde(default)]
    ignore: Vec<RawIgnoreRow>,
}

/// One ignore row as the document writes it.
///
/// The name is optional to the decoder and required by the surface. A row that
/// omits it is refused in this module's own words rather than by the parser's,
/// because an author who wrote a bare pattern was writing something the surface
/// has a sentence about, and a reader is owed that sentence rather than the name
/// of a missing field.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIgnoreRow {
    #[serde(default)]
    name: Option<String>,
    pattern: String,
}

/// Decode the shape document from the bytes standing in the declared surface.
///
/// One decoder rather than one per caller. The join and the loader both need the
/// universe answer and the ignore relation, and two decoders each claiming this
/// schema would be two places for it to drift apart.
///
/// # Errors
///
/// Returns every defect the document carries rather than only the first, because
/// an author repairing a document wants the whole list.
#[cfg(test)]
fn decode_shape(text: &str) -> Result<CorpusShape, Vec<ShapeDefect>> {
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(error) => return Err(vec![ShapeDefect::Malformed(error.to_string())]),
    };

    decode_shape_table(&table, text)
}

/// Project the corpus shape from an already-parsed surface document.
pub fn decode_shape_table(
    table: &toml::Table,
    text: &str,
) -> Result<CorpusShape, Vec<ShapeDefect>> {
    let raw: RawShape = match table.clone().try_into() {
        Ok(raw) => raw,
        Err(mut error) => {
            error.set_input(Some(text));

            return Err(vec![ShapeDefect::Malformed(error.to_string())]);
        }
    };

    let mut defects = Vec::new();
    let universe = UniverseKind::parse(&raw.universe);

    if universe.is_none() {
        defects.push(ShapeDefect::UnknownUniverse(raw.universe.clone()));
    }

    let mut rows = Vec::new();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();

    for row in &raw.ignore {
        let Some(name) = row.name.clone() else {
            defects.push(ShapeDefect::NamelessRow {
                pattern: row.pattern.clone(),
            });

            continue;
        };

        // Names are unique and regions are not. Two rows removing overlapping
        // paths are two true statements, and the union takes both; two rows
        // answering to one name are two answers to which region a report means.
        if let Some(first) = seen.get(&name) {
            defects.push(ShapeDefect::DuplicateName {
                name: name.clone(),
                first: first.clone(),
                second: row.pattern.clone(),
            });

            continue;
        }

        seen.insert(name.clone(), row.pattern.clone());

        match AbnfPattern::parse(&row.pattern) {
            Ok(pattern) => rows.push(IgnoreRow::new(name, pattern)),
            Err(defect) => defects.push(ShapeDefect::MalformedPattern {
                name,
                message: defect.to_string(),
            }),
        }
    }

    match universe {
        Some(universe) if defects.is_empty() => Ok(CorpusShape::new(universe, rows)),
        _ => Err(defects),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{CorpusShape, IgnoreRow, UniverseKind};
    use crate::declaration::AbnfPattern;
    use crate::pattern::BytePath;

    struct RetiringResolution {
        universe: UniverseKind,
        ignored: BTreeSet<BytePath>,
        surviving: BTreeSet<BytePath>,
    }

    fn retiring_resolution(
        shape: &CorpusShape,
        base: impl IntoIterator<Item = BytePath>,
    ) -> RetiringResolution {
        let mut ignored = BTreeSet::new();
        let mut surviving = BTreeSet::new();

        for path in base {
            if shape.ignore().iter().any(|row| row.matches(&path)) {
                ignored.insert(path);
            } else {
                surviving.insert(path);
            }
        }

        RetiringResolution {
            universe: shape.universe(),
            ignored,
            surviving,
        }
    }

    fn ignoring<'a>(shape: &'a CorpusShape, path: &BytePath) -> Option<&'a IgnoreRow> {
        shape.ignore().iter().find(|row| row.matches(path))
    }

    /// An invented base universe, of a corpus whose directories are not this one's.
    fn base() -> Vec<BytePath> {
        [
            "quarry/slate.md",
            "quarry/notes/draft.md",
            "spoil/heap.bin",
            "spoil/heap.log",
        ]
        .into_iter()
        .map(|path| BytePath::from_bytes(path.as_bytes()).expect("a relative path"))
        .collect()
    }

    fn row(name: &str, pattern: &str) -> IgnoreRow {
        IgnoreRow::new(
            name,
            AbnfPattern::parse(pattern).expect("a compiling pattern"),
        )
    }

    fn displays(paths: &BTreeSet<BytePath>) -> Vec<String> {
        paths.iter().map(BytePath::display).collect()
    }

    /// A corpus declares one of exactly two universe answers, and the resolver
    /// carries the answer it was given rather than resolving it away. An answer
    /// outside the pair reads as nothing rather than falling back to either kind,
    /// because a misspelling that defaulted would hand a corpus a universe it did
    /// not declare and the two kinds differ in exactly the paths a reviewer could
    /// then not account for.
    ///
    /// ´claim:universe:a-corpus-declares-one-of-exactly-two-universe-answers´
    /// ´test:unit:carries-either-declared-universe-answer´
    #[test]
    fn carries_either_declared_universe_answer() {
        for (text, kind) in [
            ("git-tracked", UniverseKind::GitTracked),
            ("as-written", UniverseKind::AsWritten),
        ] {
            assert_eq!(UniverseKind::parse(text), Some(kind), "the answer `{text}`");
            assert_eq!(kind.as_str(), text, "the spelling of `{text}`");
            let shape = CorpusShape::new(kind, []);
            assert_eq!(
                retiring_resolution(&shape, base()).universe,
                kind,
                "the resolver carries the answer `{text}`"
            );
        }

        for text in ["tracked", "git_tracked", "as-found", ""] {
            assert!(
                UniverseKind::parse(text).is_none(),
                "`{text}` is no declared universe answer"
            );
        }
    }

    /// An empty ignore relation is a live declaration that nothing is removed,
    /// and it leaves the base universe entire. That is the recommended state of a
    /// corpus whose universe answer and owner activations already resolve every
    /// exclusion class, and it must be distinguishable from a relation that
    /// removes something rather than being the same code path with no rows in it
    /// by accident.
    ///
    /// ´claim:universe:an-empty-ignore-relation-leaves-the-base-universe-entire´
    /// ´test:unit:an-empty-ignore-relation-removes-nothing´
    #[test]
    fn an_empty_ignore_relation_removes_nothing() {
        let shape = CorpusShape::new(UniverseKind::GitTracked, []);
        let resolved = retiring_resolution(&shape, base());

        assert!(
            resolved.ignored.is_empty(),
            "an empty relation removes nothing"
        );
        assert_eq!(
            displays(&resolved.surviving),
            vec![
                "quarry/notes/draft.md".to_owned(),
                "quarry/slate.md".to_owned(),
                "spoil/heap.bin".to_owned(),
                "spoil/heap.log".to_owned(),
            ],
            "the base universe survives entire"
        );
    }

    /// A nonempty relation removes the union of every matching row, so two rows
    /// reaching one path is ordinary rather than a conflict and the order they
    /// are written in decides nothing. Sequential application was the
    /// alternative, and it buys a precedence question nobody asked: a row moved
    /// for tidiness could then change which paths the corpus accounts for.
    ///
    /// ´claim:universe:the-ignore-relation-removes-the-union-of-its-matching-rows´
    /// ´test:unit:removes-the-union-of-every-matching-row´
    #[test]
    fn removes_the_union_of_every_matching_row() {
        let heap = row("spoil-tree", "%s\"spoil/\" *VCHAR");
        let logs = row("log-files", "*VCHAR %s\".log\"");

        let forwards_shape =
            CorpusShape::new(UniverseKind::AsWritten, [heap.clone(), logs.clone()]);
        let backwards_shape = CorpusShape::new(UniverseKind::AsWritten, [logs, heap]);
        let forwards = retiring_resolution(&forwards_shape, base());
        let backwards = retiring_resolution(&backwards_shape, base());

        assert_eq!(
            displays(&forwards.ignored),
            vec!["spoil/heap.bin".to_owned(), "spoil/heap.log".to_owned()],
            "the union of the two overlapping rows leaves the universe"
        );
        assert_eq!(
            displays(&forwards.surviving),
            vec![
                "quarry/notes/draft.md".to_owned(),
                "quarry/slate.md".to_owned()
            ],
            "everything no row reaches survives"
        );
        assert_eq!(
            forwards.surviving, backwards.surviving,
            "the rows removed a set, so their order changed nothing"
        );
    }

    /// A globally ignored path is absent from the set every projection is taken
    /// from rather than skipped by each of them in turn, so nothing above the
    /// removal can see it: the partition never attributes it, no policy selects
    /// it, and the carrier never reads it. The row that removed it is still
    /// nameable, because a report saying only that some row removed a path is not
    /// a report anybody can act on.
    ///
    /// ´claim:universe:an-ignored-path-is-absent-from-every-projection-above-it´
    /// ´test:unit:an-ignored-path-survives-in-no-projection´
    #[test]
    fn an_ignored_path_survives_in_no_projection() {
        let shape = CorpusShape::new(
            UniverseKind::GitTracked,
            [row("draft-notes", "%s\"quarry/notes/\" *VCHAR")],
        );
        let resolved = retiring_resolution(&shape, base());

        let draft =
            BytePath::from_bytes(b"quarry/notes/draft.md".to_vec()).expect("a relative path");
        let slate = BytePath::from_bytes(b"quarry/slate.md".to_vec()).expect("a relative path");

        assert!(
            !resolved.surviving.contains(&draft),
            "the removed path reaches no projection"
        );
        assert!(
            resolved.ignored.contains(&draft),
            "the removed path is accounted as removed"
        );
        assert!(
            resolved.surviving.contains(&slate),
            "its sibling is untouched"
        );

        assert_eq!(
            ignoring(&shape, &draft).map(IgnoreRow::name),
            Some("draft-notes"),
            "the row that removed it is nameable"
        );
        assert_eq!(
            ignoring(&shape, &slate),
            None,
            "no row reaches the surviving sibling"
        );
    }

    /// An ignore row's name is unique and its region is not. Two rows answering
    /// to one name refuse the document and the refusal carries both patterns,
    /// because the repair is a choice between two regions and the name alone
    /// does not say which two. Two differently named rows removing the very same
    /// path are no defect at all: a union takes both, and a path answering to
    /// several rules is an ordinary thing for one to do.
    ///
    /// ´claim:universe:an-ignore-name-is-unique-and-an-ignore-region-is-not´
    /// ´test:unit:an-ignore-name-is-unique-and-an-ignore-region-is-not´
    #[test]
    fn an_ignore_name_is_unique_and_an_ignore_region_is_not() {
        let repeated = "universe = \"git-tracked\"\n\
             ignore = [\n  \
             { name = \"spoil\", pattern = '%s\"spoil/\" *VCHAR' },\n  \
             { name = \"spoil\", pattern = '%s\"quarry/\" *VCHAR' },\n\
             ]\n";

        let defects = super::decode_shape(repeated).expect_err("two rows answer to one name");

        assert_eq!(
            defects,
            vec![super::ShapeDefect::DuplicateName {
                name: String::from("spoil"),
                first: String::from("%s\"spoil/\" *VCHAR"),
                second: String::from("%s\"quarry/\" *VCHAR"),
            }],
            "the defect names the name and both regions"
        );

        let overlapping = "universe = \"git-tracked\"\n\
             ignore = [\n  \
             { name = \"the-heap\", pattern = '%s\"spoil/\" *VCHAR' },\n  \
             { name = \"the-logs\", pattern = '[ *VCHAR \"/\" ] *VCHAR %s\".log\"' },\n\
             ]\n";

        let shape = super::decode_shape(overlapping).expect("overlapping regions are no defect");
        let resolved = retiring_resolution(&shape, base());

        assert_eq!(
            displays(&resolved.ignored),
            vec!["spoil/heap.bin".to_owned(), "spoil/heap.log".to_owned()],
            "one path answering to two named rows is removed once"
        );
    }

    /// An ignore row states the name of the region it removes, and a row
    /// carrying only a pattern is refused in the surface's own words rather than
    /// by the parser's. The name is what the accounting says when it reports the
    /// paths this row took out of the universe, so a row without one removes
    /// files for a reason no report can state — and an author who wrote a bare
    /// pattern is owed that sentence rather than the name of a missing field.
    ///
    /// ´claim:universe:a-nameless-ignore-row-refuses-in-the-surfaces-own-words´
    /// ´test:unit:a-nameless-ignore-row-refuses-in-the-surfaces-own-words´
    #[test]
    fn a_nameless_ignore_row_refuses_in_the_surfaces_own_words() {
        let nameless = "universe = \"git-tracked\"\n\
             ignore = [{ pattern = '%s\"spoil/\" *VCHAR' }]\n";

        let defects = super::decode_shape(nameless).expect_err("a row names the region it removes");

        assert_eq!(
            defects,
            vec![super::ShapeDefect::NamelessRow {
                pattern: String::from("%s\"spoil/\" *VCHAR"),
            }]
        );

        let rendered = defects[0].to_string();

        assert!(
            rendered.contains("a row names the region it removes, and name is the word for it"),
            "the refusal is the surface's own sentence: {rendered}"
        );
        assert!(
            !rendered.contains("missing field"),
            "the parser's sentence does not reach the reader: {rendered}"
        );
    }
}
