#[cfg(not(target_arch = "wasm32"))]
use crate::asset::runtime_root;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterConfig {
    #[serde(default)]
    pub characters: Vec<CharacterDefinition>,
}

impl CharacterConfig {
    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        let raw = include_str!("../../assets/config/character-presets.toml");

        toml::from_str::<Self>(raw).unwrap_or_else(|error| {
            panic!("failed to parse embedded config assets/config/character-presets.toml: {error}")
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        let path = runtime_root()
            .join("assets")
            .join("config")
            .join("character-presets.toml");

        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read config {}: {error}", path.display()));

        toml::from_str::<Self>(&raw)
            .unwrap_or_else(|error| panic!("failed to parse config {}: {error}", path.display()))
    }

    pub fn get(&self, id: u32) -> Option<&CharacterDefinition> {
        self.characters.iter().find(|character| character.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterDefinition {
    /// Entity unique ID. This has the same meaning as dialogue node source.
    pub id: u32,
    pub name: String,
    pub description: String,
    pub dialogue_portrait_image_path: String,
    pub coin_portrait_image_path: String,
}
