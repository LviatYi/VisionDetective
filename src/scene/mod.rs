use bevy::camera::{Camera2d, OrthographicProjection, Projection};
use bevy::math::Vec2;
use bevy::prelude::Transform;
#[cfg(all(debug_assertions, feature = "dev-inspector"))]
use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;
use serde::{Deserialize, Serialize};

pub mod demo_level;
pub mod terrain;

#[derive(Debug, Copy, Clone)]
pub enum SceneLayer {
    TerrainBackground,
    SceneObjects,
    PlayerVision,
    PlayerCoinLandingEffect,
    PlayerCoin,
    GizmoAimingMarker,
    Coin,
}

impl SceneLayer {
    pub fn get_layer_base_z(&self) -> f32 {
        match self {
            SceneLayer::TerrainBackground => 1000.0,
            SceneLayer::SceneObjects => 2000.0,
            SceneLayer::PlayerVision => 4000.0,
            SceneLayer::PlayerCoinLandingEffect => 4000.5,
            SceneLayer::PlayerCoin => 4001.0,
            SceneLayer::GizmoAimingMarker => {
                SceneLayer::PlayerCoin.get_layer_base_z() + Z_OFFSET_PLAYER_GIZMO_AIMING_MARKER
            }
            SceneLayer::Coin => 3100.0,
        }
    }
}

#[cfg(all(debug_assertions, feature = "dev-inspector"))]
pub fn get_layered_game_scene_camera2d_bundle()
-> (Camera2d, Projection, Transform, PrimaryEguiContext) {
    (
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            near: -10000.0,
            far: 100.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
        PrimaryEguiContext,
    )
}

#[cfg(not(all(debug_assertions, feature = "dev-inspector")))]
pub fn get_layered_game_scene_camera2d_bundle() -> (Camera2d, Projection, Transform) {
    (
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            near: -10000.0,
            far: 100.0,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
    )
}

//region Card Range Z [0,0.01)
pub const Z_OFFSET_CARD_BACKGROUND: f32 = 0.001;
pub const Z_OFFSET_CARD_IMAGE: f32 = 0.002;
pub const Z_OFFSET_CARD_TITLE: f32 = 0.003;
pub const Z_OFFSET_QUESTION_MARK_FILL_MASK: f32 = 0.004;
pub const Z_OFFSET_QUESTION_MARK_CARD_IMAGE: f32 = 0.005;
pub const Z_OFFSET_QUESTION_MARK_IMAGE: f32 = 0.006;
//endregion

//region Terrain Range [-0.5,0]
pub const Z_OFFSET_CARD_OF_TERRAIN: f32 = -0.5;
pub const Z_OFFSET_TERRAIN_BOUNDARY: f32 = 0.99;
//endregion

pub const Z_OFFSET_PLAYER_GIZMO_AIMING_MARKER: f32 = 0.01;

pub const SCENE_ROTATION_STEP: f32 = std::f32::consts::PI / 6.0;

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
