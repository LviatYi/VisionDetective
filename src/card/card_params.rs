use crate::card::CardKind;
use crate::config::card_config::CardPresetsConfig;
use anyhow::Result;
use bevy::ecs::system::EntityCommands;
use bevy::math::Vec2;
use bevy::prelude::{Res, Resource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Runtime card instance parameters for scene loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardParam {
    pub scene_param: CardSceneParam,

    pub appearance_id: u32,

    pub specialized_id: u32,
}

impl CardParam {
    pub fn load_appearance(&self, config: &CardPresetsConfig) -> CardAppearanceConfig {
        config
            .appearances
            .iter()
            .find(|c| c.id == self.appearance_id)
            .cloned()
            .unwrap_or_else(CardAppearanceConfig::placeholder)
    }

    pub fn load_specialized_config(
        &self,
        config: &CardPresetsConfig,
        registry: Res<CardSpecializedRegistry>,
    ) -> Option<Box<dyn CardSpecialized>> {
        config
            .specialized
            .iter()
            .find(|c| c.id == self.specialized_id)
            .cloned()
            .and_then(|c| {
                registry
                    .get(c.type_id.as_str())
                    .cloned()
                    .map(|item| (item, c.params))
            })
            .and_then(|(registration, json)| registration.deserialize(&json).ok())
    }
}

/// Scene param for card instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSceneParam {
    pub position: Vec2,

    pub rotation: f32,
}

/// Appearance preset for a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAppearanceConfig {
    pub id: u32,
    pub title: String,
    /// Optional override of the background color.
    #[serde(default)]
    pub background_color_appearance_override: String,
    #[serde(default)]
    pub image_layout_type: CardImageLayoutType,
    pub image_res_path: String,
}

impl CardAppearanceConfig {
    pub fn placeholder() -> Self {
        Self {
            id: 0,
            title: "Placeholder".to_string(),
            background_color_appearance_override: "#FFFFFFFF".to_string(),
            image_layout_type: CardImageLayoutType::Normal,
            image_res_path: String::new(),
        }
    }
}

/// Layout modes for card artwork.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardImageLayoutType {
    /// Draw the image within the normal card content area.
    #[default]
    Normal,
    /// Draw the image in a full-bleed style.
    Full,
}

/// Trait implemented by every card-kind-specific parameter payload.
pub trait CardSpecialized: Send + Sync {
    /// Returns the concrete card kind this specialized payload spawns.
    fn kind(&self) -> CardKind;

    /// Inserts kind-specific ECS components into the spawned entity.
    fn insert_components(&self, entity: &mut EntityCommands<'_>);
}

/// Serialized specialized preset entry loaded from configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSpecializedConfig {
    pub id: u32,

    /// Registered specialized type name used to locate the deserializer.
    #[serde(rename = "type")]
    pub type_id: String,

    /// Raw JSON payload for the specialized type.
    #[serde(default)]
    pub params: Value,
}

//region Card Specialized Registry

/// Function signature used to deserialize a specialized payload from raw JSON.
pub type CardSpecializedDeserializer = fn(&Value) -> Result<Box<dyn CardSpecialized>>;

/// Static registration entry collected through `inventory`.
#[derive(Debug, Clone)]
pub struct CardSpecializedRegistration {
    pub type_id: &'static str,

    pub json_src_deserializer: CardSpecializedDeserializer,
}

impl CardSpecializedRegistration {
    pub const fn new(
        type_id: &'static str,
        json_src_deserializer: CardSpecializedDeserializer,
    ) -> Self {
        Self {
            type_id,
            json_src_deserializer,
        }
    }

    fn deserialize(&self, json: &Value) -> Result<Box<dyn CardSpecialized>> {
        (self.json_src_deserializer)(json)
    }
}

inventory::collect!(CardSpecializedRegistration);

#[macro_export]
macro_rules! register_card_specialized_param {
    ($name:expr, $param_type:ty) => {
        inventory::submit! {
            $crate::card::card_params::CardSpecializedRegistration::new(
                $name,
                |value: &serde_json::Value| -> anyhow::Result<
                    Box<dyn $crate::card::card_params::CardSpecialized>,
                > {
                    let params = serde_json::from_value::<$param_type>(value.clone())?;
                    Ok(Box::new(params))
                }
            )
        }
    };
}

/// The registry of CardSpecialized providers.
#[derive(Resource)]
pub struct CardSpecializedRegistry {
    registrations: HashMap<&'static str, &'static CardSpecializedRegistration>,
}

impl CardSpecializedRegistry {
    pub fn get(&self, type_id: &str) -> Option<&'static CardSpecializedRegistration> {
        self.registrations.get(type_id).copied()
    }
}

impl Default for CardSpecializedRegistry {
    fn default() -> Self {
        Self {
            registrations: inventory::iter::<CardSpecializedRegistration>
                .into_iter()
                .map(|registration| (registration.type_id, registration))
                .collect(),
        }
    }
}

//endregion

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::CardKind;

    #[test]
    fn registry_can_find_and_deserialize_obstacle_specialized_param() {
        let registry = CardSpecializedRegistry::default();

        let registration = registry
            .get("obstacle")
            .expect("obstacle specialized type should be registered");

        let specialized = registration
            .deserialize(&serde_json::json!({
                "obstacle_def": "full"
            }))
            .expect("registered obstacle deserializer should parse json");

        assert_eq!(specialized.kind(), CardKind::Obstacle);
    }
}
