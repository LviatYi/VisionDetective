pub mod card_config;
pub mod character_config;

use crate::asset::runtime_root;
use bevy::color::Color;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Resource, Srgba};
use serde::Deserialize;
use std::fs;

#[derive(Resource, Clone, Deserialize)]
pub struct GameConfig {
    pub window: WindowConfig,
    pub visuals: VisualConfig,
    pub player: PlayerConfig,
    pub ui: UiConfig,
    pub cards: CardConfig,
    pub physics: PhysicsConfig,
    pub vision: VisionConfig,
    pub obstacles: ObstacleRenderConfig,
    pub scene: SceneConfig,
    pub assets: AssetConfig,
}

impl GameConfig {
    pub fn load() -> Self {
        let path = runtime_root()
            .join("assets")
            .join("config")
            .join("game-static-config.toml");
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read config {}: {error}", path.display()));

        toml::from_str(&raw)
            .unwrap_or_else(|error| panic!("failed to parse config {}: {error}", path.display()))
    }
}

#[derive(Clone, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub clear_color: String,
}

impl WindowConfig {
    pub fn clear_color(&self) -> Color {
        Color::Srgba(Srgba::hex(&self.clear_color).unwrap_or(Srgba::WHITE))
    }
}

#[derive(Clone, Deserialize)]
pub struct VisualConfig {
    pub player_radius: f32,
    pub player_color: [f32; 3],
    pub pointer_radius: f32,
    pub pointer_color: [f32; 3],
}

impl VisualConfig {
    pub fn player_color(&self) -> Color {
        Color::srgb(
            self.player_color[0],
            self.player_color[1],
            self.player_color[2],
        )
    }

    pub fn pointer_color(&self) -> Color {
        Color::srgb(
            self.pointer_color[0],
            self.pointer_color[1],
            self.pointer_color[2],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct PlayerConfig {
    pub max_eject_distance: f32,
    pub max_planar_speed: f32,
    pub max_vertical_speed: f32,
    pub min_vertical_speed: f32,
    pub min_launch_distance: f32,
    pub height_scale_factor: f32,
    pub aim_ring_padding: f32,
    pub aim_ring_color: [f32; 4],
    pub charge_line_color: [f32; 3],
    pub launch_line_color: [f32; 3],
    pub launch_marker_radius: f32,
}

impl PlayerConfig {
    pub fn aim_ring_color(&self) -> Color {
        Color::srgba(
            self.aim_ring_color[0],
            self.aim_ring_color[1],
            self.aim_ring_color[2],
            self.aim_ring_color[3],
        )
    }

    pub fn charge_line_color(&self) -> Color {
        Color::srgb(
            self.charge_line_color[0],
            self.charge_line_color[1],
            self.charge_line_color[2],
        )
    }

    pub fn launch_line_color(&self) -> Color {
        Color::srgb(
            self.launch_line_color[0],
            self.launch_line_color[1],
            self.launch_line_color[2],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct UiConfig {
    pub tutorial_text: String,
    pub status_initial_text: String,
    pub tutorial_font_size: f32,
    pub status_font_size: f32,
    pub tutorial_offset: [f32; 2],
    pub status_offset: [f32; 2],
    pub status_color: [f32; 3],
}

impl UiConfig {
    pub fn status_color(&self) -> Color {
        Color::srgb(
            self.status_color[0],
            self.status_color[1],
            self.status_color[2],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct PhysicsConfig {
    pub bounce_factor: f32,
    pub landing_speed_loss: f32,
    pub sliding_friction: f32,
    pub stop_speed: f32,
    pub gravity: f32,
    pub max_ground_bounce_count: u8,
}

#[derive(Clone, Deserialize)]
pub struct VisionConfig {
    pub radius: f32,
    pub ray_count: usize,
    pub vertex_epsilon: f32,
    pub intersection_epsilon: f32,
    pub fill_color: [f32; 4],
    pub outline_color: [f32; 4],
}

impl VisionConfig {
    pub fn fill_color(&self) -> Color {
        Color::srgba(
            self.fill_color[0],
            self.fill_color[1],
            self.fill_color[2],
            self.fill_color[3],
        )
    }

    pub fn outline_color(&self) -> Color {
        Color::srgba(
            self.outline_color[0],
            self.outline_color[1],
            self.outline_color[2],
            self.outline_color[3],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct ObstacleRenderConfig {
    pub edge_color: [f32; 3],
    pub vertex_color: [f32; 3],
    pub vertex_radius: f32,
}

impl ObstacleRenderConfig {
    pub fn edge_color(&self) -> Color {
        Color::srgb(self.edge_color[0], self.edge_color[1], self.edge_color[2])
    }

    pub fn vertex_color(&self) -> Color {
        Color::srgb(
            self.vertex_color[0],
            self.vertex_color[1],
            self.vertex_color[2],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct SceneConfig {
    pub bezier_steps_per_curve: usize,
    pub card_dealing_deck_position: [f32; 2],
}

#[derive(Clone, Deserialize)]
pub struct CardConfig {
    pub background_card_image_path: String,
    pub scenery_fill_color: String,
    pub obstacle_fill_color: String,
    pub interaction_fill_color: String,
    pub clue_fill_color: String,
    pub trap_fill_color: String,
    pub default_fill_color: String,
    pub corner_radius: f32,
    pub rounded_corner_segments: usize,
    pub normal_image_size_ratio: [f32; 2],
    pub normal_image_offset_y: f32,
    pub title_font_size: f32,
    pub title_text_width_ratio: f32,
    pub title_offset_y_ratio: f32,
    pub title_glass_padding: [f32; 2],
    pub title_glass_corner_radius: f32,
}

impl CardConfig {
    pub fn card_fill_color(&self, kind: crate::card::CardKind) -> Color {
        match kind {
            crate::card::CardKind::Scenery => Srgba::hex(&self.scenery_fill_color),
            crate::card::CardKind::Obstacle => Srgba::hex(&self.obstacle_fill_color),
            crate::card::CardKind::Interaction => Srgba::hex(&self.interaction_fill_color),
            crate::card::CardKind::Clue => Srgba::hex(&self.clue_fill_color),
            crate::card::CardKind::Trap => Srgba::hex(&self.trap_fill_color),
        }
        .map(|c| Color::Srgba(c))
        .unwrap_or_else(|_| self.default_fill_color())
    }

    pub fn terrain_fill_color(&self, kind: crate::scene::terrain::TerrainType) -> Color {
        match kind {
            crate::scene::terrain::TerrainType::Scenery => Srgba::hex(&self.scenery_fill_color),
            crate::scene::terrain::TerrainType::Trap => Srgba::hex(&self.trap_fill_color),
        }
        .map(|c| Color::Srgba(c))
        .unwrap_or_else(|_| self.default_fill_color())
    }

    pub fn default_fill_color(&self) -> Color {
        Color::Srgba(Srgba::hex(&self.default_fill_color).unwrap_or_default())
    }

    pub fn normal_image_size_ratio(&self) -> Vec2 {
        Vec2::new(
            self.normal_image_size_ratio[0],
            self.normal_image_size_ratio[1],
        )
    }

    pub fn title_glass_padding(&self) -> Vec2 {
        Vec2::new(self.title_glass_padding[0], self.title_glass_padding[1])
    }
}

#[derive(Clone, Deserialize)]
pub struct ObstacleConfig {
    pub translation: [f32; 3],
    pub rotation_z: f32,
    pub path: Option<Vec<[f32; 2]>>,
    pub bezier: Option<BezierObstacleConfig>,
}

impl ObstacleConfig {
    pub fn translation(&self) -> Vec3 {
        Vec3::new(
            self.translation[0],
            self.translation[1],
            self.translation[2],
        )
    }
}

#[derive(Clone, Deserialize)]
pub struct BezierObstacleConfig {
    pub anchors: Vec<[f32; 2]>,
    pub controls_a: Vec<[f32; 2]>,
    pub controls_b: Vec<[f32; 2]>,
}

#[derive(Clone, Deserialize)]
pub struct AssetConfig {
    pub default_font: String,
}

pub fn vec2_from_pair(pair: [f32; 2]) -> Vec2 {
    Vec2::new(pair[0], pair[1])
}
