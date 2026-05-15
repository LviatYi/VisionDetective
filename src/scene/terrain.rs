use crate::GameView;
use crate::card::card_params::{CardSceneParam, SpawnCardSystemParams};
use crate::card::spawn_scenery_by_appearance;
use crate::card::specialized::trap::Trap;
use crate::config::GameConfig;
use crate::physics::area::Area;
use crate::scene::{SCENE_ROTATION_STEP, SceneLayer, SceneParam, Z_OFFSET_CARD_OF_TERRAIN};
use bevy::asset::RenderAssetUsages;
use bevy::math::Vec2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::{
    Assets, ChildSpawnerCommands, Color, ColorMaterial, Commands, Component, Deref, Entity, Gizmos,
    Mesh, Mesh2d, MeshMaterial2d, Quat, Query, Res, ResMut, Transform, Visibility, Without,
};
use bevy::utils::default;
use fast_poisson::Poisson2D;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

const TERRAIN_BOUNDARY_WIDTH: f32 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainParam {
    #[serde(default, rename = "type")]
    pub terrain_type: TerrainType,

    #[serde(default)]
    pub path: Vec<Vec2>,

    #[serde(default)]
    pub appearance_id: u32,

    #[serde(default = "default_min_distance")]
    pub min_distance: f32,

    #[serde(default = "default_rejection_attempts")]
    pub rejection_attempts: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cards: Option<usize>,

    #[serde(default)]
    pub rotation: f32,

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
            appearance_id: default_appearance_id(),
            min_distance: default_min_distance(),
            rejection_attempts: default_rejection_attempts(),
            max_cards: None,
            rotation: 0.0,
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

#[derive(Default, Debug, Clone, Deref, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerrainSceneParam(pub SceneParam);

impl From<SceneParam> for TerrainSceneParam {
    fn from(scene_param: SceneParam) -> Self {
        Self(scene_param)
    }
}

impl From<TerrainSceneParam> for SceneParam {
    fn from(terrain_scene_param: TerrainSceneParam) -> Self {
        terrain_scene_param.0
    }
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

    let mut entity = commands.spawn((
        terrain_transform(terrain),
        TerrainBoundary::new(terrain.path.clone(), terrain.terrain_type),
        Visibility::Visible,
        GameView,
    ));

    if terrain.terrain_type == TerrainType::Trap {
        entity.insert(Trap::new(terrain.path.clone()));
    }
}

#[derive(Component, Clone)]
pub struct TerrainBoundary {
    pub area: Area,
    pub terrain_type: TerrainType,
}

impl TerrainBoundary {
    pub fn new(local_path: Vec<Vec2>, terrain_type: TerrainType) -> Self {
        Self {
            area: Area::new(local_path),
            terrain_type,
        }
    }
}

#[derive(Component)]
pub struct TerrainBoundaryMeshSpawned;

pub fn draw_editor_terrain_paths(
    mut gizmos: Gizmos,
    config: Res<GameConfig>,
    terrain_query: Query<(&Transform, &TerrainBoundary)>,
) {
    for (transform, terrain) in &terrain_query {
        let color = config.cards.terrain_fill_color(terrain.terrain_type);
        draw_path_by_gizmo(&mut gizmos, &terrain.area.world_path(transform), color);
    }
}

pub fn spawn_terrain_boundary_meshes(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain_query: Query<(Entity, &TerrainBoundary), Without<TerrainBoundaryMeshSpawned>>,
) {
    for (entity, terrain) in &terrain_query {
        let color = config.cards.terrain_fill_color(terrain.terrain_type);
        let mut entity_commands = commands.entity(entity);
        entity_commands.with_children(|parent| {
            spawn_terrain_boundary(
                parent,
                meshes.as_mut(),
                materials.as_mut(),
                &terrain.area.local_path,
                color,
            );
        });
        entity_commands.insert(TerrainBoundaryMeshSpawned);
    }
}

fn draw_path_by_gizmo(gizmos: &mut Gizmos, world_path: &[Vec2], color: Color) {
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

fn spawn_terrain_boundary(
    parent: &mut ChildSpawnerCommands<'_>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    local_path: &[Vec2],
    color: Color,
) {
    let Some(mesh) = terrain_boundary_mesh(local_path, TERRAIN_BOUNDARY_WIDTH) else {
        return;
    };

    parent.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(materials.add(color)),
        Transform::default(),
    ));
}

fn terrain_boundary_mesh(local_path: &[Vec2], width: f32) -> Option<Mesh> {
    let mut positions = Vec::with_capacity(local_path.len() * 4);
    let mut uvs = Vec::with_capacity(local_path.len() * 4);
    let mut indices = Vec::with_capacity(local_path.len() * 6);
    let half_width = width * 0.5;

    for index in 0..local_path.len() {
        let start = local_path[index];
        let end = local_path[(index + 1) % local_path.len()];
        let edge = end - start;
        if edge.length_squared() <= f32::EPSILON {
            continue;
        }

        let normal = Vec2::new(-edge.y, edge.x).normalize() * half_width;
        let base = positions.len() as u32;
        let vertices = [start + normal, end + normal, end - normal, start - normal];

        positions.extend(vertices.map(|point| [point.x, point.y, 0.0]));
        uvs.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
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
    if terrain.min_distance <= 0.0 {
        return;
    }

    let Some(appearance) = spawn_params
        .card_presets_config
        .appearances
        .iter()
        .find(|appearance| appearance.id == terrain.appearance_id)
        .cloned()
    else {
        bevy::log::warn!("terrain appearance {} is not found", terrain.appearance_id);
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
    let inner_z_start_at = Z_OFFSET_CARD_OF_TERRAIN;
    let inner_z_offset = Z_OFFSET_CARD_OF_TERRAIN.abs() / points_count as f32;

    for (index, point) in points.into_iter().enumerate() {
        let world_position = transform.transform_point(point.extend(0.0)).truncate();
        let rotation = terrain.rotation
            + terrain.scene_param.rotation
            + deterministic_rotation(index as u64, terrain.seed);

        let entity = spawn_scenery_by_appearance(
            commands,
            spawn_params,
            &appearance,
            CardSceneParam {
                data: SceneParam {
                    position: world_position,
                    rotation,
                    order: terrain.scene_param.order
                        + inner_z_start_at
                        + index as f32 * inner_z_offset,
                    enable_if: terrain.scene_param.enable_if.clone(),
                    disable_if: terrain.scene_param.disable_if.clone(),
                    ..default()
                },
                ..default()
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
            .extend(SceneLayer::SceneObjects.get_layer_base_z() + terrain.scene_param.order),
    )
    .with_rotation(Quat::from_rotation_z(terrain.scene_param.rotation))
}

fn default_min_distance() -> f32 {
    120.0
}

fn default_appearance_id() -> u32 {
    100014
}

fn default_rejection_attempts() -> usize {
    30
}

pub fn random_terrain_seed() -> u64 {
    rand::rng().random()
}

fn deterministic_rotation(index: u64, seed: u64) -> f32 {
    let step_count = (std::f32::consts::TAU / SCENE_ROTATION_STEP).round() as u32;
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(index));
    let step = rng.random_range(0..step_count);
    step as f32 * SCENE_ROTATION_STEP
}
