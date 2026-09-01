// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Source locations and the finding taxonomy.
//!
//! Every rule the calculus can break becomes one variant of [`Finding`], and
//! every variant carries the locations a reader needs to act on it. The
//! duplicate-mint variant carries both locations because the unique-mint
//! invariant (ADR-T-014, A calculus of documentation and source labels) requires a second occurrence to
//! be reported together with the first, never as a harmless repeat.
//!
//! The inventory variants come from ADR-T-015 and the inventory invariant
//! (ADR-T-014, A calculus of documentation and source labels). They are kept apart on purpose rather than
//! collapsed into one "bad label" report, because the repairs differ: a missing
//! label is written by the fix mode, a label whose text differs from the
//! derivation is rewritten by it, a colliding derivation is a naming defect of
//! the assets that only renaming repairs, and a label of an inventory kind
//! standing away from its standard place is the hard failure of the
//! warrant-totality invariant (ADR-T-014, A calculus of documentation and source labels), which no
//! fix mode may paper over.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`counts_lines_and_columns_from_one`] | report | A location counts lines and columns from one, the way an editor does, so a reported position can be typed straight into one: the first byte of a source is line one column one, and the count restarts at each newline. |
//! | [`counts_columns_in_characters`] | report | Columns are counted in characters rather than bytes, so an accented letter earlier in the line does not push the reported column past where the reader's cursor actually sits. |
//! | [`clamps_offsets_past_the_end`] | report | An offset beyond the end of a source clamps to the end rather than panicking, so a caller's arithmetic mistake costs an imprecise location and never the whole run. |
//! | [`near_misses_are_warnings_and_the_rest_are_failures`] | report | The severity split falls in one place: a near miss is a warning, and every other finding is a failure. A span that merely looks like a label therefore cannot fail a run, while anything the calculus actually rejects always does. |

use std::fmt;
use std::path::{Path, PathBuf};

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::label::{Label, Prefix};
use crate::pattern::BytePath;

/// A position in a carrier source, counted in characters from one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    path: PathBuf,
    line: usize,
    column: usize,
    offset: usize,
}

impl Location {
    /// Build a location from a path and a byte offset into that source's text.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, source: &str, offset: usize) -> Self {
        let clamped = offset.min(source.len());
        let preceding = &source[..clamped];
        let line = preceding.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let line_start = preceding.rfind('\n').map_or(0, |index| index + 1);
        let column = source[line_start..clamped].chars().count() + 1;

        Self {
            path: path.into(),
            line,
            column,
            offset: clamped,
        }
    }

    /// The carrier source this location points into.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The one-based line number.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// The one-based column number, counted in characters.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// The zero-based byte offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.path.display(),
            self.line,
            self.column
        )
    }
}

impl Serialize for Location {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Location", 4)?;
        state.serialize_field("path", &self.path.to_string_lossy())?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("column", &self.column)?;
        state.serialize_field("offset", &self.offset)?;
        state.end()
    }
}

/// Render a source path in the reversible byte display.
///
/// A path the display cannot decode is one no declared row could have named
/// either, so it falls back to the path's own rendering rather than being
/// dropped from the message.
fn reversible(path: &Path) -> String {
    BytePath::from_bytes(path_bytes(path))
        .map_or_else(|_| path.display().to_string(), |path| path.display())
}

/// A path's bytes, without a lossy conversion.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

/// A path's bytes, without a lossy conversion.
#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_str().unwrap_or_default().as_bytes().to_vec()
}

/// Whether a finding blocks the check or merely advises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A hard failure: the corpus is not in good standing.
    Failure,
    /// An advisory report that does not block the check.
    Warning,
}

/// Why a covered test does not carry its label at the standard place.
///
/// The standard place of the test profile is the final line of the test
/// function's documentation comment, so each variant names one way that line can
/// fail to be the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingLabelReason {
    /// The test function carries no documentation comment at all.
    NoDocComment,
    /// The final documentation line carries no label.
    NoLabelOnFinalLine,
    /// The final documentation line carries the label beside other text.
    LabelWithExtraText,
}

impl fmt::Display for MissingLabelReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoDocComment => "it has no documentation comment",
            Self::NoLabelOnFinalLine => "the final documentation line carries no label",
            Self::LabelWithExtraText => {
                "the final documentation line carries the label beside other text"
            }
        })
    }
}

/// One rule violation or advisory report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Finding {
    /// A participating import named an owner the citing corpus does not reach.
    UnreachedImport {
        /// The corpus the import stands in.
        citing_owner: String,
        /// The prefix the import named.
        prefix: Prefix,
        /// The owner that prefix registers.
        cited_owner: String,
        /// The manifest edge whose absence refuses the import.
        absent_edge: String,
        /// Where the import stands.
        location: Location,
    },
    /// The root corpus's prose carried an import of a package prefix.
    PolicyImport {
        /// The prefix the import named.
        prefix: Prefix,
        /// The owner that prefix registers.
        cited_owner: String,
        /// Where the import stands.
        location: Location,
    },
    /// A declared may-cite row set disagrees with the manifests it states.
    ReachDeclarationDivergence {
        /// The owner whose rows disagree.
        corpus: String,
        /// The owners the declaration states it may cite.
        registered: Vec<String>,
        /// The owners the manifests and the two standing rules actually admit.
        derived: Vec<String>,
        /// Where the declaration stands.
        location: Location,
    },
    /// The workspace builds a corpus the declaration heads no row for.
    ReachDeclarationOmission {
        /// The owner the workspace builds and the declaration passes over.
        corpus: String,
        /// Where the declaration stands.
        location: Location,
    },
    /// A second bare occurrence minted a label already minted in this owner.
    DuplicateMint {
        /// The doubly minted label.
        label: Label,
        /// The owner whose registry both occurrences fall in.
        owner: String,
        /// The occurrence that minted first in traversal order.
        first: Location,
        /// The occurrence that repeated the mint.
        second: Location,
    },
    /// A bare occurrence used a reserved kind that no profile governs.
    UnwarrantedReservedKind {
        /// The label whose kind is reserved for derivation.
        label: Label,
        /// Where the unwarranted occurrence stands.
        location: Location,
    },
    /// A same-owner citation resolved to no mint anywhere.
    UnresolvedCitation {
        /// The cited label.
        label: Label,
        /// Where the citation stands.
        location: Location,
    },
    /// A same-owner citation resolved to no mint in its own owner, but the
    /// label mints in another owner and wants the import form.
    UnresolvedCitationWantingImport {
        /// The cited label.
        label: Label,
        /// Where the citation stands.
        location: Location,
        /// The owner that does mint the label.
        minting_owner: String,
        /// The import form the citation should have taken.
        suggestion: String,
    },
    /// An imported citation named a prefix the signature does not register.
    UnregisteredPrefix {
        /// The unregistered prefix.
        prefix: Prefix,
        /// The label the citation named.
        label: Label,
        /// Where the citation stands.
        location: Location,
    },
    /// An imported citation named the citing occurrence's own owner.
    SelfQualifiedImport {
        /// The prefix naming the citing occurrence's own owner.
        prefix: Prefix,
        /// The label the citation named.
        label: Label,
        /// Where the citation stands.
        location: Location,
    },
    /// An import-shaped span stood without the enclosing parentheses.
    NonParenthesizedImport {
        /// The prefix the span named.
        prefix: Prefix,
        /// The label the span named.
        label: Label,
        /// Where the span stands.
        location: Location,
    },
    /// A prose block left an unpaired backtick, so its spans are undefined.
    UnpairedBacktick {
        /// Where the block containing the unpaired backtick begins.
        location: Location,
    },
    /// A carrier source could not be read or traversed.
    TraversalFailure {
        /// The path that could not be traversed.
        path: String,
        /// The underlying reason.
        message: String,
    },
    /// A Rust carrier source could not be parsed, so its census is unknown.
    SourceParseFailure {
        /// The path that could not be parsed.
        path: String,
        /// The parser's complaint.
        message: String,
    },
    /// A covered test carries no label at its profile's standard place.
    MissingInventoryLabel {
        /// The label the derivation gives the asset.
        label: Label,
        /// The owner the asset belongs to.
        owner: String,
        /// The asset's bare identifier.
        asset: String,
        /// Which way the standard place failed to carry the label.
        reason: MissingLabelReason,
        /// Where the asset stands.
        location: Location,
    },
    /// A covered test carries a label at the standard place that is not its own.
    ///
    /// Writing the label is attestation, not naming: the derivation-warrant
    /// inference rule (ADR-T-014, A calculus of documentation and source labels) makes an
    /// occurrence at the standard place whose text differs from the derivation
    /// warrant nothing at all.
    WrongInventoryLabel {
        /// The label the derivation gives the asset.
        expected: Label,
        /// The label the standard place actually carries.
        found: Label,
        /// The owner the asset belongs to.
        owner: String,
        /// The asset's bare identifier.
        asset: String,
        /// Where the asset stands.
        location: Location,
    },
    /// Two covered assets of one owner share a bare identifier.
    ///
    /// A collision is a naming defect of the assets under the inventory
    /// invariant (ADR-T-014, A calculus of documentation and source labels), repaired by renaming them
    /// and never by exempting them. ADR-T-015 states the requirement of the
    /// test profile in terms of the identifier rather than the label, which is
    /// the stricter reading: two assets of one owner sharing an identifier in
    /// two areas derive two labels and still collide, because the reader of
    /// either label cannot tell which test is meant.
    CollidingDerivation {
        /// The bare identifier both assets share.
        asset: String,
        /// The owner both assets belong to.
        owner: String,
        /// The label the first asset derives.
        first_label: Label,
        /// The label the second asset derives.
        second_label: Label,
        /// The asset that came first in traversal order.
        first: Location,
        /// The asset that shared its identifier.
        second: Location,
    },
    /// A label of an inventory kind stands somewhere other than a standard place.
    MisplacedInventoryLabel {
        /// The label standing away from any standard place.
        label: Label,
        /// The owner the occurrence falls in.
        owner: String,
        /// The asset in whose documentation the occurrence stands.
        asset: String,
        /// Where the occurrence stands.
        location: Location,
    },
    /// The fix mode could not write a label at an asset's standard place.
    ///
    /// The fix mode writes labels; it never rewrites prose. Where the standard
    /// place cannot be reached by a whole-line edit that preserves the author's
    /// text, the asset is left alone and reported for a reader to settle.
    UnfixableStandardPlace {
        /// The owner the asset belongs to.
        owner: String,
        /// The asset's bare identifier.
        asset: String,
        /// Why the standard place could not be written.
        reason: String,
        /// Where the asset stands.
        location: Location,
    },
    /// A label of the todo kind stands on a line no marker heads.
    ///
    /// The standard place of ADR-T-016's profile is the marker's own line, so a
    /// label of that kind standing anywhere else warrants nothing: there is no
    /// covered notice for it to attest, and the warrant-totality invariant
    /// (ADR-T-014, A calculus of documentation and source labels) makes it a hard failure rather
    /// than a mint.
    OrphanTodoLabel {
        /// The label standing where no notice does.
        label: Label,
        /// Where the occurrence stands.
        location: Location,
    },
    /// A legacy-inventory label stands on a line no legacy marker heads.
    OrphanLegacyLabel {
        /// The label standing where no marked implementation does.
        label: Label,
        /// Where the occurrence stands.
        location: Location,
    },
    /// A covered asset's bare identifier does not transform into a label name.
    UnderivableAssetName {
        /// The owner the asset belongs to.
        owner: String,
        /// The asset's bare identifier.
        asset: String,
        /// The name the transformation produced.
        transformed: String,
        /// Where the asset stands.
        location: Location,
    },
    /// An environment head carries no mint, so its environment has no name.
    MintlessHead {
        /// The head as written.
        head: String,
        /// Where the head stands.
        location: Location,
    },
    /// A mint stands away from every environment head, so it names nothing.
    HeadlessMint {
        /// The label the bare occurrence carries.
        label: Label,
        /// Where the occurrence stands.
        location: Location,
    },
    /// A head's name is one the kind registry does not catalogue.
    UncataloguedHead {
        /// The head as written.
        head: String,
        /// The base name presentation reduction produced.
        base: String,
        /// The label the head mints.
        label: Label,
        /// Where the head stands.
        location: Location,
    },
    /// A head's kind is not one the registry assigns that head's name.
    MisclassifiedHead {
        /// The head as written.
        head: String,
        /// The base name presentation reduction produced.
        base: String,
        /// The label the head mints.
        label: Label,
        /// The kinds the registry does assign the base name.
        catalogued: Vec<String>,
        /// Where the head stands.
        location: Location,
    },
    /// An outline entry claims a head its tracked document does not carry.
    UnfulfilledOutlineEntry {
        /// The head the entry claims.
        head: Label,
        /// The document the entry says carries it.
        document: String,
        /// Where the claiming row stands in the outline.
        location: Location,
    },
    /// A tracked document carries a head no outline entry claims.
    UnclaimedHead {
        /// The head standing unclaimed.
        head: Label,
        /// The outline that tracks the document but omits this head.
        outline: String,
        /// Where the head stands in the tracked document.
        location: Location,
        /// Where the outline declares its tracking of that document.
        declaration: Location,
    },
    /// Two outline entries claim one head, so the tracking is not a function.
    DoublyClaimedHead {
        /// The doubly claimed head.
        head: Label,
        /// The tracked document the head stands in.
        document: String,
        /// The row that claimed first in document order.
        first: Location,
        /// The row that claimed it again.
        second: Location,
    },
    /// A tracking row names an entry the outline itself never mints.
    UnknownOutlineEntry {
        /// The entry the row named.
        entry: Label,
        /// Where the row stands.
        location: Location,
    },
    /// A tracking row names a document the carrier never read.
    UntrackableDocument {
        /// The head the row claims.
        head: Label,
        /// The document the row named.
        document: String,
        /// Where the row stands.
        location: Location,
    },
    /// A row of a tracking table is not a tracking.
    MalformedTrackingRow {
        /// The row's cells, as written.
        text: String,
        /// What a well-formed row would have been.
        reason: String,
        /// Where the row stands.
        location: Location,
    },
    /// A tracking cell is written to participate rather than to display.
    ParticipatingTrackingCell {
        /// The span's interior text.
        text: String,
        /// Where the cell's span stands.
        location: Location,
    },
    /// A parts directory carries no manifest, so its order is undeclared.
    MissingAssemblyManifest {
        /// The parts directory.
        parts: String,
        /// The manifest that would have declared the order.
        manifest: String,
    },
    /// A row of an assembly manifest is not a part.
    MalformedManifestRow {
        /// The row's cells, as written.
        text: String,
        /// What a well-formed row would have been.
        reason: String,
        /// Where the row stands.
        location: Location,
    },
    /// A manifest lists a part the parts directory does not carry.
    AbsentPart {
        /// The part the row named.
        part: String,
        /// The parts directory it was looked for in.
        parts: String,
        /// Where the listing row stands.
        location: Location,
    },
    /// A parts directory carries a part no manifest row lists.
    UnassembledPart {
        /// The part standing unlisted.
        part: String,
        /// The parts directory it stands in.
        parts: String,
        /// The manifest that omits it.
        manifest: String,
    },
    /// A committed publication is not the bytes its parts assemble into.
    StaleAssembly {
        /// The published document.
        target: String,
        /// The parts directory it is assembled from.
        parts: String,
        /// How the committed bytes and the assembly part company.
        reason: String,
    },
    /// A write was requested against a draft assembly, and refused.
    DraftAssemblyWrite {
        /// The published document the write would have overwritten.
        target: String,
        /// The parts directory whose manifest still carries the draft marker.
        parts: String,
        /// The manifest carrying the draft marker.
        manifest: String,
    },
    /// Running text references a division by its number.
    LegacySectionReference {
        /// The reference as written.
        text: String,
        /// Where the reference stands.
        location: Location,
    },
    /// A participating span cites a tag of the superseded two-level system.
    LegacyTagReference {
        /// The tag as written.
        text: String,
        /// Where the citation stands.
        location: Location,
    },
    /// Running text names a decision record by its series and number.
    LegacyRecordReference {
        /// The reference as written.
        text: String,
        /// Where the reference stands.
        location: Location,
    },
    /// Running text names a decision record by a bare number, naming no series.
    ///
    /// This is the ambiguous half of the same shape, reported apart from it
    /// because what is wrong with the two is different. A lettered reference
    /// names a record that existed and cites it the retired way. A bare one
    /// names a number two corpora both use, so it cannot be resolved by reading
    /// it: the reader must find the record meant before the reference can be
    /// rewritten at all. Two registers count them for that reason, and a sweep
    /// that treated them alike would be inferring the series it could not read.
    LegacyUnprefixedRecordReference {
        /// The reference as written.
        text: String,
        /// Where the reference stands.
        location: Location,
    },
    /// A file holds more occurrences of a burned family than its register allows.
    ///
    /// The ratchet of the burn discipline
    /// (ADR-T-020, The migration disciplines) only turns one way, so this is the
    /// failure that forbids growth: the corpus wrote a form the campaign is
    /// retiring, and no commit may add one.
    BurnListGrowth {
        /// The family the burn list counts.
        family: String,
        /// The file whose census grew.
        path: String,
        /// How many occurrences the declared ratchet accounts for.
        registered: usize,
        /// How many occurrences stand there now.
        found: usize,
        /// The first occurrence standing beyond the ratchet, as written.
        text: String,
        /// Where that occurrence stands, when the census could point at one.
        location: Option<Location>,
    },
    /// A burn census is activated and its policy's document says nowhere to walk.
    ///
    /// Where a census looks is the corpus's to state, and it states it in the
    /// document of the policy that activates the census. A policy activated with
    /// no surface declared is therefore a census that would read nothing, report
    /// nothing, and pass its ratchet against nothing — telling the corpus its debt
    /// is discharged by a walk that never happened. It fails loudly here rather
    /// than passing quietly there.
    UndeclaredBurnSurface {
        /// The family whose census was activated.
        family: String,
        /// The policy the activation names.
        policy: String,
        /// The domain whose document would declare the surface.
        document: String,
    },
    /// A register entry outlives the occurrences it counted.
    ///
    /// A burn list that is allowed to overstate becomes a document about a corpus
    /// that no longer exists, so a file whose occurrences have gone must leave the
    /// register in the commit that removed them.
    StaleBurnEntry {
        /// The family the burn list counts.
        family: String,
        /// The file the entry declares.
        path: String,
        /// How many occurrences the declared ratchet accounts for.
        registered: usize,
        /// How many occurrences stand there now.
        found: usize,
        /// Where the entry stands in the canonical list.
        location: Location,
    },
    /// A covered test of a package whose authoring wave has closed carries no claim.
    ///
    /// Staged until that commit, by the staging requirement of ADR-T-017, The test
    /// documentation policy: a claimless test is a coverage figure
    /// while its package's wave is open, and this failure the moment the wave
    /// closes.
    MissingClaimLabel {
        /// The package owning the covered test.
        owner: String,
        /// The test function's bare identifier.
        asset: String,
        /// Where the test stands.
        location: Location,
    },
    /// A claim label stands in a test's documentation, and not as the convention has it.
    DefectiveClaimLine {
        /// The package owning the covered test.
        owner: String,
        /// The test function's bare identifier.
        asset: String,
        /// What the claim line failed to be.
        defect: crate::claim::ClaimDefect,
        /// Where the test stands.
        location: Location,
    },
    /// Two covered tests of one package mint the same statement.
    ///
    /// The repair is to make one of the two statements say what distinguishes it,
    /// never to exempt either: a claim nobody can tell from its neighbour was not
    /// worth writing.
    DuplicateClaimMint {
        /// The doubly minted claim.
        label: Label,
        /// The package both mints fall in.
        owner: String,
        /// The test that minted first in census order.
        first: Location,
        /// The test that repeated the mint.
        second: Location,
    },
    /// A test cites a claim no test of its package mints.
    UnresolvedClaimCitation {
        /// The cited claim.
        label: Label,
        /// The package the citation resolves within.
        owner: String,
        /// The citing test function's bare identifier.
        asset: String,
        /// Where the citing test stands.
        location: Location,
    },
    /// A Rust source's committed test index is not what its labels say it is.
    ///
    /// A hand-edit inside a generated region is indistinguishable from
    /// staleness and is treated as staleness, per the enforcement requirement
    /// (ADR-T-017, The test documentation policy): the check reports it and the fix
    /// overwrites it.
    StaleTestIndex {
        /// The Rust source carrying the index.
        path: String,
        /// How many rows the regeneration produced.
        expected: usize,
        /// How many rows the committed index carries.
        found: usize,
        /// Where the committed index's header stands.
        location: Location,
    },
    /// A folder's committed test matrix is not what its labels say it is.
    StaleFolderMatrix {
        /// The readme carrying the matrix.
        path: String,
        /// The matrix head's mint.
        label: Label,
        /// Where the head stands.
        location: Location,
    },
    /// A folder's readme carries more than one test matrix head.
    RepeatedFolderMatrix {
        /// The readme carrying them.
        path: String,
        /// Where the first head stands.
        first: Location,
        /// Where the second head stands.
        second: Location,
    },
    /// A matrix head states a level the folder's classification does not give it.
    WrongMatrixLevel {
        /// The readme carrying the head.
        path: String,
        /// The head's title, as written.
        title: String,
        /// The level the folder's path classifies it at.
        expected: String,
        /// The mint the derivation gives the head.
        label: Label,
        /// Where the head stands.
        location: Location,
    },
    /// A span that is nearly, but not, an occurrence.
    NearMiss {
        /// The span's interior text.
        text: String,
        /// Why the span did not parse as an occurrence.
        reason: String,
        /// Where the span stands.
        location: Location,
    },
    /// A bare span stands in code commentary, which no census reaches.
    ///
    /// In scanned code text an opening acute declares intent to mint or cite —
    /// the local-classification rule of the participation judgment
    /// (ADR-T-014, A calculus of documentation and source labels) — and commentary can warrant
    /// neither: the profiles census their standard places and nothing else, and
    /// the carrier never mints. The repair is the author's choice of intent:
    /// wrap the span in hugging parentheses to cite, or drop the acutes to
    /// display the label without meaning it.
    UnwarrantedCommentaryMint {
        /// The label the bare span names.
        label: Label,
        /// Where the span stands.
        location: Location,
    },
    /// A parenthesis in code commentary carries a label beside other content.
    ///
    /// A citation is exactly one label hugged by its parentheses. A parenthesis
    /// holding prose beside the span, or a second label, reads as citation
    /// intent that failed to take the form — and the opening acute's declared
    /// intent may not be lost, so the span is reported rather than dropped.
    MalformedCommentaryCitation {
        /// The label the mis-parenthesised span names.
        label: Label,
        /// Where the span stands.
        location: Location,
    },
    /// A citation in a generated region resolves nowhere.
    ///
    /// The generated-compliance invariant of ADR-T-014, A calculus of documentation
    /// and source labels, makes a generated citation
    /// resolve as every citation must, against the completed registries the
    /// generator emitted from. One that dangles is therefore never the author's
    /// slip: either the register is stale — regeneration repairs it — or the
    /// generator wrote a citation of nothing, which is a defect of the
    /// generator itself. Reported apart from the ordinary unresolved citation
    /// so a reader is sent to the regeneration rather than to an edit of the
    /// region.
    DanglingGeneratedCitation {
        /// The label the generated citation names.
        label: Label,
        /// Where the citation stands.
        location: Location,
    },
    /// A bare occurrence stands in generated output that no profile warrants.
    ///
    /// A generated mint is legal exactly where a profile sets its standard
    /// place in a generated register, and no adopted profile does. A bare span
    /// in generated output is therefore a generator writing a mint nothing
    /// warrants — a defect of the generator, reported beside the exactness
    /// check rather than as an author's unwarranted mint.
    BareGeneratedOccurrence {
        /// The label the bare span names.
        label: Label,
        /// Where the span stands.
        location: Location,
    },
    /// A fence opened in commentary is never closed before its document ends.
    ///
    /// A comment region's own document — one item's documentation, or one
    /// block comment — closes at a definite boundary, per rustdoc's model. A
    /// fence still open there is not a warning: everything after the opening
    /// delimiter went unscanned in full, silently, which is exactly the
    /// silent-pass defect a hard finding exists to stop. Reported at the
    /// opening delimiter, the location that names the fence responsible.
    UnclosedCommentaryFence {
        /// Where the unclosed fence's opening delimiter stands.
        location: Location,
    },
    /// A constant's identity stands in a shape its standard place does not own.
    ///
    /// ADR-T-018 gives the identity's place one shape — the mint, a space, and
    /// the program citation hugged by its parentheses, alone on the line — so
    /// that a reader can confirm it by eye. A line minting this kind in any
    /// other shape is reported rather than passed over, because a shape the
    /// checker did not recognise is exactly the case an author needs told about.
    MalformedConstantIdentity {
        /// Where the line stands.
        location: Location,
    },
    /// A constant's program citation names no catalogued pinning program.
    ///
    /// The class of a constant is a citation rather than a word, so adding a
    /// class means minting a program's environment in ADR-T-018 and
    /// implementing its derivation. A citation of anything else is reported at
    /// the constant, where the repair is, rather than left to dangle.
    UncataloguedPinningProgram {
        /// The label the citation names.
        cited: Label,
        /// Where the identity's standard place stands.
        location: Location,
    },
    /// A constant's value is not a shape its cited program accepts.
    ///
    /// The citation says what the value is supposed to be, so a value that is
    /// not that has one of the two wrong — a float cited as a count, or a count
    /// citation over a float — and the author knows which.
    RefusedConstantValue {
        /// The identity whose value was refused.
        identity: Label,
        /// The program that refused it.
        program: String,
        /// The value's normalised source text, when the declaration has one.
        value: Option<String>,
        /// Where the identity's standard place stands.
        location: Location,
    },
    /// A constant derives a pin that is not standing at its standard place.
    MissingConstantPin {
        /// The pin the derivation gives.
        expected: Label,
        /// Where the identity's standard place stands.
        location: Location,
    },
    /// A constant's pin stands, and is not the derivation.
    ///
    /// Kept apart from the missing pin because a reader meeting this one is
    /// being told that the value moved under a pin nobody re-swept, which is the
    /// staleness the exactness discipline exists to catch.
    WrongConstantPin {
        /// The pin the derivation gives.
        expected: Label,
        /// The pin standing at the place.
        found: Label,
        /// Where the standing pin is.
        location: Location,
    },
    /// A pin stands where nothing warrants one.
    UnwarrantedConstantPin {
        /// The label the bare span names.
        label: Label,
        /// What is missing that would have warranted it.
        reason: UnwarrantedPinReason,
        /// Where the span stands.
        location: Location,
    },
    /// A constant carries an identity and cites no warrant for its value.
    ///
    /// The warrant is the occurrence that answers the question a reader of a
    /// magic number actually has, and ADR-T-018 accepts a to-do standard place
    /// in its stead during the bootstrap. What is refused is the third state:
    /// an identity over documentation that neither cites a warrant nor admits
    /// owing one.
    MissingConstantWarrant {
        /// The identity standing without a warrant.
        identity: Label,
        /// Where the identity's standard place stands.
        location: Location,
    },
    /// A surviving file is accounted to no owner by any declared inclusion row.
    ///
    /// The fallback that once answered for an unaccounted path is gone with
    /// nothing put in its place, so the corpus asks whose the file is instead of
    /// assuming (ADR-T-019, The layer owner graph). The path is rendered
    /// in the reversible display, because a file whose name is not text is
    /// exactly the file a lossy report would lose.
    UnaccountedPath {
        /// The path, in the reversible byte display.
        path: String,
    },
    /// A surviving file is accounted by more than one declared inclusion row.
    ///
    /// Exclusivity is stated at the row and not at the owner: two rows matching
    /// one file is a defect even when both name the same owner, because a
    /// relation that can resolve its own conflicts is a relation whose conflicts
    /// nobody has to notice.
    MultiplyAccountedPath {
        /// The path, in the reversible byte display.
        path: String,
        /// How many inclusion rows matched it.
        count: usize,
        /// The matching rows, sorted by owner and then pattern.
        matches: Vec<String>,
    },
    /// An activated pair's prerequisite pair is not activated.
    ///
    /// Activating a policy for an owner activates a claim about what that verdict
    /// rests on, and the prerequisites are this binary's rather than the
    /// declaration's. Absence is not a waiver: the dependent pair is what makes
    /// the prerequisite applicable (´req:commandcontract:dependency-contract´).
    MissingPolicyDependency {
        /// The owner whose pair required it.
        owner: String,
        /// The policy that required it.
        policy: String,
        /// Which scope reached the required pair.
        scope: &'static str,
        /// The owner the prerequisite is wanted of.
        required_owner: String,
        /// The prerequisite policy.
        required_policy: String,
        /// The first citation that required it, for a cited-owner scope.
        location: Option<Location>,
    },
    /// A path is matched by more than one inclusion row of one half.
    ///
    /// Exclusivity is row-level in the owner file's exact sense, so a path
    /// matching two rows fails even when both rows name the same set entry.
    /// There is no priority and no longest-match selection, so an exception is
    /// carved by writing disjoint rows and never by shadowing a broad row with a
    /// narrow one.
    SpdxMultiplyIncluded {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section the rows stand in.
        owner: String,
        /// The half they state.
        half: &'static str,
        /// How many inclusion rows matched.
        count: usize,
        /// The matching rows, sorted.
        matches: Vec<String>,
    },
    /// A path survives a half's exclusion rules and no inclusion row reaches it.
    ///
    /// Totality is what makes an exclusion list carry information: without it,
    /// *not included* and *excluded* are one state, and a reviewer cannot tell an
    /// intentional omission from a forgotten one because both look like absence.
    SpdxUngovernedPath {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner it is accounted to.
        owner: String,
        /// The half that reaches neither way.
        half: &'static str,
    },
    /// A governed file is of a type this binary catalogues no comment leader for.
    ///
    /// Such a file can never conform, so it is named at configuration time rather
    /// than failed forever, and the remedy is an exclusion row somebody writes.
    /// The declared pattern and the catalog decide together whether a file is a
    /// header carrier, so the linter never sniffs a file to guess.
    SpdxUncataloguedType {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
        /// The half that governs it.
        half: &'static str,
        /// The inclusion row that governs it.
        name: String,
    },
    /// A governed entry is a symbolic link, which has no content of its own to head.
    ///
    /// The universe counts a link as an entry in its own right and never follows
    /// it, so a link has no content to inspect and cannot conform. It is not
    /// removed implicitly, for the same reason no ignore-file convention removes
    /// a tracked entry implicitly.
    SpdxLinkedPath {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
        /// The half that governs it.
        half: &'static str,
        /// The inclusion row that governs it.
        name: String,
    },
    /// A section row reaches no accounted path at all.
    ///
    /// A row matching only paths the repository excludes matches nothing, and it
    /// is reported rather than kept as a dead row nobody notices — on the same
    /// principle that makes a stale burn row fail.
    SpdxIdleRow {
        /// The owner whose section the row stands in.
        owner: String,
        /// The half it states.
        half: &'static str,
        /// The list it stands in.
        list: &'static str,
        /// The row's name.
        name: String,
    },
    /// A governed file carries no header line for the field its half requires.
    SpdxMissingHeader {
        /// The path, in the reversible byte display.
        path: String,
        /// The half whose field is absent.
        half: &'static str,
        /// Whether any line carried the field with another text.
        unmatched: bool,
        /// The owner whose section governs it.
        owner: String,
        /// The inclusion row that governs it.
        name: String,
        /// The text that row requires.
        required: String,
    },
    /// A governed file declares a licence its inclusion row does not name.
    SpdxWrongIdentifier {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
        /// The inclusion row that governs it.
        name: String,
        /// The text that row requires.
        required: String,
        /// The text the file declares, in the reversible byte display.
        found: String,
    },
    /// A governed file declares its licence more than once.
    ///
    /// A file has one licence. Two identifier lines are an ambiguity rather than
    /// a formatting lag, and they fail whether or not the two texts agree —
    /// forbidding the second is what keeps the first one meaningful.
    SpdxRepeatedIdentifier {
        /// The path, in the reversible byte display.
        path: String,
        /// How many identifier lines the region carries.
        count: usize,
    },
    /// A path is matched by more than one interchange include row.
    ///
    /// Exclusivity is row-level, so a path matching two rows fails even where
    /// both rows are of the same kind. There is no priority and no longest-match
    /// selection, so an exception is carved by writing disjoint rows.
    InterchangeMultiplyIncluded {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section the rows stand in.
        owner: String,
        /// How many include rows matched.
        count: usize,
        /// The matching rows, sorted.
        matches: Vec<String>,
    },
    /// A governed path the section's declared include gloss does not name.
    ///
    /// Exclusion alone computes governance, so the path is governed whatever the
    /// gloss says. What fails is the claim the gloss makes about it: an include
    /// list, once written, asserts a complete partition of the computed governed
    /// set, and a governed path no row names leaves that assertion false. The
    /// remedy is to widen the gloss or to stop declaring one, never to add an
    /// exclusion — the path's governance was never in question.
    InterchangeGlossUncovered {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
    },
    /// A governed entry is a symbolic link, which carries no document.
    ///
    /// The universe counts a link and never follows it, so a link has no content
    /// of its own to parse. It is not removed implicitly, and the remedy is an
    /// exclude row somebody writes.
    InterchangeLinkedPath {
        /// The path, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
        /// The gloss row naming it, where a declared gloss names it.
        name: Option<String>,
    },
    /// An interchange row reaches nothing the list it stands in is judged over.
    ///
    /// An exclude row is judged over the owner's in-domain share, so a row
    /// matching no path of that share is idle. A gloss row is judged over the
    /// computed governed set, so a row reaching only paths the exclusions removed
    /// or paths outside the domain reaches nothing it could partition and is idle
    /// on the same ground. A dead row nobody notices is worse than one reported.
    InterchangeIdleRow {
        /// The owner whose section the row stands in.
        owner: String,
        /// Which list it stands in.
        list: &'static str,
        /// The row's name.
        name: String,
    },
    /// A governed document or comment names another tracked file by path.
    ///
    /// A label carries the target's identity, its owner and its reach rules; a
    /// path carries none of those and goes stale the moment the target moves.
    /// The finding quotes the spelling its author wrote rather than a file it
    /// might resolve to, because an ambiguous suffix and an ambiguous
    /// segmentation each name no target, and putting a spelling nobody wrote
    /// into a finding would be the recognizer guessing.
    FilePathCitation {
        /// The source, in the reversible byte display.
        path: String,
        /// The owner whose section governs the source.
        owner: String,
        /// The candidate as written, relative prefix included.
        spelling: String,
        /// Which kind of region it stands in.
        region_kind: &'static str,
        /// Where it stands in the source.
        location: Location,
    },
    /// A governed source the section's declared include gloss does not name.
    ///
    /// The `include` records are absent by default and owe nothing where absent.
    /// Where they stand they claim a complete partition of the sources this
    /// policy reads, and a governed source no row names leaves that claim false.
    /// The gloss supplies diagnostic gloss only, so the source is read either
    /// way: the remedy is to widen the gloss or to stop declaring one, never to
    /// exclude a source in order to make a declaration come true.
    FilePathGlossUncovered {
        /// The source, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
    },
    /// A governed source more than one include gloss row names.
    ///
    /// Disjointness is stated at the row, so two rows naming one source fail even
    /// where both stand in the same owner's list. There is no priority and no
    /// longest-match selection, and an exception is carved by writing rows that
    /// do not overlap.
    FilePathGlossMultiplyIncluded {
        /// The source, in the reversible byte display.
        path: String,
        /// The owner whose section the rows stand in.
        owner: String,
        /// How many gloss rows named it.
        count: usize,
        /// The naming rows, sorted.
        matches: Vec<String>,
    },
    /// An include gloss row reaches nothing in the governed set it partitions.
    ///
    /// The gloss is judged over the governed set and nothing wider, so a row
    /// naming only sources the exclusions removed, or only sources of another
    /// owner's share, reaches nothing it could partition. A dead row nobody
    /// notices is worse than one reported.
    FilePathIdleGlossRow {
        /// The owner whose section the row stands in.
        owner: String,
        /// The row's name.
        name: String,
    },
    /// A governed source's kind has no reader in the total carrier catalog.
    ///
    /// The owner ruled that this policy reaches all comments, and an
    /// implementation cannot prove it met that reach by declining to ask whether
    /// a new syntax has comments. So an unclassified kind is loud rather than a
    /// silent skip, and a kind that genuinely carries no comment region is
    /// catalogued as having none rather than left out of the catalog. This
    /// differs deliberately from the interchange program, which was ruled to
    /// ignore the carrier it cannot read.
    FilePathCarrier {
        /// The source, in the reversible byte display.
        path: String,
        /// The owner whose section governs it.
        owner: String,
    },
    /// A governed document's interchange envelope is absent or malformed.
    ///
    /// The verdict is base-theory satisfaction and stops there: the two keys
    /// present, in place, with values of the right shape, and the reserved names
    /// unused as content. It asks nothing about what the label denotes, whether
    /// the repository has allocated it, or whether the content matches a theory
    /// assigned to that coordinate. The check is that the document identifies
    /// itself, not that it identifies itself correctly.
    InterchangeEnvelope {
        /// The path, in the reversible byte display.
        path: String,
        /// What is wrong, in the carrier's own words.
        defect: String,
    },
}

/// Why a pin standing at a constant's documentation warrants nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnwarrantedPinReason {
    /// No identity heads the line the pin stands beneath.
    NoIdentity,
    /// An identity heads it, and derives no such pin from the declaration.
    NoValue,
}

impl fmt::Display for UnwarrantedPinReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoIdentity => "no identity heads it",
            Self::NoValue => "the identity above it derives no such pin",
        })
    }
}

impl Finding {
    /// Whether this finding blocks the check.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        match self {
            Self::NearMiss { .. } => Severity::Warning,
            _ => Severity::Failure,
        }
    }

    /// Whether this finding blocks the check.
    #[must_use]
    pub const fn is_failure(&self) -> bool {
        matches!(self.severity(), Severity::Failure)
    }

    /// The location a reader should be sent to first, when there is one.
    ///
    /// A duplicate mint reports both locations; the second occurrence is the one
    /// that broke the invariant, so it leads.
    #[must_use]
    pub const fn primary_location(&self) -> Option<&Location> {
        match self {
            Self::DuplicateMint {
                second: location, ..
            }
            | Self::CollidingDerivation {
                second: location, ..
            }
            | Self::DuplicateClaimMint {
                second: location, ..
            }
            | Self::RepeatedFolderMatrix {
                second: location, ..
            }
            | Self::DoublyClaimedHead {
                second: location, ..
            }
            | Self::UnfulfilledOutlineEntry { location, .. }
            | Self::UnclaimedHead { location, .. }
            | Self::UnknownOutlineEntry { location, .. }
            | Self::UntrackableDocument { location, .. }
            | Self::MalformedTrackingRow { location, .. }
            | Self::ParticipatingTrackingCell { location, .. }
            | Self::MalformedManifestRow { location, .. }
            | Self::AbsentPart { location, .. }
            | Self::StaleBurnEntry { location, .. }
            | Self::LegacySectionReference { location, .. }
            | Self::LegacyTagReference { location, .. }
            | Self::LegacyRecordReference { location, .. }
            | Self::LegacyUnprefixedRecordReference { location, .. }
            | Self::UnwarrantedReservedKind { location, .. }
            | Self::UnresolvedCitation { location, .. }
            | Self::UnresolvedCitationWantingImport { location, .. }
            | Self::UnregisteredPrefix { location, .. }
            | Self::UnreachedImport { location, .. }
            | Self::PolicyImport { location, .. }
            | Self::ReachDeclarationDivergence { location, .. }
            | Self::ReachDeclarationOmission { location, .. }
            | Self::SelfQualifiedImport { location, .. }
            | Self::NonParenthesizedImport { location, .. }
            | Self::UnpairedBacktick { location }
            | Self::MissingInventoryLabel { location, .. }
            | Self::WrongInventoryLabel { location, .. }
            | Self::MisplacedInventoryLabel { location, .. }
            | Self::OrphanTodoLabel { location, .. }
            | Self::OrphanLegacyLabel { location, .. }
            | Self::UnfixableStandardPlace { location, .. }
            | Self::UnderivableAssetName { location, .. }
            | Self::MintlessHead { location, .. }
            | Self::HeadlessMint { location, .. }
            | Self::UncataloguedHead { location, .. }
            | Self::MisclassifiedHead { location, .. }
            | Self::MissingClaimLabel { location, .. }
            | Self::DefectiveClaimLine { location, .. }
            | Self::UnresolvedClaimCitation { location, .. }
            | Self::StaleTestIndex { location, .. }
            | Self::StaleFolderMatrix { location, .. }
            | Self::WrongMatrixLevel { location, .. }
            | Self::NearMiss { location, .. }
            | Self::UnwarrantedCommentaryMint { location, .. }
            | Self::MalformedCommentaryCitation { location, .. }
            | Self::DanglingGeneratedCitation { location, .. }
            | Self::BareGeneratedOccurrence { location, .. }
            | Self::MalformedConstantIdentity { location }
            | Self::UncataloguedPinningProgram { location, .. }
            | Self::RefusedConstantValue { location, .. }
            | Self::MissingConstantPin { location, .. }
            | Self::WrongConstantPin { location, .. }
            | Self::UnwarrantedConstantPin { location, .. }
            | Self::MissingConstantWarrant { location, .. }
            | Self::FilePathCitation { location, .. }
            | Self::UnclosedCommentaryFence { location } => Some(location),
            // The occurrence beyond the register is named when the census can
            // point at one, and a growth found with no occurrence to name is a
            // register claiming a file the census never read.
            Self::BurnListGrowth { location, .. }
            | Self::MissingPolicyDependency { location, .. } => location.as_ref(),
            Self::TraversalFailure { .. }
            | Self::UndeclaredBurnSurface { .. }
            | Self::SourceParseFailure { .. }
            | Self::MissingAssemblyManifest { .. }
            | Self::UnassembledPart { .. }
            | Self::StaleAssembly { .. }
            | Self::UnaccountedPath { .. }
            | Self::MultiplyAccountedPath { .. }
            | Self::SpdxMultiplyIncluded { .. }
            | Self::SpdxUngovernedPath { .. }
            | Self::SpdxUncataloguedType { .. }
            | Self::SpdxLinkedPath { .. }
            | Self::SpdxIdleRow { .. }
            | Self::SpdxMissingHeader { .. }
            | Self::SpdxWrongIdentifier { .. }
            | Self::SpdxRepeatedIdentifier { .. }
            | Self::InterchangeMultiplyIncluded { .. }
            | Self::InterchangeGlossUncovered { .. }
            | Self::InterchangeLinkedPath { .. }
            | Self::InterchangeIdleRow { .. }
            | Self::InterchangeEnvelope { .. }
            | Self::FilePathCarrier { .. }
            | Self::FilePathGlossUncovered { .. }
            | Self::FilePathGlossMultiplyIncluded { .. }
            | Self::FilePathIdleGlossRow { .. }
            | Self::DraftAssemblyWrite { .. } => None,
        }
    }

    /// The key findings are presented under: source, then position, then code.
    ///
    /// A finding with no location — a traversal or parse failure — sorts before
    /// the located ones, so a tree the checker could not read is reported before
    /// what it managed to read.
    #[must_use]
    pub fn sort_key(&self) -> (Option<&Path>, usize, &'static str) {
        let location = self.primary_location();

        (
            location.map(Location::path),
            location.map_or(0, Location::offset),
            self.code(),
        )
    }

    /// The stable machine-readable code for this finding, matching the JSON tag.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::DuplicateMint { .. } => "duplicate_mint",
            Self::UnwarrantedReservedKind { .. } => "unwarranted_reserved_kind",
            Self::UnresolvedCitation { .. } => "unresolved_citation",
            Self::UnresolvedCitationWantingImport { .. } => "unresolved_citation_wanting_import",
            Self::UnregisteredPrefix { .. } => "unregistered_prefix",
            Self::UnreachedImport { .. } => "unreached_import",
            Self::PolicyImport { .. } => "policy_import",
            Self::ReachDeclarationDivergence { .. } => "reach_declaration_divergence",
            Self::ReachDeclarationOmission { .. } => "reach_declaration_omission",
            Self::SelfQualifiedImport { .. } => "self_qualified_import",
            Self::NonParenthesizedImport { .. } => "non_parenthesized_import",
            Self::UnpairedBacktick { .. } => "unpaired_backtick",
            Self::TraversalFailure { .. } => "traversal_failure",
            Self::SourceParseFailure { .. } => "source_parse_failure",
            Self::MissingInventoryLabel { .. } => "missing_inventory_label",
            Self::WrongInventoryLabel { .. } => "wrong_inventory_label",
            Self::CollidingDerivation { .. } => "colliding_derivation",
            Self::MisplacedInventoryLabel { .. } => "misplaced_inventory_label",
            Self::OrphanTodoLabel { .. } => "orphan_todo_label",
            Self::OrphanLegacyLabel { .. } => "orphan_legacy_label",
            Self::UnfixableStandardPlace { .. } => "unfixable_standard_place",
            Self::UnderivableAssetName { .. } => "underivable_asset_name",
            Self::MintlessHead { .. } => "mintless_head",
            Self::HeadlessMint { .. } => "headless_mint",
            Self::UncataloguedHead { .. } => "uncatalogued_head",
            Self::MisclassifiedHead { .. } => "misclassified_head",
            Self::UnfulfilledOutlineEntry { .. } => "unfulfilled_outline_entry",
            Self::UnclaimedHead { .. } => "unclaimed_head",
            Self::DoublyClaimedHead { .. } => "doubly_claimed_head",
            Self::UnknownOutlineEntry { .. } => "unknown_outline_entry",
            Self::UntrackableDocument { .. } => "untrackable_document",
            Self::MalformedTrackingRow { .. } => "malformed_tracking_row",
            Self::ParticipatingTrackingCell { .. } => "participating_tracking_cell",
            Self::MissingAssemblyManifest { .. } => "missing_assembly_manifest",
            Self::MalformedManifestRow { .. } => "malformed_manifest_row",
            Self::AbsentPart { .. } => "absent_part",
            Self::UnassembledPart { .. } => "unassembled_part",
            Self::StaleAssembly { .. } => "stale_assembly",
            Self::DraftAssemblyWrite { .. } => "draft_assembly_write",
            Self::BurnListGrowth { .. } => "burn_list_growth",
            Self::StaleBurnEntry { .. } => "stale_burn_entry",
            Self::UndeclaredBurnSurface { .. } => "undeclared_burn_surface",
            Self::LegacySectionReference { .. } => "legacy_section_reference",
            Self::LegacyTagReference { .. } => "legacy_tag_reference",
            Self::LegacyRecordReference { .. } => "legacy_record_reference",
            Self::LegacyUnprefixedRecordReference { .. } => "legacy_unprefixed_record_reference",
            Self::MissingClaimLabel { .. } => "missing_claim_label",
            Self::DefectiveClaimLine { .. } => "defective_claim_line",
            Self::DuplicateClaimMint { .. } => "duplicate_claim_mint",
            Self::UnresolvedClaimCitation { .. } => "unresolved_claim_citation",
            Self::StaleTestIndex { .. } => "stale_test_index",
            Self::StaleFolderMatrix { .. } => "stale_folder_matrix",
            Self::RepeatedFolderMatrix { .. } => "repeated_folder_matrix",
            Self::WrongMatrixLevel { .. } => "wrong_matrix_level",
            Self::NearMiss { .. } => "near_miss",
            Self::UnwarrantedCommentaryMint { .. } => "unwarranted_commentary_mint",
            Self::MalformedCommentaryCitation { .. } => "malformed_commentary_citation",
            Self::DanglingGeneratedCitation { .. } => "dangling_generated_citation",
            Self::BareGeneratedOccurrence { .. } => "bare_generated_occurrence",
            Self::UnclosedCommentaryFence { .. } => "unclosed_commentary_fence",
            Self::MalformedConstantIdentity { .. } => "malformed_constant_identity",
            Self::UncataloguedPinningProgram { .. } => "uncatalogued_pinning_program",
            Self::RefusedConstantValue { .. } => "refused_constant_value",
            Self::MissingConstantPin { .. } => "missing_constant_pin",
            Self::WrongConstantPin { .. } => "wrong_constant_pin",
            Self::UnwarrantedConstantPin { .. } => "unwarranted_constant_pin",
            Self::MissingConstantWarrant { .. } => "missing_constant_warrant",
            Self::UnaccountedPath { .. } => "unaccounted_path",
            Self::MultiplyAccountedPath { .. } => "multiply_accounted_path",
            Self::MissingPolicyDependency { .. } => "missing_policy_dependency",
            Self::SpdxMultiplyIncluded { .. } => "spdx_multiply_included",
            Self::SpdxUngovernedPath { .. } => "spdx_ungoverned_path",
            Self::SpdxUncataloguedType { .. } => "spdx_uncatalogued_type",
            Self::SpdxLinkedPath { .. } => "spdx_linked_path",
            Self::SpdxIdleRow { .. } => "spdx_idle_row",
            Self::SpdxMissingHeader { .. } => "spdx_missing_header",
            Self::SpdxWrongIdentifier { .. } => "spdx_wrong_identifier",
            Self::SpdxRepeatedIdentifier { .. } => "spdx_repeated_identifier",
            Self::InterchangeMultiplyIncluded { .. } => "interchange_multiply_included",
            Self::InterchangeGlossUncovered { .. } => "interchange_gloss_uncovered",
            Self::InterchangeLinkedPath { .. } => "interchange_linked_path",
            Self::InterchangeIdleRow { .. } => "interchange_idle_row",
            Self::InterchangeEnvelope { .. } => "interchange_envelope",
            Self::FilePathCitation { .. } => "file_path_citation",
            Self::FilePathCarrier { .. } => "file_path_carrier",
            Self::FilePathGlossUncovered { .. } => "file_path_gloss_uncovered",
            Self::FilePathGlossMultiplyIncluded { .. } => "file_path_gloss_multiply_included",
            Self::FilePathIdleGlossRow { .. } => "file_path_idle_gloss_row",
        }
    }
}

impl fmt::Display for Finding {
    // One exhaustive arm per variant of the taxonomy, each a single write.
    // Splitting the match would scatter the taxonomy's renderings across
    // several functions and make an unrendered variant easy to miss, which is
    // the one mistake this impl exists to prevent.
    #[allow(
        clippy::too_many_lines,
        reason = "one arm per variant of an exhaustive taxonomy"
    )]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateMint {
                label,
                owner,
                first,
                second,
            } => write!(
                formatter,
                "duplicate mint of {label} in owner {owner}: first at {first}, again at {second}"
            ),
            Self::UnwarrantedReservedKind { label, location } => write!(
                formatter,
                "{location}: {label} uses reserved kind {} that no profile governs",
                label.kind()
            ),
            Self::UnresolvedCitation { label, location } => {
                write!(
                    formatter,
                    "{location}: citation of {label} resolves nowhere"
                )
            }
            Self::UnresolvedCitationWantingImport {
                label,
                location,
                minting_owner,
                suggestion,
            } => write!(
                formatter,
                "{location}: citation of {label} resolves nowhere in its own owner; \
                 owner {minting_owner} mints it, so write {suggestion}"
            ),
            Self::UnregisteredPrefix {
                prefix,
                label,
                location,
            } => write!(
                formatter,
                "{location}: imported citation of {label} names unregistered prefix {prefix}"
            ),
            Self::UnreachedImport {
                citing_owner,
                prefix,
                cited_owner,
                absent_edge,
                location,
            } => write!(
                formatter,
                "{location}: owner {citing_owner} imports prefix {prefix} of owner {cited_owner}, \
                 which it does not reach: {absent_edge}"
            ),
            Self::PolicyImport {
                prefix,
                cited_owner,
                location,
            } => write!(
                formatter,
                "{location}: repo-wide policy imports prefix {prefix} of package {cited_owner}; \
                 promote the head instead"
            ),
            Self::ReachDeclarationDivergence {
                corpus,
                registered,
                derived,
                location,
            } => write!(
                formatter,
                "{location}: the may-cite rows state {corpus} citing [{}] and its manifests admit [{}]",
                registered.join(", "),
                derived.join(", ")
            ),
            Self::ReachDeclarationOmission { corpus, location } => write!(
                formatter,
                "{location}: the workspace builds {corpus} and the may-cite rows head none for it"
            ),
            Self::SelfQualifiedImport {
                prefix,
                label,
                location,
            } => write!(
                formatter,
                "{location}: imported citation of {label} qualifies its own owner with {prefix}"
            ),
            Self::NonParenthesizedImport {
                prefix,
                label,
                location,
            } => write!(
                formatter,
                "{location}: import-shaped span [{prefix}-{label}] stands without enclosing parentheses"
            ),
            Self::UnpairedBacktick { location } => {
                write!(
                    formatter,
                    "{location}: unpaired backtick leaves this block's spans undefined"
                )
            }
            Self::TraversalFailure { path, message } => {
                write!(formatter, "{path}: traversal failed: {message}")
            }
            Self::SourceParseFailure { path, message } => {
                write!(
                    formatter,
                    "{path}: could not be parsed, so its census is unknown: {message}"
                )
            }
            Self::MissingInventoryLabel {
                label,
                owner,
                asset,
                reason,
                location,
            } => write!(
                formatter,
                "{location}: {asset} of owner {owner} does not carry {label} at the standard place: {reason}"
            ),
            Self::WrongInventoryLabel {
                expected,
                found,
                owner,
                asset,
                location,
            } => write!(
                formatter,
                "{location}: {asset} of owner {owner} carries {found} at the standard place, but derives {expected}"
            ),
            Self::CollidingDerivation {
                asset,
                owner,
                first_label,
                second_label,
                first,
                second,
            } => write!(
                formatter,
                "colliding derivation on {asset} in owner {owner}: \
                 first at {first} deriving {first_label}, again at {second} deriving {second_label}"
            ),
            Self::MisplacedInventoryLabel {
                label,
                owner,
                asset,
                location,
            } => write!(
                formatter,
                "{location}: {label} stands in the documentation of {asset} of owner {owner} away from any standard place"
            ),
            Self::OrphanTodoLabel { label, location } => write!(
                formatter,
                "{location}: {label} stands where no to-do marker heads a line, so it attests no notice"
            ),
            Self::OrphanLegacyLabel { label, location } => write!(
                formatter,
                "{location}: {label} stands where no legacy marker heads a line, so it attests no implementation site"
            ),
            Self::UnfixableStandardPlace {
                owner,
                asset,
                reason,
                location,
            } => write!(
                formatter,
                "{location}: the standard place of {asset} of owner {owner} cannot be written: {reason}"
            ),
            Self::UnderivableAssetName {
                owner,
                asset,
                transformed,
                location,
            } => write!(
                formatter,
                "{location}: the identifier of {asset} of owner {owner} transforms to `{transformed}`, which is no label name"
            ),
            Self::MintlessHead { head, location } => {
                write!(
                    formatter,
                    "{location}: the head {head} mints no label, so it names nothing"
                )
            }
            Self::HeadlessMint { label, location } => write!(
                formatter,
                "{location}: {label} is minted away from any environment head, so it names nothing"
            ),
            Self::UncataloguedHead {
                head,
                base,
                label,
                location,
            } => write!(
                formatter,
                "{location}: the head {head} of {label} reduces to {base}, which the kind registry does not catalogue"
            ),
            Self::MisclassifiedHead {
                head,
                base,
                label,
                catalogued,
                location,
            } => write!(
                formatter,
                "{location}: the head {head} of {label} reduces to {base}, which the kind registry classifies {}, not {}",
                catalogued.join(", "),
                label.kind()
            ),
            Self::UnfulfilledOutlineEntry {
                head,
                document,
                location,
            } => write!(
                formatter,
                "{location}: this row claims {head}, which {document} does not carry"
            ),
            Self::UnclaimedHead {
                head,
                outline,
                location,
                declaration,
            } => write!(
                formatter,
                "{location}: {head} is claimed by no entry of {outline}, which tracks this document at {declaration}"
            ),
            Self::DoublyClaimedHead {
                head,
                document,
                first,
                second,
            } => write!(
                formatter,
                "{head} of {document} is claimed twice: first at {first}, again at {second}"
            ),
            Self::UnknownOutlineEntry { entry, location } => {
                write!(
                    formatter,
                    "{location}: this row tracks from {entry}, which its outline never mints"
                )
            }
            Self::UntrackableDocument {
                head,
                document,
                location,
            } => write!(
                formatter,
                "{location}: this row claims {head} of {document}, a document the carrier never read"
            ),
            Self::MalformedTrackingRow {
                text,
                reason,
                location,
            } => {
                write!(
                    formatter,
                    "{location}: the tracking row `{text}` is not one: {reason}"
                )
            }
            Self::ParticipatingTrackingCell { text, location } => write!(
                formatter,
                "{location}: the tracking cell `{text}` participates; a tracking table displays its labels"
            ),
            Self::MissingAssemblyManifest { parts, manifest } => write!(
                formatter,
                "{parts} carries parts but no {manifest}, so the order they assemble in is undeclared"
            ),
            Self::MalformedManifestRow {
                text,
                reason,
                location,
            } => {
                write!(
                    formatter,
                    "{location}: the manifest row `{text}` is not a part: {reason}"
                )
            }
            Self::AbsentPart {
                part,
                parts,
                location,
            } => {
                write!(
                    formatter,
                    "{location}: this row lists {part}, which {parts} does not carry"
                )
            }
            Self::UnassembledPart {
                part,
                parts,
                manifest,
            } => write!(
                formatter,
                "{parts}/{part} is listed by no row of {manifest}, so nothing assembles it"
            ),
            Self::StaleAssembly {
                target,
                parts,
                reason,
            } => {
                write!(
                    formatter,
                    "{target} is not what {parts} assembles into: {reason}"
                )
            }
            Self::DraftAssemblyWrite {
                target,
                parts,
                manifest,
            } => write!(
                formatter,
                "{target} was not written: {manifest} marks {parts} a draft, so its parts are not yet the publication's source of truth"
            ),
            Self::BurnListGrowth {
                family,
                path,
                registered,
                found,
                text,
                location,
            } => match location {
                Some(location) => write!(
                    formatter,
                    "{location}: {text} takes {path} to {found} {family}, and the list accounts for {registered}; \
                     a burn list may only shrink"
                ),
                None => write!(
                    formatter,
                    "{path} holds {found} {family} and the list accounts for {registered}; a burn list may only shrink"
                ),
            },
            Self::StaleBurnEntry {
                family,
                path,
                registered,
                found,
                location,
            } => write!(
                formatter,
                "{location}: the list accounts for {registered} {family} in {path}, where {found} now stand; \
                 a row leaves the list in the commit that empties it"
            ),
            Self::UndeclaredBurnSurface {
                family,
                policy,
                document,
            } => write!(
                formatter,
                "{policy} activates the census of {family} and no owner of {document} declares where it walks; \
                 a census with no surface reads nothing and would pass its ratchet against nothing"
            ),
            Self::LegacySectionReference { text, location } => write!(
                formatter,
                "{location}: {text} references a division by its number, which carries no identity"
            ),
            Self::LegacyTagReference { text, location } => write!(
                formatter,
                "{location}: `{text}` cites a tag of the superseded system, not a label"
            ),
            Self::LegacyRecordReference { text, location } => write!(
                formatter,
                "{location}: {text} names a decision record by its number, which carries no identity"
            ),
            Self::LegacyUnprefixedRecordReference { text, location } => write!(
                formatter,
                "{location}: {text} names a record by a number and no series, so which record it means cannot be read from it"
            ),
            Self::MissingClaimLabel {
                owner,
                asset,
                location,
            } => write!(
                formatter,
                "{location}: covered test {asset} of {owner} carries no claim, and its package's authoring wave has closed"
            ),
            Self::DefectiveClaimLine {
                owner,
                asset,
                defect,
                location,
            } => write!(
                formatter,
                "{location}: the claim of covered test {asset} of {owner} is not at the standard place: {defect}"
            ),
            Self::DuplicateClaimMint {
                label,
                owner,
                first,
                second,
            } => write!(
                formatter,
                "duplicate claim mint of {label} in owner {owner}: first at {first}, again at {second}"
            ),
            Self::UnresolvedClaimCitation {
                label,
                owner,
                asset,
                location,
            } => write!(
                formatter,
                "{location}: covered test {asset} cites {label}, which no test of {owner} mints"
            ),
            Self::StaleTestIndex {
                path,
                expected,
                found,
                location,
            } => write!(
                formatter,
                "{location}: the test index of {path} carries {found} rows where its labels give {expected}"
            ),
            Self::StaleFolderMatrix {
                path,
                label,
                location,
            } => write!(
                formatter,
                "{location}: the test matrix of {path} under {label} is not what its folder's labels say it is"
            ),
            Self::RepeatedFolderMatrix {
                path,
                first,
                second,
            } => write!(
                formatter,
                "{path} carries two test matrix heads, at {first} and at {second}, where one folder has one level"
            ),
            Self::WrongMatrixLevel {
                path,
                title,
                expected,
                label,
                location,
            } => write!(
                formatter,
                "{location}: the matrix head of {path} is titled {title}, where the folder classifies at {expected} and derives {label}"
            ),
            Self::NearMiss {
                text,
                reason,
                location,
            } => {
                write!(formatter, "{location}: near-miss span `{text}`: {reason}")
            }
            Self::UnwarrantedCommentaryMint { label, location } => write!(
                formatter,
                "{location}: bare span of {label} stands in commentary, which no census warrants: \
                 parenthesise it to cite, or drop the acutes to display it"
            ),
            Self::MalformedCommentaryCitation { label, location } => write!(
                formatter,
                "{location}: the parenthesis around {label} carries other content beside the span: \
                 a citation is one label hugged by its own parentheses"
            ),
            Self::DanglingGeneratedCitation { label, location } => write!(
                formatter,
                "{location}: generated citation of {label} resolves nowhere: \
                 the register is stale or its generator wrote a citation of nothing — regenerate the projection"
            ),
            Self::BareGeneratedOccurrence { label, location } => write!(
                formatter,
                "{location}: bare span of {label} stands in generated output, \
                 where no profile sets a standard place: the generator mints without warrant"
            ),
            Self::UnclosedCommentaryFence { location } => write!(
                formatter,
                "{location}: a fence opened in commentary is never closed before the comment region ends, \
                 so everything after it went unscanned"
            ),
            Self::MalformedConstantIdentity { location } => write!(
                formatter,
                "{location}: a constant's identity place holds the identity mint and its program citation alone"
            ),
            Self::UncataloguedPinningProgram { cited, location } => write!(
                formatter,
                "{location}: {cited} is no catalogued pinning program, so no derivation pins this constant"
            ),
            Self::RefusedConstantValue {
                identity,
                program,
                value,
                location,
            } => match value {
                Some(value) => write!(
                    formatter,
                    "{location}: the {program} program refuses the value of {identity}, which reads {value}"
                ),
                None => write!(
                    formatter,
                    "{location}: the {program} program pins a value, and the declaration of {identity} carries none"
                ),
            },
            Self::MissingConstantPin { expected, location } => write!(
                formatter,
                "{location}: the derivation gives {expected}, and no such pin stands beneath the identity"
            ),
            Self::WrongConstantPin {
                expected,
                found,
                location,
            } => write!(
                formatter,
                "{location}: the pin reads {found}, and the derivation gives {expected}"
            ),
            Self::UnwarrantedConstantPin {
                label,
                reason,
                location,
            } => {
                write!(
                    formatter,
                    "{location}: the pin {label} warrants nothing, because {reason}"
                )
            }
            Self::MissingConstantWarrant { identity, location } => write!(
                formatter,
                "{location}: {identity} cites no warrant, and its documentation admits owing none"
            ),
            Self::UnaccountedPath { path } => {
                write!(
                    formatter,
                    "owner partition: {path}: unaccounted after exclusion preprocessing"
                )
            }
            Self::MultiplyAccountedPath {
                path,
                count,
                matches,
            } => write!(
                formatter,
                "owner partition: {path}: matched {count} inclusion rows after exclusion preprocessing: {}",
                matches.join(", ")
            ),
            Self::MissingPolicyDependency {
                owner,
                policy,
                scope,
                required_owner,
                required_policy,
                location,
            } => {
                write!(
                    formatter,
                    "policy dependency: {owner} : {policy}: missing {scope} pair {required_owner} : {required_policy}"
                )?;

                // The location's path is rendered in the reversible display
                // rather than through its own rendering, because a file whose
                // name is not text is exactly the one a lossy report would lose.
                location.as_ref().map_or(Ok(()), |location| {
                    write!(
                        formatter,
                        "; first required by {}:{}:{}",
                        reversible(location.path()),
                        location.line(),
                        location.column()
                    )
                })
            }
            Self::SpdxMultiplyIncluded {
                path,
                owner,
                half,
                count,
                matches,
            } => write!(
                formatter,
                "spdx section: {path}: matched by {count} {owner} {half} partitions rows: {}",
                matches.join(", ")
            ),
            Self::SpdxUngovernedPath { path, owner, half } => write!(
                formatter,
                "spdx section: {path}: accounted to {owner}, excluded by no {half} row and matched by no {half} partitions row"
            ),
            Self::SpdxUncataloguedType {
                path,
                owner,
                half,
                name,
            } => write!(
                formatter,
                "spdx section: {path}: governed by {owner} {half} partitions row {name}; \
                 no comment leader is catalogued for this file"
            ),
            Self::SpdxLinkedPath {
                path,
                owner,
                half,
                name,
            } => write!(
                formatter,
                "spdx section: {path}: governed by {owner} {half} partitions row {name}; a symbolic link carries no header"
            ),
            Self::SpdxIdleRow {
                owner,
                half,
                list,
                name,
            } => write!(
                formatter,
                "spdx section: {owner} {half} {list} row {name}: pattern matches no accounted path"
            ),
            Self::SpdxMissingHeader {
                path,
                half,
                unmatched,
                owner,
                name,
                required,
            } => {
                let field = if *half == "identifier" {
                    crate::spdx::IDENTIFIER_FIELD
                } else {
                    crate::spdx::COPYRIGHT_FIELD
                };
                let verdict = if *unmatched { " matches" } else { "" };

                write!(
                    formatter,
                    "spdx {half}: {path}: no {field} header{verdict}; {owner} row {name} requires {required}"
                )
            }
            Self::SpdxWrongIdentifier {
                path,
                owner,
                name,
                required,
                found,
            } => write!(
                formatter,
                "spdx identifier: {path}: header declares {found}; {owner} row {name} requires {required}"
            ),
            Self::SpdxRepeatedIdentifier { path, count } => write!(
                formatter,
                "spdx identifier: {path}: {count} {} headers; a file declares one licence",
                crate::spdx::IDENTIFIER_FIELD
            ),
            Self::InterchangeMultiplyIncluded {
                path,
                owner,
                count,
                matches,
            } => write!(
                formatter,
                "interchange section: {path}: matched by {count} {owner} include rows: {}",
                matches.join(", ")
            ),
            Self::InterchangeGlossUncovered { path, owner } => write!(
                formatter,
                "interchange section: {path}: governed under {owner} and named by no include row; \
                 the declared include gloss does not cover the governed set"
            ),
            Self::InterchangeLinkedPath { path, owner, name } => match name {
                Some(name) => write!(
                    formatter,
                    "interchange section: {path}: governed under {owner}, glossed by include row {name}; \
                     a symbolic link carries no document"
                ),
                None => write!(
                    formatter,
                    "interchange section: {path}: governed under {owner}; a symbolic link carries no document"
                ),
            },
            Self::InterchangeIdleRow { owner, list, name } => write!(
                formatter,
                "interchange section: {owner} {list} row {name}: pattern matches no {} path",
                match *list {
                    "include" => "governed",
                    _ => "accounted",
                }
            ),
            Self::InterchangeEnvelope { path, defect } => {
                write!(formatter, "interchange envelope: {path}: {defect}")
            }
            Self::FilePathCitation { path, spelling, .. } => {
                write!(
                    formatter,
                    "file-path citation: {path}: {spelling} names another tracked file; cite its label"
                )
            }
            Self::FilePathCarrier { path, .. } => {
                write!(
                    formatter,
                    "file-path carrier: {path}: the tracked file kind has no declared region reader"
                )
            }
            Self::FilePathGlossUncovered { path, owner } => write!(
                formatter,
                "file-path section: {path}: governed under {owner} and named by no include row; \
                 the declared include gloss does not cover the governed set"
            ),
            Self::FilePathGlossMultiplyIncluded {
                path,
                owner,
                count,
                matches,
            } => write!(
                formatter,
                "file-path section: {path}: named by {count} {owner} include rows: {}",
                matches.join(", ")
            ),
            Self::FilePathIdleGlossRow { owner, name } => write!(
                formatter,
                "file-path section: {owner} include row {name}: pattern matches no governed path"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Finding, Location, Severity};
    use crate::label::Label;

    /// A location counts lines and columns from one, the way an editor does, so
    /// a reported position can be typed straight into one: the first byte of a
    /// source is line one column one, and the count restarts at each newline.
    ///
    /// ´claim:report:a-location-counts-lines-and-columns-from-one´
    /// ´test:unit:counts-lines-and-columns-from-one´
    #[test]
    fn counts_lines_and_columns_from_one() {
        let source = "alpha\nbeta\ngamma";

        let first = Location::new("f.md", source, 0);
        assert_eq!((first.line(), first.column()), (1, 1));

        let second = Location::new("f.md", source, 6);
        assert_eq!((second.line(), second.column()), (2, 1));

        let third = Location::new("f.md", source, 13);
        assert_eq!((third.line(), third.column()), (3, 3));
    }

    /// Columns are counted in characters rather than bytes, so an accented
    /// letter earlier in the line does not push the reported column past where
    /// the reader's cursor actually sits.
    ///
    /// ´claim:report:a-location-counts-columns-in-characters-not-bytes´
    /// ´test:unit:counts-columns-in-characters´
    #[test]
    fn counts_columns_in_characters() {
        let source = "héllo world";
        let location = Location::new("f.md", source, source.find('w').expect("present"));

        assert_eq!(location.column(), 7);
    }

    /// An offset beyond the end of a source clamps to the end rather than
    /// panicking, so a caller's arithmetic mistake costs an imprecise location
    /// and never the whole run.
    ///
    /// ´claim:report:an-offset-past-the-end-clamps-rather-than-panics´
    /// ´test:unit:clamps-offsets-past-the-end´
    #[test]
    fn clamps_offsets_past_the_end() {
        let source = "abc";
        let location = Location::new("f.md", source, 99);

        assert_eq!(location.offset(), 3);
    }

    /// The severity split falls in one place: a near miss is a warning, and
    /// every other finding is a failure. A span that merely looks like a label
    /// therefore cannot fail a run, while anything the calculus actually
    /// rejects always does.
    ///
    /// ´claim:report:near-misses-warn-and-every-other-finding-fails´
    /// ´test:unit:near-misses-are-warnings-and-the-rest-are-failures´
    #[test]
    fn near_misses_are_warnings_and_the_rest_are_failures() {
        let location = Location::new("f.md", "x", 0);

        let warning = Finding::NearMiss {
            text: "Sec:labels:syntax".to_owned(),
            reason: "wrong casing".to_owned(),
            location: location.clone(),
        };
        assert_eq!(warning.severity(), Severity::Warning);
        assert!(!warning.is_failure());

        let failure = Finding::UnresolvedCitation {
            label: Label::parse("sec:labels:syntax").expect("well-formed"),
            location,
        };
        assert_eq!(failure.severity(), Severity::Failure);
        assert!(failure.is_failure());
    }
}
