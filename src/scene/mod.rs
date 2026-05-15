use bevy::camera::{Camera2d, OrthographicProjection, Projection};
use bevy::math::Vec2;
use serde::{Deserialize, Serialize};

pub mod demo_level;
pub mod terrain;

#[derive(Debug, Copy, Clone)]
pub enum SceneLayer {
    Card,
    PlayerVision,
    PlayerCoin,
    GizmoAimingMarker,
    Coin,
}

impl SceneLayer {
    pub fn get_layer_base_z(&self) -> f32 {
        match self {
            SceneLayer::Card => 10000.0,
            SceneLayer::PlayerVision => 30000.0,
            SceneLayer::PlayerCoin => 30001.0,
            SceneLayer::GizmoAimingMarker => {
                SceneLayer::PlayerCoin.get_layer_base_z() + Z_OFFSET_PLAYER_GIZMO_AIMING_MARKER
            }
            SceneLayer::Coin => 30100.0,
        }
    }
}

pub fn get_layered_game_scene_camera2d_bundle() -> (Camera2d, Projection) {
    (
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            near: -100000.0,
            far: 100000.0,
            ..OrthographicProjection::default_2d()
        }),
    )
}

pub const Z_OFFSET_CARD_BACKGROUND: f32 = 0.01;
pub const Z_OFFSET_CARD_IMAGE: f32 = 0.02;
pub const Z_OFFSET_CARD_TITLE: f32 = 0.03;

pub const Z_OFFSET_PLAYER_GIZMO_AIMING_MARKER: f32 = 0.01;

/// Scene param for card instance.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct SceneParam {
    pub position: Vec2,

    #[serde(default)]
    pub rotation: f32,

    #[serde(default)]
    pub order: f32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_if: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_if: Option<String>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}
