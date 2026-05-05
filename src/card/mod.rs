pub mod card_params;
pub mod specialized;

use crate::card::card_params::{CardImageLayoutType, CardSceneParam};
use crate::card::card_params::{CardParam, CardSpawnParams, CardSpecializedRegistry};
use crate::config::GameConfig;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use geo::orient::Direction;
use geo::{
    BooleanOps, Coord as GeoCoord, LineString as GeoLineString, Orient, Polygon as GeoPolygon,
    TriangulateEarcut,
};
use serde::Deserialize;

pub struct CardPlugin;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Scenery,
    Obstacle,
    Interaction,
    Clue,
}

pub const STANDARD_CARD_SIZE: Vec2 = Vec2::new(53.9, 85.6);
pub const IN_GAME_CARD_SIZE_SCALE: f32 = 2.0;
pub const CARD_SIZE: Vec2 = Vec2::new(
    STANDARD_CARD_SIZE.x * IN_GAME_CARD_SIZE_SCALE,
    STANDARD_CARD_SIZE.y * IN_GAME_CARD_SIZE_SCALE,
);

#[derive(Component, Clone, Debug)]
pub struct Card {
    pub title: String,
    pub instance_type: CardInstanceType,
}

#[derive(Clone, Debug)]
pub enum CardInstanceType {
    Prefab(u32),
}

impl CardInstanceType {
    pub fn get_prefab_id(&self) -> u32 {
        match self {
            CardInstanceType::Prefab(id) => *id,
        }
    }
}

impl Card {
    pub const fn card_area() -> f32 {
        CARD_SIZE.x * CARD_SIZE.y
    }

    pub fn card_geo_polygon() -> GeoPolygon<f32> {
        let half = CARD_SIZE * 0.5;
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

        let half_size = CARD_SIZE * 0.5;

        let closest_x = local_point.x.clamp(-half_size.x, half_size.x);
        let closest_y = local_point.y.clamp(-half_size.y, half_size.y);

        let distance_x = local_point.x - closest_x;
        let distance_y = local_point.y - closest_y;

        (distance_x * distance_x + distance_y * distance_y) <= (radius * radius)
    }

    pub fn contains_point(&self, transform: &GlobalTransform, point: Vec2) -> bool {
        self.intersect_circle(transform, point, 0.0)
    }

    pub fn to_card_param(&self, transform: &Transform) -> CardParam {
        CardParam {
            scene_param: CardSceneParam {
                position: transform.translation.truncate(),
                rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
                order: transform.translation.z,
            },
            prefab_id: self.instance_type.get_prefab_id(),
        }
    }
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardSpecializedRegistry>();
        app.add_plugins((
            specialized::interactive::InteractionCardPlugin,
            specialized::clue::ClueCardPlugin,
        ));
    }
}

pub fn spawn_card_by_card_param(
    commands: &mut Commands,
    spawn_params: &mut CardSpawnParams<'_>,
    card_param: &CardParam,
) -> Entity {
    let appearance = card_param.load_appearance(&spawn_params.card_presets_config);
    let specialized = card_param.load_specialized_config(
        &spawn_params.card_presets_config,
        &spawn_params.card_specialized_registry,
    );
    let card_kind = specialized
        .as_ref()
        .map(|specialized| specialized.kind())
        .unwrap_or(CardKind::Scenery);
    let fill_color = parse_hex_color(&appearance.background_color_appearance_override)
        .unwrap_or_else(|| {
            if specialized.is_some() {
                spawn_params.config.cards.fill_color(card_kind)
            } else {
                spawn_params.config.cards.default_fill_color()
            }
        });

    let mut entity = commands.spawn((
        Transform::from_translation(
            card_param
                .scene_param
                .position
                .extend(card_param.scene_param.order),
        )
        .with_rotation(Quat::from_rotation_z(card_param.scene_param.rotation)),
        Card {
            title: appearance.title.clone(),
            instance_type: CardInstanceType::Prefab(card_param.prefab_id),
        },
        card_kind,
        Pickable::default(),
        Mesh2d(spawn_params.meshes.add(rounded_rectangle_mesh(
            CARD_SIZE,
            spawn_params.config.cards.corner_radius,
            spawn_params.config.cards.rounded_corner_segments,
        ))),
        MeshMaterial2d(spawn_params.materials.add(fill_color)),
    ));

    if let Some(specialized) = specialized {
        specialized.spawn_with(&mut entity, spawn_params);
    }

    entity.with_children(|parent| {
        spawn_card_image(
            parent,
            spawn_params.asset_server.as_ref(),
            &spawn_params.config,
            spawn_params.meshes.as_mut(),
            spawn_params.materials.as_mut(),
            &appearance.image_layout_type,
            &appearance.image_res_path,
        );
        spawn_card_title(
            parent,
            spawn_params.asset_server.as_ref(),
            &spawn_params.config,
            spawn_params.meshes.as_mut(),
            spawn_params.materials.as_mut(),
            &appearance.title,
        );
    });

    entity.id()
}

fn spawn_card_image(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    image_layout_type: &CardImageLayoutType,
    image_res_path: &str,
) {
    let Some(image_path) = normalize_asset_path(image_res_path) else {
        return;
    };

    let image_size = match image_layout_type {
        CardImageLayoutType::Full => CARD_SIZE,
        CardImageLayoutType::Normal => CARD_SIZE * config.cards.normal_image_size_ratio(),
    };

    parent.spawn((
        Mesh2d(meshes.add(rounded_rectangle_mesh(
            image_size,
            config.cards.corner_radius,
            config.cards.rounded_corner_segments,
        ))),
        MeshMaterial2d(materials.add(ColorMaterial::from(asset_server.load(image_path)))),
        Transform::from_xyz(
            0.0,
            match image_layout_type {
                CardImageLayoutType::Full => 0.0,
                CardImageLayoutType::Normal => config.cards.normal_image_offset_y,
            },
            0.1,
        ),
    ));
}

fn spawn_card_title(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    title: &str,
) {
    let title_position = CARD_SIZE.y * config.cards.title_offset_y_ratio;
    let title_size = title_glass_size(config, title_position);

    parent.spawn((
        Mesh2d(meshes.add(rounded_rectangle_mesh(
            title_size,
            config.cards.title_glass_corner_radius,
            config.cards.rounded_corner_segments,
        ))),
        MeshMaterial2d(materials.add(config.cards.title_glass_color())),
        Transform::from_xyz(0.0, title_position, 0.2),
    ));

    parent.spawn((
        Text2d::new(title.to_string()),
        TextFont {
            font: asset_server.load(config.assets.default_font.clone()),
            font_size: config.cards.title_font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, title_position, 0.22).with_scale(Vec3::new(
            title_text_scale_x(title, config),
            1.0,
            1.0,
        )),
    ));
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

fn title_glass_size(config: &GameConfig, title_position: f32) -> Vec2 {
    let padding = config.cards.title_glass_padding();
    let height = config.cards.title_font_size * 1.25 + padding.y * 2.0;

    let max_height = (CARD_SIZE.y * 0.5 - title_position.abs()).max(0.0) * 2.0;

    Vec2::new(CARD_SIZE.x, height.min(max_height))
}

fn title_text_scale_x(title: &str, config: &GameConfig) -> f32 {
    let max_width = (CARD_SIZE.x - config.cards.title_glass_padding().x * 2.0).max(1.0);
    let text_width = estimated_title_width(title, config).max(1.0);

    (max_width / text_width).min(1.0)
}

fn estimated_title_width(title: &str, config: &GameConfig) -> f32 {
    title
        .chars()
        .map(|character| {
            if character.is_ascii() {
                config.cards.title_font_size * 0.55
            } else {
                config.cards.title_font_size
            }
        })
        .sum::<f32>()
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

fn parse_hex_color(input: &str) -> Option<Color> {
    let input = input.trim().trim_start_matches('#');
    match input.is_empty() {
        true => None,
        false => Srgba::hex(input).ok().map(|c| Color::Srgba(c)),
    }
}
