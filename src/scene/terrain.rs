use crate::GameView;
use crate::card::card_params::{CardSceneParam, CardSpawnParams};
use crate::card::spawn_scenery_card_by_appearance;
use crate::config::terrain_config::TerrainPresetsConfig;
use crate::physics::area::Area;
use crate::scene::SceneLayer;
use bevy::math::Vec2;
use bevy::prelude::{Commands, Quat, Transform};
use fast_poisson::Poisson2D;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TerrainParam {
    pub preset_id: u32,

    #[serde(default)]
    pub scene_param: TerrainSceneParam,
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
    spawn_params: &mut CardSpawnParams<'_>,
    terrain_presets: &TerrainPresetsConfig,
    terrain: &TerrainParam,
) {
    let Some(preset) = terrain_presets.get(terrain.preset_id) else {
        bevy::log::warn!("terrain preset {} is not found", terrain.preset_id);
        return;
    };
    if preset.appearance_ids.is_empty() || preset.min_distance <= 0.0 {
        return;
    }

    let appearances = preset
        .appearance_ids
        .iter()
        .filter_map(|appearance_id| {
            spawn_params
                .card_presets_config
                .appearances
                .iter()
                .find(|appearance| appearance.id == *appearance_id)
                .cloned()
        })
        .collect::<Vec<_>>();
    if appearances.is_empty() {
        return;
    }

    let area = Area::new(preset.shape_def.sample_path(&spawn_params.config.cards));
    let points = sample_poisson_disk_in_area(
        &area,
        preset.min_distance,
        preset.rejection_attempts,
        preset.max_cards,
    );
    let terrain_transform = Transform::from_translation(
        terrain
            .scene_param
            .position
            .extend(SceneLayer::Card.get_layer_base_z() + terrain.scene_param.order),
    )
    .with_rotation(Quat::from_rotation_z(terrain.scene_param.rotation));

    for (index, point) in points.into_iter().enumerate() {
        let appearance = &appearances[index % appearances.len()];
        let world_position = terrain_transform
            .transform_point(point.extend(0.0))
            .truncate();
        let rotation = preset.rotation
            + terrain.scene_param.rotation
            + deterministic_signed_unit(index as u64, appearance.id as u64)
                * preset.rotation_jitter;

        let entity = spawn_scenery_card_by_appearance(
            commands,
            spawn_params,
            appearance,
            CardSceneParam {
                position: world_position,
                rotation,
                order: terrain.scene_param.order
                    + preset.order_offset
                    + index as f32 * preset.order_step,
                enable_if: terrain.scene_param.enable_if.clone(),
                disable_if: terrain.scene_param.disable_if.clone(),
                ..Default::default()
            },
        );
        commands.entity(entity).insert(GameView);
    }
}

fn sample_poisson_disk_in_area(
    area: &Area,
    min_distance: f32,
    rejection_attempts: usize,
    max_points: Option<usize>,
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
        .with_seed(area_seed(area, min_distance))
        .with_samples(rejection_attempts.max(1) as u32)
        .iter()
        .map(|point| min + Vec2::new(point[0] as f32, point[1] as f32))
        .filter(|point| area.contains_local_point(*point))
        .take(max_points)
        .collect()
}

fn area_seed(area: &Area, min_distance: f32) -> u64 {
    let mut hash = min_distance.to_bits() as u64;
    for point in &area.local_path {
        hash = hash.wrapping_mul(1_099_511_628_211);
        hash ^= point.x.to_bits() as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
        hash ^= point.y.to_bits() as u64;
    }
    hash
}

fn deterministic_signed_unit(index: u64, salt: u64) -> f32 {
    let hash = index
        .wrapping_mul(1_315_423_911)
        .wrapping_add(salt)
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    ((hash >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0
}
