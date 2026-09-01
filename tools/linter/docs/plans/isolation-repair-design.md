# Isolation repair: one declared execution plan · `plan:isolation:repair-design`

This plan replaces compiled repository knowledge with one resolved plan built from declared inputs. It is an implementation design, not a second census of the tree and not an amendment to the root records it cites.

## Boundary and settled exceptions · `sec:isolation:boundary`

**Requirement (The package knows programs while the repository declares instances)** · `req:isolation:boundary`

The isolation test is whether the package can be checked in a standalone checkout against an invented declared surface. It may know the schemas and generic meanings of supported declarations, generic language and manifest conventions, policy programs, codecs, and command behavior. It may not know a sibling identity, repository path, current owner roster, concrete topology, adoption population, migration payload, or current count. Repository facts reach it only through a complete declared snapshot; absence never reconstructs today's repository and instead means not applicable, empty, or refused according to the owning schema (`spec:commandcontract:configuration`).

Three boundaries are settled rather than generalized. Direct use of the root decision-record directory remains the sanctioned policy-authority reach. The shared sibling CLI library remains an allowed dependency and may be named by this package. The package readme's link to the root licence remains a narrow legal-packaging exception. None licenses a second sibling dependency, a general walk above the package, or a package test against the live tree.

Self-hosting leaves the package. Whole-tree conformance becomes one root CI invocation of the shipped `check` command; it does not become a root acceptance-test crate or a renamed package suite. Package integration tests pass a fixture root and a fixture snapshot. The exported helper that derives the repository root from package layout is deleted, and command callers supply their root explicitly.

## Outline attack · `sec:isolation:outline-attack`

**Report (The attacked outline survives only with eight seams made explicit)** · `rep:isolation:outline-attack`

| Attack | Failure exposed | Correction retained in the design |
| --- | --- | --- |
| A central deployment file gathers unrelated policy data | The file would have no verdict or owning program and would become an unbounded repository census | Put each policy choice in the parameter document of the policy that consumes it; reserve the sole shape document for the universe kind and global-ignore relation and derive calculus-fixed inputs |
| Parameter files can redefine a policy | Configuration could turn a repository choice into executable semantics | Keep tokenization, participation, matching, diagnostics, codec, and dependency templates in a closed program catalogue |
| Three prefix-number payloads collapse into one ratchet | Independent migrations could trade debt or share accidental identity | Give each declared family its own file, family key, activation, allowance table, and run while sharing one compiled program and schema |
| Source selection duplicates ownership | A policy scope could become a second attribution map | Derive the label carrier from the calculus; let other content policies select inputs in their own documents, then attribute every selected source through the owner partition exactly once |
| Moving tests to CI recreates a root suite under another name | Package ownership would survive in a wrapper | Use one root pipeline invocation of the shipped command; retain no root locator or root assertions in package tests |
| A shadow plan becomes a permanent fallback | Missing declarations could silently recover today's topology | Make shadow equivalence temporary and delete the compiled constructors in the cut-over bite |
| Namespace allocation leaks into configuration | A data file would perform the owner act that the adoption record reserves | Stamp allocated schema identities in files, but keep allocation solely in the root record |
| One plan still permits analyzers to reach global catalogues | The new type would be decorative while old knowledge remained callable | Require a plan or typed subplan at every analyzer and writer boundary; remove repository constructors from those modules |

The attack rejects a single omnibus “repository” policy and any shape document broader than the minimal universe-and-ignore declaration. The one shape document has no verdict, recognizer, codec, or allowance list and is closed to the corpus universe kind and global-ignore relation. Each policy choice instead stands in the parameter document of the policy that consumes it, and resolution joins the documents only to validate their cross-instance relations; each verdict remains independently activated with its own ratchet.

## Declared policy instances · `sec:isolation:policy-instances`

**Schema (A policy key separates compiled meaning from declared family identity)** · `schema:isolation:policy-key`

A `PolicyProgram` is compiled meaning: identifier, parameter schema, scanner and judgment, diagnostic fields, allowance codec, and dependency templates. A `PolicyKey` is a program identifier plus an optional declared family key. Existing singleton programs omit the family. The three new parameterized programs require one, allowing several repository-declared instances without teaching the binary which instances this repository happens to have. A program identifier names a verdict while an envelope namespace names the parameter schema; they are deliberately different kinds under ADR-T-023, Adopting the interchange conventions for first-party structured configuration.

The parameter document declares its program and family. An activation row names owner, program, and the same family; its allowance table is keyed the same way. For a family-bearing key the table shape is `[OWNER."POLICY"."FAMILY"]`; singleton tables keep `[OWNER."POLICY"]`. Thus the unit defined by ADR-T-020, The migration disciplines, remains one owner and one instantiated policy, while independent instances cannot share debt. A parameter document with no matching key, a key with no parameter document, duplicate family keys, a family on a singleton program, or a missing family on an instanceable program is a snapshot refusal.

The policy document holds both the small values that specialize compiled meaning and the repository choices that deploy that policy instance: its source bound and, for a census family, its register destination. Activation and tolerated debt remain in the fixed core declarations. The resolved `PolicyRun` joins the key, owner, compiled program, validated parameters, selected sources, codec-shaped allowances, optional register destination, and dependency-closed schedule. Findings and control receipts carry the full key, so two instances using one program never alias.

## Mark-numbered references · `sec:isolation:mark-numbered`

**Proposal (Marked ordinal references are one bounded policy)** · `proposal:isolation:mark-numbered`

Good practice: a marked ordinal is navigation within a mutable numbered sequence, not a stable identity; authored prose cites the stable identity of the thing instead. The compiled program is `references.mark-numbered-absent`, uses the path-count codec, and has no policy dependency.

The program bakes in participating-region selection, token opening, reading the entire decimal run, rejecting a longer run rather than accepting a prefix, inclusive-bound comparison, display exclusion, source-offset mapping, deterministic ordering, and diagnostics. `.linter/policy-mark-numbered-references.toml` holds family `scenarios`, the mark, the inclusive numeric bound, that instance's include and exclude scope, and its register destination. Its initial data moves the current mark, range, scope, and destination unchanged. The allocated schema label is ``com.torrust.index.linter.policy-mark-numbered-references``; its allocation is recorded outside configuration under ADR-T-023, Adopting the interchange conventions for first-party structured configuration.

```toml
namespace = "com.torrust.index.linter.policy-mark-numbered-references"
version = [1, 0, 0]
policy = "references.mark-numbered-absent"
family = "scenarios"

[parameters]
mark = "#"
minimum = 1
maximum = 91

[scope]
include = []
exclude = []

[register]
destination = ""
```

## Literal-set references · `sec:isolation:literal-set`

**Proposal (Private heading vocabularies are one literal-set policy)** · `proposal:isolation:literal-set`

Good practice: a selected heading sentence used verbatim as a locator is an implicit identity system; references use the stable identity minted by the referenced environment instead. The compiled program is `references.literal-set-absent`, uses path-count debt, and has no policy dependency.

The program owns participating-region selection, exact full-literal matching, finding every match, display exclusion, offsets, ordering, and diagnostics. It rejects an empty literal, duplicate value, or value carrying a line break. `.linter/policy-literal-set-references.toml` holds family `divisions`, the verbatim string set, that instance's include and exclude scope, and its register destination; the data bite transcribes the current values unchanged without repeating them in this design. Its allocated schema label is ``com.torrust.index.linter.policy-literal-set-references``.

```toml
namespace = "com.torrust.index.linter.policy-literal-set-references"
version = [1, 0, 0]
policy = "references.literal-set-absent"
family = "divisions"

[parameters]
values = []

[scope]
include = []
exclude = []

[register]
destination = ""
```

## Work-package prefix numbers · `sec:isolation:work-package-numbers`

**Proposal (Enumerated prefixed plan references are one prefix-number instance)** · `proposal:isolation:work-package-numbers`

Good practice: a work item named only by a plan-local prefixed number loses its referent when the plan changes or retires; prose cites the work's stable identity. This is family `work-packages` of the compiled `references.prefix-numbers-absent` program and keeps its own path-count table.

The shared program bakes in prefix token-opening, consumption of one complete dot-joined decimal token, rejection of alphanumeric tails, display exclusion, offsets, ordering, and diagnostics. For locator-shaped instances it also bakes in shielding for a complete occurrence claimed by the generic section-reference grammar; configuration cannot widen or disable that precedence. The exact-token bound compares the whole normalized token, so dotted work-item numbers remain distinct. `.linter/policy-prefix-numbers-work-packages.toml` holds the current prefix, exact enumeration, instance scope, and register destination, copied unchanged in the data bite.

```toml
namespace = "com.torrust.index.linter.policy-prefix-numbers"
version = [1, 0, 0]
policy = "references.prefix-numbers-absent"
family = "work-packages"

[parameters]
prefix = "WP-"
exact = []

[scope]
include = []
exclude = []

[register]
destination = ""
```

## Chapter prefix numbers · `sec:isolation:chapter-numbers`

**Proposal (Ranged prefixed chapter references are one prefix-number instance)** · `proposal:isolation:chapter-numbers`

Good practice: a chapter locator is navigation within one publication, not an identity that remains valid after the publication's structure changes; prose cites the chapter's stable head. Family `chapters` shares the prefix-number program and schema but has independent activation, scope, debt, and diagnostics.

For a leading-component range the program still consumes the complete dotted token, then compares its first decimal component with the inclusive bound. Suffixes therefore belong to the one occurrence rather than becoming a second token. `.linter/policy-prefix-numbers-chapters.toml` holds the current prefix, inclusive leading range, instance scope, and register destination.

```toml
namespace = "com.torrust.index.linter.policy-prefix-numbers"
version = [1, 0, 0]
policy = "references.prefix-numbers-absent"
family = "chapters"

[parameters]
prefix = "L-"

[parameters.leading_range]
minimum = 1
maximum = 30

[scope]
include = []
exclude = []

[register]
destination = ""
```

## Record prefix numbers · `sec:isolation:record-numbers`

**Proposal (Enumerated prefixed record locators are one prefix-number instance)** · `proposal:isolation:record-numbers`

Good practice: a record locator stripped of the record system that qualified it is ambiguous historical shorthand; prose names the record's surviving stable identity. Family `records` is another independent prefix-number run.

The leading-set bound consumes the complete dotted token and admits it only when its first component is in the configured integer set. `.linter/policy-prefix-numbers-records.toml` carries the same current prefix and transcribes the current gapped enumeration without reproducing it here, together with its instance scope and register destination. Resolution over all policy documents rejects overlapping active prefix-number domains over the same source bound, so the ranged chapter family and enumerated record family cannot count one occurrence twice. All three prefix-number files stamp the one allocated schema label ``com.torrust.index.linter.policy-prefix-numbers`` because namespace identifies their common parameter schema, not any of the three policy instances. Their full `PolicyKey` values remain distinct.

```toml
namespace = "com.torrust.index.linter.policy-prefix-numbers"
version = [1, 0, 0]
policy = "references.prefix-numbers-absent"
family = "records"

[parameters]
prefix = "L-"
leading = []

[scope]
include = []
exclude = []

[register]
destination = ""
```

Splitting the old combined observer does not discard its payload-free remainder. The word-shaped-mark complement remains a compiled generic policy because its grammar is executable meaning rather than repository data; its own document under the mark-numbered-reference schema carries only its family, deployment scope, and register destination, and it receives an independent allowance table like every other run.

## Owner and crate names · `sec:isolation:owner-crate-names`

**Proposal (Participating owner spellings are derived from crate names)** · `proposal:isolation:owner-crate-names`

Good practice: a participating owner's spelling is the deterministic projection of its crate name, not an independently chosen repository identity. The compiled program is `owners.crate-names-conform`, uses the fingerprint codec, and has no policy dependency.

The program bakes in the one derivation rule—strip the leading project namespace when present, remove hyphens, and uppercase the remainder—and reconciliation in both directions between discovered workspace members and registered owners participating in at least one policy or profile pair. Every discovered workspace member must derive a participating registered owner, and every participating registered owner must be accounted for by either one discovered member or one declared unbuilt entry; an invalid derivation, a collision, a missing registration or participation, an unexplained participating registration, or disagreement between a declared unbuilt directory and the owner partition is a policy finding. A registered owner with no activated pair makes no crate claim and is outside this verdict. These are structured relationship identities carrying the relevant crate, owner, and directory fields, not occurrences within a path, so the fingerprint codec preserves distinct defects that either path codec would alias.

Following the parameter-file rule established by the SPDX precedent, `.linter/policy-owner-crate-names.toml` holds the project namespace the derivation strips, together with the crate-name and directory rows for participating registered owners whose manifests the workspace does not supply. The namespace is declared rather than compiled for the same reason the rows are: which spelling a corpus's crate names open with is repository data, and a program instantiated from a namespace and an unbuilt set must receive both or half of its value stays behind in code. Built members contribute no row because their names and directories come from manifests. The allocated schema label is ``com.torrust.index.linter.policy-owner-crate-names``.

```toml
namespace = "com.torrust.index.linter.policy-owner-crate-names"
version = [1, 0, 0]
policy = "owners.crate-names-conform"

[parameters]
namespace = "torrust-"

[[parameters.unbuilt]]
crate_name = "torrust-unbuilt"
directory = ""
```

Activation is ordinary declared policy state rather than an implicit reconciliation pass: one owner activates the repository-wide singleton program without a family key, its enveloped parameter document is discovered with the other policy documents, and that pair has one fingerprint allowance table. The resolved run receives the complete participating-owner set. A missing or repeated parameter document, no activation, more than one activation, or a mismatched list refuses the snapshot; absence of every policy and profile pair for a registered non-crate owner is non-applicability and never reconstructs a compiled sibling constant.

## Policy-owned repository data · `sec:isolation:policy-data`

**Justification (Each repository choice stands with the policy that consumes it)** · `just:isolation:policy-data`

The retiring design and namespace censuses mixed declared inputs with historical counts, look-alike strings, package rosters, and worked current-state tables. The measurements perform no runtime job and are deleted. Each surviving datum is justified by one consuming policy and stands in that policy's parameter document, except where the calculus, the one shape document, or existing adoption data already determines it; there is no omnibus schema or authored membership list.

*Repository shape.* Beyond its envelope, `.linter/shape.toml` contains exactly two declarations: one `universe` answer, either `git-tracked` or `as-written`, and one `ignore` list of `{ name, pattern }` rows. The universe choice is repository data declared once rather than meaning compiled into each policy program, so the linter remains generic over corpora that do not use git. This repository declares `git-tracked`; an untracked draft is therefore outside the base universe and creates neither a finding nor debt. The ignore relation then removes the union of every matching row before anything ranges over that universe: every policy, every profile, the label corpus, and the owner partition's accounting all receive only the surviving paths, and a globally ignored path needs no owner attribution. The document is closed at those two content declarations; adding a third question or relation is an owner act, not a schema extension inferred by implementation.

Ignore rows reuse the owner surface's established vocabulary: each row carries a unique declared name and a standard regular-expression pattern matched in full against a repository-relative byte path. Row order is immaterial, overlapping matches are legal because removal is their set union, and a duplicate name or malformed pattern refuses during content decoding. The flagged recommendation is that this repository's list is empty: its `git-tracked` answer and owner activations resolve every calculus exclusion class without a residual path to ignore. The relation remains a generic capability for a future corpus whose declared universe and ownership leave such a residue.

The document is enveloped under the allocated schema label ``com.torrust.index.linter.shape``. Its allocation is recorded outside configuration under ADR-T-023, Adopting the interchange conventions for first-party structured configuration.

```toml
namespace = "com.torrust.index.linter.shape"
version = [1, 0, 0]
universe = "git-tracked"

ignore = [
]
```

This plan recommends, and flags for owner decision, that `.linter/shape.toml` is a fixed core document whose absence refuses the snapshot like absence of any other fixed core document. The configuration loader is grounded below both shape declarations and is not configurable: it reads `.linter/` documents physically and byte-for-byte as written, never consulting git and never applying the global ignore, including when it reads `.linter/shape.toml` itself. The file that declares the corpus universe and its global ignore cannot depend on either declaration it supplies.

*Manifestless members.* The declared parameters of `owners.crate-names-conform` carry each intentionally unbuilt crate-name and directory pair in `.linter/policy-owner-crate-names.toml`. Discovery cannot distinguish intentional manifest absence from a drifted or deleted manifest, and owner spelling is not uniquely reversible to a crate name, so the named policy needs both fields to reconcile the participating roster in both directions. A row leaves when its manifest arrives, and the parameter set becomes empty if every participating owner builds.

*The label corpus.* The label family takes no carrier parameter. The Minting judgment defines its carrier as every committed prose and code source, authored and generated alike, excluding version-control internals, build and dependency directories, archived and vendored trees, and uncommitted generated artifacts under ADR-T-014, A calculus of documentation and source labels. Under this repository's `git-tracked` answer, version-control internals and uncommitted generated artifacts are outside the universe by definition, while its build and dependency directories are untracked and therefore absent. Tracked archived and vendored material remains in the partition but belongs to registered owners with no policy or profile activations. This reconciles the carrier honestly with the total partition: the partition accounts every surviving universe path, while the calculus carrier is the subset owned by participants, so an owner with no activated pair contributes no carrier source. No label-policy document or schema is introduced.

*The vendored owner.* A tracked archived or vendored tree is registered and partitioned as an owner, then made nonparticipating by activating no policy or profile pair for that owner; absence is non-applicability exactly as it is for claim-wave closure, with no carve-out and no ignore row. The exact current split is flagged for the owner: register `SUEXEC`; replace the repository owner's broad contribution-tree row with `^contrib/dev-tools/(?:container|init)(?:/.*)?$` and `^contrib/dev-tools/su-exec/AUDIT\.md$`; and give `SUEXEC` the disjoint row `^contrib/dev-tools/su-exec/(?:LICENSE|Makefile|README\.md|su-exec\.c)$`. The exact audit row keeps the repository-authored audit with the repository owner, the vendored row reaches only upstream material, and a new unmatched entry fails partition totality until its ownership is decided.

*Owner exclusion after the split.* The existing `vcs-metadata` row in `.linter/owners.toml` has no surviving consumer: the partition exclusion pass is its only machinery, but the `git-tracked` universe never presents version-control internals for that pass to remove. Folding the row into global ignore would preserve the same idle rule, while retaining it in the owner surface would preserve a pre-attribution filter whose owner field does no scoping. The flagged recommendation is therefore that the row dies as vacuous at cut-over and this repository's global-ignore list remains empty.

*Label participation.* Fenced displays, scanned prose regions, code comments, generated regions, and pre-calculus nonparticipation are code-owned recognition or existing adoption data, not file selection under ADR-T-014, A calculus of documentation and source labels. The calculus is parametric in exactly the owner signature, owner partition, profile signature, reserved kinds, typed-data classes that cite synthetically, documents maintaining citation indexes, and scanned-region recognition under the same record. None is a carrier selector. Census scopes bound tolerated debt and do not redefine that corpus, so there is no corpus-agreement relation for the resolver to validate: independently declared label carriers do not exist.

*Census scopes.* Each census-versus-register instance carries its include and exclude scope in its own policy parameter document. The scope bounds the occurrences that instance censuses from the post-ignore corpus universe; it neither restores a globally ignored path nor defines the label carrier. The five reference-policy files shown above gain their `[scope]` tables, and every other content policy with repository-specific selection keeps that selection in its own parameter document, as the SPDX and interchange policies already do. Cross-instance overlap remains visible because resolution compares the union of all decoded scopes.

*Assemblies.* The `assembly.publications-current` program consumes publication rows of owner, parts, and target, so `.linter/policy-assembly-publications.toml` owns exactly those rows under the allocated schema label ``com.torrust.index.linter.policy-assembly-publications``. Generated-target nonparticipation and the at-most-one-generator invariant are derived at resolution from the decoded rows rather than declared again.

```toml
namespace = "com.torrust.index.linter.policy-assembly-publications"
version = [1, 0, 0]
policy = "assembly.publications-current"

[[parameters.publications]]
owner = "ALPHA"
parts = ""
target = ""
```

*View destinations.* Each census-family parameter document names its own register destination in `[register]`. The register-equals-census gate and authored-shell writer consume that value, while resolution over all policy documents proves destination uniqueness and exclusion from the instance's own scope under ADR-T-020, The migration disciplines. No second pointer or generated preamble exists.

*Common test.* None of these parameter values carries a count, verdict, or evidence. A value is declared only when one named policy consumes information that is neither derivable nor already declared elsewhere; absence refuses an applicable policy rather than reconstructing today's repository. Inclusion rows already attribute paths, manifests identify built packages, the label record fixes its carrier, generated assembly targets derive from publication rows, and the repository-wide dependency target follows from the singleton graph-reconciliation activation.

The owner ratified physical directory enumeration as snapshot membership. The resolver first requires the fixed core documents, including `.linter/shape.toml` if the flagged refusal recommendation is accepted, then enumerates every remaining regular file directly in `.linter/`; each must have a `policy-NAME.toml` name and an allocated envelope, and every such entry is loaded before content decoding. An unknown extra entry, a duplicate key, an activated parameterized key without exactly one document, or a document with no matching key refuses the snapshot. Because configuration never checks git, an untracked policy document is a member and a deleted-but-tracked document is absent; required absence refuses under the loader rule.

The repository-wide graph-reconciliation policy must be activated by exactly one owner, and fixed-owner dependency templates resolve to that unique activation; no designated owner or compiled owner constant participates. Claim-wave closure needs no new field: activation of `profile.claims-conform` for an owner is the declaration that the policy binds there, so the compiled closed-owner set disappears. Assembly targets become generated and nonparticipating by derivation, and each census instance's policy-owned scope and destination replace compiled burn roots, carrier defaults, compiled scan exclusions, and register locations.

## Uniform declaration grammar · `sec:isolation:grammar`

**Grammar (One envelope, named sets, and three owner selection shapes carry every declaration)** · `gram:isolation:declaration`

An audit of the declared surface found five schema dialects living side by side, and the owner's review of all ten policy documents closed on 2026-08-26 with a single grammar ratified for the whole configuration surface. That grammar is stated here and governs the rewrite wave. The per-policy envelopes sketched earlier in this plan — their `policy` and `family` keys, their `[parameters]` tables, their `[scope]` and `[register]` blocks — are pre-grammar drafts kept only for the program semantics they describe; where a draft and this grammar disagree, this grammar governs, and the rewrite wave converts every document to the form below.

*Envelope and identity.* Every declared document carries a dotted `namespace` and a `version`, and the namespace is the document's whole identity. The `policy` key and the `family` key die everywhere. Two instances merge into one document exactly when the same policy program acts on their sets: one program, one document, one namespace, several named entries. Different program behaviour is a different namespace and therefore a different document. The three prefix-number payloads consequently merge into one document whose named entries are the work-package enumeration, the chapter leading range, and the record leading set, because one program reads all three. The marked-ordinal instance and the literal-set instance keep their own namespaces because their programs differ. The merge also repairs a live collision, since the three prefix-number documents today stamp one namespace between them while standing as three separate files.

*Sets.* A document states its data in `[set.TYPE]` tables — singular `set`, a form the owner accepted explicitly — keyed by the type of entry the consuming program reads. Every entry is named. Its value is a string when the entry is simple and an inline table when the entry is parameterized. The ratified vocabulary of this repository's documents is `[set.numbered-marks]` for a mark with an inclusive numeric bound, `[set.literals]` for verbatim strings, `[set.prefix-numbers]` for a prefix with its enumeration or range, `[set.name-key]` for the manifest key a name is read from, and `[set.name-prefix-ignore]` for a literal prefix stripped before comparison. Program-owned lists leave configuration entirely: the interchange document's `types` list is compiled meaning, not repository data, and does not survive as a set.

```toml
namespace = "com.torrust.index.linter.policy.references.scenarios"
version = [1, 0, 0]

[set.numbered-marks]
hash-one-to-91 = { mark = "#", minimum = 1, maximum = 91 }
```

*Owner sections and three selection shapes.* Repository choices attach to owners, and exactly three shapes are lawful. The first is pattern rows: an `[owners.OWNER.TYPE]` table carrying `exclude` and `include` arrays of `{ name, pattern }` rows, where an include row's name references a set entry by name and an exclude row's name labels the exclusion itself. This is the shape established by the licence-header precedent, and it is the shape to use whenever entries apply across files. The second is the singular reference: where exactly one entry of a type applies to an owner, no list is written and the table carries a `use` key naming the entry, a key the owner accepted by name. The third is owner-bound data on the naked `[owners.OWNER]` table, which is lawful exactly for content that has no set behind it; the owner ruled that the manifest table collapses this way, so the index owner's manifest declaration becomes a `cargo-toml` key sitting directly on its naked owner table rather than a `[owners.INDEX.manifest]` subtable.

```toml
[set.prefix-numbers]
work-packages = { prefix = "WP-", exact = [] }
chapters = { prefix = "L-", leading-minimum = 1, leading-maximum = 30 }
records = { prefix = "L-", leading = [] }

[owners.EXAMPLE.prefix-numbers]
include = [
  { name = "work-packages", pattern = "\"handbook\" [ \"/\" *VCHAR ]" },
]
exclude = [
  { name = "linter-package", pattern = "\"packages/linter\" [ \"/\" *VCHAR ]" },
]
```

*Amendment: a row's name is its own region's, and a reference is spelled under the set's own type.* The owner ruled on the row shape after the paragraph above was written, and the ruling is recorded here rather than written over it. The paragraph says that an include row's name references a set entry by name while an exclude row's name labels the exclusion itself, which made the name mean two different things depending on which array it stood in — and left a report naming a row unable to say what it had named. Under the ruling every row's name is the name of the region that row claims, in both arrays and in the owner document's partition rows alike; a row admitting sources carries its reference to a set entry as a separate field spelled under the set's own type, so the two declarations a row makes are two fields rather than one field read two ways. The example above stands superseded on this point by the one beneath it, whose include row carries its name and its set-typed reference as separate keys.

*Amendment: the uniqueness law, confirmed and extended.* The owner confirmed the law on 2026-08-27 and extended its reach, and it is stated here in full because it is the half of the row shape a reader is most likely to get wrong. Row names are unique under `partitions`, under `exclude`, and — this is the extension — under the shape document's `ignore`. They are deliberately not unique under `include`, where the regions of one deployment overlap by construction and a repeated name is one true thing said twice. An entry reference is unique under either admitting word, because where one entry reaches is one question. Regions are never held apart at all: it is lawful and ordinary for a path to match several named excludes or several named ignore rows, since the excluded set is a union and a union does not care how many of its members claimed a given path. What uniqueness buys is that a report naming a row names exactly one region — that, and nothing about overlap.

A derived consequence of the third shape, flagged here as derived rather than ratified word for word, is that assembly publications need no set either: each publication becomes a named inline table on the naked owner table, and the document's namespace supplies the typing that the retired `owner` field used to carry.

```toml
[owners.EXAMPLE]
guide = { parts = "handbook/guide", target = "handbook/guide.md" }
```

*Activation.* The activation document activates a policy program for an owner, as owner and program rows exactly as it does today; the instance document then clarifies and parameterizes what that activation means for that owner. Non-application for one owner is expressed by omitting the row and by nothing else. The crate-name program is therefore simply not activated for the manifestless owner, and the `unbuilt` machinery of its old parameter document dies with the omission rather than surviving as a declared exception list.

## Pattern language · `sec:isolation:patterns`

**Language (One pattern schema spans the whole configuration surface)** · `lang:isolation:patterns`

Every pattern in every declared document is written in the augmented Backus–Naur form of RFC 5234, and no second pattern schema exists anywhere in the configuration surface. The conversion is total: the owner document's inclusion patterns, every include and exclude row of every owner section, and the shape document's ignore rows all convert. The checker gains an augmented-Backus–Naur-form matcher, and regular expressions leave the declared surface completely rather than surviving as a second accepted dialect.

The grammar has no capture semantics, and the designs above are built so that none is needed. This is why the crate-name derivation decomposes into two named sets rather than one pattern with a capturing group: the manifest key to read is one set entry, the literal prefix to strip before comparison is another, and each is a plain value the program applies rather than a fragment a match hands back. Any future design that reaches for a capture is a signal to decompose it the same way.

## The declared owner-name document · `sec:isolation:owner-names`

**Schema (Crate-name conformance declares a key, a stripped prefix, and each owner's manifest)** · `schema:isolation:owner-names`

The owner ratified this document in full, and it replaces the crate-name parameter document that (`sec:isolation:owner-crate-names`) sketched. Its namespace is ``com.torrust.index.linter.policy.owner.names``. It carries two sets: `[set.name-key]` naming the manifest key that holds a package name, and `[set.name-prefix-ignore]` naming the literal prefix stripped before the owner spelling is compared. Each participating crate owner then takes a naked owner table carrying the path of the manifest that governs it, plus one singular-reference table per set.

```toml
namespace = "com.torrust.index.linter.policy.owner.names"
version = [1, 0, 0]

[set.name-key]
cargo-package-name = "name"

[set.name-prefix-ignore]
torrust = "torrust-"

[owners.INDEX]
cargo-toml = "Cargo.toml"

[owners.INDEX.name-key]
use = "cargo-package-name"

[owners.INDEX.name-prefix-ignore]
use = "torrust"
```

Two owners are absent from the document: the manifestless owner and the vendored owner. Their absence is non-application under the activation rule and not a declared exception. The manifestless owner is re-includable later at no schema cost, because a governing manifest is read from the path its own naked owner table declares; whether the workspace builds that member has stopped being a fact the program needs, so the concept of an unbuilt entry disappears from the schema rather than moving.

## Registers and ratchet lists · `sec:isolation:registers`

**Convention (Retired registers become records or lists, and a ratchet key follows the namespace)** · `conv:isolation:registers`

The generated burn-register files retire outright rather than moving into the new grammar. Each register's content has one of two destinations. Content that is an argument becomes a decision record of the linter's own record series — the division-name prose is exactly such a case — and it is cited by label, never by file path, because a file-path citation in prose is itself a finding. Content that is a ratchet lives in the canonical list document. No configuration key points at a register file, so the `[register]` blocks of the earlier drafts have no successor.

Two more constructs die with them. The `[register]` block and the global `[scope]` block both disappear: owner sections plus the ownership bounds already say which paths an instance reaches, so the six-subdirectory scope enumerations that every census document repeats today become unnecessary rather than being restated in the new grammar. And the `excluded` key of the owner document dies, because that concept is the shape document's `ignore` relation and declaring it twice is what let two dialects drift apart.

A ratchet key in the list document is the policy namespace followed by the exact set entry the ratchet belongs to, so a key names a program and a datum rather than a file and a family. The drafted default leads that key with the owner component, matching how allowance tables are keyed today.

```toml
[ASSAYER."com.torrust.index.linter.policy.references.divisions"."information"]
allowances = []
```

Owner-first is ratified and no longer merely drafted. The owner leads the key, the policy namespace follows, and the entry that names the datum comes last, so a list key is owner, then policy namespace, then policy-set entry — the block above is that shape exactly. The ratification is recorded in (`reg:isolation:owner-questions`).

## Resolution into one plan · `sec:isolation:resolution`

**Proposal (Atomic resolution precedes every analyzer and writer)** · `proposal:isolation:one-plan`

Every command begins with the same resolver. It receives the caller-supplied root and returns either a runnable `ExecutionPlan`, a parsed-but-unrunnable configuration report, or a refusal. No analyzer participates in resolution, and no analyzer or writer reopens a declaration after it.

| Resolution phase | Input | Output or failure |
| --- | --- | --- |
| Envelope | Fixed core documents read physically as written, then every remaining regular file enumerated directly from `.linter/` | The shape document and every policy document identify their allocated schemas before content decoding; an unknown entry or malformed or missing envelope refuses first under ADR-T-023, Adopting the interchange conventions for first-party structured configuration |
| Content | Closed decoders and the compiled policy-program catalogue | Typed declarations, including the one corpus universe answer and global-ignore relation; unknown keys, malformed values, unknown programs, duplicate keys, or codec mismatch refuse |
| Union | Fixed core declarations and all decoded policy documents together | One universe kind, one global-ignore relation, closed owners, environment kinds, policy keys, activations, lists, parameters, per-instance scopes and destinations, publication rows, and exactly one graph-reconciliation activation; any unresolved, unpaired, absent, or duplicate datum refuses |
| Topology | The base corpus universe and global-ignore relation declared once in `.linter/shape.toml`, plus the generic manifest reader | The post-ignore universe, total owner attribution including nonparticipating owners, the calculus-defined carrier projected from activated owners, discovered crate members, resolved per-policy source sets, and publication containment; a structural disagreement that prevents those typed inputs from forming is a configuration finding and yields no runnable plan |
| Dependency | Imported-citation form harvested from the resolved participating label sources | Instantiated dependency edges, fixed-owner templates resolved to the unique graph-reconciliation activation, and a topological policy schedule; a missing prerequisite is a configuration finding and yields no runnable plan |
| Assembly | The validated relations and compiled programs | One immutable plan plus typed subplans handed to analyzers and writers |

Resolution validates cross-instance facts over the union of all policy documents, never through a privileged file. Prefix-number domains may not overlap over an intersecting source bound; register destinations are unique and excluded from their own instance's scan; no publication target has more than one generator; and the graph-reconciliation policy has exactly one activation. These checks run only after every envelope and parameter value is decoded, so document order cannot change their result.

The topology phase materializes the base universe declared in `.linter/shape.toml` and removes the global-ignore union exactly once. With this repository's `git-tracked` answer and recommended empty ignore list, the partition accounts the entire finite tracked set, including archived and vendored owners, while each policy-owned scope, profile census, label carrier, header and envelope conformance, and publication containment projects only the owners activating that relation. An owner with no activated pair remains accounted but contributes no policy finding, profile debt, or carrier source. No corpus policy walks the physical checkout or reads an untracked file, so an untracked draft creates neither a finding nor debt. A corpus choosing `as-written` would start from the physical set and apply its declared ignore union before the same partition and activation projections, without changing the loader's independently fixed physical behavior. A globally ignored path likewise reaches no partition row, policy, profile, or carrier. No corpus-agreement validation exists: the label carrier comes from the Minting judgment and activation, while census scopes are independent bounds on debt within participating owners. Citation harvesting at dependency resolution reads only enough participating label syntax to instantiate code-owned dependency templates; it emits no label verdict. This preserves the two-pass order required by ADR-T-014, A calculus of documentation and source labels, before policy analysis.

The immutable plan contains the declared universe kind, global-ignore rows and resolved ignored set, complete owner partition and reach graph, participating-owner set, calculus-defined label corpus, discovered workspace members, declared unbuilt entries, profile inputs, decoded publication rows, derived generated-target set, activated policy runs with typed parameters, selected sources, allowances, and register destinations, and a dependency schedule. The `owners.crate-names-conform` subplan receives the participating roster and its two member sources without requiring their agreement first. Each generic analyzer accepts only the root plus its typed subplan and returns observations in the policy's structured identity. The common comparison layer applies the run's codec and allowances, attributes findings to its owner and full policy key, and feeds reports and control operations.

Configuration failures divide exactly as the command contract requires. A defect knowable from declaration bytes is a refusal: no report, analyzer, or writer runs. A parsed declaration that cannot be related to its declared corpus universe well enough to form a typed plan is a configuration finding: the command may report that disagreement, but no policy analyzer runs and every writer refuses. A relation that an activated program exists to judge remains runnable policy input instead; disagreement between the workspace and the participating roster therefore produces `owners.crate-names-conform` findings rather than a configuration finding. Only a closed runnable plan produces policy findings. This ordering replaces the current construction of compiled adoption, carrier, assembly, and burn state before configuration validation.

The old repository constructors have no compatibility role after cut-over. Adoption is built from declared owners and code-owned participation; label analysis receives the derived calculus corpus; other content readers receive their policy-owned source sets; assembly verification receives decoded publication rows; and burn and control receive resolved policy runs and destinations. `check`, `burn`, `assemble`, `project`, reporting commands, and every mutation mode all receive the same plan. Missing policy parameters after cut-over refuse the applicable key, never request compiled defaults.

## Audit coverage · `sec:isolation:audit-coverage`

**Inventory (Every audit class has one repair destination)** · `reg:isolation:audit-coverage`

The audit remains the evidence register; this table is only the disposition of its production rows.

| Production knowledge | Disposition |
| --- | --- |
| Shared sibling CLI dependency and import | Retain under the explicit sibling-CLI exception; add no adapter and infer no wider sibling reach |
| Project crate-prefix derivation, the hand-added manifestless member, and the two compiled repository-wide owner spellings | Delete the repository constructor and constants; move the hand-added crate-name and directory pair to `.linter/policy-owner-crate-names.toml`; make generic derivation and bidirectional roster reconciliation among participating owners the `owners.crate-names-conform` program; derive file attribution from the complete partition; and resolve repository-wide dependencies through the graph-reconciliation policy's singleton activation |
| Compiled migrated documents and directories | Move each include and exclude bound into the parameter document of the census policy that consumes it; the generic legacy analyzer receives that resolved instance source set |
| Compiled claim-wave closure | Delete; presence of the owner's `profile.claims-conform` activation is closure and absence is non-applicability |
| Compiled assembly adoption and its concrete policy identity | Move publication rows to `.linter/policy-assembly-publications.toml`, use generic `assembly.publications-current`, and derive generated targets and generator uniqueness from the decoded union |
| Burn roots, exclusions, family catalogue, register locations, and generated label stems | Replace with activated `PolicyRun` values whose own documents carry source bounds and register destinations, plus canonical lists; preserve authored register shells rather than generating sibling preambles |
| Carrier roots, package discovery, root files, named-file reach, and traversal exclusions | Materialize the universe and global-ignore relation declared once in `.linter/shape.toml`; partition every survivor, including archived and vendored owners; derive the label carrier from the Minting judgment and owner activations; and let every other content policy receive only the participating selection declared in its own parameter document |
| The check path constructing compiled catalogues before validating configuration | Replace with the resolver of (`proposal:isolation:one-plan`); no analyzer runs without a closed plan |
| Exported checkout locator | Delete; commands and fixtures receive their root from the caller |
| Retired-scenario, division, work-package, chapter, and record payloads | Move into the five parameter documents of (`sec:isolation:policy-instances`); leave only three generalized programs in code |

Reality-copying fixtures move by semantic cluster rather than by search-and-replace. The new fixture world is fictional and is always constructed through the same snapshot and plan resolver production uses.

| Fixture roles from the audit | Repair |
| --- | --- |
| Adoption, control, dependency, layer, partition, pattern, snapshot, and cross-owner integration | Use invented owners, prefixes, manifests, scopes, paths, a single universe answer, and global-ignore rows; include a partitioned owner with no activations and assert the same graph, non-applicability, refusal, overlap, and ratchet properties through an `ExecutionPlan` |
| Carrier and matrix derivation | Exercise invented `git-tracked` and `as-written` universes, empty and nonempty global-ignore relations, activated and nonactivated owners, policy-owned source bounds, and package shapes; prove that physical configuration membership is independent of corpus selection and that partitioned nonparticipants enter no carrier, and let no live package home or current tree convention appear |
| Burn exclusion and assembly integration | Declare fictional census scopes and destinations in census documents and fictional publication rows in the publications document; exercise independent lists, union validation, and writer idempotence through typed subplans |
| SPDX, constant, comment-leader, and test-index attribution | Replace current attribution, owner vocabulary, and label words with fictional values while preserving syntax and placement |
| Retired recognizer | Use an invented corpus identity and parameters supplied by the policy fixture rather than a current repository-like negative example |

Documentation follows the same ownership line.

| Documentation aggregate | Disposition |
| --- | --- |
| The mixed configuration design and namespace survey | Retire under the documentation restructure. Live schema rules move to attacked package records, allocations remain in root authority, unfinished implementation remains in this plan, and census evidence drops |
| The package readme's root licence link | Retain unchanged under the narrow legal-packaging exception |
| Source-test and integration-test matrices | Rewrite around generic configured behavior and fixture tests; the root CI invocation is described as a pipeline gate, not as package test coverage |
| Outline and legacy module examples | Replace sibling campaign language with fictional or corpus-neutral examples |
| Binary command commentary about another program | State this command's contract only; the sanctioned shared library dependency does not sanction knowledge of another executable's behavior |

## Package self-hosting removal · `sec:isolation:self-hosting`

**Proposal (Live-repository tests have a mechanical disposition)** · `proposal:isolation:test-disposition`

“Moved to CI” means consolidated into the single root `check` invocation, not copied into another test harness. “Fixture test” means the claim survives against an invented root and declared plan. “Dropped” means the claim is repository magnitude, duplicated conformance, or behavior removed with its helper.

| Audited test | Disposition | Mechanical replacement |
| --- | --- | --- |
| `the_repository_prose_is_in_good_standing` | Moved to CI | Root `check` is the whole-tree good-standing assertion |
| `the_repository_seeds_one_mint_for_every_covered_test` | Fixture test | Invented covered assets prove one derived mint each; scale assertions disappear |
| `a_document_citing_a_live_test_resolves_and_a_dead_one_does_not` | Fixture test | An invented owner and derived test label exercise live, dead, imported, and unwarranted forms |
| `the_assayer_backlog_participates_and_validates` | Dropped | Carrier ownership and document validity are already covered by the root run; no package-specific document is a behavior fixture |
| `the_repository_graph_report_is_sane` | Fixture test | A small declared graph proves totals, orphan accounting, dangling detection, and hub ordering |
| `the_assembly_of_the_specification_is_live_and_fresh` | Moved to CI | The generic assembly run inside root `check` verifies every declared publication |
| `the_repository_shape_report_is_sane` | Dropped | Current percentile populations and outlier existence are descriptive evidence; generic resolver fixtures retain the arithmetic without an omnibus deployment schema |
| `the_repository_census_covers_every_test_function` | Fixture test | Invented manifests populate each supported test area and prove complete classification without magnitude pins |
| `the_repository_derivation_is_injective` | Fixture test | Paired collision and non-collision fixtures prove injectivity directly |
| `the_repository_check_is_clean` | Moved to CI | Root `check` owns whole-tree cleanliness and profile conformance |
| `the_repository_declared_surface_loads_and_partitions` | Moved to CI | Root `check` validates physical directory-derived snapshot membership, the once-declared universe and global-ignore relation, the total owner partition, activation-based participation, the policy-document union, dependencies, and lists before policies run |
| `the_adopted_records_mint_and_cite` | Dropped | Mint and citation magnitude is not conformance; resolution and non-vacuity remain in fixtures |
| `every_edge_of_the_reference_graph_lands_on_a_mint` | Fixture test | A synthetic graph with real analysis output proves both endpoints exist |
| `the_repository_burn_registers_are_the_census_as_found` | Moved to CI | Every resolved policy run compares its observed path counts with the canonical list during root `check` |
| `the_burn_command_emits_json_and_writes_nothing_by_default` | Fixture test | A temporary declared plan proves report shape and no mutation; root register equality is covered by CI |
| `every_mark_in_the_corpus_parses_to_one_reading` | Fixture test | One explicit fixture per reading and boundary replaces the physical-tree sweep |
| `derives_the_prefix_table_of_the_decision_record` | Dropped | Prefix derivation and its transcribed table are removed; declared owner identity is validated instead |
| `reads_the_workspace_members_of_this_repository` | Fixture test | Invented root, built member, declared unbuilt entry, and registered non-crate owner prove generic discovery and bidirectional reconciliation through the singleton `owners.crate-names-conform` run without pulling the non-crate owner into its participating roster |
| `derives_the_reach_register_of_the_decision_record` | Moved to CI | Root `check` reconciles declared reach with generic manifest edges |
| `resolves_the_repository_root_above_the_package` | Dropped | The checkout-topology helper and its assertion are deleted together |

The fixed-vector prefix test that does not itself open the live checkout is rewritten with invented crate names when the derivation disappears. Root-record-only edition and registry checks remain under the explicitly sanctioned root-authority reach; they are not a corpus acceptance suite and do not inspect sibling topology.

## Ordered migration bites · `sec:isolation:migration`

**Proposal (Each bite is committable and has a local gate)** · `proposal:isolation:migration-bites`

Each row is one reviewable state. The order is dependency order; the bite names, not their positions, are their identity.

| Bite | Work | Local green gate |
| --- | --- | --- |
| Authority prerequisite | Amend root configuration and migration authority for family-bearing policy keys; the new schema allocations are already recorded following owner approval | Root label checks resolve; existing configuration still loads byte-for-byte |
| General policy programs | Extract owner-name reconciliation, generic `assembly.publications-current`, and mark-numbered, literal-set, and prefix-number scanners over typed parameters; keep temporary wrappers supplying the old payloads, publication, and unbuilt entry | Invented built, unbuilt, missing, unexplained, collision, publication, boundary, display, offset, overlap, and codec fixtures pass; existing behavior is unchanged |
| Plan schema in shadow | Add the closed shape decoder, global-ignore resolver, `PolicyKey`, policy-parameter decoders, physical declared-directory enumeration, union resolver types, and refusal diagnostics without routing commands through them | Complete and malformed fixture snapshots prove envelope-first physical loading, both universe answers, empty and nonempty ignore relations, ignored-set precedence, the flagged fixed-core absence refusal, exact directory membership, union closure, and cross-instance validation; current command reports are unchanged |
| Declared repository data | Add `.linter/shape.toml`, `.linter/policy-owner-crate-names.toml`, `.linter/policy-assembly-publications.toml`, and the reference-policy files; register the vendored owner with the flagged disjoint split and no activations; remove the vacuous owner exclusion if ratified; retain one crate-name activation and extend the other activations and lists by owner and full policy key; and snapshot the universe answer, empty global-ignore relation, unbuilt rows, publication rows, per-instance scopes, destinations, and debt from the live recognizers | The physically directory-derived snapshot parses atomically, the post-ignore universe is partitioned totally, the vendored owner enters no policy or profile input, every policy parameter has one declared home, each observed occurrence is represented exactly once, and ordinary checker findings do not grow |
| Shadow equivalence | Build a declared plan beside the compiled route and compare the universe kind, ignored set, normalized complete owner partition, participating-owner set, calculus corpus, per-policy selections, publications, generated targets, register destinations, parameters, and allowances | The comparison is empty over every compiled family present at the cut-over base, including families added after the audit |
| Atomic execution cut-over | Resolve configuration before analysis; pass one plan to every check, reporting, assembly, burn, control, and writing path; delete compiled adoption, carrier, assembly, burn, payload, fixed-owner, owner-exclusion, pending-member, and claim-closure data in the same commit | Package fixtures pass; a missing or divergent snapshot cannot run a policy or writer; the last shadow comparison is empty before its bridge is removed |
| CI transfer and self-host removal | Land the root `check` pipeline gate, then delete the audited live-tree tests, checkout locator, and package-root assumptions | Package tests pass in a standalone fixture checkout; the root pipeline command passes against the real declarations |
| Fixture neutralization | Convert every audit fixture cluster through the production plan builder and replace current identities and attribution with fictional data | An isolation scan finds no current sibling identity or topology in package fixtures outside the sanctioned root-authority reach and the settled concrete exceptions |
| Documentation closure | Retire the mixed design and namespace census, rewrite test matrices and source examples, retain the licence exception, and reconcile the documentation-restructure ruling ledger; the mixed design and the namespace census are retired, and the restructure proposal that planned this split retires with them now that the package records carry the argument, so the test-matrix and source-example rewrites are what remain | Package documentation has one genre per document, no current census, no stale test row, and no new checker finding |
| Bridge deletion | Remove temporary comparison code, legacy wrappers, compatibility constructors, and stale exports after all callers use the plan | No package symbol can construct current repository state; the declared snapshot and generic fixtures are the only execution inputs |

The data bite is intentionally one snapshot commit: the fixed shape document must arrive with its universe answer and ignore relation, the vendored owner and disjoint contribution-tree split must arrive together so the partition remains total and exclusive, and physical directory enumeration sees each new parameter file immediately, so adding a parameter document without its activation, list, scope, destination where applicable, and envelope would make the intermediate snapshot unquotable. The cut-over bite is likewise atomic because retaining any compiled fallback would make absence ambiguous. Neither constraint makes the surrounding bites larger.

The first five rows of this table landed as they were drawn, and they landed over the declared surface as it was spelled before the grammar review closed. The five rows after them did not survive that review unchanged. The grammar ratified in (`gram:isolation:declaration`) changes what a document is, and changing what a document is changes what there is to cut over to, so a row drawn against the retiring spelling would have been executed against a surface that no longer exists. The closing rows are therefore superseded rather than edited in place, and the table stays readable as the decision it was. (`proposal:isolation:staging`) states the staging that replaces them, and (`reg:isolation:ladder-disposition`) says row by row what became of each.

## The staging that replaces the closing rows · `sec:isolation:staging`

**Proposal (Four bites carry the ratified grammar from staged copies to the only surface)** · `proposal:isolation:staging`

Two of the landed bites have to be paid for twice. The data bite authored this repository's declared surface and the equivalence bite proved that surface says what the compiled route says, and both were done in the dialects the grammar review then closed. The ratified grammar is not an edit to those documents: it changes the envelope, merges documents by program, replaces positional lists with named entries, replaces the pattern language entire, and re-keys the canonical list. What stands in `.linter/` today is therefore a surface to be replaced rather than a surface to be amended.

The replacement is authored as staged copies first, because the live route reads the committed documents on every command and a half-converted directory is a directory no command can run against. The staged copies stand in `.linter-staged/`, a directory at the repository root holding exactly what `.linter/` will hold. Three properties of the design decide that placement rather than a preference: snapshot membership is the physical enumeration of `.linter/` and of nothing else, so a sibling directory is invisible to it and needs no exemption; the join takes decoded declarations rather than a directory, so a staged document is resolvable wherever it stands; and the cut-over then moves files rather than rewriting them, which is what lets a reviewer read the cut-over commit as a switch. A staging subdirectory inside `.linter/` was the obvious alternative and it is refused on the loader's own behaviour rather than on taste: membership refuses every entry that is neither a fixed core name nor a `policy-NAME.toml` name, and a directory is an entry. The one cost is that a staged directory is tracked content while the owner partition is total, so the staged directory arrives with the inclusion row that accounts it and that row leaves with it.

Each bite below is one reviewable commit with exactly one thing that can fail it. The names are the identity, as they are in the ladder above.

| Bite | Work | The one thing that fails it |
| --- | --- | --- |
| Staged surface | Author the whole declared surface in the ratified grammar under `.linter-staged/`: the ten committed policy documents as the eight instance documents root authority has allocated; the owner document with its `excluded` key gone and its inclusion, exclusion, and may-cite rows converted; the activation document with the family key the namespace absorbs gone; the shape document with its ignore relation converted; and the canonical list re-keyed owner, then policy namespace, then set entry. Add the inclusion row that accounts the staged directory, and move the join's reading of the three surface documents off the committed dialect and onto the staged spelling | A staged document the decoder or the join refuses |
| Staged equivalence | Point the comparison at the resolved staged surface and compare every relation it already names — universe kind, ignored set, normalized owner partition, participating-owner set, calculus corpus, per-instance selections, publications, generated targets, parameters, and allowances. Retire the one relation that is asserted rather than equated, together with the test that pins it | A relation the two routes read differently |
| Atomic cut-over | One commit. The live route stops reading five files as one snapshot and reads physical membership, then envelopes, then the ratified decoder, then the join; `.linter-staged/` becomes `.linter/` and its inclusion row goes; the environment document wires into the kind registry; the retired burn registers delete; the pattern surface becomes the augmented Backus–Naur form end to end; and the compiled adoption, carrier, assembly, burn, payload, fixed-owner, owner-exclusion, pending-member, and claim-closure data go, taking the comparison bridge with them | A command that ran before the commit and refuses after it |
| Shadow retirement | Delete the pre-grammar shadow the cut-over left without callers: the policy-key module entire, the loader's pre-grammar content decoding and the exports that carried it, the refusal variants naming spellings no document can now be written in, the regular-expression matcher, and every remaining wrapper and compatibility constructor | A module named for deletion that a live caller still reaches |

*The eight instance documents.* Ten policy documents stand in `.linter/` today and eight instances succeed them, because two payloads merge into one document exactly when one program acts on both and the three prefix-number payloads are one program's (`gram:isolation:declaration`). Their namespaces are the ones root authority has allocated: ``com.torrust.index.linter.policy.spdx``, ``com.torrust.index.linter.policy.interchange``, ``com.torrust.index.linter.policy.assembly-publications``, ``com.torrust.index.linter.policy.owner.names``, ``com.torrust.index.linter.policy.references.divisions``, ``com.torrust.index.linter.policy.references.scenarios``, ``com.torrust.index.linter.policy.references.prefix-numbers``, and ``com.torrust.index.linter.policy.references.path-linking``. The last of them is the file-path reference policy's, and it is the one allocated name this plan's earlier sections never spelled: those sections generalize the five payload families they are about, and the file-path policy has no payload to generalize. Under the ratified grammar its whole declaration is an exclude-only owner section over a type no set declares, which decodes because an exclude row's name labels the exclusion itself and promises nothing about a set. Its confirmation is recorded in (`reg:isolation:owner-questions`).

The eight are what the cut-over landed and are no longer the surface. A ninth document stood beside them from the cut-over itself, stamping the owner-areas namespace, and it retired on 2026-08-27 when the owner ruled that a scan surface belongs to the domain of the policy that scans; seven documents were allocated and written on that day, one for each censused family whose policy had no document of its own. Fifteen policy documents stand in `.linter/` now. The paragraph above is left as the bite it specified, and the membership follows ADR-T-023, Adopting the interchange conventions for first-party structured configuration, rather than any count in this plan.

*One carve-out dissolves rather than moving.* The landed comparison holds the three prefix-number families to a pending state instead of to equality, because three declared instances could not share the one register the compiled route shares, and which three destinations replaced the one was an owner act rather than a transcription. The registers convention answered it by retiring registers outright: no configuration key points at a register file, the `[register]` block has no successor, and there is therefore no destination relation left for the three instances to be pending about (`conv:isolation:registers`). The carve-out retires with the block, and all three families are compared for equality like every other. A comparison that carries a pending state after the question is closed is measuring the calendar rather than the corpus.

*Three things the cut-over does that the superseded row could not name.* The first is the loader it switches to, which did not exist when that row was drawn: after the commit the live route reads membership, envelopes, the ratified decoder, and the join, in that fixed order, and the five-file snapshot reading has no caller. The second is the environment document, which wires into the kind registry here. Until it does, the fourteen rows the title campaign declared have no reader, which is exactly why (`reg:isolation:owner-questions`) makes that campaign's renames wait for this commit instead of mirroring the rows into the compiled extension set. The third is the deletion of the retired burn registers: their ratchets already stand in the canonical list, the division prose already stands as a record of the linter's own series, and the register-equals-census gate becomes a list-equals-census gate over the same numbers, so the files leave owing nothing.

*Two things the retirement bite deliberately does not reach, and both were found in the code rather than in this plan.* The corpus-shape module is not pre-grammar shadow: the ratified join reads its universe answer, its ignore relation, and the corpus they resolve to, so it survives the cut-over entire and only the language of its rows changes. And the path module is half-retired rather than retired, because two languages meet in it: the regular-expression matcher loses its last reader when the pattern surface converts, while the reversible byte-path display standing beside it is how a path value is written and reported under any pattern language and outlives every matcher. An earlier reading of this staging had both modules retiring with the shadow, and it was wrong about both.

*The bridge dies at the cut-over and not after it.* It is the one module written to be deleted, and it is deleted one bite earlier than the pre-grammar ladder placed it, for a structural reason rather than a tidy one: a comparison has two arguments and the cut-over deletes one of them. Its last run is the cut-over's own precondition — an empty comparison is what makes the switch a transcription rather than a guess — and the commit that empties its right-hand side is the commit it cannot survive. What the superseded bridge-deletion row still had to say about wrappers, compatibility constructors, and stale exports is real work, and it is the retirement bite's.

*The pre-grammar dialect refusals retire with their producers and not before.* A refusal naming a `policy` key, a `family` key, an `excluded` key, a `[register]` block, or a global `[scope]` block is a refusal about a spelling, and a spelling no document can be written in is a spelling nothing will present. Those variants therefore go in the retirement bite rather than in the cut-over, because until the committed documents are gone the refusals still have live producers, and a refusal deleted while its producer stands is a defect the loader would accept in silence.

## The pre-grammar ladder's closing rows · `sec:isolation:ladder-disposition`

**Register (Every closing row is retained, folded, or superseded by name)** · `reg:isolation:ladder-disposition`

The ladder of (`proposal:isolation:migration-bites`) stands as the decision it was, and this register says what became of each of its closing rows once the grammar was ratified. No row is rewritten; a row is retained, folded, or superseded, and a superseded row's successor is named here rather than left to be inferred.

| Closing row | Disposition | What carries it now |
| --- | --- | --- |
| Atomic execution cut-over | Superseded | The cut-over bite of (`proposal:isolation:staging`), which names the loader it switches to, the environment document's wiring, the burn-register deletion, and the pattern conversion — four things the superseded row could not name because none of them existed when it was drawn |
| CI transfer and self-host removal | Retained as drawn, and it clears one known-red row | The row stands unchanged: the root pipeline gate lands, then the audited live-tree tests, the checkout locator, and the package-root assumptions go. It also clears the live-tree test that compares authored prose mints against derived test mints, sentenced to fixture conversion by (`proposal:isolation:test-disposition`) and standing red until this bite executes |
| Fixture neutralization | Retained, with its standard widened | Every fixture still goes through the production plan builder with fictional identities, and the standard now also fails a fixture written in a retired dialect: a fictional corpus spelled in a grammar no corpus can use is not a fixture of anything the checker does |
| Documentation closure | Retained unchanged | The mixed design and the namespace census retire, the test matrices and source examples are rewritten, the licence exception stays, and the restructure ruling ledger is reconciled. This plan is not among the documents that retire: unfinished implementation is what it holds |
| Bridge deletion | Superseded | Its comparison half is the cut-over bite's, which deletes one of the comparison's two arguments; its wrapper, compatibility-constructor, and stale-export half is the retirement bite's |

The three retained rows keep their order among themselves and follow the retirement bite. Each of them is a statement about a package that has already stopped carrying compiled repository knowledge, and running any of them earlier would be asserting a property of a package that the shadow modules are still contradicting.

## Owner questions · `sec:isolation:questions`

**Register (Shape refusal and owner dispositions are ratified)** · `reg:isolation:owner-questions`

The allocations of ``com.torrust.index.linter.shape``, ``com.torrust.index.linter.policy-owner-crate-names``, ``com.torrust.index.linter.policy-assembly-publications``, ``com.torrust.index.linter.policy-mark-numbered-references``, ``com.torrust.index.linter.policy-literal-set-references``, and ``com.torrust.index.linter.policy-prefix-numbers`` are now recorded in root authority under ADR-T-023, Adopting the interchange conventions for first-party structured configuration. The global-ignore relation rides in the allocated shape schema and adds no allocation. The three prefix-number documents share the last label because namespace identifies their common schema, while their family keys identify independent policy instances; adding scope and register fields to the census schemas creates no further identity. The calculus-defined label corpus creates neither a parameter document nor an allocation.

The flagged refusal choice is ratified: absence of `.linter/shape.toml` refuses, because the universe and global-ignore relation cannot otherwise be resolved without a fallback. The owner dispositions are ratified exactly as they were flagged: `SUEXEC` is registered with the exact disjoint split stated above and no activated pairs, the repository-authored audit stays with the repository owner, the vacuous `vcs-metadata` owner exclusion dies rather than moving into the global-ignore relation, and a new unmatched vendored entry is a partition finding until its ownership is decided. The two shape answers are ratified with them: this repository declares the `git-tracked` universe and an empty ignore list. Family-bearing policy keys are the necessary encoding of the settled requirement that each payload family be an independent generalized policy; the sibling CLI dependency, self-hosting removal, census removal, licence exception, recognizer generalization, physical configuration loading, and physical snapshot membership are not reconsidered here.

The configuration-grammar review is ratified in full, on 2026-08-26. The owner read all ten policy documents against an audit that had found five schema dialects living side by side, and closed the review by declaring it finished. The uniform grammar recorded in (`gram:isolation:declaration`), (`lang:isolation:patterns`), (`schema:isolation:owner-names`), and (`conv:isolation:registers`) is that review's ratified outcome, and it is authoritative over every document envelope this plan sketched before it.

The ratified rulings are these. A dotted namespace and a version are the whole envelope, and the namespace alone is the instance identity, so the `policy` and `family` keys die in every document. Instances merge into one document exactly when one policy program acts on their sets, which merges the three prefix-number payloads into a single document of three named entries while leaving the marked-ordinal and literal-set instances separate — and which also resolves the live collision of three files sharing one namespace today. Data is declared in singular `[set.TYPE]` tables of named entries, string-valued when simple and inline-table-valued when parameterized. Repository selection attaches to owners in exactly three shapes: pattern rows of `{ name, pattern }` under `[owners.OWNER.TYPE]`, a singular `use` key naming one entry where exactly one applies, and owner-bound data on the naked `[owners.OWNER]` table for content with no set behind it. Every pattern in the surface is augmented Backus–Naur form and nothing else, so regular expressions leave configuration and designs decompose rather than capture. Activation stays owner and program rows in the activation document, and per-owner non-application is the omission of a row. The crate-name document is ratified entry by entry, and the burn registers, the `[register]` blocks, the global `[scope]` blocks, and the owner document's `excluded` key all retire.

Two consequences reach rulings already recorded above. Family-bearing policy keys were the earlier encoding of independent payload families; under the ratified grammar that role passes to the namespace and its named set entries, so the encoding described in (`schema:isolation:policy-key`) is superseded while the requirement it served — that each payload family be an independently activated, independently ratcheted instance — stands unchanged. The two questions the data bite had left open are closed by the same grammar: the crate-name derivation's stripped prefix has its home in a named `[set.name-prefix-ignore]` entry rather than a bare `namespace` key, and the three prefix-number register destinations are moot because registers retire outright instead of multiplying.

The last detail of the grammar is ratified too, on 2026-08-26, and owner-first is the ratified answer. The owner stated the shape as owner, then policy namespace, then detail, so a ratchet key in the canonical list document is owner, then policy namespace, then policy-set entry, and it reads ``[ASSAYER."com.torrust.index.linter.policy.references.divisions"."information"]``. The alternative of a bare namespace-and-entry key with ownership derived from the partition is refused, so a key carries its owner rather than recovering one from the partition, and allowance tables and ratchet lists are keyed alike.

*Amendment: the middle component is relative to the corpus prefix.* The owner refined the key on 2026-08-26, after the shape above was ratified and before the cut-over executed it, and the refinement is recorded here rather than written over the paragraph it amends. The three components stand exactly as ratified — owner, then policy namespace, then policy-set entry — and what changes is how much of the namespace a key spells: the corpus prefix every document of a surface shares is implied by the list document's own envelope and is never written in a key, so the ratified example above stands superseded by ``[ASSAYER."policy.references.scenarios"."hash-one-to-91"]``. The prefix a key is relative to is that document's namespace less its final component, which is why the shorter form is generic rather than a convenience for this repository: a corpus whose documents open with another prefix keys its lists the same way and the reader needs no compiled answer. A key that spelled the prefix would repeat, once per row, the one thing the envelope has already said.

Three further questions are ratified on 2026-08-26, and the first is the one this register had left open. No question flagged in this plan is open after them.

*The title campaign's renames wait for the cut-over.* The fourteen new environment rows do not ride the compiled local extensions. The mirror alternative is refused by name rather than merely not taken: copying the rows into the acceptee's compiled extension set would buy the campaign an earlier green with a duplicate that a later commit had to delete again, and for as long as the duplicate stood the compiled set and the declared document would be two authorities over one relation, with nothing in the checker able to say which of them had drifted. The renames therefore wait for the cut-over bite of (`proposal:isolation:staging`) to wire the declared environment document into the kind registry, which is the moment the declared rows first have a reader. The campaign's completion is coupled to this plan's schedule, and that coupling is the accepted price of the refusal.

*No citation edge is opened from the root corpus to this package.* The adoption record's flag is answered, and the answer changes nothing: the reach graph stays exactly as it stands, the root record goes on naming the ratified grammar in non-participating display spans, and no may-cite row is added. Reach follows what a corpus imports under ADR-T-019, The layer owner graph; the root corpus does not import this package, and opening a standing licence for root authority to cite a package's plan in order to relieve one document of a display span would pay for a phrasing convenience with a permanent edge. Under ADR-T-014, A calculus of documentation and source labels, a display span exhibits a token rather than referencing it, so the record's present form is the right one rather than a workaround waiting for an edge to arrive.

*The file-path policy's namespace is confirmed.* ``com.torrust.index.linter.policy.references.path-linking`` is the ratified name of the file-path reference policy's declaration schema, and it is now spelled in this plan wherever the policy family is enumerated (`proposal:isolation:staging`). It was the one allocated name this plan's own text had never spelled, and the reason is worth recording rather than repairing silently: the sections above generalize the five payload families they are about, and this policy has no payload to generalize. Its whole declaration under the ratified grammar is an exclude-only owner section — which sources each owner exempts from it and nothing else — so it passed through every payload argument in this plan without ever being the subject of one.

*Amendment: the two repository-wide burn families are registered rather than retired.* The owner ruled on 2026-08-26, after the staged surface stood and before the cut-over executed, and the ruling is recorded here rather than written over the paragraphs it amends. The cut-over's burn-register deletion was drawn against ten registers whose ratchets were said to stand already in the canonical list (`proposal:isolation:staging`), and eight of them do. Two do not. The section-sign census over the whole repository and the record-number census over the whole repository are declared over the corpus root, and the compiled route recorded each of them in a generated register and in nothing else: no activation in either dialect, no ratchet in either list, no declaration in any staged document. Deleting those two registers as drawn would have deleted the only declaration of three thousand four hundred and thirty-six pinned occurrences, and every gate would have stayed green while it happened. The ruling is that they are registered: the canonical list gains an activation and a ratchet table for each, and the cut-over's deletion becomes lawful for all ten because all ten then owe nothing.

The registration follows the shape the eight already have. Each family takes an activation row of owner and program in the activation document, and a ratchet table in the list document keyed owner, then program — the program-level key rather than the three-component one, because these two are programs with no instance document and no set entry for a third component to name, which is the narrow reading the key ratification already carries. Their owner is the root owner, and for the reason the graph reconciliation's is: a census declared over the corpus root reaches every share at once, so the ratchet it earns is one repository-wide artifact that no member can repair alone under ADR-T-019, The layer owner graph. Two consequences follow from that alone and are recorded so no implementation has to rediscover them. A row of such a table names a file the root owner does not own, so owner containment — which holds every ordinary list row to the attribution of its own path — is asked only of a policy that divides the partition, and a corpus-wide census is exempt by the same argument that filed it under the root owner. And the observation a maintenance mode takes for such a pair is the whole census rather than the owner's share of it, because a pair that could only see its own owner's files would ratchet a fraction of what its family counts. Whether a family is corpus-wide is read off its declared surfaces rather than declared beside them: a surface naming the corpus root is the whole of the test.

The staged equivalence bite had a blind spot here and it is closed rather than noted. Its ratchet comparison related each staged key to the committed pair it re-keys, and these two families have no committed pair, so their absence from both surfaces was invisible to a comparison that only ever looked at keys both surfaces had. The comparison now holds a registered corpus-wide key to the family's own census, row by row, which is the relation the register-equals-census gate held its register to and the relation the list-equals-census gate inherits at the cut-over. The rows themselves were generated through the append mode of the shipped burn command rather than transcribed, so the bytes in the list are the codec's own.

*A fixture may spell the schema namespaces this package implements.* The owner ruled on 2026-08-27 that a policy is referenced by its namespace, so a fixture stamping a name under ``com.torrust.index.linter`` on an invented corpus is naming the program it exercises and nothing else. The distinction the boundary requirement already draws is the whole of the ruling: a namespace of this family is the identity of a schema this package implements, which is self-knowledge and sits on the may-know side of (`req:isolation:boundary`), while a sibling identity, a repository path, an owner roster, and a current count are repository knowledge and sit on the other. The asymmetry is not a concession to convenience: a fictional corpus must declare its policies in some vocabulary, the only vocabulary the shipped decoder accepts is the allocated one, and a fixture that invented a private family in order to look neutral would be a fixture of a decoder this package does not ship — the same defect the widened standard already names in a fixture written in a retired dialect (`reg:isolation:ladder-disposition`).

The consequence for the fixture-neutralization bite is that this ground is settled and owes it nothing (`proposal:isolation:migration-bites`). The standard treats a fixture's use of the checker's own schema identity as conforming, an isolation scan reads such a spelling as generic rather than as copied reality, and no fixture is rewritten to shed a namespace it needs in order to decode. What the standard still asks of those fixtures is unchanged and is asked elsewhere in each of them: fictional identities, invented topology, and a current dialect.

## Implementation gate · `sec:isolation:gate`

**Gate (Isolation repair is complete only when absence cannot recreate the repository)** · `gate:isolation:implementation`

The repair is complete only when all of the following hold:

- `.linter/shape.toml` is an enveloped fixed core document containing exactly the universe key and global-ignore relation, a third shape question or relation requires an owner act, every remaining regular file in `.linter/` is an enveloped `policy-NAME.toml` document, all references between declarations close, and allocated schema labels agree with their recorded owner act;
- configuration is always read physically and byte-for-byte as written without consulting git, physical directory-derived snapshot membership is exact, an untracked policy document is a member, and a deleted-but-tracked required document is absent and refuses;
- the base corpus universe and global-ignore relation are resolved once from `.linter/shape.toml` before any policy runs, this repository declares `git-tracked` and an empty ignore list, the partition accounts the post-ignore universe while policy, profile, and carrier projections restrict it by activation, no corpus policy reads an untracked file, and a refusal or configuration finding reaches no analyzer or writer;
- every analyzer and writer accepts an `ExecutionPlan` or typed subplan, and no package API constructs repository adoption, policy selection, publication rows, burn deployment, or workspace exceptions from compiled data;
- the five reference payload families and every unbuilt participating-owner entry exist only as declared parameter values, while the package contains only generalized policy programs and fictional parameter fixtures;
- each family-bearing policy key has its own activation, policy-owned source scope, allowance table, diagnostics, control identity, and register destination where it is a census family;
- the partition accounts every post-ignore path, `SUEXEC` owns exactly the flagged vendored rows with no policy or profile activation, the repository owner retains the exact audit row, and the vacuous owner exclusion is removed if the flagged recommendation is ratified;
- the label corpus is exactly the activated-owner subset admitted by the Minting judgment from the post-ignore tracked universe: version-control internals and uncommitted generated artifacts are outside that universe, build and dependency directories are untracked, archived and vendored owners have no activations, participation remains code-owned or existing adoption data, and no labels carrier parameter or corpus-agreement validation exists;
- intentional manifest absence comes only from `owners.crate-names-conform` parameters; publication rows come only from `.linter/policy-assembly-publications.toml`; generated targets and their nonparticipation derive once; and the participating roster and workspace agree in both directions under the policy;
- resolution over the complete policy-document union proves prefix-number domain non-overlap, register-destination uniqueness and self-scan exclusion, at-most-one-generator, and the singleton graph-reconciliation activation before any analyzer or writer runs;
- no package test locates or inspects the live checkout, package integration runs from invented snapshots covering each universe kind, empty and nonempty global-ignore relations, physical configuration membership, total partitioning, and nonactivated owners, and the root CI gate invokes the shipped command without creating an acceptance suite;
- the mixed design census and namespace census are retired, reality-copying fixtures and examples are neutralized, and test indexes describe the resulting fixture suite;
- the shared sibling CLI dependency and root licence link are the only concrete exceptions and remain as narrow as (`req:isolation:boundary`) states;
- removing or corrupting `.linter/shape.toml` or an applicable policy parameter document refuses the command rather than selecting a fallback; and
- the pinned checker reports no failures, no warnings, and no finding added by this plan.
