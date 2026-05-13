use crate::card::card_params::{
    CardParam, CardRuntimeSpecializedConfig, CardSceneParam, CardSpawnParams,
    CardSpecializedConfigData, CardSpecializedParam,
};
use crate::card::specialized::obstacle::Obstacle;
use crate::card::{Card, CardKind, CardSpecializedInstaller, spawn_card_by_card_param};
use crate::coin::player::PlayerCoin;
use crate::coin::player::controller::{PlayerCoinState, RefPlayerCoinStateExt};
use crate::config::GameConfig;
use crate::editor::{
    EditorInteractionState, EditorLinkedEntities, EditorPlacedCard, EditorRuntimeSpecializedParam,
    EditorSpecializedAuxiliaryCard, spawn_editor_card,
};
use crate::physics::vision::compute_visible_points;
use crate::progress::GameProgress;
use crate::scene::SceneLayer;
use crate::tools::Disable;
use crate::{AppStatus, GameView, register_card_specialized_installer};
use crate::{GameLoadingSet, GameStatus, GameplaySet, register_card_editor_systems};
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

//region Installer

pub struct ClueCardSpecializedInstaller;

impl CardSpecializedInstaller for ClueCardSpecializedInstaller {
    type Param = ClueCardParams;

    const TYPE_ID: &'static str = "clue";

    fn install(app: &mut App) {
        app.add_systems(
            OnEnter(GameStatus::Loading),
            restore_reveal_clues.in_set(GameLoadingSet::Restore),
        );
        app.add_systems(
            Update,
            reveal_clues.in_set(GameplaySet::SceneModifiedCardLogic),
        );
    }
}

register_card_specialized_installer!(ClueCardSpecializedInstaller);

//endregion

//region Card Specialized Param

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClueCardParams {
    #[serde(default, skip_serializing_if = "ClueRevealThreshold::is_default")]
    pub reveal_threshold: ClueRevealThreshold,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_card_param: Option<CardParam>,
}

impl Default for ClueCardParams {
    fn default() -> Self {
        Self {
            reveal_threshold: Default::default(),
            target_card_param: Some(CardParam {
                scene_param: Default::default(),
                prefab_id: DEFAULT_EDITOR_CLUE_TARGET_PREFAB_ID,
                runtime_specialized_param: None,
            }),
        }
    }
}

impl CardSpecializedParam for ClueCardParams {
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
                            font_size: Card::SIZE.y * 0.42,
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
            param: self.clone(),
            question_mark,
            illumination: None,
            illuminated_regions: Vec::new(),
        });
    }
}

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
    fn is_default(&self) -> bool {
        matches!(self, ClueRevealThreshold::Normal)
    }

    pub fn get_threshold(&self) -> f32 {
        match self {
            ClueRevealThreshold::Any => 0.01,
            ClueRevealThreshold::Normal => 0.6,
            ClueRevealThreshold::Full => 1.0,
            ClueRevealThreshold::Custom(value) => *value,
        }
    }
}

//endregion

//region Component

#[derive(Component, Debug, Clone)]
pub struct ClueCard {
    pub param: ClueCardParams,
    question_mark: Option<Entity>,
    illumination: Option<Entity>,
    illuminated_regions: Vec<GeoMultiPolygon<f32>>,
}

#[derive(Component)]
struct ClueQuestionMark;

#[derive(Component)]
struct ClueIllumination;

//endregion

fn restore_reveal_clues(
    mut commands: Commands,
    progress: ResMut<GameProgress>,
    mut clue_query: Query<(&Card, &mut ClueCard), Without<Disable>>,
    mut card_spawn_params: CardSpawnParams,
) {
    for (card, mut clue) in &mut clue_query {
        let revealed = progress
            .revealed_clue_instances
            .get(&card.instance_id)
            .is_some();
        if !revealed {
            continue;
        }

        despawn_clue_visual_feedback(&mut commands, &mut clue);
        spawn_revealed_card(&mut commands, &mut card_spawn_params, &clue);
        continue;
    }
}

fn reveal_clues(
    mut commands: Commands,
    player_query: Query<(Ref<PlayerCoinState>, &Transform), With<PlayerCoin>>,
    mut clue_query: Query<(Entity, &Card, &mut ClueCard, &GlobalTransform), Without<Disable>>,
    obstacle_query: Query<(&Transform, &Obstacle), (Without<PlayerCoin>, Without<Disable>)>,
    mut illumination_mesh_query: Query<&mut Mesh2d, With<ClueIllumination>>,
    mut progress: ResMut<GameProgress>,
    mut card_spawn_params: CardSpawnParams,
) {
    for (player_state, player_transform) in player_query.iter() {
        if !player_state.just_eject_finished_or_initialized() {
            continue;
        }

        for (entity, card, mut clue, global_transform) in &mut clue_query {
            let revealed = progress
                .revealed_clue_instances
                .get(&card.instance_id)
                .is_some();
            if revealed {
                continue;
            }

            let Some(stamp) = build_visible_clue_stamp(
                &card_spawn_params.config,
                player_transform.translation.truncate(),
                global_transform,
                &obstacle_query,
            ) else {
                continue;
            };
            clue.illuminated_regions.push(stamp);

            let (merged_mesh, illuminated_area) = build_merged_illumination_mesh(&clue);
            let coverage = illuminated_area / Card::card_area();

            if coverage >= clue.param.reveal_threshold.get_threshold() {
                progress
                    .revealed_clue_instances
                    .insert(card.instance_id.clone());
                despawn_clue_visual_feedback(&mut commands, &mut clue);
                spawn_revealed_card(&mut commands, &mut card_spawn_params, &clue);
                continue;
            }

            update_clue_illumination_visual(
                &mut commands,
                &card_spawn_params.config,
                entity,
                &mut clue,
                merged_mesh,
                &mut illumination_mesh_query,
                card_spawn_params.meshes.as_mut(),
                card_spawn_params.materials.as_mut(),
            );
        }
    }
}

fn spawn_revealed_card(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    clue: &ClueCard,
) {
    let Some(target_card_param) = clue.param.target_card_param.as_ref() else {
        return;
    };

    let entity = spawn_card_by_card_param(commands, spawn_params, target_card_param);
    commands.entity(entity).insert(GameView);
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
    obstacle_query: &Query<(&Transform, &Obstacle), (Without<PlayerCoin>, Without<Disable>)>,
) -> Option<GeoMultiPolygon<f32>> {
    let visible_points = compute_visible_points(config, origin, obstacle_query);
    if visible_points.len() < 3 {
        return None;
    }

    let world_to_local = transform.to_matrix().inverse();
    let mut local_polygon = Vec::with_capacity(visible_points.len());
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
            let uv = (Vec2::new(point.x, point.y) / Card::SIZE) + Vec2::splat(0.5);
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
const DEFAULT_EDITOR_CLUE_TARGET_PREFAB_ID: u32 = 1904;
const DEFAULT_EDITOR_CLUE_TARGET_OFFSET: Vec2 = Vec2::new(0.0, -Card::SIZE.y * 0.4);
const EDITOR_CLUE_TARGET_ORDER_OFFSET: f32 = 1.0;
const CLUE_LINK_DASH_LENGTH: f32 = 18.0;
const CLUE_LINK_GAP_LENGTH: f32 = 10.0;

//region Editor

#[derive(Component, Clone, Copy)]
struct EditorClueLink {
    target: Entity,
}

#[derive(Component, Clone, Copy)]
struct EditorClueTargetCard;

fn register_editor_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_editor_clue_targets,
            update_editor_runtime_params,
            draw_editor_clue_links,
        )
            .run_if(in_state(AppStatus::Editor)),
    );
}

fn spawn_editor_clue_targets(
    mut commands: Commands,
    mut spawn_params: CardSpawnParams<'_>,
    mut editor_state: ResMut<EditorInteractionState>,
    clue_query: Query<(Entity, &ClueCard, &Transform), Added<EditorPlacedCard>>,
) {
    for (clue_entity, clue, clue_transform) in &clue_query {
        let target_card_param = editor_clue_target_spawn_param(clue, clue_transform);
        let target_entity = spawn_editor_card(&mut commands, &mut spawn_params, &target_card_param);

        commands.entity(clue_entity).insert((
            EditorClueLink {
                target: target_entity,
            },
            EditorLinkedEntities {
                entities: vec![target_entity],
            },
        ));
        commands.entity(target_entity).insert((
            EditorClueTargetCard,
            EditorSpecializedAuxiliaryCard,
            EditorLinkedEntities {
                entities: vec![clue_entity],
            },
        ));
        editor_state.select_entity(target_entity);
        editor_state.set_status_message(format!(
            "已创建线索目标交互卡 #{}",
            target_card_param.prefab_id
        ));
    }
}

fn editor_clue_target_spawn_param(clue: &ClueCard, clue_transform: &Transform) -> CardParam {
    match clue.param.target_card_param.as_ref() {
        None => CardParam {
            scene_param: CardSceneParam {
                instance_id: String::new(),
                position: clue_transform.translation.truncate() + DEFAULT_EDITOR_CLUE_TARGET_OFFSET,
                rotation: 0.0,
                order: clue_transform.translation.z - SceneLayer::Card.get_layer_base_z()
                    + EDITOR_CLUE_TARGET_ORDER_OFFSET,
                enable_if: None,
                disable_if: None,
                description: String::new(),
            },
            prefab_id: DEFAULT_EDITOR_CLUE_TARGET_PREFAB_ID,
            runtime_specialized_param: None,
        },
        Some(card_param) => card_param.clone(),
    }
}

fn update_editor_runtime_params(
    mut commands: Commands,
    clue_query: Query<(Entity, &EditorClueLink), With<ClueCard>>,
    target_query: Query<
        (&Card, &Transform, Option<&EditorRuntimeSpecializedParam>),
        With<EditorClueTargetCard>,
    >,
) {
    for (clue_entity, link) in &clue_query {
        let Ok((target_card, target_transform, runtime)) = target_query.get(link.target) else {
            continue;
        };
        let params = match serde_json::to_value(ClueCardParams {
            reveal_threshold: Default::default(),
            target_card_param: Some(target_card.to_card_param(target_transform, runtime)),
        }) {
            Ok(params) => params,
            Err(error) => {
                warn!("failed to serialize clue runtime params: {error}");
                continue;
            }
        };
        if let Ok(mut entity) = commands.get_entity(clue_entity) {
            entity.try_insert(EditorRuntimeSpecializedParam(
                CardRuntimeSpecializedConfig {
                    data: CardSpecializedConfigData {
                        type_id: "clue".to_string(),
                        params,
                    },
                },
            ));
        }
    }
}

fn draw_editor_clue_links(
    mut gizmos: Gizmos,
    clue_query: Query<(&Transform, &EditorClueLink), With<ClueCard>>,
    target_query: Query<&Transform, With<EditorClueTargetCard>>,
) {
    for (clue_transform, link) in &clue_query {
        let Ok(target_transform) = target_query.get(link.target) else {
            continue;
        };
        draw_dashed_line(
            &mut gizmos,
            clue_transform.translation.truncate(),
            target_transform.translation.truncate(),
            Color::srgb(0.95, 0.82, 0.35),
        );
    }
}

fn draw_dashed_line(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color) {
    let delta = end - start;
    let distance = delta.length();
    if distance <= f32::EPSILON {
        return;
    }

    let direction = delta / distance;
    let step = CLUE_LINK_DASH_LENGTH + CLUE_LINK_GAP_LENGTH;
    let mut cursor = 0.0;
    while cursor < distance {
        let dash_end = (cursor + CLUE_LINK_DASH_LENGTH).min(distance);
        gizmos.line_2d(
            start + direction * cursor,
            start + direction * dash_end,
            color,
        );
        cursor += step;
    }
}

register_card_editor_systems!("clue", register_editor_systems);
//endregion

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
    fn clue_card_params_parse_runtime_interaction_target() {
        let params = serde_json::from_value::<ClueCardParams>(serde_json::json!({
            "reveal_threshold": "normal",
            "target_card_param": {
                "scene_param": {
                    "position": [105.0, -20.0],
                    "rotation": -0.12,
                    "order": 0.85
                },
                "prefab_id": 1005
            }
        }))
        .expect("clue params should parse runtime interaction target");

        assert_eq!(params.reveal_threshold, ClueRevealThreshold::Normal);
        let target_card_param = params
            .target_card_param
            .expect("target card param should parse");
        assert_eq!(target_card_param.prefab_id, 1005);
        assert_eq!(
            target_card_param.scene_param.position,
            Vec2::new(105.0, -20.0)
        );
        assert_eq!(target_card_param.scene_param.rotation, -0.12);
        assert_eq!(target_card_param.scene_param.order, 0.85);
    }
}
