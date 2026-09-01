// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Profile-writer round trips through the readers that consume their bytes.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`round_trips_the_written_form_past_the_claim_reader`] | fix | What the writer writes, the claim reader reads: a swept mint and a swept citation both stand at the standard place afterwards, and a second sweep of the written bytes changes nothing. The writer and the reader once disagreed about the separator and nothing in the suite noticed, because no test ran the one over the other's output; this is that test. |
//! | [`round_trips_a_swept_notice_past_the_notice_reader`] | fix | What the notice writer writes, the notice reader reads: the census taken over a swept source finds the same marker, the same summary and the same derived label it found before the sweep, now standing at the notice's standard place, with no label orphaned onto a line the scanner does not read as a notice. The rebuilt line's parts are what make the sweep settle, so they are asserted rather than assumed, on the precedent of the test profile: the two sides once disagreed about a separator across the whole corpus because no test ran the one over the other's output. |

use std::path::Path;

use crate::claim::{Standing, read_claim};
use crate::finding::Finding;
use crate::test_support::{
    notices_of, sweep_notices as sweep_todos, sweep_profile as sweep, test_assets,
};
use crate::todo::{CoveredNotice, scan_todos};

/// Sweep test labels, then read the written bytes with the claim reader.
fn round_trip(text: &str) -> (String, Vec<Standing>) {
    let (rewritten, _outcome, findings) = sweep(text, false);

    assert!(findings.is_empty(), "nothing is refused: {findings:?}");

    let standings = test_assets(&rewritten).iter().map(read_claim).collect();

    (rewritten, standings)
}

/// Read notices and orphaned labels back from the writer's output.
fn read_notices_back(text: &str) -> (Vec<CoveredNotice>, Vec<Finding>) {
    let (_notices, orphans) = scan_todos("torrust-demo", Path::new("src/demo.rs"), text);

    (notices_of(text), orphans)
}

/// What the writer writes, the claim reader reads: a swept mint and a swept
/// citation both stand at the standard place afterwards, and a second sweep
/// of the written bytes changes nothing. The writer and the reader once
/// disagreed about the separator and nothing in the suite noticed, because
/// no test ran the one over the other's output; this is that test.
///
/// ´claim:fix:the-writers-output-passes-the-claim-reader-unchanged´
/// ´test:crate:round-trips-the-written-form-past-the-claim-reader´
#[test]
fn round_trips_the_written_form_past_the_claim_reader() {
    let sources = [
        "/// Checks it.\n///\n/// \u{b4}claim:demo:it-is-checked\u{b4}\n#[test]\nfn covered() {}\n",
        "/// \u{b4}claim:demo:it-is-checked\u{b4}\n#[test]\nfn covered() {}\n",
        "/// Checks:\n/// - the case\n///\n/// (\u{b4}claim:demo:it-is-checked\u{b4})\n#[test]\nfn covered() {}\n",
    ];

    for source in sources {
        let (written, standings) = round_trip(source);

        assert!(
            matches!(standings.as_slice(), [Standing::Claimed(_)]),
            "the writer's output leaves the claim out of place in {source:?}: {standings:?} wrote {written:?}"
        );

        let (twice, outcome, _findings) = sweep(&written, false);

        assert_eq!(
            twice, written,
            "the written form is not a fixed point of the sweep"
        );
        assert_eq!(outcome.unchanged, 1);
    }
}

/// What the notice writer writes, the notice reader reads: the census taken
/// over a swept source finds the same marker, the same summary and the same
/// derived label it found before the sweep, now standing at the notice's
/// standard place, with no label orphaned onto a line the scanner does not
/// read as a notice. The rebuilt line's parts are what make the sweep settle,
/// so they are asserted rather than assumed, on the precedent of the test
/// profile: the two sides once disagreed about a separator across the whole
/// corpus because no test ran the one over the other's output.
///
/// ´claim:fix:the-notice-writers-output-passes-its-own-reader-unchanged´
/// ´test:crate:round-trips-a-swept-notice-past-the-notice-reader´
#[test]
fn round_trips_a_swept_notice_past_the_notice_reader() {
    let sources = [
        "// TODO: read the flag\n",
        "// TODO(ADR-L-320 2026-05-21): read the flag\n",
        "let count = 0; // FIXME: read the flag\n",
        "fn wrapped() {\n    // TODO: read the flag\n}\n",
    ];

    for source in sources {
        let (before, _orphans) = read_notices_back(source);
        let (written, outcome, findings) = sweep_todos(source, false);

        assert!(
            findings.is_empty(),
            "nothing is refused in {source:?}: {findings:?}"
        );
        assert_eq!(outcome.inserted, 1);

        let (after, orphans) = read_notices_back(&written);

        assert!(
            orphans.is_empty(),
            "the written label heads a notice rather than standing orphaned in {written:?}: {orphans:?}"
        );
        assert_eq!(
            after.len(),
            before.len(),
            "the reader finds the same notices in {written:?}"
        );
        assert_eq!(
            after[0].notice().marker(),
            before[0].notice().marker(),
            "the marker's own spelling survives the write"
        );
        assert_eq!(
            after[0].notice().summary(),
            before[0].notice().summary(),
            "and so does the summary every derivation reads"
        );
        assert_eq!(
            after[0].label(),
            before[0].label(),
            "so the label derived after the sweep is the label derived before it"
        );
        assert_eq!(
            after[0].notice().carried(),
            Some(after[0].label()),
            "and it is what stands at the standard place in {written:?}"
        );

        let (twice, outcome, _findings) = sweep_todos(&written, false);

        assert_eq!(
            twice, written,
            "the written form is not a fixed point of the notice sweep"
        );
        assert_eq!(outcome.unchanged, 1);
    }
}
