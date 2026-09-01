# The linter knows rules, not the repository · `rep:isolation:repository-knowledge-boundary-audit`

This report audits the isolation boundary of `packages/linter`. It is an
audit only: it changes no implementation, test, fixture, existing document,
configuration, or register. The observations are against base commit
`099fc2ee66b97856987115c66579c7677884d4e4`.

The report mints no identity. Direct references to the repository's root
decision records are sanctioned by the owner's ruling and are not findings.
Everything else is classified below by where the repository fact resides and
how it should reach the linter instead.

## The declared-surface boundary · `sec:isolation:audit-declared-surface-boundary`

The linter may know the schema and generic meaning of an explicitly supported
configuration, parameter file, or repository-owned register, but it may not
embed concrete sibling identities, paths, package topology, family membership,
adoption state, exclusions, or register locations. A concrete path supplied by
repository configuration such as `.linter/owners.toml` is repository knowledge
passed into the linter and is sanctioned. The same concrete fact encoded in
linter source, live-repository tests, reality-copying fixtures, defaults, or
package documentation is linter-package knowledge and belongs in the audit.
Direct outside references are otherwise limited to root ADR records.

That distinction is about provenance, not spelling. A sibling path in a parsed
repository-owned row is an input. The same path in a Rust constant, expected
test vector, fallback default, prose example, or package matrix is a finding.
The linter may implement a generic codec and the meaning of its fields; the
repository must provide the current rows.

Generic Rust and Cargo conventions, supported policy syntax, and wholly
invented fixture names are not concrete repository facts. They remain inside
the package. Concrete bounds and literal vocabularies copied from a sibling
document are not made generic merely by placing them behind a policy name.

## Counting method and summary arithmetic · `sec:isolation:audit-counting-method-and-summary`

The classes use different units because their isolation costs differ:

- production sites are named constants, table rows, dependency edges, or one
  cohesive default-wiring boundary;
- live-test sites are test functions that locate or inspect this checkout;
- fixture sites are one reality-copying semantic cluster in one file; and
- documentation sites are one file-and-reference-class aggregate. The anchor
  count in the documentation table measures matching lines, not token repeats.

The classes are disjoint for the arithmetic. In particular, the fixed expected
workspace vector belongs to its live test and is not counted again as a fixture.
Root-record-only tests are sanctioned and are not live-project findings.

| Class | Sites |
| --- | ---: |
| Hardcoded package or project knowledge in production code | 38 |
| Tests coupled to the live repository | 0 |
| Reality-copying fixture clusters | 15 |
| Documentation file-and-class aggregates | 12 |
| **Total findings for ruling** | **65** |

The documentation census assigns a line once within a file, in this priority:
concrete sibling identity or path, other non-ADR project artifact or path, then
remaining non-self project identity. Generic `.linter` schema references,
package-local paths, fictional examples, and root ADR paths are excluded.

## Hardcoded package or project knowledge in production code · `sec:isolation:audit-production-repository-knowledge`

| Finding | Sites | Decisive evidence | Replacement direction | Cost |
| --- | ---: | --- | --- | --- |
| The binary depends on sibling CLI code | 2 | `Cargo.toml:14` names a path dependency on `../index-cli-common`; `src/bin/linter.rs:76` imports it. The manifest and import are two independently load-bearing sites. | Give this package its own CLI adapter, or depend on a non-workspace generic library whose contract is not another package's project role. | needs-design |
| Workspace owner derivation knows the project's crate prefix | 1 | `src/workspace.rs:56` fixes `CRATE_NAME_PREFIX` to the ember workspace spelling. | Read the owner-to-package relation from the declared surface; a generic manifest reader need not know the project's naming prefix. | needs-design |
| Workspace discovery carries a hand-added sibling | 1 | `src/workspace.rs:72` fixes the pending package name and `packages/notime` path because no manifest describes it. | Put manifestless owners and their roots in a repository-owned owner register and merge that parsed input with manifest discovery. | mechanical |
| The root owner is compiled under two spellings | 2 | `src/adoption.rs:74` fixes the root area and `src/policy.rs:46` fixes the root prefix. | Declare which owner covers the repository root once, then derive both views from the parsed owner record. | needs-design |
| Migration participation is a compiled campaign ledger | 2 | The package portion of `MIGRATED_DOCUMENTS` at `src/adoption.rs:145-164` lists sibling documents; `MIGRATED_PREFIXES` at `src/adoption.rs:181` lists a sibling subtree. Root ADR rows in the first table are sanctioned; the package rows are not. | Move document and prefix adoption state, including the active rule set, to a repository-owned participation register. | needs-design |
| Claim enforcement embeds the closed authoring waves | 1 | `src/claim.rs:107-120` lists the exact packages whose claim wave has closed. | Make closure an owner-policy parameter or a repository-owned staging register. | needs-design |
| Assembly adoption is compiled twice | 2 | `src/assembly.rs:173` fixes the sibling parts root and publication; `src/policy.rs:297` fixes an Assayer-specific policy identity for the same adoption. | Declare assemblies and their policy activation in repository data; keep only generic assembly verification in code. | needs-design |
| Burn scopes encode the sibling and repository trees | 6 | `src/burn.rs:134`, `:150`, `:172`, `:202`, `:224`, and `:248` define Assayer prose, code, residual code, exclusions, wide prose, and repository code surfaces. | Put include and exclude roots in each repository-owned family declaration. The scanner should receive a resolved scope. | needs-design |
| The base burn catalogue compiles every adopted family and register | 8 | The eight declarations in `BURN_LISTS` at `src/burn.rs:278-350` bind codecs to concrete Assayer register paths and the compiled scopes above. | Parse family, codec, scope, exclusions, and register path from the declared surface; dispatch only on generic codec identity. | needs-design |
| Burn register rendering fixes a sibling label namespace | 1 | `src/burn.rs:1126-1127` emits Assayer-specific section and register labels for every generated register. | Obtain owner and label stem from the family declaration, or make the repository render the authored preamble around a generic generated region. | mechanical |
| Carrier discovery compiles the current repository layout | 7 | `src/carrier.rs:62`, `:76`, `:110`, `:124`, `:139`, `:152`, and `:166` fix excluded trees, root prose roots, root files, the package container, package prose roots, readme name, and readme search trees. | Make carrier inclusions, exclusions, package roots, and named-file selectors declared inputs. Retain only generic walking and path-safety behavior. | needs-design |
| The main check still executes compiled catalogues beside configuration | 1 | `src/report.rs:346-405` constructs compiled adoption and carrier state, then runs compiled assembly and burn catalogues; configuration is verified later rather than driving those passes. Related paths are `src/report.rs:308-322`, `:423-459`, and `src/control.rs:1244`. | Build one resolved execution plan from the declared surface and pass it to generic analyzers, verifiers, and writers. | needs-design |
| Retired-matrix recognizers compile sibling-document facts | 2 | `src/retired.rs:73-116` fixes the retired scenario bound and every literal division heading copied from the sibling matrix. The generic ability to scan a configured range or literal set is not the finding; these payloads are. | Put bounded ranges and literal sets in a repository-owned family parameter record and feed them to generic recognizers. | needs-design |
| Residual recognizers compile sibling-document enumerations | 2 | `src/residual.rs:128-178` fixes the retired plan's issued work-package set and the old specification and record locator bounds. The prefix and scanning mechanics can remain generic; the current payload cannot. | Parameterize the bounded number sets and ranges in the repository-owned family declaration. | needs-design |

The production count is the sum of the `Sites` column. Arrays are counted by
their load-bearing unit: each burn declaration is independently adopted, while
the values that together define one retired recognizer family count as one site.

*One row is struck.* The public-helper row recorded a repository locator whose root was the package manifest directory's grandparent, exported so that package tests could find this checkout. The helper and the module holding it were removed entire when the whole-tree corpus gate moved to continuous integration, and nothing in the package now derives a root from its own manifest position. The row is removed rather than marked repaired, on the same reasoning the fixture strike below records: a register row whose evidence no longer exists points a later reader at nothing to check. The production class therefore reads thirty-eight sites against the thirty-nine this audit found at its base.

## Tests coupled to the live repository · `sec:isolation:audit-live-repository-tests`

*The class is struck entire.* Four rows recorded the tests that located this checkout: sixteen integration sites that took the repository as their corpus, two workspace unit tests that derived the root record's prefix table and pinned the current members, one layer test that reconciled the recorded reach register against the live workspace, and the locator's own unit test proving the package-grandparent layout. Every one of them reached the tree through the public locator struck from the production class above, and all twenty went when the whole-tree corpus gate moved to continuous integration — seventeen removed outright, three recast onto temporary fixture roots. No test in this package now locates or inspects the checkout by any route, and the registry and edition tests this class deliberately excluded as sanctioned root-record readings stand in that same fixture form. The rows are removed rather than marked repaired, because a register row whose evidence no longer exists points a later reader at nothing to check; the commentary that glossed them goes with them, its subject being the same departed suite. The ruling this class asked for — whether the live self-hosting suite survived as an optional root acceptance suite or was replaced entirely by configuration-driven fixtures — was answered in both halves at once: package behaviour is now exercised against synthetic corpora, and whole-tree conformance is owned outside the package. The live class therefore reads no sites against the twenty this audit found at its base, and the total for ruling, eighty-six after the fixture strike recorded below, now reads sixty-five against the eighty-seven at base; the base figures stand in these paragraphs rather than in a table that would then describe neither state.

## Fixture-embedded repository knowledge · `sec:isolation:audit-repository-copying-fixtures`

| Fixture cluster | Sites | Decisive evidence | Replacement direction | Cost |
| --- | ---: | --- | --- | --- |
| Adoption fixtures mirror current owners and migration paths | 1 | `src/adoption.rs:443-706` uses the root, Assayer, Notime, real prefixes, and current migrated documents as its fixture world. | Build the same ownership and staged-migration cases with fictional owners and paths supplied to a configurable adoption constructor. | needs-design |
| Burn exclusion fixture mirrors the real register layout | 1 | `src/burn.rs:1415-1422` uses the linter subtree, Assayer burn-register subtree, and Assayer prose path. | Use fictional package and register roots while preserving the include/exclude relationship. | mechanical |
| Control fixtures mirror the Assayer burn configuration | 1 | `src/control.rs:1438-1964` repeatedly writes Assayer owners, policies, paths, and list rows. | Rename the fixture corpus and construct it through the same generic declared inputs the control path consumes. | mechanical |
| Dependency fixtures use the current owner graph | 1 | `src/depend.rs:252-426` uses Assayer, Mudlark, Sentinel, and the root owner to illustrate dependencies. | Replace them with fictional owners while preserving graph direction, missing prerequisites, and citation-derived edges. | mechanical |
| Layer fixtures import the real manifestless owner | 1 | `src/layers.rs:969-1148` mixes an invented graph with Notime, its real prefix, and its current missing-manifest condition. | Use an invented manifestless owner. Keep the surrounding alpha, beta, and gamma graph. | mechanical |
| Matrix derivation fixtures use Assayer's package shape | 1 | `src/matrix.rs:669-708` derives matrices from Assayer paths and area layout. | Use a fictional package with the same relative test-directory shapes. | mechanical |
| Partition fixtures copy the root and Assayer owners | 1 | `src/partition.rs:342`, `:561`, `:592`, and `:705-719` use their real owner names and sibling path. | Use fictional owners and roots; retain overlap, narrow-rule, and total-partition cases. | mechanical |
| SPDX fixtures copy the repository attribution | 1 | `src/spdx.rs:750-789`, `:949-957`, and `:1321-1329` use the current Wild Sky Maker attribution and an Assayer-owned path. | Use fictional copyright sets, owners, and paths. | mechanical |
| Constant fixtures copy an Assayer label vocabulary | 1 | `src/constant.rs:1745` and `:1847-1851` derive from Assayer words and a concrete Assayer constant label. | Use a fictional owner and label whose token transformations exercise the same rule. | mechanical |
| Snapshot fixtures copy current repository policy data | 1 | `src/snapshot.rs:1744-1804` and later assertions use the current Wild Sky Maker attribution and repository-shaped policy rows. | Use a complete fictional snapshot with no current owner, attribution, or package path. | mechanical |
| Cross-owner integration fixture uses the real root and Assayer | 1 | `tests/corpus.rs:316-350` builds its otherwise synthetic ownership boundary from the current owner pair and prefixes. | Use fictional owners and prefixes. | mechanical |
| Assembly integration fixture reproduces the Assayer publication | 1 | `tests/corpus.rs:608-856` fixes the real parts root, target, generated provenance text, and assembly counts. | Parameterize assembly adoption and exercise it with a fictional publication. | needs-design |
| Retired recognizer fixture names the root repository | 1 | `src/retired.rs:303` uses a current repository issue-like identity as a negative example. | Replace it with an invented corpus and identifier. | mechanical |
| Comment-leader fixture copies the current attribution | 1 | `src/leader.rs:314-320` uses the repository's current copyright line. | Use a fictional attribution with the same comment-leader shape. | mechanical |
| Test-index fixture copies the current attribution | 1 | `src/index.rs:787` places the repository's current copyright line above a synthetic index. | Use a fictional attribution while preserving header placement. | mechanical |

The invented `demo`, `alpha`, `beta`, `gamma`, `one`, `two`, and `fixture`
worlds are self-contained and are not findings. A fictional package tree may
look like a Cargo workspace; it becomes repository knowledge only when its
identity, topology, policy payload, or metadata copies the live project.

*One row is struck.* The pattern-fixture row recorded a cluster in `src/pattern.rs` that copied the sibling root into exact, subtree, searching, and malformed pattern cases. Those cases exercised the regular-expression matcher, and the matcher left the package when the declared pattern surface converted to the augmented Backus–Naur form entire, so the cluster went with its reader rather than being neutralized. The row is removed rather than marked repaired, because a register row whose evidence no longer exists points a later reader at nothing to check. The fixture class therefore reads fifteen sites against the sixteen this audit found at its base, and the total for ruling reads eighty-six against eighty-seven; the base figures stand in this paragraph rather than in a table that would then describe neither state.

## Documentation knowledge · `sec:isolation:audit-documentation-knowledge`

Documentation counts are line anchors within each aggregate. They are evidence
of scale, not a request to repair each sentence independently; the surrounding
argument moves with its external anchor.

| Document and reference class | Sites | Anchors | Decisive evidence | Replacement direction | Cost |
| --- | ---: | ---: | --- | --- | --- |
| `docs/config-surface-design.md`: sibling identities and paths | 1 | 727 | The design repeatedly names current owners, sibling packages, paths, assemblies, registers, policies, and examples drawn from them. | Move the repository census and migration plan to root-owned design material; keep package documentation about generic schema and behavior. | needs-owner-ruling |
| `docs/config-surface-design.md`: other current project artifacts and paths | 1 | 44 | Non-sibling anchors describe current root manifests, configuration, generated artifacts, machine paths, or repository layout outside root ADRs. | Replace generic examples with fictional paths and move current-state evidence to the root design. | needs-owner-ruling |
| `docs/config-surface-design.md`: remaining project identity facts | 1 | 6 | After sibling and path anchors are assigned, the document still carries current project attribution and namespace facts. | Put current allocations and attribution data in the root-owned census; retain only namespace and schema rules here. | needs-owner-ruling |
| `docs/namespace-survey.md`: sibling identities and paths | 1 | 4 | The survey names its sibling design and concrete sibling-owned configuration context. | Move the current-tree census to a root document; leave a generic namespace grammar note only if the package needs one. | needs-owner-ruling |
| `docs/namespace-survey.md`: other current project artifacts and paths | 1 | 7 | The survey cites current root files and current-tree configuration locations outside the sanctioned ADR set. | Let the root census own those locations. | needs-owner-ruling |
| `docs/namespace-survey.md`: remaining project identity facts | 1 | 16 | The tables and analysis carry current package, homepage, tracker, and allocated namespace identities. | Store allocations in repository configuration or root governance material, not package documentation. | needs-owner-ruling |
| `README.md`: root licence link | 1 | 1 | `README.md:373` reaches `../../LICENSE`. | Rule whether legal packaging is an exception; otherwise ship package-local licence material or rely on manifest metadata supplied by the workspace. | needs-owner-ruling |
| `src/README.md`: compiled-current unit matrix | 1 | 24 | Twenty-four rows document tests whose claims pin compiled adoption, burn, carrier, claim, assembly, policy, workspace, layer, or locator facts. | Rewrite or generate the matrix around generic configured behavior as those tests are replaced. | needs-design |
| `tests/README.md`: live-repository integration matrix | 1 | 16 | Sixteen rows describe the live tests catalogued above. The sanctioned root-registry row is excluded. | Move these rows with the root acceptance suite, or replace them with synthetic package-test rows. | needs-owner-ruling |
| `src/outline.rs`: Assayer example and path | 1 | 2 | Module prose at `src/outline.rs:5` and the fenced declaration at `:27` use the sibling campaign and its specification path to explain a generic table. | Use a fictional outline and document path. | mechanical |
| `src/legacy.rs`: sibling campaign commentary | 1 | 1 | `src/legacy.rs:78` explains generic legacy recognition through the Assayer campaign. | State the rule through the sanctioned root migration ADR or in corpus-neutral terms. | mechanical |
| `src/bin/linter.rs`: another program's exit behavior | 1 | 1 | The command documentation at `src/bin/linter.rs:112` knows the configuration probe's exit behavior. | Describe only this command's contract, or cite a sanctioned root command record that establishes the distinction. | mechanical |

The design document is intentionally untouched. A concurrent documentation
lane is planning its restructure; this audit records the base it must remove
from package ownership without competing with that work.

## Sanctioned outside references and non-findings · `sec:isolation:audit-sanctioned-outside-references`

| Class | Representative sites | Why it is sanctioned |
| --- | --- | --- |
| Direct root ADR records | `src/registry.rs:69`, `src/layers.rs:100,630`, `src/edition.rs:66,249`, `src/shape.rs:117-123`, the root rows in `src/adoption.rs:112-144`, and package prose links to root `adr/` | The owner expressly allows root ADR references. Embedding an edition and testing it against the same root record remains within that exception. |
| Repository facts arriving through configuration | The `.linter` snapshot read by `src/snapshot.rs`; owner rows consumed by `src/partition.rs`; policy activation consumed by `src/depend.rs`; package names and paths parsed generically from Cargo manifests | These values are supplied by the repository at runtime. The linter owns the schema, validation, and generic meaning, not the current rows. |
| Generic policy and language rules | Label parsing, comment and Markdown participation, generic manifest parsing, path safety, supported file leaders, generic Cargo source and test conventions, and codecs selected by configured policy identifiers | These are behavior of the linter rather than a census of the current project. The two concrete retired-family payload findings above are the boundary cases that must become parameters. |
| Package-local identity and behavior | The linter's own command name, report schema, package paths used only to test package-local resolution, and its own README description | A package may know itself. The finding begins when it imports a sibling fact or current repository topology. |
| Self-contained invented fixtures | Fictional demo and Greek-letter owners and trees throughout unit and integration tests | They demonstrate generic behavior without claiming that the live repository has those identities or relationships. |

The current root ADR includes and embedded editions are therefore not a reason
to generalize the exception to package records, sibling source, root manifests,
or live-tree census data.

## Repair direction and owner rulings · `sec:isolation:audit-repair-direction-and-rulings`

The smallest coherent repair is a resolved execution plan built entirely from
repository-owned declarations. It should carry owners and roots, carrier
selectors, participation state, staged profiles, assemblies, policy activation,
burn scopes and registers, and the bounded literal or numeric payloads of
repository-specific families. The package keeps parsers, validators, codecs,
generic scanners, reports, and mutation safeguards. No fallback may recreate
the present repository when a declaration is absent; absence must be an empty,
unsupported, or explicitly refused state according to the schema.

Package tests should build that plan from invented snapshots. Whole-repository
reconciliation should run from a root-owned acceptance harness that passes the
root and declarations into the package. Documentation should split on the same
line: package docs specify the generic surface; root docs own the live census,
migration history, and adoption plan.

The owner still needs to rule these migration choices:

- whether the live self-hosting suite survives as an optional root acceptance
  suite or is replaced entirely;
- how the sibling CLI dependency is removed while preserving the global output
  contract;
- where the current design and namespace census live after leaving this
  package;
- whether the root licence link receives a narrow legal-packaging exception;
  and
- whether repository-specific recognizer payloads join the existing declared
  surface or a separate repository-owned family parameter register.

The remaining `needs-design` and `mechanical` rows do not weaken the boundary;
their owner question is only sequencing after the principle is accepted.

## Concurrent growth and verification baseline · `sec:isolation:audit-concurrent-growth-and-baseline`

This audit reads the named base. The concurrent SCOPE lane is, by owner ruling,
adding two more hardcoded repository-wide families to the same compiled burn
wiring. That does not make the ruling wrong. It means the pattern is growing by
design until the isolation repair lands, and the resolved-plan repair should
absorb those families rather than special-case the base's eight.

The pinned oracle at the lane scratchpad, not the deployed local binary, checked
the clean base. `check` completed in 6.78 seconds with zero failures and zero
warnings. `burn` completed in 1.01 seconds; all eight base families reported
zero occurrences and zero failures. No compiler or Cargo command was used.
