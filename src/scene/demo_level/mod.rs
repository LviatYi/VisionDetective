use crate::card::{
    CardKind, HelloWorldInteraction, Interactive, spawn_interaction_card, spawn_obstacle_card,
    spawn_scenery_card,
};
use crate::config::{DemoCardConfig, GameConfig, InteractionEffectConfig, vec2_from_pair};
use crate::physics::obstacle::Obstacle;
use crate::GameView;
use bevy::prelude::*;

pub fn spawn_demo_cards(commands: &mut Commands, config: &GameConfig) {
    for card in &config.scene.demo_cards {
        spawn_demo_card(commands, config, card);
    }
}

fn spawn_demo_card(commands: &mut Commands, config: &GameConfig, card: &DemoCardConfig) {
    let transform = Transform::from_translation(card.translation())
        .with_rotation(Quat::from_rotation_z(card.rotation_z));

    let entity = match card.kind {
        CardKind::Scenery => spawn_scenery_card(commands, transform, card.title.clone()),
        CardKind::Obstacle => {
            let entity = spawn_obstacle_card(commands, transform, card.title.clone());
            commands
                .entity(entity)
                .insert(Obstacle::new(resolve_local_path(card, config)));
            entity
        }
        CardKind::Interaction => spawn_interaction_card(commands, transform, card.title.clone()),
    };

    if matches!(
        card.interaction_effect,
        Some(InteractionEffectConfig::LogHelloWorld)
    ) {
        commands
            .entity(entity)
            .insert((Interactive, HelloWorldInteraction));
    }

    commands.entity(entity).insert(GameView);
}

fn resolve_local_path(card: &DemoCardConfig, config: &GameConfig) -> Vec<Vec2> {
    if let Some(path) = &card.path {
        path.iter().copied().map(vec2_from_pair).collect()
    } else if let Some(bezier) = &card.bezier {
        sample_cubic_closed_path(
            &bezier.anchors,
            &bezier.controls_a,
            &bezier.controls_b,
            config.scene.bezier_steps_per_curve,
        )
    } else {
        let half_size = card.size() * 0.5;
        vec![
            Vec2::new(-half_size.x, -half_size.y),
            Vec2::new(half_size.x, -half_size.y),
            Vec2::new(half_size.x, half_size.y),
            Vec2::new(-half_size.x, half_size.y),
        ]
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
