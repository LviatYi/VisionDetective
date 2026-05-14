use crate::physics::area::ShapeType;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Resource, Debug, Serialize, Deserialize, Clone)]
pub struct TerrainPresetsConfig {
    pub terrains: Vec<TerrainPresetConfig>,
}

impl TerrainPresetsConfig {
    pub fn load() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("config")
            .join("terrain-presets.json");

        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read config {}: {error}", path.display()));

        Self::load_from(raw)
            .unwrap_or_else(|error| panic!("failed to parse config {}: {error}", path.display()))
    }

    fn load_from(raw: impl AsRef<str>) -> Result<Self, String> {
        serde_json::from_str::<Self>(raw.as_ref()).map_err(|error| error.to_string())
    }

    pub fn get(&self, id: u32) -> Option<&TerrainPresetConfig> {
        self.terrains.iter().find(|terrain| terrain.id == id)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerrainPresetConfig {
    pub id: u32,

    pub name: String,

    pub shape_def: ShapeType,

    pub appearance_ids: Vec<u32>,

    #[serde(default = "default_min_distance")]
    pub min_distance: f32,

    #[serde(default = "default_rejection_attempts")]
    pub rejection_attempts: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cards: Option<usize>,

    #[serde(default)]
    pub rotation: f32,

    #[serde(default)]
    pub rotation_jitter: f32,
}

fn default_min_distance() -> f32 {
    120.0
}

fn default_rejection_attempts() -> usize {
    30
}
