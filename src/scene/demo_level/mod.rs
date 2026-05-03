use crate::GameView;
use crate::card::card_params::{CardParam, CardSceneParam, CardSpecializedRegistry};
use crate::card::spawn_card_by_card_param;
use crate::config::DemoCardConfig;
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use bevy::prelude::*;

pub fn spawn_demo_cards(
    commands: &mut Commands,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    card_presets_config: &CardPresetsConfig,
    card_specialized_registry: &CardSpecializedRegistry,
) {
    for card in &config.scene.demo_cards {
        spawn_demo_card(
            commands,
            asset_server,
            config,
            meshes,
            materials,
            card_presets_config,
            card_specialized_registry,
            card,
        );
    }
}

fn spawn_demo_card(
    commands: &mut Commands,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    card_presets_config: &CardPresetsConfig,
    card_specialized_registry: &CardSpecializedRegistry,
    card: &DemoCardConfig,
) {
    let entity = spawn_card_by_card_param(
        commands,
        asset_server,
        config,
        meshes,
        materials,
        &CardParam {
            scene_param: CardSceneParam {
                position: card.translation().truncate(),
                rotation: card.rotation_z,
                order: card.translation().z,
            },
            prefab_id: card.prefab_id,
        },
        card_presets_config,
        card_specialized_registry,
    );

    commands.entity(entity).insert(GameView);
}
