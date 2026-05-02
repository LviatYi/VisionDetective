pub mod card_params;
pub mod specialized;

use crate::card::card_params::CardImageLayoutType;
use crate::card::card_params::{CardParam, CardSpecializedRegistry};
use crate::config::GameConfig;
use crate::config::card_config::CardPresetsConfig;
use bevy::prelude::*;
use serde::Deserialize;

pub struct CardPlugin;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Scenery,
    Obstacle,
    Interaction,
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
}

impl Card {
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
}

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardSpecializedRegistry>();
        app.add_plugins(specialized::interactive::InteractionCardPlugin);
    }
}

pub fn spawn_card_by_card_param(
    commands: &mut Commands,
    asset_server: &AssetServer,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    card_param: &CardParam,
    card_presets_config: &CardPresetsConfig,
    card_specialized_registry: &CardSpecializedRegistry,
) -> Entity {
    let appearance = card_param.load_appearance(card_presets_config);
    let specialized =
        card_param.load_specialized_config(card_presets_config, card_specialized_registry);
    let card_kind = specialized
        .as_ref()
        .map(|specialized| specialized.kind())
        .unwrap_or(CardKind::Scenery);
    let fill_color = parse_hex_color(&appearance.background_color_appearance_override)
        .unwrap_or_else(|| {
            if specialized.is_some() {
                config.cards.fill_color(card_kind)
            } else {
                config.cards.default_fill_color()
            }
        });

    let mut entity = commands.spawn((
        Transform::from_translation(card_param.scene_param.position.extend(0.0))
            .with_rotation(Quat::from_rotation_z(card_param.scene_param.rotation)),
        Card {
            title: appearance.title.clone(),
        },
        card_kind,
        Mesh2d(meshes.add(Rectangle::new(CARD_SIZE.x, CARD_SIZE.y))),
        MeshMaterial2d(materials.add(fill_color)),
    ));

    if let Some(specialized) = specialized {
        specialized.insert_components(&mut entity);
    }

    entity.with_children(|parent| {
        spawn_card_image(
            parent,
            asset_server,
            config,
            &appearance.image_layout_type,
            &appearance.image_res_path,
        );
        spawn_card_title(parent, asset_server, config, &appearance.title);
    });

    entity.id()
}

fn spawn_card_image(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    config: &GameConfig,
    image_layout_type: &CardImageLayoutType,
    image_res_path: &str,
) {
    let Some(image_path) = normalize_asset_path(image_res_path) else {
        return;
    };

    let mut sprite = Sprite::from_image(asset_server.load(image_path));
    sprite.custom_size = Some(match image_layout_type {
        CardImageLayoutType::Full => CARD_SIZE,
        CardImageLayoutType::Normal => CARD_SIZE * config.cards.normal_image_size_ratio(),
    });

    parent.spawn((
        sprite,
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
    title: &str,
) {
    parent.spawn((
        Text2d::new(title.to_string()),
        TextFont {
            font: asset_server.load(config.assets.default_font.clone()),
            font_size: config.cards.title_font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(0.0, CARD_SIZE.y * config.cards.title_offset_y_ratio, 0.2),
    ));
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
