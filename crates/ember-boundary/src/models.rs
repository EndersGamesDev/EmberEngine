//! Data files consumed by the editable web surfaces.

use serde::{Deserialize, Serialize};

use crate::{Boundary, Description};

/// Every data-file model supplied to the phase 0b renderer.
#[must_use]
pub const fn boundary_descriptions() -> &'static [&'static Description] {
    crate::boundary_descriptions![
        GameCatalog,
        Game,
        Trailer,
        GameRelease,
        HostBook,
        Mirror,
        HostEntry,
    ]
}

#[derive(Boundary, Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct GameCatalog {
    pub games: Vec<Game>,
}

#[derive(Boundary, Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct Game {
    pub id: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub title: String,
    #[serde(default)]
    pub landing: Option<String>,
    #[serde(default)]
    pub trailer: Option<Trailer>,
    pub tag: String,
    pub desc: String,
    pub versions: Vec<GameRelease>,
}

#[derive(Boundary, Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct Trailer {
    pub src: String,
    pub poster: String,
    pub captions: String,
}

#[derive(Boundary, Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct GameRelease {
    pub v: String,
    pub version: String,
    pub path: String,
    pub live: bool,
    #[serde(default)]
    pub proto: Option<u16>,
    #[serde(default)]
    pub handover: Option<bool>,
    #[serde(default)]
    pub bytes: Option<u64>,
    pub note: String,
}

#[derive(Boundary, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct HostBook {
    #[serde(default)]
    pub v: Option<String>,
    #[serde(default)]
    pub hosts: Vec<HostEntry>,
    #[serde(default)]
    pub mirrors: Vec<Mirror>,
    #[serde(default)]
    pub ws: Option<String>,
    #[serde(default)]
    pub proto: Option<u16>,
    #[serde(default)]
    pub fire_ws: Option<String>,
    #[serde(default)]
    pub fire_proto: Option<u16>,
}

#[derive(Boundary, Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct Mirror {
    pub url: String,
    pub name: String,
}

#[derive(Boundary, Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[boundary(direction = "both")]
pub struct HostEntry {
    pub name: String,
    #[serde(default)]
    pub ws: Option<String>,
    #[serde(default)]
    pub proto: Option<u16>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub fire_ws: Option<String>,
    #[serde(default)]
    pub fire_proto: Option<u16>,
    #[serde(default)]
    pub fire_version: Option<String>,
    #[serde(default)]
    pub fire_commit: Option<String>,
    #[serde(default)]
    pub kings_ws: Option<String>,
    #[serde(default)]
    pub kings_proto: Option<u16>,
    #[serde(default)]
    pub kings_version: Option<String>,
    #[serde(default)]
    pub kings_commit: Option<String>,
    #[serde(default)]
    pub league_ws: Option<String>,
    #[serde(default)]
    pub league_proto: Option<u16>,
    #[serde(default)]
    pub league_version: Option<String>,
    #[serde(default)]
    pub league_commit: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
}
