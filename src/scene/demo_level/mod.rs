use crate::GameView;
use crate::card::card_params::{
    CardParam, CardRuntimeSpecializedConfig, CardSceneParam, CardSpawnParams,
};
use crate::card::spawn_card_by_card_param;
use bevy::prelude::*;
use serde_json::json;

struct DemoCard {
    prefab_id: u32,
    position: Vec2,
    order: f32,
    rotation: f32,
    runtime_specialized_param: Option<DemoRuntimeSpecializedParam>,
}

#[derive(Clone, Copy)]
struct DemoRuntimeSpecializedParam {
    type_id: &'static str,
    interaction_prefab_id: u32,
    interaction_target_position: Vec2,
    interaction_target_order: f32,
    interaction_target_rotation: f32,
}

const DEMO_CARDS: &[DemoCard] = &[
    DemoCard {
        prefab_id: 1001,
        position: Vec2::new(-310.0, -110.0),
        order: 0.8,
        rotation: -0.06,
        runtime_specialized_param: None,
    },
    DemoCard {
        prefab_id: 1002,
        position: Vec2::new(-10.0, 20.0),
        order: 0.8,
        rotation: 0.18,
        runtime_specialized_param: None,
    },
    DemoCard {
        prefab_id: 1003,
        position: Vec2::new(265.0, -120.0),
        order: 0.8,
        rotation: 0.1,
        runtime_specialized_param: None,
    },
    DemoCard {
        prefab_id: 1004,
        position: Vec2::new(280.0, 150.0),
        order: 0.8,
        rotation: 0.0,
        runtime_specialized_param: None,
    },
    DemoCard {
        prefab_id: 1006,
        position: Vec2::new(105.0, -185.0),
        order: 0.8,
        rotation: -0.12,
        runtime_specialized_param: Some(DemoRuntimeSpecializedParam {
            type_id: "clue",
            interaction_prefab_id: 1005,
            interaction_target_position: Vec2::new(-255.0, 145.0),
            interaction_target_order: 0.8,
            interaction_target_rotation: 0.0,
        }),
    },
];

pub fn spawn_demo_cards(commands: &mut Commands, spawn_params: &mut CardSpawnParams<'_>) {
    for card in DEMO_CARDS {
        spawn_demo_card(commands, spawn_params, card);
    }
}

fn spawn_demo_card(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    card: &DemoCard,
) -> Entity {
    let entity = spawn_card_by_card_param(
        commands,
        spawn_params,
        &CardParam {
            scene_param: CardSceneParam {
                position: card.position,
                rotation: card.rotation,
                order: card.order,
            },
            prefab_id: card.prefab_id,
            runtime_specialized_param: card.runtime_specialized_param.map(|param| {
                CardRuntimeSpecializedConfig {
                    type_id: param.type_id.to_string(),
                    params: json!({
                        "interaction_prefab_id": param.interaction_prefab_id,
                        "interaction_target_scene_param": {
                            "position": [
                                param.interaction_target_position.x,
                                param.interaction_target_position.y
                            ],
                            "rotation": param.interaction_target_rotation,
                            "order": param.interaction_target_order,
                        },
                    }),
                }
            }),
        },
    );

    commands.entity(entity).insert(GameView);
    entity
}
