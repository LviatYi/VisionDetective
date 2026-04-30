use crate::card::CardKind;
use crate::config::card_config::CardPresetsConfig;
use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSceneParams {
    pub position: Vec2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CardImageLayoutType {
    #[default]
    Normal,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardAppearances {
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub background_color: String,
    #[serde(default)]
    pub image_layout_type: CardImageLayoutType,
    pub image_res_path: String,
}

impl CardAppearances {
    pub fn placeholder() -> Self {
        Self {
            id: 0,
            title: "Placeholder".to_string(),
            background_color: "#FFFFFFFF".to_string(),
            image_layout_type: CardImageLayoutType::Normal,
            image_res_path: "".to_string(),
        }
    }

    pub fn load_by_preset_config(config: &CardPresetsConfig, id: u32) -> Self {
        config
            .configs
            .iter()
            .find(|c| c.id == id)
            .map(|item| item.clone())
            .unwrap_or_else(Self::placeholder)
    }
}

pub trait CardKindProvider {
    fn kind(&self) -> CardKind;
}
