// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Parsing, declaration, and lookup for the linter's kind registry.
//!
//! Runtime construction starts from the accepted configuration snapshot. Its
//! base and repository-local relations are kept distinct in the declaration
//! and united in the effective lookup, as decided by the package-local registry
//! record (´dec:kindregistry:runtime-authority´). No Markdown document is a
//! runtime input.
//!
//! The Markdown parser remains because the repository-level synchronization
//! test projects the human registry into exact environment-name/kind pairs and
//! reserved kinds (´dec:kindregistry:sync-seam´). Convention-table rows with
//! kind tokens enter that projection. Device rows do not. A dagger is retained
//! as row status and removed from the exact catalogue name.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`reproduces_the_headline_counts_of_the_registry`] | registry | Reading the package-local fictional tables reproduces the fixture's expected counts of names, rows, kinds, and device classes. The parser's admission and normalization rules are therefore exercised without consulting repository data. |
//! | [`reads_ordinary_rows_and_their_conventions`] | registry | An ordinary fixture row is read as the classification it states — this name carries this kind — and keeps the Convention it was tabled under. |
//! | [`excludes_device_rows_and_keeps_hybrid_rows`] | registry | Declared hybrid rows are part of the relation and classify like any other, while device rows are not catalogued at all: a family of spellings is presentation rather than genre, carries no kind token, and is excluded from the fictional relation. |
//! | [`reads_the_dagger_as_a_status_mark`] | registry | The attestation mark is a status carried by a row and never a character of the name: a marked row classifies under its plain name, and the mark survives as the row's recorded standing, so the borderline entries can be listed as such without being spelled differently. |
//! | [`reserves_the_kinds_of_the_assets_convention`] | registry | The reserved kinds are drawn from the assets convention and from there alone: the kinds naming things a tool can derive are reserved, while the kinds naming things an author asserts stay authored. What a mint needs a warrant for is therefore decided by declared data, not by this module. |
//! | [`records_local_extensions_outside_the_documents_own_relation`] | registry | A locally added kind holds in the relation the checker actually uses and reserves its kind there, while standing outside the fixture's own rows and outside its headline counts. A local extension can therefore be tested without changing the parsed fixture. |
//! | [`catalogues_homonymous_names_under_several_kinds`] | registry | One name may be catalogued under several kinds: the classification is a relation and not a function, so a word used by more than one genre keeps all its readings and a head carrying any of them is well classified. |
//! | [`reduces_presentation_devices_to_the_base_name`] | registry | Presentation reduces away to the catalogued base name: an emphasising qualifier, a letter or number identifying an instance, a restatement note, a star, a parenthetical title, and a nesting prefix all leave the genre untouched. Prose may present an environment however it reads best. |
//! | [`catalogued_overriding_rows_beat_reduction`] | registry | A name the registry catalogues in its own right is never reduced away, even when it is shaped exactly like a qualifier and a base: it reduces to itself and carries its own kind rather than the kind reduction would have handed it. Being catalogued beats looking reducible. |
//! | [`validates_heads_against_the_relation`] | registry | Validating a head runs its name through reduction and then asks the relation, and answers with the base name it settled on — so a caller learns not merely that the head is well classified but what the registry took it to be. |
//! | [`reports_the_kinds_a_misclassified_head_could_have_carried`] | registry | A rejection says which of two things went wrong and gives the author what they need: a catalogued name carrying the wrong kind is reported with the kinds it could have carried, while a name the registry does not know is reported as simply uncatalogued. |

use std::collections::{BTreeMap, BTreeSet};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::label::Label;

/// The header cells that mark a table as one of the registry's Convention tables.
///
/// What contributes to the classification relation is fixed by the registry's
/// data signature: the ordinary rows of the Convention tables together with the
/// declared hybrid rows, and no heading occurrence, presentation device,
/// attestation record or generated presentation beside them
/// (´[EMBER-sig:kinds:registry-data]´). These two cells are how those tables are
/// told from every other table the document prints — the headline counts, the
/// hybrid triples, the attestation rows — so the pair is the whole admission
/// rule the signature asks for and not a formatting detail.
///
/// ´const:emberlinter:registry-table-marker´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:registry-table-marker-form-x7f03bc6a´
const CONVENTION_HEADER: [&str; 2] = ["Environment", "Kind"];

/// The Convention whose kinds this corpus reserves for derivation.
///
/// The calculus lets a corpus that also adopts a registry of kinds populate its
/// reserved set from that registry by its own recorded decision, consuming the
/// set and asking nothing of its provenance
/// (´[EMBER-sig:labels:reserved-kinds]´). This value is that decision's content,
/// and it names one Convention rather than a hand-listed set of kinds: the
/// assets-and-inventory family catalogues the labeled constructs of code, whose
/// mark is that the name is the code's own (´[EMBER-conv:kinds:assets]´), which
/// is exactly the family a profile derives rather than an author names.
///
/// ´const:emberlinter:derivable-kind-family´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:derivable-kind-family-text-x427c3899´
const RESERVED_CONVENTION: &str = "conv:kinds:assets";

/// The status mark the registry prints at a borderline row.
///
/// The attestation judgment fixes both the mark and its reading: the dagger
/// printed at a row is a status mark on the row and never a character of the
/// name, and the exact catalogue name is the row's name with the mark removed
/// (´[EMBER-judg:kinds:attestation]´). That is why one character carries two
/// facts here — stripped from the name, kept as the row's standing — and why a
/// row spelled with it classifies under its plain name.
///
/// ´const:emberlinter:attestation-status-mark´ (´[EMBER-alg:const:codepoint]´)
/// ´const:emberlinter:attestation-status-mark-codepoint-u2020´
const DAGGER: char = '†';

/// The emphasis and status modifiers the registry admits as presentation.
///
/// The definition of presentation reduction catalogues them by name — Main, Key,
/// Fundamental, Working, Standing, Blanket, Concrete, Motivating, Numerical,
/// Toy, Worked, and Running — as the modifiers a head may wear without leaving
/// the vocabulary (´[EMBER-def:kinds:presentation-reduction]´). The value is that
/// list and is closed by it: a thirteenth modifier is a new edition of the
/// registry rather than an addition here, and an expressly catalogued overriding
/// row — Working hypothesis, Standing hypothesis — beats the reduction these
/// words would otherwise licence.
///
/// ´const:emberlinter:emphasis-and-status-devices´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:emphasis-and-status-devices-form-x86d08518´
const MODIFIERS: &[&str] = &[
    "Main",
    "Key",
    "Fundamental",
    "Working",
    "Standing",
    "Blanket",
    "Concrete",
    "Motivating",
    "Numerical",
    "Toy",
    "Worked",
    "Running",
];

/// Restatement and continuation suffixes the registry admits as presentation.
///
/// Restatement and continuation stand among the devices presentation reduction
/// removes before a head is looked up, beside numbering, lettering and the rest
/// (´[EMBER-def:kinds:presentation-reduction]´), and the registry makes the same
/// point twice over — a restated theorem is its original returned to, and names
/// nothing new. What the definition fixes is that these two devices reduce away;
/// the value is the surface this corpus's prose spells them with, both as a
/// trailing note and as a parenthetical, so that a head wearing either is
/// looked up under the name it returns to.
///
/// ´const:emberlinter:continuation-devices´ (´[EMBER-alg:const:form]´)
/// ´const:emberlinter:continuation-devices-form-x55ee741d´
const RESTATEMENT_SUFFIXES: &[&str] = &[", restated", ", continued", " (continued)", " (restated)"];

/// The kind a document format's sectioning rung supplies for a heading.
///
/// Which environment class a document format declares for a head, and how a head
/// maps to one, are adoption data — the registry states that and leaves the
/// mapping to the adopting corpus (´[EMBER-def:kinds:presentation-reduction]´).
/// This corpus's format offers one rung and its nestings, and the registry's
/// sectioning Convention classifies a Section under this kind
/// (´[EMBER-conv:kinds:structure]´), so the datum is that one row: nesting within
/// the rung is the sub- prefix and is presentation, never a rank of its own, and
/// the ladder's other rungs — a Chapter, a Part, a Book — are rungs this format
/// does not offer and so are never supplied here.
///
/// ´const:emberlinter:format-heading-genre´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:format-heading-genre-word-sec´
pub const RUNG_KIND: &str = "sec";

/// The environment name a Markdown source's title supplies for its Title head.
///
/// Which environment class a document format declares for a head, and how a head
/// maps to one, are adoption data exactly as they are for the rung above
/// (´[EMBER-def:kinds:presentation-reduction]´). This corpus's format offers one
/// title, and the document-title record maps it to this name
/// (´[EMBER-dec:doctitles:title-kind]´). The name is the format's and never the
/// author's: a Title head is validated under this word however its title is
/// spelled, so the checker derives neither the head's class nor its label from
/// title text.
///
/// ´const:emberlinter:format-title-environment´ (´[EMBER-alg:const:text]´)
/// ´const:emberlinter:format-title-environment-text-x1471a974´
pub const TITLE_ENVIRONMENT: &str = "Document";

/// The attestation status the registry prints at a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// Ordinary attestation.
    Firm,
    /// Accepted, with its evidence qualified — the daggered rows.
    Borderline,
}

/// One row of the classification relation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row {
    name: String,
    kind: String,
    convention: Option<Label>,
    status: Status,
}

impl Row {
    /// The exact catalogue name, with any status mark removed.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The kind token this row assigns the name.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The Convention whose table carries this row, when the table has a mint.
    #[must_use]
    pub const fn convention(&self) -> Option<&Label> {
        self.convention.as_ref()
    }

    /// The attestation status printed at this row.
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }
}

/// Why a head did not validate against the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadDefect {
    /// The registry catalogues no such name, before or after reduction.
    UncataloguedName {
        /// The base name reduction produced.
        base: String,
    },
    /// The registry catalogues the name, but not under the kind the label carries.
    WrongKind {
        /// The base name reduction produced.
        base: String,
        /// The kinds the registry does assign that name.
        catalogued: Vec<String>,
    },
}

/// One declared classification relation and its repository-local extensions.
#[derive(Debug, Clone, Default)]
pub struct KindRegistry {
    rows: Vec<Row>,
    by_name: BTreeMap<String, BTreeSet<String>>,
    extensions: Vec<Row>,
    extension_by_name: BTreeMap<String, BTreeSet<String>>,
    reserved_kinds: BTreeSet<String>,
    device_rows: usize,
}

impl KindRegistry {
    /// Build a registry from its declared base relation and reserved-kind set.
    #[must_use]
    pub fn from_declared<'a>(
        rows: impl IntoIterator<Item = (&'a str, &'a str)>,
        reserved_kinds: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let mut registry = Self::default();

        for (name, kind) in rows {
            if !registry
                .by_name
                .entry(name.to_owned())
                .or_default()
                .insert(kind.to_owned())
            {
                continue;
            }

            registry.rows.push(Row {
                name: name.to_owned(),
                kind: kind.to_owned(),
                convention: None,
                status: Status::Firm,
            });
        }

        registry
            .reserved_kinds
            .extend(reserved_kinds.into_iter().map(str::to_owned));

        registry
    }

    /// Read a registry from the Markdown of a registry document.
    #[must_use]
    pub fn parse(document: &str) -> Self {
        let (rows, device_rows) = read_rows(document);

        let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut reserved_kinds = BTreeSet::new();

        for row in &rows {
            by_name
                .entry(row.name.clone())
                .or_default()
                .insert(row.kind.clone());

            if row
                .convention
                .as_ref()
                .is_some_and(|label| label.to_string() == RESERVED_CONVENTION)
            {
                reserved_kinds.insert(row.kind.clone());
            }
        }

        Self {
            rows,
            by_name,
            extensions: Vec::new(),
            extension_by_name: BTreeMap::new(),
            reserved_kinds,
            device_rows,
        }
    }

    /// The effective relation a corpus's own declared environment rows make.
    ///
    /// The registry's data signature lets an adopting corpus record extension rows
    /// of its own, whose effective relation is the registry's own union theirs, and
    /// is equally express that such a row is not a row of the registry — it becomes
    /// one only if a later edition incorporates it
    /// (ADR-T-011, Environment kinds). The rows are therefore held apart from
    /// the base relation throughout: its own row counts stay stable when a
    /// declared extension is added.
    ///
    /// The rows are read from the corpus's declaration rather than compiled beside
    /// it, and that is the whole of what this method changes. A compiled copy
    /// standing beside the declaration would be two authorities over one
    /// relation (´dec:kindregistry:runtime-authority´). A declared row the registry's own
    /// relation already carries is that relation's and is not repeated here.
    ///
    /// Reservation is a separate declared relation. Adding a name-to-kind row
    /// here neither reserves nor opens its kind; `with_reserved` applies the
    /// corresponding base or extension set explicitly.
    #[must_use]
    pub fn with_declared<'a>(&self, rows: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut registry = self.clone();

        registry.extensions = Vec::new();
        registry.extension_by_name = BTreeMap::new();

        for (name, kind) in rows {
            if self
                .by_name
                .get(name)
                .is_some_and(|kinds| kinds.contains(kind))
            {
                continue;
            }

            if !registry
                .extension_by_name
                .entry(name.to_owned())
                .or_default()
                .insert(kind.to_owned())
            {
                continue;
            }

            registry.extensions.push(Row {
                name: name.to_owned(),
                kind: kind.to_owned(),
                convention: None,
                status: Status::Firm,
            });
        }

        registry
    }

    /// Extend the reserved set with repository-local profile kinds.
    #[must_use]
    pub fn with_reserved<'a>(mut self, kinds: impl IntoIterator<Item = &'a str>) -> Self {
        self.reserved_kinds
            .extend(kinds.into_iter().map(str::to_owned));

        self
    }

    /// Every row of the relation, in document order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// How many device rows the tables present, which the relation excludes.
    #[must_use]
    pub const fn device_rows(&self) -> usize {
        self.device_rows
    }

    /// The exact catalogue names, counted after the dagger normalisation.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// The distinct kind tokens the relation assigns.
    #[must_use]
    pub fn kinds(&self) -> BTreeSet<&str> {
        self.rows.iter().map(|row| row.kind.as_str()).collect()
    }

    /// The kinds this corpus reserves for derivation.
    #[must_use]
    pub const fn reserved_kinds(&self) -> &BTreeSet<String> {
        &self.reserved_kinds
    }

    /// The local extension rows this corpus records beside the registry's own.
    #[must_use]
    pub fn extensions(&self) -> &[Row] {
        &self.extensions
    }

    /// Whether the effective relation holds of an exact catalogue name and a kind.
    #[must_use]
    pub fn classifies(&self, name: &str, kind: &str) -> bool {
        [&self.by_name, &self.extension_by_name]
            .iter()
            .any(|index| index.get(name).is_some_and(|kinds| kinds.contains(kind)))
    }

    /// The kinds the effective relation assigns an exact catalogue name.
    #[must_use]
    pub fn kinds_of(&self, name: &str) -> Vec<String> {
        let mut kinds: BTreeSet<String> = BTreeSet::new();

        for index in [&self.by_name, &self.extension_by_name] {
            if let Some(found) = index.get(name) {
                kinds.extend(found.iter().cloned());
            }
        }

        kinds.into_iter().collect()
    }

    /// Whether a name is an exact catalogue name of the effective relation.
    #[must_use]
    pub fn catalogues(&self, name: &str) -> bool {
        self.by_name.contains_key(name) || self.extension_by_name.contains_key(name)
    }

    /// The base name of an authored head, after the admitted devices are removed.
    ///
    /// The presentation-reduction definition
    /// (ADR-T-011, Environment kinds) lists the devices and gives
    /// an expressly catalogued overriding row precedence over reduction, so
    /// this returns a name the moment the relation catalogues it and only then
    /// removes another device.
    #[must_use]
    pub fn base_name(&self, head: &str) -> String {
        let mut name = head.trim().to_owned();

        loop {
            if self.catalogues(&name) {
                return name;
            }

            match reduce_once(&name) {
                Some(reduced) => name = reduced,
                None => return name,
            }
        }
    }

    /// Decide the head-validation judgment for an authored head and its kind.
    ///
    /// # Errors
    ///
    /// Returns the defect when the relation validates no pair for the head.
    pub fn validate_head(&self, head: &str, kind: &str) -> Result<String, HeadDefect> {
        let base = self.base_name(head);

        if self.classifies(&base, kind) {
            return Ok(base);
        }

        if self.catalogues(&base) {
            let catalogued = self.kinds_of(&base);

            return Err(HeadDefect::WrongKind { base, catalogued });
        }

        Err(HeadDefect::UncataloguedName { base })
    }
}

/// Build the package-local fictional registry used by integration tests.
#[doc(hidden)]
#[must_use]
pub fn fixture_kind_registry() -> KindRegistry {
    KindRegistry::parse(include_str!("../tests/fixtures/kind-registry.txt"))
        .with_declared([
            ("To-do", "todo"),
            ("Constant", "const"),
            ("Document", "guide"),
            ("Plan", "plan"),
            ("Backlog", "plan"),
            ("Guide", "guide"),
            ("Readme", "guide"),
            ("Manual", "guide"),
        ])
        .with_reserved(["const", "legacy", "todo"])
}

/// Remove exactly one presentation device from a head, when one is present.
///
/// The devices are tried in the order that keeps each removal unambiguous: what
/// the format attaches last is removed first, so a lettered restatement of a named
/// theorem reduces through its own spelling rather than through a guess.
fn reduce_once(name: &str) -> Option<String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return None;
    }

    if let Some(stripped) = strip_suffixes(trimmed, RESTATEMENT_SUFFIXES) {
        return Some(stripped);
    }

    if let Some(stripped) = trimmed.strip_suffix('*') {
        return Some(stripped.trim_end().to_owned());
    }

    if let Some(stripped) = strip_attached_name(trimmed) {
        return Some(stripped);
    }

    if let Some(stripped) = strip_numbering(trimmed) {
        return Some(stripped);
    }

    if let Some(stripped) = strip_sub_prefix(trimmed) {
        return Some(stripped);
    }

    strip_modifier(trimmed)
}

/// Remove a trailing restatement or continuation suffix.
fn strip_suffixes(name: &str, suffixes: &[&str]) -> Option<String> {
    suffixes
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))
        .map(|stripped| stripped.trim_end().to_owned())
}

/// Remove a parenthesised attached name, which names but does not classify.
fn strip_attached_name(name: &str) -> Option<String> {
    let without = name.strip_suffix(')')?;
    let opening = without.rfind(" (")?;

    Some(without[..opening].trim_end().to_owned())
}

/// Remove a trailing numbering or lettering device.
///
/// A device is a final token of digits and dots, or a single capital letter: the
/// spellings the registry names as Theorem 1.1 and Theorem A.
fn strip_numbering(name: &str) -> Option<String> {
    let (head, last) = name.rsplit_once(' ')?;

    let is_number = !last.is_empty()
        && last
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.');
    let is_letter = last.len() == 1 && last.chars().all(|character| character.is_ascii_uppercase());

    (is_number || is_letter).then(|| head.trim_end().to_owned())
}

/// Remove one iterated sub- prefix, so a subsection reduces towards a section.
fn strip_sub_prefix(name: &str) -> Option<String> {
    let rest = name
        .strip_prefix("Sub")
        .or_else(|| name.strip_prefix("sub"))?;

    (!rest.is_empty()).then(|| capitalize(rest))
}

/// Remove one leading emphasis or status modifier.
fn strip_modifier(name: &str) -> Option<String> {
    let (first, rest) = name.split_once(' ')?;

    MODIFIERS
        .iter()
        .any(|modifier| modifier.eq_ignore_ascii_case(first))
        .then(|| capitalize(rest.trim_start()))
}

/// Raise a name's first character, so a stripped prefix leaves a catalogue name.
fn capitalize(text: &str) -> String {
    let mut characters = text.chars();

    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// Read every Convention row of a registry document, and count the device rows.
fn read_rows(document: &str) -> (Vec<Row>, usize) {
    let options = Options::ENABLE_TABLES;

    let mut rows = Vec::new();
    let mut device_rows = 0;
    let mut convention: Option<Label> = None;

    let mut in_table = false;
    let mut header: Vec<String> = Vec::new();
    let mut cells: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_cell = false;
    let mut code_block_depth = 0_usize;

    for (event, range) in Parser::new_ext(document, options).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_block_depth = code_block_depth.saturating_sub(1),
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                header.clear();
            }
            Event::End(TagEnd::Table) => in_table = false,
            Event::Start(Tag::TableHead | Tag::TableRow) => cells.clear(),
            Event::End(TagEnd::TableHead) => header = std::mem::take(&mut cells),
            Event::End(TagEnd::TableRow) => {
                if header == CONVENTION_HEADER {
                    match read_row(&cells, convention.as_ref()) {
                        Some(row) => rows.push(row),
                        None => device_rows += 1,
                    }
                }
            }
            Event::Start(Tag::TableCell) => {
                in_cell = true;
                cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                in_cell = false;
                cells.push(cell.trim().to_owned());
            }
            Event::Text(text) if in_cell => cell.push_str(&text),
            Event::Code(text) if in_cell => cell.push_str(&text),
            Event::Code(text) if !in_table && code_block_depth == 0 => {
                if let Some(label) = mint_at(document, range.start, range.end, &text) {
                    convention = Some(label);
                }
            }
            _ => {}
        }
    }

    (rows, device_rows)
}

/// Read one body row of a Convention table, or report it as a device row.
fn read_row(cells: &[String], convention: Option<&Label>) -> Option<Row> {
    let [name, kind] = cells else {
        return None;
    };

    if !is_kind_token(kind) {
        return None;
    }

    let (name, status) = normalize_name(name);

    Some(Row {
        name,
        kind: kind.clone(),
        convention: convention.cloned(),
        status,
    })
}

/// Whether a kind cell carries a kind token rather than a device row's dash.
fn is_kind_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// Separate a row's exact catalogue name from the status mark printed at it.
fn normalize_name(text: &str) -> (String, Status) {
    text.strip_suffix(DAGGER).map_or_else(
        || (text.trim().to_owned(), Status::Firm),
        |name| (name.trim_end().to_owned(), Status::Borderline),
    )
}

/// Read a code span as a mint, when it is a bare, single-delimited label span.
fn mint_at(document: &str, start: usize, end: usize, interior: &str) -> Option<Label> {
    let opening = document
        .get(start..end)?
        .bytes()
        .take_while(|byte| *byte == b'`')
        .count();

    if opening != 1 {
        return None;
    }

    let before = document.get(..start)?.chars().next_back();

    if before == Some('(') {
        return None;
    }

    Label::parse(interior)
}

#[cfg(test)]
mod tests {
    use super::{HeadDefect, KindRegistry, Status, fixture_kind_registry};

    fn fixture() -> KindRegistry {
        fixture_kind_registry()
    }

    /// Reading the package-local fictional tables reproduces the fixture's
    /// expected counts of names, rows, kinds, and device classes. The parser's
    /// admission and normalization rules are therefore exercised without
    /// consulting repository data.
    ///
    /// ´claim:registry:reading-the-record-reproduces-its-own-headline-counts´
    /// ´test:unit:reproduces-the-headline-counts-of-the-registry´
    #[test]
    fn reproduces_the_headline_counts_of_the_registry() {
        let registry = fixture();

        assert_eq!(registry.names().count(), 32, "names");
        assert_eq!(registry.rows().len(), 35, "rows");
        assert_eq!(registry.kinds().len(), 33, "kinds");
        assert_eq!(registry.device_rows(), 4, "device classes");
    }

    /// An ordinary fixture row is read as the classification it states — this
    /// name carries this kind — and keeps the Convention it was tabled under.
    ///
    /// ´claim:registry:an-ordinary-row-classifies-its-name-and-keeps-its-convention´
    /// ´test:unit:reads-ordinary-rows-and-their-conventions´
    #[test]
    fn reads_ordinary_rows_and_their_conventions() {
        let registry = fixture();

        assert!(registry.classifies("Theorem", "thm"));
        assert!(registry.classifies("Definition", "def"));
        assert!(registry.classifies("Section", "sec"));
        assert!(registry.classifies("Entry", "entry"));
        assert!(registry.classifies("Desideratum", "goal"));

        let theorem = registry
            .rows()
            .iter()
            .find(|row| row.name() == "Theorem")
            .expect("a catalogued row");

        assert_eq!(
            theorem.convention().map(ToString::to_string).as_deref(),
            Some("conv:kinds:results")
        );
    }

    /// Declared hybrid rows are part of the relation and classify like any
    /// other, while device rows are not catalogued at all: a family of
    /// spellings is presentation rather than genre, carries no kind token, and
    /// is excluded from the fictional relation.
    ///
    /// ´claim:registry:hybrid-rows-are-catalogued-and-device-rows-are-not´
    /// ´test:unit:excludes-device-rows-and-keeps-hybrid-rows´
    #[test]
    fn excludes_device_rows_and_keeps_hybrid_rows() {
        let registry = fixture();

        assert!(registry.classifies("Definition–Proposition", "defprop"));
        assert!(registry.classifies("Definition–Theorem", "defthm"));
        assert!(registry.classifies("Lemma–Definition", "lemdef"));

        assert!(!registry.catalogues("Lettered main theorems (Theorem A, Theorem B, …)"));
        assert!(!registry.catalogues("Starred/unnumbered variants (theorem*, etc.)"));
    }

    /// The attestation mark is a status carried by a row and never a character
    /// of the name: a marked row classifies under its plain name, and the mark
    /// survives as the row's recorded standing, so the borderline entries can
    /// be listed as such without being spelled differently.
    ///
    /// ´claim:registry:the-attestation-mark-is-a-row-status-not-part-of-the-name´
    /// ´test:unit:reads-the-dagger-as-a-status-mark´
    #[test]
    fn reads_the_dagger_as_a_status_mark() {
        let registry = fixture();

        assert!(
            registry.classifies("Yoga", "yoga"),
            "the mark is not part of the name"
        );
        assert!(registry.classifies("Meta-question", "metaq"));
        assert!(registry.classifies("Schema", "dataschema"));

        let borderline: Vec<&str> = registry
            .rows()
            .iter()
            .filter(|row| row.status() == Status::Borderline)
            .map(super::Row::name)
            .collect();

        assert_eq!(borderline, ["Meta-question", "Schema", "Yoga"]);
    }

    /// The reserved kinds are drawn from the assets convention and from there
    /// alone: the kinds naming things a tool can derive are reserved, while the
    /// kinds naming things an author asserts stay authored. What a mint needs a
    /// warrant for is therefore decided by declared data, not by this module.
    ///
    /// ´claim:registry:the-reserved-kinds-are-the-assets-conventions-kinds´
    /// ´test:unit:reserves-the-kinds-of-the-assets-convention´
    #[test]
    fn reserves_the_kinds_of_the_assets_convention() {
        let registry = fixture();
        let reserved = registry.reserved_kinds();

        for kind in [
            "test", "bench", "mod", "pkg", "func", "endpoint", "envvar", "type", "lint",
        ] {
            assert!(reserved.contains(kind), "expected `{kind}` to be reserved");
        }

        for kind in [
            "sec", "lang", "gram", "sig", "judg", "inf", "inv", "metathm", "cav", "gate",
        ] {
            assert!(!reserved.contains(kind), "expected `{kind}` to be authored");
        }
    }

    /// A locally added kind holds in the relation the checker actually uses and
    /// reserves its kind there, while standing outside the fixture's own rows
    /// and outside its headline counts. A local extension can therefore be
    /// tested without changing the parsed fixture.
    ///
    /// ´claim:registry:a-local-extension-holds-in-the-relation-without-entering-the-record´
    /// ´test:unit:records-local-extensions-outside-the-documents-own-relation´
    #[test]
    fn records_local_extensions_outside_the_documents_own_relation() {
        let registry = &fixture().with_declared([
            ("To-do", "todo"),
            ("Constant", "const"),
            ("Document", "doc"),
        ]);

        assert!(
            registry.classifies("To-do", "todo"),
            "the effective relation holds it"
        );
        assert!(
            registry.reserved_kinds().contains("todo"),
            "and reserves its kind"
        );
        assert!(
            registry.classifies("Constant", "const"),
            "and holds the second row too"
        );
        assert!(
            registry.reserved_kinds().contains("const"),
            "reserving its kind as well"
        );
        assert!(
            registry.classifies("Document", "doc"),
            "and the title row beside them"
        );
        assert!(
            !registry.reserved_kinds().contains("doc"),
            "whose kind stays authored, being tabled outside the assets family"
        );

        for kind in ["todo", "const", "doc"] {
            assert!(
                !registry.rows().iter().any(|row| row.kind() == kind),
                "but a local extension is no row of the registry's own relation"
            );
        }
        for name in ["To-do", "Constant", "Document"] {
            assert!(
                !registry.names().any(|catalogued| catalogued == name),
                "so the fixture's own headline counts are untouched"
            );
        }
        assert_eq!(registry.extensions().len(), 3);
    }

    /// One name may be catalogued under several kinds: the classification is a
    /// relation and not a function, so a word used by more than one genre keeps
    /// all its readings and a head carrying any of them is well classified.
    ///
    /// ´claim:registry:one-name-may-be-catalogued-under-several-kinds´
    /// ´test:unit:catalogues-homonymous-names-under-several-kinds´
    #[test]
    fn catalogues_homonymous_names_under_several_kinds() {
        let registry = fixture();

        assert_eq!(
            registry.kinds_of("Structure"),
            ["class", "constr", "schema"]
        );
        assert_eq!(registry.kinds_of("Model"), ["constr", "model"]);
    }

    /// Presentation reduces away to the catalogued base name: an emphasising
    /// qualifier, a letter or number identifying an instance, a restatement
    /// note, a star, a parenthetical title, and a nesting prefix all leave the
    /// genre untouched. Prose may present an environment however it reads best.
    ///
    /// ´claim:registry:presentation-devices-reduce-to-the-catalogued-base-name´
    /// ´test:unit:reduces-presentation-devices-to-the-base-name´
    #[test]
    fn reduces_presentation_devices_to_the_base_name() {
        let registry = fixture();

        for (head, base) in [
            ("Theorem", "Theorem"),
            ("Main Theorem", "Theorem"),
            ("Key Lemma", "Lemma"),
            ("Theorem A", "Theorem"),
            ("Theorem 1.1", "Theorem"),
            ("Theorem 1.1, restated", "Theorem"),
            ("Theorem*", "Theorem"),
            ("Theorem (Riemann–Roch)", "Theorem"),
            ("Subsection", "Section"),
            ("Subsubsection", "Section"),
            ("Toy Example", "Example"),
            ("Worked Example", "Example"),
            ("Running Example", "Example"),
        ] {
            assert_eq!(registry.base_name(head), base, "reduction of `{head}`");
        }
    }

    /// A name the registry catalogues in its own right is never reduced away,
    /// even when it is shaped exactly like a qualifier and a base: it reduces
    /// to itself and carries its own kind rather than the kind reduction would
    /// have handed it. Being catalogued beats looking reducible.
    ///
    /// ´claim:registry:a-catalogued-name-beats-the-reduction-that-would-strip-it´
    /// ´test:unit:catalogued-overriding-rows-beat-reduction´
    #[test]
    fn catalogued_overriding_rows_beat_reduction() {
        let registry = fixture();

        assert_eq!(
            registry.base_name("Working hypothesis"),
            "Working hypothesis"
        );
        assert_eq!(
            registry.base_name("Standing hypothesis"),
            "Standing hypothesis"
        );

        assert!(registry.classifies("Working hypothesis", "assum"));
        assert!(!registry.classifies("Working hypothesis", "hyp"));
    }

    /// Validating a head runs its name through reduction and then asks the
    /// relation, and answers with the base name it settled on — so a caller
    /// learns not merely that the head is well classified but what the registry
    /// took it to be.
    ///
    /// ´claim:registry:head-validation-answers-with-the-base-name-it-settled-on´
    /// ´test:unit:validates-heads-against-the-relation´
    #[test]
    fn validates_heads_against_the_relation() {
        let registry = fixture();

        assert_eq!(
            registry.validate_head("Theorem", "thm"),
            Ok("Theorem".to_owned())
        );
        assert_eq!(
            registry.validate_head("Main Theorem", "thm"),
            Ok("Theorem".to_owned())
        );
        assert_eq!(
            registry.validate_head("Entry", "entry"),
            Ok("Entry".to_owned())
        );
    }

    /// A rejection says which of two things went wrong and gives the author
    /// what they need: a catalogued name carrying the wrong kind is reported
    /// with the kinds it could have carried, while a name the registry does not
    /// know is reported as simply uncatalogued.
    ///
    /// ´claim:registry:a-rejected-head-is-told-which-kinds-it-could-have-carried´
    /// ´test:unit:reports-the-kinds-a-misclassified-head-could-have-carried´
    #[test]
    fn reports_the_kinds_a_misclassified_head_could_have_carried() {
        let registry = fixture();

        assert_eq!(
            registry.validate_head("Theorem", "def"),
            Err(HeadDefect::WrongKind {
                base: "Theorem".to_owned(),
                catalogued: vec!["thm".to_owned()],
            })
        );

        assert_eq!(
            registry.validate_head("Blancmange", "def"),
            Err(HeadDefect::UncataloguedName {
                base: "Blancmange".to_owned(),
            })
        );
    }
}
