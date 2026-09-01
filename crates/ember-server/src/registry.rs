//! Immutable hosted-version registry construction and exact selection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ember_legacy::{
    GameFactory, GameKey, HostedManifest, InnerCodec, LegacyIngressFactory, ManifestParseError,
    ManifestValidationError, VersionLimits, VersionLimitsError, parse_hosted_manifest,
    validate_hosted_manifest,
};
use ember_net::outer::{GameNotHosted, VersionNotHosted};

/// One compiled version constructor supplied to the closed registry.
pub struct RegistryRegistration {
    key: GameKey,
    limits: VersionLimits,
    codec: Arc<dyn InnerCodec>,
    factory: Arc<dyn GameFactory>,
    legacy_ingress: Option<Arc<dyn LegacyIngressFactory>>,
}

impl RegistryRegistration {
    /// Creates one registration for a compiled version crate.
    #[must_use]
    pub fn new(
        key: GameKey,
        limits: VersionLimits,
        codec: Arc<dyn InnerCodec>,
        factory: Arc<dyn GameFactory>,
    ) -> Self {
        Self {
            key,
            limits,
            codec,
            factory,
            legacy_ingress: None,
        }
    }

    /// Adds the exact legacy adapter named by this entry's manifest selector.
    #[must_use]
    pub fn with_legacy_ingress(mut self, factory: Arc<dyn LegacyIngressFactory>) -> Self {
        self.legacy_ingress = Some(factory);
        self
    }

    /// Returns the exact game key supplied by the compiled version.
    #[must_use]
    pub const fn key(&self) -> &GameKey {
        &self.key
    }
}

/// Incremental construction of the compiled side of the hosted registry.
#[derive(Default)]
pub struct RegistryBuilder {
    registrations: BTreeMap<GameKey, RegistryRegistration>,
}

impl RegistryBuilder {
    /// Creates an empty registry builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registrations: BTreeMap::new(),
        }
    }

    /// Adds one compiled version registration.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-registration error when the exact key was already added.
    pub fn register(&mut self, registration: RegistryRegistration) -> Result<(), RegistryError> {
        let key = registration.key.clone();
        if self.registrations.insert(key.clone(), registration).is_some() {
            return Err(RegistryError::DuplicateRegistration(key));
        }
        Ok(())
    }

    /// Loads, validates, and joins the product manifest to compiled registrations.
    ///
    /// # Errors
    ///
    /// Returns a read, parse, semantic, limits, or missing-registration failure before a listener
    /// can be bound.
    pub fn load(self, path: &Path) -> Result<Registry, RegistryError> {
        let source = fs::read_to_string(path).map_err(|source| RegistryError::ManifestRead {
            path: path.to_path_buf(),
            source,
        })?;
        self.build_from_source(&source)
    }

    /// Validates and joins an injected manifest to compiled registrations.
    ///
    /// This is the fixture seam; production startup uses [`Self::load`].
    ///
    /// # Errors
    ///
    /// Returns a parse, semantic, limits, or missing-registration failure.
    pub fn build_from_source(self, source: &str) -> Result<Registry, RegistryError> {
        let manifest = parse_hosted_manifest(source).map_err(RegistryError::ManifestParse)?;
        let validation_errors = complete_manifest_validation(&manifest);
        if !validation_errors.is_empty() {
            return Err(RegistryError::ManifestValidation(validation_errors));
        }

        let mut registrations = self.registrations;
        let mut entries = BTreeMap::new();
        let mut legacy_selectors = BTreeMap::new();
        for hosted in manifest.games {
            let key = hosted.game_key();
            let registration = registrations
                .remove(&key)
                .ok_or_else(|| RegistryError::MissingRegistration(key.clone()))?;
            registration
                .limits
                .validate()
                .map_err(|error| RegistryError::InvalidLimits {
                    key: key.clone(),
                    error,
                })?;
            match (&hosted.legacy_game, &registration.legacy_ingress) {
                (Some(_), None) => {
                    return Err(RegistryError::MissingLegacyIngress(key));
                }
                (None, Some(_)) => {
                    return Err(RegistryError::UnexpectedLegacyIngress(key));
                }
                (Some(_), Some(_)) | (None, None) => {}
            }
            if let Some(selector) = &hosted.legacy_game {
                legacy_selectors.insert(selector.clone(), key.clone());
            }
            entries.insert(
                key,
                RegistryEntry {
                    limits: registration.limits,
                    codec: registration.codec,
                    factory: registration.factory,
                    legacy_ingress: registration.legacy_ingress,
                },
            );
        }

        Ok(Registry {
            entries,
            legacy_selectors,
        })
    }
}

/// One fully validated immutable registry entry.
pub(crate) struct RegistryEntry {
    limits: VersionLimits,
    codec: Arc<dyn InnerCodec>,
    factory: Arc<dyn GameFactory>,
    legacy_ingress: Option<Arc<dyn LegacyIngressFactory>>,
}

impl RegistryEntry {
    pub(crate) const fn limits(&self) -> VersionLimits {
        self.limits
    }

    pub(crate) fn codec(&self) -> Arc<dyn InnerCodec> {
        Arc::clone(&self.codec)
    }

    pub(crate) fn factory(&self) -> Arc<dyn GameFactory> {
        Arc::clone(&self.factory)
    }

    pub(crate) fn legacy_ingress(&self) -> Option<Arc<dyn LegacyIngressFactory>> {
        self.legacy_ingress.as_ref().map(Arc::clone)
    }
}

/// Closed immutable map used for every runtime selector lookup.
pub struct Registry {
    entries: BTreeMap<GameKey, RegistryEntry>,
    legacy_selectors: BTreeMap<String, GameKey>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registry")
            .field("keys", &self.entries.keys().collect::<Vec<_>>())
            .field("legacy_selectors", &self.legacy_selectors)
            .finish()
    }
}

impl Registry {
    /// Resolves an exact game and version or returns a live-registry-derived refusal.
    ///
    /// # Errors
    ///
    /// Returns `GameNotHosted` for an unknown game and `VersionNotHosted` for an absent version of
    /// a known game. No nearby or latest version is substituted.
    pub fn exact_key(&self, game_id: &str, game_version: u32) -> Result<GameKey, SelectionError> {
        let key = GameKey {
            game_id: game_id.to_string(),
            game_version,
        };
        if self.entries.contains_key(&key) {
            return Ok(key);
        }

        let hosted_versions_for_game: Vec<_> = self
            .entries
            .keys()
            .filter(|candidate| candidate.game_id == game_id)
            .map(|candidate| candidate.game_version)
            .collect();
        if hosted_versions_for_game.is_empty() {
            return Err(SelectionError::GameNotHosted(GameNotHosted {
                requested_game: game_id.to_string(),
                hosted_games: self.hosted_games(),
            }));
        }
        Err(SelectionError::VersionNotHosted(VersionNotHosted {
            requested_game: game_id.to_string(),
            requested_version: game_version,
            hosted_versions_for_game,
        }))
    }

    /// Returns every hosted game slug in deterministic order.
    #[must_use]
    pub fn hosted_games(&self) -> Vec<String> {
        self.entries
            .keys()
            .map(|key| key.game_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Resolves a closed legacy query selector before the first frame.
    #[must_use]
    pub fn legacy_key(&self, selector: &str) -> Option<GameKey> {
        self.legacy_selectors.get(selector).cloned()
    }

    /// Returns every accepted legacy query selector in deterministic order.
    #[must_use]
    pub fn legacy_selectors(&self) -> BTreeSet<String> {
        self.legacy_selectors.keys().cloned().collect()
    }

    pub(crate) fn legacy_routes(&self) -> BTreeMap<String, GameKey> {
        self.legacy_selectors.clone()
    }

    pub(crate) fn entry(&self, key: &GameKey) -> Option<&RegistryEntry> {
        self.entries.get(key)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&GameKey, &RegistryEntry)> {
        self.entries.iter()
    }
}

/// Structured exact-selection refusal containing only live registry values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionError {
    /// The requested permanent game slug is unknown.
    GameNotHosted(GameNotHosted),
    /// The game exists but the requested exact version does not.
    VersionNotHosted(VersionNotHosted),
}

/// Failure while constructing the immutable registry before listening.
#[derive(Debug)]
pub enum RegistryError {
    /// The same compiled key was registered twice.
    DuplicateRegistration(GameKey),
    /// The product manifest could not be read.
    ManifestRead {
        /// Attempted manifest path.
        path: PathBuf,
        /// Filesystem failure.
        source: std::io::Error,
    },
    /// Manifest TOML syntax or shape was invalid.
    ManifestParse(ManifestParseError),
    /// One or more manifest semantic invariants failed.
    ManifestValidation(Vec<ManifestValidationError>),
    /// A manifest entry has no compiled constructor in this binary.
    MissingRegistration(GameKey),
    /// A manifest legacy selector has no compiled adapter factory.
    MissingLegacyIngress(GameKey),
    /// A compiled adapter factory has no manifest legacy selector.
    UnexpectedLegacyIngress(GameKey),
    /// A compiled entry supplied unusable resource limits.
    InvalidLimits {
        /// Exact invalid entry.
        key: GameKey,
        /// Invalid limit category.
        error: VersionLimitsError,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRegistration(key) => {
                write!(formatter, "duplicate registration: {key:?}")
            }
            Self::ManifestRead { path, source } => {
                write!(formatter, "cannot read hosted manifest {}: {source}", path.display())
            }
            Self::ManifestParse(error) => {
                write!(formatter, "cannot parse hosted manifest: {error}")
            }
            Self::ManifestValidation(errors) => {
                write!(formatter, "hosted manifest validation failed: {errors:?}")
            }
            Self::MissingRegistration(key) => {
                write!(formatter, "manifest entry has no compiled registration: {key:?}")
            }
            Self::MissingLegacyIngress(key) => {
                write!(formatter, "manifest legacy selector has no adapter: {key:?}")
            }
            Self::UnexpectedLegacyIngress(key) => {
                write!(formatter, "compiled legacy adapter has no manifest selector: {key:?}")
            }
            Self::InvalidLimits { key, error } => {
                write!(formatter, "invalid version limits for {key:?}: {error:?}")
            }
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManifestRead { source, .. } => Some(source),
            Self::ManifestParse(source) => Some(source),
            Self::DuplicateRegistration(_)
            | Self::ManifestValidation(_)
            | Self::MissingRegistration(_)
            | Self::MissingLegacyIngress(_)
            | Self::UnexpectedLegacyIngress(_)
            | Self::InvalidLimits { .. } => None,
        }
    }
}

fn complete_manifest_validation(manifest: &HostedManifest) -> Vec<ManifestValidationError> {
    let mut errors = validate_hosted_manifest(manifest).err().unwrap_or_default();
    let mut keys = BTreeSet::new();
    let mut latest_games = BTreeSet::new();
    let mut legacy_selectors = BTreeSet::new();

    for hosted in &manifest.games {
        let key = hosted.game_key();
        if !keys.insert(key.clone()) {
            push_unique(
                &mut errors,
                ManifestValidationError::DuplicateGameKey(key.clone()),
            );
        }
        if hosted.latest && !latest_games.insert(hosted.game_id.clone()) {
            push_unique(
                &mut errors,
                ManifestValidationError::MultipleLatest {
                    game_id: hosted.game_id.clone(),
                },
            );
        }
        if hosted.package.is_empty() {
            push_unique(
                &mut errors,
                ManifestValidationError::MissingPackage(key.clone()),
            );
        }
        if hosted.limits_profile.is_empty() {
            push_unique(
                &mut errors,
                ManifestValidationError::MissingLimitsProfile(key.clone()),
            );
        }
        if hosted.fixture_suite.is_empty() {
            push_unique(
                &mut errors,
                ManifestValidationError::MissingFixtureSuite(key.clone()),
            );
        }
        if let Some(selector) = &hosted.legacy_game {
            if !ember_legacy::is_valid_legacy_selector(selector) {
                push_unique(
                    &mut errors,
                    ManifestValidationError::InvalidLegacySelector {
                        game_key: key,
                        selector: selector.clone(),
                    },
                );
            } else if !legacy_selectors.insert(selector.clone()) {
                push_unique(
                    &mut errors,
                    ManifestValidationError::DuplicateLegacySelector {
                        selector: selector.clone(),
                    },
                );
            }
        }
    }
    errors
}

fn push_unique(errors: &mut Vec<ManifestValidationError>, error: ManifestValidationError) {
    if !errors.contains(&error) {
        errors.push(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    fn builder() -> RegistryBuilder {
        let mut builder = RegistryBuilder::new();
        fixture::register(&mut builder).unwrap();
        builder
    }

    #[test]
    fn registry_reports_every_required_manifest_validation_class() {
        let source = r#"
[[games]]
game_id = "fixture"
game_version = 1
package = ""
latest = true
limits_profile = ""
fixture_suite = ""
legacy_game = "BAD selector"

[[games]]
game_id = "fixture"
game_version = 1
package = "duplicate"
latest = true
limits_profile = "limits"
fixture_suite = "fixtures"
legacy_game = "arena"
"#;
        let RegistryError::ManifestValidation(errors) = builder()
            .build_from_source(source)
            .expect_err("invalid manifest must fail before registry construction")
        else {
            panic!("expected semantic manifest validation failure");
        };
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::DuplicateGameKey(_)
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::MultipleLatest { .. }
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::MissingPackage(_)
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::MissingLimitsProfile(_)
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::MissingFixtureSuite(_)
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::InvalidLegacySelector { .. }
        )));
    }

    #[test]
    fn duplicate_legacy_selector_is_rejected() {
        let source = r#"
[[games]]
game_id = "fixture"
game_version = 1
package = "fixture-one"
latest = true
limits_profile = "limits"
fixture_suite = "fixtures"
legacy_game = "old"

[[games]]
game_id = "second"
game_version = 1
package = "fixture-two"
latest = true
limits_profile = "limits"
fixture_suite = "fixtures"
legacy_game = "old"
"#;
        let RegistryError::ManifestValidation(errors) = builder()
            .build_from_source(source)
            .expect_err("duplicate selector must fail before registrations are joined")
        else {
            panic!("expected semantic manifest validation failure");
        };
        assert!(errors.iter().any(|error| matches!(
            error,
            ManifestValidationError::DuplicateLegacySelector { selector } if selector == "old"
        )));
    }

    #[test]
    fn missing_compiled_package_registration_fails_startup() {
        let error = RegistryBuilder::new()
            .build_from_source(fixture::MANIFEST)
            .expect_err("manifest entry without compiled registration must fail");
        assert!(matches!(error, RegistryError::MissingRegistration(_)));
    }

    #[test]
    fn manifest_legacy_selector_requires_compiled_adapter() {
        let source = fixture::MANIFEST.replace(
            "fixture_suite = \"fixture-hosted-contract\"",
            "fixture_suite = \"fixture-hosted-contract\"\nlegacy_game = \"fixture\"",
        );
        let error = builder()
            .build_from_source(&source)
            .expect_err("manifest legacy selector without an adapter must fail");
        assert!(matches!(error, RegistryError::MissingLegacyIngress(_)));
    }

    #[test]
    fn invalid_version_limits_fail_startup() {
        let limits = VersionLimits {
            max_frame_bytes: 0,
            ..fixture::fixture_limits()
        };
        let mut builder = RegistryBuilder::new();
        builder
            .register(RegistryRegistration::new(
                fixture::fixture_key(),
                limits,
                Arc::new(TestCodec),
                Arc::new(TestFactory),
            ))
            .unwrap();
        assert!(matches!(
            builder.build_from_source(fixture::MANIFEST),
            Err(RegistryError::InvalidLimits { .. })
        ));
    }

    #[test]
    fn exact_selection_refusals_contain_live_registry_values() {
        let registry = builder().build_from_source(fixture::MANIFEST).unwrap();
        assert_eq!(
            registry.exact_key("fixture", 99),
            Err(SelectionError::VersionNotHosted(VersionNotHosted {
                requested_game: "fixture".to_string(),
                requested_version: 99,
                hosted_versions_for_game: vec![1],
            }))
        );
        assert_eq!(
            registry.exact_key("unknown", 1),
            Err(SelectionError::GameNotHosted(GameNotHosted {
                requested_game: "unknown".to_string(),
                hosted_games: vec!["fixture".to_string()],
            }))
        );
    }

    struct TestCodec;

    impl InnerCodec for TestCodec {
        fn decode(
            &self,
            frame: &ember_legacy::InnerFrame,
        ) -> Result<ember_legacy::DecodedInput, ember_legacy::InnerCodecError> {
            Ok(ember_legacy::DecodedInput {
                payload: match frame {
                    ember_legacy::InnerFrame::Text(text) => text.as_bytes().to_vec(),
                    ember_legacy::InnerFrame::Binary(bytes) => bytes.clone(),
                },
            })
        }

        fn encode(
            &self,
            event: &ember_legacy::EncodedEvent,
        ) -> Result<ember_legacy::InnerFrame, ember_legacy::InnerCodecError> {
            Ok(ember_legacy::InnerFrame::Binary(event.payload.clone()))
        }
    }

    struct TestFactory;

    impl GameFactory for TestFactory {
        fn create(
            &self,
            _capabilities: &ember_legacy::LegacyCapabilities,
            _creation: &ember_legacy::SessionCreationData,
        ) -> Result<Box<dyn ember_legacy::GameSession>, ember_legacy::FactoryError> {
            Err(ember_legacy::FactoryError::ConstructionFailed(
                "not used by registry validation".to_string(),
            ))
        }
    }
}
