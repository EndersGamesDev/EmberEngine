// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Owner reach, dependency topology, and imported-citation tests.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`derives_reach_from_every_dependency_table`] | layers | Reach is read from the manifests and from all three dependency tables — ordinary, development and build — so a dependency declared for tests warrants a citation exactly as one declared for the build does. A `path` key outside a dependency table, such as a library target's, declares no edge. |
//! | [`derives_an_inherited_workspace_path_dependency`] | layers | A member dependency inherited from the workspace derives the edge carried by the root workspace dependency's path. |
//! | [`derives_no_edge_from_an_inherited_dependency_without_a_path`] | layers | A member dependency inherited from a non-path workspace dependency derives no reach edge. |
//! | [`continues_to_derive_a_literal_member_path_dependency`] | layers | A literal path in a member dependency table continues to derive its edge without workspace inheritance. |
//! | [`reach_is_reflexive_and_not_transitive`] | layers | Reach is one declared hop: a corpus reaches itself, reaches what its manifest names, and reaches nothing two hops away. Closing the relation would license imports nobody wrote down. |
//! | [`admits_an_upward_import_from_any_member`] | layers | A member importing a root head is admitted whether or not its manifest names the root crate, because repo-wide policy must be citable by everyone it binds. |
//! | [`admits_an_import_along_a_declared_dependency_edge`] | layers | A package importing a head of a package its manifest depends on is admitted: the warrant is the declared edge, and the law enforces structure rather than denying it. |
//! | [`generated_imports_obey_reach`] | layers | A generated imported citation is judged by the same reach relation as an authored one: without the manifest edge the layer pass refuses the matrix row, and adding the edge repairs the fixture without changing the law. |
//! | [`a_generated_import_instantiates_its_cited_owner_prerequisite`] | depend | A generated imported citation enters the cited-owner relation and therefore instantiates the dependency pair of the owner it names, carrying the register row as the first requiring location. |
//! | [`refuses_a_sideways_import_across_no_edge`] | layers | A package importing a head of a package neither manifest names is refused, naming the citing location, the cited prefix, and the manifest edge whose absence refuses it. |
//! | [`refuses_a_package_import_in_the_root_corpus_prose`] | layers | Repo-wide policy imports nothing: the root corpus's prose carrying an import of a package prefix is refused even where the root's manifest reaches that package, because the rule is about the prose rather than about the crate. |
//! | [`admits_a_reached_package_import_in_the_root_corpus_code`] | layers | The narrow converse is narrow: the root crate's own commentary may import a package it declares a dependency on, because what the rule closes is policy resting on a private premise, not the crate's code resting on its dependencies. |
//! | [`refuses_an_import_of_a_registered_but_absent_owner`] | layers | A prefix registered against no workspace member is reachable by nobody: no manifest can declare a dependency on a crate the workspace does not build, so an import naming it is refused like any other unreached one. |
//! | [`sees_nothing_at_a_displayed_import`] | layers | A displayed import commits nothing: an import form standing in a fenced block or a double-backtick span never reaches this pass, so a record may exhibit a violating label as evidence without committing one. |
//! | [`reports_a_declared_row_set_that_diverges`] | layers | A declared may-cite row set disagreeing with the manifests fails, naming the owner and both sets, so the declaration is checked rather than believed. |
//! | [`reports_a_member_the_declaration_heads_no_row_for`] | layers | A corpus the workspace builds and the may-cite rows pass over fails on its own terms, because a declaration that may quietly lose a member is not checked but skimmed. |
//! | [`passes_over_an_owner_the_workspace_builds_no_member_for`] | layers | A registered owner with no workspace member may declare reach with no manifest edge behind it, so its rows are passed over rather than reported: the registration is the fact and the crate is merely not present yet. |
//! | [`reconciles_nothing_where_the_surface_declares_no_reach`] | layers | A surface declaring no may-cite row at all is dormant rather than divergent: the reach rules still hold, and the reconciliation reports nothing about a relation nobody stated. |
//! | [`derives_no_edge_for_a_package_the_workspace_does_not_build`] | layers | cites (´claim:layers:an-absent-owner-is-reachable-by-nobody´) |
//! | [`keeps_an_edgeless_corpus_in_the_relation`] | layers | cites (´claim:layers:reach-is-one-declared-hop´) |

use std::fs;
use std::path::Path;

use crate::adoption::{Adoption, Owner, index_adoption as build_index_adoption};
use crate::carrier::index_carrier;
use crate::code::{CodeSurface, take_code_citations};
use crate::depend::{cited_edges, retiring_verify};
use crate::engine::analyze;
use crate::finding::Finding;
use crate::layers::{derive_reach, verify_layers};
use crate::snapshot::{Pair, ReachRow};

fn index_adoption(
    packages: &[crate::workspace::Package],
    names: Option<&crate::roster::OwnerNames>,
    assemblies: &[crate::assembly::Assembly],
) -> Adoption {
    build_index_adoption(
        packages,
        names,
        assemblies,
        crate::registry::fixture_kind_registry(),
    )
}

fn packages(root: &Path) -> Vec<crate::workspace::Package> {
    crate::plan::WorkspacePlan::compile(root)
        .packages()
        .to_vec()
}

fn owner(name: &str) -> Owner {
    Owner::new(name)
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("create");
    fs::write(path, text).expect("write");
}

/// A fixture workspace of a root and three packages.
///
/// The edges are the ones the cases below need and no more: the root
/// depends on alpha ordinarily, alpha on beta for its tests alone, and
/// gamma on nobody. Beta declares a library target path, which is a `path`
/// key standing outside every dependency table and must declare no edge.
fn workspace(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[workspace]\n\
             members = [\".\", \"packages/alpha\", \"packages/beta\", \"packages/gamma\"]\n\n\
             [package]\nname = \"torrust-index\"\n\n\
             [dependencies]\ntorrust-alpha = { path = \"packages/alpha\" }\n",
    );
    write(
        root,
        "packages/alpha/Cargo.toml",
        "[package]\nname = \"torrust-alpha\"\n\n\
             [dev-dependencies]\ntorrust-beta = { path = \"../beta\" }\n",
    );
    write(
        root,
        "packages/beta/Cargo.toml",
        "[package]\nname = \"torrust-beta\"\n\n[lib]\npath = \"src/lib.rs\"\n",
    );
    write(
        root,
        "packages/gamma/Cargo.toml",
        "[package]\nname = \"torrust-gamma\"\n",
    );
}

/// The four heads the citations below reach, one per corpus.
fn heads(root: &Path) {
    write(
        root,
        "adr/policy.md",
        "# Policy\n\n## Policy · `sec:root:policy`\n\nRepo-wide.\n",
    );
    write(
        root,
        "packages/alpha/docs/head.md",
        "# Alpha\n\n## Alpha · `sec:alpha:thing`\n\nA head.\n",
    );
    write(
        root,
        "packages/beta/docs/head.md",
        "# Beta\n\n## Beta · `sec:beta:thing`\n\nA head.\n",
    );
    write(
        root,
        "packages/gamma/docs/head.md",
        "# Gamma\n\n## Gamma · `sec:gamma:thing`\n\nA head.\n",
    );
}

/// Write one generated matrix row importing beta's head from a package.
fn generated_beta_import(root: &Path, package: &str) {
    let path = format!("packages/{package}/src/tests/README.md");
    let text = format!(
        "## Crate test matrix · `tab:{package}:crate-test-matrix`\n\n\
         **Table (Crate test matrix)**\n\n\
         | Test | Area | Claim |\n|------|------|-------|\n\
         | (`[BETA-sec:beta:thing]`) | fixture | A generated import. |\n"
    );

    write(root, &path, &text);
}

fn corpus(root: &Path) -> crate::plan::CorpusPlan {
    crate::plan::CorpusPlan::compile(root, crate::universe::UniverseKind::AsWritten, &[])
        .expect("fixture topology")
}

/// Run the whole check path over a fixture root and keep the layer verdicts.
fn judge(root: &Path) -> (Vec<Finding>, crate::LayerAnalysis) {
    let packages = packages(root);
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let (sources, _carrier) = index_carrier(root, &corpus(root)).read();
    let analysis = analyze(&adoption, &sources, &CodeSurface::default());
    let (counts, findings) = verify_layers(root, &packages, &adoption, &analysis, &[]);

    (findings, counts)
}

/// The same, over a surface stating the given may-cite rows.
///
/// The reconciliation cases state the relation whole — self, upward and
/// manifest rows alike — because that is what the owner file writes and
/// therefore what the check compares.
fn judge_declaring(root: &Path, rows: &[(&str, &str)]) -> (Vec<Finding>, crate::LayerAnalysis) {
    let declared: Vec<ReachRow> = rows
        .iter()
        .map(|(owner, target)| ReachRow {
            owner: (*owner).to_owned(),
            target: (*target).to_owned(),
        })
        .collect();

    let packages = packages(root);
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let (sources, _carrier) = index_carrier(root, &corpus(root)).read();
    let analysis = analyze(&adoption, &sources, &CodeSurface::default());
    let (counts, findings) = verify_layers(root, &packages, &adoption, &analysis, &declared);

    (findings, counts)
}

/// The same, with the code surface read too, for the cases about the
/// crate's own commentary.
fn judge_with_code(root: &Path) -> (Vec<Finding>, crate::LayerAnalysis) {
    let packages = packages(root);
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let corpus = corpus(root);
    let (citations, _code_findings) = take_code_citations(root, &packages, &[], &corpus);
    let code = CodeSurface::default().with_citations(citations);
    let (sources, _carrier) = index_carrier(root, &corpus).read();
    let analysis = analyze(&adoption, &sources, &code);
    let (counts, findings) = verify_layers(root, &packages, &adoption, &analysis, &[]);

    (findings, counts)
}

/// Every finding the whole engine raised, for the assertions about what the
/// layer law leaves alone.
fn engine_codes(root: &Path) -> Vec<&'static str> {
    let packages = packages(root);
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let (sources, _carrier) = index_carrier(root, &corpus(root)).read();

    analyze(&adoption, &sources, &CodeSurface::default())
        .findings()
        .iter()
        .map(Finding::code)
        .collect()
}

fn codes(findings: &[Finding]) -> Vec<&'static str> {
    findings.iter().map(Finding::code).collect()
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary directory");
    workspace(root.path());
    heads(root.path());

    root
}

fn fixture_reach(root: &Path) -> crate::layers::Reach {
    let packages = packages(root);
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );

    derive_reach(root, &packages, &adoption)
}

/// Reach is read from the manifests and from all three dependency tables —
/// ordinary, development and build — so a dependency declared for tests
/// warrants a citation exactly as one declared for the build does. A `path`
/// key outside a dependency table, such as a library target's, declares no
/// edge.
///
/// ´claim:layers:reach-is-read-from-every-dependency-table´
/// ´test:crate:derives-reach-from-every-dependency-table´
#[test]
fn derives_reach_from_every_dependency_table() {
    let root = fixture();
    let packages = packages(root.path());
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let reach = derive_reach(root.path(), &packages, &adoption);

    assert!(
        reach.reaches(&owner("index"), &owner("torrust-alpha")),
        "an ordinary edge"
    );
    assert!(
        reach.reaches(&owner("torrust-alpha"), &owner("torrust-beta")),
        "a development edge reaches exactly as an ordinary one does"
    );
    assert!(
        !reach.reaches(&owner("torrust-beta"), &owner("torrust-gamma")),
        "a library target's path declares no dependency"
    );
    assert_eq!(reach.edge_count(), 2, "two declared edges and no more");
}

/// A member dependency inherited from the workspace derives the edge carried
/// by the root workspace dependency's path.
///
/// ´claim:layers:workspace-inheritance-carries-the-root-path-edge´
/// ´test:crate:derives-an-inherited-workspace-path-dependency´
#[test]
fn derives_an_inherited_workspace_path_dependency() {
    let root = fixture();
    write(
        root.path(),
        "Cargo.toml",
        "[workspace]\n\
             members = [\".\", \"packages/alpha\", \"packages/beta\", \"packages/gamma\"]\n\n\
             [workspace.dependencies]\ntorrust-beta = { path = \"packages/beta\" }\n\n\
             [package]\nname = \"torrust-index\"\n\n\
             [dependencies]\ntorrust-alpha = { path = \"packages/alpha\" }\n",
    );
    write(
        root.path(),
        "packages/alpha/Cargo.toml",
        "[package]\nname = \"torrust-alpha\"\n\n\
             [dev-dependencies]\ntorrust-beta = { workspace = true }\n",
    );

    let reach = fixture_reach(root.path());

    assert!(
        reach.reaches(&owner("torrust-alpha"), &owner("torrust-beta")),
        "the inherited root path declares the member edge"
    );
    assert_eq!(reach.edge_count(), 2, "the two path edges and no more");
}

/// A member dependency inherited from a non-path workspace dependency derives
/// no reach edge.
///
/// ´claim:layers:workspace-inheritance-of-a-non-path-dependency-carries-no-edge´
/// ´test:crate:derives-no-edge-from-an-inherited-dependency-without-a-path´
#[test]
fn derives_no_edge_from_an_inherited_dependency_without_a_path() {
    let root = fixture();
    write(
        root.path(),
        "Cargo.toml",
        "[workspace]\n\
             members = [\".\", \"packages/alpha\", \"packages/beta\", \"packages/gamma\"]\n\n\
             [workspace.dependencies]\nserde = \"1\"\n\n\
             [package]\nname = \"torrust-index\"\n\n\
             [dependencies]\ntorrust-alpha = { path = \"packages/alpha\" }\n",
    );
    write(
        root.path(),
        "packages/alpha/Cargo.toml",
        "[package]\nname = \"torrust-alpha\"\n\n\
             [dev-dependencies]\nserde = { workspace = true }\n",
    );

    let reach = fixture_reach(root.path());

    assert!(
        reach.declared(&owner("torrust-alpha")).is_empty(),
        "an inherited external dependency declares no edge"
    );
    assert_eq!(reach.edge_count(), 1, "only the root's literal path edge");
}

/// A literal path in a member dependency table continues to derive its edge
/// without workspace inheritance.
///
/// ´claim:layers:a-literal-member-path-dependency-still-carries-its-edge´
/// ´test:crate:continues-to-derive-a-literal-member-path-dependency´
#[test]
fn continues_to_derive_a_literal_member_path_dependency() {
    let root = fixture();
    let reach = fixture_reach(root.path());

    assert!(
        reach.reaches(&owner("torrust-alpha"), &owner("torrust-beta")),
        "the member's literal path still declares its edge"
    );
    assert_eq!(
        reach.edge_count(),
        2,
        "the two literal path edges and no more"
    );
}

/// Reach is one declared hop: a corpus reaches itself, reaches what its
/// manifest names, and reaches nothing two hops away. Closing the relation
/// would license imports nobody wrote down.
///
/// ´claim:layers:reach-is-one-declared-hop´
/// ´test:crate:reach-is-reflexive-and-not-transitive´
#[test]
fn reach_is_reflexive_and_not_transitive() {
    let root = fixture();
    let packages = packages(root.path());
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let reach = derive_reach(root.path(), &packages, &adoption);

    assert!(
        reach.reaches(&owner("torrust-gamma"), &owner("torrust-gamma")),
        "reflexive"
    );
    assert!(
        !reach.reaches(&owner("index"), &owner("torrust-beta")),
        "root reaches alpha and alpha reaches beta, and root still does not reach beta"
    );
}

/// A member importing a root head is admitted whether or not its manifest
/// names the root crate, because repo-wide policy must be citable by
/// everyone it binds.
///
/// ´claim:layers:an-upward-import-is-admitted-from-any-member´
/// ´test:crate:admits-an-upward-import-from-any-member´
#[test]
fn admits_an_upward_import_from_any_member() {
    let root = fixture();
    write(
        root.path(),
        "packages/gamma/docs/cites.md",
        "# Gamma cites\n\n## Cites · `sec:gamma:cites`\n\nUnder (`[INDEX-sec:root:policy]`).\n",
    );

    let (findings, counts) = judge(root.path());

    assert!(
        findings.is_empty(),
        "no manifest edge is wanted upward: {findings:?}"
    );
    assert_eq!(counts.upward_imports, 1);
    assert_eq!(counts.violations, 0);
}

/// A package importing a head of a package its manifest depends on is
/// admitted: the warrant is the declared edge, and the law enforces
/// structure rather than denying it.
///
/// ´claim:layers:an-import-along-a-declared-edge-is-admitted´
/// ´test:crate:admits-an-import-along-a-declared-dependency-edge´
#[test]
fn admits_an_import_along_a_declared_dependency_edge() {
    let root = fixture();
    write(
        root.path(),
        "packages/alpha/docs/cites.md",
        "# Alpha cites\n\n## Cites · `sec:alpha:cites`\n\nSee (`[BETA-sec:beta:thing]`).\n",
    );

    let (findings, counts) = judge(root.path());

    assert!(
        findings.is_empty(),
        "the declared edge warrants it: {findings:?}"
    );
    assert_eq!(counts.sideways_imports, 1);
    assert_eq!(counts.violations, 0);
}

/// A generated imported citation is judged by the same reach relation as an
/// authored one: without the manifest edge the layer pass refuses the matrix
/// row, and adding the edge repairs the fixture without changing the law.
///
/// ´claim:layers:generated-imports-obey-reach´
/// ´test:crate:generated-imports-obey-reach´
#[test]
fn generated_imports_obey_reach() {
    let root = fixture();
    generated_beta_import(root.path(), "alpha");
    write(
        root.path(),
        "packages/alpha/Cargo.toml",
        "[package]\nname = \"torrust-alpha\"\n",
    );

    let (findings, counts) = judge(root.path());
    let [Finding::UnreachedImport { location, .. }] = findings.as_slice() else {
        panic!("the mutation must fail the reach law: {findings:?}");
    };

    assert_eq!(
        location.path(),
        Path::new("packages/alpha/src/tests/README.md")
    );
    assert_eq!(
        counts.violations, 1,
        "the generated import is the violation"
    );

    write(
        root.path(),
        "packages/alpha/Cargo.toml",
        "[package]\nname = \"torrust-alpha\"\n\n\
         [dev-dependencies]\ntorrust-beta = { path = \"../beta\" }\n",
    );

    let (findings, counts) = judge(root.path());

    assert!(
        findings.is_empty(),
        "the fixture-side edge repairs it: {findings:?}"
    );
    assert_eq!(counts.sideways_imports, 1);
    assert_eq!(counts.violations, 0);
}

/// A generated imported citation enters the cited-owner relation and therefore
/// instantiates the dependency pair of the owner it names, carrying the
/// register row as the first requiring location.
///
/// ´claim:depend:a-generated-import-instantiates-its-cited-owner-prerequisite´
/// ´test:crate:a-generated-import-instantiates-its-cited-owner-prerequisite´
#[test]
fn a_generated_import_instantiates_its_cited_owner_prerequisite() {
    let root = fixture();
    generated_beta_import(root.path(), "alpha");
    let packages = packages(root.path());
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let (sources, _carrier) = index_carrier(root.path(), &corpus(root.path())).read();
    let analysis = analyze(&adoption, &sources, &CodeSurface::default());
    let edges = cited_edges(&adoption, &analysis);

    let [edge] = edges.as_slice() else {
        panic!("one generated cited-owner edge: {edges:?}");
    };

    assert_eq!(edge.owner, "torrust-alpha");
    assert_eq!(edge.target, "torrust-beta");
    assert_eq!(
        edge.location.path(),
        Path::new("packages/alpha/src/tests/README.md")
    );

    let pairs = vec![
        Pair::singleton("torrust-alpha", "labels.citations-imported-resolve"),
        Pair::singleton("torrust-alpha", "labels.citations-import-form"),
        Pair::singleton("torrust-alpha", "labels.mints-well-formed"),
    ];
    let findings = retiring_verify(&pairs, &edges, Some("index"));
    let [
        Finding::MissingPolicyDependency {
            scope,
            required_owner,
            required_policy,
            location,
            ..
        },
    ] = findings.as_slice()
    else {
        panic!("one dependency instantiated by the generated import: {findings:?}");
    };

    assert_eq!(*scope, "cited-owner");
    assert_eq!(required_owner, "torrust-beta");
    assert_eq!(required_policy, "labels.mints-unique");
    assert_eq!(
        location.as_ref().map(crate::finding::Location::path),
        Some(Path::new("packages/alpha/src/tests/README.md"))
    );
}

/// A package importing a head of a package neither manifest names is
/// refused, naming the citing location, the cited prefix, and the manifest
/// edge whose absence refuses it.
///
/// ´claim:layers:a-sideways-import-across-no-edge-is-refused´
/// ´test:crate:refuses-a-sideways-import-across-no-edge´
#[test]
fn refuses_a_sideways_import_across_no_edge() {
    let root = fixture();
    write(
        root.path(),
        "packages/gamma/docs/cites.md",
        "# Gamma cites\n\n## Cites · `sec:gamma:cites`\n\nSee (`[BETA-sec:beta:thing]`).\n",
    );

    let (findings, counts) = judge(root.path());

    let [
        Finding::UnreachedImport {
            citing_owner,
            prefix,
            cited_owner,
            absent_edge,
            location,
        },
    ] = findings.as_slice()
    else {
        panic!("one unreached import: {findings:?}");
    };

    assert_eq!(citing_owner, "torrust-gamma");
    assert_eq!(prefix.as_str(), "BETA");
    assert_eq!(cited_owner, "torrust-beta");
    assert!(
        absent_edge.contains("packages/gamma/Cargo.toml") && absent_edge.contains("packages/beta"),
        "the absent edge names both ends: {absent_edge}"
    );
    assert_eq!(location.path(), Path::new("packages/gamma/docs/cites.md"));
    assert_eq!(counts.violations, 1);

    // Resolution is untouched: the refused import still reached its mint.
    assert!(
        !engine_codes(root.path()).contains(&"unresolved_citation"),
        "a refused import still resolves"
    );
}

/// Repo-wide policy imports nothing: the root corpus's prose carrying an
/// import of a package prefix is refused even where the root's manifest
/// reaches that package, because the rule is about the prose rather than
/// about the crate.
///
/// ´claim:layers:repo-wide-policy-imports-no-package-prefix´
/// ´test:crate:refuses-a-package-import-in-the-root-corpus-prose´
#[test]
fn refuses_a_package_import_in_the_root_corpus_prose() {
    let root = fixture();
    write(
        root.path(),
        "adr/cites.md",
        "# Policy cites\n\n## Cites · `sec:root:cites`\n\nSee (`[ALPHA-sec:alpha:thing]`).\n",
    );

    let (findings, counts) = judge(root.path());

    let [
        Finding::PolicyImport {
            prefix,
            cited_owner,
            ..
        },
    ] = findings.as_slice()
    else {
        panic!("one policy import: {findings:?}");
    };

    assert_eq!(prefix.as_str(), "ALPHA");
    assert_eq!(cited_owner, "torrust-alpha");
    assert_eq!(counts.downward_imports, 1);
    assert_eq!(counts.violations, 1);
}

/// The narrow converse is narrow: the root crate's own commentary may
/// import a package it declares a dependency on, because what the rule
/// closes is policy resting on a private premise, not the crate's code
/// resting on its dependencies.
///
/// ´claim:layers:the-root-crate-s-code-is-held-to-reach-like-any-other´
/// ´test:crate:admits-a-reached-package-import-in-the-root-corpus-code´
#[test]
fn admits_a_reached_package_import_in_the_root_corpus_code() {
    let root = fixture();
    // The root reaches alpha and not beta, so the crate's own commentary is
    // admitted for the one and refused for the other — on the reach rule,
    // never on the policy rule, which reads the prose alone.
    write(
        root.path(),
        "src/lib.rs",
        "//! The root crate.\n\
             //!\n\
             //! Reached: (´[ALPHA-sec:alpha:thing]´).\n",
    );

    let (findings, counts) = judge_with_code(root.path());

    assert!(
        findings.is_empty(),
        "the manifest edge warrants it: {findings:?}"
    );
    assert_eq!(
        counts.downward_imports, 1,
        "it is still the downward direction"
    );
    assert_eq!(counts.violations, 0);
}

/// A prefix registered against no workspace member is reachable by nobody:
/// no manifest can declare a dependency on a crate the workspace does not
/// build, so an import naming it is refused like any other unreached one.
///
/// ´claim:layers:an-absent-owner-is-reachable-by-nobody´
/// ´test:crate:refuses-an-import-of-a-registered-but-absent-owner´
#[test]
fn refuses_an_import_of_a_registered_but_absent_owner() {
    let root = fixture();
    // The pending registration owns the prose beneath its directory
    // although the workspace builds no such crate.
    write(
        root.path(),
        "packages/notime/docs/head.md",
        "# Notime\n\n## Notime · `sec:notime:thing`\n\nA head.\n",
    );
    write(
        root.path(),
        "packages/gamma/docs/cites.md",
        "# Gamma cites\n\n## Cites · `sec:gamma:cites`\n\nSee (`[NOTIME-sec:notime:thing]`).\n",
    );

    let (findings, _counts) = judge(root.path());

    let [Finding::UnreachedImport { absent_edge, .. }] = findings.as_slice() else {
        panic!("one unreached import: {findings:?}");
    };

    assert!(
        absent_edge.contains("no workspace member"),
        "the answer is that nothing can declare the dependency: {absent_edge}"
    );
}

/// A displayed import commits nothing: an import form standing in a fenced
/// block or a double-backtick span never reaches this pass, so a record may
/// exhibit a violating label as evidence without committing one.
///
/// ´claim:layers:a-displayed-import-commits-nothing´
/// ´test:crate:sees-nothing-at-a-displayed-import´
#[test]
fn sees_nothing_at_a_displayed_import() {
    let root = fixture();
    write(
        root.path(),
        "adr/shows.md",
        "# Policy shows\n\n## Shows · `sec:root:shows`\n\n\
             Shown inert: (``[ALPHA-sec:alpha:thing]``).\n\n\
             ```text\n(`[BETA-sec:beta:thing]`)\n```\n",
    );

    let (findings, counts) = judge(root.path());

    assert!(
        findings.is_empty(),
        "a display is not an import: {findings:?}"
    );
    assert_eq!(
        counts.downward_imports, 0,
        "and it is counted as nothing at all"
    );
}

/// A declared may-cite row set disagreeing with the manifests fails, naming
/// the owner and both sets, so the declaration is checked rather than
/// believed.
///
/// ´claim:layers:a-divergent-declared-row-set-fails´
/// ´test:crate:reports-a-declared-row-set-that-diverges´
#[test]
fn reports_a_declared_row_set_that_diverges() {
    let root = fixture();

    // Every corpus states the whole of what it may cite, and ALPHA states
    // its self and upward rows without the manifest edge its dev-dependency
    // on beta puts there.
    let (findings, _counts) = judge_declaring(
        root.path(),
        &[
            ("INDEX", "INDEX"),
            ("INDEX", "ALPHA"),
            ("ALPHA", "ALPHA"),
            ("ALPHA", "INDEX"),
            ("BETA", "BETA"),
            ("BETA", "INDEX"),
            ("GAMMA", "GAMMA"),
            ("GAMMA", "INDEX"),
        ],
    );

    let [
        Finding::ReachDeclarationDivergence {
            corpus,
            registered,
            derived,
            ..
        },
    ] = findings.as_slice()
    else {
        panic!("one divergence: {findings:?}");
    };

    assert_eq!(corpus, "ALPHA");
    assert_eq!(registered, &["ALPHA".to_owned(), "INDEX".to_owned()]);
    assert_eq!(
        derived,
        &["ALPHA".to_owned(), "BETA".to_owned(), "INDEX".to_owned()],
        "the manifest's dev-dependency on beta is admissible and unstated"
    );
}

/// A corpus the workspace builds and the may-cite rows pass over fails on
/// its own terms, because a declaration that may quietly lose a member is
/// not checked but skimmed.
///
/// ´claim:layers:a-member-the-declaration-omits-fails´
/// ´test:crate:reports-a-member-the-declaration-heads-no-row-for´
#[test]
fn reports_a_member_the_declaration_heads_no_row_for() {
    let root = fixture();

    // GAMMA is built and stated nowhere; the other three state themselves
    // whole, so the omission is the only thing left to report.
    let (findings, _counts) = judge_declaring(
        root.path(),
        &[
            ("INDEX", "INDEX"),
            ("INDEX", "ALPHA"),
            ("ALPHA", "ALPHA"),
            ("ALPHA", "INDEX"),
            ("ALPHA", "BETA"),
            ("BETA", "BETA"),
            ("BETA", "INDEX"),
        ],
    );

    let [Finding::ReachDeclarationOmission { corpus, .. }] = findings.as_slice() else {
        panic!("one omission: {findings:?}");
    };

    assert_eq!(
        corpus, "GAMMA",
        "the workspace builds it and no row heads it"
    );
}

/// A registered owner with no workspace member may declare reach with no
/// manifest edge behind it, so its rows are passed over rather than
/// reported: the registration is the fact and the crate is merely not
/// present yet.
///
/// ´claim:layers:an-unbuilt-owner-may-declare-reach´
/// ´test:crate:passes-over-an-owner-the-workspace-builds-no-member-for´
#[test]
fn passes_over_an_owner_the_workspace_builds_no_member_for() {
    let root = fixture();

    // NOTIME is registered against a crate the workspace does not build, and
    // states the two rows a present member would state. Nothing derives
    // them and nothing reports them.
    let (findings, counts) = judge_declaring(
        root.path(),
        &[
            ("INDEX", "INDEX"),
            ("INDEX", "ALPHA"),
            ("ALPHA", "ALPHA"),
            ("ALPHA", "INDEX"),
            ("ALPHA", "BETA"),
            ("BETA", "BETA"),
            ("BETA", "INDEX"),
            ("GAMMA", "GAMMA"),
            ("GAMMA", "INDEX"),
            ("NOTIME", "NOTIME"),
            ("NOTIME", "INDEX"),
        ],
    );

    assert!(
        findings.is_empty(),
        "the unbuilt owner's rows are its own: {findings:?}"
    );
    assert_eq!(counts.registered_rows, 11, "and every row is still counted");
}

/// A surface declaring no may-cite row at all is dormant rather than
/// divergent: the reach rules still hold, and the reconciliation reports
/// nothing about a relation nobody stated.
///
/// ´claim:layers:an-unstated-relation-is-dormant´
/// ´test:crate:reconciles-nothing-where-the-surface-declares-no-reach´
#[test]
fn reconciles_nothing_where_the_surface_declares_no_reach() {
    let root = fixture();
    write(
        root.path(),
        "packages/gamma/docs/cites.md",
        "# Gamma cites\n\n## Cites · `sec:gamma:cites`\n\nSee (`[BETA-sec:beta:thing]`).\n",
    );

    let (findings, counts) = judge(root.path());

    assert!(counts.register_dormant, "the surface states no reach");
    assert_eq!(counts.registered_rows, 0);
    assert_eq!(
        codes(&findings),
        ["unreached_import"],
        "and the law still bites"
    );
}

/// A member the workspace does not build owns no row and derives no edge,
/// so a package present on disk without a manifest cannot be reached.
///
/// (´claim:layers:an-absent-owner-is-reachable-by-nobody´)
/// ´test:crate:derives-no-edge-for-a-package-the-workspace-does-not-build´
#[test]
fn derives_no_edge_for_a_package_the_workspace_does_not_build() {
    let root = fixture();
    let packages = packages(root.path());
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let reach = derive_reach(root.path(), &packages, &adoption);

    assert!(reach.crate_name_of(&owner("torrust-notime")).is_none());
    assert!(!reach.reaches(&owner("torrust-alpha"), &owner("torrust-notime")));
    assert!(
        reach.reaches(&owner("torrust-notime"), reach.root()),
        "an absent owner still reaches the root, as every corpus does"
    );
}

/// A fixture package standing outside every declared dependency keeps the
/// derivation total: an owner with no edges is present in the relation with
/// an empty edge set rather than missing from it.
///
/// (´claim:layers:reach-is-one-declared-hop´)
/// ´test:crate:keeps-an-edgeless-corpus-in-the-relation´
#[test]
fn keeps_an_edgeless_corpus_in_the_relation() {
    let root = fixture();
    let packages = packages(root.path());
    let adoption = index_adoption(
        &packages,
        Some(&crate::roster::OwnerNames::new(
            "torrust-",
            [crate::roster::UnbuiltMember::new(
                "torrust-notime",
                "packages/notime",
            )],
        )),
        &[],
    );
    let reach = derive_reach(root.path(), &packages, &adoption);

    assert_eq!(reach.corpus_count(), 4, "the root and its three packages");
    assert!(reach.declared(&owner("torrust-gamma")).is_empty());
    assert_eq!(reach.crate_name_of(&owner("index")), Some("torrust-index"));
}
