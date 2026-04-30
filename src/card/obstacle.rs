use crate::card::card_params::{CardAppearances, CardKindProvider, CardSceneParams};
use crate::card::{CardKind, generate_card_bundle};
use crate::physics::obstacle::Obstacle;
use bevy::math::Vec2;
use bevy::prelude::{Commands, Entity, Transform};

#[derive(Debug, Clone)]
pub struct ObstacleCardSceneParams {
    normal: CardSceneParams,
    appearance: CardAppearances,
    obstacle_paths: Vec<Vec2>,
}

impl CardKindProvider for ObstacleCardSceneParams {
    fn kind(&self) -> CardKind {
        CardKind::Obstacle
    }
}

pub fn spawn_obstacle_card(
    commands: &mut Commands,
    transform: Transform,
    title: String,
    local_path: Vec<Vec2>,
) -> Entity {
    commands
        .spawn(generate_card_bundle(
            transform,
            title,
            CardKind::Obstacle,
            Obstacle::new(local_path),
        ))
        .id()
}
