pub mod card_params;
pub mod specialized;

use crate::card::card_params::{CardAppearanceConfig, CardParam, SpawnCardSystemParams};
use crate::card::card_params::{CardSceneParam, CardSpecializedParam};
use crate::config::{CardConfig, GameConfig};
use crate::editor::EditorRuntimeSpecializedParam;
use crate::progress::GameProgress;
use crate::scene::{
    SceneLayer, SceneParam, Z_OFFSET_CARD_BACKGROUND, Z_OFFSET_CARD_IMAGE, Z_OFFSET_CARD_TITLE,
};
use crate::tools::Disable;
use crate::{AppStatus, GameplaySet};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{Text2dUpdateSystems, TextLayoutInfo};
use geo::orient::Direction;
use geo::{Coord as GeoCoord, LineString as GeoLineString, Orient, Polygon as GeoPolygon};
use serde::Deserialize;
use std::collections::HashMap;

pub struct CardPlugin;

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardSpecializedRegistry>();
        app.add_systems(
            Update,
            sync_card_disable_state
                .in_set(GameplaySet::SceneModifiedCardLogic)
                .run_if(in_state(AppStatus::Game)),
        );
        app.add_systems(
            PostUpdate,
            fit_card_title_text_width.after(Text2dUpdateSystems),
        );

        CardSpecializedRegistry::default()
            .registrations
            .values()
            .for_each(|registration| (registration.installer)(app));
    }
}

const TITLE_TEXT_SCALE_EPSILON: f32 = 0.001;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Scenery,
    Obstacle,
    Interaction,
    Clue,
    Trap,
}

#[derive(Component, Clone, Debug)]
pub struct Card {
    pub instance_id: String,
    pub title: String,
    pub according_type: CardAccordingType,
    pub friction: Option<f32>,
    pub landing_speed_loss: Option<f32>,
    pub enable_if: Option<String>,
    pub disable_if: Option<String>,
}

impl Card {
    const STANDARD_SIZE: Vec2 = Vec2::new(53.9, 85.6);
    const IN_GAME_SIZE_SCALE: f32 = 2.0;
    pub const SIZE: Vec2 = Vec2::new(
        Self::STANDARD_SIZE.x * Self::IN_GAME_SIZE_SCALE,
        Self::STANDARD_SIZE.y * Self::IN_GAME_SIZE_SCALE,
    );
    pub const HALF_SIZE: Vec2 = Vec2::new(Self::SIZE.x * 0.5, Self::SIZE.y * 0.5);

    pub const fn card_area() -> f32 {
        Self::SIZE.x * Self::SIZE.y
    }

    pub fn card_mesh() -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        let Vec2 {
            x: half_x,
            y: half_y,
        } = Self::HALF_SIZE;

        let positions = vec![
            [-half_x, half_y, 0.0],
            [half_x, half_y, 0.0],
            [half_x, -half_y, 0.0],
            [-half_x, -half_y, 0.0],
        ];
        let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let indices = vec![0, 1, 2, 0, 2, 3];

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }

    pub fn card_rounded_mesh(config: &CardConfig) -> Mesh {
        rounded_rectangle_mesh(
            Self::SIZE,
            config.corner_radius,
            config.rounded_corner_segments,
        )
    }

    pub fn card_cut_polygon(config: &CardConfig) -> Vec<Vec2> {
        let radius = config.corner_radius;
        let Vec2 {
            x: half_x,
            y: half_y,
        } = Self::HALF_SIZE;
        vec![
            Vec2::new(-half_x + radius, half_y),
            Vec2::new(half_x - radius, half_y),
            Vec2::new(half_x, half_y - radius),
            Vec2::new(half_x, -half_y + radius),
            Vec2::new(half_x - radius, -half_y),
            Vec2::new(-half_x + radius, -half_y),
            Vec2::new(-half_x, -half_y + radius),
            Vec2::new(-half_x, half_y - radius),
        ]
    }

    pub fn card_geo_polygon() -> GeoPolygon<f32> {
        let half = Self::SIZE * 0.5;
        GeoPolygon::new(
            GeoLineString::from(vec![
                GeoCoord {
                    x: -half.x,
                    y: -half.y,
                },
                GeoCoord {
                    x: half.x,
                    y: -half.y,
                },
                GeoCoord {
                    x: half.x,
                    y: half.y,
                },
                GeoCoord {
                    x: -half.x,
                    y: half.y,
                },
                GeoCoord {
                    x: -half.x,
                    y: -half.y,
                },
            ]),
            vec![],
        )
        .orient(Direction::Default)
    }

    pub fn intersect_circle(&self, transform: &GlobalTransform, point: Vec2, radius: f32) -> bool {
        let local_point = transform
            .to_matrix()
            .inverse()
            .transform_point3(point.extend(0.0))
            .truncate();

        let half_size = Self::SIZE * 0.5;

        let closest_x = local_point.x.clamp(-half_size.x, half_size.x);
        let closest_y = local_point.y.clamp(-half_size.y, half_size.y);

        let distance_x = local_point.x - closest_x;
        let distance_y = local_point.y - closest_y;

        (distance_x * distance_x + distance_y * distance_y) <= (radius * radius)
    }

    pub fn contains_point(&self, transform: &GlobalTransform, point: Vec2) -> bool {
        self.intersect_circle(transform, point, 0.0)
    }

    pub fn to_card_param(
        &self,
        transform: &Transform,
        runtime: Option<&EditorRuntimeSpecializedParam>,
    ) -> Option<CardParam> {
        self.according_type
            .get_prefab_id()
            .map(|prefab_id| CardParam {
                scene_param: CardSceneParam {
                    instance_id: self.instance_id.clone(),
                    data: SceneParam {
                        position: transform.translation.truncate(),
                        rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
                        order: transform.translation.z
                            - SceneLayer::SceneObjects.get_layer_base_z(),
                        enable_if: self.enable_if.clone(),
                        disable_if: self.disable_if.clone(),
                        description: String::new(),
                    },
                },
                prefab_id,
                runtime_specialized_param: runtime.cloned().map(|runtime| runtime.0),
            })
    }
}

#[derive(Clone, Debug)]
pub enum CardAccordingType {
    Prefab(u32),
    GeneratedScenery { appearance_id: u32 },
}

impl CardAccordingType {
    pub fn get_prefab_id(&self) -> Option<u32> {
        match self {
            CardAccordingType::Prefab(id) => Some(*id),
            CardAccordingType::GeneratedScenery { .. } => None,
        }
    }
}

//region Card Specialized Installer

pub type CardSpecializedParamParser =
    fn(&serde_json::Value) -> anyhow::Result<Box<dyn CardSpecializedParam>>;

pub(super) trait CardSpecializedInstaller {
    type Param: CardSpecializedParam + serde::de::DeserializeOwned + 'static;

    const TYPE_ID: &'static str;

    fn parse_json_param(
        value: &serde_json::Value,
    ) -> anyhow::Result<Box<dyn CardSpecializedParam>> {
        let params = serde_json::from_value::<Self::Param>(value.clone())?;
        Ok(Box::new(params))
    }

    fn install(_app: &mut App) {}
}

#[derive(Debug)]
pub struct CardSpecializedRegistration {
    pub type_id: &'static str,
    pub json_param_parser: CardSpecializedParamParser,
    pub installer: fn(&mut App),
}

#[derive(Resource, Clone, Debug)]
pub struct CardSpecializedRegistry {
    registrations: HashMap<&'static str, &'static CardSpecializedRegistration>,
}

impl CardSpecializedRegistry {
    pub fn get(&self, type_id: impl AsRef<str>) -> Option<&&'static CardSpecializedRegistration> {
        self.registrations.get(type_id.as_ref())
    }
}

impl Default for CardSpecializedRegistry {
    fn default() -> Self {
        Self {
            registrations: inventory::iter::<CardSpecializedRegistration>
                .into_iter()
                .map(|registration| (registration.type_id, registration))
                .collect(),
        }
    }
}

inventory::collect!(CardSpecializedRegistration);

#[macro_export]
macro_rules! register_card_specialized_installer {
    ($installer:ty) => {
        inventory::submit! {
            $crate::card::CardSpecializedRegistration {
                type_id: <$installer as $crate::card::CardSpecializedInstaller>::TYPE_ID,
                json_param_parser: <$installer as $crate::card::CardSpecializedInstaller>::parse_json_param,
                installer: <$installer as $crate::card::CardSpecializedInstaller>::install,
            }
        }
    };
}
//endregion

#[derive(Component, Clone, Copy)]
struct FitCardTitleText {
    max_width: f32,
}

pub fn spawn_card_by_card_param(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let appearance = card_param.load_appearance(&spawn_params.card_presets_config);
    let (instance_id, instance_id_from_param) = card_param.resolved_instance_id(&appearance.title);
    if !instance_id_from_param {
        warn!(
            "Card instance_id is not provided for card '{}' from scene param. Consider re-exporting the scene.",
            appearance.title
        );
    }
    let specialized = card_param.load_specialized_config(
        &spawn_params.card_presets_config,
        &spawn_params.card_specialized_registry,
    );
    let card_kind = specialized
        .as_ref()
        .map(|specialized| specialized.kind())
        .unwrap_or(CardKind::Scenery);
    let fill_color = resolve_appearance_fill_color(
        &appearance,
        specialized
            .as_ref()
            .map(|specialized| {
                spawn_params
                    .config
                    .cards
                    .card_fill_color(specialized.kind())
            })
            .unwrap_or_else(|| spawn_params.config.cards.default_fill_color()),
    );

    spawn_card_inner(
        commands,
        spawn_params,
        CardSpawnParams {
            appearance: &appearance,
            scene_param: card_param.scene_param.clone(),
            instance_id,
            according_type: CardAccordingType::Prefab(card_param.prefab_id),
            card_kind,
            fill_color,
            specialized: specialized.as_deref(),
        },
    )
}

pub fn spawn_scenery_by_appearance(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    appearance: &CardAppearanceConfig,
    scene_param: CardSceneParam,
) -> Entity {
    let instance_id = if scene_param.instance_id.is_empty() {
        card_params::make_card_instance_id(appearance.id, &appearance.title)
    } else {
        scene_param.instance_id.clone()
    };
    let card_kind = CardKind::Scenery;
    let fill_color = resolve_appearance_fill_color(
        appearance,
        spawn_params.config.cards.card_fill_color(card_kind),
    );

    spawn_card_inner(
        commands,
        spawn_params,
        CardSpawnParams {
            appearance,
            scene_param,
            instance_id,
            according_type: CardAccordingType::GeneratedScenery {
                appearance_id: appearance.id,
            },
            card_kind,
            fill_color,
            specialized: None,
        },
    )
}

struct CardSpawnParams<'a> {
    appearance: &'a CardAppearanceConfig,
    scene_param: CardSceneParam,
    instance_id: String,
    according_type: CardAccordingType,
    card_kind: CardKind,
    fill_color: Color,
    specialized: Option<&'a dyn CardSpecializedParam>,
}

fn spawn_card_inner(
    commands: &mut Commands,
    spawn_params: &mut SpawnCardSystemParams<'_>,
    spawn_config: CardSpawnParams<'_>,
) -> Entity {
    let CardSpawnParams {
        appearance,
        scene_param,
        instance_id,
        according_type,
        card_kind,
        fill_color,
        specialized,
    } = spawn_config;

    let should_disable_initially = scene_param.data.enable_if.is_some();
    let z_order = SceneLayer::SceneObjects.get_layer_base_z() + scene_param.data.order;

    let mut entity = commands.spawn((
        Transform::from_translation(scene_param.data.position.extend(z_order))
            .with_rotation(Quat::from_rotation_z(scene_param.data.rotation)),
        Card {
            instance_id,
            title: appearance.title.clone(),
            according_type,
            friction: appearance.friction,
            landing_speed_loss: appearance.landing_speed_loss,
            enable_if: scene_param.data.enable_if,
            disable_if: scene_param.data.disable_if,
        },
        card_kind,
        Pickable::default(),
        Mesh2d(
            spawn_params
                .meshes
                .add(Card::card_rounded_mesh(&spawn_params.config.cards)),
        ),
        MeshMaterial2d(spawn_params.materials.add(fill_color)),
    ));

    if let Some(specialized) = specialized {
        specialized.spawn_with(&mut entity, spawn_params);
    }

    if should_disable_initially {
        entity.insert((Disable, Visibility::Hidden));
    }

    entity.with_children(|parent| {
        spawn_card_image(
            parent,
            spawn_params.asset_server.as_ref(),
            spawn_params.meshes.as_mut(),
            spawn_params.materials.as_mut(),
            &appearance.image_res_path,
            &spawn_params.config.cards.background_card_image_path,
        );
        spawn_card_title(
            parent,
            spawn_params.asset_server.as_ref(),
            &spawn_params.config,
            &appearance.title,
        );
    });

    entity.id()
}

fn resolve_appearance_fill_color(appearance: &CardAppearanceConfig, fallback: Color) -> Color {
    if appearance.background_color_appearance_override.is_empty() {
        return fallback;
    }

    Srgba::hex(&appearance.background_color_appearance_override)
        .map(Color::Srgba)
        .unwrap_or(fallback)
}

fn sync_card_disable_state(
    mut commands: Commands,
    progress: Res<GameProgress>,
    card_query: Query<(Entity, &Card, Option<&Disable>, Option<&Visibility>)>,
) {
    for (entity, card, disable, visibility) in &card_query {
        let should_disable = card
            .enable_if
            .as_deref()
            .map(|key| !progress.is_unlocked(key))
            .unwrap_or(false)
            || card
                .disable_if
                .as_deref()
                .map(|key| progress.is_unlocked(key))
                .unwrap_or(false);

        if should_disable {
            let mut entity_commands = commands.entity(entity);
            if disable.is_none() {
                entity_commands.insert(Disable);
            }
            if !matches!(visibility, Some(&Visibility::Hidden)) {
                entity_commands.insert(Visibility::Hidden);
            }
        } else {
            let mut entity_commands = commands.entity(entity);
            if disable.is_some() {
                entity_commands.remove::<Disable>();
            }
            if !matches!(visibility, Some(&Visibility::Visible)) {
                entity_commands.insert(Visibility::Visible);
            }
        }
    }
}

fn spawn_card_image(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    image_res_path: &str,
    background_res_path: &str,
) {
    spawn_card_image_inner(
        parent,
        asset_server,
        meshes,
        materials,
        image_res_path,
        Z_OFFSET_CARD_IMAGE,
    );
    spawn_card_image_inner(
        parent,
        asset_server,
        meshes,
        materials,
        background_res_path,
        Z_OFFSET_CARD_BACKGROUND,
    );
}

fn spawn_card_image_inner(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    image_res_path: &str,
    z: f32,
) {
    if let Some(image_path) = normalize_asset_path(image_res_path) {
        parent.spawn((
            Mesh2d(meshes.add(Card::card_mesh())),
            MeshMaterial2d(materials.add(ColorMaterial::from(asset_server.load(image_path)))),
            Transform::from_xyz(0.0, 0.0, z),
        ));
    };
}

fn spawn_card_title(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    config: &GameConfig,
    title: &str,
) {
    let title_position_y = (0.5 - config.cards.title_offset_y_ratio) * Card::SIZE.y;
    let max_title_width = Card::SIZE.x * config.cards.title_text_width_ratio;

    parent.spawn((
        Text2d::new(title.to_string()),
        TextFont {
            font: asset_server.load(config.assets.default_font.clone()),
            font_size: config.cards.title_font_size,
            ..default()
        },
        TextColor(Color::BLACK),
        TextLayout::new_with_justify(Justify::Center),
        Anchor::CENTER,
        Transform::from_xyz(0.0, title_position_y, Z_OFFSET_CARD_TITLE),
        FitCardTitleText {
            max_width: max_title_width,
        },
    ));
}

fn fit_card_title_text_width(
    mut text_query: Query<(&FitCardTitleText, &TextLayoutInfo, &mut Transform)>,
) {
    for (fit, layout, mut transform) in &mut text_query {
        if fit.max_width <= f32::EPSILON || layout.size.x <= f32::EPSILON {
            continue;
        }

        let next_scale_x = (fit.max_width / layout.size.x).min(1.0);
        if (transform.scale.x - next_scale_x).abs() <= TITLE_TEXT_SCALE_EPSILON {
            continue;
        }

        transform.scale.x = next_scale_x;
    }
}

fn rounded_rectangle_mesh(size: Vec2, radius: f32, corner_segments: usize) -> Mesh {
    let half_size = size * 0.5;
    let radius = radius
        .max(0.0)
        .min(half_size.x.abs())
        .min(half_size.y.abs());
    let corner_segments = corner_segments.max(1);
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut uvs = vec![[0.5, 0.5]];
    let corners = [
        (
            Vec2::new(half_size.x - radius, half_size.y - radius),
            0.0,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            Vec2::new(-half_size.x + radius, half_size.y - radius),
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ),
        (
            Vec2::new(-half_size.x + radius, -half_size.y + radius),
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ),
        (
            Vec2::new(half_size.x - radius, -half_size.y + radius),
            std::f32::consts::PI * 1.5,
            std::f32::consts::TAU,
        ),
    ];

    for (center, start_angle, end_angle) in corners {
        for step in 0..=corner_segments {
            let t = step as f32 / corner_segments as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let point = center + Vec2::new(angle.cos(), angle.sin()) * radius;
            positions.push([point.x, point.y, 0.0]);
            uvs.push([(point.x / size.x) + 0.5, 1.0 - ((point.y / size.y) + 0.5)]);
        }
    }

    let point_count = positions.len() as u32;
    let mut indices = Vec::with_capacity((point_count as usize - 1) * 3);
    for index in 1..point_count {
        let next = if index + 1 == point_count {
            1
        } else {
            index + 1
        };
        indices.extend_from_slice(&[0, index, next]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn normalize_asset_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(
        trimmed
            .trim_start_matches('/')
            .trim_start_matches("assets/")
            .replace('\\', "/"),
    )
}
