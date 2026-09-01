// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Lossless syntax for the physically declared surface.
//!
//! A surface document is read and parsed once. The tree retains the original
//! bytes and text beside the parsed TOML table, so validators can project typed
//! views without reopening the file and writers can keep owner-authored bytes.
//! The tree assigns no policy meaning: strict compilation and recovery-oriented
//! audit remain separate consumers of the same syntax.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

/// Every declared document after one physical read and at most one TOML parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceAst {
    documents: BTreeMap<String, DocumentAst>,
}

impl SurfaceAst {
    /// Read the named members beneath one declaration directory.
    #[must_use]
    pub(crate) fn read(directory: &Path, members: impl IntoIterator<Item = String>) -> Self {
        let documents = members
            .into_iter()
            .map(|name| {
                let document = DocumentAst::read(&directory.join(&name));

                (name, document)
            })
            .collect();

        Self { documents }
    }

    /// One document by its physical member name.
    #[must_use]
    pub(crate) fn document(&self, name: &str) -> Option<&DocumentAst> {
        self.documents.get(name)
    }

    /// The documents in their physical-name order.
    pub(crate) fn documents(&self) -> impl Iterator<Item = (&str, &DocumentAst)> {
        self.documents
            .iter()
            .map(|(name, document)| (name.as_str(), document))
    }
}

/// One document's lossless bytes and its single syntax result.
#[derive(Debug, Clone)]
pub enum DocumentAst {
    /// The member could not be read at all.
    Unreadable {
        /// The filesystem's original error text.
        message: String,
    },
    /// The member has bytes but they do not form UTF-8 text.
    NonUtf8 {
        /// The bytes exactly as read.
        bytes: Vec<u8>,
        /// The text produced by the audit reader's established conversion.
        audit_message: String,
    },
    /// The member is text, with either one parsed table or one lexical defect.
    Text {
        /// The bytes exactly as read.
        bytes: Vec<u8>,
        /// The source text exactly as read.
        text: String,
        /// The single TOML parse result.
        table: Result<toml::Table, String>,
    },
}

impl PartialEq for DocumentAst {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unreadable { message: left }, Self::Unreadable { message: right }) => {
                left == right
            }
            (Self::NonUtf8 { bytes: left, .. }, Self::NonUtf8 { bytes: right, .. })
            | (Self::Text { bytes: left, .. }, Self::Text { bytes: right, .. }) => left == right,
            (Self::Unreadable { .. } | Self::NonUtf8 { .. } | Self::Text { .. }, _) => false,
        }
    }
}

impl Eq for DocumentAst {}

impl DocumentAst {
    fn read(path: &Path) -> Self {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Self::Unreadable {
                    message: error.to_string(),
                };
            }
        };

        let text = match String::from_utf8(bytes.clone()) {
            Ok(text) => text,
            Err(error) => {
                return Self::NonUtf8 {
                    bytes,
                    audit_message: error.to_string(),
                };
            }
        };

        let table = toml::from_str(&text).map_err(|error| error.to_string());

        Self::Text { bytes, text, table }
    }

    /// The exact member bytes, when the member could be read.
    pub(crate) fn bytes(&self) -> Result<&[u8], &str> {
        match self {
            Self::Unreadable { message } => Err(message),
            Self::NonUtf8 { bytes, .. } | Self::Text { bytes, .. } => Ok(bytes),
        }
    }

    /// The exact member text, when it is UTF-8.
    pub(crate) fn text(&self) -> Result<&str, &str> {
        match self {
            Self::Unreadable { message } => Err(message),
            Self::NonUtf8 { audit_message, .. } => Err(audit_message),
            Self::Text { text, .. } => Ok(text),
        }
    }

    /// The source text under the strict reader's established error classes.
    pub(crate) fn strict_text(&self) -> Result<&str, DocumentDefect> {
        match self {
            Self::Unreadable { message } => Err(DocumentDefect::Unreadable(message.clone())),
            Self::NonUtf8 { .. } => Err(DocumentDefect::Unreadable(String::from(
                "stream did not contain valid UTF-8",
            ))),
            Self::Text { text, .. } => Ok(text),
        }
    }

    /// The parsed table, or the lexical defect from the document's one parse.
    pub(crate) fn table(&self) -> Result<&toml::Table, &str> {
        match self {
            Self::Unreadable { message } => Err(message),
            Self::NonUtf8 { audit_message, .. } => Err(audit_message),
            Self::Text { table, .. } => table.as_ref().map_err(String::as_str),
        }
    }

    /// Project one typed view from the parsed table without parsing again.
    pub(crate) fn deserialize<T>(&self) -> Result<T, DocumentDefect>
    where
        T: serde::de::DeserializeOwned,
    {
        match self {
            Self::Unreadable { message } => Err(DocumentDefect::Unreadable(message.clone())),
            Self::NonUtf8 { .. } => Err(DocumentDefect::Unreadable(String::from(
                "stream did not contain valid UTF-8",
            ))),
            Self::Text {
                table: Err(message),
                ..
            } => Err(DocumentDefect::Malformed(message.clone())),
            Self::Text {
                text,
                table: Ok(table),
                ..
            } => table
                .clone()
                .try_into()
                .map_err(|mut error: toml::de::Error| {
                    error.set_input(Some(text));
                    DocumentDefect::Malformed(error.to_string())
                }),
        }
    }
}

/// Why a strict typed projection could not be formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentDefect {
    /// The source was absent, unreadable, or not UTF-8 text.
    Unreadable(String),
    /// The source was not TOML or did not have the requested typed shape.
    Malformed(String),
}

/// The one syntax tree for all list tables.
#[derive(Debug, Deserialize)]
pub struct ListDocument {
    /// The allocated schema label carried by the list file's envelope.
    #[serde(default, rename = "namespace")]
    pub(crate) _namespace: String,
    /// The list schema's version triple.
    #[serde(default, rename = "version")]
    pub(crate) _version: [u64; 3],
    /// Each owner's tables, keyed by the program or deployed instance.
    #[serde(flatten)]
    pub(crate) tables: BTreeMap<String, BTreeMap<String, ListEntry>>,
}

/// One singleton list or one list per declared family.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ListEntry {
    /// The list of a program this corpus deploys once.
    Singleton(ListTable),
    /// One list per declared family of a multiply deployed program.
    Instanced(BTreeMap<String, ListTable>),
}

/// One raw table retaining row order, duplicates, and undecoded identities.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListTable {
    pub(crate) allowances: Option<Vec<AllowanceRow>>,
    pub(crate) path_counts: Option<Vec<PathCountRow>>,
    pub(crate) paths: Option<Vec<String>>,
}

/// One undecoded fingerprint allowance.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowanceRow {
    pub(crate) fingerprint: String,
    pub(crate) maximum: i64,
}

/// One undecoded path-count row.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathCountRow {
    pub(crate) path: String,
    pub(crate) maximum: i64,
}
