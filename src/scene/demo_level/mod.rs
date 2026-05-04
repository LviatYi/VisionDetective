use crate::GameView;
use crate::card::card_params::{CardParam, CardSceneParam, CardSpecializedRegistry};
use crate::card::spawn_card_by_card_param;
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use bevy::prelude::*;

struct DemoCard {
    prefab_id: u32,
    position: Vec2,
    order: f32,
    rotation: f32,
}

const DEMO_CARDS: &[DemoCard] = &[
    DemoCard {
        prefab_id: 1001,
        position: Vec2::new(-310.0, -110.0),
        order: 0.8,
        rotation: -0.06,
    },
    DemoCard {
        prefab_id: 1002,
        position: Vec2::new(-10.0, 20.0),
        order: 0.8,
        rotation: 0.18,
    },
    DemoCard {
        prefab_id: 1003,
        position: Vec2::new(265.0, -120.0),
        order: 0.8,
        rotation: 0.1,
    },
    DemoCard {
        prefab_id: 1004,
        position: Vec2::new(280.0, 150.0),
        order: 0.8,
        rotation: 0.0,
    },
    DemoCard {
        prefab_id: 1005,
        position: Vec2::new(-255.0, 145.0),
        order: 0.8,
        rotation: 0.0,
    },
    DemoCard {
        prefab_id: 1006,
        position: Vec2::new(105.0, -185.0),
        order: 0.8,
        rotation: -0.12,
    },
];

pub fn spawn_demo_cards(
    commands: &mut Commands,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    card_presets_config: &CardPresetsConfig,
    card_specialized_registry: &CardSpecializedRegistry,
) {
    for card in DEMO_CARDS {
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
    card: &DemoCard,
) {
    let entity = spawn_card_by_card_param(
        commands,
        asset_server,
        config,
        meshes,
        materials,
        &CardParam {
            scene_param: CardSceneParam {
                position: card.position,
                rotation: card.rotation,
                order: card.order,
            },
            prefab_id: card.prefab_id,
        },
        card_presets_config,
        card_specialized_registry,
    );

    commands.entity(entity).insert(GameView);
}
