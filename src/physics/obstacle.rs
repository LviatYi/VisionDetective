use crate::config::GameConfig;
use crate::physics::area::Area;
use bevy::prelude::*;

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
