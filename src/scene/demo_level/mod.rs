use crate::GameView;
use crate::card::card_params::{CardParam, CardSpawnParams};
use crate::card::spawn_card_by_card_param;
use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

const DEMO_SCENE_PATH: &str = "assets/scene/scene-demo-01.toml";

#[derive(Deserialize, Default)]
struct SceneFile {
    #[serde(default)]
    cards: Vec<CardParam>,
}

pub fn load_demo_scene(commands: &mut Commands, spawn_params: &mut CardSpawnParams<'_>) {
    let scene_path = demo_scene_path();
    let scene = match fs::read_to_string(&scene_path)
        .map_err(|error| error.to_string())
        .and_then(|raw| toml::from_str::<SceneFile>(&raw).map_err(|error| error.to_string()))
    {
        Ok(scene) => scene,
        Err(error) => {
            bevy::log::error!(
                "failed to load demo scene {}: {error}",
                scene_path.display()
            );
            return;
        }
    };

    for card in &scene.cards {
        let entity = spawn_card_by_card_param(commands, spawn_params, card);
        commands.entity(entity).insert(GameView);
    }
}

fn demo_scene_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(DEMO_SCENE_PATH)
}
