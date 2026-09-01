# Division Names · `rec:divisions:division-names`

**Status:** Decided

## Context · `sec:divisions:context`

The Assayer's largest test document once grouped every behavioural promise the package made under twelve headings, each written as an indicative sentence rather than as a noun. That document retired into six homes, and the sentences that headed its divisions became a migration family: a bounded, literal set the checker censuses so that a name outliving the document it named is a finding rather than a habit.

The argument for treating those twelve sentences as one countable family — why the family is the whole sentence, why it needs a recognizer of its own, why the numerals beside the names are somebody else's business, and what its zero would mean — was written into the preamble of the generated register that carried the census. That was the only place it existed, and a generated register is the wrong owner for an argument.

Under (`conv:isolation:registers`) the generated register files retire outright rather than moving into the new configuration grammar, and each register's content divides: an argument becomes a decision record of this series, cited by label rather than by path, and a ratchet becomes rows in the canonical list document. This record is where the division-name argument lands. The register that held it is decided for retirement and its census leaves with it; nothing below depends on that file still existing, and nothing below was true only while it did.

The prose is the Assayer's subject and the enforcement is this package's, which is why it is placed here rather than in the sibling's own record series. The convention names this series in as many words, and two mechanical facts agree with it. A record kept where the census reaches would itself be counted by the very family it retires, so the argument would have to buy its own allowance row to be written down. And a record in the sibling's series could not cite (`conv:isolation:registers`), because reach runs the other way; here every citation this record needs resolves.

## Decision · `sec:divisions:decision`

**Definition (A division name is the whole sentence)** · `def:divisions:whole-sentence`

A member of the family is one of twelve sentences, matched entire and verbatim, not a pattern over them. The names were written as indicative sentences about the system rather than as nouns, and no rule shorter than the sentence separates such a name from ordinary prose on the same subject: a corpus about a risk engine says that stale state decays and that the fast path is fast for reasons that have nothing to do with a retired heading.

Matching the sentence entire is also what makes the family safe to count. Each of the twelve is a thing somebody wrote about one particular document, so a literal occurrence is a reference to a division and never a coincidence — which a two-word token drawn out of the same sentence would not have been. This is the local specialization of ADR-L-020, The migration disciplines, bounded by enumeration under that convention's second device; what is local is the twelve sentences, which are the retired document's own.

**Inventory (The twelve divisions and what each name claimed)** · `reg:divisions:twelve-names`

Each row is one member of the family in its exact wording, beside the stake the retired document wrote under it — what would be lost if the promises it grouped stopped holding. The stakes are recorded because they are the part of a division that no census recovers: the sentences survive as data in a policy declaration, but a declared value says nothing about why the grouping was worth making.

| Division name | What was lost if it failed |
| --- | --- |
| ``Information flows strictly forward`` | The measurement layer is corrupted by the interpretation layer, and every risk estimate becomes suspect. |
| ``The core model and the decision layer are independent`` | The boundary between the model, the derivation function, and the companion trackers is a fiction; policy could corrupt the model and the system could not be factored. |
| ``Structure can change without destroying what was learned`` | Deployments cannot evolve: adding a source poisons the model and removing one loses what it contributed to the survivors. |
| ``Stale state dissolves on a bounded schedule`` | The contamination loop wins — a transient adverse event permanently poisons a reputation, which sustains restriction, which starves the evidence that would clear it. |
| ``Learning converges despite censoring and class imbalance`` | Biased learning, posterior collapse, confounded estimates, or a cold start that never ends. |
| ``The anchor provides coverage when the sister can't`` | Cold starts are blind, regime changes are invisible, and lifecycle transients answer confidently and wrongly. |
| ``The decision landscape faithfully reflects model belief`` | The host's decision surface stops matching what the model believes: crossovers are misplaced, shapes are wrong, or hidden state leaks in. |
| ``Calibration and companion trackers stay current`` | Calibrated probabilities drift from reality, action crossovers sit in the wrong place, and stale auxiliary state produces stale decisions. |
| ``The fast path stays fast`` | Production traffic backs up, because the assessment call waits on operations it should never wait on. |
| ``The system knows when it's struggling`` | Silent degradation: the system is confidently wrong and nobody is told. |
| ``None of the above breaks under weird inputs`` | Panics, not-a-number, or silent wrong answers on configurations the specification calls valid. |
| ``Recovery preserves state and failures stay explicit`` | Operational failures become silent model corruption, lost labels, or stuck traffic, and the host cannot tell retryable backpressure from accepted durable work. |

The names appear here in code font, as displayed literals rather than as references, which is how the family's own surface rule reads them and how this record avoids being a use of what it retires.

**Decision (The enforceable set is declared, and its ratchet is a list key)** · `dec:divisions:declared-set`

The twelve sentences are policy data. They live as the parameter values of one literal-set instance in the declaring owner's policy document, and that declaration is the single enforceable statement of what the family contains; this record is its argument and never a second copy to be kept in step. A disagreement between the two is resolved by editing the declaration, because the checker reads only the declaration.

The ratchet the census used to carry lives in the canonical list document, keyed owner first, then the policy namespace, then the set entry the allowance belongs to — a key that names a program and a datum rather than a file and a family.

```toml
[OWNER."com.example.policy.references.divisions"."information"]
allowances = []
```

No configuration key points at a register file. The destination block that made a generated document part of the policy's declaration has no successor, and neither does the per-instance scope enumeration: an owner's declared bounds already say which paths the instance reaches. Growth in an allowance still fails and shrinkage still lowers a row, exactly as ADR-L-020, The migration disciplines, requires; what changes is where the row lives, not what it promises.

**Decision (The numerals beside the names belong to another family)** · `dec:divisions:numerals-elsewhere`

The roman-numeral locators that stood beside these sentences are no part of this family. They are section references, they are counted by the family that counts section references, and a second list counting them here would register one occurrence twice. A recognizer that swallowed the numeral with the sentence would also make the count depend on whether a writer happened to quote the heading with its number.

For the same reason the family keeps a recognizer of its own rather than being folded into a general legacy rule: no other rule judges a division name, so this family is the only counter of the shape and cannot come to disagree with a second one (`dec:references:declare-then-drain`).

**Decision (A division retires into the areas its claims wanted)** · `dec:divisions:areas-replace`

Retiring a division name is not a renaming. A claim's area names what a statement is about, the census counts the areas actually in use, and the owner's area register states what each area is for and what is lost when its claims fail, per ADR-L-017, The test documentation policy. Six of the twelve divisions were mixtures by the campaign's own study — two announcing it with "and" in the name, and one defined as the residue of the others — so a division is replaced by the areas its claims turn out to want, which is usually more than one.

The order matters and is the order fixed by ADR-L-017, The test documentation policy: the vocabulary was censused after the statements were written rather than chosen before them. An area set fixed in advance would have been a taxonomy that had met no sentence, which is what the twelve divisions were.

**Gate (What zero means)** · `gate:divisions:zero`

The family reaches zero when nothing in the corpus groups tests under a heading that no document supplies. Zero is therefore a statement about the corpus and not about the register: it is reached by the last replacement, not by the removal of the file that counted, and the retirement decided in (`conv:isolation:registers`) neither reaches it nor forgives it. A name reintroduced after the file is gone is still a finding, because the declaration and its list key are what the checker consults.

## Consequences · `sec:divisions:consequences`

The twelve sentences keep a home that says what they meant, and the reader who meets one in an old comment can learn both what it grouped and what replaced it without opening a generated census. The enforceable set becomes ordinary declared policy data, so the family loses its bespoke document, its destination key, and its scope enumeration, and gains nothing the other literal-set families do not already have.

The cost is that the stakes above are a historical record kept by hand. They describe a document that no longer exists and they will not be recomputed; the living statement of what an area is for belongs to the owner's area register, and a reader who wants today's grouping should go there rather than reason forward from these twelve. Should the declaration ever be widened or narrowed, this record is amended by the same change, because nothing in the checker will notice if it is not.
