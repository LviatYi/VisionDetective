use crate::card::card_params::{CardSpawnParams, CardSpecializedParam};
use crate::card::{Card, CardKind, CardSpecializedInstaller};
use crate::physics::obstacle::Obstacle;
use crate::register_card_specialized_installer;
use bevy::ecs::system::EntityCommands;
use bevy::math::Vec2;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub struct ObstacleCardSpecializedInstaller;

impl CardSpecializedInstaller for ObstacleCardSpecializedInstaller {
    type Param = ObstacleCardParams;

    const TYPE_ID: &'static str = "obstacle";
}

register_card_specialized_installer!(ObstacleCardSpecializedInstaller);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardObstacleType {
    Full,
    Path(Vec<Vec2>),
    Bezier {
        anchors: Vec<Vec2>,
        controls_a: Vec<Vec2>,
        controls_b: Vec<Vec2>,
        steps_per_curve: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObstacleCardParams {
    pub obstacle_def: CardObstacleType,
}

impl CardSpecializedParam for ObstacleCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Obstacle
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>) {
        entity.insert(Obstacle::new(match &self.obstacle_def {
            CardObstacleType::Full => Card::card_cut_polygon(&spawn_params.config.cards),
            CardObstacleType::Path(path) => path.clone(),
            CardObstacleType::Bezier {
                anchors,
                controls_a,
                controls_b,
                steps_per_curve,
            } => sample_cubic_closed_path(anchors, controls_a, controls_b, *steps_per_curve),
        }));
    }
}

fn sample_cubic_closed_path(
    anchors: &[Vec2],
    controls_a: &[Vec2],
    controls_b: &[Vec2],
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
