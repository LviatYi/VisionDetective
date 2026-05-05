use crate::GameView;
use crate::card::card_params::{CardParam, CardSceneParam, CardSpawnParams};
use crate::card::spawn_card_by_card_param;
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

pub fn spawn_demo_cards(commands: &mut Commands, spawn_params: &mut CardSpawnParams<'_>) {
    for card in DEMO_CARDS {
        spawn_demo_card(commands, spawn_params, card);
    }
}

fn spawn_demo_card(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    card: &DemoCard,
) {
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
        },
    );

    commands.entity(entity).insert(GameView);
}
