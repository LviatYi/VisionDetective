use crate::physics::obstacle::Obstacle;
use bevy::prelude::*;

pub fn spawn_demo_obstacles(commands: &mut Commands) {
    commands.spawn((
        Transform::from_translation(Vec3::new(-210.0, 70.0, 1.0))
            .with_rotation(Quat::from_rotation_z(0.34)),
        Obstacle::new(vec![
            Vec2::new(-90.0, -28.0),
            Vec2::new(80.0, -22.0),
            Vec2::new(96.0, 8.0),
            Vec2::new(-74.0, 34.0),
        ]),
    ));

    commands.spawn((
        Transform::from_translation(Vec3::new(150.0, -20.0, 1.0)),
        Obstacle::new(vec![
            Vec2::new(-60.0, -80.0),
            Vec2::new(12.0, -108.0),
            Vec2::new(82.0, -18.0),
            Vec2::new(44.0, 92.0),
            Vec2::new(-46.0, 66.0),
            Vec2::new(-92.0, -8.0),
        ]),
    ));

    commands.spawn((
        Transform::from_translation(Vec3::new(280.0, 160.0, 1.0)),
        Obstacle::new(sample_cubic_closed_path(
            [
                Vec2::new(-72.0, -16.0),
                Vec2::new(-48.0, 72.0),
                Vec2::new(60.0, 76.0),
                Vec2::new(86.0, 6.0),
            ],
            [
                Vec2::new(-64.0, 40.0),
                Vec2::new(4.0, 98.0),
                Vec2::new(90.0, 54.0),
                Vec2::new(42.0, -54.0),
            ],
            [
                Vec2::new(-8.0, 96.0),
                Vec2::new(82.0, 98.0),
                Vec2::new(108.0, -26.0),
                Vec2::new(-14.0, -76.0),
            ],
            8,
        )),
    ));
}

fn sample_cubic_closed_path(
    anchors: [Vec2; 4],
    controls_a: [Vec2; 4],
    controls_b: [Vec2; 4],
    steps_per_curve: usize,
) -> Vec<Vec2> {
    let mut points = Vec::new();

    for curve_index in 0..anchors.len() {
        let start = anchors[curve_index];
        let control_a = controls_a[curve_index];
        let control_b = controls_b[curve_index];
        let end = anchors[(curve_index + 1) % anchors.len()];

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
