// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Environment heads: what heads an environment, and which mint is its own.
//!
//! ADR-T-014 says of its own corpus that the label at each heading or
//! environment head is that environment's mint. That sentence names two head
//! shapes, and this module recognises both. A heading is the shape the document
//! format supplies: its rung is a division, and the presentation-reduction
//! presentation-reduction definition in ADR-T-011, Environment kinds, makes
//! the rung the base a named division reduces to. A bold run opening a
//! paragraph — the environment's name, then the name attached to this instance
//! of it — is the shape the corpus writes where the format offers no
//! environment of its own.
//!
//! Pairing a head with its mint is the module's whole job, and it is what the
//! later judgments need. Head validation asks which kind a head's name may carry,
//! and cannot ask before it knows which name goes with which label. So both ways
//! of separating the two are findings: a head that mints nothing has no identity,
//! and a mint standing away from every head names nothing.
//!
//! Two shapes of head deserve their own note. A heading may leave its naming to
//! the bold run below it — the heading mints, and the bold run says which
//! environment the mint names — and where it does, that bold run is not a head of
//! its own and is not asked for a mint. And a bold run that carries no attached
//! name is not a head at all: a paragraph opening in bold is how running prose
//! emphasises its first words, and reading every such paragraph as an environment
//! would find heads in text that heads nothing.
//!
//! # The Title head
//!
//! A third head class stands beside those two, and the format supplies it as
//! surely as it supplies the rung. ADR-T-024 adds Title to the head classes
//! declared for Markdown and maps it to one environment name, Document, whose
//! kind the acceptee's extension set records
//! (ADR-T-024, Document-title labels). The first structural level-one heading
//! of a source is that head and the only one, even where front matter precedes
//! it; a later level-one heading is an ordinary division and mints a label of
//! its own, judged against the name it writes
//! (ADR-T-024, Document-title labels).
//!
//! Two asymmetries with the heads above are deliberate and both come from the
//! same place. A Title head that mints nothing is passed over rather than
//! reported, because this stage makes a title mint possible and valid while the
//! stage after it is what makes one required — reporting the corpus's titles
//! here would report the migration's backlog as today's defects
//! (ADR-T-024, Document-title labels). And a Title head is asked about
//! under the name the format supplies rather than the words it is titled with,
//! so no identity is derived from title text.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`pairs_a_bold_head_with_its_mint`] | head | A paragraph opening with a bold run that names an environment and the instance of it, followed by a label, is a head paired with that mint. This is the shape the corpus writes where the document format offers no environment of its own. |
//! | [`pairs_a_heading_with_its_mint_as_a_division`] | head | A heading carrying a label is a head too, and the environment it heads is a division — the one shape the document format supplies itself. The heading's level is kept, so how deeply it sits is part of what it is. |
//! | [`lets_a_bold_run_name_the_heading_above_it`] | head | A heading may leave its naming to the bold run beneath it: the heading carries the mint, the run says which environment that mint names, and the two are one head. The naming run is not a head of its own and is therefore never asked for a mint it does not have. |
//! | [`reads_no_head_in_a_paragraph_that_merely_opens_in_bold`] | head | Bold with no name attached to it heads nothing: a paragraph beginning with an emphasised word is how running prose stresses its opening, and reading every such paragraph as an environment would find heads throughout text that heads nothing. |
//! | [`leaves_the_document_title_out_of_the_heads`] | head | A title that mints nothing is passed over rather than reported: this stage makes a title mint possible and valid, and the stage after it is what makes one required, so a titled source carrying no document label is still in good standing. |
//! | [`pairs_a_lone_title_with_its_document_mint`] | head | The first structural level-one heading of a source is its Title head, and the mint it carries names the document: the head is paired with that mint like any other, and the environment it heads is the whole source rather than one of its divisions. |
//! | [`titles_a_front_mattered_source_at_its_first_heading`] | head | Front matter standing before the title changes nothing: the rule is the first structural level-one heading and not the first line, so a source opening with a preamble still titles itself at the heading beneath it. |
//! | [`reads_a_later_level_one_heading_as_a_division`] | head | Only the first structural level-one heading is a title. A later one is an ordinary division head and mints its own division label, so a long specification keeps its parts and appendices without minting several document concepts. |
//! | [`judges_a_document_mint_away_from_the_title_by_the_name_its_head_writes`] | head | Where a mint of the Document environment's kind stands is no judgment of its own. A bold run writing that environment's name carries the pair the relation catalogues and validates wherever it opens a paragraph, while a heading minting the same kind is asked about under the word the heading writes and fails there, exactly as any head fails whose name the catalogue does not give its label's kind. |
//! | [`takes_the_title_class_at_the_first_of_two_document_mints`] | head | A source has at most one Title head: the first structural level-one heading takes the class, and a second one heading a division of the same source is a division head like any other. Both mints pair with their heads, and the second is answered for under the word its own heading writes rather than under the environment the format gave the first. |
//! | [`validates_a_title_head_under_every_kind_the_relation_rows_it`] | head | A Title head is decided by the pair its format-supplied environment name makes with the kind its mint carries, so the Document environment carries as many senses as the relation rows against that name: a title minting any one of them validates, and a title minting a kind the relation rows against the name nowhere fails at the catalogue as any head would. |
//! | [`validates_a_title_head_through_the_acceptee`] | head | The Title head validates through the acceptee's extension set rather than through the words the source is titled with: the format supplies the environment name, the local pair supplies its kind, and a title minting any other kind is reported against that name. |
//! | [`leaves_a_titleless_source_untouched`] | head | A source with no structural level-one heading has no Title head, and nothing about it changes: its divisions head as they did, and no title, concept or label is invented for it. |
//! | [`reads_no_title_in_a_fenced_level_one_heading`] | head | A level-one heading drawn inside a fenced block is text rather than a heading, so it neither takes the title class from a real title beneath it nor supplies one to a source that has none. |
//! | [`reports_a_head_that_mints_nothing`] | head | A head that mints nothing has no identity and is reported by name, whether it is a bold run or a heading. Only a head already named by the division above it escapes the requirement, because that one is not a head of its own. |
//! | [`reports_a_mint_that_heads_nothing`] | head | A mint standing in running prose, away from every head, names nothing and is reported carrying the label it wrote. Minting and heading are two halves of one act, and neither half stands alone. |
//! | [`fails_a_head_whose_kind_the_registry_denies_its_name`] | head | A head whose name the registry catalogues under a different kind than the one its label carries is a failure, and the report names both the head's base word and the kind the registry actually gives it, so the author is told what to change and to what. |
//! | [`fails_a_head_the_registry_does_not_catalogue`] | head | A head whose name the registry does not catalogue at all is a failure naming that word, so the vocabulary of environments is the registered one and cannot be widened by inventing a head. |
//! | [`validates_a_head_through_presentation_reduction`] | head | A head is validated after its presentation is reduced away: a qualified name reduces to the catalogued word it modifies, and a heading at any depth reduces to the division rung. Prose may therefore read naturally without stepping outside the registered vocabulary. |

use std::path::Path;

use crate::finding::{Finding, Location};
use crate::label::Label;
use crate::occurrence::Occurrence;
use crate::prose::{BlockKind, ProseBlock};
use crate::registry::{HeadDefect, KindRegistry, RUNG_KIND, TITLE_ENVIRONMENT};

/// Which shape headed an environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadStyle {
    /// The format's sectioning rung, at the given depth.
    Heading {
        /// How deep the rung sits, with the document title at one.
        level: usize,
    },
    /// A bold run opening a paragraph.
    Bold,
}

/// How a head names the environment it heads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadName {
    /// The head writes the environment's name: a bold run, or a heading's.
    Named(String),
    /// The head names a division, whose base the format's rung supplies.
    Division(String),
    /// The head titles the source, whose base the format's title supplies.
    Title(String),
}

impl HeadName {
    /// The name as written, whichever way.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Named(name) | Self::Division(name) | Self::Title(name) => name,
        }
    }

    /// The name the registry is asked about, which the format may supply.
    ///
    /// A Title head is named by the format and never by its own spelling: the
    /// document-title record maps the class to one environment name
    /// (ADR-T-024, Document-title labels), and validating under that name is
    /// what sends the head through the acceptee's extension set rather than
    /// through the words an author happened to title the source with. Every
    /// other head is asked about under the name it writes.
    #[must_use]
    pub fn catalogue_name(&self) -> &str {
        match self {
            Self::Named(name) | Self::Division(name) => name,
            Self::Title(_) => TITLE_ENVIRONMENT,
        }
    }
}

/// One environment head, paired with the mint that names its environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    style: HeadStyle,
    name: HeadName,
    label: Label,
    location: Location,
}

impl Head {
    /// Which shape headed the environment.
    #[must_use]
    pub const fn style(&self) -> HeadStyle {
        self.style
    }

    /// How the head names its environment.
    #[must_use]
    pub const fn name(&self) -> &HeadName {
        &self.name
    }

    /// The label this head mints.
    #[must_use]
    pub const fn label(&self) -> &Label {
        &self.label
    }

    /// Where the head stands.
    #[must_use]
    pub const fn location(&self) -> &Location {
        &self.location
    }
}

/// Read the environment heads of one scanned source, and what failed to pair.
///
/// The occurrences are the source's participating occurrences, and the blocks its
/// heading and paragraph blocks, both as the prose scan produced them.
#[must_use]
pub fn read_heads(
    path: &Path,
    source: &str,
    blocks: &[ProseBlock],
    occurrences: &[Occurrence],
) -> (Vec<Head>, Vec<Finding>) {
    let mints: Vec<&Occurrence> = occurrences
        .iter()
        .filter(|occurrence| occurrence.is_mint())
        .collect();

    let mut heads = Vec::new();
    let mut findings = Vec::new();
    let mut claimed = vec![false; mints.len()];
    let mut naming = vec![false; blocks.len()];
    let title = title_block(blocks);

    for (index, block) in blocks.iter().enumerate() {
        if naming[index] {
            continue;
        }

        let Some(shape) = head_shape(block, title == Some(index)) else {
            continue;
        };

        let mint = mints
            .iter()
            .enumerate()
            .find(|(position, mint)| !claimed[*position] && block.holds(mint.location().offset()))
            .map(|(position, _mint)| position);

        let Some(position) = mint else {
            // A Title head that mints nothing is asked for nothing. Recognition
            // makes a title mint possible and valid; requiring one of every
            // titled source is the mint sweep's stage, and reporting the whole
            // corpus's titles here would report that migration's backlog as
            // today's defects (ADR-T-024, Document-title labels).
            if matches!(shape, Shape::Title(_)) {
                continue;
            }

            let (Shape::Bold(head) | Shape::Division(head) | Shape::Title(head)) = shape;

            findings.push(Finding::MintlessHead {
                head,
                location: Location::new(path, source, block.start()),
            });
            continue;
        };

        claimed[position] = true;

        let name = match shape {
            Shape::Bold(name) => HeadName::Named(name),
            Shape::Title(title) => HeadName::Title(title),
            Shape::Division(title) => {
                match naming_block(blocks, index, &mints, mints[position].label()) {
                    Some((naming_index, name)) => {
                        naming[naming_index] = true;

                        HeadName::Named(name)
                    }
                    None => HeadName::Division(title),
                }
            }
        };

        heads.push(Head {
            style: style_of(block),
            name,
            label: mints[position].label().clone(),
            location: mints[position].location().clone(),
        });
    }

    for (position, mint) in mints.iter().enumerate() {
        if !claimed[position] {
            findings.push(Finding::HeadlessMint {
                label: mint.label().clone(),
                location: mint.location().clone(),
            });
        }
    }

    findings.sort_by_key(|finding| finding.primary_location().map_or(0, Location::offset));

    (heads, findings)
}

/// Validate every head of a source against the registry.
#[must_use]
pub fn validate_heads(registry: &KindRegistry, heads: &[Head]) -> Vec<Finding> {
    heads
        .iter()
        .filter_map(|head| validate_head(registry, head))
        .collect()
}

/// Validate one head, reporting the defect when the judgment does not hold.
///
/// Nothing here reads which kind a head's mint carries except to settle one
/// case the relation cannot be asked about. A division's base is the rung the
/// format supplies rather than the word the heading is written with
/// (ADR-T-011, Environment kinds), so a division minting the rung
/// kind is the format's own pair and is decided here; every other head, the
/// source's Title among them, goes to the relation under its catalogue name and
/// is decided by the pair it makes there.
fn validate_head(registry: &KindRegistry, head: &Head) -> Option<Finding> {
    let kind = head.label.kind();

    if matches!(head.name, HeadName::Division(_)) && kind == RUNG_KIND {
        return None;
    }

    let name = head.name.catalogue_name();

    match registry.validate_head(name, kind) {
        Ok(_base) => None,
        Err(HeadDefect::WrongKind { base, catalogued }) => Some(Finding::MisclassifiedHead {
            head: name.to_owned(),
            base,
            label: head.label.clone(),
            catalogued,
            location: head.location.clone(),
        }),
        Err(HeadDefect::UncataloguedName { base }) => Some(Finding::UncataloguedHead {
            head: name.to_owned(),
            base,
            label: head.label.clone(),
            location: head.location.clone(),
        }),
    }
}

/// What a block heads, when it heads anything.
enum Shape {
    /// A bold run naming an environment and the name attached to this instance.
    Bold(String),
    /// A heading, whose base the format's rung supplies unless a bold run names it.
    Division(String),
    /// The source's title, whose base the format's title supplies.
    Title(String),
}

/// Which block is the source's Title head, when the source has one.
///
/// The rule is structural order and nothing else: the first structural level-one
/// heading is the sole title head, even when front matter precedes it
/// (ADR-T-024, Document-title labels). The blocks handed here are already
/// the structural ones — a level-one heading drawn inside a fenced block is text
/// the scan never raised to a block — so the first of them at that rung is the
/// answer, and a source with no such heading has no Title head at all.
///
/// Reading the first line rather than the first level-one block would miss a
/// front-mattered source, and reading every level-one heading as a title would
/// mint several document concepts in one document; a later one therefore stays
/// an ordinary division head.
fn title_block(blocks: &[ProseBlock]) -> Option<usize> {
    blocks
        .iter()
        .position(|block| matches!(block.kind(), BlockKind::Heading { level: 1 }))
}

/// Decide what a block heads.
fn head_shape(block: &ProseBlock, is_title: bool) -> Option<Shape> {
    match block.kind() {
        BlockKind::Heading { level } => {
            let title = division_title(block.text());

            // A level-one heading heads the document when it is the first of
            // them, and an ordinary division when it is a later one.
            if level == 1 && is_title {
                return Some(Shape::Title(title));
            }

            Some(Shape::Division(title))
        }
        BlockKind::Paragraph => block.strong().and_then(environment_name).map(Shape::Bold),
    }
}

const fn style_of(block: &ProseBlock) -> HeadStyle {
    match block.kind() {
        BlockKind::Heading { level } => HeadStyle::Heading { level },
        BlockKind::Paragraph => HeadStyle::Bold,
    }
}

/// The bold run that names a heading's environment, when the heading leaves it one.
///
/// A bold run naming an environment and minting nothing of its own, standing
/// immediately below a heading, is that heading's naming: the heading mints, and
/// the run says what was minted. A run that mints is a head in its own right and
/// names nothing above it.
///
/// Only a heading that mints something other than the rung looks below itself for
/// a name. A heading minting the rung's own kind is a division and is named by its
/// own title, so a defective bold run beneath it stays its own defect rather than
/// being absorbed into the division's name.
fn naming_block(
    blocks: &[ProseBlock],
    heading: usize,
    mints: &[&Occurrence],
    label: &Label,
) -> Option<(usize, String)> {
    if label.kind() == RUNG_KIND {
        return None;
    }

    let index = heading + 1;
    let next = blocks.get(index)?;

    if next.kind() != BlockKind::Paragraph {
        return None;
    }

    if mints
        .iter()
        .any(|mint| next.holds(mint.location().offset()))
    {
        return None;
    }

    let name = environment_name(next.strong()?)?;

    Some((index, name))
}

/// Read a bold run as an environment name, when it is written as one.
///
/// The corpus writes a bold head as the environment's name followed by the name
/// attached to this instance of it, and nothing else opens a paragraph that way.
/// The attached name is kept here and removed by presentation reduction, so a name
/// that is itself parenthesised in the catalogue still reads as itself.
fn environment_name(strong: &str) -> Option<String> {
    let trimmed = strong.trim();

    if !trimmed.ends_with(')') || !trimmed.contains(" (") {
        return None;
    }

    let first = trimmed.chars().next()?;

    first.is_uppercase().then(|| trimmed.to_owned())
}

/// A heading's title, with the separator that introduces its mint removed.
fn division_title(text: &str) -> String {
    text.split('·').next().unwrap_or(text).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{HeadName, HeadStyle, read_heads, validate_heads};
    use crate::finding::Finding;
    use crate::prose::scan_markdown;
    use crate::registry::{KindRegistry, fixture_kind_registry};

    fn heads_of(source: &str) -> (Vec<super::Head>, Vec<Finding>) {
        let path = Path::new("doc.md");
        let (occurrences, blocks, _findings) = scan_markdown(path, source).into_parts();

        read_heads(path, source, &blocks, &occurrences)
    }

    /// The effective relation a fixture validates against.
    ///
    /// The extension rows are a corpus's own declaration rather than a compiled
    /// set, so a fixture wanting them says which it wants. These three are the
    /// ones this module's own heads pair with (´gram:isolation:declaration´).
    fn registry() -> KindRegistry {
        fixture_kind_registry().with_declared([
            ("Document", "doc"),
            ("To-do", "todo"),
            ("Constant", "const"),
        ])
    }

    fn defects(source: &str) -> Vec<Finding> {
        let (heads, mut findings) = heads_of(source);

        findings.extend(validate_heads(&registry(), &heads));

        findings
    }

    /// A paragraph opening with a bold run that names an environment and the
    /// instance of it, followed by a label, is a head paired with that mint.
    /// This is the shape the corpus writes where the document format offers no
    /// environment of its own.
    ///
    /// ´claim:head:a-named-bold-run-heads-an-environment-and-pairs-with-its-mint´
    /// ´test:unit:pairs-a-bold-head-with-its-mint´
    #[test]
    fn pairs_a_bold_head_with_its_mint() {
        let (heads, findings) =
            heads_of("**Invariant (Unique mint)** · `inv:labels:unique-mint`\n\nProse.\n");

        assert_eq!(findings, []);
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].style(), HeadStyle::Bold);
        assert_eq!(
            heads[0].name(),
            &HeadName::Named("Invariant (Unique mint)".to_owned())
        );
        assert_eq!(heads[0].label().to_string(), "inv:labels:unique-mint");
    }

    /// A heading carrying a label is a head too, and the environment it heads
    /// is a division — the one shape the document format supplies itself. The
    /// heading's level is kept, so how deeply it sits is part of what it is.
    ///
    /// ´claim:head:a-labelled-heading-heads-a-division´
    /// ´test:unit:pairs-a-heading-with-its-mint-as-a-division´
    #[test]
    fn pairs_a_heading_with_its_mint_as_a_division() {
        let (heads, findings) =
            heads_of("## Syntax · `sec:labels:syntax`\n\nProse citing (`sec:labels:syntax`).\n");

        assert_eq!(findings, []);
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].style(), HeadStyle::Heading { level: 2 });
        assert_eq!(heads[0].name(), &HeadName::Division("Syntax".to_owned()));
    }

    /// A heading may leave its naming to the bold run beneath it: the heading
    /// carries the mint, the run says which environment that mint names, and
    /// the two are one head. The naming run is not a head of its own and is
    /// therefore never asked for a mint it does not have.
    ///
    /// ´claim:head:a-bold-run-may-name-the-heading-above-it-without-heading-anything´
    /// ´test:unit:lets-a-bold-run-name-the-heading-above-it´
    #[test]
    fn lets_a_bold_run_name_the_heading_above_it() {
        let (heads, findings) = heads_of(
            "## The profile · `conv:profiles:test-profile`\n\n**Convention (Test profile)**\n\nProse.\n",
        );

        assert_eq!(findings, [], "the naming run is not a head of its own");
        assert_eq!(heads.len(), 1);
        assert_eq!(
            heads[0].name(),
            &HeadName::Named("Convention (Test profile)".to_owned())
        );
        assert_eq!(
            defects(
                "## The profile · `conv:profiles:test-profile`\n\n**Convention (Test profile)**\n"
            ),
            []
        );
    }

    /// Bold with no name attached to it heads nothing: a paragraph beginning
    /// with an emphasised word is how running prose stresses its opening, and
    /// reading every such paragraph as an environment would find heads
    /// throughout text that heads nothing.
    ///
    /// ´claim:head:bold-emphasis-without-an-attached-name-is-not-a-head´
    /// ´test:unit:reads-no-head-in-a-paragraph-that-merely-opens-in-bold´
    #[test]
    fn reads_no_head_in_a_paragraph_that_merely_opens_in_bold() {
        let (heads, findings) = heads_of(
            "**Entry (Outline tracking)** · `entry:demo:one`\n\n**OPEN.** The entry is not started.\n",
        );

        assert_eq!(findings, []);
        assert_eq!(
            heads.len(),
            1,
            "a bold run with no attached name heads nothing"
        );
    }

    /// A title that mints nothing is passed over rather than reported: this
    /// stage makes a title mint possible and valid, and the stage after it is
    /// what makes one required, so a titled source carrying no document label is
    /// still in good standing.
    ///
    /// ´claim:head:a-title-that-mints-nothing-is-asked-for-no-mint´
    /// ´test:unit:leaves-the-document-title-out-of-the-heads´
    #[test]
    fn leaves_the_document_title_out_of_the_heads() {
        let (heads, findings) =
            heads_of("# A Calculus of Labels\n\n## Syntax · `sec:labels:syntax`\n");

        assert_eq!(
            findings,
            [],
            "the title mints nothing and is asked for nothing"
        );
        assert_eq!(heads.len(), 1);
        assert_eq!(
            defects("# A Calculus of Labels\n\n## Syntax · `sec:labels:syntax`\n"),
            []
        );
    }

    /// The first structural level-one heading of a source is its Title head, and
    /// the mint it carries names the document: the head is paired with that mint
    /// like any other, and the environment it heads is the whole source rather
    /// than one of its divisions.
    ///
    /// ´claim:head:the-first-level-one-heading-is-the-title-head-and-may-mint-the-document´
    /// ´test:unit:pairs-a-lone-title-with-its-document-mint´
    #[test]
    fn pairs_a_lone_title_with_its_document_mint() {
        let (heads, findings) =
            heads_of("# A Calculus of Labels · `doc:labels:calculus`\n\nProse.\n");

        assert_eq!(findings, []);
        assert_eq!(heads.len(), 1);
        assert_eq!(heads[0].style(), HeadStyle::Heading { level: 1 });
        assert_eq!(
            heads[0].name(),
            &HeadName::Title("A Calculus of Labels".to_owned())
        );
        assert_eq!(heads[0].label().to_string(), "doc:labels:calculus");
    }

    /// Front matter standing before the title changes nothing: the rule is the
    /// first structural level-one heading and not the first line, so a source
    /// opening with a preamble still titles itself at the heading beneath it.
    ///
    /// ´claim:head:front-matter-before-a-title-leaves-it-the-title-head´
    /// ´test:unit:titles-a-front-mattered-source-at-its-first-heading´
    #[test]
    fn titles_a_front_mattered_source_at_its_first_heading() {
        let (heads, findings) = heads_of(
            "<!-- Assembled from the parts. Edit the parts, not this file. -->\n\n# A Calculus of Labels · `doc:labels:calculus`\n\nProse.\n",
        );

        assert_eq!(findings, [], "got {findings:?}");
        assert_eq!(heads.len(), 1);
        assert_eq!(
            heads[0].name(),
            &HeadName::Title("A Calculus of Labels".to_owned()),
            "the heading beneath the preamble is the title"
        );
        assert_eq!(heads[0].label().to_string(), "doc:labels:calculus");
    }

    /// Only the first structural level-one heading is a title. A later one is an
    /// ordinary division head and mints its own division label, so a long
    /// specification keeps its parts and appendices without minting several
    /// document concepts.
    ///
    /// ´claim:head:a-later-level-one-heading-is-an-ordinary-division-head´
    /// ´test:unit:reads-a-later-level-one-heading-as-a-division´
    #[test]
    fn reads_a_later_level_one_heading_as_a_division() {
        let (heads, findings) = heads_of(
            "# The Specification · `doc:spec:specification`\n\nProse.\n\n# Appendix · `sec:spec:appendix`\n\nProse.\n",
        );

        assert_eq!(findings, []);
        assert_eq!(heads.len(), 2);
        assert_eq!(
            heads[0].name(),
            &HeadName::Title("The Specification".to_owned())
        );
        assert_eq!(heads[1].name(), &HeadName::Division("Appendix".to_owned()));
        assert_eq!(
            defects(
                "# The Specification · `doc:spec:specification`\n\n# Appendix · `sec:spec:appendix`\n"
            ),
            [],
            "a division at the same rung is validated as a division"
        );
    }

    /// Where a mint of the Document environment's kind stands is no judgment of
    /// its own. A bold run writing that environment's name carries the pair the
    /// relation catalogues and validates wherever it opens a paragraph, while a
    /// heading minting the same kind is asked about under the word the heading
    /// writes and fails there, exactly as any head fails whose name the
    /// catalogue does not give its label's kind.
    ///
    /// ´claim:head:a-document-mint-away-from-the-title-head-is-judged-by-its-own-name´
    /// ´test:unit:judges-a-document-mint-away-from-the-title-by-the-name-its-head-writes´
    #[test]
    fn judges_a_document_mint_away_from_the_title_by_the_name_its_head_writes() {
        assert_eq!(
            defects(
                "# The Specification · `doc:spec:specification`\n\n**Document (Part)** · `doc:spec:part`\n"
            ),
            [],
            "a bold run writing the environment's own name carries a catalogued pair"
        );

        let later = defects(
            "# The Specification · `doc:spec:specification`\n\n# Appendix · `doc:spec:appendix`\n",
        );

        assert!(
            matches!(later.as_slice(), [Finding::MisclassifiedHead { base, catalogued, .. }] if base == "Appendix" && catalogued == &["app".to_owned()]),
            "a later level-one heading answers for the word it writes: {later:?}"
        );

        let nested = defects(
            "# The Specification · `doc:spec:specification`\n\n## Part · `doc:spec:part`\n",
        );

        assert!(
            matches!(nested.as_slice(), [Finding::MisclassifiedHead { base, catalogued, .. }] if base == "Part" && catalogued == &["part".to_owned()]),
            "and so does a nested division: {nested:?}"
        );
    }

    /// A source has at most one Title head: the first structural level-one
    /// heading takes the class, and a second one heading a division of the same
    /// source is a division head like any other. Both mints pair with their
    /// heads, and the second is answered for under the word its own heading
    /// writes rather than under the environment the format gave the first.
    ///
    /// ´claim:head:a-source-carries-at-most-one-title-head´
    /// ´test:unit:takes-the-title-class-at-the-first-of-two-document-mints´
    #[test]
    fn takes_the_title_class_at_the_first_of_two_document_mints() {
        let source =
            "# First · `doc:spec:first`\n\nProse.\n\n# Second · `doc:spec:second`\n\nProse.\n";
        let (heads, findings) = heads_of(source);

        assert_eq!(findings, [], "both heads pair with their mints");
        assert_eq!(
            heads
                .iter()
                .filter(|head| matches!(head.name(), HeadName::Title(_)))
                .count(),
            1,
            "but only one of them is the title"
        );
        assert!(
            matches!(defects(source).as_slice(), [Finding::UncataloguedHead { base, .. }] if base == "Second"),
            "so the second head answers for its own word: {:?}",
            defects(source)
        );
    }

    /// A Title head is decided by the pair its format-supplied environment name
    /// makes with the kind its mint carries, so the Document environment carries
    /// as many senses as the relation rows against that name: a title minting
    /// any one of them validates, and a title minting a kind the relation rows
    /// against the name nowhere fails at the catalogue as any head would.
    ///
    /// ´claim:head:a-title-validates-on-every-kind-the-relation-rows-its-environment´
    /// ´test:unit:validates-a-title-head-under-every-kind-the-relation-rows-it´
    #[test]
    fn validates_a_title_head_under_every_kind_the_relation_rows_it() {
        const GENRES: [&str; 9] = [
            "rec", "rep", "reg", "log", "proposal", "spec", "thesis", "plan", "guide",
        ];
        const ROWED: &str = "| Environment | Kind |\n| --- | --- |\n\
             | Document | rec |\n| Document | rep |\n| Document | reg |\n\
             | Document | log |\n| Document | proposal |\n| Document | spec |\n\
             | Document | thesis |\n| Document | plan |\n| Document | guide |\n";

        let registry = KindRegistry::parse(ROWED);

        for genre in GENRES {
            let source = format!("# A Titled Document · `{genre}:demo:document`\n");
            let (heads, findings) = heads_of(&source);

            assert_eq!(findings, [], "the title pairs with its {genre} mint");
            assert_eq!(
                validate_heads(&registry, &heads),
                [],
                "and the {genre} pair validates"
            );
        }

        let (heads, _) = heads_of("# A Titled Document · `memo:demo:document`\n");
        let unrowed = validate_heads(&registry, &heads);

        assert!(
            matches!(unrowed.as_slice(), [Finding::MisclassifiedHead { base, .. }] if base == "Document"),
            "a kind rowed against the name nowhere fails at the catalogue: {unrowed:?}"
        );
    }

    /// The Title head validates through the acceptee's extension set rather than
    /// through the words the source is titled with: the format supplies the
    /// environment name, the local pair supplies its kind, and a title minting
    /// any other kind is reported against that name.
    ///
    /// ´claim:head:a-title-head-validates-under-the-format-supplied-environment-name´
    /// ´test:unit:validates-a-title-head-through-the-acceptee´
    #[test]
    fn validates_a_title_head_through_the_acceptee() {
        assert_eq!(
            defects("# Anything At All · `doc:labels:calculus`\n"),
            [],
            "the title's own words are never looked up"
        );

        let findings = defects("# A Calculus of Labels · `sec:labels:calculus`\n");
        let [
            Finding::MisclassifiedHead {
                head,
                base,
                catalogued,
                ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one misclassified head, got {findings:?}");
        };

        assert_eq!(head, "Document");
        assert_eq!(base, "Document");
        assert_eq!(catalogued, &["doc".to_owned()]);
    }

    /// A source with no structural level-one heading has no Title head, and
    /// nothing about it changes: its divisions head as they did, and no title,
    /// concept or label is invented for it.
    ///
    /// ´claim:head:a-titleless-source-carries-no-title-head´
    /// ´test:unit:leaves-a-titleless-source-untouched´
    #[test]
    fn leaves_a_titleless_source_untouched() {
        let (heads, findings) = heads_of(
            "## Syntax · `sec:labels:syntax`\n\nProse.\n\n### Nested · `sec:labels:nested`\n",
        );

        assert_eq!(findings, []);
        assert_eq!(heads.len(), 2);
        assert!(
            heads
                .iter()
                .all(|head| matches!(head.name(), HeadName::Division(_))),
            "no head is a title: {heads:?}"
        );
    }

    /// A level-one heading drawn inside a fenced block is text rather than a
    /// heading, so it neither takes the title class from a real title beneath it
    /// nor supplies one to a source that has none.
    ///
    /// ´claim:head:a-fenced-level-one-heading-is-not-a-title-head´
    /// ´test:unit:reads-no-title-in-a-fenced-level-one-heading´
    #[test]
    fn reads_no_title_in_a_fenced_level_one_heading() {
        let (heads, findings) = heads_of(
            "```text\n# Displayed Title · `doc:labels:displayed`\n```\n\n# The Real Title · `doc:labels:real`\n",
        );

        assert_eq!(findings, [], "got {findings:?}");
        assert_eq!(heads.len(), 1);
        assert_eq!(
            heads[0].name(),
            &HeadName::Title("The Real Title".to_owned())
        );
    }

    /// A head that mints nothing has no identity and is reported by name,
    /// whether it is a bold run or a heading. Only a head already named by the
    /// division above it escapes the requirement, because that one is not a
    /// head of its own.
    ///
    /// ´claim:head:a-head-that-mints-nothing-is-reported´
    /// ´test:unit:reports-a-head-that-mints-nothing´
    #[test]
    fn reports_a_head_that_mints_nothing() {
        let findings = defects(
            "## Syntax · `sec:labels:syntax`\n\n**Invariant (Unique mint)**\n\nProse.\n\n## Orphan\n",
        );

        let heads: Vec<&str> = findings
            .iter()
            .filter_map(|finding| match finding {
                Finding::MintlessHead { head, .. } => Some(head.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(
            heads,
            ["Invariant (Unique mint)", "Orphan"],
            "got {findings:?}"
        );
        assert_eq!(findings.len(), 2, "a division names itself: {findings:?}");
    }

    /// A mint standing in running prose, away from every head, names nothing
    /// and is reported carrying the label it wrote. Minting and heading are two
    /// halves of one act, and neither half stands alone.
    ///
    /// ´claim:head:a-mint-that-heads-nothing-is-reported´
    /// ´test:unit:reports-a-mint-that-heads-nothing´
    #[test]
    fn reports_a_mint_that_heads_nothing() {
        let findings = defects(
            "## Syntax · `sec:labels:syntax`\n\nA paragraph minting `inv:labels:unique-mint` in running text.\n",
        );

        assert!(
            matches!(findings.as_slice(), [Finding::HeadlessMint { label, .. }] if label.to_string() == "inv:labels:unique-mint"),
            "expected one headless mint, got {findings:?}"
        );
    }

    /// A head whose name the registry catalogues under a different kind than
    /// the one its label carries is a failure, and the report names both the
    /// head's base word and the kind the registry actually gives it, so the
    /// author is told what to change and to what.
    ///
    /// ´claim:head:a-head-whose-kind-contradicts-its-name-is-a-failure´
    /// ´test:unit:fails-a-head-whose-kind-the-registry-denies-its-name´
    #[test]
    fn fails_a_head_whose_kind_the_registry_denies_its_name() {
        let findings = defects("**Theorem (Warrant lapse)** · `def:labels:warrant-lapse`\n");

        let [
            Finding::MisclassifiedHead {
                base, catalogued, ..
            },
        ] = findings.as_slice()
        else {
            panic!("expected one misclassified head, got {findings:?}");
        };

        assert_eq!(base, "Theorem");
        assert_eq!(catalogued, &["thm".to_owned()]);
    }

    /// A head whose name the registry does not catalogue at all is a failure
    /// naming that word, so the vocabulary of environments is the registered
    /// one and cannot be widened by inventing a head.
    ///
    /// ´claim:head:an-uncatalogued-head-name-is-a-failure´
    /// ´test:unit:fails-a-head-the-registry-does-not-catalogue´
    #[test]
    fn fails_a_head_the_registry_does_not_catalogue() {
        let findings = defects("**Blancmange (Of the day)** · `def:labels:blancmange`\n");

        assert!(
            matches!(findings.as_slice(), [Finding::UncataloguedHead { base, .. }] if base == "Blancmange"),
            "expected one uncatalogued head, got {findings:?}"
        );
    }

    /// A head is validated after its presentation is reduced away: a qualified
    /// name reduces to the catalogued word it modifies, and a heading at any
    /// depth reduces to the division rung. Prose may therefore read naturally
    /// without stepping outside the registered vocabulary.
    ///
    /// ´claim:head:a-head-validates-through-its-presentation-reduction´
    /// ´test:unit:validates-a-head-through-presentation-reduction´
    #[test]
    fn validates_a_head_through_presentation_reduction() {
        assert_eq!(
            defects("**Main Theorem (Warrant lapse)** · `thm:labels:warrant-lapse`\n"),
            []
        );
        assert_eq!(
            defects("**Working hypothesis (Convergence)** · `assum:labels:convergence`\n"),
            []
        );
        assert_eq!(defects("### Subsection · `sec:labels:nested`\n"), []);
    }
}
