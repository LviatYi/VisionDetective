use crate::card::card_params::{CardSceneParam, CardSpawnParams, CardSpecialized};
use crate::card::{CARD_SIZE, Card, CardKind};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::PlayerCoinState;
use crate::config::GameConfig;
use crate::game_view::GameState;
use crate::physics::obstacle::Obstacle;
use crate::physics::vision::compute_visible_points;
use crate::register_card_specialized_param;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::EntityCommands;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use geo::algorithm::unary_union;
use geo::orient::{Direction, Orient};
use geo::{
    Area, BooleanOps, Coord as GeoCoord, LineString as GeoLineString,
    MultiPolygon as GeoMultiPolygon, Polygon as GeoPolygon, TriangulateEarcut,
};
use serde::{Deserialize, Serialize};

pub struct ClueCardPlugin;

#[derive(Default, Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClueRevealThreshold {
    Any,
    #[default]
    Normal,
    Full,
    Custom(f32),
}

impl ClueRevealThreshold {
    pub fn get_threshold(&self) -> f32 {
        match self {
            ClueRevealThreshold::Any => 0.01,
            ClueRevealThreshold::Normal => 0.6,
            ClueRevealThreshold::Full => 1.0,
            ClueRevealThreshold::Custom(value) => *value,
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct ClueCard {
    pub revealed: bool,
    pub param: ClueCardParams,
    question_mark: Option<Entity>,
    illumination: Option<Entity>,
    illuminated_regions: Vec<GeoMultiPolygon<f32>>,
}

#[derive(Component)]
struct ClueQuestionMark;

#[derive(Component)]
struct ClueIllumination;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClueCardParams {
    #[serde(default)]
    pub reveal_threshold: ClueRevealThreshold,
    pub interaction_prefab_id: u32,
    pub interaction_target_scene_param: CardSceneParam,
}

impl CardSpecialized for ClueCardParams {
    fn kind(&self) -> CardKind {
        CardKind::Clue
    }

    fn spawn_with(&self, entity: &mut EntityCommands<'_>, spawn_params: &mut CardSpawnParams<'_>) {
        let mut question_mark = None;
        entity.with_children(|parent| {
            question_mark = Some(
                parent
                    .spawn((
                        Text2d::new("?"),
                        TextFont {
                            font: spawn_params
                                .asset_server
                                .load(spawn_params.config.assets.default_font.clone()),
                            font_size: CARD_SIZE.y * 0.42,
                            ..default()
                        },
                        TextColor(Color::srgb(0.08, 0.08, 0.09)),
                        Anchor::CENTER,
                        Transform::from_xyz(0.0, 0.0, 0.42),
                        ClueQuestionMark,
                    ))
                    .id(),
            );
        });

        entity.insert(ClueCard {
            revealed: false,
            param: self.clone(),
            question_mark,
            illumination: None,
            illuminated_regions: Vec::new(),
        });
    }
}

impl Plugin for ClueCardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, reveal_clues.run_if(in_state(GameState::InGame)));
    }
}

fn reveal_clues(
    mut commands: Commands,
    config: Res<GameConfig>,
    player_coin_state: Res<PlayerCoinState>,
    player_query: Query<&Transform, With<PlayerCoin>>,
    mut clue_query: Query<(Entity, &mut ClueCard, &GlobalTransform)>,
    obstacle_query: Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
    mut illumination_mesh_query: Query<&mut Mesh2d, With<ClueIllumination>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !player_coin_state.just_ejected() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for (entity, mut clue, transform) in &mut clue_query {
        if clue.revealed {
            continue;
        }

        let Some(stamp) = build_visible_clue_stamp(
            &config,
            player_transform.translation.truncate(),
            transform,
            &obstacle_query,
        ) else {
            continue;
        };
        clue.illuminated_regions.push(stamp);

        let (merged_mesh, illuminated_area) = build_merged_illumination_mesh(&clue);
        let coverage = illuminated_area / Card::card_area();

        if coverage >= clue.param.reveal_threshold.get_threshold() {
            clue.revealed = true;
            despawn_clue_visual_feedback(&mut commands, &mut clue);
            continue;
        }

        update_clue_illumination_visual(
            &mut commands,
            &config,
            entity,
            &mut clue,
            merged_mesh,
            &mut illumination_mesh_query,
            meshes.as_mut(),
            materials.as_mut(),
        );
    }
}

fn despawn_clue_visual_feedback(commands: &mut Commands, clue: &mut ClueCard) {
    if let Some(question_mark) = clue.question_mark.take() {
        commands.entity(question_mark).despawn();
    }
    if let Some(illumination) = clue.illumination.take() {
        commands.entity(illumination).despawn();
    }
}

fn update_clue_illumination_visual(
    commands: &mut Commands,
    config: &GameConfig,
    entity: Entity,
    clue: &mut ClueCard,
    merged_mesh: Option<Mesh>,
    illumination_mesh_query: &mut Query<&mut Mesh2d, With<ClueIllumination>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let Some(mesh) = merged_mesh else {
        if let Some(illumination) = clue.illumination.take() {
            commands.entity(illumination).despawn();
        }
        return;
    };

    let mesh_handle = meshes.add(mesh);
    if let Some(illumination) = clue.illumination
        && let Ok(mut mesh) = illumination_mesh_query.get_mut(illumination)
    {
        *mesh = Mesh2d(mesh_handle);
        return;
    }

    commands.entity(entity).with_children(|parent| {
        clue.illumination = Some(
            parent
                .spawn((
                    Mesh2d(mesh_handle),
                    MeshMaterial2d(materials.add(config.cards.fill_color(CardKind::Interaction))),
                    Transform::from_xyz(0.0, 0.0, 0.36),
                    ClueIllumination,
                ))
                .id(),
        );
    });
}

fn build_visible_clue_stamp(
    config: &GameConfig,
    origin: Vec2,
    transform: &GlobalTransform,
    obstacle_query: &Query<(&Transform, &Obstacle), Without<PlayerCoin>>,
) -> Option<GeoMultiPolygon<f32>> {
    let visible_points = compute_visible_points(config, origin, obstacle_query);
    if visible_points.len() < 3 {
        return None;
    }

    let world_to_local = transform.to_matrix().inverse();
    let mut local_polygon = Vec::with_capacity(visible_points.len() + 1);
    local_polygon.push(
        world_to_local
            .transform_point3(origin.extend(0.0))
            .truncate(),
    );

    for point in visible_points {
        local_polygon.push(
            world_to_local
                .transform_point3(point.extend(0.0))
                .truncate(),
        );
    }

    let visible_polygon = polygon_from_points(local_polygon)?;
    let clipped = visible_polygon.intersection(&Card::card_geo_polygon());
    if clipped.unsigned_area() <= GEOMETRY_EPSILON {
        None
    } else {
        Some(clipped.orient(Direction::Default))
    }
}

fn build_merged_illumination_mesh(clue: &ClueCard) -> (Option<Mesh>, f32) {
    if clue.illuminated_regions.is_empty() {
        return (None, 0.0);
    }

    let merged = unary_union(&clue.illuminated_regions).orient(Direction::Default);
    let area = merged.unsigned_area();
    if area <= GEOMETRY_EPSILON {
        return (None, 0.0);
    }

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for polygon in merged.iter() {
        append_polygon_triangles(polygon, &mut positions, &mut uvs, &mut indices);
    }
    if positions.is_empty() {
        return (None, 0.0);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    (Some(mesh), area)
}

fn append_polygon_triangles(
    polygon: &GeoPolygon<f32>,
    positions: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    for triangle in polygon.earcut_triangles() {
        let base = positions.len() as u32;
        for point in [triangle.v1(), triangle.v2(), triangle.v3()] {
            positions.push([point.x, point.y, 0.0]);
            let uv = (Vec2::new(point.x, point.y) / CARD_SIZE) + Vec2::splat(0.5);
            uvs.push([uv.x, uv.y]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

fn polygon_from_points(points: Vec<Vec2>) -> Option<GeoPolygon<f32>> {
    let mut result = Vec::new();
    for point in points {
        if result
            .last()
            .map(|last: &Vec2| last.distance_squared(point) <= GEOMETRY_EPSILON_SQUARED)
            .unwrap_or(false)
        {
            continue;
        }
        result.push(point);
    }

    if result.len() > 1
        && result[0].distance_squared(*result.last().expect("result is not empty"))
            <= GEOMETRY_EPSILON_SQUARED
    {
        result.pop();
    }

    if result.len() < 3 {
        None
    } else {
        let first = *result.first().expect("polygon has at least three points");
        result.push(first);
        let exterior = GeoLineString::from(
            result
                .into_iter()
                .map(|point| GeoCoord {
                    x: point.x,
                    y: point.y,
                })
                .collect::<Vec<_>>(),
        );
        let polygon = GeoPolygon::new(exterior, vec![]).orient(Direction::Default);
        (polygon.unsigned_area() > GEOMETRY_EPSILON).then_some(polygon)
    }
}

const GEOMETRY_EPSILON: f32 = 0.001;
const GEOMETRY_EPSILON_SQUARED: f32 = GEOMETRY_EPSILON * GEOMETRY_EPSILON;

register_card_specialized_param!("clue", ClueCardParams);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clue_reveal_threshold_parses_named_presets() {
        assert_eq!(
            serde_json::from_value::<ClueRevealThreshold>(serde_json::json!("any"))
                .expect("any threshold should parse"),
            ClueRevealThreshold::Any
        );
        assert_eq!(
            serde_json::from_value::<ClueRevealThreshold>(serde_json::json!("normal"))
                .expect("normal threshold should parse"),
            ClueRevealThreshold::Normal
        );
        assert_eq!(
            serde_json::from_value::<ClueRevealThreshold>(serde_json::json!("full"))
                .expect("full threshold should parse"),
            ClueRevealThreshold::Full
        );

        let threshold =
            serde_json::from_value::<ClueRevealThreshold>(serde_json::json!({ "custom": 0.55 }))
                .expect("custom threshold should parse");
        assert_eq!(threshold, ClueRevealThreshold::Custom(0.55));
        assert_eq!(threshold.get_threshold(), 0.55);
    }

    #[test]
    fn clue_card_params_parse_interaction_spawn_data() {
        let params = serde_json::from_value::<ClueCardParams>(serde_json::json!({
            "reveal_threshold": "normal",
            "interaction_prefab_id": 1005,
            "interaction_target_scene_param": {
                "position": [105.0, -20.0],
                "rotation": -0.12,
                "order": 0.85
            }
        }))
        .expect("clue params should parse interaction spawn data");

        assert_eq!(params.reveal_threshold, ClueRevealThreshold::Normal);
        assert_eq!(params.interaction_prefab_id, 1005);
        assert_eq!(
            params.interaction_target_scene_param.position,
            Vec2::new(105.0, -20.0)
        );
        assert_eq!(params.interaction_target_scene_param.rotation, -0.12);
        assert_eq!(params.interaction_target_scene_param.order, 0.85);
    }
}
