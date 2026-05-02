use crate::card::card_params::{CardAppearanceConfig, CardPrefab, CardSpecializedConfig};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Resource, Debug, Serialize, Deserialize, Clone)]
pub struct CardPresetsConfig {
    pub appearances: Vec<CardAppearanceConfig>,

    pub specialized: Vec<CardSpecializedConfig>,

    pub prefabs: Vec<CardPrefab>,
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
      "title": "景观卡",
      "background_color_appearance_override": "#4C8A68FF",
      "image_layout_type": "normal",
      "image_res_path": ""
    },
    {
      "id": 1002,
      "title": "障碍卡",
      "background_color_appearance_override": "#7D6148FF",
      "image_layout_type": "full",
      "image_res_path": "/assets/config/pics/wall-bricks.png"
    },
    {
      "id": 1003,
      "title": "交互卡",
      "background_color_appearance_override": "#C6783DFF",
      "image_layout_type": "normal",
      "image_res_path": ""
    },
    {
      "id": 1004,
      "title": "曲线障碍卡",
      "background_color_appearance_override": "#7D6148FF",
      "image_layout_type": "full",
      "image_res_path": "/assets/config/pics/wall-bricks.png"
    }
  ],
  "specialized": [
    {
      "id": 10000001,
      "type": "scenery",
      "params": {}
    },
    {
      "id": 10000002,
      "type": "obstacle",
      "params": {
        "obstacle_def": {
          "path": [[-90.0, -56.0], [74.0, -68.0], [102.0, 12.0], [18.0, 72.0], [-88.0, 42.0]]
        }
      }
    },
    {
      "id": 10000003,
      "type": "interactive",
      "params": {
        "interaction": "log_hello_world"
      }
    },
    {
      "id": 10000004,
      "type": "obstacle",
      "params": {
        "obstacle_def": {
          "bezier": {
            "anchors": [[-72.0, -16.0], [-48.0, 72.0], [60.0, 76.0], [86.0, 6.0]],
            "controls_a": [[-64.0, 40.0], [4.0, 98.0], [90.0, 54.0], [42.0, -54.0]],
            "controls_b": [[-8.0, 96.0], [82.0, 98.0], [108.0, -26.0], [-14.0, -76.0]],
            "steps_per_curve": 8
          }
        }
      }
    }
  ],
  "prefabs": [
    {
      "id": 2001,
      "appearance_id": 1001,
      "specialized_id": 10000001
    },
    {
      "id": 2002,
      "appearance_id": 1002,
      "specialized_id": 10000002
    },
    {
      "id": 2003,
      "appearance_id": 1003,
      "specialized_id": 10000003
    },
    {
      "id": 2004,
      "appearance_id": 1004,
      "specialized_id": 10000004
    }
  ]
}"##,
        );
        let config = config.expect("failed to parse config");
        assert_eq!(config.appearances.len(), 4);
        assert_eq!(config.specialized.len(), 4);
        assert_eq!(config.prefabs.len(), 4);

        assert_eq!(config.appearances[0].id, 1001);
        assert_eq!(config.appearances[0].title, "景观卡");
        assert_eq!(
            config.appearances[0].background_color_appearance_override,
            "#4C8A68FF"
        );
        assert_eq!(
            config.appearances[0].image_layout_type,
            CardImageLayoutType::Normal
        );
        assert_eq!(config.appearances[0].image_res_path, "");

        assert_eq!(config.specialized[0].id, 10000001);
        assert_eq!(config.specialized[0].type_id, "scenery");
        assert_eq!(config.specialized[0].params, serde_json::json!({}));

        assert_eq!(config.specialized[2].id, 10000003);
        assert_eq!(config.specialized[2].type_id, "interactive");
        assert_eq!(
            config.specialized[2].params,
            serde_json::json!({
                "interaction": "log_hello_world"
            })
        );

        assert_eq!(config.prefabs[0].id, 2001);
        assert_eq!(config.prefabs[0].appearance_id, 1001);
        assert_eq!(config.prefabs[0].specialized_id, 10000001);
        assert_eq!(config.prefabs[3].id, 2004);
        assert_eq!(config.prefabs[3].appearance_id, 1004);
        assert_eq!(config.prefabs[3].specialized_id, 10000004);
    }
}
