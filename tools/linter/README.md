# Ember Linter · `guide:emberlinter:overview`

Repository-wide label-calculus and Markdown linter for the ember workspace.

## Overview · `sec:emberlinter:readme-overview`

This package is the checker the label calculus names as its gate. It reads the
corpus's authored prose and its Rust sources, harvests every mint into its
owner's registry, resolves every citation against those registries, validates
every environment head against the kind registry read from the record's own
tables, and reports what does not hold. Beside the gate it runs the inventory
profiles for tests, to-do notices and declared constants, the claim census of
the test documentation policy, the burn registers that ratchet a migration down
to zero,
and three generated surfaces — ADR-T-017's two projections, the in-file test
index and per-folder test matrix, plus ADR-T-018's constant pins — which it
verifies by exact byte comparison and rewrites on request.

The commands divide by whether they judge. The check, the assembly, the burn
census and the projection verification decide something and exit accordingly;
the report, the coverage and the shape commands describe and exit zero however
long their listings run, because a mint nobody cites, a promise nobody has kept
and a long environment are all ordinary and none of them is a defect.

```sh
linter check --root . | jq .
linter coverage --root . | jq .
linter project --root . --write | jq .
```

## The area register · `sec:emberlinter:area-register`

Every claim this package mints names an area, and this register says what each
area is for. The requirement it answers is
ADR-T-017, The test documentation policy: an owner minting claims carries one
register in its own prose, an entry per area, whose head prose states the stake —
what is lost if claims of this area fail.

The stake is the one thing about an area that no census can compute and no
individual claim states, because a claim says what the system does rather than
why anyone should care that it does it. This owner is a peculiar case and the
entries say so where it matters: because the package is the gate, the shallow
answer for every area is that the gate stops meaning what it says, and the
entries below are written to say instead what specifically stops being
checkable and what a corpus would then quietly accumulate.

The areas are named after the modules that mint in them, with three exceptions
worth knowing when a reader goes looking for the source: the owner area lives
in the workspace-discovery module, the check area in the engine, and the report
area is split across the finding taxonomy and the report envelope. That split
makes the report area a genuine mixture, and the projection area — the largest
here, covering two artefacts that share a staleness discipline and little else —
is the clearest candidate for a split of its own. An unregistered area is a
report line rather than a finding, so a claim minted in a new one is admitted
and counted, and this register is what catches up.

**Section (adoption)** · `sec:emberlinter:area-adoption`

If this fails: the corpus loses its account of who owns what. Every resolution
judgment runs through the signature, so a prefix mapping to no owner, or to
two, decides whether an imported citation reaches a registry or reaches
nothing — and a partition that leaves a source unowned quietly excuses that
source from the calculus rather than failing it. Participation rides here too,
and it now carries one datum rather than two: a document assembled from parts
is read past, because its mints stand in the parts it was assembled from.

**Section (assembly)** · `sec:emberlinter:area-assembly`

If this fails: a document published from parts stops being those parts.
Membership is checked both ways because the likely accident is writing a new
part and forgetting to list it, which looks exactly like having finished;
freshness is checked by exact byte comparison because a publication nobody
republished is a document describing an older corpus. Lose either and a part
is silently dropped from a publication, or an edited part sits unpublished
indefinitely while the committed document reads plausibly and says something
its own sources no longer say. The draft marker is the one deliberate
suspension, and it suspends freshness alone.

**Section (burn)** · `sec:emberlinter:area-burn`

If this fails: the migration loses its ratchet. A register kept per file is
what stops one document paying for another's regression, and it is checked in
both directions, so a register allowed to overstate becomes a document about a
corpus that no longer exists and the floor it was meant to be turns into a
ceiling nobody has measured. The recognisers are the lint's own for the same
reason: a census counting references its own way would drift from the gate's
notion of what a reference is, and the tallies would keep falling while the
debt stayed exactly where it was.

**Section (carrier)** · `sec:emberlinter:area-carrier`

If this fails: the checker judges a corpus that is not the one on disk.
Everything downstream — minting, resolution, head validation — is drawn from
whatever the carrier gathered, so a directory quietly skipped is a tree
exempted from the calculus, and a document silently admitted is prose held to
rules nobody meant it to meet. The distinction carrying the most weight is
absence against unreadability: a tree that simply is not there costs nothing,
while a tree that cannot be read must become a finding, because an unreadable
corpus reported as an empty one passes every check there is.

**Section (census)** · `sec:emberlinter:area-census`

If this fails: the inventory stops being the tests that actually run. The
census is read from the abstract syntax rather than from the bytes, so a test
attribute inside a string literal, a macro body or a comment is never mistaken
for the real thing, and files are enumerated rather than module declarations
followed — the alternative visits one shared support module once per target
and invents collisions that exist nowhere in the source. It also records the
positions and indentation a later pass writes against, so an error here
surfaces as a sweep laying lines in the wrong place rather than as a miscount.

**Section (check)** · `sec:emberlinter:area-check`

If this fails: a citation stops meaning that somebody said the thing. The two
passes are kept strictly apart so that the order the corpus is traversed in
decides nothing; resolution is total so that a label-shaped span reaching no
mint fails rather than lapsing into text; and derived labels are seeded before
the harvest so that prose citing a test breaks when the test is renamed instead
of leaving a quiet lie behind. Weaken any of the three and the corpus goes on
checking clean while accumulating references to statements nobody made, which
is the exact condition the calculus exists to prevent.

**Section (claim)** · `sec:emberlinter:area-claim`

If this fails: what a test establishes stops being written down where the test
is. The claim is authored rather than derived, so nothing computes it and
nothing recovers it once the placement rules stop holding: a claim below the
derived label, sharing its line with other words, or stacked two to a comment
is a statement the projections will render wrongly or not at all. Shared
coverage rests on the mint-and-citation distinction, so losing that turns a
scenario covered many times over back into something a reader must count by
hand across many files.

**Section (cli)** · `sec:emberlinter:area-cli`

If this fails: the gate becomes unusable from anything but a human's eyes. The
check emits one object on standard output and exits zero when clean and with
its documented code when not, so a caller learns both facts from one
invocation; the report, shape and burn commands describe without judging and
exit zero however long their listings run, which is what keeps advisory
information out of the pass-or-fail decision. The sweep's refusal over a dirty
tree is the other guarantee: a mechanical edit across thousands of files is
reviewable only against a clean baseline.

**Section (comment)** · `sec:emberlinter:area-comment`

If this fails: the linter reads its own fixtures as the corpus's debt. A string
literal may carry anything, and these sources carry plenty holding exactly the
shapes the registers count — a reject table, a test's expected output, a URL
whose authority is introduced by two solidi — so a scanner reading a string as
commentary produces a census that can never reach zero without renaming the
code. The quiet failures are the awkward cases: a leading quote opening a
lifetime rather than a literal swallows everything up to the next quote in the
file, and a raw string closing early takes the rest of its line with it.

**Section (constant)** · `sec:emberlinter:area-constant`

If this fails: a value stops being answerable to the argument that fixed it.
The pin is what makes a change loud — every citation of a value dangles in the
commit that moves it — so a derivation that is not injective, or a program that
accepts a shape it cannot encode, turns that alarm into a silence, and a corpus
goes on citing a number the code no longer holds. The census is the other half:
a declaration the census misses is a value nobody can adopt, and a local or a
test fixture the census wrongly reads is a warrant demanded where no record
could ever exist.

**Section (coverage)** · `sec:emberlinter:area-coverage`

If this fails: the corpus loses the instrument the scenario matrix retired
behind. An intent is a statement written down in prose that no test yet
carries, and the join between the two registries is the whole of the evidence
that nothing was dropped when a document full of promises stopped existing.
Nothing here moves an exit status, deliberately, so an error is invisible in
the verdict and shows only as a reviewer being told a promise is kept when no
test establishes it — or being steered by figures computed from some corpus
other than the one the gate judged.

**Section (fix)** · `sec:emberlinter:area-fix`

If this fails: labels go back to being maintained by hand at a scale where
nobody can maintain them. The sweep's whole licence is that it writes labels
and never rewrites prose: every edit is a whole line inserted, a line replaced
when that line was nothing but an attestation, or a notice's own marker line
rebuilt from the parts its reader already parsed — no byte before the marker
moved and no continuation line read at all — and a place it cannot reach any of
those ways is handed back to its author rather than guessed at. Lose that
boundary and a mechanical pass edits somebody's sentences across thousands of
files at once. Idempotence is what makes the sweep safe to run at all, since
one whose second pass differed from its first could never be trusted with the
first.

**Section (graph)** · `sec:emberlinter:area-graph`

If this fails: outline review loses its instrument without losing its exit
status, which is the worst way to lose one. None of these questions is a rule
and none can be — a mint nothing cites is often exactly right, since whole
genres of environment are minted to be flipped in place or pinned later — so
the listings are advisory and a long one is something a reviewer reads rather
than something a run fails on. The derived inventory stays out of every listing
for arithmetic reasons: admit it and the orphan listing runs to thousands with
the handful anybody wanted buried somewhere inside.

**Section (head)** · `sec:emberlinter:area-head`

If this fails: an environment stops having an identity. Minting and heading are
two halves of one act, so a head carrying no label names nothing a citation
could reach, and a label standing away from every head names nothing at all —
and both failures read, in the source, as perfectly ordinary prose. The
recognition has to be narrow in both directions at once: a paragraph merely
opening in bold is how running text stresses its first words, and reading every
one of those as an environment head would find heads throughout prose that
heads nothing, while missing the real shape leaves whole documents unlabelled
and unremarked.

**Section (label)** · `sec:emberlinter:area-label`

If this fails: nothing downstream can trust the shape of what it holds. A value
of the label type exists only for text matching the grammar exactly, which is
what lets every later stage read a kind, an area or a name off the value
instead of splitting the text again and disagreeing about the result. Rendering
back to the source text is what keeps a label written into a report or a
projection the same string its author wrote. The ordering matters as quietly: a
sorted registry and a sorted list of rendered labels must agree, or two views
of one corpus disagree about what comes first.

**Section (legacy)** · `sec:emberlinter:area-legacy`

If this fails: retiring a notation buys nothing, because the retired forms can
still be written. The recognizer reads a source for the shapes the campaign
leaves behind, and what holds a corpus to them is the burn ratchet: each family
is enumerated per file in a register that may only shrink, over the surface its
own list names. A per-document register in the checker was the earlier
arrangement and it retires, because the ratchet already says which sources are
counted and says it where the corpus can read it. Where a form may stand is as
much of the rule as what it looks like: a form in code font is a token exhibited
rather than a reference made, which is what lets the campaign's own documents
name what they ban.

**Section (occurrence)** · `sec:emberlinter:area-occurrence`

If this fails: the difference between establishing a statement and referring to
one stops being carried by the notation. Parentheses alone separate a mint from
a citation, and brackets alone name another owner, so a reader confusing the
forms either invents mints throughout ordinary prose or lets a cross-owner
citation pass unresolved for the loss of two characters. Totality is what keeps
the calculus safe to adopt: a span matching no form is ordinary text, so prose
may quote commands, paths and file names in the same delimiters without any of
them becoming an occurrence a run can fail on.

**Section (outline)** · `sec:emberlinter:area-outline`

If this fails: an outline becomes a document nobody can trust, which is worse
than having no outline at all. The whole point of a document saying entry by
entry what another must contain is that it can be believed, and it is only
believable while something checks both directions — an entry claiming a head
the document does not carry, and a head no entry claims, are equally drift, and
a check running one way lets a document grow silently past its own contract.
The relation is declared rather than inferred for the same reason: a checker
guessing which of a row's citations was the target is the guess an outline
exists to remove.

**Section (owner)** · `sec:emberlinter:area-owner`

If this fails: the signature stops matching the tree it describes. Prefixes are
computed from crate names by one rule rather than transcribed, so a package
joins the corpus by joining the workspace and nobody writes a prefix by hand at
a mint; the record's own table of owners is checked against the computed set
rather than believed, which is the only thing keeping the document and the
workspace from drifting apart unremarked. A manifest that cannot be read has to
be reported rather than passed over, because owners going missing quietly means
a run over a damaged tree reports a smaller corpus and calls it clean.

**Section (profile)** · `sec:emberlinter:area-profile`

If this fails: a test's label stops being a fact about the test. The area comes
from where the file sits and the name from the identifier with its separators
rewritten, and nothing else feeds either, so a classification reading module
nesting instead of the file would make a label change whenever somebody
reorganised a source. Attestation is the sharper edge: the label at the
standard place attests the derivation rather than naming the test, so a renamed
function whose label was not updated must fail — otherwise a citation reaches a
label that describes nothing any more, and the prose that cited it goes on
reading as though it did.

**Section (projection)** · `sec:emberlinter:area-projection`

If this fails: the generated tables stop being owned by the labels and start
being owned by whoever last typed in them. Both projections are compared byte
for byte, so a hand-edit is indistinguishable from staleness and is treated as
staleness: reported by the check, overwritten by the sweep. Recognition is by
exact header row or by label rather than by position, which is what lets an
author add a paragraph above a table without the generator losing track of its
own region, and what leaves the corpus's many hand-written test tables as a
migration backlog rather than as failures on the day the machinery lands.

**Section (prose)** · `sec:emberlinter:area-prose`

If this fails: the checker reads documentation as bytes rather than as prose,
and the participation rule collapses with it. Delimiter pairing and block
structure must be settled before any span's interior is read, or an occurrence
inside a quotation or split across a wrapped list item reads differently from
the same occurrence in a plain paragraph. The display boundary is what makes
the calculus writable at all: a fenced block or a doubled-delimiter span
exhibits a token without meaning it, so a document may show mints and citations
while making neither, and an unpaired delimiter costs its own block rather than
the whole file.

**Section (registry)** · `sec:emberlinter:area-registry`

If this fails: the vocabulary of environments stops being the recorded one. The
classification relation is read from the decision record's own tables rather
than transcribed, and the embedded edition is held against the committed
document, so the two cannot drift apart unnoticed — a transcription would go
stale in silence, and head validation would then reject names the record
plainly catalogues. Presentation reduction is where the subtlety sits: prose
must be free to qualify, number and nest a name without stepping outside the
vocabulary, while a name catalogued in its own right has to beat the reduction
that would strip it back to something it is not.

**Section (report)** · `sec:emberlinter:area-report`

If this fails: a reader cannot act on what the run found. Locations count lines
and columns from one and count columns in characters, so a reported position
can be typed straight into an editor rather than landing past the text an
accented letter displaced, and an offset beyond the end clamps rather than
costing the whole run. The finding variants are kept apart on purpose because
the repairs differ — one the sweep writes, one it rewrites, one only renaming
fixes, one nothing may paper over — and the stable codes beside them are what
let a consumer select the findings it cares about without parsing sentences.

**Section (retired)** · `sec:emberlinter:area-retired`

If this fails: hundreds of references outlive the document that gave them
meaning. Nothing outside the retired matrix defines what its ninetieth scenario
is, or which promises one of its named divisions covers, so once that document
goes a reference to either names nothing at all — and these references sit in
test comments and planning prose where they read perfectly naturally. The
bounds are what make the family countable: an unbounded rule would sweep up
every mark and number in the corpus, giving the register a floor it could never
reach, because reaching zero would mean rewriting prose that was never about
the matrix.

**Section (shape)** · `sec:emberlinter:area-shape`

If this fails: the standard for a well-shaped record goes back to having no
numbers attached to it. Nothing here is a rule and nothing may become one — a
long environment may be exactly right, and a short one may be the sharpest
paragraph in the corpus — so the report raises no finding and the reader
decides. What gives the numbers their meaning is that the band comes from the
documents the corpus already considers well shaped rather than from a constant
somebody typed, and that divisions are measured apart, since a section heading
whose next block is a subsection heads little but its own line.

**Section (todo)** · `sec:emberlinter:area-todo`

If this fails: the deficiency backlog stops being measurable. A notice has no
identifier the language gives it, so its author's own opening words are the
identifier — the alternative, a slug written beside the label, is authorship
wearing a derivation's clothes and lets the two disagree. Only the marker's own
line feeds the name, which is what lets a reader confirm a label against the
line it stands on and what stops re-wrapping a long notice re-deriving its
label. The check and the register read through one recogniser for the reason
the ratchet needs: two counters would eventually disagree about what a notice
is.

## License · `sec:emberlinter:readme-license`

Copyright (c) 2024 Wild Sky Maker.

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU Affero General Public License as published by the Free
Software Foundation, version 3.

See [LICENSE](../../LICENSE) for details.
