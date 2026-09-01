// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! The shape report: how big the corpus's documents and environments are.
//!
//! The campaign's standard for a rewritten record is "short and super sharp",
//! and that standard has no numbers attached to it. This module attaches them.
//! It measures every document the carrier reads — how many environments it
//! heads, how long each of them is, how densely each of them cites — and it
//! measures the same quantities over a set of benchmark documents the corpus
//! already considers well shaped, so that "long" means "long beside the records
//! we are happy with" rather than "long beside a number somebody typed".
//!
//! # It is a report, and must be
//!
//! Nothing here is a rule. A long environment may be exactly right — a grammar,
//! a case table, a worked example — and a short one may be a stub or may be the
//! sharpest paragraph in the corpus. A gate over size would be wrong about both,
//! and would push authors into splitting environments to satisfy a checker. So
//! this module raises no finding, the command exits zero however long its
//! listings are, and the reader decides. It is the graph report's sibling, and it
//! is advisory for the same reason.
//!
//! # The extent of an environment
//!
//! Words per environment needs an environment to have an extent, and the corpus's
//! head shapes give it one. An environment runs from the start of the block that
//! heads it to the start of the next such block, where "next such block" is
//! whichever comes first of the next environment head and the next heading of any
//! rung, or the end of the document when neither comes. The second clause is what
//! makes a section boundary close an environment: a heading that mints nothing —
//! a document title, or a division of a document the migration has not finished —
//! is no head, but it plainly ends whatever ran before it, and an environment
//! that swallowed the sections below it would report one document as one
//! environment.
//!
//! Everything before the first head belongs to no environment. That is the
//! document's front matter, and counting it into the first environment would make
//! every document's opening environment look bloated by exactly its title.
//!
//! # Divisions are measured separately
//!
//! The extent rule above is flat: an environment is the text standing directly
//! under its own head, and never the environments below it. That is the right
//! measure for the environments the standard is about — a definition, an
//! invariant, a judgment — and it makes a division's own measurement almost
//! meaningless, because a section heading whose next block is a subsection heads
//! nothing but its own line. Measuring both together would put a crowd of
//! four-word divisions at the bottom of every distribution and drag its lower
//! reaches down to where nothing else can be told apart.
//!
//! So the two are separated. A head that names an environment — a bold run, or a
//! heading a bold run names — is distributed with its like. A head that names a
//! division is measured and reported per document and in its own distribution,
//! apart from them, because its size says nothing about the prose it organises.
//!
//! # What a word is
//!
//! A word is a maximal run of non-whitespace characters in the extent's source
//! text, counted as written. Markup counts: a table row's pipes, a fenced block's
//! contents, and a label span are all words. The rule is crude on purpose — it is
//! reproducible from the bytes by anyone and needs no rendering, so every
//! measurement here is distorted in exactly the same way as every other.
//!
//! # The report measures and compares nothing
//!
//! What comes back is three distributions — words and citations per named
//! environment, and words per division — each reported in full: count, extremes,
//! mean, and the tenth, fiftieth, seventy-fifth and ninetieth percentiles. There
//! is no yardstick beside them and no environment is called an outlier. A
//! benchmark set nominated in this file was the alternative, and it is the
//! arrangement in which the binary decides which of a corpus's own documents are
//! the well shaped ones — a judgment no linter has the standing to make, and one
//! that made the whole report unreadable over any corpus but the one the list was
//! written for. A reviewer reading the percentiles can see what is long here; the
//! command's business is to measure and say so.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`measures_words_and_citations_per_environment`] | shape | Each environment is measured over its own extent: the words from its head down to the next one, and the citations standing within it. A citation belongs to the environment it was written in, so density is a property of the environment rather than of the document around it. |
//! | [`closes_an_environment_at_a_section_boundary`] | shape | A heading closes whatever ran before it even when it mints nothing and so heads no environment of its own. Without that, an environment would swallow every section beneath it and a whole document would measure as one enormous environment. |
//! | [`leaves_front_matter_out_of_every_environment`] | shape | Whatever stands before the first head belongs to no environment: it is the document's front matter, still counted into the document's own size but charged to none of its environments, so an opening environment is not made to look bloated by the title above it. |
//! | [`measures_nothing_in_a_document_that_mints_nothing`] | shape | A document that mints nothing is measured not at all rather than measured as one large environment, so prose the migration has not reached contributes nothing to the distributions it would distort. |
//! | [`takes_percentiles_by_nearest_rank`] | shape | A distribution reports its count, its extremes, its mean and its percentiles taken by nearest rank, so every quoted percentile is a measurement that actually occurred rather than an interpolation between two that did. An empty distribution is empty rather than an error. |
//! | [`separates_divisions_from_named_environments`] | shape | Divisions and named environments are counted apart from one another, because the extent rule measures an environment flat: a division holding only a sub-environment measures barely its own line, and banding it beside real environments would drown them in near-empty measurements. |
//! | [`summarises_the_corpus_it_measured`] | shape | One summary carries the whole picture: how many documents were measured, the environment counts, the three distributions, and a per-document breakdown whose paths are relative so the report does not name the machine it was run on. It compares the corpus with nothing, which is why it can be read over any corpus at all. |

use std::path::Path;

use serde::Serialize;

use crate::adoption::Adoption;
use crate::carrier::Source;
use crate::finding::Location;
use crate::head::{Head, HeadName, read_heads};
use crate::label::Label;
use crate::occurrence::Occurrence;
use crate::prose::{BlockKind, ProseBlock, scan_markdown};

/// A measured distribution, reported rather than judged.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Distribution {
    /// How many measurements the distribution is over.
    pub count: usize,
    /// The smallest measurement.
    pub min: usize,
    /// The tenth percentile, and the band's lower edge.
    pub p10: usize,
    /// The median.
    pub p50: usize,
    /// The seventy-fifth percentile.
    pub p75: usize,
    /// The ninetieth percentile, and the band's upper edge.
    pub p90: usize,
    /// The largest measurement.
    pub max: usize,
    /// The arithmetic mean.
    pub mean: f64,
}

impl Distribution {
    /// Take a distribution over a set of measurements.
    ///
    /// Percentiles are nearest-rank over the sorted measurements, which needs no
    /// interpolation and lands on a measurement that was actually taken — a
    /// reader can go and look at the environment that is the median.
    #[must_use]
    pub fn over(measurements: &[usize]) -> Self {
        let mut sorted = measurements.to_vec();
        sorted.sort_unstable();

        let Some((&min, &max)) = sorted.first().zip(sorted.last()) else {
            return Self::default();
        };

        let total: usize = sorted.iter().sum();
        #[allow(
            clippy::cast_precision_loss,
            reason = "a mean over corpus-sized counts"
        )]
        let mean = total as f64 / sorted.len() as f64;

        Self {
            count: sorted.len(),
            min,
            p10: percentile(&sorted, 10),
            p50: percentile(&sorted, 50),
            p75: percentile(&sorted, 75),
            p90: percentile(&sorted, 90),
            max,
            mean,
        }
    }
}

/// The nearest-rank percentile of a sorted, non-empty set of measurements.
fn percentile(sorted: &[usize], which: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }

    let rank = (which * sorted.len()).div_ceil(100).max(1);

    sorted[rank.min(sorted.len()) - 1]
}

/// One environment, measured.
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentShape {
    /// The label the environment's head mints.
    pub label: Label,
    /// The head as written.
    pub head: String,
    /// The environment's kind.
    pub kind: String,
    /// Whether the head names a division rather than an environment.
    pub division: bool,
    /// How many words stand in the environment's extent.
    pub words: usize,
    /// How many citations stand in the environment's extent.
    pub citations: usize,
    /// Where the environment's head stands.
    pub location: Location,
}

/// One document, measured.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentShape {
    /// The document, by its path from the repository root.
    pub document: String,
    /// How many environments the document heads.
    pub heads: usize,
    /// How many of them name an environment rather than a division.
    pub named: usize,
    /// How many words the whole document carries.
    pub words: usize,
    /// How many citations the whole document makes.
    pub citations: usize,
    /// Every environment the document heads, in document order.
    pub environments: Vec<EnvironmentShape>,
}

/// What the shape report says about the corpus.
#[derive(Debug, Clone, Serialize)]
pub struct ShapeSummary {
    /// How many documents were measured.
    pub documents_measured: usize,
    /// How many environments, of both kinds, were measured.
    pub environments: usize,
    /// How many of them name an environment rather than a division.
    pub named: usize,
    /// Words per named environment, over every measured document.
    pub words: Distribution,
    /// Citations per named environment, over every measured document.
    pub citations: Distribution,
    /// Words per division, over every measured document.
    pub division_words: Distribution,
    /// Every measured document, in path order.
    pub by_document: Vec<DocumentShape>,
}

/// Measure every participating source the carrier read.
///
/// A source heading no environment is left out rather than reported as a
/// document of size zero: a document that mints nothing is not writing under the
/// calculus yet, and the discipline of the engine already says so.
#[must_use]
pub fn measure(adoption: &Adoption, sources: &[Source]) -> Vec<DocumentShape> {
    let mut shapes: Vec<DocumentShape> = sources
        .iter()
        .filter(|source| adoption.participates(source.path()))
        .filter_map(|source| measure_source(adoption, source.path(), source.text()))
        .collect();

    shapes.sort_by(|left, right| left.document.cmp(&right.document));

    shapes
}

/// Measure one source, when it heads any environment at all.
fn measure_source(adoption: &Adoption, path: &Path, text: &str) -> Option<DocumentShape> {
    let (occurrences, blocks, _findings) = scan_markdown(path, text).into_parts();

    if !occurrences.iter().any(Occurrence::is_mint) {
        return None;
    }

    let (heads, _head_findings) = read_heads(path, text, &blocks, &occurrences);

    let mut environments = Vec::new();

    for head in &heads {
        if adoption.is_reserved_kind(head.label().kind()) {
            continue;
        }

        let Some(start) = block_start_of(&blocks, head.location().offset()) else {
            continue;
        };

        let end = extent_end(&blocks, start, &heads);

        environments.push(EnvironmentShape {
            label: head.label().clone(),
            head: head.name().as_str().to_owned(),
            kind: head.label().kind().to_owned(),
            division: matches!(head.name(), HeadName::Division(_)),
            words: words_in(&text[start.min(text.len())..end.min(text.len())]),
            citations: occurrences
                .iter()
                .filter(|occurrence| !occurrence.is_mint())
                .filter(|occurrence| (start..end).contains(&occurrence.location().offset()))
                .count(),
            location: head.location().clone(),
        });
    }

    if environments.is_empty() {
        return None;
    }

    Some(DocumentShape {
        document: path.to_string_lossy().into_owned(),
        heads: environments.len(),
        named: environments
            .iter()
            .filter(|environment| !environment.division)
            .count(),
        words: words_in(text),
        citations: occurrences
            .iter()
            .filter(|occurrence| !occurrence.is_mint())
            .count(),
        environments,
    })
}

/// The start of the block holding an offset.
fn block_start_of(blocks: &[ProseBlock], offset: usize) -> Option<usize> {
    blocks
        .iter()
        .find(|block| block.holds(offset))
        .map(ProseBlock::start)
}

/// Where an environment opening at a block stops.
///
/// Whichever comes first of the next environment head and the next heading of
/// any rung, and the end of the document when neither comes. Blocks are in source
/// order, so the first block starting after this one that is either is the answer.
fn extent_end(blocks: &[ProseBlock], start: usize, heads: &[Head]) -> usize {
    let boundary = blocks
        .iter()
        .filter(|block| block.start() > start)
        .find(|block| {
            matches!(block.kind(), BlockKind::Heading { .. })
                || heads
                    .iter()
                    .any(|head| block.holds(head.location().offset()))
        })
        .map(ProseBlock::start);

    boundary.unwrap_or(usize::MAX)
}

/// How many words a run of source text carries.
fn words_in(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Measure the whole corpus and report what it measured.
#[must_use]
pub fn summarise_shape(adoption: &Adoption, sources: &[Source]) -> ShapeSummary {
    let shapes = measure(adoption, sources);

    let environments: Vec<&EnvironmentShape> = shapes
        .iter()
        .flat_map(|shape| shape.environments.iter())
        .collect();
    let named: Vec<&&EnvironmentShape> = environments
        .iter()
        .filter(|environment| !environment.division)
        .collect();

    let words: Vec<usize> = named.iter().map(|environment| environment.words).collect();
    let citations: Vec<usize> = named
        .iter()
        .map(|environment| environment.citations)
        .collect();
    let divisions: Vec<usize> = environments
        .iter()
        .filter(|environment| environment.division)
        .map(|environment| environment.words)
        .collect();

    let counts = (environments.len(), named.len());

    ShapeSummary {
        documents_measured: shapes.len(),
        environments: counts.0,
        named: counts.1,
        words: Distribution::over(&words),
        citations: Distribution::over(&citations),
        division_words: Distribution::over(&divisions),
        by_document: shapes,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Distribution, measure, summarise_shape};
    use crate::adoption::{Adoption, index_adoption as build_index_adoption};
    use crate::carrier::Source;

    fn index_adoption(
        packages: &[crate::workspace::Package],
        names: Option<&crate::roster::OwnerNames>,
        assemblies: &[crate::assembly::Assembly],
    ) -> Adoption {
        build_index_adoption(
            packages,
            names,
            assemblies,
            crate::registry::fixture_kind_registry(),
        )
    }

    fn shapes_of(sources: &[Source]) -> Vec<super::DocumentShape> {
        measure(
            &index_adoption(
                &[],
                Some(&crate::roster::OwnerNames::new(
                    "torrust-",
                    [crate::roster::UnbuiltMember::new(
                        "torrust-notime",
                        "packages/notime",
                    )],
                )),
                &[],
            ),
            sources,
        )
    }

    /// Each environment is measured over its own extent: the words from its
    /// head down to the next one, and the citations standing within it. A
    /// citation belongs to the environment it was written in, so density is a
    /// property of the environment rather than of the document around it.
    ///
    /// ´claim:shape:each-environment-is-measured-over-its-own-extent´
    /// ´test:unit:measures-words-and-citations-per-environment´
    #[test]
    fn measures_words_and_citations_per_environment() {
        let sources = [Source::new(
            "adr/one.md",
            "# The title\n\n\
             ## First · `sec:fixture:first`\n\n\
             One two three.\n\n\
             ## Second · `sec:fixture:second`\n\n\
             Cites (`sec:fixture:first`) and (`sec:fixture:first`) again.\n",
        )];

        let shapes = shapes_of(&sources);

        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].heads, 2);

        let first = &shapes[0].environments[0];
        assert_eq!(first.label.to_string(), "sec:fixture:first");
        assert_eq!(first.words, 7, "the head line and the three words below it");
        assert_eq!(first.citations, 0);

        let second = &shapes[0].environments[1];
        assert_eq!(
            second.citations, 2,
            "both citations stand in the second environment"
        );
        assert_eq!(shapes[0].citations, 2);
    }

    /// A heading closes whatever ran before it even when it mints nothing and
    /// so heads no environment of its own. Without that, an environment would
    /// swallow every section beneath it and a whole document would measure as
    /// one enormous environment.
    ///
    /// ´claim:shape:a-heading-closes-the-environment-above-it-even-when-it-mints-nothing´
    /// ´test:unit:closes-an-environment-at-a-section-boundary´
    #[test]
    fn closes_an_environment_at_a_section_boundary() {
        let sources = [Source::new(
            "adr/one.md",
            "**Invariant (One)** · `inv:fixture:one`\n\n\
             Body of the invariant.\n\n\
             ## An unminted division\n\n\
             Text below the boundary, which the invariant does not reach.\n",
        )];

        let shapes = shapes_of(&sources);

        assert_eq!(shapes[0].environments.len(), 1);
        assert_eq!(
            shapes[0].environments[0].words, 8,
            "the bold head and its body, stopping at the heading below"
        );
    }

    /// Whatever stands before the first head belongs to no environment: it is
    /// the document's front matter, still counted into the document's own size
    /// but charged to none of its environments, so an opening environment is
    /// not made to look bloated by the title above it.
    ///
    /// ´claim:shape:front-matter-belongs-to-the-document-and-to-no-environment´
    /// ´test:unit:leaves-front-matter-out-of-every-environment´
    #[test]
    fn leaves_front_matter_out_of_every_environment() {
        let sources = [Source::new(
            "adr/one.md",
            "# The title\n\n\
             A long paragraph of front matter that no environment may claim.\n\n\
             ## First · `sec:fixture:first`\n\n\
             Short.\n",
        )];

        let shapes = shapes_of(&sources);

        assert_eq!(
            shapes[0].environments[0].words, 5,
            "the head line and one word"
        );
        assert!(
            shapes[0].words > shapes[0].environments[0].words,
            "the document is larger than its one environment"
        );
    }

    /// A document that mints nothing is measured not at all rather than
    /// measured as one large environment, so prose the migration has not
    /// reached contributes nothing to the distributions it would distort.
    ///
    /// ´claim:shape:a-document-that-mints-nothing-is-not-measured´
    /// ´test:unit:measures-nothing-in-a-document-that-mints-nothing´
    #[test]
    fn measures_nothing_in_a_document_that_mints_nothing() {
        let sources = [Source::new(
            "adr/one.md",
            "# A title\n\n## A division\n\nProse.\n",
        )];

        assert!(shapes_of(&sources).is_empty());
    }

    /// A distribution reports its count, its extremes, its mean and its
    /// percentiles taken by nearest rank, so every quoted percentile is a
    /// measurement that actually occurred rather than an interpolation between
    /// two that did. An empty distribution is empty rather than an error.
    ///
    /// ´claim:shape:percentiles-are-taken-by-nearest-rank´
    /// ´test:unit:takes-percentiles-by-nearest-rank´
    #[test]
    fn takes_percentiles_by_nearest_rank() {
        let distribution = Distribution::over(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100]);

        assert_eq!(distribution.count, 10);
        assert_eq!(distribution.min, 10);
        assert_eq!(distribution.max, 100);
        assert_eq!(distribution.p10, 10);
        assert_eq!(distribution.p50, 50);
        assert_eq!(distribution.p75, 80);
        assert_eq!(distribution.p90, 90);
        assert!((distribution.mean - 55.0).abs() < f64::EPSILON);

        assert_eq!(
            Distribution::over(&[]).count,
            0,
            "an empty distribution is empty"
        );
    }

    /// One document whose division carries one named environment of given length.
    fn document(name: &str, body: &str) -> String {
        format!(
            "## A division · `sec:fixture:{name}`\n\n**Invariant ({name})** · `inv:fixture:{name}`\n\n{body}\n"
        )
    }

    /// Divisions and named environments are counted apart from one another,
    /// because the extent rule measures an environment flat: a division holding
    /// only a sub-environment measures barely its own line, and banding it
    /// beside real environments would drown them in near-empty measurements.
    ///
    /// ´claim:shape:divisions-and-named-environments-are-measured-separately´
    /// ´test:unit:separates-divisions-from-named-environments´
    #[test]
    fn separates_divisions_from_named_environments() {
        let sources = [Source::new("adr/one.md", document("one", "One two three."))];
        let shapes = shapes_of(&sources);

        assert_eq!(shapes[0].heads, 2);
        assert_eq!(shapes[0].named, 1);
        assert!(
            shapes[0].environments[0].division,
            "the heading names a division"
        );
        assert!(
            !shapes[0].environments[1].division,
            "the bold run names an environment"
        );
        assert_eq!(
            shapes[0].environments[0].words, 5,
            "a division holding only a sub-environment measures its own line"
        );
    }

    /// One summary carries the whole picture: how many documents were measured,
    /// the environment counts, the three distributions, and a per-document
    /// breakdown whose paths are relative so the report does not name the
    /// machine it was run on. It compares the corpus with nothing, which is why
    /// it can be read over any corpus at all.
    ///
    /// ´claim:shape:one-summary-carries-the-corpus-it-measured-and-no-comparison´
    /// ´test:unit:summarises-the-corpus-it-measured´
    #[test]
    fn summarises_the_corpus_it_measured() {
        let sources = [Source::new(
            "adr/one.md",
            document("one", "More prose here."),
        )];
        let summary = summarise_shape(
            &index_adoption(
                &[],
                Some(&crate::roster::OwnerNames::new(
                    "torrust-",
                    [crate::roster::UnbuiltMember::new(
                        "torrust-notime",
                        "packages/notime",
                    )],
                )),
                &[],
            ),
            &sources,
        );

        assert_eq!(summary.documents_measured, 1);
        assert_eq!(summary.environments, 2);
        assert_eq!(summary.named, 1);
        assert_eq!(summary.words.count, 1);
        assert_eq!(summary.citations.count, 1);
        assert_eq!(summary.division_words.count, 1);
        assert_eq!(summary.by_document[0].document, "adr/one.md");
        assert!(Path::new(&summary.by_document[0].document).is_relative());
    }
}
