// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Adoption data as code: the signature, the owner partition, and the reserved
//! kinds.
//!
//! ADR-L-014 is parametric in seven data, and a corpus adopts it by fixing
//! them. The owners environment (ADR-L-014, A calculus of documentation and source labels) fixes the
//! signature — a partial map from registered prefixes to owners — together with
//! the owner partition, a map from carrier sources and covered assets to owners
//! that is total on the carrier. The reserved-kinds environment
//! (ADR-L-014, A calculus of documentation and source labels) fixes the set of kinds intended for
//! derivation only.
//!
//! Wave L1 gave the repository's prose one owner. Wave T1 partitions prose the
//! way ADR-L-015 already partitions code: a package's documentation and decision
//! records belong to that package, so a package's prose and its code share one
//! owner and one registered prefix, while prose above the packages — the
//! repository's own records, its documentation tree, and its readme — belongs to
//! the repository.
//!
//! One consequence is worth stating where the reader will meet it. The prefix
//! ADR-L-015 derives for the root crate is the prefix the repository's prose
//! wants, and the signature is a map: one prefix names one owner. The root
//! package and the repository's prose are therefore one owner, carrying the name
//! wave L1 gave it, so package prose citing a repository record writes that
//! prefix and reaches exactly one registry. Every other package keeps its crate
//! name as its owner's name.
//!
//! The reserved kinds arrive with the kind registry compiled from the declared
//! configuration snapshot (´dec:kindregistry:runtime-authority´). One profile
//! now governs one of them — the test profile of ADR-L-015 governs
//! `test` — and for every other the warrant-totality invariant
//! (ADR-L-014, A calculus of documentation and source labels) still makes a bare occurrence a hard
//! failure awaiting its derivation, never an authored mint.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`partitions_every_carrier_source_to_one_owner`] | adoption | Prose standing above the packages belongs to the repository as its owner — the decision records, the documentation tree at any depth, and the readme alike — so no source in the carrier is without an owner. |
//! | [`partitions_package_prose_to_its_package`] | adoption | A package's documentation and decision records belong to that package, so its prose and its code share one owner. A package the workspace does not build still owns the prose beneath it, and prose under no registered package falls back to the repository rather than to nobody. |
//! | [`registers_one_prefix_per_package`] | adoption | The signature relates each registered prefix to one owner and reads either way round — prefix to owner and owner back to prefix — so an imported citation can be resolved and one can equally be written. |
//! | [`registers_the_packages_the_record_registers_without_a_manifest`] | adoption | An owner the decision record registers is registered even where the workspace builds no such package: the signature is fixed by the record rather than by the build, so prose can cite a specification whose crate does not exist yet. |
//! | [`leaves_unregistered_prefixes_unregistered`] | adoption | The signature is partial: a prefix nobody registered names no owner, so a citation importing from an unknown corpus fails to resolve rather than silently finding some owner to attach itself to. |
//! | [`unites_the_repository_prose_with_the_root_package`] | adoption | The root package and the repository's own prose are one owner under one prefix, not two owners contending for it. Package prose citing a repository record therefore writes a prefix that reaches exactly one registry, and the root crate's name is no owner of its own. |
//! | [`reserves_the_inventory_kinds`] | registry | cites (´claim:registry:the-reserved-kinds-are-the-assets-conventions-kinds´) |
//! | [`leaves_authored_kinds_unreserved`] | registry | cites (´claim:registry:the-reserved-kinds-are-the-assets-conventions-kinds´) |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::assembly::Assembly;
use crate::label::Prefix;
use crate::registry::KindRegistry;
use crate::roster::OwnerNames;
use crate::workspace::{Package, pending_packages};

/// The owner covering the repository's own prose, and the root package with it.
///
/// The spelling is derived rather than chosen: an owner's prefix is computed
/// from its crate name by one rule and never transcribed at a mint, and where
/// that prefix stands as an area it is written lowercased, the label language
/// admitting no hyphen in an area (´[EMBER-conv:profiles:owner-prefixes]´). This
/// is the root package's own prefix read that way, so the value moves only if
/// the crate is renamed.
///
/// ´const:emberlinter:root-owner-area´ (´[EMBER-alg:const:word]´)
/// ´const:emberlinter:root-owner-area-word-index´
const INDEX_OWNER: &str = "index";

/// An owner: one cell of the partition of the corpus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Owner(String);

impl Owner {
    /// Name an owner.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The owner's name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Owner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The adoption data the checker consumes.
#[derive(Debug, Clone)]
pub struct Adoption {
    prefixes: BTreeMap<Prefix, Owner>,
    registry: KindRegistry,
    carrier_owner: Owner,
    path_owners: BTreeMap<PathBuf, Owner>,
    package_owners: Vec<(PathBuf, Owner)>,
    generated: Vec<PathBuf>,
}

impl Adoption {
    /// Build adoption data from a prefix signature, the kind registry, and the
    /// owner that the partition assigns to a carrier source above the packages.
    #[must_use]
    pub const fn new(
        prefixes: BTreeMap<Prefix, Owner>,
        registry: KindRegistry,
        carrier_owner: Owner,
    ) -> Self {
        Self {
            prefixes,
            registry,
            carrier_owner,
            path_owners: BTreeMap::new(),
            package_owners: Vec::new(),
            generated: Vec::new(),
        }
    }

    /// Record the documents that are generated rather than authored.
    ///
    /// A publication assembled from parts is the second way a carried document
    /// can fail to participate, beside a generated region, and the reason is the
    /// stronger of the two: its every mint already stands in the part it was
    /// written in, so reading the publication as well would report each of them
    /// as a duplicate of itself.
    #[must_use]
    pub fn with_generated_documents(
        mut self,
        documents: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        self.generated = documents.into_iter().collect();

        self
    }

    /// Partition the prose under package directories to those packages' owners.
    ///
    /// Directories are matched as path prefixes, longest first, so a package
    /// nested inside another's directory would still take its own prose.
    #[must_use]
    pub fn with_package_owners(
        mut self,
        owners: impl IntoIterator<Item = (PathBuf, Owner)>,
    ) -> Self {
        self.package_owners = owners
            .into_iter()
            .filter(|(directory, _owner)| !directory.as_os_str().is_empty())
            .collect();
        self.package_owners.sort_by(|left, right| {
            right
                .0
                .components()
                .count()
                .cmp(&left.0.components().count())
                .then_with(|| left.0.cmp(&right.0))
        });

        self
    }

    /// Take the topology compiler's exact path attribution.
    ///
    /// A planned run therefore never derives ownership from directory
    /// geography inside the label engine.
    #[must_use]
    pub fn with_path_owners(mut self, owners: impl IntoIterator<Item = (PathBuf, Owner)>) -> Self {
        self.path_owners = owners.into_iter().collect();

        self
    }

    /// Register every owner the validated declaration names.
    ///
    /// A package-derived prefix already carries its crate owner's identity and
    /// keeps it. A prefix the package roster did not derive instead names the
    /// declared owner directly, which gives a crateless share the same citation
    /// entry point as a crate-bearing one. The declaration loader has already
    /// applied prefix grammar and uniqueness before this projection is built.
    ///
    /// The partition's declared root prefix names the standing carrier owner.
    /// That identity predates the declared spelling and remains the key on
    /// existing findings, while the new prefix makes the same owner citable.
    #[must_use]
    pub fn with_declared_owners(mut self, owners: &[String], root_owner: Option<&str>) -> Self {
        for declared in owners {
            let Some(prefix) = Prefix::parse(declared) else {
                continue;
            };

            let owner = if Some(declared.as_str()) == root_owner {
                self.carrier_owner.clone()
            } else {
                Owner::new(declared)
            };

            self.prefixes.entry(prefix).or_insert(owner);
        }

        self
    }

    /// The owner the partition assigns to a carrier source.
    ///
    /// The partition is total on the carrier: prose under a package directory
    /// belongs to that package, and prose above the packages belongs to the
    /// repository. This is the prose half of the partition ADR-L-015 fixes for
    /// code, and it agrees with that half by construction — the owners are the
    /// same owners, named the same way.
    #[must_use]
    pub fn owner_of(&self, source: &Path) -> &Owner {
        if let Some(owner) = self.path_owners.get(source) {
            return owner;
        }

        if let Some(owner) = self
            .package_owners
            .iter()
            .find_map(|(directory, owner)| source.starts_with(directory).then_some(owner))
        {
            return owner;
        }

        &self.carrier_owner
    }

    /// The kind registry this corpus adopted.
    #[must_use]
    pub const fn registry(&self) -> &KindRegistry {
        &self.registry
    }

    /// Whether a carrier source's spans participate.
    ///
    /// A generated document is carried and read, but forms no minting or
    /// resolution judgment: its mints stand in the sources it was assembled
    /// from, and reading it as well would report each of them twice.
    #[must_use]
    pub fn participates(&self, source: &Path) -> bool {
        !self.generated.iter().any(|document| source == document)
    }

    /// The owner a registered prefix names, when the signature registers it.
    #[must_use]
    pub fn owner_of_prefix(&self, prefix: &Prefix) -> Option<&Owner> {
        self.prefixes.get(prefix)
    }

    /// The prefix registered for an owner, when one is.
    #[must_use]
    pub fn prefix_of_owner(&self, owner: &Owner) -> Option<&Prefix> {
        self.prefixes
            .iter()
            .find_map(|(prefix, registered)| (registered == owner).then_some(prefix))
    }

    /// Whether a kind is reserved for derivation.
    #[must_use]
    pub fn is_reserved_kind(&self, kind: &str) -> bool {
        self.registry.reserved_kinds().contains(kind)
    }

    /// Every owner the adoption knows of.
    #[must_use]
    pub fn owners(&self) -> BTreeSet<&Owner> {
        let mut owners: BTreeSet<&Owner> = self.prefixes.values().collect();
        owners.insert(&self.carrier_owner);
        owners.extend(self.path_owners.values());
        owners.extend(self.package_owners.iter().map(|(_directory, owner)| owner));
        owners
    }
}

/// The owner a package's prose and code both belong to.
///
/// Every package is its own owner under its crate name, save the root package,
/// whose owner is the repository's: the root package's directory is the
/// repository, its derived prefix is the prefix the repository's prose wants, and
/// the signature maps one prefix to one owner.
#[must_use]
pub fn owner_of_package(package: &Package) -> Owner {
    if package.directory().as_os_str().is_empty() {
        Owner::new(INDEX_OWNER)
    } else {
        Owner::new(package.name())
    }
}

/// The label-calculus adoption for this repository.
///
/// The signature registers one prefix per workspace package, together with the
/// registered packages the workspace does not yet build. The kind registry is
/// supplied from the accepted configuration snapshot.
#[must_use]
pub fn index_adoption(
    packages: &[Package],
    names: Option<&OwnerNames>,
    assemblies: &[Assembly],
    registry: KindRegistry,
) -> Adoption {
    let registered: Vec<Package> = packages
        .iter()
        .cloned()
        .chain(pending_packages(names))
        .collect();
    let package_owners = registered
        .iter()
        .map(|package| (package.directory().to_path_buf(), owner_of_package(package)));

    index_signature(packages, names, assemblies, registry).with_package_owners(package_owners)
}

/// The label signature without a directory-geography ownership fallback.
#[must_use]
pub fn index_signature(
    packages: &[Package],
    names: Option<&OwnerNames>,
    assemblies: &[Assembly],
    registry: KindRegistry,
) -> Adoption {
    let registered: Vec<Package> = packages
        .iter()
        .cloned()
        .chain(pending_packages(names))
        .collect();

    let mut prefixes = BTreeMap::new();

    for package in &registered {
        if let Some(prefix) = names.and_then(|names| names.derive(package.name())) {
            prefixes.insert(prefix, owner_of_package(package));
        }
    }

    Adoption::new(prefixes, registry, Owner::new(INDEX_OWNER)).with_generated_documents(
        assemblies
            .iter()
            .map(|assembly| assembly.target().to_path_buf()),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Adoption, Owner, index_adoption as build_index_adoption};
    use crate::assembly::Assembly;
    use crate::label::Prefix;
    use crate::registry::fixture_kind_registry;
    use crate::roster::{OwnerNames, UnbuiltMember};
    use crate::workspace::Package;

    fn index_adoption(
        packages: &[Package],
        names: Option<&OwnerNames>,
        assemblies: &[Assembly],
    ) -> Adoption {
        build_index_adoption(packages, names, assemblies, fixture_kind_registry())
    }

    fn packages() -> Vec<Package> {
        vec![
            Package::new("ember-assayer", "packages/assayer"),
            Package::new("ember-index", ""),
            Package::new("linter", "packages/linter"),
        ]
    }

    /// The reconciliation the owner-name document declares, as these tests read it.
    fn names() -> OwnerNames {
        OwnerNames::new(
            "ember-",
            [UnbuiltMember::new("ember-notime", "packages/notime")],
        )
    }

    fn prefix(text: &str) -> Prefix {
        Prefix::parse(text).expect("well-formed")
    }

    /// Prose standing above the packages belongs to the repository as its
    /// owner — the decision records, the documentation tree at any depth, and
    /// the readme alike — so no source in the carrier is without an owner.
    ///
    /// ´claim:adoption:prose-above-the-packages-belongs-to-the-repository´
    /// ´test:unit:partitions-every-carrier-source-to-one-owner´
    #[test]
    fn partitions_every_carrier_source_to_one_owner() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);
        let expected = Owner::new("index");

        assert_eq!(
            adoption.owner_of(Path::new("adr/014-label-calculus.md")),
            &expected
        );
        assert_eq!(adoption.owner_of(Path::new("README.md")), &expected);
        assert_eq!(
            adoption.owner_of(Path::new("docs/nested/deep/note.md")),
            &expected
        );
    }

    /// A package's documentation and decision records belong to that package,
    /// so its prose and its code share one owner. A package the workspace does
    /// not build still owns the prose beneath it, and prose under no registered
    /// package falls back to the repository rather than to nobody.
    ///
    /// ´claim:adoption:a-packages-prose-belongs-to-that-package´
    /// ´test:unit:partitions-package-prose-to-its-package´
    #[test]
    fn partitions_package_prose_to_its_package() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        assert_eq!(
            adoption.owner_of(Path::new("packages/assayer/docs/plans/backlog.md")),
            &Owner::new("ember-assayer")
        );
        assert_eq!(
            adoption.owner_of(Path::new(
                "packages/assayer/adr/110-feed-forward-boundary.md"
            )),
            &Owner::new("ember-assayer")
        );
        assert_eq!(
            adoption.owner_of(Path::new("packages/notime/docs/spec.md")),
            &Owner::new("ember-notime"),
            "a package the workspace does not build still owns its prose"
        );
        assert_eq!(
            adoption.owner_of(Path::new("packages/unknown/docs/note.md")),
            &Owner::new("index"),
            "prose of no registered package falls to the repository"
        );
    }

    /// The signature relates each registered prefix to one owner and reads
    /// either way round — prefix to owner and owner back to prefix — so an
    /// imported citation can be resolved and one can equally be written.
    ///
    /// ´claim:adoption:the-signature-relates-one-prefix-to-one-owner-both-ways´
    /// ´test:unit:registers-one-prefix-per-package´
    #[test]
    fn registers_one_prefix_per_package() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        assert_eq!(
            adoption.owner_of_prefix(&prefix("ASSAYER")),
            Some(&Owner::new("ember-assayer"))
        );
        assert_eq!(
            adoption.owner_of_prefix(&prefix("LINTER")),
            Some(&Owner::new("linter"))
        );
        assert_eq!(
            adoption.prefix_of_owner(&Owner::new("linter")),
            Some(&prefix("LINTER"))
        );
    }

    /// An owner the decision record registers is registered even where the
    /// workspace builds no such package: the signature is fixed by the record
    /// rather than by the build, so prose can cite a specification whose crate
    /// does not exist yet.
    ///
    /// ´claim:adoption:an-owner-the-record-registers-needs-no-manifest´
    /// ´test:unit:registers-the-packages-the-record-registers-without-a-manifest´
    #[test]
    fn registers_the_packages_the_record_registers_without_a_manifest() {
        let adoption = index_adoption(&[], Some(&names()), &[]);

        assert_eq!(
            adoption.owner_of_prefix(&prefix("NOTIME")),
            Some(&Owner::new("ember-notime"))
        );
    }

    /// The signature is partial: a prefix nobody registered names no owner, so
    /// a citation importing from an unknown corpus fails to resolve rather than
    /// silently finding some owner to attach itself to.
    ///
    /// ´claim:adoption:an-unregistered-prefix-names-no-owner´
    /// ´test:unit:leaves-unregistered-prefixes-unregistered´
    #[test]
    fn leaves_unregistered_prefixes_unregistered() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        assert!(adoption.owner_of_prefix(&prefix("SPEC")).is_none());
    }

    /// The root package and the repository's own prose are one owner under one
    /// prefix, not two owners contending for it. Package prose citing a
    /// repository record therefore writes a prefix that reaches exactly one
    /// registry, and the root crate's name is no owner of its own.
    ///
    /// ´claim:adoption:the-root-package-and-the-repository-prose-are-one-owner´
    /// ´test:unit:unites-the-repository-prose-with-the-root-package´
    #[test]
    fn unites_the_repository_prose_with_the_root_package() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        assert_eq!(
            adoption.prefix_of_owner(&Owner::new("index")),
            Some(&prefix("INDEX")),
            "package prose citing a repository record has a prefix to write"
        );
        assert!(
            !adoption.owners().contains(&Owner::new("ember-index")),
            "the root package and the repository's prose are one owner"
        );
    }

    /// The adoption datum reserves the kinds naming things a tool derives, as
    /// supplied by the package-local fictional registry.
    ///
    /// (´claim:registry:the-reserved-kinds-are-the-assets-conventions-kinds´)
    /// ´test:unit:reserves-the-inventory-kinds´
    #[test]
    fn reserves_the_inventory_kinds() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        for kind in [
            "test", "bench", "mod", "pkg", "func", "endpoint", "envvar", "type",
        ] {
            assert!(
                adoption.is_reserved_kind(kind),
                "expected `{kind}` to be reserved"
            );
        }
    }

    /// The other half of that same reservation: the kinds the corpus actually
    /// mints by hand stay unreserved, so authoring one needs no warrant beyond
    /// the author's.
    ///
    /// (´claim:registry:the-reserved-kinds-are-the-assets-conventions-kinds´)
    /// ´test:unit:leaves-authored-kinds-unreserved´
    #[test]
    fn leaves_authored_kinds_unreserved() {
        let adoption = index_adoption(&packages(), Some(&names()), &[]);

        for kind in [
            "sec", "lang", "gram", "sig", "judg", "inf", "inv", "metathm", "cav", "ansatz", "gate",
        ] {
            assert!(
                !adoption.is_reserved_kind(kind),
                "expected `{kind}` to be authored"
            );
        }
    }
}
