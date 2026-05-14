use crate::card::card_params::{CardSpecializedParam, SpawnCardSystemParams};
use crate::card::{CardKind, CardSpecializedInstaller};
use crate::config::GameConfig;
use crate::physics::area::{Area, ShapeType};
use crate::register_card_specialized_installer;
use bevy::ecs::system::EntityCommands;
use bevy::math::Vec2;
use bevy::prelude::{Component, Deref, Gizmos, Query, Res, Transform};
use serde::{Deserialize, Serialize};

//region Installer

pub struct ObstacleCardSpecializedInstaller;

impl CardSpecializedInstaller for ObstacleCardSpecializedInstaller {
    type Param = ObstacleCardParams;

    const TYPE_ID: &'static str = "obstacle";
}

register_card_specialized_installer!(ObstacleCardSpecializedInstaller);

//endregion

//region Card Specialized Param

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstacleCardParams {
    pub shape_def: ShapeType,
}

impl CardSpecializedParam for ObstacleCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Obstacle
    }

    fn spawn_with(
        &self,
        entity: &mut EntityCommands<'_>,
        spawn_params: &mut SpawnCardSystemParams<'_>,
    ) {
        entity.insert(Obstacle::new(
            self.shape_def.sample_path(&spawn_params.config.cards),
        ));
    }
}

//endregion

//region Component

#[derive(Component, Clone, Deref)]
pub struct Obstacle {
    pub area: Area,
}

impl Obstacle {
    pub fn new(local_path: Vec<Vec2>) -> Self {
        Self {
            area: Area { local_path },
        }
    }
}

//endregion

//region Gizmos

pub fn draw_obstacle_paths(
    config: Res<GameConfig>,
    mut gizmos: Gizmos,
    obstacle_query: Query<(&Transform, &Obstacle)>,
) {
    for (transform, obstacle) in &obstacle_query {
        let world_path = obstacle.world_path(transform);
        if world_path.len() < 2 {
            continue;
        }

        for index in 0..world_path.len() {
            let a = world_path[index];
            let b = world_path[(index + 1) % world_path.len()];
            gizmos.line_2d(a, b, config.obstacles.edge_color());
            gizmos.circle_2d(
                a,
                config.obstacles.vertex_radius,
                config.obstacles.vertex_color(),
            );
        }
    }
}

//endregion
