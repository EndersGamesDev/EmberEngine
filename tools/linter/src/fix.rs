// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! The fix mode: writing the derived label at every standard place.
//!
//! The mechanization requirement of ADR-L-015, The test label profile, is
//! explicit that labels are never
//! maintained by hand at scale — the fix mode performs the sweep and the check
//! enforces it thereafter. This module is that sweep.
//!
//! # What it may and may not touch
//!
//! The fix mode writes labels; it never rewrites prose. Every edit is a whole
//! line inserted, or a whole line replaced when that line was itself nothing
//! but an attestation — an acute-delimited span and nothing else, which the
//! derivation-warrant inference rule (ADR-L-014, A calculus of documentation and source labels)
//! says warrants nothing when its text differs from the derivation. Every other
//! line of every file is left byte for byte as it was, line endings included.
//! Where the standard place cannot be reached that way — a block documentation
//! comment, or a final line mixing the label with an author's words — the asset
//! is reported and left alone, because a fix mode that guesses at prose is
//! worse than one that stops.
//!
//! # Idempotence
//!
//! Running the fix mode twice changes nothing the second time: an asset already
//! carrying its derived label at the standard place is left untouched, and every
//! insertion produces exactly the line that check then accepts.
//!
//! # The dirty-tree refusal
//!
//! A sweep across thousands of files is only reviewable against a clean starting
//! point, so the fix mode refuses to run when the working tree has changes,
//! unless the caller insists. The refusal is a precondition, not a rule of the
//! calculus.
//!
//! # The second profile's sweep
//!
//! ADR-L-016's to-do profile puts its standard place inside a line rather than
//! above one, so its sweep rewrites the marker's own line where the test
//! profile's inserts a line above an attribute. Everything else is shared: the
//! same whole-line editor applies the edits bottom-up, the same outcome counts
//! them, the same dry run withholds the write, and the same refusal reports a
//! place no edit reaches instead of guessing at it. The two sweeps differ in
//! where the label goes and in nothing else, which is what lets one command take
//! a profile rather than two commands take one each.
//!
//! Rewriting a line is a stronger power than inserting one, so the to-do sweep
//! bounds itself twice over. It rebuilds the line from the parts its own reader
//! parsed — prefix, marker, qualifier, label, summary — so no byte before the
//! marker moves and no continuation line is read at all, and it declines any
//! line whose recorded marker is no longer there.
//!
//! That rebuilt line is also what decides whether there was anything to do: both
//! sweeps ask whether the place already reads as the writer would write it, and
//! neither settles for the label being the right one. A right label in a form the
//! writer would not emit is a repair in both profiles — a label pressed against
//! the prose above it in one, a label missing the colon after it in the other —
//! because a form nobody repairs is a form that never converges.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`inserts_a_documentation_line_where_there_is_none`] | fix | A test with no documentation at all gains a documentation line carrying its derived label, so a bare test is brought into the inventory without anybody typing the label. |
//! | [`appends_a_line_to_existing_documentation`] | fix | Documentation an author already wrote is kept in full and the label is added below it, so a sweep adds to a test's prose and never replaces it. |
//! | [`replaces_a_wrong_attestation`] | fix | A label that is not the derivation is replaced by the one the site actually derives, counted as a repair rather than an insertion. Renaming a function and running the sweep is enough to put its label right. |
//! | [`separates_a_label_from_list_documentation`] | fix | The label is separated from the prose above it by a blank documentation line, so it begins a paragraph of its own rather than being swallowed into whatever the author's last line was — a list item, for instance. |
//! | [`inserts_a_separator_before_an_unseparated_label`] | fix | cites (´claim:fix:the-label-is-separated-from-the-prose-above-it´) |
//! | [`writes_a_label_directly_below_a_claim`] | fix | A claim standing above the label takes no separator: the record fixes the two labels as one paragraph below the gloss, and a break driven between them puts the claim out of the place its reader looks in. |
//! | [`never_overwrites_a_claim_standing_last`] | fix | A claim standing on the final line is authored text the sweep writes below rather than over. Its bare form is indistinguishable from a stale derived label, and replacing it would delete a statement no derivation could recompute. |
//! | [`leaves_a_correct_label_alone`] | fix | A site already correct is left byte-for-byte alone and its file is not counted as changed, so running a sweep over a conforming tree touches nothing. |
//! | [`is_idempotent`] | fix | A sweep is idempotent: running it a second time produces the same bytes and reports the site as unchanged, so the sweep can be run freely without a first run and a second disagreeing. |
//! | [`writes_nothing_on_a_dry_run`] | fix | A dry run leaves every file exactly as it was while still counting what it would have done, so the size of a sweep can be learned before any of it is committed. |
//! | [`preserves_indentation_and_line_endings`] | fix | A written line matches the indentation of the site it joins and the line endings of the file it enters, so a sweep over a nested or carriage-return-terminated source produces no incidental diff. |
//! | [`fixes_several_tests_in_one_file`] | fix | Every site in one file is swept in a single pass, each getting the treatment its own state calls for, and the positions of the later sites are not disturbed by what the earlier ones gained. |
//! | [`refuses_a_block_documentation_comment`] | fix | A standard place the sweep cannot repair is refused and the file is left untouched, with the refusal reported. The sweep would rather hand a site back to its author than guess at rewriting it. Documentation written as one block comment is such a site. |
//! | [`refuses_a_final_line_mixing_the_label_with_words`] | fix | cites (´claim:fix:a-place-the-sweep-cannot-repair-is-refused-and-left-untouched´) |
//! | [`writes_a_notices_label_into_its_marker_line`] | fix | A notice gains its label inside its own marker line, between the marker and the summary, which is the standard place for this profile: a notice is one line, and its label goes in it rather than above it. |
//! | [`keeps_a_notices_qualifier_where_it_stood`] | fix | A qualifier attached to the marker stays exactly where its author put it, with the label written after it, so an attribution or a date is neither moved nor absorbed into the derivation. |
//! | [`leaves_a_notices_continuation_line_alone`] | fix | Only the marker's own line is rewritten: a notice continuing onto the line below keeps that line untouched, because the continuation is prose the label never reads. |
//! | [`writes_into_a_trailing_comment_without_touching_its_code`] | fix | Every byte before the marker is kept, program text included: a notice trailing a statement gains its label without the statement being reformatted or moved. |
//! | [`rewrites_a_notices_wrong_label`] | fix | cites (´claim:fix:a-wrong-label-is-replaced-by-the-derivation´) |
//! | [`leaves_a_correctly_labelled_notice_alone`] | fix | cites (´claim:fix:a-site-already-correct-is-left-untouched´) |
//! | [`repairs_a_notices_right_label_in_a_non_canonical_form`] | fix | A notice carrying the right label in a form the writer would not have written is repaired rather than kept, on the same terms as the test profile's unseparated label: the standard place is a form and not merely a name, so a missing colon after the label, or spacing loose around it, is a repair the sweep counts. Were it kept, the line would be reported unchanged for ever and the sweep would never converge on it. |
//! | [`sweeps_notices_idempotently`] | fix | cites (´claim:fix:sweeping-twice-changes-nothing-the-second-time´) |
//! | [`writes_no_notice_on_a_dry_run`] | fix | cites (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´) |
//! | [`preserves_a_notices_indentation_and_line_endings`] | fix | cites (´claim:fix:a-written-line-matches-the-files-indentation-and-line-endings´) |
//! | [`sweeps_several_notices_in_one_file`] | fix | cites (´claim:fix:every-site-in-one-file-is-swept-in-one-pass´) |
//! | [`refuses_a_notice_whose_marker_has_moved`] | fix | cites (´claim:fix:a-place-the-sweep-cannot-repair-is-refused-and-left-untouched´) |
//! | [`restores_a_perturbed_notice_to_the_canonical_form`] | fix | A notice perturbed by hand is restored to exactly the bytes the sweep writes from a clean line — not merely to something the reader accepts. The canonical form is taken from the writer itself rather than typed into the test, so the guarantee cannot drift with the writer: a label that is not the derivation, a label of the wrong area, and no label at all all converge on the one form, and the restored line then stands. |

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::claim::line_holds_claim;
use crate::finding::Finding;
use crate::profile::{self, CoveredAsset};
use crate::todo::{CoveredNotice, Placement, place_label};

/// What one asset's standard place needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Repair {
    /// A documentation line must be added.
    Insert,
    /// A line that is nothing but a wrong attestation must be rewritten.
    Replace,
}

/// One whole-line edit of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LineEdit {
    /// The one-based line the edit applies at.
    line: usize,
    /// Whether the line is replaced, or the text inserted before it.
    replace: bool,
    /// The line's new text, without its terminator.
    text: String,
}

/// What the fix mode did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FixOutcome {
    /// How many covered assets the sweep considered.
    pub covered: usize,
    /// How many carried no label at all and gained one.
    ///
    /// For the test profile that is a documentation line added above the
    /// attribute; for the to-do profile it is a label written into the marker's
    /// own line. The count is of assets that had no attestation and now have
    /// one, which is the fact a reviewer wants in either profile.
    pub inserted: usize,
    /// How many had a wrong attestation rewritten.
    pub repaired: usize,
    /// How many already carried their label at the standard place.
    pub unchanged: usize,
    /// How many were left alone because their standard place cannot be written.
    pub refused: usize,
    /// How many files the sweep rewrote.
    pub files_changed: usize,
}

/// Whether the working tree at a root has uncommitted changes.
///
/// # Errors
///
/// Returns a message when the tree's state cannot be established at all, which
/// the caller must treat as a refusal rather than as a clean tree.
pub fn is_tree_dirty(root: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }

    Ok(!output.stdout.is_empty())
}

/// Write the derived label at the standard place of every covered asset.
///
/// With `dry_run` set, every edit is computed and counted but no file is
/// written, so a caller can see the sweep before consenting to it.
#[must_use]
pub fn fix_profile(
    root: &Path,
    assets: &[CoveredAsset],
    dry_run: bool,
) -> (FixOutcome, Vec<Finding>) {
    let mut outcome = FixOutcome::default();
    let mut findings = Vec::new();
    let mut by_file: BTreeMap<PathBuf, Vec<&CoveredAsset>> = BTreeMap::new();

    for asset in assets {
        outcome.covered += 1;
        by_file
            .entry(asset.test().path().to_path_buf())
            .or_default()
            .push(asset);
    }

    for (path, file_assets) in by_file {
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

        let mut edits = Vec::new();

        for asset in file_assets {
            match plan(asset) {
                Ok(None) => outcome.unchanged += 1,
                Ok(Some((Repair::Insert, edit))) => {
                    outcome.inserted += 1;
                    edits.push(edit);
                }
                Ok(Some((Repair::Replace, edit))) => {
                    outcome.repaired += 1;
                    edits.push(edit);
                }
                Err(reason) => {
                    outcome.refused += 1;
                    findings.push(Finding::UnfixableStandardPlace {
                        owner: asset.test().package().to_owned(),
                        asset: asset.test().function().to_owned(),
                        reason,
                        location: asset.test().location().clone(),
                    });
                }
            }
        }

        if edits.is_empty() {
            continue;
        }

        outcome.files_changed += 1;

        if dry_run {
            continue;
        }

        let rewritten = apply(&text, &mut edits);

        if let Err(error) = fs::write(root.join(&path), rewritten) {
            findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
        }
    }

    (outcome, findings)
}

/// Write the derived label at the standard place of every covered notice.
///
/// The to-do half of the sweep. With `dry_run` set, every edit is computed and
/// counted but no file is written, exactly as the test profile's sweep does.
#[must_use]
pub fn fix_todos(
    root: &Path,
    notices: &[CoveredNotice],
    dry_run: bool,
) -> (FixOutcome, Vec<Finding>) {
    let mut outcome = FixOutcome::default();
    let mut findings = Vec::new();
    let mut by_file: BTreeMap<PathBuf, Vec<&CoveredNotice>> = BTreeMap::new();

    for notice in notices {
        outcome.covered += 1;
        by_file
            .entry(notice.notice().path().to_path_buf())
            .or_default()
            .push(notice);
    }

    for (path, file_notices) in by_file {
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

        let mut edits = Vec::new();

        for covered in file_notices {
            let notice = covered.notice();
            let placed = place_label(
                &text,
                notice.location().offset(),
                notice.marker(),
                covered.label(),
            );

            match placed {
                Ok(Placement::Kept) => outcome.unchanged += 1,
                Ok(Placement::Written(line)) => {
                    outcome.inserted += 1;
                    edits.push(LineEdit {
                        line: notice.location().line(),
                        replace: true,
                        text: line,
                    });
                }
                Ok(Placement::Rewritten(line)) => {
                    outcome.repaired += 1;
                    edits.push(LineEdit {
                        line: notice.location().line(),
                        replace: true,
                        text: line,
                    });
                }
                Err(reason) => {
                    outcome.refused += 1;
                    findings.push(Finding::UnfixableStandardPlace {
                        owner: notice.package().to_owned(),
                        asset: notice.summary().to_owned(),
                        reason,
                        location: notice.location().clone(),
                    });
                }
            }
        }

        if edits.is_empty() {
            continue;
        }

        outcome.files_changed += 1;

        if dry_run {
            continue;
        }

        let rewritten = apply(&text, &mut edits);

        if let Err(error) = fs::write(root.join(&path), rewritten) {
            findings.push(Finding::TraversalFailure {
                path: path.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
        }
    }

    (outcome, findings)
}

/// Decide what one asset's standard place needs, if anything.
fn plan(asset: &CoveredAsset) -> Result<Option<(Repair, LineEdit)>, String> {
    let wanted = asset.standard_place_text();
    let test = asset.test();
    let line = format!("{}/// {wanted}", test.indent());

    let Some(doc) = test.doc() else {
        return Ok(Some((
            Repair::Insert,
            LineEdit {
                line: test.attribute_line(),
                replace: false,
                text: line,
            },
        )));
    };

    if !doc.is_line_oriented() {
        return Err(
            "its documentation is a block comment, which no whole-line edit reaches".to_owned(),
        );
    }

    let final_line = doc.final_line().unwrap_or_default();

    // The label must stand as its own paragraph: directly below authored doc
    // text, Markdown can read it as a lazy continuation of a list or
    // paragraph, so a blank documentation line separates them. The embedded
    // line feed relies on the carrier being line-feed-terminated, as the
    // whole-line editor already does for inserted lines.
    //
    // A claim line above is not authored prose and takes no separator. The
    // record fixes the order as gloss, claim, derived label, with the claim
    // standing directly above the label and nothing between them
    // (ADR-L-017, The test documentation policy), which is the placement the claim
    // reader enforces. The paragraph break belongs above the claim, where the
    // gloss ends, and the labels stand together as the one paragraph they are;
    // a break driven between them writes a comment the check refuses. The
    // judgment of what a claim line is comes from the claim profile's own
    // recognizer rather than a copy made here
    // (ADR-L-020, The migration disciplines).
    let separator_needed = |above: &str| !above.trim().is_empty() && !line_holds_claim(above);
    let separator_above_last = |lines: &[String]| {
        lines
            .len()
            .checked_sub(2)
            .and_then(|above| lines.get(above))
            .is_some_and(|text| separator_needed(text))
    };

    if final_line == wanted {
        let lines = doc.lines();

        if separator_above_last(lines) {
            return Ok(Some((
                Repair::Replace,
                LineEdit {
                    line: doc.last_line(),
                    replace: false,
                    text: format!("{}///", test.indent()),
                },
            )));
        }

        return Ok(None);
    }

    // A claim standing last is the staging's ordinary shape: the author has
    // written the statement and no sweep has yet written the derived label
    // below it. The label goes directly under the claim, adjacent, which is
    // where the record puts it and where the reader looks for it. The claim is
    // authored text and no sweep may overwrite it — its bare form is otherwise
    // indistinguishable from a stale derived label, and the replacement below
    // would delete a statement nothing could recompute.
    if line_holds_claim(final_line) {
        return Ok(Some((
            Repair::Insert,
            LineEdit {
                line: doc.last_line() + 1,
                replace: false,
                text: line,
            },
        )));
    }

    if is_sole_attestation(final_line) {
        let lines = doc.lines();
        let replacement = if separator_above_last(lines) {
            format!("{}///\n{line}", test.indent())
        } else {
            line
        };

        return Ok(Some((
            Repair::Replace,
            LineEdit {
                line: doc.last_line(),
                replace: true,
                text: replacement,
            },
        )));
    }

    if final_line.contains(profile::ACUTE) {
        return Err("its final documentation line mixes a label with other words".to_owned());
    }

    let appended = if separator_needed(final_line) {
        format!("{}///\n{line}", test.indent())
    } else {
        line
    };

    Ok(Some((
        Repair::Insert,
        LineEdit {
            line: doc.last_line() + 1,
            replace: false,
            text: appended,
        },
    )))
}

/// Whether a line is one acute-delimited span and nothing else.
fn is_sole_attestation(line: &str) -> bool {
    let acute = profile::ACUTE;
    let mut characters = line.chars();

    if characters.next() != Some(acute) || characters.next_back() != Some(acute) {
        return false;
    }

    let interior = &line[acute.len_utf8()..line.len() - acute.len_utf8()];

    !interior.is_empty() && !interior.contains(acute)
}

/// Apply whole-line edits to a text, from the bottom up.
///
/// Working upwards keeps every not-yet-applied edit's line number valid, and
/// each edit borrows the terminator of the line it lands on, so a file with
/// carriage returns keeps them.
fn apply(text: &str, edits: &mut [LineEdit]) -> String {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();

    edits.sort_by(|left, right| {
        right
            .line
            .cmp(&left.line)
            .then(right.replace.cmp(&left.replace))
    });

    for edit in &*edits {
        let index = edit.line.saturating_sub(1);

        if edit.replace {
            if let Some(existing) = lines.get_mut(index) {
                let terminator = terminator_of(existing);
                *existing = format!("{}{terminator}", edit.text);
            }
            continue;
        }

        let terminator = lines
            .get(index)
            .map_or("\n", |line| terminator_of(line))
            .to_owned();
        let terminator = if terminator.is_empty() {
            "\n".to_owned()
        } else {
            terminator
        };

        lines.insert(index.min(lines.len()), format!("{}{terminator}", edit.text));
    }

    lines.concat()
}

/// The line terminator a line carries, if any.
fn terminator_of(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::fix_todos;
    use crate::finding::Finding;
    use crate::test_support::{notices_of, sweep_notices as sweep_todos, sweep_profile as sweep};

    /// The acute the code syntax delimits a label with.
    const ACUTE: char = '\u{b4}';

    /// A test with no documentation at all gains a documentation line carrying
    /// its derived label, so a bare test is brought into the inventory without
    /// anybody typing the label.
    ///
    /// ´claim:fix:a-test-with-no-documentation-gains-a-line-carrying-its-label´
    /// ´test:unit:inserts-a-documentation-line-where-there-is-none´
    #[test]
    fn inserts_a_documentation_line_where_there_is_none() {
        let (rewritten, outcome, findings) = sweep("#[test]\nfn covered() {}\n", false);

        assert_eq!(
            rewritten,
            "/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n"
        );
        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.files_changed, 1);
        assert!(findings.is_empty(), "nothing is refused: {findings:?}");
    }

    /// Documentation an author already wrote is kept in full and the label is
    /// added below it, so a sweep adds to a test's prose and never replaces it.
    ///
    /// ´claim:fix:existing-documentation-is-kept-and-the-label-added-below-it´
    /// ´test:unit:appends-a-line-to-existing-documentation´
    #[test]
    fn appends_a_line_to_existing_documentation() {
        let (rewritten, outcome, _findings) =
            sweep("/// Checks it.\n#[test]\nfn covered() {}\n", false);

        assert_eq!(
            rewritten,
            "/// Checks it.\n///\n/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n"
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// A label that is not the derivation is replaced by the one the site
    /// actually derives, counted as a repair rather than an insertion. Renaming
    /// a function and running the sweep is enough to put its label right.
    ///
    /// ´claim:fix:a-wrong-label-is-replaced-by-the-derivation´
    /// ´test:unit:replaces-a-wrong-attestation´
    #[test]
    fn replaces_a_wrong_attestation() {
        let (rewritten, outcome, _findings) = sweep(
            "/// Checks it.\n/// \u{b4}test:unit:old-name\u{b4}\n#[test]\nfn covered() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            "/// Checks it.\n///\n/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n"
        );
        assert_eq!(outcome.repaired, 1);
        assert_eq!(outcome.inserted, 0);
    }

    /// The label is separated from the prose above it by a blank documentation
    /// line, so it begins a paragraph of its own rather than being swallowed
    /// into whatever the author's last line was — a list item, for instance.
    ///
    /// ´claim:fix:the-label-is-separated-from-the-prose-above-it´
    /// ´test:unit:separates-a-label-from-list-documentation´
    #[test]
    fn separates_a_label_from_list_documentation() {
        let (rewritten, outcome, _findings) = sweep(
            "/// Checks:\n/// - the case\n#[test]\nfn covered() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            "/// Checks:\n/// - the case\n///\n/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n"
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// A label already correct but pressed against the prose above it gains its
    /// separator, which the sweep counts as a repair: the separation is part of
    /// the standard place and not merely a matter of taste.
    ///
    /// (´claim:fix:the-label-is-separated-from-the-prose-above-it´)
    /// ´test:unit:inserts-a-separator-before-an-unseparated-label´
    #[test]
    fn inserts_a_separator_before_an_unseparated_label() {
        let (rewritten, outcome, _findings) = sweep(
            "/// Checks it.\n/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            "/// Checks it.\n///\n/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n"
        );
        assert_eq!(outcome.repaired, 1);
        assert_eq!(outcome.unchanged, 0);
    }

    /// A claim standing above the label takes no separator: the record fixes
    /// the two labels as one paragraph below the gloss, and a break driven
    /// between them puts the claim out of the place its reader looks in.
    ///
    /// ´claim:fix:a-claim-above-the-label-takes-no-separator´
    /// ´test:unit:writes-a-label-directly-below-a-claim´
    #[test]
    fn writes_a_label_directly_below_a_claim() {
        let (rewritten, outcome, _findings) = sweep(
            "/// Checks it.\n///\n/// \u{b4}claim:demo:it-is-checked\u{b4}\n#[test]\nfn covered() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            concat!(
                "/// Checks it.\n///\n/// \u{b4}claim:demo:it-is-checked\u{b4}\n",
                "/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n",
            )
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// A claim standing on the final line is authored text the sweep writes
    /// below rather than over. Its bare form is indistinguishable from a stale
    /// derived label, and replacing it would delete a statement no derivation
    /// could recompute.
    ///
    /// ´claim:fix:a-claim-standing-last-is-written-below-and-never-over´
    /// ´test:unit:never-overwrites-a-claim-standing-last´
    #[test]
    fn never_overwrites_a_claim_standing_last() {
        let (rewritten, outcome, _findings) = sweep(
            "/// \u{b4}claim:demo:it-is-checked\u{b4}\n#[test]\nfn covered() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            concat!(
                "/// \u{b4}claim:demo:it-is-checked\u{b4}\n",
                "/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n",
            )
        );
        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.repaired, 0);
    }

    /// A site already correct is left byte-for-byte alone and its file is not
    /// counted as changed, so running a sweep over a conforming tree touches
    /// nothing.
    ///
    /// ´claim:fix:a-site-already-correct-is-left-untouched´
    /// ´test:unit:leaves-a-correct-label-alone´
    #[test]
    fn leaves_a_correct_label_alone() {
        let text = "/// \u{b4}test:unit:covered\u{b4}\n#[test]\nfn covered() {}\n";
        let (rewritten, outcome, _findings) = sweep(text, false);

        assert_eq!(rewritten, text);
        assert_eq!(outcome.unchanged, 1);
        assert_eq!(outcome.files_changed, 0);
    }

    /// A sweep is idempotent: running it a second time produces the same bytes
    /// and reports the site as unchanged, so the sweep can be run freely
    /// without a first run and a second disagreeing.
    ///
    /// ´claim:fix:sweeping-twice-changes-nothing-the-second-time´
    /// ´test:unit:is-idempotent´
    #[test]
    fn is_idempotent() {
        let (once, _outcome, _findings) =
            sweep("/// Checks it.\n#[test]\nfn covered() {}\n", false);
        let (twice, outcome, _findings) = sweep(&once, false);

        assert_eq!(once, twice);
        assert_eq!(outcome.unchanged, 1);
        assert_eq!(outcome.inserted, 0);
    }

    /// A dry run leaves every file exactly as it was while still counting what
    /// it would have done, so the size of a sweep can be learned before any of
    /// it is committed.
    ///
    /// ´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´
    /// ´test:unit:writes-nothing-on-a-dry-run´
    #[test]
    fn writes_nothing_on_a_dry_run() {
        let text = "#[test]\nfn covered() {}\n";
        let (rewritten, outcome, _findings) = sweep(text, true);

        assert_eq!(rewritten, text, "a dry run leaves the file alone");
        assert_eq!(outcome.inserted, 1, "but still counts what it would do");
        assert_eq!(outcome.files_changed, 1);
    }

    /// A written line matches the indentation of the site it joins and the line
    /// endings of the file it enters, so a sweep over a nested or
    /// carriage-return-terminated source produces no incidental diff.
    ///
    /// ´claim:fix:a-written-line-matches-the-files-indentation-and-line-endings´
    /// ´test:unit:preserves-indentation-and-line-endings´
    #[test]
    fn preserves_indentation_and_line_endings() {
        let (rewritten, _outcome, _findings) = sweep(
            "mod inner {\r\n    #[test]\r\n    fn covered() {}\r\n}\r\n",
            false,
        );

        assert_eq!(
            rewritten,
            "mod inner {\r\n    /// \u{b4}test:unit:covered\u{b4}\r\n    #[test]\r\n    fn covered() {}\r\n}\r\n"
        );
    }

    /// Every site in one file is swept in a single pass, each getting the
    /// treatment its own state calls for, and the positions of the later sites
    /// are not disturbed by what the earlier ones gained.
    ///
    /// ´claim:fix:every-site-in-one-file-is-swept-in-one-pass´
    /// ´test:unit:fixes-several-tests-in-one-file´
    #[test]
    fn fixes_several_tests_in_one_file() {
        let (rewritten, outcome, _findings) = sweep(
            "#[test]\nfn first() {}\n\n/// Second.\n#[test]\nfn second() {}\n",
            false,
        );

        assert_eq!(
            rewritten,
            concat!(
                "/// \u{b4}test:unit:first\u{b4}\n#[test]\nfn first() {}\n\n",
                "/// Second.\n///\n/// \u{b4}test:unit:second\u{b4}\n#[test]\nfn second() {}\n",
            )
        );
        assert_eq!(outcome.inserted, 2);
        assert_eq!(outcome.files_changed, 1);
    }

    /// A standard place the sweep cannot repair is refused and the file is left
    /// untouched, with the refusal reported. The sweep would rather hand a site
    /// back to its author than guess at rewriting it. Documentation written as
    /// one block comment is such a site.
    ///
    /// ´claim:fix:a-place-the-sweep-cannot-repair-is-refused-and-left-untouched´
    /// ´test:unit:refuses-a-block-documentation-comment´
    #[test]
    fn refuses_a_block_documentation_comment() {
        let text = "/** Checks it.\nOver two lines. */\n#[test]\nfn covered() {}\n";
        let (rewritten, outcome, findings) = sweep(text, false);

        assert_eq!(rewritten, text, "the file is untouched");
        assert_eq!(outcome.refused, 1);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::UnfixableStandardPlace { .. }]
            ),
            "expected one refusal, got {findings:?}"
        );
    }

    /// A final line mixing a label into a sentence is refused too: rewriting it
    /// would mean editing the author's words, so it is reported instead.
    ///
    /// (´claim:fix:a-place-the-sweep-cannot-repair-is-refused-and-left-untouched´)
    /// ´test:unit:refuses-a-final-line-mixing-the-label-with-words´
    #[test]
    fn refuses_a_final_line_mixing_the_label_with_words() {
        let text = "/// see \u{b4}test:unit:covered\u{b4} here\n#[test]\nfn covered() {}\n";
        let (rewritten, outcome, findings) = sweep(text, false);

        assert_eq!(rewritten, text, "the author's words are left alone");
        assert_eq!(outcome.refused, 1);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::UnfixableStandardPlace { .. }]
            ),
            "expected one refusal, got {findings:?}"
        );
    }

    /// A notice gains its label inside its own marker line, between the marker
    /// and the summary, which is the standard place for this profile: a notice
    /// is one line, and its label goes in it rather than above it.
    ///
    /// ´claim:fix:a-notice-gains-its-label-inside-its-marker-line´
    /// ´test:unit:writes-a-notices-label-into-its-marker-line´
    #[test]
    fn writes_a_notices_label_into_its_marker_line() {
        let (rewritten, outcome, findings) = sweep_todos("// TODO: read the flag\n", false);

        assert_eq!(
            rewritten,
            format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n")
        );
        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.files_changed, 1);
        assert!(findings.is_empty(), "nothing is refused: {findings:?}");
    }

    /// A qualifier attached to the marker stays exactly where its author put
    /// it, with the label written after it, so an attribution or a date is
    /// neither moved nor absorbed into the derivation.
    ///
    /// ´claim:fix:a-notices-qualifier-stays-where-its-author-put-it´
    /// ´test:unit:keeps-a-notices-qualifier-where-it-stood´
    #[test]
    fn keeps_a_notices_qualifier_where_it_stood() {
        let (rewritten, outcome, _findings) =
            sweep_todos("// TODO(ADR-L-320 2026-05-21): read the flag\n", false);

        assert_eq!(
            rewritten,
            format!(
                "// TODO(ADR-L-320 2026-05-21) {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n"
            ),
            "the attribution stands where its author put it, and enters no derivation"
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// Only the marker's own line is rewritten: a notice continuing onto the
    /// line below keeps that line untouched, because the continuation is prose
    /// the label never reads.
    ///
    /// ´claim:fix:only-the-markers-own-line-is-rewritten´
    /// ´test:unit:leaves-a-notices-continuation-line-alone´
    #[test]
    fn leaves_a_notices_continuation_line_alone() {
        let (rewritten, outcome, _findings) = sweep_todos(
            "// TODO: read the flag\n// before deciding anything.\n",
            false,
        );

        assert_eq!(
            rewritten,
            format!(
                "// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n// before deciding anything.\n"
            ),
            "the continuation is prose the label never reads"
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// Every byte before the marker is kept, program text included: a notice
    /// trailing a statement gains its label without the statement being
    /// reformatted or moved.
    ///
    /// ´claim:fix:every-byte-before-the-marker-is-kept-code-included´
    /// ´test:unit:writes-into-a-trailing-comment-without-touching-its-code´
    #[test]
    fn writes_into_a_trailing_comment_without_touching_its_code() {
        let (rewritten, outcome, _findings) =
            sweep_todos("let count = 0; // TODO: read the flag\n", false);

        assert_eq!(
            rewritten,
            format!(
                "let count = 0; // TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n"
            ),
            "every byte before the marker is kept, code included"
        );
        assert_eq!(outcome.inserted, 1);
    }

    /// A notice carrying a label that is not its derivation has it replaced,
    /// counted as a repair, so editing a summary and sweeping puts its label
    /// right.
    ///
    /// (´claim:fix:a-wrong-label-is-replaced-by-the-derivation´)
    /// ´test:unit:rewrites-a-notices-wrong-label´
    #[test]
    fn rewrites_a_notices_wrong_label() {
        let text = format!("// TODO {ACUTE}todo:code:something-else{ACUTE}: read the flag\n");
        let (rewritten, outcome, _findings) = sweep_todos(&text, false);

        assert_eq!(
            rewritten,
            format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n")
        );
        assert_eq!(outcome.repaired, 1);
        assert_eq!(outcome.inserted, 0);
    }

    /// A notice already carrying its derived label is left exactly as it is,
    /// and its file is not counted as changed.
    ///
    /// (´claim:fix:a-site-already-correct-is-left-untouched´)
    /// ´test:unit:leaves-a-correctly-labelled-notice-alone´
    #[test]
    fn leaves_a_correctly_labelled_notice_alone() {
        let text = format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n");
        let (rewritten, outcome, _findings) = sweep_todos(&text, false);

        assert_eq!(rewritten, text);
        assert_eq!(outcome.unchanged, 1);
        assert_eq!(outcome.files_changed, 0);
    }

    /// A notice carrying the right label in a form the writer would not have
    /// written is repaired rather than kept, on the same terms as the test
    /// profile's unseparated label: the standard place is a form and not merely
    /// a name, so a missing colon after the label, or spacing loose around it,
    /// is a repair the sweep counts. Were it kept, the line would be reported
    /// unchanged for ever and the sweep would never converge on it.
    ///
    /// ´claim:fix:a-right-label-in-the-wrong-form-is-a-repair´
    /// ´test:unit:repairs-a-notices-right-label-in-a-non-canonical-form´
    #[test]
    fn repairs_a_notices_right_label_in_a_non_canonical_form() {
        let canonical = format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n");

        for source in [
            format!("// TODO {ACUTE}todo:code:read-the-flag{ACUTE} read the flag\n"),
            format!("// TODO   {ACUTE}todo:code:read-the-flag{ACUTE}   :   read the flag\n"),
        ] {
            let (rewritten, outcome, findings) = sweep_todos(&source, false);

            assert!(
                findings.is_empty(),
                "nothing is refused in {source:?}: {findings:?}"
            );
            assert_eq!(
                rewritten, canonical,
                "the sweep writes its own form over {source:?}"
            );
            assert_eq!(
                outcome.repaired, 1,
                "and counts it a repair, as the test profile does"
            );
            assert_eq!(outcome.unchanged, 0);
            assert_eq!(
                outcome.inserted, 0,
                "the label was already there to be put right"
            );
        }
    }

    /// The notice sweep is idempotent on the same terms: a second run produces
    /// the same bytes and changes no file.
    ///
    /// (´claim:fix:sweeping-twice-changes-nothing-the-second-time´)
    /// ´test:unit:sweeps-notices-idempotently´
    #[test]
    fn sweeps_notices_idempotently() {
        let (once, _outcome, _findings) = sweep_todos("// TODO(ADR-L-320): read the flag\n", false);
        let (twice, outcome, _findings) = sweep_todos(&once, false);

        assert_eq!(once, twice);
        assert_eq!(outcome.unchanged, 1);
        assert_eq!(outcome.inserted, 0);
        assert_eq!(outcome.files_changed, 0);
    }

    /// A dry run of the notice sweep leaves the file alone while still counting
    /// what it would have written.
    ///
    /// (´claim:fix:a-dry-run-counts-what-it-would-do-and-writes-nothing´)
    /// ´test:unit:writes-no-notice-on-a-dry-run´
    #[test]
    fn writes_no_notice_on_a_dry_run() {
        let text = "// TODO: read the flag\n";
        let (rewritten, outcome, _findings) = sweep_todos(text, true);

        assert_eq!(rewritten, text, "a dry run leaves the file alone");
        assert_eq!(outcome.inserted, 1, "but still counts what it would do");
        assert_eq!(outcome.files_changed, 1);
    }

    /// A notice rewritten inside an indented block keeps that indentation and
    /// the file's line endings.
    ///
    /// (´claim:fix:a-written-line-matches-the-files-indentation-and-line-endings´)
    /// ´test:unit:preserves-a-notices-indentation-and-line-endings´
    #[test]
    fn preserves_a_notices_indentation_and_line_endings() {
        let (rewritten, _outcome, _findings) = sweep_todos(
            "fn wrapped() {\r\n    // TODO: read the flag\r\n}\r\n",
            false,
        );

        assert_eq!(
            rewritten,
            format!(
                "fn wrapped() {{\r\n    // TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\r\n}}\r\n"
            )
        );
    }

    /// Several notices in one file are all swept in one pass, and each keeps
    /// the marker word its author chose: the spelling of the marker stands and
    /// enters no label.
    ///
    /// (´claim:fix:every-site-in-one-file-is-swept-in-one-pass´)
    /// ´test:unit:sweeps-several-notices-in-one-file´
    #[test]
    fn sweeps_several_notices_in_one_file() {
        let (rewritten, outcome, _findings) = sweep_todos(
            "// TODO: read the flag\n\n// FIXME: write the other flag\n",
            false,
        );

        assert_eq!(
            rewritten,
            format!(
                "// TODO {ACUTE}todo:code:read-the-flag{ACUTE}: read the flag\n\n\
                 // FIXME {ACUTE}todo:code:write-the-other-flag{ACUTE}: write the other flag\n"
            ),
            "the marker's own spelling stands, and enters no label"
        );
        assert_eq!(outcome.inserted, 2);
        assert_eq!(outcome.files_changed, 1);
    }

    /// A notice whose marker is no longer where the census found it is refused
    /// and the file left alone, so a sweep run against a stale census cannot
    /// write a label into a line that has become something else.
    ///
    /// (´claim:fix:a-place-the-sweep-cannot-repair-is-refused-and-left-untouched´)
    /// ´test:unit:refuses-a-notice-whose-marker-has-moved´
    #[test]
    fn refuses_a_notice_whose_marker_has_moved() {
        let root = tempfile::tempdir().expect("temporary directory");
        let relative = Path::new("src/demo.rs");
        fs::create_dir_all(root.path().join("src")).expect("create");

        let censused = "// TODO: read the flag\n";
        let notices = notices_of(censused);

        fs::write(root.path().join(relative), "// the notice has gone\n").expect("write");

        let (outcome, findings) = fix_todos(root.path(), &notices, false);
        let rewritten = fs::read_to_string(root.path().join(relative)).expect("read");

        assert_eq!(
            rewritten, "// the notice has gone\n",
            "a moved marker is left alone"
        );
        assert_eq!(outcome.refused, 1);
        assert!(
            matches!(
                findings.as_slice(),
                [Finding::UnfixableStandardPlace { .. }]
            ),
            "expected one refusal, got {findings:?}"
        );
    }

    /// A notice perturbed by hand is restored to exactly the bytes the sweep
    /// writes from a clean line — not merely to something the reader accepts.
    /// The canonical form is taken from the writer itself rather than typed into
    /// the test, so the guarantee cannot drift with the writer: a label that is
    /// not the derivation, a label of the wrong area, and no label at all all
    /// converge on the one form, and the restored line then stands.
    ///
    /// ´claim:fix:a-perturbed-notice-is-restored-to-the-form-the-sweep-writes´
    /// ´test:unit:restores-a-perturbed-notice-to-the-canonical-form´
    #[test]
    fn restores_a_perturbed_notice_to_the_canonical_form() {
        let (canonical, _outcome, _findings) =
            sweep_todos("// TODO(ADR-L-320): read the flag\n", false);

        let perturbed = [
            format!("// TODO(ADR-L-320) {ACUTE}todo:code:something-else{ACUTE}: read the flag\n"),
            format!("// TODO(ADR-L-320) {ACUTE}todo:test:read-the-flag{ACUTE}: read the flag\n"),
            "// TODO(ADR-L-320): read the flag\n".to_owned(),
            format!("// TODO(ADR-L-320) {ACUTE}todo:code:read-the-flag{ACUTE} read the flag\n"),
            format!(
                "// TODO(ADR-L-320)   {ACUTE}todo:code:read-the-flag{ACUTE}   :   read the flag\n"
            ),
        ];

        for source in perturbed {
            let (repaired, outcome, findings) = sweep_todos(&source, false);

            assert!(
                findings.is_empty(),
                "nothing is refused in {source:?}: {findings:?}"
            );
            assert_eq!(outcome.refused, 0);
            assert_eq!(
                repaired, canonical,
                "the sweep restores the canonical form of {source:?}"
            );

            let (again, outcome, _findings) = sweep_todos(&repaired, false);

            assert_eq!(again, canonical, "and the restored line stands");
            assert_eq!(outcome.unchanged, 1);
        }
    }
}
