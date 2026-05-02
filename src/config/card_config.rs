use crate::card::card_params::{CardAppearanceConfig, CardSpecializedConfig};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Resource, Debug, Serialize, Deserialize, Clone)]
pub struct CardPresetsConfig {
    pub appearances: Vec<CardAppearanceConfig>,

    pub specialized: Vec<CardSpecializedConfig>,
}

impl CardPresetsConfig {
    pub fn load() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("config")
            .join("card-presets.json");

        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read config {}: {error}", path.display()));

        Self::load_from(raw)
            .unwrap_or_else(|error| panic!("failed to parse config {}: {error}", path.display()))
    }

    fn load_from(raw: impl AsRef<str>) -> Result<Self, String> {
        serde_json::from_str::<Self>(raw.as_ref()).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::card_params::CardImageLayoutType;

    #[test]
    fn test_parse_json() {
        let config = CardPresetsConfig::load_from(
            r##"{
  "appearances": [
    {
      "id": 1001,
      "title": "Wall",
      "background_color_appearance_override": "#000000",
      "image_layout_type": "full",
      "image_res_path": "/assets/config/pics/wall-bricks.png"
    }
  ],
  "specialized": [
    {
      "id": 10000001,
      "type": "obstacle",
      "params": {
        "obstacle_def": "full"
      }
    }
  ]
}"##,
        );
        let config = config.expect("failed to parse config");
        assert_eq!(config.appearances.len(), 1);
        assert_eq!(config.specialized.len(), 1);

        assert_eq!(config.appearances[0].id, 1001);
        assert_eq!(config.appearances[0].title, "Wall");
        assert_eq!(
            config.appearances[0].background_color_appearance_override,
            "#000000"
        );
        assert_eq!(
            config.appearances[0].image_layout_type,
            CardImageLayoutType::Full
        );
        assert_eq!(
            config.appearances[0].image_res_path,
            "/assets/config/pics/wall-bricks.png"
        );

        assert_eq!(config.specialized[0].id, 10000001);
        assert_eq!(config.specialized[0].type_id, "obstacle");
        assert_eq!(
            config.specialized[0].params,
            serde_json::json!({
                "obstacle_def": "full"
            })
        );
    }
}
