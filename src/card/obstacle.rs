use crate::card::card_params::CardSpecialized;
use crate::card::{Card, CardKind};
use crate::physics::obstacle::Obstacle;
use crate::register_card_specialized_param;
use bevy::ecs::system::EntityCommands;
use bevy::math::Vec2;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardObstacleType {
    Full,
    Path(Vec<Vec2>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstacleCardParams {
    pub obstacle_def: CardObstacleType,
}

impl CardSpecialized for ObstacleCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Obstacle
    }

    fn insert_components(&self, entity: &mut EntityCommands<'_>) {
        entity.insert(Obstacle::new(match &self.obstacle_def {
            CardObstacleType::Full => {
                vec![
                    Vec2::new(-crate::card::CARD_SIZE.x * 0.5, -crate::card::CARD_SIZE.y * 0.5),
                    Vec2::new(crate::card::CARD_SIZE.x * 0.5, -crate::card::CARD_SIZE.y * 0.5),
                    Vec2::new(crate::card::CARD_SIZE.x * 0.5, crate::card::CARD_SIZE.y * 0.5),
                    Vec2::new(-crate::card::CARD_SIZE.x * 0.5, crate::card::CARD_SIZE.y * 0.5),
                ]
            },
            CardObstacleType::Path(path) => {
                path.clone()
            }
        }));
    }
}

pub fn spawn_obstacle_card(
    commands: &mut Commands,
    transform: Transform,
    title: String,
    obstacle_paths: Vec<Vec2>,
) -> Entity {
    commands
        .spawn((
            transform,
            Card { title },
            CardKind::Obstacle,
            Obstacle::new(obstacle_paths),
        ))
        .id()
}

register_card_specialized_param!("obstacle", ObstacleCardParams);
