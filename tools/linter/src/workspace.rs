// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Wild Sky Maker

//! Workspace discovery: the packages that own covered assets, and their
//! prefixes.
//!
//! The owners environment of ADR-L-014, A calculus of documentation and source labels, lets a
//! family of sources register one owner per member, with the prefix derived by
//! the family's own rule and never written at a mint. ADR-L-015 fixes that rule
//! for this workspace: the owner of a covered test is its package, and the
//! registered prefix of a package derives mechanically from the crate name by
//! stripping a leading `ember-`, removing hyphens, and uppercasing the rest.
//!
//! The prefixes are therefore computed, not transcribed. This module reads the
//! workspace member list and each member's package name from the manifests on
//! disk, so a new package joins the signature by joining the workspace. The
//! table in ADR-L-015 is checked against the computed set by a test rather than
//! being the source of the set.
//!
//! Two facts about this workspace are worth recording where the reader will
//! meet them. The member list uses plain relative paths rather than globs, so
//! path expansion is deliberately not implemented and an unreadable member
//! surfaces as a traversal diagnostic under the coexistence caveat
//! (ADR-L-014, A calculus of documentation and source labels). A declared owner row may also name a
//! crate whose manifest is intentionally absent, so that datum is carried as a
//! pending registration rather than reconstructed from this workspace.
//!
//! # Test index
//!
//! | Test | Area | Claim |
//! |------|------|-------|
//! | [`derives_prefixes_by_stripping_removing_and_uppercasing`] | owner | An owner's prefix is computed from its crate name by one rule — strip the leading project prefix, remove the hyphens, uppercase the rest — and the rule yields the registered prefix for every crate the record names. A prefix is therefore never written by hand at a mint. |
//! | [`reads_the_members_a_workspace_manifest_lists`] | owner | Owners are discovered by reading the manifests on disk, so a package joins the signature by joining the workspace: every member the manifest lists is found with the directory it stands in, and the member written as the current directory normalises to the root itself rather than to a directory named for a dot. |
//! | [`reads_a_root_without_a_manifest_as_no_workspace`] | owner | A directory that is simply not a workspace yields no owners and no complaint: absence of a manifest is an answer, not a defect, so the linter can be pointed at any tree without inventing diagnostics. |
//! | [`reports_an_unparsable_root_manifest`] | owner | A manifest that exists but cannot be read is reported rather than passed over, so owners never go missing silently and a run over a damaged tree says so instead of reporting a smaller corpus. Here the root manifest is the unparsable one. |
//! | [`reports_a_member_whose_manifest_is_missing`] | owner | cites (´claim:owner:a-manifest-that-cannot-be-read-is-reported-not-skipped´) |
//! | [`reports_a_member_without_a_package_name`] | owner | cites (´claim:owner:a-manifest-that-cannot-be-read-is-reported-not-skipped´) |
//! | [`orders_discovered_packages_by_crate_name`] | owner | Owners order by crate name rather than by where they sit on disk, so a report's ordering follows the names a reader knows and does not shift when a package is moved between directories. |

#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::finding::Finding;
use crate::label::Prefix;
use crate::roster::{OwnerNames, derive_owner};

/// One workspace package: an owner of covered assets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Package {
    name: String,
    directory: PathBuf,
}

impl Package {
    /// Name a package by its crate name and its directory relative to the root.
    #[must_use]
    pub fn new(name: impl Into<String>, directory: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            directory: directory.into(),
        }
    }

    /// The crate name, which is also the owner's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The package directory, relative to the workspace root.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The prefix the derivation rule registers for this package.
    ///
    /// The stripped namespace is passed rather than known, because it is the
    /// corpus's to state and this type is any workspace's.
    #[must_use]
    pub fn prefix(&self, namespace: &str) -> Option<Prefix> {
        prefix_for_crate(namespace, &self.name)
    }
}

/// Derive a package's registered prefix from its crate name.
///
/// The rule is the program's and the namespace is the corpus's, so this is the
/// one applied to the other. The namespace is passed rather than an instance
/// constructed, because a caller deriving one prefix per package should not
/// build a roster to do it.
#[must_use]
pub fn prefix_for_crate(namespace: &str, name: &str) -> Option<Prefix> {
    derive_owner(namespace, name)
}

/// The registered crates a reconciliation says the workspace does not build.
///
/// An absent reconciliation registers none, because which crates are registered
/// without a manifest is a thing the corpus states rather than a thing this
/// binary knows.
#[must_use]
pub fn pending_packages(names: Option<&OwnerNames>) -> Vec<Package> {
    names
        .map(|names| {
            names
                .unbuilt()
                .iter()
                .map(|entry| Package::new(entry.crate_name(), entry.directory()))
                .collect()
        })
        .unwrap_or_default()
}

/// Read the workspace members from the manifests under a root.
///
/// Members are returned ordered by crate name. An unreadable or unparsable
/// manifest becomes a traversal diagnostic rather than an omission, so a broken
/// manifest can never quietly shrink the owner partition. A root with no
/// manifest at all is not a defect but a root with no workspace — a prose-only
/// tree, which the check must still be able to read.
#[must_use]
#[cfg(test)]
pub fn retiring_read_workspace(root: &Path) -> (Vec<Package>, Vec<Finding>) {
    let mut packages = Vec::new();
    let mut findings = Vec::new();

    if !root.join("Cargo.toml").is_file() {
        return (packages, findings);
    }

    let Some(manifest) = read_manifest(root, Path::new("."), &mut findings) else {
        return (packages, findings);
    };

    let members = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array);

    let Some(members) = members else {
        findings.push(Finding::TraversalFailure {
            path: "Cargo.toml".to_owned(),
            message: "the root manifest declares no workspace members".to_owned(),
        });
        return (packages, findings);
    };

    for member in members {
        let Some(member) = member.as_str() else {
            findings.push(Finding::TraversalFailure {
                path: "Cargo.toml".to_owned(),
                message: "a workspace member is not a string".to_owned(),
            });
            continue;
        };

        let directory = PathBuf::from(member);

        let Some(member_manifest) = read_manifest(root, &directory, &mut findings) else {
            continue;
        };

        let name = member_manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(toml::Value::as_str);

        match name {
            Some(name) => packages.push(Package::new(name, normalize(&directory))),
            None => findings.push(Finding::TraversalFailure {
                path: manifest_path(&directory).to_string_lossy().into_owned(),
                message: "the manifest declares no package name".to_owned(),
            }),
        }
    }

    packages.sort();

    (packages, findings)
}

/// Read and parse one member manifest, reporting either failure.
#[cfg(test)]
fn read_manifest(
    root: &Path,
    directory: &Path,
    findings: &mut Vec<Finding>,
) -> Option<toml::Table> {
    let relative = manifest_path(directory);
    let text = match fs::read_to_string(root.join(&relative)) {
        Ok(text) => text,
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: relative.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
            return None;
        }
    };

    match toml::from_str::<toml::Table>(&text) {
        Ok(table) => Some(table),
        Err(error) => {
            findings.push(Finding::TraversalFailure {
                path: relative.to_string_lossy().into_owned(),
                message: error.to_string(),
            });
            None
        }
    }
}

/// The manifest path of a member directory, relative to the root.
#[cfg(test)]
fn manifest_path(directory: &Path) -> PathBuf {
    normalize(directory).join("Cargo.toml")
}

/// Drop the `.` a root member writes for itself, so paths join cleanly.
#[cfg(test)]
fn normalize(directory: &Path) -> PathBuf {
    if directory == Path::new(".") {
        PathBuf::new()
    } else {
        directory.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{Package, prefix_for_crate, retiring_read_workspace};

    /// The crate-to-prefix table of ADR-L-015, transcribed for the assertion.
    const ADR_TABLE: &[(&str, &str)] = &[
        ("ember-index", "INDEX"),
        ("ember-assayer", "ASSAYER"),
        ("ember-mudlark", "MUDLARK"),
        ("ember-sentinel", "SENTINEL"),
        ("ember-notime", "NOTIME"),
        ("ember-index-auth-keypair", "INDEXAUTHKEYPAIR"),
        ("cli-common", "CLICOMMON"),
        ("ember-index-config", "INDEXCONFIG"),
        ("ember-index-config-probe", "INDEXCONFIGPROBE"),
        ("ember-index-entry-script", "INDEXENTRYSCRIPT"),
        ("ember-index-health-check", "INDEXHEALTHCHECK"),
        ("linter", "LINTER"),
        ("ember-index-render-text-as-image", "INDEXRENDERTEXTASIMAGE"),
    ];

    /// An owner's prefix is computed from its crate name by one rule — strip the
    /// leading project prefix, remove the hyphens, uppercase the rest — and the
    /// rule yields the registered prefix for every crate the record names. A
    /// prefix is therefore never written by hand at a mint.
    ///
    /// ´claim:owner:a-prefix-is-computed-from-the-crate-name-by-one-rule´
    /// ´test:unit:derives-prefixes-by-stripping-removing-and-uppercasing´
    #[test]
    fn derives_prefixes_by_stripping_removing_and_uppercasing() {
        for (name, expected) in ADR_TABLE {
            let derived = prefix_for_crate("ember-", name).expect("a well-formed prefix");

            assert_eq!(derived.as_str(), *expected, "prefix of `{name}`");
        }
    }

    /// Owners are discovered by reading the manifests on disk, so a package
    /// joins the signature by joining the workspace: every member the manifest
    /// lists is found with the directory it stands in, and the member written as
    /// the current directory normalises to the root itself rather than to a
    /// directory named for a dot.
    ///
    /// ´claim:owner:owners-are-discovered-by-reading-the-manifests-on-disk´
    /// ´test:unit:reads-the-members-a-workspace-manifest-lists´
    #[test]
    fn reads_the_members_a_workspace_manifest_lists() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\".\", \"packages/alpha\"]\n\n[package]\nname = \"ember-fixture\"\n",
        )
        .expect("write");
        fs::create_dir_all(root.path().join("packages/alpha")).expect("create");
        fs::write(
            root.path().join("packages/alpha/Cargo.toml"),
            "[package]\nname = \"ember-alpha\"\n",
        )
        .expect("write");

        let (packages, findings) = retiring_read_workspace(root.path());

        assert!(
            findings.is_empty(),
            "the workspace reads cleanly: {findings:?}"
        );
        assert_eq!(
            packages.len(),
            2,
            "both listed members are discovered: {packages:?}"
        );

        let member = packages
            .iter()
            .find(|package| package.name() == "ember-fixture")
            .expect("the root package");
        assert_eq!(
            member.directory(),
            Path::new(""),
            "the dot member is the root itself"
        );

        let alpha = packages
            .iter()
            .find(|package| package.name() == "ember-alpha")
            .expect("the listed member");
        assert_eq!(alpha.directory(), Path::new("packages/alpha"));
    }

    /// A directory that is simply not a workspace yields no owners and no
    /// complaint: absence of a manifest is an answer, not a defect, so the
    /// linter can be pointed at any tree without inventing diagnostics.
    ///
    /// ´claim:owner:a-root-without-a-manifest-is-no-workspace-and-no-defect´
    /// ´test:unit:reads-a-root-without-a-manifest-as-no-workspace´
    #[test]
    fn reads_a_root_without_a_manifest_as_no_workspace() {
        let root = tempfile::tempdir().expect("temporary directory");

        let (packages, findings) = retiring_read_workspace(root.path());

        assert!(
            packages.is_empty(),
            "no package is discovered: {packages:?}"
        );
        assert!(
            findings.is_empty(),
            "and no defect is reported: {findings:?}"
        );
    }

    /// A manifest that exists but cannot be read is reported rather than passed
    /// over, so owners never go missing silently and a run over a damaged tree
    /// says so instead of reporting a smaller corpus. Here the root manifest is
    /// the unparsable one.
    ///
    /// ´claim:owner:a-manifest-that-cannot-be-read-is-reported-not-skipped´
    /// ´test:unit:reports-an-unparsable-root-manifest´
    #[test]
    fn reports_an_unparsable_root_manifest() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(root.path().join("Cargo.toml"), "[workspace\n").expect("write");

        let (packages, findings) = retiring_read_workspace(root.path());

        assert!(
            packages.is_empty(),
            "no package is discovered: {packages:?}"
        );
        assert_eq!(findings.len(), 1, "one traversal diagnostic: {findings:?}");
    }

    /// A member the workspace declares but whose manifest is absent is reported
    /// too: declaring a member and not shipping it is a defect of the tree.
    ///
    /// (´claim:owner:a-manifest-that-cannot-be-read-is-reported-not-skipped´)
    /// ´test:unit:reports-a-member-whose-manifest-is-missing´
    #[test]
    fn reports_a_member_whose_manifest_is_missing() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"packages/absent\"]\n",
        )
        .expect("write");

        let (packages, findings) = retiring_read_workspace(root.path());

        assert!(
            packages.is_empty(),
            "no package is discovered: {packages:?}"
        );
        assert_eq!(
            findings.len(),
            1,
            "a declared member that is absent is a defect: {findings:?}"
        );
    }

    /// A member whose manifest names no package is reported on the same terms:
    /// an owner with no name could carry no prefix, so the omission is a defect
    /// rather than a package to skip.
    ///
    /// (´claim:owner:a-manifest-that-cannot-be-read-is-reported-not-skipped´)
    /// ´test:unit:reports-a-member-without-a-package-name´
    #[test]
    fn reports_a_member_without_a_package_name() {
        let root = tempfile::tempdir().expect("temporary directory");
        fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"packages/one\"]\n",
        )
        .expect("write");
        fs::create_dir_all(root.path().join("packages/one")).expect("create");
        fs::write(
            root.path().join("packages/one/Cargo.toml"),
            "[dependencies]\n",
        )
        .expect("write");

        let (packages, findings) = retiring_read_workspace(root.path());

        assert!(
            packages.is_empty(),
            "no package is discovered: {packages:?}"
        );
        assert_eq!(findings.len(), 1, "one manifest diagnostic: {findings:?}");
    }

    /// Owners order by crate name rather than by where they sit on disk, so a
    /// report's ordering follows the names a reader knows and does not shift
    /// when a package is moved between directories.
    ///
    /// ´claim:owner:owners-order-by-crate-name´
    /// ´test:unit:orders-discovered-packages-by-crate-name´
    #[test]
    fn orders_discovered_packages_by_crate_name() {
        let first = Package::new("ember-assayer", "packages/assayer");
        let second = Package::new("ember-index", "");

        assert!(first < second);
    }
}
