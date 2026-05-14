use crate::GameView;
use crate::card::Card;
use crate::card::card_params::{CardSceneParam, SpawnCardSystemParams};
use crate::card::spawn_scenery_by_appearance;
use crate::card::specialized::trap::is_covered_by_higher_card;
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinBehaviorStatus, PlayerCoinState};
use crate::physics::area::Area;
use crate::scene::SceneLayer;
use crate::tools::Disable;
use bevy::math::Vec2;
use bevy::prelude::{
    Color, Commands, Component, Deref, Entity, Gizmos, GlobalTransform, Mut, Quat, Query,
    Transform, With, Without,
};
use fast_poisson::Poisson2D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainParam {
    #[serde(default, rename = "type")]
    pub terrain_type: TerrainType,

    #[serde(default)]
    pub path: Vec<Vec2>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance_id: Option<u32>,

    #[serde(default = "default_min_distance")]
    pub min_distance: f32,

    #[serde(default = "default_rejection_attempts")]
    pub rejection_attempts: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cards: Option<usize>,

    #[serde(default = "default_order_offset")]
    pub order_offset: f32,

    #[serde(default)]
    pub rotation: f32,

    #[serde(default)]
    pub rotation_jitter: f32,

    #[serde(default)]
    pub seed: u64,

    #[serde(default)]
    pub scene_param: TerrainSceneParam,
}

impl Default for TerrainParam {
    fn default() -> Self {
        Self {
            terrain_type: TerrainType::default(),
            path: Vec::new(),
            appearance_id: None,
            min_distance: default_min_distance(),
            rejection_attempts: default_rejection_attempts(),
            max_cards: None,
            order_offset: default_order_offset(),
            rotation: 0.0,
            rotation_jitter: 0.0,
            seed: 0,
            scene_param: TerrainSceneParam::default(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainType {
    #[default]
    Trap,
    Scenery,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TerrainSceneParam {
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

pub fn spawn_terrain(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    terrain: &TerrainParam,
) {
    if terrain.path.len() < 3 {
        bevy::log::warn!("terrain path must contain at least 3 points");
        return;
    }

    spawn_terrain_fill(commands, spawn_params, terrain);

    if terrain.terrain_type == TerrainType::Trap {
        commands.spawn((
            terrain_transform(terrain),
            TrapTerrain::new(terrain.path.clone()),
            GameView,
        ));
    }
}

#[derive(Component, Clone, Deref)]
pub struct TrapTerrain {
    pub area: Area,
}

impl TrapTerrain {
    pub fn new(local_path: Vec<Vec2>) -> Self {
        Self {
            area: Area::new(local_path),
        }
    }
}

pub fn handle_player_trap_terrain_collision(
    mut player_query: Query<(Mut<PlayerCoinState>, &Transform), With<PlayerCoin>>,
    trap_terrain_query: Query<(Entity, &Transform, &TrapTerrain), Without<Disable>>,
    card_query: Query<(Entity, &Card, &GlobalTransform), Without<Disable>>,
) {
    for (mut player_state, player_transform) in &mut player_query {
        if !player_state.is_on_ground() {
            continue;
        }

        let player_position = player_transform.translation.truncate();
        let falls_into_trap = trap_terrain_query
            .iter()
            .filter(|(_, terrain_transform, terrain)| {
                terrain.contains_world_point(terrain_transform, player_position)
            })
            .any(|(terrain_entity, terrain_transform, _)| {
                !is_covered_by_higher_card(
                    terrain_entity,
                    terrain_transform.translation.z,
                    player_position,
                    &card_query,
                )
            });

        if falls_into_trap {
            player_state.set_state(PlayerCoinBehaviorStatus::Death);
        }
    }
}

pub fn draw_trap_terrain_paths(
    mut gizmos: Gizmos,
    trap_terrain_query: Query<(&Transform, &TrapTerrain)>,
) {
    for (transform, terrain) in &trap_terrain_query {
        draw_path(
            &mut gizmos,
            &terrain.world_path(transform),
            Color::srgb(0.95, 0.24, 0.24),
        );
    }
}

pub fn draw_path(gizmos: &mut Gizmos, world_path: &[Vec2], color: Color) {
    if world_path.len() < 2 {
        return;
    }

    for index in 0..world_path.len() {
        let a = world_path[index];
        let b = world_path[(index + 1) % world_path.len()];
        gizmos.line_2d(a, b, color);
        gizmos.circle_2d(a, 3.0, color);
    }
}

fn sample_poisson_disk_in_area(
    area: &Area,
    min_distance: f32,
    rejection_attempts: usize,
    max_points: Option<usize>,
    seed: u64,
) -> Vec<Vec2> {
    let Some((min, max)) = area.local_bounds() else {
        return Vec::new();
    };
    let size = max - min;
    if size.x <= 0.0 || size.y <= 0.0 {
        return Vec::new();
    }

    let max_points = max_points.unwrap_or(usize::MAX);
    if max_points == 0 {
        return Vec::new();
    }

    Poisson2D::new()
        .with_dimensions([size.x as f64, size.y as f64], min_distance as f64)
        .with_seed(seed)
        .with_samples(rejection_attempts.max(1) as u32)
        .iter()
        .map(|point| min + Vec2::new(point[0] as f32, point[1] as f32))
        .filter(|point| area.contains_local_point(*point))
        .take(max_points)
        .collect()
}

fn spawn_terrain_fill(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    terrain: &TerrainParam,
) {
    let Some(appearance_id) = terrain.appearance_id else {
        return;
    };
    if terrain.min_distance <= 0.0 {
        return;
    }

    let Some(appearance) = spawn_params
        .card_presets_config
        .appearances
        .iter()
        .find(|appearance| appearance.id == appearance_id)
        .cloned()
    else {
        bevy::log::warn!("terrain appearance {} is not found", appearance_id);
        return;
    };

    let area = Area::new(terrain.path.clone());
    let points = sample_poisson_disk_in_area(
        &area,
        terrain.min_distance,
        terrain.rejection_attempts,
        terrain.max_cards,
        terrain.seed,
    );
    if points.is_empty() {
        return;
    }

    let transform = terrain_transform(terrain);
    let points_count = points.len();
    let inner_z_offset = 1.0 / points_count as f32;

    for (index, point) in points.into_iter().enumerate() {
        let world_position = transform.transform_point(point.extend(0.0)).truncate();
        let rotation = terrain.rotation
            + terrain.scene_param.rotation
            + deterministic_signed_unit(index as u64, appearance.id as u64)
                * terrain.rotation_jitter;

        let entity = spawn_scenery_by_appearance(
            commands,
            spawn_params,
            &appearance,
            CardSceneParam {
                position: world_position,
                rotation,
                order: terrain.scene_param.order
                    + terrain.order_offset
                    + index as f32 * inner_z_offset,
                enable_if: terrain.scene_param.enable_if.clone(),
                disable_if: terrain.scene_param.disable_if.clone(),
                ..Default::default()
            },
        );
        commands.entity(entity).insert(GameView);
    }
}

fn terrain_transform(terrain: &TerrainParam) -> Transform {
    Transform::from_translation(
        terrain
            .scene_param
            .position
            .extend(SceneLayer::Card.get_layer_base_z() + terrain.scene_param.order),
    )
    .with_rotation(Quat::from_rotation_z(terrain.scene_param.rotation))
}

fn default_min_distance() -> f32 {
    120.0
}

fn default_rejection_attempts() -> usize {
    30
}

fn default_order_offset() -> f32 {
    0.01
}

fn deterministic_signed_unit(index: u64, salt: u64) -> f32 {
    let hash = index
        .wrapping_mul(1_315_423_911)
        .wrapping_add(salt)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    ((hash >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0
}
