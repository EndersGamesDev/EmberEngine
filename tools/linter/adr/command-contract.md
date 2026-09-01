# The Linter Command Contract · `rec:commandcontract:command-contract`

**Status:** Decided

The source doctrines are ADR-L-010, The global command-line output contract;
ADR-L-014, A calculus of documentation and source labels; ADR-L-018, The
constant label profile; ADR-L-019, The layer owner graph; ADR-L-020, The
migration disciplines; and ADR-L-021, The linter command contract root policy.
The last of those promotes the three statements root policy needs. This record
specializes them for the shipped binary and cites those promoted heads where the
specialization begins.

ADR-L-010, The global command-line output contract, fixed the repository's
command-line output contract and deliberately
stopped short of one thing. It fixed the shared exit classes — success, failure,
usage — and left "the exact exit-code taxonomy for root maintenance commands
beyond the baseline" to future command-specific contracts, saying that a command
needing a further class must document it in its own. The corpus checker needs
one, has used one since its first commit, and has never had the contract that
documents it. This record is that contract, for that one binary.

The gap was not theoretical. Two constants in the checker's own source carry an
interim notice where their warrant should stand, each saying in the corpus's own
vocabulary that the record they want does not exist: one on the exit code the
check returns when a corpus is unclean, one on the schema stamp of the stdout
result object. The first named a document rather than a number, which is no
warrant for the number; the second named nothing at all, because the editions it
would have cited were never written down. Both are discharged here, and
discharging them is most of why this record exists rather than an incidental
benefit of it.

Nothing here amends ADR-L-010, The global command-line output contract. The
shared classes keep the meanings that record gave them, the JSON-only stream
rules apply unchanged, and the terminal refusal
applies to every command below. What is added is the command-specific half that
record left open: which subcommands there are, what each takes and emits, the
one non-usage class this binary adds, and the register that says what the stdout
object has promised at each edition of its stamp.

## Scope · `sec:commandcontract:scope`

This record governs the local binary `linter` and nothing else. Its process
boundary is the workspace's generic `cli-common` package rather than an upstream
sibling CLI package. Its subject is that command's argument surface, its exit
codes, and the schema of the JSON objects it writes to stdout.

**Note (The other exit 3 belongs to another program)** · `rem:commandcontract:not-the-config-probe`

ADR-L-009, Container Infrastructure Refactor, records an exit code 3 already,
belonging to the configuration probe, and the global output contract preserves
that program's codes explicitly where it admits command-specific ones. The two
threes are unrelated. They are different
binaries with different contracts, and neither number was chosen with reference
to the other; that both landed on 3 is what a small integer taken from a short
free range looks like twice, not a shared taxonomy.

The note is written because the coincidence is a trap for exactly the reader
this record is for — somebody holding a nonzero status from an ember command
and looking for what it means. The answer depends on which command produced it,
and there is no repository-wide meaning of 3 to look up. What the global output
contract fixes
across all commands is 0, 1 and 2; every code above them is the owning
command's, and this record is the owner of this command's.

## The command surface · `sec:commandcontract:commands`

Nine subcommands, each emitting stdout result data. Every one of them therefore
refuses to run when stdout is attached to a terminal, under the unconditional
refusal of the global output contract, and every one of them writes exactly one
JSON object
followed by one newline. None uses that record's streaming exception. Eight
corpus subcommands take a repository root, defaulting to the working directory;
the formatter instead takes explicit Markdown paths. All nine take the shared
debug flag. The specifications below state what each adds to those and what its
object is for, and say nothing about how any of it is computed. Three of those
specifications head no subcommand of their own: one states the declared
configuration every corpus subcommand loads before anything else, and two state
the maintenance modes of the burn command and the artifacts they leave.

**Specification (The declared configuration)** · `spec:commandcontract:configuration`

This is the package specialization of the whole-key loading specification
promoted to root policy (`[EMBER-spec:lintercli:configuration]`).

Every corpus subcommand reads one declared configuration snapshot before it does anything else. The accepted surface comprises a fixed core — the owner file, the environment relation, the owner-and-policy activation pairs, and the list carried at each pair — plus one declaration named for each parameterized policy family. They are one snapshot and not separate settings. The command requires every core filename and every declaration required by the running binary, parses them all, cross-validates them against each other, and only then runs a policy; there is no partial load, no per-file default, and no compiled or Markdown fallback behind any of them. The `fmt` command is outside this preflight because it formats only the explicit paths it is given and forms no repository-policy verdict.

For the corpus subcommands this is a general rule rather than a series of exceptions, and the rule is what a reader needs. A policy that takes parameters carries them in its own file, named for the policy family, and the surface grows one such file per parameterized family. Nothing else about the snapshot changes as it grows: the new file is required like the others, parsed like the others, and cross-validated with the others, and a family that takes no parameters adds no file. The parameterized families today are the licence-header policy and the interchange-envelope policy.

The envelope policy is one family rather than one family per carrier. It governs first-party documents in three carriers with one identifier and one parameter file, ignores files outside its declared carrier set like every other policy program ignores out-of-scope inputs, and makes the singular identifier deliberate.

The policy vocabulary is the running binary's and takes two identifiers from these rulings: the licence-header check, ``spdx.headers-conform``, and the envelope check under the single family word the naming ruling fixes. Both are identifiers of the checker's own vocabulary and neither is a namespace label, however alike the two vocabularies look — a rule this corpus states where it can be obeyed rather than left to shape under ADR-L-023, Adopting the interchange conventions for first-party structured configuration. The vocabulary itself is still owed a record of its own; until one exists these two identifiers are recorded here, beside the specification that requires their parameter files, and they move with the rest of the vocabulary when it finds its home. Minting either of them is one of the two acts that rule binds, and nothing in the command surface below checks it: the rule emits no finding, so the discipline is the writer's at the moment of minting and there is no verdict to go looking for in any report this record specifies.

One further growth of that general rule is settled here rather than left to a lane, because it changes what a declaration is named for. A policy program carrying repository data in its parameters may be deployed more than once under different payloads, and what the surface activates is then a *policy key* — the program identifier together with a declared family key naming the deployment — rather than the identifier alone. The activation pair's policy component is that key, the list is carried at the key, and the parameter declaration is one per activated key rather than one per program identifier or per family word: a program with three activated keys has three declarations against one parameter schema, while a program admitting no family keeps the single declaration described above. The independence of those keys as debt is the independence rule of ADR-L-020, The migration disciplines, and nothing else about the snapshot changes as it grows this way either — each declaration is required, parsed, and cross-validated like every other.

The word *family* now does two jobs in this surface, and a reader who conflates them will look for the wrong file. The family word of a policy identifier is its first atom, the word before the dot, shared by every check of one subject, and it is what a parameter file has always been named for. A declared family key is the second component of a policy key and names one deployment of one program. The first is the checker's own vocabulary and is minted with the identifier; the second is repository data and is declared — which is why one family word may stand over several programs, and one program over several family keys, without either fact constraining the other. They are different kinds sharing a word, exactly as a namespace label and a policy identifier are under ADR-L-023, Adopting the interchange conventions for first-party structured configuration, and the surface keeps them apart by naming the kind rather than by any test of shape.

Cross-validation is the part worth stating in a command contract, because it
decides what a caller's status means. Two questions are asked in order. The
first is whether the snapshot is a snapshot at all — whether it parses, whether
every name it uses is a name the running binary knows, whether every activated
pair has exactly one list in the codec its policy selects. The second is
whether the snapshot is coherent as a description of this repository — whether
the inclusion rows account for every surviving file exactly once, and whether
every activated pair's prerequisites are activated too. The two questions get
different answers from this command, under
(`rule:commandcontract:configuration-verdicts`), and conflating them is what that
rule exists to prevent.

A third question is asked before either of them, and it is the only ordering constraint inside the load that is visible from outside the command. Every declared file is itself a first-party structured-configuration document, so every one of them carries the interchange envelope the corpus adopted under ADR-L-023, Adopting the interchange conventions for first-party structured configuration. The command reads that envelope first, for each declared file, before it interprets the file's content and before any cross-validation; the priority is envelope, then content, then cross-validation. A declared file whose envelope is absent or malformed refuses the command, and it refuses before its content is read, because a file that has not identified itself is not a snapshot to report against.

The command never edits a declared file to make either answer come out
better. A disagreement between a declaration and the tree is reported, and the
declaration stands until a human changes it; the one exception is the list
row array, which the burn command's writing modes maintain under
ADR-L-020, The migration disciplines, and never beyond it.

**Specification (The check command)** · `spec:commandcontract:check`

The check takes a root and nothing else. Its object is the whole verdict: what
the carrier was found to be, in counts; what each pass over it found, one object
per pass; whether the corpus stands in good standing; and every finding, ordered
by source and position, each tagged with a stable machine-readable code beside
its severity. It is the only command whose object carries the corpus's standing
as a single boolean, and it is the command the gates run.

**Specification (The report command)** · `spec:commandcontract:report`

The report takes a root, an optional label whose citations are to be listed, and
how many of the most-cited mints to name. Its object describes the reference
graph: the summary, the hubs, the orphans, and the citations reaching no mint. A
label that is not well-formed is a usage failure rather than an empty answer,
because asking for the citations of a token no occurrence could carry is a
mistake in the question, and answering "none" would hide it.

**Specification (The coverage command)** · `spec:commandcontract:coverage`

The coverage command takes a root and how many uncited statements to list. Its
object says what the claims come to: the statements written, the intents nobody
has kept, and the mints nothing cites.

**Specification (The shape command)** · `spec:commandcontract:shape`

The shape command takes a root. Its object reports the size of every document and every environment the corpus carries, distributed: words and citations per named environment, and words per division, each carrying its count, extremes, mean and percentiles. It compares the corpus against nothing. A benchmark set of documents nominated inside the binary was the earlier arrangement, and it retires with this wording: which of a corpus's own records are the well shaped ones is a judgment this command has no standing to make, and one that made the report unreadable over any corpus but the one the list was written for.

**Specification (The assemble command)** · `spec:commandcontract:assemble`

The assemble command takes a root and a write flag. Its object carries one
record per declared publication — whether it is dormant, whether it is draft,
whether it was written, and what it was assembled from. Without the flag it
compares and writes nothing; with it, stale publications are regenerated first
and what remains is what writing could not fix.

**Specification (The burn command)** · `spec:commandcontract:burn`

The burn command takes a root and a write flag. Its object carries one record
per declared burn family: the surfaces censused, the occurrences found, the
files holding them, and the rows the register carries. Without the flag it
verifies the registers; with it, it rewrites each to what its census says.

**Specification (The burn maintenance modes)** · `spec:commandcontract:burn-modes`

The command carries three mutually exclusive maintenance modes, and they are
modes of this one command rather than three subcommands because they are three
readings of one relation — what the tree holds against what the lists declare.
Bare verification is the fourth reading and the default.

*Audit* is read-only and formally selective. It reads from standard input a
request naming the
pairs and the identities to compare, and answers with each one classified
exactly once: equal, grown, shrunk, or stale — a row still declared where the
tree now holds nothing. It reports the structurally parseable defects a
declaration can carry — a wrong codec, a non-canonical ordering or encoding, a
pair without its list, an identity attributed to the wrong owner, a stale view
— and it repairs none of them. Audit is where a caller finds out what the
lists say before deciding what to do about it, which is why it must survive a
configuration it would refuse to run policy against.

*Append* is the growth door defined by ADR-L-020, The migration disciplines, and
the only writer that may raise a ceiling. It reads from standard input a request
carrying complete
proposed rows and an authority, re-reads the tree under a lock, and accepts a
row only when it names growth the tree currently holds and its ceiling equals
exactly that current observation. The batch is atomic. *Write* is the lowering
writer and takes no request at all, because lowering needs no authority: it
records what the corpus has already earned.

Audit and append each read exactly one request from standard input after
configuration preflight and require `--output`; there is no `--input` option or
other path-based request transport (`dec:controlsurface:request-stream`). The
output path is accepted only when lexical normalization makes it a direct child
of `--root`. A nested or outside destination and the declared configuration
directory or any direct member of it are refused before the response writer
runs (`dec:controlsurface:lexical-receipt`). The command reads the stream whole
and replaces the response atomically.

**Requirement (The response is the receipt and the stdout object at once)** · `req:commandcontract:control-artifacts`

The response file and the stdout object are the same bytes: exactly one JSON
object and one newline, made durable as a file and then copied to stdout. The
response object's `input` member is the literal string `stdin`, recording the
only request transport. The terminal refusal of the global output contract
applies unchanged, so the mode that writes a
receipt still refuses to speak into a terminal.

Writing both is not redundancy. The stdout object is what the caller in the
loop reads, and the file is what travels with the change as evidence that a
growth was ruled rather than typed — the pair of artifacts is the whole
provenance the corpus gets, since ADR-L-020, The migration disciplines, explains that a
direct edit leaves none. Requiring them to be byte-identical is what stops the
two from telling different stories: a receipt that could differ from the
reported outcome would be evidence of nothing in particular.

The one window where they can part is recorded rather than papered over. An
append makes the list durable before the response, so a crash between the two
loses the external receipt while the batch stands committed entire. It cannot
partially apply the batch, and the digests the response carries — the list
before and after — with the next audit are what expose the window to a caller
who lands in it.

**Specification (The project command)** · `spec:commandcontract:project`

The project command takes a root and a write flag. Its object carries one
outcome per generated projection — how many sources were considered, how many
were unchanged, how many were rewritten, and how many were created from nothing.
Without the flag it regenerates in memory and compares; with it, it writes.

**Specification (The fix command)** · `spec:commandcontract:fix`

The fix command takes a root, the inventory profile to sweep, a write flag, and
a flag admitting a working tree with uncommitted changes. Without the write
flag it reports what the sweep would do; with it, it writes. This write-opt-in
polarity matches the caution of the tree-state precondition, where the former
dry-run opt-in had made writing the default. Its object says what the sweep came
to: covered, inserted, repaired, unchanged, refused, and how many files it
touched. It is the only command that edits sources it was not asked to write by
name, which is why it is the only one with a precondition on the state of the
working tree.

**Specification (The fmt command)** · `spec:commandcontract:fmt`

The fmt command takes one or more Markdown files or directories and an optional
check flag. It recursively discovers lowercase `.md` files without following
symbolic links, parses and serializes every input, and reparses the result to
prove equivalent Markdown events before the first write. Without the flag it
writes every guarded change; with `--check` it reports the paths whose bytes
would change and writes nothing. Its object carries the check mode, files
scanned, files changed, changed paths and elapsed times.

## The exit taxonomy · `sec:commandcontract:exit-codes`

**Rule (The shared classes keep their meanings)** · `rule:commandcontract:shared-classes`

Codes 0, 1 and 2 mean here exactly what the global output contract says they
mean everywhere:
success, a failure of the command itself, and a usage failure including the
stdout terminal refusal. Stdout is empty for 1 and for 2. This record adds one
class above them and redefines none of them.

**Rule (Exit code 3 is a completed run that found failures)** · `rule:commandcontract:findings-exit`

This command exits with code **3** when it ran to completion, wrote its stdout
result object, and that object reports at least one finding of failing severity.
The number is fixed here: it is 3, it is this binary's only command-specific
code, and no other value carries this meaning for this command.

The class exists because the shared three cannot express what this command does.
A check that runs to completion and reports findings has not failed as a
command — it has produced its result, and that result is the point of running
it. Code 1 would be a lie about the run and would oblige an empty stdout,
throwing away the report the caller asked for; code 0 would be a lie about the
corpus and would make every gate that runs this command useless. So the outcome
travels in the status while the report travels on stdout, and 3 is the number
that pairs them. A caller branches on it exactly as it branches on 0, and reads
stdout in both cases.

The check, assemble, project and burn subcommands reach it as checks, and the fix
command reaches it for assets whose standard place it refused to write. The fmt
command reaches it only with `--check`, when at least one guarded output differs;
its write mode exits zero after applying every change. In every findings case
the object on stdout is complete and names what failed.

**Rule (An informational command never exits 3)** · `rule:commandcontract:informational-exit`

The report, coverage and shape commands exit 0 however long their listings are.
They decide nothing, so they have no verdict to signal. A mint nothing cites is
ordinary, an intent nobody has kept is a promise written down rather than a rule
broken, and a long environment may be exactly right — a status that graded any
of them would be inventing a judgment the corpus deliberately does not make.
Codes 1 and 2 keep their shared meanings for these commands, so a caller can
still tell "the listing is in your hands" from "the command did not run".

**Rule (A refused precondition is a failure of the command)** · `rule:commandcontract:precondition-failure`

Where a command refuses to start, it exits 1 with an empty stdout, not 3. The
sweep's refusal to run against a working tree carrying uncommitted changes is
the standing instance: nothing was examined, so there is no result object and no
verdict, and reporting a finding-bearing status would name a corpus condition
where the truth is that the command declined to look. The distinction is the
whole content of the class above — 3 says the run happened and the news is bad,
1 says the run did not happen.

**Rule (A configuration defect is a finding when the configuration parsed)** · `rule:commandcontract:configuration-verdicts`

This is the command-specific specialization of the snapshot-boundary rule
promoted to root policy (`[EMBER-rule:lintercli:configuration-verdicts]`).

The two questions of (`spec:commandcontract:configuration`) fall on opposite sides
of the class above, and the line between them is whether there is a snapshot
to report against.

A snapshot that does not parse, names something the running binary does not
know, duplicates a row or a declared name, or fails to pair an activated policy with exactly one
list in its policy's codec is a refused precondition. It exits 1 with an empty
stdout under (`rule:commandcontract:precondition-failure`): no report is produced,
no policy runs, and no writing mode changes a byte, because a command that
cannot read its own configuration has no standing to say anything about the
corpus.

Instanceable policies add instances to that side too, and likewise no new kind. A family key on a program admitting none, a missing family key on a program requiring one, a repeated key, a declaration whose key is not activated, and an activated key with no declaration are each either a name the running binary cannot resolve or a failure to pair exactly once, and each is refused on the ground the class already gives. A key is well formed or it is not before any of it is a question about this repository, which is why none of these is a finding.

The parameterized policies added instances to that side and no new kind. A pattern the regex engine will not compile is a lexical defect of the declared file and is ranked with the unknown key and the malformed path display under ADR-L-019, The layer owner graph; a set name a section uses without declaring it is a name the binary cannot resolve; and an activated pair whose owner has no section in its policy's parameter file is the pair-to-section defect, which is the parameter file's form of failing to pair. Each is refused for the reason the class already gives.

A snapshot that parses and is internally shaped correctly, but disagrees with
the repository, is judged rather than refused. A path no inclusion row
accounts for, a path more than one accounts for, and an activated pair whose
prerequisite pair is not activated are ordinary findings of failing severity:
the check reports them and exits 3, and an audit that completes with any of
them among its anomalies exits 3 likewise. The run happened, the object is
complete, and the news is bad — which is the whole content of the class.

The parameterized policies added instances to this side too, and the line between the sides is exactly where it was. A containment defect, a totality or exclusivity failure, a catalogue defect and a dead row are all disagreements between a snapshot that parsed and the tree it describes, and all are findings. The rule of thumb the two lists share is that a defect the command can name without consulting the repository is a refusal, and a defect it can only find by looking at the repository is a finding.

One case is new in kind rather than in instance, and it is the envelope of a declared file. A declared file whose envelope is absent or malformed is refused before its content is interpreted at all (`spec:commandcontract:configuration`), which puts it earlier than every other refusal on this side rather than beside them. The ground is the same one — a file that has not identified itself is not a snapshot to report against — and the reason it is worth distinguishing is that the ordering is observable: a snapshot with both a missing envelope and a content defect is refused for the envelope, and a caller who repairs the reported defect first learns nothing new.

Every writing mode refuses to mutate under either answer. That is stricter
than the exit codes alone require, and deliberately so: a partition with an
unaccounted file or a pair whose prerequisite is missing means the command
does not know whose the file is or what the verdict rests on, and a writer
that proceeded would be recording a conclusion drawn from a question the
corpus had not answered.

**Requirement (A missing prerequisite names itself)** · `req:commandcontract:dependency-contract`

Activating a policy for an owner activates a claim about what that verdict
rests on. The prerequisites are the running binary's, held in its policy
catalog beside each policy's recognizer and row codec, and they are not
configurable — a repository ruling selects which policies apply, and the
linter defines what a policy means and therefore what it presupposes. A
declared pair set that is not closed under those prerequisites is the finding
above, and the finding must name the missing pair rather than the symptom.

Three scopes reach a required pair, and each is reported as itself: the
prerequisite wanted of the same owner, the prerequisite wanted of the root
owner for a repository-wide reconciliation, and the prerequisite wanted of
another owner because the requiring owner's sources actually cite into it. The
third carries the first citing location that required it, in source order, and
several citations wanting one missing pair produce one finding rather than a
drift of duplicates. A citation naming a prefix nobody registered instantiates
no requirement at all: it is a defect of the citation, and inventing an owner
to require something of would report the wrong fault twice.

Satisfaction is the presence of the required pair and nothing more. A
prerequisite whose own list is nonempty still satisfies, because a tolerated
defect is a debt the corpus has ruled on rather than a reason to stop judging
what depends on it. Absence is correspondingly not a waiver: the dependent
pair is what makes the prerequisite applicable, so a missing one fails
configuration instead of quietly excusing itself.

## The edition register · `sec:commandcontract:editions`

The stdout result objects carry a schema stamp, and the stamp has been bumped
twelve times without anything recording what each edition promised. A version
nobody documented is a version nobody can migrate against, which is what the
interim notice on the stamp said and what the register below answers.

The register is read as one growing list rather than a specification of each
edition's full shape. What a row states is what the object *began* promising at
that edition — the field or the result shape that edition added, and whether a
reader switching exhaustively on finding codes met codes it had not seen. Every
edition so far has been compatible in the same way: no field of an older edition
has ever changed name or meaning, and the stamp has advanced for additions
alone. That is a fact about the twelve rather than a promise about the
thirteenth; a breaking edition would be a row saying so.

**Table (The edition register)** · `tab:commandcontract:edition-register`

Each row is pinned to the commit where the stamp first carried that value,
derived from the repository history rather than transcribed from a changelog.

| Edition | Commit | What the object began promising |
| --- | --- | --- |
| 1 | `f7fbffc1` | The check report itself: the root, the carrier counts, whether the corpus is clean, and every finding tagged with a code. |
| 2 | `475d3342` | The test profile's counts, in one new object beside the prose counts; and a second result shape, the sweep's own report, travelling under the same stamp. |
| 3 | `23e37ecd` | One count of the environment heads validated against the kind registry, and the four finding codes that pass can raise. |
| 4 | `294c5663` | No field at all: the outline pass and the migration lint reached the report as ten further finding codes, and the graph report gained a result shape of its own. |
| 5 | `6735f46c` | One object counting the publications assembled and compared; and two further result shapes, the assemble command's and the shape command's. |
| 6 | `03be5479` | One object counting the burn families censused and what they came to, and the burn command's own result shape. |
| 7 | `1ecb76e4` | One field per assembly record, marking a publication mid-rewrite as draft, and the finding code for the write a draft refuses. |
| 8 | `0c296bef` | One object counting what the to-do profile found, and the finding code for a notice standing where no label does. |
| 9 | `3bcaf9d2` | Three objects — what the claims, the in-file indexes and the folder matrices came to — and the project command's own result shape. |
| 10 | `9d9e5c57` | One object counting what the claims come to and the intents standing unwitnessed beside them, and the coverage command's own result shape. No finding code: nothing here fails. |
| 11 | `0840213b` | One object counting what the constant profile found, seven finding codes, and a third projection outcome on the project command's result. |
| 12 | `22f13f63` | One object counting what the layer owner graph was found to be, and the four finding codes the reach law raises. |
| 13 | — | One object counting what this register was reconciled to, and the two finding codes that reconciliation raises. |
| 14 | — | One object counting what the declared configuration was read and validated to — the owner, inclusion, exclusion, environment and pair totals, the ceiling rows they carry, the physical partition with its per-rule exclusion tally, and the missing dependencies — and the four finding codes configuration validation raises; and the burn command's two maintenance result shapes, the audit response and the append response, travelling under the same stamp. |

The edition reconciliation retired when the checker stopped extracting runtime data from Markdown documents. Nothing reads this table now, and it stands as the changelog it always was for a consumer tracing an edition back to its diff. Two rows describe promises the object no longer keeps in the words they use — the thirteenth counts a retired reconciliation, and the twelfth names finding codes later renamed when reach reconciliation moved to the declared surface — and neither row is edited, because a register whose rows are never edited is the only kind whose old rows can be trusted.

The stamp itself retired with the decision that report shape is not versioned: the member leaves every result shape the binary emits, the compiled constant and the edition object go with it, and this table stands unedited as the changelog of what the object promised across the fourteen editions it was stamped through.

**Convention (The register is a ratchet)** · `conv:commandcontract:edition-ratchet`

A commit that moves the stamp adds its row in the same commit. The register only
grows: a row is never edited once its edition has shipped, never removed, and
never renumbered, because a consumer pinned to an edition is reading the row that
describes what it was promised. Editions are consecutive from 1, so a gap or a
repeat is a lost row rather than a numbering style.

Writing the row in the bumping commit is what makes the register worth keeping.
A register updated afterwards records what somebody remembered; a register
updated in the same commit records what the diff actually did, and is checkable
against the stamp it describes — which is the requirement following.

The ratchet decides who writes the row for the configuration work above, and
the answer is not this record. The maintenance modes, the configuration
findings and the dependency contract all add result shape, and every one of
them therefore moves the stamp; but a row added here ahead of the commit that
moves it would make the register's newest edition disagree with the stamped
value, which is precisely the divergence
(`req:commandcontract:register-reconciliation`) exists to fail on. The record fixes
what the editions must promise; the implementing commits stamp and row them,
each in the commit that bumps, and this record assigns no number in advance.

One cell cannot be written when its row is. A row pins the commit that first
stamped its edition, and the commit adding a row for its own bump does not yet
know its own hash, so the newest row carries a dash there until the next commit
that moves the stamp fills it in. That completion is the one edit the ratchet
permits, because it finishes a cell that could not have been written rather than
revising a statement that was. The Edition column is what the check reads; the
Commit column is archaeology for a reader tracing an edition back to its diff,
and no check depends on it.

**Requirement (The register is reconciled against the stamp)** · `req:commandcontract:register-reconciliation`

The corpus check reconciles this register against the stamp in the source and
reports every divergence as a finding, so that this record is checked rather
than trusted, exactly as the reach register of ADR-L-019, The layer owner graph,
already is. What the
implementation owes:

- it reads the stamp from the checked source and the register from this
  document, and fails when the newest row's edition is not the stamped value —
  which is the shape a bump without a row takes, and the shape a row without a
  bump takes too;
- it recognizes the register by its header cells alone, following the
  recognition contract in ADR-L-020, The migration disciplines, so that the other
  tables this record prints are passed over and a table naming its columns
  otherwise is not a register with a defect but not one;
- it reads the editions for the ratchet above and fails on a gap, a repeat, or a
  row whose edition cell is not a number;
- it is dormant where this document is absent, as the reach register's
  reconciliation is, so that a fixture tree and any repository adopting the
  checker without this record report nothing rather than diverging forever;
- it reuses the table reader the reach register already uses rather than writing
  a second recognizer for the same shape, for the reason
  given by ADR-L-020, The migration disciplines, about censuses and gates.

The reconciliation is the reason this record registers itself with the migration
lint on arrival rather than deferring, as ADR-L-019, The layer owner graph, and
ADR-L-020, The migration disciplines, each did. Those
records deferred because the lane that would enforce them was still to come.
This record's enforcement lands with it.

## Rejected alternatives · `sec:commandcontract:rejected`

**Ansatz (Reuse code 1 for a corpus with findings)** · `ansatz:commandcontract:reuse-failure`

Signal an unclean corpus with the shared failure class and add no code. Then
The global output contract's rule that stdout is empty on a nonzero exit throws
away the report,
and the command that exists to produce a report produces none exactly when it
has the most to say. A caller would have to re-run the command to see what
failed, against a tree that may have moved. Rejected: the whole difficulty is
that this command has a result *and* a verdict, and one status cannot carry both
unless it has a class for it.

**Ansatz (A code per failing pass)** · `ansatz:commandcontract:code-per-pass`

Give each pass its own exit code — one for the profile, one for the layers, one
for the burn lists — so a caller can branch on which pass failed. Then the
taxonomy grows by one every time a record adds a pass, the numbers run out, and
every existing caller's branch table is wrong the moment a pass is added. The
findings already carry stable per-finding codes on stdout, which is where
per-pass detail belongs: a status has room for a class, not for a catalogue.
Rejected.

**Ansatz (The stamp is the register)** · `ansatz:commandcontract:stamp-alone`

Keep bumping the stamp and write nothing down, on the ground that the diff is
the record. Then the question a consumer actually has — what changed between the
edition I was written against and this one — is answerable only by reading
twelve commits of a package the consumer does not build, and the stamp becomes a
number that means "something moved". Rejected; the notice on the stamp had
already named this as the defect, and a stamp whose editions are unwritten is
the state that notice was standing in for.

**Ansatz (One register row per commit)** · `ansatz:commandcontract:row-per-commit`

Let the register record every commit that touched a result object rather than
every edition of the stamp. Then the register is a changelog, it grows without
bound, and it no longer answers the one question the stamp poses — what this
number promises — because most rows would carry no number at all. Rejected: the
register is indexed by the stamp because the stamp is what a consumer pins.
