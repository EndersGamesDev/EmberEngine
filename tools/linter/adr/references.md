# Path Citations · `rec:references:path-citations`

**Status:** Decided

## Context · `sec:references:context`

The label language already gives authored concepts stable, owner-scoped names under ADR-T-014, A calculus of documentation and source labels. This record closes the competing channel for references to tracked files while preserving the structural boundary between authored citation and carried data.

## Decision · `sec:references:decision`

**Decision (Labels are the inter-file citation instrument)** · `dec:references:labels-only`

An authored document or comment cites another tracked file only through its label. A concrete path or filename in a citation-bearing region is a defect even beside a valid label; the label is the reference, not an excuse for the duplicate locator. A source's full tracked name, its component-aligned suffixes, and its basename are lexical self-reference, including when module documentation names its own source, and are outside the prohibition.

**Rule (Structure bounds total reach)** · `rule:references:total-reach`

Every tracked document and every comment is in reach. The program owns a total carrier catalogue that classifies each tracked kind with a comment reader or as having no comments; an uncatalogued kind is a finding, never silent nonparticipation.

Structural role alone distinguishes data from citation. Register fields, generated projections, complete patterns and configuration values, schema-marked path columns, and display blocks are data; authored prose around them remains governed. This policy-specific reading changes none of the label calculus's participation judgments of ADR-T-014, A calculus of documentation and source labels.

**Definition (Recognition joins shape to membership)** · `def:references:recognition`

Recognition first admits a finite path-shaped candidate and then requires its spelling to name a member of the tracked corpus. Shape without membership invents hypothetical files; membership without shape turns every word shared with a tracked basename into a filename.

A mini-tokenizer reads a maximal run as a candidate followed by a tail of full stops. The candidate is the longest member-bearing prefix and the tail consumes the remainder, so membership rather than typography decides the boundary; a non-tail remainder refuses the run. More than one accepting segmentation contributes one occurrence naming no target rather than a guessed resolution.

**Decision (The carrier exception has one declaration shape)** · `dec:references:declarations`

README documents alone are a carrier exception: they are not scanned, while another carrier that names a README remains governed. Every owner section carries the identical suffix-shaped `readmes` exclusion row. Because each section is offered only its owner's share (`dec:rows:owner-input`), these copies are one rule rather than owner-specific variants.

`include` records are absent by default. When present they must satisfy the checked partition (`dec:rows:subtract-then-partition`), but they supply diagnostic gloss only: they neither admit carriers nor narrow total reach. A decoder that still rejects `include` under deny-unknown behavior predates this ruling and does not define it.

**Decision (The census declares before it drains)** · `dec:references:declare-then-drain`

The landed recognizer generates the complete per-source census before the verdict is routed (`dec:catalogue:routing-and-codec`); no table is transcribed from a probe. Each owner-policy list then ratchets under ADR-T-020, The migration disciplines: growth fails, and shrinkage lowers or removes its row through the writer.

Burn-down is slow and label-first. Following ADR-T-024, Document-title labels, the target mint lands before the resolving citation, the concrete locator leaves next, and the citing source's allowance lowers last. When no label states the intended claim, the claim is exposed or promoted before replacement; no label is invented from a filename.

## Consequences · `sec:references:consequences`

The policy is total over authored documents and comments yet finite over tracked names. Data remains usable as data, README links remain a declared exception, ambiguous spellings receive no invented target, unknown carriers fail loudly, and the declared census can only drain.
