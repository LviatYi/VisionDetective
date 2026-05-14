use crate::card::Card;
use crate::config::CardConfig;
use bevy::math::Vec2;
use bevy::prelude::Transform;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeType {
    Full,
    Path(Vec<Vec2>),
    Bezier {
        anchors: Vec<Vec2>,
        controls_a: Vec<Vec2>,
        controls_b: Vec<Vec2>,
        steps_per_curve: usize,
    },
}

impl ShapeType {
    pub fn sample_path(&self, card_config: &CardConfig) -> Vec<Vec2> {
        match &self {
            ShapeType::Full => Card::card_cut_polygon(&card_config),
            ShapeType::Path(path) => path.clone(),
            ShapeType::Bezier {
                anchors,
                controls_a,
                controls_b,
                steps_per_curve,
            } => sample_cubic_closed_path(anchors, controls_a, controls_b, *steps_per_curve),
        }
    }
}

pub fn sample_cubic_closed_path(
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

#[derive(Debug, Clone)]
pub struct Area {
    pub local_path: Vec<Vec2>,
}

impl Area {
    pub fn new(local_path: Vec<Vec2>) -> Self {
        Self { local_path }
    }

    pub fn local_bounds(&self) -> Option<(Vec2, Vec2)> {
        let mut points = self.local_path.iter();
        let first = *points.next()?;
        let mut min = first;
        let mut max = first;

        for &point in points {
            min = min.min(point);
            max = max.max(point);
        }

        Some((min, max))
    }

    pub fn world_path(&self, transform: &Transform) -> Vec<Vec2> {
        self.local_path
            .iter()
            .map(|point| transform.transform_point(point.extend(0.0)).truncate())
            .collect()
    }

    pub fn contains_local_point(&self, point: Vec2) -> bool {
        point_in_polygon(point, &self.local_path)
    }

    pub fn contains_world_point(&self, transform: &Transform, point: Vec2) -> bool {
        let world_path = self.world_path(transform);
        point_in_polygon(point, &world_path)
    }
}

fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut previous = polygon.len() - 1;

    for current in 0..polygon.len() {
        let current_point = polygon[current];
        let previous_point = polygon[previous];

        let intersects = (current_point.y > point.y) != (previous_point.y > point.y)
            && point.x
                < (previous_point.x - current_point.x) * (point.y - current_point.y)
                    / (previous_point.y - current_point.y)
                    + current_point.x;

        if intersects {
            inside = !inside;
        }

        previous = current;
    }

    inside
}
