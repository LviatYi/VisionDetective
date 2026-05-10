use crate::card::CardKind;
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use anyhow::Result;
use bevy::color::{Color, Srgba};
use bevy::ecs::system::{EntityCommands, SystemParam};
use bevy::math::Vec2;
use bevy::prelude::{AssetServer, Assets, ColorMaterial, Mesh, Res, ResMut, Resource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Runtime card instance parameters for scene loading.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CardParam {
    pub scene_param: CardSceneParam,

    pub prefab_id: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_specialized_param: Option<CardRuntimeSpecializedConfig>,
}

/// A card prefab that binds one appearance preset to one specialized preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPrefab {
    pub id: u32,

    pub appearance_id: u32,

    pub specialized_id: u32,

    pub description: Option<String>,
}

impl CardParam {
    pub fn load_prefab(&self, config: &CardPresetsConfig) -> Option<CardPrefab> {
        config
            .prefabs
            .iter()
            .find(|prefab| prefab.id == self.prefab_id)
            .cloned()
    }

    pub fn load_appearance(&self, config: &CardPresetsConfig) -> CardAppearanceConfig {
        let Some(prefab) = self.load_prefab(config) else {
            return CardAppearanceConfig::placeholder();
        };

        config
            .appearances
            .iter()
            .find(|appearance| appearance.id == prefab.appearance_id)
            .cloned()
            .unwrap_or_else(CardAppearanceConfig::placeholder)
    }

    pub fn load_specialized_config(
        &self,
        card_presets_config: &CardPresetsConfig,
        registry: &CardSpecializedRegistry,
    ) -> Option<Box<dyn CardSpecialized>> {
        let prefab = self.load_prefab(card_presets_config)?;

        card_presets_config
            .specialized
            .iter()
            .find(|specialized| specialized.id == prefab.specialized_id)
            .cloned()
            .and_then(|base| {
                let (type_id, params) = self.merged_specialized_params(base)?;
                registry
                    .get(type_id.as_str())
                    .cloned()
                    .map(|item| (item, params))
            })
            .and_then(|(registration, json)| registration.deserialize(&json).ok())
    }

    pub fn resolve_fill_color(
        &self,
        config: &GameConfig,
        card_presets_config: &CardPresetsConfig,
        registry: &CardSpecializedRegistry,
    ) -> Color {
        let appearance = self.load_appearance(card_presets_config);
        if appearance.background_color_appearance_override.is_empty() {
            None
        } else {
            Srgba::hex(&appearance.background_color_appearance_override)
                .map(|c| Color::Srgba(c))
                .ok()
        }
        .or_else(|| {
            self.load_specialized_config(card_presets_config, registry)
                .map(|item| config.cards.fill_color(item.kind()))
        })
        .unwrap_or_else(|| config.cards.default_fill_color())
    }

    fn merged_specialized_params(&self, base: CardSpecializedConfig) -> Option<(String, Value)> {
        let Some(runtime) = &self.runtime_specialized_param else {
            return Some((base.type_id, base.params));
        };

        if runtime.type_id != base.type_id {
            bevy::log::warn!(
                "runtime specialized type {} does not match prefab specialized type {}",
                runtime.type_id,
                base.type_id
            );
            return None;
        }

        Some((
            base.type_id,
            merge_json_objects(base.params, runtime.params.clone()),
        ))
    }
}

/// Scene param for card instance.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CardSceneParam {
    pub position: Vec2,

    pub rotation: f32,

    #[serde(default)]
    pub order: f32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn_if: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destroy_if: Option<String>,

    #[serde(default, skip_deserializing, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Appearance preset for a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAppearanceConfig {
    pub id: u32,
    pub title: String,
    /// Optional override of the background color.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub background_color_appearance_override: String,
    pub image_res_path: String,
}

impl CardAppearanceConfig {
    pub fn placeholder() -> Self {
        Self {
            id: 0,
            title: "Placeholder".to_string(),
            background_color_appearance_override: "".to_string(),
            image_res_path: String::new(),
        }
    }
}

/// Trait implemented by every card-kind-specific parameter payload.
pub trait CardSpecialized: Send + Sync {
    /// Returns the concrete card kind this specialized payload spawns.
    fn kind(&self) -> CardKind;

    /// Inserts kind-specific ECS components into the spawned entity.
    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>);
}

#[derive(SystemParam)]
pub struct CardSpawnParams<'w> {
    pub asset_server: Res<'w, AssetServer>,
    pub config: Res<'w, GameConfig>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<ColorMaterial>>,
    pub card_presets_config: Res<'w, CardPresetsConfig>,
    pub card_specialized_registry: Res<'w, CardSpecializedRegistry>,
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

/// Serialized specialized override attached to one scene card instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRuntimeSpecializedConfig {
    /// Registered specialized type name. Must match the card prefab specialized type.
    #[serde(rename = "type")]
    pub type_id: String,

    /// Runtime JSON payload merged over the prefab specialized params.
    #[serde(default)]
    pub params: Value,
}

fn merge_json_objects(mut base: Value, runtime: Value) -> Value {
    match (&mut base, runtime) {
        (Value::Object(base_object), Value::Object(runtime_object)) => {
            for (key, runtime_value) in runtime_object {
                let merged = match base_object.remove(&key) {
                    Some(base_value) => merge_json_objects(base_value, runtime_value),
                    None => runtime_value,
                };
                base_object.insert(key, merged);
            }
            base
        }
        (_, runtime_value) => runtime_value,
    }
}

//region Card Specialized Registry

/// Function signature used to deserialize a specialized payload from raw JSON.
pub type CardSpecializedDeserializer = fn(&Value) -> Result<Box<dyn CardSpecialized>>;

/// Static registration entry collected through `inventory`.
#[derive(Debug, Clone)]
pub(super) struct CardSpecializedRegistration {
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
    fn get(&self, type_id: &str) -> Option<&'static CardSpecializedRegistration> {
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
    use crate::config::card_config::CardPresetsConfig;

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

    #[test]
    fn card_param_resolves_prefab_appearance_and_specialized() {
        let config = serde_json::from_str::<CardPresetsConfig>(
            r##"{
  "appearances": [
    {
      "id": 1003,
      "title": "交互卡",
      "background_color_appearance_override": "#C6783DFF",
      "image_layout_type": "normal",
      "image_res_path": ""
    },
    {
      "id": 1002,
      "title": "障碍卡",
      "background_color_appearance_override": "#7D6148FF",
      "image_layout_type": "full",
      "image_res_path": "/assets/config/pics/wall-bricks.png"
    }
  ],
  "specialized": [
    {
      "id": 10000003,
      "type": "interactive",
      "params": {
        "interaction": {
          "type": "log_hello_world",
          "params": {}
        }
      }
    },
    {
      "id": 10000002,
      "type": "obstacle",
      "params": {
        "obstacle_def": {
          "path": [[-90.0, -56.0], [74.0, -68.0], [102.0, 12.0], [18.0, 72.0], [-88.0, 42.0]]
        }
      }
    }
  ],
  "prefabs": [
    {
      "id": 2003,
      "appearance_id": 1003,
      "specialized_id": 10000003
    },
    {
      "id": 2002,
      "appearance_id": 1002,
      "specialized_id": 10000002
    }
  ]
}"##,
        )
        .expect("card presets json should parse");

        let card_param = CardParam {
            scene_param: CardSceneParam {
                position: Vec2::new(10.0, 20.0),
                rotation: 0.25,
                order: 3.0,
                spawn_if: None,
                destroy_if: None,
                description: String::new(),
            },
            prefab_id: 2003,
            runtime_specialized_param: None,
        };

        let appearance = card_param.load_appearance(&config);
        assert_eq!(appearance.id, 1003);
        assert_eq!(appearance.title, "交互卡");

        let registry = CardSpecializedRegistry::default();
        let specialized = card_param
            .load_specialized_config(&config, &registry)
            .expect("prefab specialized config should deserialize");

        assert_eq!(specialized.kind(), CardKind::Interaction);
    }

    #[test]
    fn card_param_merges_runtime_specialized_params_over_prefab_params() {
        let card_param = CardParam {
            scene_param: CardSceneParam::default(),
            prefab_id: 1006,
            runtime_specialized_param: Some(CardRuntimeSpecializedConfig {
                type_id: "clue".to_string(),
                params: serde_json::json!({
                    "interaction_prefab_id": 1005,
                    "interaction_target_scene_param": {
                        "position": [105.0, -20.0],
                        "rotation": -0.12,
                        "order": 0.85
                    }
                }),
            }),
        };
        let base = CardSpecializedConfig {
            id: 10000006,
            type_id: "clue".to_string(),
            params: serde_json::json!({
                "reveal_threshold": "normal"
            }),
        };

        let (_, merged) = card_param
            .merged_specialized_params(base)
            .expect("matching runtime specialized params should merge");

        assert_eq!(
            merged,
            serde_json::json!({
                "reveal_threshold": "normal",
                "interaction_prefab_id": 1005,
                "interaction_target_scene_param": {
                    "position": [105.0, -20.0],
                    "rotation": -0.12,
                    "order": 0.85
                }
            })
        );
    }
}
