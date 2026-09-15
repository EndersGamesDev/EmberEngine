//! Deterministic TypeScript declarations and JSON schemas from Rust boundaries.

use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use ember_boundary::models::{GameCatalog, HostBook, Mirror};
use ember_boundary::{Description, Direction, Field, Shape, TypeRef, VariantShape, View, Wide};
use minijinja::{Environment, UndefinedBehavior, context};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Error type used by generation and file emission.
pub type WebgenResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const DECLARATION_TEMPLATE: &str = include_str!("../templates/declaration.d.ts.j2");
const EXHAUSTIVE_TEMPLATE: &str = include_str!("../templates/exhaustive-consumer.ts.j2");
const WASM_BINDGEN_COMPAT_TEMPLATE: &str = include_str!("../templates/wasm-bindgen-compat.d.ts.j2");
const HOSTS_TEMPLATE: &str = include_str!("../../../web/hosts.ts.j2");
const LOADER_TEMPLATE: &str = include_str!("../../../web/loader.ts.j2");
const HOSTS_TEST_TEMPLATE: &str = include_str!("../../../web/hosts.test.mts.j2");
const LOADER_TEST_TEMPLATE: &str = include_str!("../../../web/loader.test.mts.j2");
const CHECK_HOSTS_TEMPLATE: &str = include_str!("../../../deploy/check-hosts.mts.j2");
const CHECK_HOSTS_TEST_TEMPLATE: &str = include_str!("../../../deploy/check-hosts.test.mts.j2");
const FIRE_RACE_TEMPLATE: &str = include_str!("../../../web/games/fire/v2/race.ts.j2");
const FIRE_GARAGE_TEMPLATE: &str = include_str!("../../../web/games/fire/v2/garage.ts.j2");
const FIRE_TEST_TEMPLATE: &str = include_str!("../../../web/games/fire/v2/fire.test.mts.j2");
const CORE_TEMPLATES: [(&str, &str); 3] = [
    ("declaration", DECLARATION_TEMPLATE),
    ("exhaustive", EXHAUSTIVE_TEMPLATE),
    ("wasm-bindgen-compat", WASM_BINDGEN_COMPAT_TEMPLATE),
];

#[derive(Clone, Copy)]
struct BoundaryImport {
    module: &'static str,
    specifier: &'static str,
    names: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct BehaviourTemplate {
    name: &'static str,
    source_path: &'static str,
    output_path: &'static str,
    source: &'static str,
    imports: &'static [BoundaryImport],
}

const BEHAVIOUR_TEMPLATES: [BehaviourTemplate; 9] = [
    BehaviourTemplate {
        name: "hosts",
        source_path: "web/hosts.ts.j2",
        output_path: "ts/hosts.ts",
        source: HOSTS_TEMPLATE,
        imports: &[BoundaryImport {
            module: "ember-boundary",
            specifier: "./ember-boundary.js",
            names: &["HostBook", "HostEntry", "Mirror", "ServerGameId"],
        }],
    },
    BehaviourTemplate {
        name: "loader",
        source_path: "web/loader.ts.j2",
        output_path: "ts/loader.ts",
        source: LOADER_TEMPLATE,
        imports: &[
            BoundaryImport {
                module: "ember-loader",
                specifier: "./ember-loader.js",
                names: &["Phase", "Status", "Event", "EventOutput"],
            },
            BoundaryImport {
                module: "ember-boundary",
                specifier: "./ember-boundary.js",
                names: &["GameCatalog", "GameRelease"],
            },
        ],
    },
    BehaviourTemplate {
        name: "hosts-test",
        source_path: "web/hosts.test.mts.j2",
        output_path: "node-ts/hosts.test.mts",
        source: HOSTS_TEST_TEMPLATE,
        imports: &[BoundaryImport {
            module: "ember-boundary",
            specifier: "../ts/ember-boundary.js",
            names: &[
                "GameCatalog",
                "GameRelease",
                "HostBook",
                "HostEntry",
                "Mirror",
                "ServerGameId",
            ],
        }],
    },
    BehaviourTemplate {
        name: "loader-test",
        source_path: "web/loader.test.mts.j2",
        output_path: "node-ts/loader.test.mts",
        source: LOADER_TEST_TEMPLATE,
        imports: &[
            BoundaryImport {
                module: "ember-loader",
                specifier: "../ts/ember-loader.js",
                names: &["Phase", "Status", "Event", "EventOutput"],
            },
            BoundaryImport {
                module: "ember-boundary",
                specifier: "../ts/ember-boundary.js",
                names: &["GameCatalog", "GameRelease"],
            },
        ],
    },
    BehaviourTemplate {
        name: "check-hosts",
        source_path: "deploy/check-hosts.mts.j2",
        output_path: "node-ts/check-hosts.mts",
        source: CHECK_HOSTS_TEMPLATE,
        imports: &[BoundaryImport {
            module: "ember-boundary",
            specifier: "../ts/ember-boundary.js",
            names: &[
                "GameCatalog",
                "GameRelease",
                "HostBook",
                "HostEntry",
                "Mirror",
                "ServerGameId",
            ],
        }],
    },
    BehaviourTemplate {
        name: "check-hosts-test",
        source_path: "deploy/check-hosts.test.mts.j2",
        output_path: "node-ts/check-hosts.test.mts",
        source: CHECK_HOSTS_TEST_TEMPLATE,
        imports: &[BoundaryImport {
            module: "ember-boundary",
            specifier: "../ts/ember-boundary.js",
            names: &[
                "GameCatalog",
                "GameRelease",
                "HostBook",
                "HostEntry",
                "Mirror",
                "ServerGameId",
            ],
        }],
    },
    BehaviourTemplate {
        name: "fire-race",
        source_path: "web/games/fire/v2/race.ts.j2",
        output_path: "ts/games/fire/v2/race.ts",
        source: FIRE_RACE_TEMPLATE,
        imports: &[
            BoundaryImport {
                module: "fire-core",
                specifier: "../../../fire-core.js",
                names: &["C2SOutput", "LobbyInfoInput", "S2CInput"],
            },
            BoundaryImport {
                module: "ember-loader",
                specifier: "../../../ember-loader.js",
                names: &["Event"],
            },
        ],
    },
    BehaviourTemplate {
        name: "fire-garage",
        source_path: "web/games/fire/v2/garage.ts.j2",
        output_path: "ts/games/fire/v2/garage.ts",
        source: FIRE_GARAGE_TEMPLATE,
        imports: &[],
    },
    BehaviourTemplate {
        name: "fire-test",
        source_path: "web/games/fire/v2/fire.test.mts.j2",
        output_path: "node-ts/fire.test.mts",
        source: FIRE_TEST_TEMPLATE,
        imports: &[BoundaryImport {
            module: "fire-core",
            specifier: "../ts/fire-core.js",
            names: &["S2CInput"],
        }],
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
/// One generated artifact relative to the SHA-scoped output root.
pub struct RenderedFile {
    /// Stable slash-separated path.
    pub path: String,
    /// Exact bytes written to disk.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// All artifacts rendered from one source identifier.
pub struct RenderedBundle {
    /// Source commit or fixed test identifier.
    pub source_sha: String,
    /// Hash of the registered production template names and sources.
    pub template_source_hash: String,
    /// Sorted fields containing 64-bit integers and their declarations.
    pub integer_64_fields: Vec<Integer64Field>,
    /// Emitted files, excluding the self-referential manifest.
    pub files: Vec<RenderedFile>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Script representation selected for one 64-bit boundary field.
pub enum Integer64Representation {
    /// Preserve all identity bits as a TypeScript `bigint`.
    Exact,
    /// Use a TypeScript `number` within a declared bound.
    Precise,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// One generated 64-bit field declaration.
pub struct Integer64Field {
    /// Stable module, type, optional variant and field path.
    pub path: String,
    /// TypeScript representation selected by the Rust field.
    pub representation: Integer64Representation,
    /// Precision premise for `Precise`, or null for `Exact`.
    pub bound: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// Provenance for a generated file.
pub struct ManifestFile {
    /// SHA-scoped file location.
    pub path: String,
    /// Fixed compiler-input location, when this file is compiled.
    pub compiler_path: Option<String>,
    /// Exact byte length.
    pub bytes: usize,
    /// Lower-case SHA-256 digest.
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// Provenance manifest written beside a generated bundle.
pub struct Manifest {
    /// Source commit or fixed test identifier.
    pub source_sha: String,
    /// SHA-scoped output root.
    pub source_root: String,
    /// Stable compiler-input root refreshed by this invocation.
    pub compiler_root: String,
    /// Stable Node compiler-input root refreshed by this invocation.
    pub node_compiler_root: String,
    /// Hash of every registered production template.
    pub template_source_hash: String,
    /// Sorted fields containing 64-bit integers and their declarations.
    pub integer_64_fields: Vec<Integer64Field>,
    /// Every emitted file except this manifest.
    pub files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
/// Mapping from one rendered TypeScript input back to its tracked template.
pub struct RenderedLineMap {
    /// Mapping format version.
    pub version: u8,
    /// Slash-separated path relative to the generated bundle root.
    pub rendered_path: String,
    /// Canonical tracked template path.
    pub template_path: String,
    /// Template line for each rendered line, or null for generated preamble.
    pub lines: Vec<Option<usize>>,
}

#[derive(Serialize)]
struct BehaviourImportContext {
    module: String,
    names: Vec<String>,
}

#[derive(Serialize)]
struct DeclarationContext {
    name: String,
    input: String,
    output: String,
    canonical: String,
}

#[derive(Serialize)]
struct UnionContext {
    type_name: String,
    alias: String,
    module: String,
    function_name: String,
    discriminant: String,
    cases: Vec<String>,
}

struct Feed {
    module: &'static str,
    descriptions: &'static [&'static Description],
}

/// Return the stable hash of all production template names and bytes.
#[must_use]
pub fn template_source_hash() -> String {
    let mut hasher = Sha256::new();
    for (name, source) in CORE_TEMPLATES {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(source.as_bytes());
        hasher.update([u8::MAX]);
    }
    for template in BEHAVIOUR_TEMPLATES {
        hasher.update(template.name.as_bytes());
        hasher.update([0]);
        hasher.update(template.source_path.as_bytes());
        hasher.update([0]);
        hasher.update(template.source.as_bytes());
        hasher.update([u8::MAX]);
    }
    hex(&hasher.finalize())
}

/// Render every production declaration, schema and exhaustive consumer.
///
/// # Errors
///
/// Returns an error when the source identifier is empty, a registration feed
/// omits a referenced named type, or a strict template cannot render.
pub fn render(source_sha: &str, game_ids: &[String]) -> WebgenResult<RenderedBundle> {
    if source_sha.is_empty() {
        return Err("source SHA is empty".into());
    }
    let feeds = feeds();
    validate_feeds(&feeds)?;
    let integer_64_fields = integer_64_fields(&feeds)?;
    let environment = environment()?;
    let game_ids = game_ids
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let server_game_ids = server_game_ids(&feeds, &game_ids)?;
    let game_ids = game_ids
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    let server_game_ids = server_game_ids
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    let mut files = Vec::new();
    for feed in &feeds {
        let declarations = feed
            .descriptions
            .iter()
            .map(|description| declaration_context(description))
            .collect::<Vec<_>>();
        let feed_game_ids: &[String] = if feed.module == "ember-boundary" {
            &game_ids
        } else {
            &[]
        };
        let feed_server_game_ids: &[String] = if feed.module == "ember-boundary" {
            &server_game_ids
        } else {
            &[]
        };
        let rendered = environment.get_template("declaration")?.render(context! {
            declarations,
            game_ids => feed_game_ids,
            server_game_ids => feed_server_game_ids,
        })?;
        files.push(RenderedFile {
            path: format!("ts/{}.d.ts", feed.module),
            bytes: clean_rendered(&rendered),
        });
    }
    let unions = union_contexts(&feeds);
    let exhaustive = environment
        .get_template("exhaustive")?
        .render(context! { unions })?;
    files.push(RenderedFile {
        path: "ts/exhaustive-consumer.ts".into(),
        bytes: clean_rendered(&exhaustive),
    });
    let wasm_bindgen_compat = environment
        .get_template("wasm-bindgen-compat")?
        .render(context! {})?;
    files.push(RenderedFile {
        path: "ts/wasm-bindgen-compat.d.ts".into(),
        bytes: clean_rendered(&wasm_bindgen_compat),
    });
    files.extend(render_behaviour_templates(
        &environment,
        &feeds,
        source_sha,
    )?);
    files.extend(schema_files()?);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(RenderedBundle {
        source_sha: source_sha.into(),
        template_source_hash: template_source_hash(),
        integer_64_fields,
        files,
    })
}

/// Write a bundle to its SHA-scoped root and refresh the fixed compiler tree.
///
/// Existing `ts` and `schema` children are replaced, while a sibling
/// `pkg-types` directory staged by wasm-bindgen is retained.
///
/// # Errors
///
/// Returns an error for an unsafe output layout or any failed filesystem or
/// manifest serialization operation.
pub fn write(out: &Path, bundle: &RenderedBundle) -> WebgenResult<Manifest> {
    let parent = out
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or("output must have a parent directory")?;
    if out.file_name().and_then(|name| name.to_str()) == Some("ts") {
        return Err("SHA-scoped output cannot be the fixed ts directory".into());
    }
    let compiler_root = parent.join("ts");
    let node_compiler_root = parent.join("node-ts");
    remove_dir(&out.join("ts"))?;
    remove_dir(&out.join("node-ts"))?;
    remove_dir(&out.join("schema"))?;
    remove_dir(&compiler_root)?;
    remove_dir(&node_compiler_root)?;
    remove_file(&out.join("manifest.json"))?;

    let mut manifest_files = Vec::with_capacity(bundle.files.len());
    for file in &bundle.files {
        let relative = Path::new(&file.path);
        let destination = out.join(relative);
        write_file(&destination, &file.bytes)?;
        let compiler_path = relative
            .strip_prefix("ts")
            .ok()
            .map(|compiler_relative| compiler_root.join(compiler_relative))
            .or_else(|| {
                relative
                    .strip_prefix("node-ts")
                    .ok()
                    .map(|compiler_relative| node_compiler_root.join(compiler_relative))
            });
        let compiler_display = if let Some(fixed) = &compiler_path {
            write_file(fixed, &file.bytes)?;
            Some(path_string(fixed))
        } else {
            None
        };
        manifest_files.push(ManifestFile {
            path: path_string(&destination),
            compiler_path: compiler_display,
            bytes: file.bytes.len(),
            sha256: sha256(&file.bytes),
        });
    }
    let manifest = Manifest {
        source_sha: bundle.source_sha.clone(),
        source_root: path_string(out),
        compiler_root: path_string(&compiler_root),
        node_compiler_root: path_string(&node_compiler_root),
        template_source_hash: bundle.template_source_hash.clone(),
        integer_64_fields: bundle.integer_64_fields.clone(),
        files: manifest_files,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    write_file(&out.join("manifest.json"), &bytes)?;
    Ok(manifest)
}

/// Parse and validate the real game catalog, returning its sorted identifiers.
///
/// # Errors
///
/// Returns an error when the file cannot be read, is not a valid catalog, or
/// contains a duplicate or empty game identifier.
pub fn game_ids(path: &Path) -> WebgenResult<Vec<String>> {
    let catalog: GameCatalog = serde_json::from_slice(&fs::read(path)?)?;
    let mut ids = catalog
        .games
        .into_iter()
        .map(|game| game.id)
        .collect::<Vec<_>>();
    ids.sort();
    if ids.iter().any(String::is_empty) {
        return Err("catalog contains an empty game id".into());
    }
    let before = ids.len();
    ids.dedup();
    if ids.len() != before {
        return Err("catalog contains duplicate game ids".into());
    }
    Ok(ids)
}

/// Map TypeScript diagnostics for rendered inputs back to tracked templates.
///
/// Diagnostics without a readable adjacent line map are returned unchanged.
/// A mapped diagnostic retains its rendered coordinate in brackets.
///
/// # Errors
///
/// Returns an error when an adjacent map exists but is unreadable or invalid.
pub fn map_diagnostics(diagnostics: &str) -> WebgenResult<String> {
    let mut output = String::new();
    for line in diagnostics.lines() {
        if let Some(mapped) = map_diagnostic_line(line)? {
            writeln!(output, "{mapped}")?;
        } else {
            writeln!(output, "{line}")?;
        }
    }
    Ok(output)
}

fn map_diagnostic_line(line: &str) -> WebgenResult<Option<String>> {
    let Some(coordinate_end) = line.find("): ") else {
        return Ok(None);
    };
    let Some(coordinate_start) = line[..coordinate_end].rfind('(') else {
        return Ok(None);
    };
    let rendered_path = &line[..coordinate_start];
    let Some((rendered_line, rendered_column)) =
        parse_coordinate(&line[coordinate_start + 1..coordinate_end])
    else {
        return Ok(None);
    };
    let map_path = PathBuf::from(format!("{rendered_path}.map.json"));
    let map_bytes = match fs::read(&map_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let line_map: RenderedLineMap = serde_json::from_slice(&map_bytes)?;
    if line_map.version != 1 {
        return Err(format!(
            "unsupported rendered line-map version in {}",
            path_string(&map_path)
        )
        .into());
    }
    let Some(Some(template_line)) = line_map.lines.get(rendered_line.saturating_sub(1)) else {
        return Ok(None);
    };
    let message = &line[coordinate_end + 3..];
    Ok(Some(format!(
        "{}({template_line},{rendered_column}): {message} [rendered {rendered_path}({rendered_line},{rendered_column})]",
        line_map.template_path
    )))
}

fn parse_coordinate(value: &str) -> Option<(usize, usize)> {
    let (line, column) = value.split_once(',')?;
    Some((line.parse().ok()?, column.parse().ok()?))
}

/// Return every generated union and its descriptor-owned variant count.
#[must_use]
pub fn union_variant_counts() -> Vec<(String, String, usize)> {
    feeds()
        .into_iter()
        .flat_map(|feed| {
            feed.descriptions
                .iter()
                .filter(|description| description.variant_count() != 0)
                .map(move |description| {
                    (
                        feed.module.into(),
                        description.name.into(),
                        description.variant_count(),
                    )
                })
        })
        .collect()
}

const fn feeds() -> [Feed; 6] {
    [
        Feed {
            module: "arena-core",
            descriptions: arena_core::proto::boundary_descriptions(),
        },
        Feed {
            module: "fire-core",
            descriptions: fire_core::proto::boundary_descriptions(),
        },
        Feed {
            module: "kings-core",
            descriptions: kings_core::proto::boundary_descriptions(),
        },
        Feed {
            module: "league-core",
            descriptions: league_core::proto::boundary_descriptions(),
        },
        Feed {
            module: "ember-loader",
            descriptions: ember_loader::boundary_descriptions(),
        },
        Feed {
            module: "ember-boundary",
            descriptions: ember_boundary::models::boundary_descriptions(),
        },
    ]
}

fn environment() -> WebgenResult<Environment<'static>> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    for (name, source) in CORE_TEMPLATES {
        environment.add_template(name, source)?;
    }
    for template in BEHAVIOUR_TEMPLATES {
        environment.add_template(template.name, template.source)?;
    }
    Ok(environment)
}

fn render_behaviour_templates(
    environment: &Environment<'_>,
    feeds: &[Feed],
    source_sha: &str,
) -> WebgenResult<Vec<RenderedFile>> {
    let mut files = Vec::new();
    for template in BEHAVIOUR_TEMPLATES {
        validate_behaviour_template(&template, feeds)?;
        let imports = template
            .imports
            .iter()
            .map(|boundary_import| BehaviourImportContext {
                module: boundary_import.module.into(),
                names: boundary_import
                    .names
                    .iter()
                    .map(|name| (*name).into())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let rendered_body = environment
            .get_template(template.name)?
            .render(context! { imports, source_sha })?;
        if rendered_body.lines().count() != template.source.lines().count() {
            return Err(format!(
                "behaviour template {} must preserve its source line count",
                template.source_path
            )
            .into());
        }
        let mut rendered = format!(
            "// Generated from {} at {source_sha}. Do not edit.\n",
            template.source_path
        );
        for boundary_import in template.imports {
            let names = boundary_import.names.join(", ");
            let _ = writeln!(
                rendered,
                "import type {{ {names} }} from '{}';",
                boundary_import.specifier
            );
        }
        rendered.push('\n');
        rendered.push_str(&rendered_body);
        let rendered_bytes = clean_rendered(&rendered);
        let rendered_line_count = rendered_bytes.split(|byte| *byte == b'\n').count() - 1;
        let preamble_lines = 2 + template.imports.len();
        let mut lines = vec![None; preamble_lines];
        lines.extend((1..=template.source.lines().count()).map(Some));
        if lines.len() != rendered_line_count {
            return Err(format!(
                "behaviour template {} produced an inconsistent line map",
                template.source_path
            )
            .into());
        }
        let line_map = RenderedLineMap {
            version: 1,
            rendered_path: template.output_path.into(),
            template_path: template.source_path.into(),
            lines,
        };
        let mut line_map_bytes = serde_json::to_vec_pretty(&line_map)?;
        line_map_bytes.push(b'\n');
        files.push(RenderedFile {
            path: template.output_path.into(),
            bytes: rendered_bytes,
        });
        files.push(RenderedFile {
            path: format!("{}.map.json", template.output_path),
            bytes: line_map_bytes,
        });
    }
    Ok(files)
}

fn validate_behaviour_template(template: &BehaviourTemplate, feeds: &[Feed]) -> WebgenResult<()> {
    if template.source.contains("{%")
        || template.source.contains("{#")
        || template.source.contains("{{")
    {
        return Err(format!(
            "behaviour template {} may not contain Minijinja constructs",
            template.source_path
        )
        .into());
    }
    for boundary_import in template.imports {
        let feed = feeds
            .iter()
            .find(|feed| feed.module == boundary_import.module)
            .ok_or_else(|| format!("unknown boundary module {}", boundary_import.module))?;
        let available = available_import_names(feed);
        for name in boundary_import.names {
            if !available.contains(*name) {
                return Err(format!(
                    "{} cannot import unknown {} type {name}",
                    template.source_path, boundary_import.module
                )
                .into());
            }
        }
    }
    Ok(())
}

fn available_import_names(feed: &Feed) -> HashSet<String> {
    let mut names = HashSet::new();
    for description in feed.descriptions {
        names.insert(description.name.into());
        names.insert(format!("{}Input", description.name));
        names.insert(format!("{}Output", description.name));
    }
    if feed.module == "ember-boundary" {
        for name in [
            "GameId",
            "ServerGameId",
            "GameHostAddressKey",
            "GameHostProtocolKey",
            "GameHostVersionKey",
            "GameHostCommitKey",
        ] {
            names.insert(name.into());
        }
    }
    names
}

fn validate_feeds(feeds: &[Feed]) -> WebgenResult<()> {
    for feed in feeds {
        let names = feed
            .descriptions
            .iter()
            .map(|description| description.name)
            .collect::<HashSet<_>>();
        for description in feed.descriptions {
            visit_description(description, &mut |ty| {
                if let TypeRef::Named { name, .. } = ty
                    && !names.contains(name)
                {
                    return Err(format!(
                        "{} registration omits named boundary type {name}",
                        feed.module
                    )
                    .into());
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn declaration_context(description: &Description) -> DeclarationContext {
    let canonical = match description.direction {
        Direction::Input | Direction::Both => format!("{}Input", description.name),
        Direction::Output => format!("{}Output", description.name),
    };
    DeclarationContext {
        name: description.name.into(),
        input: shape_type(description, View::Input),
        output: shape_type(description, View::Output),
        canonical,
    }
}

fn shape_type(description: &Description, view: View) -> String {
    match description.shape {
        Shape::Object(fields) => object_type(fields, &[], view),
        Shape::Enum { tag, variants } => {
            let body = variants
                .iter()
                .map(|variant| variant_type(tag, variant, view))
                .collect::<Vec<_>>()
                .join("\n  | ");
            format!("\n  | {body}")
        }
    }
}

fn variant_type(tag: Option<&str>, variant: &ember_boundary::Variant, view: View) -> String {
    match (tag, variant.shape) {
        (None, VariantShape::Unit) => quoted(variant.name),
        (Some(tag), VariantShape::Unit) => object_type(&[], &[(tag, variant.name)], view),
        (Some(tag), VariantShape::Object(fields)) => {
            object_type(fields, &[(tag, variant.name)], view)
        }
        (Some(tag), VariantShape::Intersection(ty)) => {
            format!(
                "{} & {}",
                object_type(&[], &[(tag, variant.name)], view),
                type_ref(ty, view)
            )
        }
        _ => unreachable!("the derive rejects unsupported enum representations"),
    }
}

fn object_type(fields: &[Field], tags: &[(&str, &str)], view: View) -> String {
    let mut output = String::from("{");
    for &(name, value) in tags {
        let _ = write!(output, "\n  {}: {};", quoted(name), quoted(value));
    }
    for field in fields {
        match declared_wide(field.ty, field.name).expect("production feeds were validated") {
            Some(Wide::Exact) => output.push_str(
                "\n  /** Exact 64-bit integer identity; parse JSON without a JavaScript number intermediate. */",
            ),
            Some(Wide::Precise { bound }) => {
                let _ = write!(
                    output,
                    "\n  /** Precise 64-bit integer represented as number; bound: {bound}. */"
                );
            }
            None => {}
        }
        let optional = match view {
            View::Input => field.input_optional,
            View::Output => field.output_optional,
        };
        let marker = if optional { "?" } else { "" };
        let _ = write!(
            output,
            "\n  {}{marker}: {};",
            quoted(field.name),
            field_type(field, view)
        );
    }
    output.push_str("\n}");
    output
}

fn field_type(field: &Field, view: View) -> String {
    if matches!(view, View::Output)
        && field.output_optional
        && let TypeRef::Nullable(inner) = field.ty
    {
        return type_ref(*inner, view);
    }
    type_ref(field.ty, view)
}

fn server_game_ids(feeds: &[Feed], game_ids: &[String]) -> WebgenResult<Vec<String>> {
    let host_entry = feeds
        .iter()
        .find(|feed| feed.module == "ember-boundary")
        .and_then(|feed| {
            feed.descriptions
                .iter()
                .find(|description| description.name == "HostEntry")
        })
        .ok_or("ember-boundary registration omits HostEntry")?;
    let Shape::Object(fields) = host_entry.shape else {
        return Err("HostEntry boundary must be an object".into());
    };
    Ok(game_ids
        .iter()
        .filter(|game_id| {
            ["ws", "proto", "version", "commit"].iter().all(|suffix| {
                let field_name = if game_id.as_str() == "arena" {
                    (*suffix).to_owned()
                } else {
                    format!("{game_id}_{suffix}")
                };
                fields.iter().any(|field| field.name == field_name)
            })
        })
        .cloned()
        .collect())
}

fn type_ref(ty: TypeRef, view: View) -> String {
    match ty {
        TypeRef::Bool => "boolean".into(),
        TypeRef::String => "string".into(),
        TypeRef::Integer {
            bits: 64,
            wide: Some(Wide::Exact),
            ..
        } => "bigint".into(),
        TypeRef::Integer {
            bits: 64,
            wide: None,
            ..
        } => unreachable!("production feeds were validated"),
        TypeRef::Integer { .. } | TypeRef::Number { .. } => "number".into(),
        TypeRef::Nullable(inner) => format!("{} | null", type_ref(*inner, view)),
        TypeRef::Sequence(inner) => format!("Array<{}>", type_ref(*inner, view)),
        TypeRef::Array(inner, length) => {
            let items = vec![type_ref(*inner, view); length].join(", ");
            format!("[{items}]")
        }
        TypeRef::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| type_ref(*item, view))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{items}]")
        }
        TypeRef::Named { name, .. } => format!("{name}{}", view_suffix(view)),
    }
}

const fn view_suffix(view: View) -> &'static str {
    match view {
        View::Input => "Input",
        View::Output => "Output",
    }
}

fn union_contexts(feeds: &[Feed]) -> Vec<UnionContext> {
    feeds
        .iter()
        .flat_map(|feed| {
            feed.descriptions.iter().filter_map(|description| {
                let Shape::Enum { tag, variants } = description.shape else {
                    return None;
                };
                let prefix = identifier(feed.module);
                Some(UnionContext {
                    type_name: description.name.into(),
                    alias: format!("{prefix}{}", description.name),
                    module: feed.module.into(),
                    function_name: format!("check{prefix}{}", description.name),
                    discriminant: tag
                        .map_or_else(|| "value".into(), |tag| format!("value[{}]", quoted(tag))),
                    cases: variants
                        .iter()
                        .map(|variant| quoted(variant.name))
                        .collect(),
                })
            })
        })
        .collect()
}

fn identifier(value: &str) -> String {
    let mut output = String::new();
    for part in value.split(|character: char| !character.is_ascii_alphanumeric()) {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.extend(characters);
        }
    }
    output
}

fn schema_files() -> WebgenResult<Vec<RenderedFile>> {
    let games = ember_boundary::json_schema::<GameCatalog>(View::Input);
    let server = ember_boundary::json_schema::<HostBook>(View::Input);
    let mut mirror = ember_boundary::json_schema::<Mirror>(View::Input);
    if let Value::Object(object) = &mut mirror {
        object.remove("$schema");
        object.remove("title");
    }
    let mirrors = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MirrorList",
        "type": "array",
        "items": mirror
    });
    [("games", games), ("mirrors", mirrors), ("server", server)]
        .into_iter()
        .map(|(name, schema)| {
            let mut bytes = serde_json::to_vec_pretty(&schema)?;
            bytes.push(b'\n');
            Ok(RenderedFile {
                path: format!("schema/{name}.schema.json"),
                bytes,
            })
        })
        .collect()
}

fn integer_64_fields(feeds: &[Feed]) -> WebgenResult<Vec<Integer64Field>> {
    let mut fields = Vec::new();
    for feed in feeds {
        for description in feed.descriptions {
            match description.shape {
                Shape::Object(object_fields) => {
                    collect_integer_fields(
                        &mut fields,
                        feed.module,
                        description.name,
                        None,
                        object_fields,
                    )?;
                }
                Shape::Enum { variants, .. } => {
                    for variant in variants {
                        if let VariantShape::Object(object_fields) = variant.shape {
                            collect_integer_fields(
                                &mut fields,
                                feed.module,
                                description.name,
                                Some(variant.name),
                                object_fields,
                            )?;
                        }
                    }
                }
            }
        }
    }
    fields.sort_by(|left, right| left.path.cmp(&right.path));
    fields.dedup_by(|left, right| left.path == right.path);
    Ok(fields)
}

fn collect_integer_fields(
    output: &mut Vec<Integer64Field>,
    module: &str,
    description: &str,
    variant: Option<&str>,
    fields: &[Field],
) -> WebgenResult<()> {
    for field in fields {
        let variant = variant.map_or_else(String::new, |name| format!(".{name}"));
        let path = format!("{module}.{description}{variant}.{}", field.name);
        if let Some(wide) = declared_wide(field.ty, &path)? {
            let (representation, bound) = match wide {
                Wide::Exact => (Integer64Representation::Exact, None),
                Wide::Precise { bound } => {
                    (Integer64Representation::Precise, Some(bound.to_owned()))
                }
            };
            output.push(Integer64Field {
                path,
                representation,
                bound,
            });
        }
    }
    Ok(())
}

fn declared_wide(ty: TypeRef, path: &str) -> WebgenResult<Option<Wide>> {
    match ty {
        TypeRef::Integer { bits: 64, wide, .. } => wide.map(Some).ok_or_else(|| {
            format!(
                "undeclared 64-bit integer representation for {path}; expected exact or precise"
            )
            .into()
        }),
        TypeRef::Nullable(inner) | TypeRef::Sequence(inner) | TypeRef::Array(inner, _) => {
            declared_wide(*inner, path)
        }
        TypeRef::Tuple(items) => {
            let mut declaration = None;
            for item in items {
                if let Some(current) = declared_wide(*item, path)? {
                    if declaration.is_some_and(|previous| previous != current) {
                        return Err(format!(
                            "mixed 64-bit integer representations within boundary field {path}"
                        )
                        .into());
                    }
                    declaration = Some(current);
                }
            }
            Ok(declaration)
        }
        _ => Ok(None),
    }
}

fn visit_description(
    description: &Description,
    visitor: &mut impl FnMut(TypeRef) -> WebgenResult<()>,
) -> WebgenResult<()> {
    match description.shape {
        Shape::Object(fields) => visit_fields(fields, visitor),
        Shape::Enum { variants, .. } => {
            for variant in variants {
                match variant.shape {
                    VariantShape::Unit => {}
                    VariantShape::Object(fields) => visit_fields(fields, visitor)?,
                    VariantShape::Intersection(ty) => visit_type(ty, visitor)?,
                }
            }
            Ok(())
        }
    }
}

fn visit_fields(
    fields: &[Field],
    visitor: &mut impl FnMut(TypeRef) -> WebgenResult<()>,
) -> WebgenResult<()> {
    for field in fields {
        visit_type(field.ty, visitor)?;
    }
    Ok(())
}

fn visit_type(
    ty: TypeRef,
    visitor: &mut impl FnMut(TypeRef) -> WebgenResult<()>,
) -> WebgenResult<()> {
    visitor(ty)?;
    match ty {
        TypeRef::Nullable(inner) | TypeRef::Sequence(inner) | TypeRef::Array(inner, _) => {
            visit_type(*inner, visitor)
        }
        TypeRef::Tuple(items) => {
            for item in items {
                visit_type(*item, visitor)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> WebgenResult<()> {
    fs::create_dir_all(path.parent().ok_or("generated file has no parent")?)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn remove_dir(path: &Path) -> WebgenResult<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_file(path: &Path) -> WebgenResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"invalid utf-8\"".into())
}

fn clean_rendered(rendered: &str) -> Vec<u8> {
    let mut output = String::new();
    for line in rendered.lines() {
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output.into_bytes()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNDECLARED_FIELDS: &[Field] = &[Field {
        name: "identity",
        ty: TypeRef::Integer {
            signed: false,
            bits: 64,
            wide: None,
        },
        input_optional: false,
        output_optional: false,
    }];
    const UNDECLARED_DESCRIPTION: Description = Description {
        name: "Undeclared",
        direction: Direction::Output,
        deny_unknown_fields: false,
        shape: Shape::Object(UNDECLARED_FIELDS),
    };
    const UNDECLARED_DESCRIPTIONS: &[&Description] = &[&UNDECLARED_DESCRIPTION];

    #[test]
    fn undeclared_wide_integer_is_a_named_generator_error() {
        let error = integer_64_fields(&[Feed {
            module: "fixture",
            descriptions: UNDECLARED_DESCRIPTIONS,
        }])
        .expect_err("missing declarations must fail");
        assert_eq!(
            error.to_string(),
            "undeclared 64-bit integer representation for fixture.Undeclared.identity; expected exact or precise"
        );
    }
}
