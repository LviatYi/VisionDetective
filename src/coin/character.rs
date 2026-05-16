use crate::card::normalize_asset_path;
use crate::config::GameConfig;
use crate::config::character_config::{CharacterConfig, CharacterDefinition};
use crate::scene::{SceneLayer, SceneParam};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use serde::{Deserialize, Serialize};

pub const COIN_BACKGROUND_IMAGE_PATH: &str = "pic/coin-background.png";
pub const COIN_BACKGROUND_Z_OFFSET: f32 = 0.0005;
pub const COIN_PORTRAIT_Z_OFFSET: f32 = 0.001;
pub const COIN_GOLD_COLOR: Color = Color::srgb(0.95, 0.68, 0.18);

#[derive(Component, Clone, Debug)]
pub struct CharacterCoin {
    pub character_id: u32,
    pub character: CharacterDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterCoinParam {
    pub character_id: u32,

    #[serde(default)]
    pub scene_param: SceneParam,
}

pub fn spawn_character_coin<M: Component>(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    config: &GameConfig,
    character_config: &CharacterConfig,
    param: &CharacterCoinParam,
    marker: M,
) -> Option<Entity> {
    let Some(character) = character_config.get(param.character_id).cloned() else {
        warn!(
            "failed to spawn character coin: character id {} is not configured",
            param.character_id
        );
        return None;
    };

    let transform = Transform::from_translation(
        param
            .scene_param
            .position
            .extend(SceneLayer::Coin.get_layer_base_z() + param.scene_param.order),
    )
    .with_rotation(Quat::from_rotation_z(param.scene_param.rotation));
    let image_path = normalize_asset_path(&character.coin_portrait_image_path);

    let entity = commands
        .spawn((
            Mesh2d(meshes.add(Circle::new(config.visuals.player_radius))),
            MeshMaterial2d(materials.add(COIN_GOLD_COLOR)),
            transform,
            CharacterCoin {
                character_id: param.character_id,
                character,
            },
            Pickable::IGNORE,
            marker,
        ))
        .id();

    commands.entity(entity).with_children(|parent| {
        spawn_coin_visual_layers(parent, asset_server, config, &image_path);
    });

    Some(entity)
}

pub fn spawn_coin_visual_layers(
    parent: &mut ChildSpawnerCommands<'_>,
    asset_server: &AssetServer,
    config: &GameConfig,
    portrait_image_path: &str,
) {
    let size = coin_visual_size(config);
    parent.spawn((
        Sprite {
            image: asset_server.load(COIN_BACKGROUND_IMAGE_PATH),
            custom_size: Some(size),
            ..default()
        },
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, COIN_BACKGROUND_Z_OFFSET),
        Pickable::IGNORE,
    ));

    if !portrait_image_path.is_empty() {
        parent.spawn((
            Sprite {
                image: asset_server.load(portrait_image_path.to_string()),
                custom_size: Some(size),
                ..default()
            },
            Anchor::CENTER,
            Transform::from_xyz(0.0, 0.0, COIN_PORTRAIT_Z_OFFSET),
            Pickable::IGNORE,
        ));
    }
}

pub fn coin_visual_size(config: &GameConfig) -> Vec2 {
    Vec2::splat(config.visuals.player_radius * 2.0)
}

pub fn character_coin_to_param(coin: &CharacterCoin, transform: &Transform) -> CharacterCoinParam {
    CharacterCoinParam {
        character_id: coin.character_id,
        scene_param: SceneParam {
            position: transform.translation.truncate(),
            rotation: transform.rotation.to_euler(EulerRot::XYZ).2,
            order: transform.translation.z - SceneLayer::Coin.get_layer_base_z(),
            enable_if: None,
            disable_if: None,
            description: String::new(),
        },
    }
}
