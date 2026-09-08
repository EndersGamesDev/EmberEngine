# Julibrot implementation notes

This directory holds component-level design, audit and refactor records for the Julibrot app, kernels, math, presentation and worker crates, including precision and tiled-reprojection contracts.

The Julibrot lab crates and their reviewers depend on these pages for decisions that are more focused than the lab overview.

Begin with [`../julibrot-lab.md`](../julibrot-lab.md), follow repository-wide rules in [`../../CLAUDE.md`](../../CLAUDE.md), and update the matching component page when an implementation boundary changes.

The [`navigation calculation inventory`](navigation-calculations.md) traces pointer edits through the stored view and every presentation conversion, with precision, operation count, cost, and the `ember-camera` adoption boundary.
