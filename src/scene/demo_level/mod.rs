use crate::config::{vec2_from_pair, GameConfig};
use crate::physics::obstacle::Obstacle;
use bevy::prelude::*;

pub fn spawn_demo_obstacles(commands: &mut Commands, config: &GameConfig) {
    for obstacle in &config.scene.demo_obstacles {
        let local_path = if let Some(path) = &obstacle.path {
            path.iter().copied().map(vec2_from_pair).collect()
        } else if let Some(bezier) = &obstacle.bezier {
            sample_cubic_closed_path(
                &bezier.anchors,
                &bezier.controls_a,
                &bezier.controls_b,
                config.scene.bezier_steps_per_curve,
            )
        } else {
            continue;
        };

        commands.spawn((
            Transform::from_translation(obstacle.translation())
                .with_rotation(Quat::from_rotation_z(obstacle.rotation_z)),
            Obstacle::new(local_path),
        ));
    }
}

fn sample_cubic_closed_path(
    anchors: &[[f32; 2]],
    controls_a: &[[f32; 2]],
    controls_b: &[[f32; 2]],
    steps_per_curve: usize,
) -> Vec<Vec2> {
    let mut points = Vec::new();

    for curve_index in 0..anchors.len() {
        let start = vec2_from_pair(anchors[curve_index]);
        let control_a = vec2_from_pair(controls_a[curve_index]);
        let control_b = vec2_from_pair(controls_b[curve_index]);
        let end = vec2_from_pair(anchors[(curve_index + 1) % anchors.len()]);

        for step in 0..steps_per_curve {
            let t = step as f32 / steps_per_curve as f32;
            points.push(sample_cubic_bezier(start, control_a, control_b, end, t));
        }
    }

    points
}

fn sample_cubic_bezier(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let one_minus_t = 1.0 - t;
    p0 * one_minus_t.powi(3)
        + p1 * 3.0 * one_minus_t.powi(2) * t
        + p2 * 3.0 * one_minus_t * t * t
        + p3 * t.powi(3)
}
