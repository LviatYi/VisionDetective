use bevy::prelude::*;

const OBSTACLE_COLOR: Color = Color::srgb(0.64, 0.67, 0.78);
const OBSTACLE_VERTEX_COLOR: Color = Color::srgb(0.78, 0.81, 0.90);

#[derive(Component, Clone)]
pub struct Obstacle {
    pub local_path: Vec<Vec2>,
}

impl Obstacle {
    pub fn new(local_path: Vec<Vec2>) -> Self {
        Self { local_path }
    }

    pub fn world_path(&self, transform: &Transform) -> Vec<Vec2> {
        self.local_path
            .iter()
            .map(|point| transform.transform_point(point.extend(0.0)).truncate())
            .collect()
    }
}

pub fn draw_obstacle_paths(mut gizmos: Gizmos, obstacle_query: Query<(&Transform, &Obstacle)>) {
    for (transform, obstacle) in &obstacle_query {
        let world_path = obstacle.world_path(transform);
        if world_path.len() < 2 {
            continue;
        }

        for index in 0..world_path.len() {
            let a = world_path[index];
            let b = world_path[(index + 1) % world_path.len()];
            gizmos.line_2d(a, b, OBSTACLE_COLOR);
            gizmos.circle_2d(a, 3.0, OBSTACLE_VERTEX_COLOR);
        }
    }
}
