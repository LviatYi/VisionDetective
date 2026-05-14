use bevy::camera::{Camera2d, OrthographicProjection, Projection};

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
        (match self {
            SceneLayer::Card => 10000,
            SceneLayer::PlayerVision => 30000,
            SceneLayer::PlayerCoin => 30001,
            SceneLayer::GizmoAimingMarker => 30002,
            SceneLayer::Coin => 30100,
        } as f32)
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
